//! Lumped zone components (F-113, ADR 0012): `components.<name>` of type `fan`, `porous_jump`, `heat_exchanger` or
//! `network_outlet`. The physics helpers are in `rcfd_fv::lumped`. A fan or a porous jump is a pressure jump on the
//! downstream boundary plane of its zone (`FaceJump` of the SIMPLE kernel, ADR 0012); the zone only locates the plane
//! and the flow through it.

use crate::case::{Case, Diagnostic};
use crate::sources::{check_zone, gf, gs, gv, zone_cells};
use rcfd_core::vec3::{self, Vec3};
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
        if !matches!(alg.as_str(), "simple" | "simplec") {
            bad(
                p("type"),
                format!("lumped components are available with the steady algorithms (simple, simplec) only, not with \"{alg}\""),
            );
        }
        if kind != "network_outlet" {
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
                if gs(case, &p("mode")) == "effectiveness" {
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
    },
    Network {
        patch: String,
        p_ref: f64,
        r1: f64,
        r2: f64,
        p_used: f64,
    },
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
            let zone = match zone_cells(case, &format!("components.{n}"), geom) {
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
            if matches!(kind_s.as_str(), "fan" | "porous_jump") && flow.exit.iter().all(|&(f, _)| f >= geom.n_internal)
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
                _ => Kind::Hx {
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
                },
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
                },
            ) = (&it.flow, &mut it.kind)
            else {
                continue;
            };
            let q = flow.flow(phi);
            it.last_q = q;
            let t_in = flow.inlet_temperature(geom, phi, &t.values, &tb).unwrap_or_else(|| {
                let (mut s, mut n) = (0.0, 0.0);
                for (c, &z) in it.zone.iter().enumerate() {
                    if z {
                        s += t.values[c];
                        n += 1.0;
                    }
                }
                s / n
            });
            *last_t_in = t_in;
            let heat = match effectiveness {
                Some((eps, t_sec, c_sec)) => lz::effectiveness_duty(*eps, rho_cp * q.max(0.0), *c_sec, *t_sec, t_in),
                None => *duty,
            };
            *last_duty = heat;
            let s = src.get_or_insert_with(|| vec![0.0; geom.n_cells()]);
            let per_vol = heat / (rho_cp * flow.volume);
            for (c, &z) in it.zone.iter().enumerate() {
                if z {
                    s[c] += per_vol;
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
                    last_duty, last_t_in, ..
                } => {
                    v.push((format!("{}_duty", it.name), *last_duty));
                    v.push((format!("{}_t_in", it.name), *last_t_in));
                }
                _ => v.push((format!("{}_dp", it.name), it.last_dp)),
            }
        }
        v
    }
}
