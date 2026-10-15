//! Lagrangian particle tracking in the settings tree (F-056, ADR 0028 D9).
//!
//! The numerics live in `rcfd-particles`; this module reads and checks the `particles` section, builds the tracker on the
//! simulation mesh and runs it on a frozen (converged or prescribed) velocity field or alongside a transient flow. The flow
//! is never modified (one-way coupling). Every numeric key has an explicit unset sentinel (`-1`) with a located error when it
//! is needed and unset.

#![allow(clippy::neg_cmp_op_on_partial_ord)]

use crate::case::{Case, Diagnostic};
use crate::sim::Simulation;
use rcfd_core::settings::Schema;
use rcfd_core::vec3::Vec3;
use rcfd_particles::vtp::write_vtp;
use rcfd_particles::{
    DragLaw, Flow, InjKind, InjectorSpec, Interp, Options, Scheme, SizeDist, Tracker, VelMode, WallAction,
};

/// Sentinel of the numeric keys that are not set.
pub const UNSET: f64 = -1.0;

fn d(path: impl Into<String>, message: impl Into<String>) -> Diagnostic {
    Diagnostic {
        path: path.into(),
        line: None,
        message: message.into(),
    }
}

fn gf(c: &Case, p: &str) -> f64 {
    c.tree.get(p).ok().and_then(|v| v.as_f64()).unwrap_or(0.0)
}

fn gs(c: &Case, p: &str) -> String {
    c.tree
        .get(p)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default()
}

fn gb(c: &Case, p: &str) -> bool {
    c.tree.get(p).ok().and_then(|v| v.as_bool()).unwrap_or(false)
}

fn gi(c: &Case, p: &str) -> i64 {
    c.tree.get(p).ok().and_then(|v| v.as_i64()).unwrap_or(0)
}

/// A list setting: `None` when the list is empty, `Err(len)` for a wrong length.
fn vec3_of(c: &Case, p: &str) -> std::result::Result<Option<Vec3>, usize> {
    let v = c.tree.get(p).ok();
    let a = v.as_ref().and_then(|v| v.as_array()).cloned().unwrap_or_default();
    match a.len() {
        0 => Ok(None),
        3 => Ok(Some([
            a[0].as_f64().unwrap_or(f64::NAN),
            a[1].as_f64().unwrap_or(f64::NAN),
            a[2].as_f64().unwrap_or(f64::NAN),
        ])),
        n => Err(n),
    }
}

/// Is the particle section enabled?
pub fn enabled(case: &Case) -> bool {
    gb(case, "particles.enabled")
}

/// Names of the configured injections.
pub fn injection_names(case: &Case) -> Vec<String> {
    case.tree.names("particles.injection").unwrap_or_default()
}

/// Schema of the `particles` section.
pub fn schema() -> Schema {
    let injection = Schema::named(
        Schema::object()
            .doc("One particle injection (ADR 0028 D7)")
            .field(
                "kind",
                Schema::enumeration("unset", &["unset", "point", "surface"])
                    .doc("point: all parcels at `position`; surface: uniformly over the faces of the boundary patch `patch`. \"unset\" is an error"),
            )
            .field("position", Schema::list(Schema::float(0.0)).doc("kind = point: position [x, y, z] in metres (3 components)"))
            .field("patch", Schema::string("").doc("kind = surface: name of the boundary patch"))
            .field(
                "rate",
                Schema::float(UNSET).doc("Parcels per second; must be set (> 0), -1 = unset. The number injected up to time t is floor(rate (t - start_time))"),
            )
            .field("start_time", Schema::float(0.0).at_least(0.0).doc("Start of injection [s]"))
            .field(
                "end_time",
                Schema::float(UNSET).doc("End of injection [s], inclusive; -1 = particles.end_time"),
            )
            .field(
                "velocity_mode",
                Schema::enumeration("given", &["given", "fluid"])
                    .doc("given: initial velocity `velocity`; fluid: the local fluid velocity"),
            )
            .field(
                "velocity",
                Schema::list(Schema::float(0.0)).doc("velocity_mode = given: initial velocity [m/s] (3 components; empty list = at rest)"),
            )
            .field("parcel_count", Schema::float(1.0).doc("Real particles represented by one parcel (> 0)"))
            .field(
                "distribution",
                Schema::enumeration("constant", &["constant", "rosin_rammler"])
                    .doc("Diameter distribution: constant (`diameter`) or Rosin-Rammler (oversize mass fraction exp(-(d/rr_d_bar)^rr_n))"),
            )
            .field("diameter", Schema::float(UNSET).doc("distribution = constant: diameter [m] (> 0), -1 = unset"))
            .field("rr_d_bar", Schema::float(UNSET).doc("Rosin-Rammler size constant [m] (> 0), -1 = unset"))
            .field("rr_n", Schema::float(UNSET).doc("Rosin-Rammler spread parameter (> 0), -1 = unset"))
            .field("d_min", Schema::float(UNSET).doc("Rosin-Rammler lower truncation [m]; -1 = none"))
            .field("d_max", Schema::float(UNSET).doc("Rosin-Rammler upper truncation [m]; -1 = none")),
    )
    .doc("Particle injections by name");
    let walls = Schema::named(
        Schema::object()
            .doc("Particle action on one boundary patch")
            .field(
                "interaction",
                Schema::enumeration("default", &["default", "reflect", "stick", "escape"])
                    .doc("default: reflect on walls (particles.wall_interaction), escape at inlets, outlets and generic patches, specular reflection on symmetry planes"),
            )
            .field(
                "restitution",
                Schema::float(UNSET).doc("Restitution of a reflecting patch in [0, 1]; -1 = particles.restitution"),
            ),
    )
    .doc("Per-patch particle boundary actions");
    Schema::object()
        .doc("One-way coupled Lagrangian particle tracking (ADR 0028). Point particles with drag, gravity and buoyancy on a frozen or transient flow field; no effect on the flow")
        .field("enabled", Schema::boolean(false).doc("Enable the particle section"))
        .field("density", Schema::float(UNSET).doc("Particle density [kg/m^3] (> 0); -1 = unset (an error when enabled)"))
        .field(
            "drag",
            Schema::enumeration("schiller_naumann", &["schiller_naumann", "stokes", "none"])
                .doc("Drag law (Schiller-Naumann: 24/Re (1 + 0.15 Re^0.687) up to Re = 1000, 0.44 above; the switch at 1000 is unverified)"),
        )
        .field(
            "integration",
            Schema::enumeration("analytic", &["analytic", "euler", "rk2", "rk4"])
                .doc("analytic: exponential relaxation with frozen fluid velocity (exact for Stokes drag in a uniform field); euler, rk2 (midpoint), rk4"),
        )
        .field(
            "interpolation",
            Schema::enumeration("gradient", &["gradient", "cell_centre"])
                .doc("Fluid velocity at the particle: cell value plus (x - x_c) . grad u (least squares), or the cell value"),
        )
        .field("gravity", Schema::boolean(true).doc("Apply physics.gravity (with buoyancy (1 - rho_f/rho_p))"))
        .field(
            "dt",
            Schema::float(UNSET).doc("Tracking step [s] for track_frozen (-1 = solver.dt, which must then be positive); transient tracking uses the flow step"),
        )
        .field("end_time", Schema::float(UNSET).doc("End of tracking [s]; must be set (> 0), -1 = unset"))
        .field("seed", Schema::int(1).at_least(0.0).doc("Seed of the random numbers (injection positions and diameters); a run is reproducible bit for bit"))
        .field(
            "wall_interaction",
            Schema::enumeration("reflect", &["reflect", "stick", "escape"]).doc("Action at wall patches without an entry in `walls`"),
        )
        .field("restitution", Schema::float(1.0).range(0.0, 1.0).doc("Restitution coefficient of reflecting walls"))
        .field("walls", walls)
        .field("injection", injection)
        .field("output_file", Schema::string("").doc("VTK PolyData (.vtp) file of the particles written at the end; empty = none"))
        .field(
            "output_every",
            Schema::int(0).at_least(0.0).doc("Also write <output_file stem>_<n>.vtp every n tracking steps (0 = only the final file)"),
        )
}

/// Settings checks (no mesh needed).
pub fn check(case: &Case) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    if !enabled(case) {
        return out;
    }
    if gf(case, "particles.density") <= 0.0 {
        out.push(d(
            "particles.density",
            "particle density must be set (> 0); -1 means unset",
        ));
    }
    if gf(case, "particles.end_time") <= 0.0 {
        out.push(d(
            "particles.end_time",
            "end of tracking must be set (> 0); -1 means unset",
        ));
    }
    let dt = gf(case, "particles.dt");
    if dt != UNSET && dt <= 0.0 {
        out.push(d(
            "particles.dt",
            "the tracking step must be positive (or -1 for solver.dt)",
        ));
    }
    if gf(case, "physics.rho") <= 0.0 || gf(case, "physics.nu") <= 0.0 {
        out.push(d(
            "physics.nu",
            "particles need positive physics.rho and physics.nu (constant fluid properties)",
        ));
    }
    if crate::vd::enabled(case) || crate::comp::enabled(case) {
        out.push(d(
            "particles.enabled",
            "particles are available with the incompressible constant-property solvers only (ADR 0028)",
        ));
    }
    let names = injection_names(case);
    if names.is_empty() {
        out.push(d(
            "particles.injection",
            "at least one injection is needed (particles.injection.<name>)",
        ));
    }
    if let Ok(w) = case.tree.names("particles.walls") {
        for n in w {
            let r = gf(case, &format!("particles.walls.{n}.restitution"));
            if r != UNSET && !(0.0..=1.0).contains(&r) {
                out.push(d(
                    format!("particles.walls.{n}.restitution"),
                    "restitution must be in [0, 1] (or -1 for particles.restitution)",
                ));
            }
        }
    }
    for n in &names {
        let p = |f: &str| format!("particles.injection.{n}.{f}");
        let kind = gs(case, &p("kind"));
        match kind.as_str() {
            "point" => match vec3_of(case, &p("position")) {
                Ok(Some(v)) if v.iter().all(|x| x.is_finite()) => {}
                Ok(_) => out.push(d(p("position"), "a point injection needs position = [x, y, z]")),
                Err(k) => out.push(d(p("position"), format!("expected 3 components [x, y, z], got {k}"))),
            },
            "surface" => {
                if gs(case, &p("patch")).is_empty() {
                    out.push(d(p("patch"), "a surface injection needs the name of a boundary patch"));
                }
            }
            _ => out.push(d(p("kind"), "injection kind must be set: \"point\" or \"surface\"")),
        }
        let rate = gf(case, &p("rate"));
        if !(rate > 0.0) {
            out.push(d(
                p("rate"),
                "injection rate [parcels/s] must be set (> 0); -1 means unset",
            ));
        }
        let t0 = gf(case, &p("start_time"));
        let t1 = gf(case, &p("end_time"));
        if t1 != UNSET && t1 < t0 {
            out.push(d(p("end_time"), format!("end_time {t1} is before start_time {t0}")));
        }
        if gf(case, &p("parcel_count")) <= 0.0 {
            out.push(d(p("parcel_count"), "parcel_count must be positive"));
        }
        if gs(case, &p("velocity_mode")) == "given" {
            if let Err(k) = vec3_of(case, &p("velocity")) {
                out.push(d(p("velocity"), format!("expected 3 components [x, y, z], got {k}")));
            }
        }
        match gs(case, &p("distribution")).as_str() {
            "rosin_rammler" => {
                if gf(case, &p("rr_d_bar")) <= 0.0 {
                    out.push(d(p("rr_d_bar"), "Rosin-Rammler size constant must be set (> 0)"));
                }
                if gf(case, &p("rr_n")) <= 0.0 {
                    out.push(d(p("rr_n"), "Rosin-Rammler spread parameter must be set (> 0)"));
                }
                let (lo, hi) = (gf(case, &p("d_min")), gf(case, &p("d_max")));
                if lo != UNSET && lo <= 0.0 {
                    out.push(d(p("d_min"), "d_min must be positive (or -1 for none)"));
                }
                if hi != UNSET && hi <= 0.0 {
                    out.push(d(p("d_max"), "d_max must be positive (or -1 for none)"));
                }
                if lo > 0.0 && hi > 0.0 && hi <= lo {
                    out.push(d(p("d_max"), "d_max must exceed d_min"));
                }
            }
            _ => {
                if gf(case, &p("diameter")) <= 0.0 {
                    out.push(d(p("diameter"), "particle diameter must be set (> 0); -1 means unset"));
                }
            }
        }
    }
    out
}

fn options(case: &Case) -> Options {
    let g = if gb(case, "particles.gravity") {
        vec3_of(case, "physics.gravity").ok().flatten().unwrap_or([0.0; 3])
    } else {
        [0.0; 3]
    };
    let wall = match gs(case, "particles.wall_interaction").as_str() {
        "stick" => WallAction::Stick,
        "escape" => WallAction::Escape,
        _ => WallAction::Reflect(gf(case, "particles.restitution")),
    };
    Options {
        drag: match gs(case, "particles.drag").as_str() {
            "stokes" => DragLaw::Stokes,
            "none" => DragLaw::None,
            _ => DragLaw::SchillerNaumann,
        },
        scheme: match gs(case, "particles.integration").as_str() {
            "euler" => Scheme::Euler,
            "rk2" => Scheme::Rk2,
            "rk4" => Scheme::Rk4,
            _ => Scheme::Analytic,
        },
        interp: match gs(case, "particles.interpolation").as_str() {
            "cell_centre" => Interp::CellCentre,
            _ => Interp::Gradient,
        },
        gravity: g,
        default_wall: wall,
        ..Options::default()
    }
}

/// Build the tracker on the mesh of a simulation, with all injectors. Mesh-dependent problems (unknown patches, points
/// outside the mesh, periodic patches) are reported as located diagnostics.
pub fn build_tracker(case: &Case, sim: &Simulation) -> std::result::Result<Tracker, Vec<Diagnostic>> {
    let mut errs = check(case);
    if !errs.is_empty() {
        return Err(errs);
    }
    let mesh = sim.geom.mesh.clone();
    let opts = options(case);
    let mut overrides = Vec::new();
    for n in case.tree.names("particles.walls").unwrap_or_default() {
        let p = |f: &str| format!("particles.walls.{n}.{f}");
        let e = match gf(case, &p("restitution")) {
            r if r == UNSET => gf(case, "particles.restitution"),
            r => r,
        };
        let a = match gs(case, &p("interaction")).as_str() {
            "reflect" => Some(WallAction::Reflect(e)),
            "stick" => Some(WallAction::Stick),
            "escape" => Some(WallAction::Escape),
            _ => None,
        };
        if mesh.patch(&n).is_none() {
            errs.push(d(
                format!("particles.walls.{n}"),
                format!("unknown boundary patch '{n}'"),
            ));
        } else if let Some(a) = a {
            overrides.push((n.clone(), a));
        } else if gf(case, &p("restitution")) != UNSET {
            // default interaction with a restitution: reflect with that value
            overrides.push((n.clone(), WallAction::Reflect(e)));
        }
    }
    if !errs.is_empty() {
        return Err(errs);
    }
    let mut tr = Tracker::new(mesh, opts, &overrides).map_err(|e| vec![d("particles", e.to_string())])?;
    let density = gf(case, "particles.density");
    let seed = gi(case, "particles.seed").max(0) as u64;
    let end = gf(case, "particles.end_time");
    for (k, n) in injection_names(case).iter().enumerate() {
        let p = |f: &str| format!("particles.injection.{n}.{f}");
        let kind = if gs(case, &p("kind")) == "point" {
            InjKind::Point(vec3_of(case, &p("position")).ok().flatten().unwrap_or([0.0; 3]))
        } else {
            InjKind::Surface(gs(case, &p("patch")))
        };
        let velocity = if gs(case, &p("velocity_mode")) == "fluid" {
            VelMode::Fluid
        } else {
            VelMode::Given(vec3_of(case, &p("velocity")).ok().flatten().unwrap_or([0.0; 3]))
        };
        let opt = |v: f64| if v == UNSET { None } else { Some(v) };
        let dist = if gs(case, &p("distribution")) == "rosin_rammler" {
            SizeDist::RosinRammler {
                d_bar: gf(case, &p("rr_d_bar")),
                n: gf(case, &p("rr_n")),
                d_min: opt(gf(case, &p("d_min"))),
                d_max: opt(gf(case, &p("d_max"))),
            }
        } else {
            SizeDist::Constant(gf(case, &p("diameter")))
        };
        let t_end = match gf(case, &p("end_time")) {
            t if t == UNSET => end,
            t => t,
        };
        let spec = InjectorSpec {
            name: n.clone(),
            kind,
            rate: gf(case, &p("rate")),
            t_start: gf(case, &p("start_time")),
            t_end,
            velocity,
            dist,
            density,
            parcel_count: gf(case, &p("parcel_count")),
            seed: seed.wrapping_mul(1_000_003).wrapping_add(k as u64),
        };
        tr.add_injector(spec).map_err(|e| {
            let field = match gs(case, &p("kind")).as_str() {
                "point" => "position",
                _ => "patch",
            };
            vec![d(p(field), e.to_string())]
        })?;
    }
    Ok(tr)
}

/// Fluid data of a simulation for the tracker (cell velocity, least-squares gradient, constant properties).
struct FlowData {
    u: Vec<Vec3>,
    grad: Vec<[Vec3; 3]>,
    rho: f64,
    mu: f64,
}

fn flow_data(case: &Case, sim: &Simulation) -> FlowData {
    let u = sim.velocity();
    let geom = &sim.geom;
    let bv = u.boundary_values(geom);
    let n = u.values.len();
    let mut grad = vec![[[0.0; 3]; 3]; n];
    for k in 0..3 {
        let cells: Vec<f64> = u.values.iter().map(|v| v[k]).collect();
        let b: Vec<f64> = bv.iter().map(|v| v[k]).collect();
        let g = rcfd_fv::gradient::least_squares(geom, &cells, &b);
        for c in 0..n {
            grad[c][k] = g[c];
        }
    }
    let rho = gf(case, "physics.rho");
    FlowData {
        u: u.values.clone(),
        grad,
        rho,
        mu: rho * gf(case, "physics.nu"),
    }
}

impl FlowData {
    fn flow(&self) -> Flow<'_> {
        Flow {
            u: &self.u,
            grad: Some(&self.grad),
            rho: self.rho,
            mu: self.mu,
        }
    }
}

fn io_diag(e: std::io::Error, path: &str) -> Vec<Diagnostic> {
    vec![d("particles.output_file", format!("cannot write '{path}': {e}"))]
}

fn write_series(case: &Case, tr: &Tracker, index: Option<usize>) -> std::result::Result<(), Vec<Diagnostic>> {
    let base = gs(case, "particles.output_file");
    if base.is_empty() {
        return Ok(());
    }
    let path = match index {
        None => base.clone(),
        Some(i) => {
            let stem = base.strip_suffix(".vtp").unwrap_or(&base);
            format!("{stem}_{i}.vtp")
        }
    };
    write_vtp(&path, &tr.particles).map_err(|e| io_diag(e, &path))
}

/// Track the particles of `case` on the current velocity field of `sim` (a prescribed `solver.frozen_flow` field or a
/// converged steady solution), from time 0 to `particles.end_time` in steps of `particles.dt`.
pub fn track_frozen(case: &Case, sim: &Simulation) -> std::result::Result<Tracker, Vec<Diagnostic>> {
    let mut tr = build_tracker(case, sim)?;
    let mut dt = gf(case, "particles.dt");
    if dt == UNSET {
        dt = gf(case, "solver.dt");
    }
    if !(dt > 0.0) {
        return Err(vec![d(
            "particles.dt",
            "set the tracking step (solver.dt is not positive either)",
        )]);
    }
    let end = gf(case, "particles.end_time");
    let every = gi(case, "particles.output_every").max(0) as usize;
    let fd = flow_data(case, sim);
    let flow = fd.flow();
    let mut step = 0usize;
    while tr.time < end * (1.0 - 1e-12) {
        let h = dt.min(end - tr.time);
        tr.advance(&flow, h).map_err(|e| vec![d("particles", e.to_string())])?;
        step += 1;
        if every > 0 && step % every == 0 {
            write_series(case, &tr, Some(step))?;
        }
    }
    write_series(case, &tr, None)?;
    Ok(tr)
}

/// Advance `sim` step by step (a transient algorithm) and track the particles over each flow step on the current field,
/// until `particles.end_time` or the end of the simulation.
pub fn track_transient(case: &Case, sim: &mut Simulation) -> std::result::Result<Tracker, Vec<Diagnostic>> {
    let mut tr = build_tracker(case, sim)?;
    if sim.is_steady() {
        return Err(vec![d(
            "solver.algorithm",
            "transient tracking needs a transient algorithm (piso or pimple); use track_frozen for steady flows",
        )]);
    }
    let end = gf(case, "particles.end_time");
    let every = gi(case, "particles.output_every").max(0) as usize;
    let mut step = 0usize;
    let mut t_prev = sim.time();
    while tr.time < end * (1.0 - 1e-12) && !sim.is_done() {
        sim.step();
        let dt = sim.time() - t_prev;
        t_prev = sim.time();
        if dt <= 0.0 {
            break;
        }
        let h = dt.min(end - tr.time);
        let fd = flow_data(case, sim);
        tr.advance(&fd.flow(), h)
            .map_err(|e| vec![d("particles", e.to_string())])?;
        step += 1;
        if every > 0 && step % every == 0 {
            write_series(case, &tr, Some(step))?;
        }
    }
    write_series(case, &tr, None)?;
    Ok(tr)
}
