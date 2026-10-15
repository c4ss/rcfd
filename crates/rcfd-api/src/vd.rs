//! The steady variable-density (ideal-gas) pressure-based solver in the case schema (ADR 0005):
//! `fluid.density = "ideal_gas"` selects `rcfd_fv::varden` instead of the incompressible kernels.

use crate::case::{Case, Diagnostic};
use crate::sim::{get_b, get_f, get_i, get_s, get_v};
use rcfd_core::eos_data::critical_data;
use rcfd_core::fluids::{CubicEos, CubicKind, R_UNIVERSAL};
use rcfd_core::vec3;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::realgas::RealGas;
use rcfd_fv::simple::Coupling;
use rcfd_fv::varden::{
    DensityMode, DensityScheme, IdealGas, Property, StagnationInlet, TransientSettings, VarDensitySolver, VdInfo,
    VdSettings,
};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_mesh::PatchKind;
use std::sync::Arc;

/// Is the variable-density solver (ideal gas or real gas) selected?
pub fn enabled(case: &Case) -> bool {
    matches!(get_s(case, "fluid.density").as_str(), "ideal_gas" | "real_gas")
}

/// Is the real-gas equation of state selected (ADR 0007)?
pub fn real_gas(case: &Case) -> bool {
    get_s(case, "fluid.density") == "real_gas"
}

/// `fluid.*` keys of the real-gas model and their defaults (a different value needs `density = real_gas`).
const REAL_ONLY_FLOATS: [&str; 5] = [
    "fluid.critical_temperature",
    "fluid.critical_pressure",
    "fluid.acentric_factor",
    "fluid.molar_mass",
    "fluid.specific_heat",
];

/// Build the real-gas model from the `fluid.*` keys: the critical data of `fluid.species` overridden by the explicit
/// keys, the molar mass, the equation of state, and `cp0`.
pub fn real_gas_model(case: &Case) -> Result<RealGas, Vec<Diagnostic>> {
    let mut out = Vec::new();
    let species = get_s(case, "fluid.species");
    let lib = if species.is_empty() {
        None
    } else {
        match critical_data(&species) {
            Some(d) => Some(d),
            None => {
                out.push(diag(
                    "fluid.species",
                    format!(
                        "unknown fluid '{species}' for the real-gas model; built-in fluids: {}; or leave fluid.species empty and set critical_temperature, critical_pressure and molar_mass",
                        rcfd_core::eos_data::NAMES.join(", ")
                    ),
                ));
                None
            }
        }
    };
    let pick = |key: &str, from_lib: Option<f64>, out: &mut Vec<Diagnostic>, needed: bool| -> f64 {
        let v = get_f(case, &format!("fluid.{key}"));
        if v > 0.0 {
            v
        } else if let Some(x) = from_lib {
            x
        } else {
            if needed && species.is_empty() {
                out.push(diag(
                    &format!("fluid.{key}"),
                    format!("fluid.density = \"real_gas\" needs fluid.species or a positive fluid.{key}"),
                ));
            }
            0.0
        }
    };
    let tc = pick("critical_temperature", lib.map(|d| d.tc), &mut out, true);
    let pc = pick("critical_pressure", lib.map(|d| d.pc), &mut out, true);
    let mm = pick("molar_mass", lib.map(|d| d.molar_mass), &mut out, true);
    let omega = {
        let v = get_f(case, "fluid.acentric_factor");
        if v != 0.0 {
            v
        } else {
            lib.map(|d| d.omega).unwrap_or(0.0)
        }
    };
    let kind = if get_s(case, "fluid.eos") == "redlich_kwong" {
        CubicKind::RedlichKwong
    } else {
        CubicKind::PengRobinson
    };
    if !out.is_empty() {
        return Err(out);
    }
    let eos = CubicEos::new(kind, tc, pc, omega, mm).map_err(|e| vec![diag("fluid.eos", e.to_string())])?;
    let r = R_UNIVERSAL / mm;
    let gamma = get_f(case, "fluid.gamma");
    let cp_given = get_f(case, "fluid.specific_heat");
    let cp0 = if cp_given > 0.0 {
        cp_given
    } else {
        gamma * r / (gamma - 1.0)
    };
    if cp0 <= r {
        return Err(vec![diag(
            "fluid.specific_heat",
            format!("the ideal-gas specific heat {cp0} J/kg/K must exceed R = {r:.3} J/kg/K"),
        )]);
    }
    Ok(RealGas { eos, cp0 })
}

fn diag(path: &str, message: impl Into<String>) -> Diagnostic {
    Diagnostic {
        path: path.into(),
        line: None,
        message: message.into(),
    }
}

/// `fluid.*` keys and their defaults (a different value with `density = constant` is an error).
const FLUID_DEFAULT_FLOATS: [(&str, f64); 10] = [
    ("fluid.gas_constant", 287.058),
    ("fluid.gamma", 1.4),
    ("fluid.viscosity", 1.8e-5),
    ("fluid.sutherland_t_ref", 273.0),
    ("fluid.sutherland_s", 110.5),
    ("fluid.prandtl", 0.71),
    ("fluid.conductivity", 0.0),
    ("fluid.reference_pressure", 101325.0),
    ("fluid.relax_density", 1.0),
    ("fluid.inner_tolerance", 1e-8),
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
    if enabled(case) {
        // the variable-density heat_flow evaluation takes the conductivity from fluid.* and never reads the report key
        // (ADR 0021 D6): a positive value would be silently ignored
        for r in case.tree.names("reports").unwrap_or_default() {
            let base = format!("reports.{r}");
            if get_s(case, &format!("{base}.type")) == "heat_flow" && get_f(case, &format!("{base}.conductivity")) > 0.0
            {
                out.push(diag(
                    &format!("{base}.conductivity"),
                    "the variable-density solver takes the conductivity from fluid.* and ignores this key; remove it",
                ));
            }
        }
    }
    if !real_gas(case) {
        for path in REAL_ONLY_FLOATS {
            if get_f(case, path) != 0.0 {
                out.push(diag(path, "this fluid setting needs fluid.density = \"real_gas\""));
            }
        }
        for (path, d) in [("fluid.eos", "peng_robinson"), ("fluid.species", "")] {
            if get_s(case, path) != d {
                out.push(diag(path, "this fluid setting needs fluid.density = \"real_gas\""));
            }
        }
    }
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
        if get_i(case, "fluid.inner_iterations") != 30 {
            out.push(diag(
                "fluid.inner_iterations",
                "this fluid setting needs fluid.density = \"ideal_gas\"",
            ));
        }
        if get_b(case, "energy.viscous_heating") {
            out.push(diag(
                "energy.viscous_heating",
                "viscous heating needs fluid.density = \"ideal_gas\" (the Boussinesq energy equation has none)",
            ));
        }
        if get_f(case, "energy.heat_source") != 0.0 {
            out.push(diag(
                "energy.heat_source",
                "a volumetric heat source needs fluid.density = \"ideal_gas\"",
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
    if !matches!(alg.as_str(), "simple" | "simplec" | "pimple") {
        out.push(diag(
            "solver.algorithm",
            format!(
                "the ideal-gas variable-density solver supports steady SIMPLE / SIMPLEC and the transient \"pimple\" (SIMPLE-inner iterations per time step, ADR 0009), got \"{alg}\"; set solver.algorithm = \"simplec\" or \"pimple\""
            ),
        ));
    }
    if alg == "pimple" {
        if get_f(case, "solver.max_courant") > 0.0 {
            out.push(diag(
                "solver.max_courant",
                "adaptive time stepping is not supported with fluid.density = \"ideal_gas\"; use a fixed solver.dt (set solver.max_courant = 0)",
            ));
        }
    } else {
        if get_i(case, "fluid.inner_iterations") != 30 {
            out.push(diag(
                "fluid.inner_iterations",
                "the inner iterations belong to the transient solver: set solver.algorithm = \"pimple\"",
            ));
        }
        if (get_f(case, "fluid.inner_tolerance") - 1e-8).abs() > 1e-300 {
            out.push(diag(
                "fluid.inner_tolerance",
                "the inner tolerance belongs to the transient solver: set solver.algorithm = \"pimple\"",
            ));
        }
    }
    if real_gas(case) {
        if (get_f(case, "fluid.gas_constant") - 287.058).abs() > 1e-300 {
            out.push(diag(
                "fluid.gas_constant",
                "the real-gas model derives R from the molar mass (R = 8.314462618 / molar_mass); remove fluid.gas_constant",
            ));
        }
        if get_s(case, "fluid.mode") == "low_mach" {
            out.push(diag(
                "fluid.mode",
                "fluid.density = \"real_gas\" supports the compressible mode only (the low_mach mode is ideal-gas)",
            ));
        }
        if get_b(case, "fluid.hydrostatic_init") {
            out.push(diag(
                "fluid.hydrostatic_init",
                "fluid.hydrostatic_init is not supported with fluid.density = \"real_gas\"",
            ));
        }
        if let Err(d) = real_gas_model(case) {
            out.extend(d);
        }
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
    unsupported(crate::lumped::enabled(case), "components", "lumped components");
    unsupported(crate::radiation::enabled(case), "radiation.model", "radiation");
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
    /// Time steps taken (transient runs).
    pub steps: usize,
}

/// Convection scheme of a `solver.convection` name, upwind if unknown.
fn scheme(case: &Case) -> ConvScheme {
    crate::sim::solver_conv_scheme(case)
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
        let real = if real_gas(case) {
            Some(real_gas_model(case)?)
        } else {
            None
        };
        let r = match &real {
            Some(rg) => rg.r(),
            None => get_f(case, "fluid.gas_constant"),
        };
        let gamma = get_f(case, "fluid.gamma");
        let cp = match &real {
            Some(rg) => rg.cp0,
            None => gamma * r / (gamma - 1.0),
        };
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
        s.real = real;
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
        s.viscous_heating = get_b(case, "energy.viscous_heating");
        s.heat_source = get_f(case, "energy.heat_source");
        if get_s(case, "solver.algorithm") == "pimple" {
            s.transient = Some(TransientSettings {
                scheme: crate::sim::solver_time_scheme(case),
                max_inner: get_i(case, "fluid.inner_iterations").max(1) as usize,
                tol: get_f(case, "fluid.inner_tolerance"),
            });
        }
        s.p_ref = (
            get_i(case, "solver.p_ref_cell").max(0) as usize,
            get_f(case, "solver.p_ref_value"),
        );
        s.h_from_solution = get_s(case, "solver.h_operator") != "previous_iterate";
        s.component_diag_correction = get_s(case, "solver.component_diagonal") == "corrected";
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
            steps: 0,
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
