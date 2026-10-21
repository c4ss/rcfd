//! Lumped zone components (F-113, ADR 0012): `components.<name>` of type `fan`, `porous_jump`, `heat_exchanger` or
//! `network_outlet`. The physics helpers are in `rcfd_fv::lumped`. A fan or a porous jump is a pressure jump on the
//! downstream boundary plane of its zone (`FaceJump` of the SIMPLE kernel, ADR 0012); the zone only locates the plane
//! and the flow through it.

use crate::case::{Case, Diagnostic};
use crate::sources::{check_zone, gf, gs, gv, zone_cells};
use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::hx::{self, Arrangement};
use rcfd_fv::lumped::{self as lz, FaceJump, FanCurve, ZoneFlow};
use rcfd_fv::{FvGeometry, ScalarField};

fn getb(c: &Case, p: &str) -> bool {
    c.tree.get(p).ok().and_then(|v| v.as_bool()).unwrap_or(false)
}

fn list(c: &Case, p: &str) -> Vec<f64> {
    c.tree
        .get(p)
        .ok()
        .and_then(|v| v.as_array().map(|a| a.iter().filter_map(|x| x.as_f64()).collect()))
        .unwrap_or_default()
}

/// Names of the configured components.
pub fn names(case: &Case) -> Vec<String> {
    case.tree.names("components").unwrap_or_default()
}

/// Is any component configured?
pub fn enabled(case: &Case) -> bool {
    !names(case).is_empty()
}

fn direction(case: &Case, p: &str) -> Result<Vec3, String> {
    match gv(case, p) {
        Some(Some(v)) if vec3::norm(v) > 0.0 => Ok(v),
        Some(Some(_)) => Err("the flow direction must not be the zero vector".into()),
        Some(None) => Err("expected 3 components [x, y, z]".into()),
        None => Err("this component needs a flow direction [x, y, z]".into()),
    }
}

/// Settings checks (values, curves, algorithm and energy requirements).
pub fn check(case: &Case) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let mut bad = |path: String, message: String| {
        out.push(Diagnostic {
            path,
            line: None,
            message,
        })
    };
    let alg = gs(case, "solver.algorithm");
    for n in names(case) {
        let p = |f: &str| format!("components.{n}.{f}");
        let kind = gs(case, &p("type"));
        if !matches!(alg.as_str(), "simple" | "simplec" | "piso" | "pimple") {
            bad(
                p("type"),
                format!("lumped components are available with the algorithms simple, simplec, piso and pimple, not with \"{alg}\" (it has no pressure-jump hook)"),
            );
        } else if matches!(alg.as_str(), "piso" | "pimple") {
            // transient hook (ADR 0045): not available with the fractional-step advance or a moving mesh
            if gs(case, "solver.time_advance") == "fractional_step" {
                bad(
                    p("type"),
                    "lumped components are not available with solver.time_advance = \"fractional_step\" (no outer iteration to re-evaluate them in; use iterative)".into(),
                );
            }
            if getb(case, "dynamic_mesh.enabled") {
                bad(
                    p("type"),
                    "lumped components are not available with dynamic_mesh.enabled (the mesh moves under the component planes)".into(),
                );
            }
        }
        if !matches!(kind.as_str(), "network_outlet" | "fan_boundary") {
            for d in check_zone(case, &format!("components.{n}")) {
                bad(d.path, d.message);
            }
            if let Err(m) = direction(case, &p("direction")) {
                bad(p("direction"), m);
            }
        }
        match kind.as_str() {
            "fan" => {
                if let Err(m) = FanCurve::new(list(case, &p("curve_flow")), list(case, &p("curve_pressure"))) {
                    bad(p("curve_flow"), m);
                }
            }
            "porous_jump" => {
                let (t, k, c2) = (
                    gf(case, &p("thickness")),
                    gf(case, &p("permeability")),
                    gf(case, &p("inertial_resistance")),
                );
                if t.is_nan() || t <= 0.0 {
                    bad(p("thickness"), "must be positive [m]".into());
                }
                if k < 0.0 || c2 < 0.0 {
                    bad(
                        if k < 0.0 {
                            p("permeability")
                        } else {
                            p("inertial_resistance")
                        },
                        "must not be negative".into(),
                    );
                }
                if k <= 0.0 && c2 <= 0.0 {
                    bad(
                        p("permeability"),
                        "a porous jump needs a positive permeability [m^2] and/or inertial_resistance [1/m]".into(),
                    );
                }
            }
            "heat_exchanger" => {
                if !getb(case, "energy.enabled") {
                    bad(
                        p("type"),
                        "a heat_exchanger needs the energy equation (energy.enabled = true)".into(),
                    );
                }
                for f in ["loss_coefficient", "loss_linear"] {
                    let v = gf(case, &p(f));
                    if v.is_nan() || v < 0.0 {
                        bad(p(f), "must not be negative (0 = none)".into());
                    }
                }
                let dual = getb(case, &p("dual_cell"));
                if dual && gs(case, &p("mode")) != "ntu" {
                    bad(p("dual_cell"), "a dual cell needs mode = \"ntu\"".into());
                }
                if gs(case, &p("mode")) == "ntu" {
                    if Arrangement::parse(&gs(case, &p("arrangement"))).is_none() {
                        bad(
                            p("arrangement"),
                            format!("unknown arrangement; use one of {}", Arrangement::NAMES.join(", ")),
                        );
                    }
                    let ua = gf(case, &p("ua"));
                    if !(ua > 0.0 && ua.is_finite()) {
                        bad(
                            p("ua"),
                            format!("set the conductance UA [W/K] (> 0); found {ua} (-1 means unset)"),
                        );
                    }
                    if dual {
                        let sp = |f: &str| format!("components.{n}.secondary.{f}");
                        for d in check_zone(case, &format!("components.{n}.secondary")) {
                            bad(d.path, d.message);
                        }
                        if let Err(m) = direction(case, &sp("direction")) {
                            bad(sp("direction"), m);
                        }
                        for f in ["loss_coefficient", "loss_linear"] {
                            let v = gf(case, &sp(f));
                            if v.is_nan() || v < 0.0 {
                                bad(sp(f), "must not be negative (0 = none)".into());
                            }
                        }
                    } else {
                        if gf(case, &p("secondary_capacity_rate")) < 0.0 {
                            bad(
                                p("secondary_capacity_rate"),
                                "must not be negative (0 = infinite)".into(),
                            );
                        }
                        if gf(case, &p("secondary_temperature")) <= 0.0 {
                            bad(p("secondary_temperature"), "must be a positive temperature [K]".into());
                        }
                    }
                } else if gs(case, &p("mode")) == "effectiveness" {
                    let e = gf(case, &p("effectiveness"));
                    if !(e > 0.0 && e <= 1.0) {
                        bad(p("effectiveness"), "must be in (0, 1]".into());
                    }
                    if gf(case, &p("secondary_capacity_rate")) < 0.0 {
                        bad(
                            p("secondary_capacity_rate"),
                            "must not be negative (0 = infinite)".into(),
                        );
                    }
                    if gf(case, &p("secondary_temperature")) <= 0.0 {
                        bad(p("secondary_temperature"), "must be a positive temperature [K]".into());
                    }
                } else if !gf(case, &p("duty")).is_finite() {
                    bad(p("duty"), "must be finite".into());
                }
            }
            "fan_boundary" => {
                if let Err(m) = FanCurve::new(list(case, &p("curve_flow")), list(case, &p("curve_pressure"))) {
                    bad(p("curve_flow"), m);
                }
                let patch = gs(case, &p("patch"));
                if patch.is_empty() {
                    bad(p("patch"), "a fan_boundary needs the name of a boundary patch".into());
                } else if case.tree.get(&format!("boundaries.{patch}.pressure")).is_err() {
                    bad(p("patch"), format!("no boundary '{patch}' is defined"));
                } else if gs(case, &format!("boundaries.{patch}.pressure")) != "fixed" {
                    bad(
                        p("patch"),
                        format!("boundaries.{patch}.pressure must be \"fixed\" (the fan curve sets its value)"),
                    );
                }
            }
            "network_outlet" => {
                let patch = gs(case, &p("patch"));
                if patch.is_empty() {
                    bad(p("patch"), "a network_outlet needs the name of a boundary patch".into());
                } else if case.tree.get(&format!("boundaries.{patch}.pressure")).is_err() {
                    bad(p("patch"), format!("no boundary '{patch}' is defined"));
                } else if gs(case, &format!("boundaries.{patch}.pressure")) != "fixed" {
                    bad(
                        p("patch"),
                        format!("boundaries.{patch}.pressure must be \"fixed\" (the network sets its value)"),
                    );
                }
                if gf(case, &p("resistance_linear")) < 0.0 || gf(case, &p("resistance_quadratic")) < 0.0 {
                    bad(
                        p("resistance_linear"),
                        "the network resistances must not be negative".into(),
                    );
                }
            }
            _ => {}
        }
    }
    out
}

enum Kind {
    Fan {
        curve: FanCurve,
        dp_used: Option<f64>,
    },
    Jump {
        nu: f64,
        mu: f64,
        alpha: f64,
        c2: f64,
        t: f64,
    },
    Hx {
        effectiveness: Option<(f64, f64, f64)>,
        duty: f64,
        last_duty: f64,
        last_t_in: f64,
        /// NTU model (`mode = "ntu"`).
        ntu: Option<NtuModel>,
        /// Pressure-loss closure of the primary zone: linear coefficient [Pa s/m] and loss coefficient.
        loss: (f64, f64),
        last_eps: f64,
        last_ntu: f64,
    },
    FanBoundary {
        patch: String,
        p_ref: f64,
        curve: FanCurve,
        p_used: Option<f64>,
    },
    Network {
        patch: String,
        p_ref: f64,
        r1: f64,
        r2: f64,
        p_used: f64,
    },
}

/// Effectiveness-NTU exchanger of a heat-exchanger component (ADR 0039 D3).
struct NtuModel {
    arrangement: Arrangement,
    ua: f64,
    t_secondary: f64,
    c_secondary: f64,
    /// Second flow-through zone (dual cell).
    secondary: Option<Secondary>,
}

/// Secondary zone of a dual-cell exchanger.
struct Secondary {
    zone: Vec<bool>,
    flow: ZoneFlow,
    loss: (f64, f64),
    last_q: f64,
    last_t_in: f64,
    last_duty: f64,
}

struct Item {
    name: String,
    kind: Kind,
    zone: Vec<bool>,
    flow: Option<ZoneFlow>,
    relax: f64,
    last_q: f64,
    last_dp: f64,
}

/// Configured components.
pub struct Components {
    items: Vec<Item>,
    rho: f64,
    cp: f64,
}

impl Components {
    /// Build from the case; `None` when none is configured.
    pub fn from_case(case: &Case, geom: &FvGeometry) -> Result<Option<Components>, Vec<Diagnostic>> {
        if !enabled(case) {
            return Ok(None);
        }
        let (rho, nu) = (gf(case, "physics.rho"), gf(case, "physics.nu"));
        let mut items = Vec::new();
        let mut errs = Vec::new();
        for n in names(case) {
            let p = |f: &str| format!("components.{n}.{f}");
            let kind_s = gs(case, &p("type"));
            let relax = gf(case, &p("relax")).clamp(0.01, 1.0);
            if kind_s == "fan_boundary" {
                let patch = gs(case, &p("patch"));
                if geom.mesh.patch(&patch).is_none() {
                    errs.push(Diagnostic {
                        path: p("patch"),
                        line: None,
                        message: format!("the mesh has no patch '{patch}'"),
                    });
                    continue;
                }
                let curve = match FanCurve::new(list(case, &p("curve_flow")), list(case, &p("curve_pressure"))) {
                    Ok(c) => c,
                    Err(m) => {
                        errs.push(Diagnostic {
                            path: p("curve_flow"),
                            line: None,
                            message: m,
                        });
                        continue;
                    }
                };
                items.push(Item {
                    name: n.clone(),
                    kind: Kind::FanBoundary {
                        patch,
                        p_ref: gf(case, &p("reference_pressure")),
                        curve,
                        p_used: None,
                    },
                    zone: Vec::new(),
                    flow: None,
                    relax,
                    last_q: 0.0,
                    last_dp: 0.0,
                });
                continue;
            }
            if kind_s == "network_outlet" {
                let patch = gs(case, &p("patch"));
                if geom.mesh.patch(&patch).is_none() {
                    errs.push(Diagnostic {
                        path: p("patch"),
                        line: None,
                        message: format!("the mesh has no patch '{patch}'"),
                    });
                    continue;
                }
                items.push(Item {
                    name: n.clone(),
                    kind: Kind::Network {
                        patch,
                        p_ref: gf(case, &p("reference_pressure")),
                        r1: gf(case, &p("resistance_linear")),
                        r2: gf(case, &p("resistance_quadratic")),
                        p_used: gf(case, &p("reference_pressure")),
                    },
                    zone: Vec::new(),
                    flow: None,
                    relax,
                    last_q: 0.0,
                    last_dp: 0.0,
                });
                continue;
            }
            let zone = match zone_cells(case, &format!("components.{n}"), geom, None) {
                Ok(z) => z,
                Err(d) => {
                    errs.push(d);
                    continue;
                }
            };
            let dir = match direction(case, &p("direction")) {
                Ok(d) => vec3::unit(d),
                Err(m) => {
                    errs.push(Diagnostic {
                        path: p("direction"),
                        line: None,
                        message: m,
                    });
                    continue;
                }
            };
            let flow = match ZoneFlow::new(geom, &zone, dir) {
                Ok(f) => f,
                Err(m) => {
                    errs.push(Diagnostic {
                        path: p("zone"),
                        line: None,
                        message: m,
                    });
                    continue;
                }
            };
            let has_loss = kind_s == "heat_exchanger"
                && (gf(case, &p("loss_coefficient")) > 0.0 || gf(case, &p("loss_linear")) > 0.0);
            if (matches!(kind_s.as_str(), "fan" | "porous_jump") || has_loss)
                && flow.exit.iter().all(|&(f, _)| f >= geom.n_internal)
            {
                errs.push(Diagnostic {
                    path: p("zone"),
                    line: None,
                    message: "the downstream side of the zone must be an interior plane of the mesh (a fan or porous jump cannot sit on a boundary patch)".into(),
                });
                continue;
            }
            let kind = match kind_s.as_str() {
                "fan" => match FanCurve::new(list(case, &p("curve_flow")), list(case, &p("curve_pressure"))) {
                    Ok(curve) => Kind::Fan { curve, dp_used: None },
                    Err(m) => {
                        errs.push(Diagnostic {
                            path: p("curve_flow"),
                            line: None,
                            message: m,
                        });
                        continue;
                    }
                },
                "porous_jump" => Kind::Jump {
                    nu,
                    mu: rho * nu,
                    alpha: gf(case, &p("permeability")),
                    c2: gf(case, &p("inertial_resistance")),
                    t: gf(case, &p("thickness")),
                },
                _ => {
                    let mut ntu = None;
                    if gs(case, &p("mode")) == "ntu" {
                        let mut secondary = None;
                        if getb(case, &p("dual_cell")) {
                            let sp = format!("components.{n}.secondary");
                            let sz = match zone_cells(case, &sp, geom, None) {
                                Ok(z) => z,
                                Err(d) => {
                                    errs.push(d);
                                    continue;
                                }
                            };
                            let sdir = match direction(case, &format!("{sp}.direction")) {
                                Ok(d) => vec3::unit(d),
                                Err(m) => {
                                    errs.push(Diagnostic {
                                        path: format!("{sp}.direction"),
                                        line: None,
                                        message: m,
                                    });
                                    continue;
                                }
                            };
                            if sz.iter().zip(&zone).any(|(a, b)| *a && *b) {
                                errs.push(Diagnostic {
                                    path: format!("{sp}.zone"),
                                    line: None,
                                    message:
                                        "the secondary zone of a dual cell must not share cells with the primary zone"
                                            .into(),
                                });
                                continue;
                            }
                            let sflow = match ZoneFlow::new(geom, &sz, sdir) {
                                Ok(f) => f,
                                Err(m) => {
                                    errs.push(Diagnostic {
                                        path: format!("{sp}.zone"),
                                        line: None,
                                        message: m,
                                    });
                                    continue;
                                }
                            };
                            let sloss = (
                                gf(case, &format!("{sp}.loss_linear")),
                                gf(case, &format!("{sp}.loss_coefficient")),
                            );
                            if (sloss.0 > 0.0 || sloss.1 > 0.0) && sflow.exit.iter().all(|&(f, _)| f >= geom.n_internal)
                            {
                                errs.push(Diagnostic {
                                    path: format!("{sp}.zone"),
                                    line: None,
                                    message: "a pressure-loss closure needs an interior plane on the downstream side of the zone"
                                        .into(),
                                });
                                continue;
                            }
                            secondary = Some(Secondary {
                                zone: sz,
                                flow: sflow,
                                loss: sloss,
                                last_q: 0.0,
                                last_t_in: 0.0,
                                last_duty: 0.0,
                            });
                        }
                        ntu = Some(NtuModel {
                            arrangement: Arrangement::parse(&gs(case, &p("arrangement")))
                                .unwrap_or(Arrangement::Counterflow),
                            ua: gf(case, &p("ua")),
                            t_secondary: gf(case, &p("secondary_temperature")),
                            c_secondary: gf(case, &p("secondary_capacity_rate")),
                            secondary,
                        });
                    }
                    Kind::Hx {
                        effectiveness: (gs(case, &p("mode")) == "effectiveness").then(|| {
                            (
                                gf(case, &p("effectiveness")),
                                gf(case, &p("secondary_temperature")),
                                gf(case, &p("secondary_capacity_rate")),
                            )
                        }),
                        duty: gf(case, &p("duty")),
                        last_duty: 0.0,
                        last_t_in: 0.0,
                        ntu,
                        loss: (gf(case, &p("loss_linear")), gf(case, &p("loss_coefficient"))),
                        last_eps: 0.0,
                        last_ntu: 0.0,
                    }
                }
            };
            items.push(Item {
                name: n.clone(),
                kind,
                zone,
                flow: Some(flow),
                relax,
                last_q: 0.0,
                last_dp: 0.0,
            });
        }
        if !errs.is_empty() {
            return Err(errs);
        }
        Ok(Some(Components {
            items,
            rho,
            cp: gf(case, "physics.cp"),
        }))
    }

    /// Pressure jumps of the fans and porous jumps for the current flux; `None` without any. The jump sits on the
    /// internal faces of the downstream boundary of each zone: a fan raises the kinematic pressure by `dp/rho`, a porous
    /// jump resists with `K_f |phi|` (`dp = (mu/alpha) t v + C2 t rho v |v|/2`, `v = phi/|S_f|`, linear part implicit).
    pub fn face_jump(&mut self, geom: &FvGeometry, phi: &[f64]) -> Option<FaceJump> {
        let mut jump: Option<FaceJump> = None;
        let sf = geom.mesh.face_area_vectors();
        for it in &mut self.items {
            let Some(flow) = &it.flow else { continue };
            let q = flow.flow(phi);
            it.last_q = q;
            match &mut it.kind {
                Kind::Fan { curve, dp_used } => {
                    // the first evaluation takes the curve value of the initial flow, later ones are relaxed
                    let used = match *dp_used {
                        Some(d) => d + it.relax * (curve.eval(q) - d),
                        None => it.relax * curve.eval(q),
                    };
                    *dp_used = Some(used);
                    it.last_dp = used;
                    let j = jump.get_or_insert_with(|| FaceJump::zeros(geom.mesh.n_faces()));
                    for &(f, sign) in flow.exit.iter().filter(|&&(f, _)| f < geom.n_internal) {
                        j.rise[f] += sign * used / self.rho;
                    }
                }
                Kind::Jump { nu, mu, alpha, c2, t } => {
                    let j = jump.get_or_insert_with(|| FaceJump::zeros(geom.mesh.n_faces()));
                    for &(f, _) in flow.exit.iter().filter(|&&(f, _)| f < geom.n_internal) {
                        let a = vec3::norm(sf[f]);
                        let visc = if *alpha > 0.0 { *nu / *alpha * *t / a } else { 0.0 };
                        j.resist[f] += visc + *c2 * *t * phi[f].abs() / (2.0 * a * a);
                    }
                    it.last_dp = lz::porous_jump_dp(*mu, self.rho, *alpha, *c2, *t, q / flow.area);
                }
                Kind::Hx { loss, ntu, .. } => {
                    let mut planes: Vec<(&ZoneFlow, (f64, f64))> = vec![(flow, *loss)];
                    if let Some(sec) = ntu.as_mut().and_then(|m| m.secondary.as_mut()) {
                        sec.last_q = sec.flow.flow(phi);
                        planes.push((&sec.flow, sec.loss));
                    }
                    for (pl, (r1, k)) in planes {
                        if r1 <= 0.0 && k <= 0.0 {
                            continue;
                        }
                        let j = jump.get_or_insert_with(|| FaceJump::zeros(geom.mesh.n_faces()));
                        for &(f, _) in pl.exit.iter().filter(|&&(f, _)| f < geom.n_internal) {
                            let a = vec3::norm(sf[f]);
                            j.resist[f] += r1 / (self.rho * a) + k * phi[f].abs() / (2.0 * a * a);
                        }
                    }
                    it.last_dp = hx::core_dp(loss.0, loss.1, self.rho, q / flow.area);
                }
                _ => {}
            }
        }
        jump
    }

    /// Heat-exchanger energy source (K/s per cell) for the current flux and temperature, `None` without one.
    pub fn heat_sources(&mut self, geom: &FvGeometry, phi: &[f64], t: &ScalarField) -> Option<Vec<f64>> {
        let mut src: Option<Vec<f64>> = None;
        let rho_cp = self.rho * self.cp;
        let tb = t.boundary_values(geom);
        for it in &mut self.items {
            let (
                Some(flow),
                Kind::Hx {
                    effectiveness,
                    duty,
                    last_duty,
                    last_t_in,
                    ntu,
                    last_eps,
                    last_ntu,
                    ..
                },
            ) = (&it.flow, &mut it.kind)
            else {
                continue;
            };
            let q = flow.flow(phi);
            it.last_q = q;
            let mean_t = |zone: &[bool]| {
                let (mut s, mut n) = (0.0, 0.0);
                for (c, &z) in zone.iter().enumerate() {
                    if z {
                        s += t.values[c];
                        n += 1.0;
                    }
                }
                s / n
            };
            let t_in = flow
                .inlet_temperature(geom, phi, &t.values, &tb)
                .unwrap_or_else(|| mean_t(&it.zone));
            *last_t_in = t_in;
            let mut sec_heat: Option<(f64, usize)> = None;
            let heat = if let Some(m) = ntu {
                let c_p = rho_cp * q.max(0.0);
                let (c_s, t_s) = match &mut m.secondary {
                    Some(sec) => {
                        let qs = sec.flow.flow(phi);
                        sec.last_q = qs;
                        let ts = sec
                            .flow
                            .inlet_temperature(geom, phi, &t.values, &tb)
                            .unwrap_or_else(|| mean_t(&sec.zone));
                        sec.last_t_in = ts;
                        (rho_cp * qs.max(0.0), ts)
                    }
                    None => (m.c_secondary, m.t_secondary),
                };
                // a dual cell without secondary flow transfers no heat (zero capacity rate is not "infinite" there)
                let r = if m.secondary.is_some() && c_s <= 0.0 {
                    hx::solve_exchanger(m.arrangement, 0.0, 0.0, m.ua, t_in, t_s)
                } else {
                    hx::solve_exchanger(m.arrangement, c_p, c_s, m.ua, t_in, t_s)
                };
                *last_eps = r.eps;
                *last_ntu = r.ntu;
                if let Some(sec) = &mut m.secondary {
                    sec.last_duty = -r.q;
                    sec_heat = Some((-r.q, 0));
                }
                r.q
            } else {
                match effectiveness {
                    Some((eps, t_sec, c_sec)) => {
                        lz::effectiveness_duty(*eps, rho_cp * q.max(0.0), *c_sec, *t_sec, t_in)
                    }
                    None => *duty,
                }
            };
            *last_duty = heat;
            let s = src.get_or_insert_with(|| vec![0.0; geom.n_cells()]);
            let per_vol = heat / (rho_cp * flow.volume);
            for (c, &z) in it.zone.iter().enumerate() {
                if z {
                    s[c] += per_vol;
                }
            }
            if let (Some((h, _)), Some(m)) = (sec_heat, ntu.as_ref()) {
                if let Some(sec) = &m.secondary {
                    let per_vol = h / (rho_cp * sec.flow.volume);
                    for (c, &z) in sec.zone.iter().enumerate() {
                        if z {
                            s[c] += per_vol;
                        }
                    }
                }
            }
        }
        src
    }

    /// New fixed pressure (kinematic) of every network outlet patch for the current flux.
    pub fn network_pressures(&mut self, geom: &FvGeometry, phi: &[f64]) -> Vec<(String, f64)> {
        let mut out = Vec::new();
        for it in &mut self.items {
            if let Kind::Network {
                patch,
                p_ref,
                r1,
                r2,
                p_used,
            } = &mut it.kind
            {
                let q: f64 = geom
                    .mesh
                    .patch(patch)
                    .map(|p| p.faces().filter(|&f| !geom.empty[f]).map(|f| phi[f]).sum())
                    .unwrap_or(0.0);
                it.last_q = q;
                *p_used += it.relax * (lz::network_pressure(*p_ref, *r1, *r2, q) - *p_used);
                it.last_dp = *p_used;
                out.push((patch.clone(), *p_used / self.rho));
            }
            if let Kind::FanBoundary {
                patch,
                p_ref,
                curve,
                p_used,
            } = &mut it.kind
            {
                // volume flow INTO the domain through the patch
                let q_in: f64 = geom
                    .mesh
                    .patch(patch)
                    .map(|p| -p.faces().filter(|&f| !geom.empty[f]).map(|f| phi[f]).sum::<f64>())
                    .unwrap_or(0.0);
                it.last_q = q_in;
                // the first evaluation takes the curve value at the initial flow (no relaxation from the reference pressure)
                let target = *p_ref + curve.eval(q_in);
                let cur = p_used.unwrap_or(target);
                let new = cur + it.relax * (target - cur);
                *p_used = Some(new);
                it.last_dp = new - *p_ref;
                out.push((patch.clone(), new / self.rho));
            }
        }
        out
    }

    /// Does any component need the temperature field?
    pub fn has_heat_exchanger(&self) -> bool {
        self.items.iter().any(|i| matches!(i.kind, Kind::Hx { .. }))
    }

    /// Monitor values of the last evaluation: `<name>_flow` [m^3/s] and `<name>_dp` [Pa] (fan: pressure rise; porous
    /// jump: analytic drop at the current flow; network: patch pressure) / `<name>_duty` [W], `<name>_t_in` [K].
    pub fn values(&self) -> Vec<(String, f64)> {
        let mut v = Vec::new();
        for it in &self.items {
            v.push((format!("{}_flow", it.name), it.last_q));
            match &it.kind {
                Kind::Hx {
                    last_duty,
                    last_t_in,
                    ntu,
                    last_eps,
                    last_ntu,
                    ..
                } => {
                    v.push((format!("{}_duty", it.name), *last_duty));
                    v.push((format!("{}_t_in", it.name), *last_t_in));
                    v.push((format!("{}_dp", it.name), it.last_dp));
                    if let Some(m) = ntu {
                        v.push((format!("{}_eps", it.name), *last_eps));
                        v.push((format!("{}_ntu", it.name), *last_ntu));
                        if let Some(sec) = &m.secondary {
                            v.push((format!("{}_sec_flow", it.name), sec.last_q));
                            v.push((format!("{}_sec_t_in", it.name), sec.last_t_in));
                            v.push((format!("{}_sec_duty", it.name), sec.last_duty));
                        }
                    }
                }
                _ => v.push((format!("{}_dp", it.name), it.last_dp)),
            }
        }
        v
    }
}
