//! The steady variable-density (ideal-gas) pressure-based solver in the case schema (ADR 0005):
//! `fluid.density = "ideal_gas"` selects `rcfd_fv::varden` instead of the incompressible kernels.

use crate::case::{Case, Diagnostic};
use crate::sim::{conv_scheme, get_b, get_f, get_i, get_s, get_v};
use rcfd_core::vec3;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::simple::Coupling;
use rcfd_fv::varden::{
    DensityMode, DensityScheme, IdealGas, Property, StagnationInlet, VarDensitySolver, VdInfo, VdSettings,
};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_mesh::PatchKind;
use std::sync::Arc;

/// Is the ideal-gas variable-density solver selected?
pub fn enabled(case: &Case) -> bool {
    get_s(case, "fluid.density") == "ideal_gas"
}

fn diag(path: &str, message: impl Into<String>) -> Diagnostic {
    Diagnostic {
        path: path.into(),
        line: None,
        message: message.into(),
    }
}

/// `fluid.*` keys and their defaults (a different value with `density = constant` is an error).
const FLUID_DEFAULT_FLOATS: [(&str, f64); 9] = [
    ("fluid.gas_constant", 287.058),
    ("fluid.gamma", 1.4),
    ("fluid.viscosity", 1.8e-5),
    ("fluid.sutherland_t_ref", 273.0),
    ("fluid.sutherland_s", 110.5),
    ("fluid.prandtl", 0.71),
    ("fluid.conductivity", 0.0),
    ("fluid.reference_pressure", 101325.0),
    ("fluid.relax_density", 1.0),
];

const FLUID_DEFAULT_STRINGS: [(&str, &str); 4] = [
    ("fluid.mode", "compressible"),
    ("fluid.viscosity_model", "constant"),
    ("fluid.density_scheme", "linear_upwind"),
    ("fluid.density", "constant"),
];

/// Semantic checks: combinations the variable-density solver does not support, with located messages.
pub fn check(case: &Case) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let names = case.tree.names("boundaries").unwrap_or_default();
    if !enabled(case) {
        for (path, d) in FLUID_DEFAULT_FLOATS {
            if (get_f(case, path) - d).abs() > 1e-300 {
                out.push(diag(path, "this fluid setting needs fluid.density = \"ideal_gas\""));
            }
        }
        for (path, d) in FLUID_DEFAULT_STRINGS {
            if get_s(case, path) != d {
                out.push(diag(path, "this fluid setting needs fluid.density = \"ideal_gas\""));
            }
        }
        if get_b(case, "fluid.hydrostatic_init") {
            out.push(diag(
                "fluid.hydrostatic_init",
                "this fluid setting needs fluid.density = \"ideal_gas\"",
            ));
        }
        for n in &names {
            for f in ["inlet", "total_pressure_value"] {
                let p = format!("boundaries.{n}.{f}");
                let unset = if f == "inlet" {
                    get_s(case, &p) == "none"
                } else {
                    // total_pressure_value also serves compressible_bc = total_inlet (ADR 0006)
                    get_f(case, &p) == 0.0 || get_s(case, "solver.algorithm") == "compressible"
                };
                if !unset {
                    out.push(diag(&p, "needs fluid.density = \"ideal_gas\""));
                }
            }
        }
        return out;
    }
    let alg = get_s(case, "solver.algorithm");
    if !matches!(alg.as_str(), "simple" | "simplec") {
        out.push(diag(
            "solver.algorithm",
            format!(
                "the ideal-gas variable-density solver is steady SIMPLE / SIMPLEC only (got \"{alg}\"); set solver.algorithm = \"simplec\""
            ),
        ));
    }
    if !get_b(case, "energy.enabled") {
        out.push(diag(
            "energy.enabled",
            "the ideal-gas equation of state needs the temperature: set energy.enabled = true (energy.buoyancy and the Boussinesq physics settings are ignored, buoyancy follows from the density)",
        ));
    }
    let mut unsupported = |present: bool, path: &str, what: &str| {
        if present {
            out.push(diag(
                path,
                format!("{what} is not supported with fluid.density = \"ideal_gas\" (laminar steady variable-density flow only)"),
            ));
        }
    };
    unsupported(crate::turb::enabled(case), "turbulence.model", "a turbulence model");
    unsupported(
        crate::rheology::enabled(case),
        "rheology.model",
        "non-Newtonian rheology",
    );
    unsupported(
        crate::properties::enabled(case),
        "properties",
        "temperature-dependent properties (use fluid.viscosity_model / fluid.conductivity instead)",
    );
    unsupported(crate::scalars::enabled(case), "scalars", "passive scalars and species");
    unsupported(crate::sources::enabled(case), "sources", "momentum sources");
    unsupported(crate::stats::enabled(case), "statistics.enabled", "run-time statistics");
    unsupported(
        get_f(case, "solver.bulk_velocity") != 0.0,
        "solver.bulk_velocity",
        "a bulk-velocity driver",
    );
    unsupported(
        vec3::norm(get_v(case, "physics.body_force", [0.0; 3])) > 0.0,
        "physics.body_force",
        "physics.body_force (use physics.gravity)",
    );
    unsupported(
        get_s(case, "solver.pseudo_mode") != "off",
        "solver.pseudo_mode",
        "pseudo-transient stepping",
    );
    for f in ["mesh.periodic_x", "mesh.periodic_y"] {
        unsupported(get_b(case, f), f, "periodic meshes");
    }
    for n in case.tree.names("udf").unwrap_or_default() {
        for f in ["momentum_source", "viscosity"] {
            let p = format!("udf.{n}.{f}");
            unsupported(get_b(case, &p), &p, "a user-function momentum source or viscosity");
        }
    }
    let low_mach = get_s(case, "fluid.mode") == "low_mach";
    for n in &names {
        let b = format!("boundaries.{n}");
        if get_s(case, &format!("{b}.inlet")) == "stagnation" {
            if low_mach {
                out.push(diag(
                    &format!("{b}.inlet"),
                    "stagnation inlets need fluid.mode = \"compressible\" (the low_mach mode has no acoustic pressure level)",
                ));
            }
            if get_f(case, &format!("{b}.total_pressure_value")) <= 0.0 {
                out.push(diag(
                    &format!("{b}.total_pressure_value"),
                    "a stagnation inlet needs a positive total_pressure_value [Pa]",
                ));
            }
            if get_f(case, &format!("{b}.temperature_value")) <= 0.0 {
                out.push(diag(
                    &format!("{b}.temperature_value"),
                    "a stagnation inlet needs a positive total temperature_value [K]",
                ));
            }
        }
        if get_s(case, &format!("{b}.temperature")) == "heat_flux" {
            out.push(diag(
                &format!("{b}.temperature"),
                "heat_flux boundaries are not supported with fluid.density = \"ideal_gas\" (use fixed or zero_gradient)",
            ));
        }
        if get_s(case, &format!("{b}.turbulence")) != "auto" {
            out.push(diag(
                &format!("{b}.turbulence"),
                "turbulence roles do not apply to the variable-density solver",
            ));
        }
    }
    out
}

/// Runtime state of a variable-density simulation.
pub(crate) struct VdState {
    /// Solver.
    pub solver: VarDensitySolver,
    /// Information of the last iteration.
    pub last: Option<VdInfo>,
    /// Largest velocity change of the last iteration.
    pub max_du: f64,
}

/// Convection scheme of a `solver.convection` name, upwind if unknown.
fn scheme(case: &Case) -> ConvScheme {
    conv_scheme(&get_s(case, "solver.convection")).unwrap_or(ConvScheme::Upwind)
}

impl VdState {
    /// Build the solver from the case on `geom` with velocity, pressure and temperature fields that carry the
    /// boundary conditions of the generic setup.
    pub fn new(
        case: &Case,
        geom: &Arc<FvGeometry>,
        mut u: VectorField,
        mut p: ScalarField,
        mut t: ScalarField,
    ) -> Result<Self, Vec<Diagnostic>> {
        for patch in geom.mesh.patches() {
            if patch.kind == PatchKind::Cyclic {
                return Err(vec![diag(
                    "mesh",
                    format!(
                        "patch '{}' is periodic; the variable-density solver has no periodic boundaries",
                        patch.name
                    ),
                )]);
            }
        }
        let low_mach = get_s(case, "fluid.mode") == "low_mach";
        let r = get_f(case, "fluid.gas_constant");
        let gamma = get_f(case, "fluid.gamma");
        let cp = gamma * r / (gamma - 1.0);
        let pr = get_f(case, "fluid.prandtl");
        let mu_ref = get_f(case, "fluid.viscosity");
        let (mu, k) = if get_s(case, "fluid.viscosity_model") == "sutherland" {
            let (t_ref, s) = (get_f(case, "fluid.sutherland_t_ref"), get_f(case, "fluid.sutherland_s"));
            let mu = Property::Sutherland {
                reference: mu_ref,
                t_ref,
                s,
            };
            let k = match get_f(case, "fluid.conductivity") {
                kc if kc > 0.0 => Property::Constant(kc),
                _ => Property::Sutherland {
                    reference: mu_ref * cp / pr,
                    t_ref,
                    s,
                },
            };
            (mu, k)
        } else {
            let k = match get_f(case, "fluid.conductivity") {
                kc if kc > 0.0 => kc,
                _ => mu_ref * cp / pr,
            };
            (Property::Constant(mu_ref), Property::Constant(k))
        };
        let coupling = if get_s(case, "solver.algorithm") == "simple" {
            Coupling::Simple
        } else {
            Coupling::Simplec
        };
        let mut s = VdSettings::new(IdealGas { r, cp }, mu, k, coupling);
        s.mode = if low_mach {
            DensityMode::LowMach
        } else {
            DensityMode::Compressible
        };
        s.gravity = get_v(case, "physics.gravity", [0.0; 3]);
        s.reference_pressure = get_f(case, "fluid.reference_pressure");
        s.alpha_u = get_f(case, "solver.alpha_u");
        s.alpha_p = get_f(case, "solver.alpha_p");
        s.alpha_h = get_f(case, "energy.relax");
        s.alpha_rho = get_f(case, "fluid.relax_density");
        s.convection = scheme(case);
        s.energy_convection = scheme(case);
        s.density_scheme = match get_s(case, "fluid.density_scheme").as_str() {
            "upwind" => DensityScheme::Upwind,
            "central" => DensityScheme::Central,
            _ => DensityScheme::LinearUpwind,
        };
        s.p_ref = (
            get_i(case, "solver.p_ref_cell").max(0) as usize,
            get_f(case, "solver.p_ref_value"),
        );
        s.h_from_solution = get_s(case, "solver.h_operator") != "previous_iterate";
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
        // stagnation inlets: zero-gradient velocity, fixed total pressure and temperature as the boundary values
        for n in case.tree.names("boundaries").unwrap_or_default() {
            let b = format!("boundaries.{n}");
            if get_s(case, &format!("{b}.inlet")) != "stagnation" || geom.mesh.patch(&n).is_none() {
                continue;
            }
            let (p0, t0) = (
                get_f(case, &format!("{b}.total_pressure_value")),
                get_f(case, &format!("{b}.temperature_value")),
            );
            let _ = u.set_bc(geom, &n, Box::new(rcfd_fv::bc::ZeroGradient));
            let _ = p.set_bc(geom, &n, Box::new(rcfd_fv::bc::FixedValue::scalar(p0)));
            let _ = t.set_bc(geom, &n, Box::new(rcfd_fv::bc::FixedValue::scalar(t0)));
            s.inlets.push(StagnationInlet { patch: n, p0, t0 });
        }
        // initial pressure: absolute in compressible mode, dynamic (zero) in low-Mach mode
        let p_init = if low_mach {
            0.0
        } else {
            let pi = get_f(case, "initial.pressure");
            if pi > 0.0 {
                pi
            } else {
                s.reference_pressure
            }
        };
        p.values.iter_mut().for_each(|v| *v = p_init);
        let mut solver =
            VarDensitySolver::new(geom.clone(), u, p, t, s).map_err(|e| vec![diag("fluid", e.to_string())])?;
        if get_b(case, "fluid.hydrostatic_init") {
            solver.initialise_hydrostatic(30);
        }
        Ok(VdState {
            solver,
            last: None,
            max_du: f64::NAN,
        })
    }

    /// Named cell field (`rho`, `T`, `mach`, `p_abs`), if it is one of the variable-density ones.
    pub fn field(&self, name: &str) -> Option<Vec<f64>> {
        Some(match name {
            "rho" => self.solver.rho.clone(),
            "mach" => self.solver.mach(),
            "p_abs" => self.solver.absolute_pressure(),
            _ => return None,
        })
    }
}
