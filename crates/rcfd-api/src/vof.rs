//! Volume-of-fluid free-surface flow in the settings tree (F-055, ADR 0049 D6): `multiphase.enabled` selects
//! [`rcfd_fv::vof`] (two incompressible immiscible fluids, algebraic interface compression, CSF surface tension, well-balanced
//! `p_rgh` gravity) instead of the single-phase transient kernel. Nothing changes for a case without `multiphase.enabled`.

use crate::case::{Case, Diagnostic};
use crate::sim::{get_b, get_f, get_i, get_s, get_v};
use rcfd_core::settings::Schema;
use rcfd_core::vec3;
use rcfd_fv::vof::{shape_fraction, Shape, VofSettings, VofSolver};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use std::sync::Arc;

/// Sentinel of the unset positive numbers.
const UNSET: f64 = -1.0;

fn d(path: impl Into<String>, message: impl Into<String>) -> Diagnostic {
    Diagnostic {
        path: path.into(),
        line: None,
        message: message.into(),
    }
}

/// Is the free-surface solver selected?
pub fn enabled(case: &Case) -> bool {
    get_b(case, "multiphase.enabled")
}

/// Schema of the `multiphase` section (all keys inert while `enabled = false`; setting one then is an error).
pub fn schema() -> Schema {
    let nums = |doc: &'static str| Schema::list(Schema::float(0.0)).doc(doc);
    let initial = Schema::object()
        .doc("Initial phase fraction: a shape; the cells are sub-sampled so a face-aligned box is exact. alpha = 1 is phase 1")
        .field(
            "shape",
            Schema::enumeration("unset", &["unset", "box", "sphere", "half_space"])
                .doc("Shape of the region: box (box_min, box_max), sphere (centre, radius; a disk in 2-D) or half_space (normal, point: the side n . (x - point) <= 0); must be set"),
        )
        .field(
            "inside",
            Schema::enumeration("phase_1", &["phase_1", "phase_2"])
                .doc("Which phase fills the shape; the other phase fills the rest of the domain"),
        )
        .field("box_min", nums("box: lower corner [x, y, z] (in 2-D give a z range that covers the mesh thickness); empty = unset"))
        .field("box_max", nums("box: upper corner [x, y, z]; empty = unset"))
        .field("centre", nums("sphere: centre [x, y, z]; empty = unset"))
        .field("radius", Schema::float(UNSET).doc("sphere: radius [m] (> 0); -1 = unset"))
        .field("normal", nums("half_space: normal [nx, ny, nz] (non-zero); empty = unset"))
        .field("point", nums("half_space: a point on the plane [x, y, z]; empty = unset"));
    Schema::object()
        .doc("Volume-of-fluid free-surface flow of two incompressible immiscible fluids (F-055, ADR 0049): phase fraction with interface compression and a bounded (flux-corrected) explicit limiter, CSF surface tension, p_rgh gravity, transient piso / pimple only. Gravity is physics.gravity; wall conditions are the boundaries.<patch> conditions (velocity fixed or zero_gradient, pressure fixed (a value of p_rgh) or zero_gradient); pressures are in Pa")
        .field("enabled", Schema::boolean(false).doc("Solve the two-phase free-surface flow"))
        .field("model", Schema::enumeration("vof", &["vof"]).doc("Multiphase model (only the algebraic volume of fluid)"))
        .field("density_1", Schema::float(UNSET).doc("Density of phase 1 (alpha = 1) [kg/m^3]; must be set (> 0), -1 = unset"))
        .field("density_2", Schema::float(UNSET).doc("Density of phase 2 (alpha = 0) [kg/m^3]; must be set (> 0), -1 = unset"))
        .field("viscosity_1", Schema::float(UNSET).doc("Dynamic viscosity of phase 1 [Pa s]; must be set (> 0), -1 = unset"))
        .field("viscosity_2", Schema::float(UNSET).doc("Dynamic viscosity of phase 2 [Pa s]; must be set (> 0), -1 = unset"))
        .field("surface_tension", Schema::float(0.0).at_least(0.0).doc("Surface tension coefficient [N/m] (continuum surface force, 90 degree contact angle at walls); 0 = none"))
        .field("compression", Schema::float(1.0).range(0.0, 4.0).doc("Interface compression coefficient c_alpha (0 = none, 1 = standard)"))
        .field("alpha_courant", Schema::float(0.25).range(1e-3, 0.5).doc("Largest Courant number of a phase-fraction sub-step (the phase fraction is sub-cycled inside each flow step)"))
        .field("curvature_smoothing", Schema::int(2).range(0.0, 10.0).doc("Smoothing passes of the phase fraction before the curvature is taken"))
        .field("initial", initial)
}

/// Settings checks (no mesh needed).
pub fn check(case: &Case) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let lst = |p: &str| -> usize {
        case.tree
            .get(p)
            .ok()
            .and_then(|v| v.as_array().map(|a| a.len()))
            .unwrap_or(0)
    };
    if !enabled(case) {
        for k in ["density_1", "density_2", "viscosity_1", "viscosity_2", "radius"] {
            let p = if k == "radius" {
                "multiphase.initial.radius".to_string()
            } else {
                format!("multiphase.{k}")
            };
            if get_f(case, &p) != UNSET {
                out.push(d(p, "only used with multiphase.enabled = true"));
            }
        }
        for (p, def) in [
            ("multiphase.surface_tension", 0.0),
            ("multiphase.compression", 1.0),
            ("multiphase.alpha_courant", 0.25),
        ] {
            if (get_f(case, p) - def).abs() > 1e-300 {
                out.push(d(p, "only used with multiphase.enabled = true"));
            }
        }
        if get_i(case, "multiphase.curvature_smoothing") != 2 {
            out.push(d(
                "multiphase.curvature_smoothing",
                "only used with multiphase.enabled = true",
            ));
        }
        for (p, def) in [
            ("multiphase.initial.shape", "unset"),
            ("multiphase.initial.inside", "phase_1"),
        ] {
            if get_s(case, p) != def {
                out.push(d(p, "only used with multiphase.enabled = true"));
            }
        }
        for k in ["box_min", "box_max", "centre", "normal", "point"] {
            let p = format!("multiphase.initial.{k}");
            if lst(&p) != 0 {
                out.push(d(p, "only used with multiphase.enabled = true"));
            }
        }
        return out;
    }
    // required phase properties
    for k in ["density_1", "density_2", "viscosity_1", "viscosity_2"] {
        let p = format!("multiphase.{k}");
        if get_f(case, &p) <= 0.0 {
            out.push(d(
                p,
                format!("multiphase.enabled = true needs a positive {k} (-1 = unset)"),
            ));
        }
    }
    // initial shape
    let shape = get_s(case, "multiphase.initial.shape");
    let need = |out: &mut Vec<Diagnostic>, k: &str| {
        let p = format!("multiphase.initial.{k}");
        if lst(&p) != 3 {
            out.push(d(p, format!("initial.shape = \"{shape}\" needs {k} = [x, y, z]")));
        }
    };
    match shape.as_str() {
        "unset" => out.push(d(
            "multiphase.initial.shape",
            "set the initial phase distribution: box, sphere or half_space",
        )),
        "box" => {
            need(&mut out, "box_min");
            need(&mut out, "box_max");
            if lst("multiphase.initial.box_min") == 3 && lst("multiphase.initial.box_max") == 3 {
                let (lo, hi) = (
                    get_v(case, "multiphase.initial.box_min", [0.0; 3]),
                    get_v(case, "multiphase.initial.box_max", [0.0; 3]),
                );
                if (0..3).any(|k| hi[k] <= lo[k]) {
                    out.push(d(
                        "multiphase.initial.box_max",
                        "every component of box_max must exceed box_min",
                    ));
                }
            }
        }
        "sphere" => {
            need(&mut out, "centre");
            if get_f(case, "multiphase.initial.radius") <= 0.0 {
                out.push(d(
                    "multiphase.initial.radius",
                    "initial.shape = \"sphere\" needs a positive radius (-1 = unset)",
                ));
            }
        }
        _ => {
            need(&mut out, "normal");
            need(&mut out, "point");
            if lst("multiphase.initial.normal") == 3
                && vec3::norm(get_v(case, "multiphase.initial.normal", [0.0; 3])) <= 0.0
            {
                out.push(d("multiphase.initial.normal", "the normal must be non-zero"));
            }
        }
    }
    for (k, shapes) in [
        ("box_min", "box"),
        ("box_max", "box"),
        ("centre", "sphere"),
        ("normal", "half_space"),
        ("point", "half_space"),
    ] {
        let p = format!("multiphase.initial.{k}");
        if lst(&p) != 0 && shape != shapes {
            out.push(d(p, format!("only used with initial.shape = \"{shapes}\"")));
        }
    }
    if shape != "sphere" && get_f(case, "multiphase.initial.radius") != UNSET {
        out.push(d(
            "multiphase.initial.radius",
            "only used with initial.shape = \"sphere\"",
        ));
    }
    // solver
    let alg = get_s(case, "solver.algorithm");
    if !matches!(alg.as_str(), "piso" | "pimple") {
        out.push(d(
            "solver.algorithm",
            format!(
                "the free-surface solver is transient: set solver.algorithm = \"piso\" or \"pimple\" (coupled, compressible and the steady algorithms are not supported), got \"{alg}\""
            ),
        ));
    }
    if get_s(case, "fluid.density") != "constant" {
        out.push(d(
            "fluid.density",
            "multiphase.enabled uses the phase densities multiphase.density_1/2; set fluid.density = \"constant\"",
        ));
    }
    let mut unsupported = |present: bool, path: &str, what: &str| {
        if present {
            out.push(d(
                path,
                format!(
                    "{what} is not supported with multiphase.enabled = true (laminar isothermal two-phase flow only)"
                ),
            ));
        }
    };
    unsupported(crate::turb::enabled(case), "turbulence.model", "a turbulence model");
    unsupported(get_b(case, "energy.enabled"), "energy.enabled", "the energy equation");
    unsupported(
        crate::rheology::enabled(case),
        "rheology.model",
        "non-Newtonian rheology",
    );
    unsupported(
        crate::properties::enabled(case),
        "properties",
        "temperature-dependent properties",
    );
    unsupported(crate::scalars::enabled(case), "scalars", "passive scalars and species");
    unsupported(crate::sources::enabled(case), "sources", "momentum sources");
    unsupported(crate::lumped::enabled(case), "components", "lumped components");
    unsupported(crate::radiation::enabled(case), "radiation.model", "radiation");
    unsupported(crate::stats::enabled(case), "statistics.enabled", "run-time statistics");
    unsupported(crate::dynmesh::enabled(case), "dynamic_mesh.enabled", "a moving mesh");
    unsupported(
        crate::particles::enabled(case),
        "particles.enabled",
        "particle tracking",
    );
    unsupported(crate::chemistry::enabled(case), "chemistry", "chemistry");
    unsupported(crate::electric::enabled(case), "electric.enabled", "the electric field");
    unsupported(
        crate::phase_change::enabled(case),
        "phase_change.enabled",
        "melting and solidification",
    );
    unsupported(crate::cht::enabled(case), "solids", "conjugate heat transfer");
    unsupported(crate::zone_sources::enabled(case), "zone_sources", "zone sources");
    unsupported(
        get_s(case, "solver.time_advance") == "fractional_step",
        "solver.time_advance",
        "the fractional-step projection",
    );
    unsupported(
        get_f(case, "solver.max_courant") > 0.0,
        "solver.max_courant",
        "adaptive time stepping (use a fixed solver.dt; set solver.max_courant = 0)",
    );
    unsupported(
        get_s(case, "solver.time_scheme") != "euler",
        "solver.time_scheme",
        "a time scheme other than euler",
    );
    unsupported(
        get_s(case, "solver.viscous_stress") == "full",
        "solver.viscous_stress",
        "the full viscous stress",
    );
    unsupported(
        get_f(case, "solver.bulk_velocity") != 0.0,
        "solver.bulk_velocity",
        "a bulk-velocity driver",
    );
    unsupported(
        get_b(case, "solver.frozen_flow"),
        "solver.frozen_flow",
        "a frozen flow field",
    );
    unsupported(
        vec3::norm(get_v(case, "physics.body_force", [0.0; 3])) > 0.0,
        "physics.body_force",
        "physics.body_force (use physics.gravity)",
    );
    for f in ["mesh.periodic_x", "mesh.periodic_y"] {
        unsupported(get_b(case, f), f, "periodic meshes");
    }
    if (get_f(case, "physics.rho") - 1.0).abs() > 1e-300 {
        out.push(d(
            "physics.rho",
            "ignored with multiphase.enabled (the phase densities are multiphase.density_1/2); remove it",
        ));
    }
    if (get_f(case, "physics.nu") - 0.01).abs() > 1e-300 {
        out.push(d(
            "physics.nu",
            "ignored with multiphase.enabled (the phase viscosities are multiphase.viscosity_1/2); remove it",
        ));
    }
    for n in case.tree.names("boundaries").unwrap_or_default() {
        let b = format!("boundaries.{n}");
        let v = get_s(case, &format!("{b}.velocity"));
        if !matches!(v.as_str(), "fixed" | "zero_gradient") {
            out.push(d(
                format!("{b}.velocity"),
                format!("velocity \"{v}\" (symmetry, partial_slip, marangoni, freestream, moving_wall, shear) is not supported with multiphase.enabled; use fixed (no-slip walls: value 0) or zero_gradient"),
            ));
        }
        let p = get_s(case, &format!("{b}.pressure"));
        if !matches!(p.as_str(), "fixed" | "zero_gradient") {
            out.push(d(
                format!("{b}.pressure"),
                format!("pressure \"{p}\" is not supported with multiphase.enabled; use fixed (a value of p_rgh [Pa]) or zero_gradient"),
            ));
        }
        if crate::bc_cat::kind(case, &n) != "none" {
            out.push(d(
                b.clone(),
                "catalogue boundary conditions are not supported with multiphase.enabled",
            ));
        }
    }
    out
}

/// Runtime state of a free-surface simulation.
pub(crate) struct VofState {
    /// Solver.
    pub solver: VofSolver,
    /// Time steps taken.
    pub steps: usize,
}

impl VofState {
    /// Build the solver on `geom` from the velocity and `p_rgh` fields that carry the boundary conditions of the generic setup.
    pub fn new(
        case: &Case,
        geom: &Arc<FvGeometry>,
        u: VectorField,
        p_rgh: ScalarField,
    ) -> Result<Self, Vec<Diagnostic>> {
        let v3 = |k: &str| get_v(case, &format!("multiphase.initial.{k}"), [0.0; 3]);
        let shape = match get_s(case, "multiphase.initial.shape").as_str() {
            "box" => Shape::Box {
                min: v3("box_min"),
                max: v3("box_max"),
            },
            "sphere" => Shape::Sphere {
                centre: v3("centre"),
                radius: get_f(case, "multiphase.initial.radius"),
            },
            _ => {
                let n = v3("normal");
                let nn = vec3::norm(n);
                let nh = vec3::scale(n, 1.0 / nn);
                Shape::HalfSpace {
                    normal: nh,
                    offset: vec3::dot(nh, v3("point")),
                }
            }
        };
        let invert = get_s(case, "multiphase.initial.inside") == "phase_2";
        let mut alpha = ScalarField::uniform("alpha", geom, 0.0);
        alpha.values = shape_fraction(geom, shape, invert, 10);
        let mut s = VofSettings::new(
            [get_f(case, "multiphase.density_1"), get_f(case, "multiphase.density_2")],
            [
                get_f(case, "multiphase.viscosity_1"),
                get_f(case, "multiphase.viscosity_2"),
            ],
        );
        s.sigma = get_f(case, "multiphase.surface_tension");
        s.gravity = get_v(case, "physics.gravity", [0.0; 3]);
        s.c_alpha = get_f(case, "multiphase.compression");
        s.alpha_courant = get_f(case, "multiphase.alpha_courant");
        s.n_smooth = get_i(case, "multiphase.curvature_smoothing").max(0) as usize;
        s.convection = crate::sim::solver_conv_scheme(case);
        s.n_outer = if get_s(case, "solver.algorithm") == "pimple" {
            get_i(case, "solver.n_outer").max(1) as usize
        } else {
            1
        };
        s.n_correctors = get_i(case, "solver.n_correctors").max(1) as usize;
        s.non_orth_correctors = get_i(case, "solver.non_orthogonal_correctors").max(0) as usize;
        s.alpha_u = get_f(case, "solver.alpha_u");
        s.alpha_p = get_f(case, "solver.alpha_p");
        s.ddt_correction = get_b(case, "solver.ddt_correction");
        s.p_ref = (
            get_i(case, "solver.p_ref_cell").max(0) as usize,
            get_f(case, "solver.p_ref_value"),
        );
        // never AMG: the AMG-preconditioned CG stagnates and then breaks down on the 1000:1 density-jump pressure equation (the
        // 120 x 40 dam break: 3 x 5000 iterations at step 120, then NaN; ADR 0049 addendum), the plain CG of the registered runs is used
        s.amg_pressure = false;
        let (mt, mi) = (
            get_f(case, "solver.momentum_rel_tol"),
            get_i(case, "solver.momentum_max_iter"),
        );
        let (pt, pi) = (
            get_f(case, "solver.pressure_rel_tol"),
            get_i(case, "solver.pressure_max_iter"),
        );
        if mt > 0.0 {
            s.momentum_solver.tol_rel = mt;
        }
        if mi > 0 {
            s.momentum_solver.max_iter = mi as usize;
        }
        if pt > 0.0 {
            s.pressure_solver.tol_rel = pt;
        }
        let pa = get_f(case, "solver.pressure_abs_tol");
        if pa > 0.0 {
            s.pressure_solver.tol_abs = pa;
        }
        if pi > 0 {
            s.pressure_solver.max_iter = pi as usize;
        }
        s.linear = crate::linsolver::options(case);
        let solver =
            VofSolver::new(geom.clone(), u, p_rgh, alpha, s).map_err(|e| vec![d("multiphase", e.to_string())])?;
        Ok(VofState { solver, steps: 0 })
    }

    /// Named cell field (`alpha`, `p_rgh`, `p`, `rho`) of the free-surface solver.
    pub fn field(&self, name: &str) -> Option<Vec<f64>> {
        Some(match name {
            "alpha" => self.solver.transport.alpha.values.clone(),
            "p_rgh" => self.solver.p_rgh.values.clone(),
            "p" => self.solver.p.clone(),
            "rho" => self.solver.density(),
            _ => return None,
        })
    }
}
