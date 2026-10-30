//! Turbulence modelling in the case schema: patch roles, inlet / initial-value
//! helpers, validation and the k-epsilon state of a running simulation (F-042).
//!
//! The inlet helpers are the usual engineering relations (Pope, *Turbulent
//! Flows*, 2000, ch. 10; Versteeg and Malalasekera, *An Introduction to
//! Computational Fluid Dynamics*, 2nd ed., sec. 3.12):
//!
//! ```text
//! k = 3/2 (I |U|)^2
//! eps = Cmu^(3/4) k^(3/2) / l          (intensity and length scale)
//! eps = Cmu k^2 / (nu * nut/nu)        (intensity and viscosity ratio)
//! ```

use crate::case::{Case, Diagnostic};
use crate::sim::{get_b, get_f, get_s, get_v};
use crate::wall_opts::WallOptionsFromCase as _;
use rcfd_core::vec3;
use rcfd_fv::bc::{FixedValue, Profile};
use rcfd_fv::FvGeometry;
use rcfd_turb::model::ModelContext;
use rcfd_turb::{KEpsilonVariant, SLimiter, SaWallMode, TurbulenceModel, WallTreatment};
use std::sync::Arc;

/// What a boundary patch does in the turbulence model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Wall treated with wall functions.
    Wall,
    /// Inflow with prescribed k and epsilon.
    Inlet,
    /// Zero-gradient k and epsilon (outlet, symmetry, ...).
    ZeroGradient,
}

/// Turbulent kinetic energy `k = 3/2 (I U)^2` from the intensity `I` and the speed `U`.
pub fn k_from_intensity(intensity: f64, speed: f64) -> f64 {
    1.5 * (intensity * speed).powi(2)
}

/// Dissipation `eps = Cmu^(3/4) k^(3/2) / l` from `k` and a turbulence length scale `l`.
pub fn epsilon_from_length(cmu: f64, k: f64, length: f64) -> f64 {
    cmu.powf(0.75) * k.powf(1.5) / length
}

/// Dissipation `eps = Cmu k^2 / (nu * ratio)` from `k` and the viscosity ratio `nut / nu`.
pub fn epsilon_from_viscosity_ratio(cmu: f64, k: f64, nu: f64, ratio: f64) -> f64 {
    cmu * k * k / (nu * ratio)
}

/// `Cmu` of a k-epsilon variant (the value the model constructs itself with).
pub fn variant_cmu(variant: KEpsilonVariant) -> f64 {
    match variant {
        KEpsilonVariant::Rng => 0.0845,
        _ => 0.09,
    }
}

/// Variant selected by `turbulence.variant`.
pub fn variant_of(case: &Case) -> KEpsilonVariant {
    match get_s(case, "turbulence.variant").as_str() {
        "rng" => KEpsilonVariant::Rng,
        "realizable" => KEpsilonVariant::Realizable,
        _ => KEpsilonVariant::Standard,
    }
}

/// Is a turbulence model (k-epsilon, k-omega SST, Spalart-Allmaras, a subgrid-scale model, the mixing length or a plugin)
/// selected?
pub fn enabled(case: &Case) -> bool {
    matches!(
        get_s(case, "turbulence.model").as_str(),
        "k_epsilon" | "k_omega_sst" | "spalart_allmaras" | "smagorinsky" | "wale" | "mixing_length" | "rsm" | "plugin"
    )
}

/// Is the Reynolds stress transport model selected (ADR 0032), by `model = "rsm"` or as the plugin key `rsm`?
pub fn is_rsm(case: &Case) -> bool {
    model_key(case) == "rsm"
}

/// Is a subgrid-scale (large-eddy) model selected (ADR 0026)?
pub fn is_sgs(case: &Case) -> bool {
    matches!(get_s(case, "turbulence.model").as_str(), "smagorinsky" | "wale")
}

/// Is the mixing-length closure selected (ADR 0026)?
pub fn is_mixing_length(case: &Case) -> bool {
    get_s(case, "turbulence.model") == "mixing_length"
}

/// Is one of the algebraic models without transported scalars (subgrid models, mixing length) selected?
pub fn is_algebraic(case: &Case) -> bool {
    is_sgs(case) || is_mixing_length(case)
}

/// Does the case use a transient algorithm (piso, pimple)?
pub fn is_transient(case: &Case) -> bool {
    matches!(get_s(case, "solver.algorithm").as_str(), "piso" | "pimple")
}

/// Is k-omega SST selected?
pub fn is_sst(case: &Case) -> bool {
    get_s(case, "turbulence.model") == "k_omega_sst"
}

/// Is Spalart-Allmaras selected?
pub fn is_sa(case: &Case) -> bool {
    get_s(case, "turbulence.model") == "spalart_allmaras"
}

/// Registry key of the selected model (`turbulence.plugin` for `model = plugin`).
pub fn model_key(case: &Case) -> String {
    match get_s(case, "turbulence.model").as_str() {
        "plugin" => get_s(case, "turbulence.plugin"),
        other => other.to_string(),
    }
}

/// Name of the selected model for reports.
pub fn model_label(case: &Case) -> String {
    match get_s(case, "turbulence.model").as_str() {
        "k_omega_sst" => "k-omega SST".to_string(),
        "k_epsilon" => format!("k-epsilon ({})", get_s(case, "turbulence.variant")),
        "spalart_allmaras" => "Spalart-Allmaras".to_string(),
        "smagorinsky" => "Smagorinsky (LES)".to_string(),
        "wale" => "WALE (LES)".to_string(),
        "mixing_length" => "mixing length".to_string(),
        "rsm" => format!(
            "Reynolds stress ({})",
            if get_s(case, "turbulence.rsm.pressure_strain") == "lrr_ip" {
                "LRR-IP"
            } else {
                "SSG"
            }
        ),
        "plugin" => format!("plugin ({})", get_s(case, "turbulence.plugin")),
        _ => "laminar".to_string(),
    }
}

fn velocity_is_zero(case: &Case, b: &str) -> bool {
    let v = get_v(case, &format!("{b}.velocity_value"), [0.0; 3]);
    let has_expr = ["x", "y", "z"]
        .iter()
        .any(|c| !get_s(case, &format!("{b}.velocity_expression_{c}")).trim().is_empty());
    !has_expr && vec3::norm(v) == 0.0
}

/// Is k-epsilon (any variant) selected?
fn is_k_epsilon(case: &Case) -> bool {
    get_s(case, "turbulence.model") == "k_epsilon"
}

/// Does the patch take the free-stream turbulence condition (inlet values on the inflow faces, zero gradient on the
/// outflow faces)? Explicitly by `turbulence = "freestream"` (ADR 0164 amendment 1), or by default: under
/// `turbulence = "auto"`, a `velocity = "freestream"` patch with a non-zero `velocity_value` with Spalart-Allmaras
/// (ADR 0166 D2; its free-stream value `nutilde_ratio * nu` needs no further key), k-omega SST or k-epsilon (ADR 0169 D1;
/// without an inlet specification their values come from [`freestream_fallback`]). Catalogue and wall-motion kinds
/// resolve first, as in [`role`]. Every other `auto` patch, and every other model (the Reynolds stress model has no inlet
/// condition, plugin and algebraic models keep theirs), resolves as before.
pub fn freestream_role(case: &Case, patch: &str) -> bool {
    let b = format!("boundaries.{patch}");
    match get_s(case, &format!("{b}.turbulence")).as_str() {
        "freestream" => true,
        "auto" => {
            (is_sa(case) || is_sst(case) || is_k_epsilon(case))
                && get_s(case, &format!("{b}.velocity")) == "freestream"
                && crate::bc_cat::role(case, patch).is_none()
                && !crate::wall_motion::is_wall_motion(case, patch)
                && vec3::norm(get_v(case, &format!("{b}.velocity_value"), [0.0; 3])) > 0.0
        }
        _ => false,
    }
}

/// Does a k-omega SST or k-epsilon free-stream patch ([`freestream_role`]) take its values from the initial-field keys
/// (ADR 0169 D2)? True when the patch gives no inlet specification, which is exactly the input the inlet check refuses
/// otherwise: `turbulence_spec = "intensity_length"` (the default) with `turbulence_length` unset (0). The values are then
/// those of `turbulence_spec = "intensity_viscosity_ratio"` with `turbulence.initial_intensity` and
/// `turbulence.initial_viscosity_ratio` ([`freestream_values`], [`freestream_values_sst`]). Every explicit specification
/// is used as given.
pub fn freestream_fallback(case: &Case, patch: &str) -> bool {
    let b = format!("boundaries.{patch}");
    (is_sst(case) || is_k_epsilon(case))
        && freestream_role(case, patch)
        && get_s(case, &format!("{b}.turbulence_spec")) == "intensity_length"
        && get_f(case, &format!("{b}.turbulence_length")) <= 0.0
}

/// `(k, epsilon)` of a free-stream face with speed `u` without an inlet specification (ADR 0169 D2):
/// `k = 1.5 (I U)^2` with `I = turbulence.initial_intensity`, `epsilon = Cmu k^2 / (nu R)` with
/// `R = turbulence.initial_viscosity_ratio`, the clamps of [`inlet_values`].
pub fn freestream_values(case: &Case, cmu: f64, nu: f64, u: f64) -> (f64, f64) {
    let k = k_from_intensity(get_f(case, "turbulence.initial_intensity"), u).max(1e-10);
    let e = epsilon_from_viscosity_ratio(cmu, k, nu, get_f(case, "turbulence.initial_viscosity_ratio"));
    (k, e.max(1e-10))
}

/// `(k, omega)` of a k-omega SST free-stream face without an inlet specification: [`freestream_values`] with
/// `Cmu = beta*`, `omega = epsilon / (beta* k)`.
pub fn freestream_values_sst(case: &Case, nu: f64, u: f64) -> (f64, f64) {
    let beta_star = rcfd_turb::sst::BETA_STAR;
    let (k, e) = freestream_values(case, beta_star, nu, u);
    (k, (e / (beta_star * k)).max(1e-10))
}

/// Turbulence role of a boundary entry (`turbulence = auto` is resolved from the velocity condition).
pub fn role(case: &Case, patch: &str) -> Role {
    let b = format!("boundaries.{patch}");
    match get_s(case, &format!("{b}.turbulence")).as_str() {
        "wall" => Role::Wall,
        // `freestream` (ADR 0164 amendment 1): inlet values on the inflow faces only, built in `TurbState::new`
        "inlet" | "freestream" => Role::Inlet,
        "zero_gradient" => Role::ZeroGradient,
        _ => {
            if let Some(r) = crate::bc_cat::role(case, patch) {
                // boundary-condition catalogue kinds (ADR 0022 D6b): their velocity/velocity_value are inert
                r
            } else if crate::wall_motion::is_wall_motion(case, patch) {
                // moving, sheared, slipping and thermocapillary boundaries are impermeable walls (ADR 0016)
                Role::Wall
            } else if get_s(case, &format!("{b}.velocity")) == "fixed" {
                if velocity_is_zero(case, &b) {
                    Role::Wall
                } else {
                    Role::Inlet
                }
            } else if freestream_role(case, patch) {
                // ADR 0166 D2, ADR 0169 D1: an SA, SST or k-epsilon free-stream far field takes the `freestream` role
                Role::Inlet
            } else {
                Role::ZeroGradient
            }
        }
    }
}

fn diag(path: &str, message: impl Into<String>) -> Diagnostic {
    Diagnostic {
        path: path.into(),
        line: None,
        message: message.into(),
    }
}

/// Largest velocity magnitude the case prescribes (initial and non-wall boundary values).
fn reference_speed(case: &Case, geom: Option<&rcfd_fv::FvGeometry>) -> f64 {
    let mut u = vec3::norm(get_v(case, "initial.velocity", [0.0; 3]));
    for n in case.tree.names("boundaries").unwrap_or_default() {
        if role(case, &n) == Role::Inlet {
            u = u.max(match crate::bc_cat::nominal_speed(case, geom, &n) {
                // boundary-condition catalogue kind: its speed comes from the flow rate (ADR 0022 D6b)
                Some(s) => s,
                None => vec3::norm(get_v(case, &format!("boundaries.{n}.velocity_value"), [0.0; 3])),
            });
        }
    }
    u
}

/// Semantic checks of the turbulence settings and of the combinations they cannot be used in.
pub fn check(case: &Case) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let names = case.tree.names("boundaries").unwrap_or_default();
    check_algebraic_options(case, &mut out);
    check_viscous_stress(case, &mut out);
    if !enabled(case) {
        for n in &names {
            let p = format!("boundaries.{n}.turbulence");
            if get_s(case, &p) != "auto" {
                out.push(diag(
                    &p,
                    "the turbulence role of a patch needs a RANS turbulence.model (k_epsilon, k_omega_sst, spalart_allmaras or plugin); the model is laminar",
                ));
            }
        }
        for r in case.tree.names("reports").unwrap_or_default() {
            if get_s(case, &format!("reports.{r}.type")) == "y_plus" {
                out.push(diag(
                    &format!("reports.{r}.type"),
                    "a y_plus report needs a RANS turbulence.model (k_epsilon, k_omega_sst, spalart_allmaras or plugin)",
                ));
            }
        }
        return out;
    }
    let sst = is_sst(case);
    let sa = is_sa(case);
    let sgs = is_sgs(case);
    let alg = is_algebraic(case);
    let rsm = is_rsm(case);
    let plugin = get_s(case, "turbulence.model") == "plugin";
    let mname = match get_s(case, "turbulence.model").as_str() {
        "k_omega_sst" => "k-omega SST".to_string(),
        "spalart_allmaras" => "Spalart-Allmaras".to_string(),
        "smagorinsky" => "Smagorinsky".to_string(),
        "wale" => "WALE".to_string(),
        "mixing_length" => "mixing_length".to_string(),
        "rsm" => "the Reynolds stress model".to_string(),
        "plugin" => format!("plugin model '{}'", get_s(case, "turbulence.plugin")),
        _ => "k-epsilon".to_string(),
    };
    let mname = mname.as_str();
    let ke = !sst && !sa && !plugin && !alg && !rsm;
    if plugin && !rcfd_turb::is_registered(&get_s(case, "turbulence.plugin")) {
        out.push(diag(
            "turbulence.plugin",
            format!(
                "no turbulence model is registered as '{}' (registered: {})",
                get_s(case, "turbulence.plugin"),
                rcfd_turb::model_keys()
                    .iter()
                    .map(|k| k.0.clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        ));
    } else if !plugin && !get_s(case, "turbulence.plugin").is_empty() {
        out.push(diag(
            "turbulence.plugin",
            "turbulence.plugin needs turbulence.model = \"plugin\"",
        ));
    }
    if !ke {
        if get_s(case, "turbulence.variant") != "standard" {
            out.push(diag(
                "turbulence.variant",
                format!("turbulence.variant applies to k_epsilon only, not to {mname}"),
            ));
        }
        if get_s(case, "turbulence.wall_treatment") != "smooth" {
            out.push(diag(
                "turbulence.wall_treatment",
                format!("turbulence.wall_treatment applies to k_epsilon only; {mname} has its own wall treatment"),
            ));
        }
    }
    let a0 = get_f(case, "turbulence.realizable_a0");
    if a0 != 0.0 {
        if get_s(case, "turbulence.model") != "k_epsilon" || variant_of(case) != KEpsilonVariant::Realizable {
            out.push(diag(
                "turbulence.realizable_a0",
                "turbulence.realizable_a0 applies to turbulence.model = \"k_epsilon\" with variant = \"realizable\" only",
            ));
        } else if !(3.0..=5.0).contains(&a0) {
            out.push(diag(
                "turbulence.realizable_a0",
                format!("realizable_a0 = {a0} is outside [3, 5] (the published values are 4.0 and 4.04; 0 keeps 4.04)"),
            ));
        }
    }
    if (sst || sa || plugin || alg) && !rsm && get_f(case, "turbulence.epsilon_initial") > 0.0 {
        out.push(diag(
            "turbulence.epsilon_initial",
            format!("turbulence.epsilon_initial applies to k_epsilon only, not to {mname}"),
        ));
    }
    if !sst && get_f(case, "turbulence.omega_initial") > 0.0 {
        out.push(diag(
            "turbulence.omega_initial",
            "turbulence.omega_initial needs turbulence.model = \"k_omega_sst\"",
        ));
    }
    if sa && get_f(case, "turbulence.k_initial") > 0.0 {
        out.push(diag(
            "turbulence.k_initial",
            "Spalart-Allmaras has no k: use turbulence.nutilde_ratio for the initial nuTilda",
        ));
    }
    if alg && get_f(case, "turbulence.k_initial") > 0.0 {
        out.push(diag(
            "turbulence.k_initial",
            format!("{mname} is algebraic and has no transported k"),
        ));
    }
    if !sa {
        for (k, v) in [("sa.wall_mode", "auto"), ("sa.limiter", "allmaras")] {
            if get_s(case, &format!("turbulence.{k}")) != v {
                out.push(diag(
                    &format!("turbulence.{k}"),
                    "turbulence.sa options need turbulence.model = \"spalart_allmaras\"",
                ));
            }
        }
    }
    let des = sa && get_s(case, "turbulence.sa.des") != "off";
    if !sa {
        for (k, v) in [("sa.des", "off"), ("sa.des_delta", "max_extent")] {
            if get_s(case, &format!("turbulence.{k}")) != v {
                out.push(diag(
                    &format!("turbulence.{k}"),
                    "turbulence.sa options need turbulence.model = \"spalart_allmaras\"",
                ));
            }
        }
        if get_f(case, "turbulence.sa.c_des") != 0.65 {
            out.push(diag(
                "turbulence.sa.c_des",
                "turbulence.sa options need turbulence.model = \"spalart_allmaras\"",
            ));
        }
    }
    if !rsm && get_s(case, "turbulence.rsm.pressure_strain") != "ssg" {
        out.push(diag(
            "turbulence.rsm.pressure_strain",
            "turbulence.rsm options need turbulence.model = \"rsm\"",
        ));
    }
    check_coefficients(case, &mut out);
    let algorithm = get_s(case, "solver.algorithm");
    if rsm {
        if !is_transient(case) {
            out.push(diag(
                "turbulence.model",
                format!(
                    "the Reynolds stress model is available with the transient algorithms (piso, pimple) only, not with solver.algorithm = \"{algorithm}\" (no steady stress sweep is validated)"
                ),
            ));
        }
    } else if des {
        if !is_transient(case) {
            out.push(diag(
                "turbulence.sa.des",
                format!(
                    "turbulence.sa.des = \"{}\" runs with the transient algorithms (piso, pimple) only, not with solver.algorithm = \"{algorithm}\"",
                    get_s(case, "turbulence.sa.des")
                ),
            ));
        }
    } else if sgs {
        if !is_transient(case) {
            out.push(diag(
                "turbulence.model",
                format!(
                    "{mname} is a large-eddy subgrid model and is available with the transient algorithms (piso, pimple) only, not with solver.algorithm = \"{algorithm}\""
                ),
            ));
        }
    } else if is_mixing_length(case) {
        if !matches!(algorithm.as_str(), "simple" | "simplec" | "piso" | "pimple") {
            out.push(diag(
                "turbulence.model",
                format!(
                    "{mname} is available with the segregated algorithms (simple, simplec, piso, pimple) only, not with solver.algorithm = \"{algorithm}\""
                ),
            ));
        }
    } else if !matches!(algorithm.as_str(), "simple" | "simplec") {
        out.push(diag(
            "turbulence.model",
            format!(
                "{mname} is available with the steady algorithms (simple, simplec) only, not with solver.algorithm = \"{algorithm}\""
            ),
        ));
    }
    if get_b(case, "energy.enabled") {
        out.push(diag(
            "turbulence.model",
            "RANS turbulence together with the energy equation is not supported (no turbulent heat flux model)",
        ));
    }
    if get_f(case, "physics.nu") <= 0.0 {
        out.push(diag(
            "physics.nu",
            "the RANS wall treatment needs a positive laminar viscosity physics.nu",
        ));
    }
    let second = if sst { "omega_initial" } else { "epsilon_initial" };
    let (k0, e0) = (
        get_f(case, "turbulence.k_initial"),
        get_f(case, &format!("turbulence.{second}")),
    );
    if !(ke || sst || rsm) {
        // SA and plugins: nothing to pair
    } else if (k0 > 0.0) != (e0 > 0.0) {
        out.push(diag(
            &if k0 > 0.0 {
                format!("turbulence.{second}")
            } else {
                "turbulence.k_initial".to_string()
            },
            format!("set both turbulence.k_initial and turbulence.{second}, or neither"),
        ));
    } else if k0 <= 0.0 && reference_speed(case, None) <= 0.0 {
        out.push(diag(
            "turbulence.k_initial",
            "the initial k cannot be derived (no initial or boundary velocity): set turbulence.k_initial and the initial epsilon / omega",
        ));
    }
    let mut walls = 0;
    for n in &names {
        let b = format!("boundaries.{n}");
        if get_s(case, &format!("{b}.turbulence")) == "freestream"
            && get_s(case, &format!("{b}.velocity")) != "freestream"
        {
            out.push(diag(
                &format!("{b}.turbulence"),
                "turbulence = \"freestream\" needs velocity = \"freestream\" on the patch (inflow and outflow faces are told apart by velocity_value)",
            ));
        }
        match role(case, n) {
            Role::Wall => {
                walls += 1;
                if rsm {
                    out.push(diag(
                        &format!("{b}.turbulence"),
                        "the Reynolds stress model has no wall treatment (no wall functions, no low-Reynolds terms, no wall reflection): it is available for flows without wall patches (periodic boxes, free-slip or symmetry boundaries)",
                    ));
                }
                // a `moving_wall` builds the same fixed-value boundary as a `fixed` moving wall, which SST accepts (ADR 0078 S2)
                if crate::wall_motion::is_wall_motion(case, n)
                    && !(sst && get_s(case, &format!("{b}.velocity")) == "moving_wall")
                {
                    out.push(diag(
                        &format!("{b}.velocity"),
                        "moving_wall, shear, partial_slip and marangoni walls are not supported by the turbulence wall treatments (they assume a no-slip wall at rest); use a laminar run",
                    ));
                }
                // k-omega SST evaluates its wall law with the velocity relative to the wall (ADR 0075), so a fixed moving wall is
                // allowed there (e.g. the stationary casing of a rotor solved in the rotating frame); the other models keep the refusal.
                if !sst && !velocity_is_zero(case, &b) && get_s(case, &format!("{b}.velocity")) == "fixed" {
                    out.push(diag(
                        &format!("{b}.turbulence"),
                        "moving walls are not supported by the wall functions (the wall velocity must be zero)",
                    ));
                }
            }
            Role::Inlet if rsm => {
                out.push(diag(
                    &format!("{b}.turbulence"),
                    "the Reynolds stress model has no inlet condition for the stresses in this version: use periodic boundaries or zero-gradient boundaries",
                ));
            }
            Role::Inlet if sa || plugin || alg => {}
            Role::Inlet => {
                let spec = get_s(case, &format!("{b}.turbulence_spec"));
                let speed = match crate::bc_cat::nominal_speed(case, None, n) {
                    Some(s) => s,
                    None => vec3::norm(get_v(case, &format!("{b}.velocity_value"), [0.0; 3])),
                };
                let has_expr = ["x", "y", "z"]
                    .iter()
                    .any(|c| !get_s(case, &format!("{b}.velocity_expression_{c}")).trim().is_empty());
                match spec.as_str() {
                    "values" => {
                        for f in ["k_value", if sst { "omega_value" } else { "epsilon_value" }] {
                            if get_f(case, &format!("{b}.{f}")) <= 0.0 {
                                out.push(diag(
                                    &format!("{b}.{f}"),
                                    format!("turbulence_spec = \"values\" needs a positive {f}"),
                                ));
                            }
                        }
                    }
                    other => {
                        if speed <= 0.0 && !has_expr {
                            out.push(diag(
                                &format!("{b}.turbulence_spec"),
                                "the inlet velocity is zero, so k = 1.5 (I U)^2 vanishes: prescribe the velocity or use turbulence_spec = \"values\"",
                            ));
                        }
                        // ADR 0169 D2: a free-stream far field without a length takes the initial-field keys instead
                        if other == "intensity_length"
                            && get_f(case, &format!("{b}.turbulence_length")) <= 0.0
                            && !freestream_fallback(case, n)
                        {
                            out.push(diag(
                                &format!("{b}.turbulence_length"),
                                "turbulence_spec = \"intensity_length\" needs a positive turbulence_length [m] (about 0.07 times the hydraulic diameter for internal flow)",
                            ));
                        }
                    }
                }
            }
            Role::ZeroGradient => {}
        }
    }
    let van_driest = get_s(case, "turbulence.wall_damping") == "van_driest";
    if walls == 0 && !(sgs && !van_driest) && !rsm {
        // a subgrid model without wall damping is valid in a periodic box (ADR 0026)
        let (path, why) = if van_driest && alg {
            ("turbulence.wall_damping", "with wall_damping = van_driest needs")
        } else {
            ("turbulence.model", "needs")
        };
        out.push(diag(
            path,
            format!(
                "{mname} {why} at least one wall patch (a boundary with a fixed zero velocity or turbulence = \"wall\")"
            ),
        ));
    }
    // y_plus reports need a wall patch
    for r in case.tree.names("reports").unwrap_or_default() {
        if get_s(case, &format!("reports.{r}.type")) == "y_plus" {
            let patch = get_s(case, &format!("reports.{r}.patch"));
            if !names.contains(&patch) || role(case, &patch) != Role::Wall {
                out.push(diag(
                    &format!("reports.{r}.patch"),
                    format!("y_plus needs a wall patch with a boundary entry, '{patch}' is not one"),
                ));
            }
        }
    }
    out
}

/// `solver.viscous_stress = "full"` (ADR 0032) is available for the incompressible constant-density transient algorithms only.
fn check_viscous_stress(case: &Case, out: &mut Vec<Diagnostic>) {
    if get_s(case, "solver.viscous_stress") != "full" {
        return;
    }
    let algorithm = get_s(case, "solver.algorithm");
    if !is_transient(case) {
        out.push(diag(
            "solver.viscous_stress",
            format!(
                "solver.viscous_stress = \"full\" is available with the transient algorithms (piso, pimple) only, not with solver.algorithm = \"{algorithm}\""
            ),
        ));
    } else if crate::vd::enabled(case) {
        out.push(diag(
            "solver.viscous_stress",
            "solver.viscous_stress = \"full\" is not available with variable density (the transposed stress is implemented for the constant-density momentum equation)",
        ));
    }
}

/// Keys that belong to the algebraic models (`wall_damping`, `length_scale`, `mixing_profile`, ADR 0026).
fn check_algebraic_options(case: &Case, out: &mut Vec<Diagnostic>) {
    let model = get_s(case, "turbulence.model");
    let damping = get_s(case, "turbulence.wall_damping");
    if damping != "none" && !matches!(model.as_str(), "smagorinsky" | "mixing_length") {
        out.push(diag(
            "turbulence.wall_damping",
            if model == "wale" {
                "WALE has no wall damping (its operator already scales as y^3 at a wall): leave turbulence.wall_damping = \"none\""
                    .to_string()
            } else {
                format!(
                    "turbulence.wall_damping applies to turbulence.model = \"smagorinsky\" or \"mixing_length\", not to \"{model}\""
                )
            },
        ));
    }
    let r = get_f(case, "turbulence.length_scale");
    if model == "mixing_length" {
        if r <= 0.0 {
            out.push(diag(
                "turbulence.length_scale",
                "mixing_length needs a positive turbulence.length_scale [m] (half height of a channel, radius of a pipe)",
            ));
        }
    } else if r != 0.0 {
        out.push(diag(
            "turbulence.length_scale",
            "turbulence.length_scale applies to turbulence.model = \"mixing_length\" only",
        ));
    }
    if model != "mixing_length" && get_s(case, "turbulence.mixing_profile") != "linear" {
        out.push(diag(
            "turbulence.mixing_profile",
            "turbulence.mixing_profile applies to turbulence.model = \"mixing_length\" only",
        ));
    }
}

/// Coefficient overrides in `turbulence.coefficients.*`: every group is validated; only the active model's group may differ
/// from the defaults (k-epsilon: 0 = variant default).
fn check_coefficients(case: &Case, out: &mut Vec<Diagnostic>) {
    let active = get_s(case, "turbulence.model");
    let groups: [(&str, &str, Vec<rcfd_turb::CoefficientInfo>, bool); 7] = [
        ("k_epsilon", "k_epsilon", rcfd_turb::ke_coefficient_table(), true),
        ("k_omega_sst", "k_omega_sst", rcfd_turb::sst_coefficient_table(), false),
        (
            "sa",
            "spalart_allmaras",
            rcfd_turb::sa_infos(rcfd_turb::SaCoefficients::default()),
            false,
        ),
        (
            "smagorinsky",
            "smagorinsky",
            rcfd_turb::smagorinsky_coefficient_table(),
            false,
        ),
        ("wale", "wale", rcfd_turb::wale_coefficient_table(), false),
        (
            "mixing_length",
            "mixing_length",
            rcfd_turb::mixing_length_coefficient_table(),
            false,
        ),
        ("rsm", "rsm", rcfd_turb::rsm_coefficient_table(), false),
    ];
    for (group, model, table, zero_default) in groups {
        for c in table {
            let path = format!("turbulence.coefficients.{group}.{}", c.name);
            let v = get_f(case, &path);
            let default = if zero_default { 0.0 } else { c.default };
            if v == default {
                continue;
            }
            if active != model {
                out.push(diag(
                    &path,
                    format!(
                        "{} = {v} is set but only applies to turbulence.model = \"{model}\" (the model is \"{active}\")",
                        c.name
                    ),
                ));
            } else if model == "k_omega_sst" {
                out.push(diag(
                    &path,
                    format!(
                        "the k-omega SST coefficients are fixed at their published values in this version; {} = {v} differs from {}",
                        c.name, c.default
                    ),
                ));
            } else if model == "k_epsilon" && c.name == "c1" && variant_of(case) == KEpsilonVariant::Realizable {
                out.push(diag(
                    &path,
                    "c1 does not apply to the realizable variant (its C1 = max(0.43, eta/(eta+5)) is variable)",
                ));
            }
        }
    }
    if active != "plugin" {
        for n in case.tree.names("turbulence.plugin_coefficients").unwrap_or_default() {
            out.push(diag(
                &format!("turbulence.plugin_coefficients.{n}"),
                "plugin coefficients need turbulence.model = \"plugin\"",
            ));
        }
    }
}

/// Runtime state of a RANS turbulence run.
pub(crate) struct TurbState {
    /// The model.
    pub model: Box<dyn TurbulenceModel>,
    /// Largest relative change of the eddy viscosity in the last sweep.
    pub change: f64,
    /// Wall patch names.
    pub walls: Vec<String>,
    /// Wall faces of all wall patches (global indices).
    pub wall_faces: Vec<usize>,
    /// Per-face speed of each inlet patch (kept to rebuild the model in a live session).
    pub inlet_speeds: Vec<(String, Vec<f64>)>,
    geom: Arc<FvGeometry>,
    /// Cells of the laminar regions (`regions.<n>.turbulence = "laminar"`, ADR 0036) and the molecular viscosity; `None` without any.
    laminar: Option<(Vec<bool>, f64)>,
}

impl TurbState {
    /// Build the model: wall set from the patch roles, initial values and inlet conditions.
    /// `inlet_speeds` gives the per-face speed of each inlet patch.
    pub fn new(
        case: &Case,
        geom: &Arc<FvGeometry>,
        inlet_speeds: &[(String, Vec<f64>)],
    ) -> Result<Self, Vec<Diagnostic>> {
        let nu = get_f(case, "physics.nu");
        let sst = is_sst(case);
        let sa = is_sa(case);
        let variant = variant_of(case);
        let cmu = if sst {
            rcfd_turb::sst::BETA_STAR
        } else {
            variant_cmu(variant)
        };
        let names = case.tree.names("boundaries").unwrap_or_default();
        let walls: Vec<String> = names
            .iter()
            .filter(|n| role(case, n) == Role::Wall && geom.mesh.patch(n).is_some())
            .cloned()
            .collect();
        let needs_wall = is_mixing_length(case) || get_s(case, "turbulence.wall_damping") == "van_driest";
        if walls.is_empty() && ((!is_sgs(case) && !is_rsm(case)) || needs_wall) {
            return Err(vec![diag(
                "turbulence.model",
                "the turbulence model found no wall patch among the mesh patches",
            )]);
        }
        let (mut k0, mut e0) = (
            get_f(case, "turbulence.k_initial"),
            get_f(
                case,
                if sst {
                    "turbulence.omega_initial"
                } else {
                    "turbulence.epsilon_initial"
                },
            ),
        );
        if k0 <= 0.0 {
            k0 = k_from_intensity(
                get_f(case, "turbulence.initial_intensity"),
                reference_speed(case, Some(&**geom)),
            );
            e0 = epsilon_from_viscosity_ratio(cmu, k0, nu, get_f(case, "turbulence.initial_viscosity_ratio"));
            if sst {
                e0 /= cmu * k0; // omega = epsilon / (beta* k)
            }
        }
        let ctx = ModelContext {
            geom: geom.clone(),
            nu,
            walls: walls.clone(),
            k0,
            eps0: if sst { 0.0 } else { e0 },
            omega0: if sst { e0 } else { 0.0 },
            nutilde0: get_f(case, "turbulence.nutilde_ratio") * nu,
            variant,
            wall_treatment: match get_s(case, "turbulence.wall_treatment").as_str() {
                "switch" => WallTreatment::Switch,
                "hysteresis" => WallTreatment::Hysteresis { band: 0.1 },
                _ => WallTreatment::Smooth,
            },
            sa_wall_mode: match get_s(case, "turbulence.sa.wall_mode").as_str() {
                "resolved" => SaWallMode::Resolved,
                "auto_sa_law" => SaWallMode::SaLaw,
                _ => SaWallMode::Auto,
            },
            sa_limiter: match get_s(case, "turbulence.sa.limiter").as_str() {
                "min_0p3" => SLimiter::Min03,
                "positive" => SLimiter::Positive,
                _ => SLimiter::Allmaras,
            },
            sa_negative: get_b(case, "turbulence.sa.negative_nutilde"),
            kappa: get_f(case, "turbulence.kappa"),
            e_wall: get_f(case, "turbulence.e_wall"),
            convection: crate::sim::conv_scheme(&get_s(case, "turbulence.convection"))
                .unwrap_or(rcfd_fv::convection::ConvScheme::Upwind),
            relax_turb: get_f(case, "turbulence.relax"),
            relax_nut: get_f(case, "turbulence.relax_nut"),
            wall_damping: get_s(case, "turbulence.wall_damping") == "van_driest",
            length_scale: get_f(case, "turbulence.length_scale"),
            mixing_profile: match get_s(case, "turbulence.mixing_profile").as_str() {
                "nikuradse" => rcfd_turb::MixingProfile::Nikuradse,
                _ => rcfd_turb::MixingProfile::Linear,
            },
            time_accurate: is_transient(case),
        };
        let key = model_key(case);
        let mut model = rcfd_turb::create_model(&key, &ctx).map_err(|e| {
            vec![diag(
                if get_s(case, "turbulence.model") == "plugin" {
                    "turbulence.plugin"
                } else {
                    "turbulence.model"
                },
                e.to_string(),
            )]
        })?;

        // near-wall options (ADR 0015 C5); a plugin model owns its wall treatment, the case validation rejects
        // wall options on it, and applying the defaults here would overwrite what its factory chose
        if get_s(case, "turbulence.model") != "plugin" {
            model
                .apply_wall_options(&crate::wall_opts::WallOptions::from_case(case))
                .map_err(|e| vec![diag("turbulence", e)])?;
        }

        // coefficient overrides from the settings tree
        let mut errs = Vec::new();
        let (group, zero_default) = match get_s(case, "turbulence.model").as_str() {
            "k_epsilon" => ("k_epsilon", true),
            "k_omega_sst" => ("k_omega_sst", false),
            "spalart_allmaras" => ("sa", false),
            "smagorinsky" => ("smagorinsky", false),
            "wale" => ("wale", false),
            "mixing_length" => ("mixing_length", false),
            "rsm" => ("rsm", false),
            _ => ("", false),
        };
        if !group.is_empty() {
            let a0 = get_f(case, "turbulence.realizable_a0");
            if group == "k_epsilon" && a0 != 0.0 {
                if let Err(e) = model.set_coefficient("a0", a0) {
                    errs.push(diag("turbulence.realizable_a0", e));
                }
            }
            for c in model.coefficients() {
                if c.name == "a0" {
                    continue;
                }
                let path = format!("turbulence.coefficients.{group}.{}", c.name);
                let v = get_f(case, &path);
                if zero_default && v == 0.0 {
                    continue;
                }
                if let Err(e) = model.set_coefficient(c.name, v) {
                    errs.push(diag(&path, e));
                }
            }
        } else {
            for n in case.tree.names("turbulence.plugin_coefficients").unwrap_or_default() {
                let path = format!("turbulence.plugin_coefficients.{n}.value");
                let known: Vec<&'static str> = model.coefficients().iter().map(|c| c.name).collect();
                match known.iter().find(|k| **k == n) {
                    Some(name) => {
                        if let Err(e) = model.set_coefficient(name, get_f(case, &path)) {
                            errs.push(diag(&path, e));
                        }
                    }
                    None => errs.push(diag(
                        &format!("turbulence.plugin_coefficients.{n}"),
                        format!(
                            "the plugin model has no coefficient '{n}' (it has: {})",
                            known.join(", ")
                        ),
                    )),
                }
            }
        }
        if !errs.is_empty() {
            return Err(errs);
        }

        // hybrid RANS-LES options of Spalart-Allmaras (ADR 0032)
        if sa {
            for (key, value) in [
                ("des", get_s(case, "turbulence.sa.des")),
                ("des_delta", get_s(case, "turbulence.sa.des_delta")),
                ("c_des", format!("{}", get_f(case, "turbulence.sa.c_des"))),
            ] {
                if let Err(e) = model.set_option(key, &value) {
                    errs.push(diag(&format!("turbulence.sa.{key}"), e));
                }
            }
            if !errs.is_empty() {
                return Err(errs);
            }
        }

        // pressure-strain model of the Reynolds stress model (ADR 0032)
        if is_rsm(case) {
            if let Err(e) = model.set_option("pressure_strain", &get_s(case, "turbulence.rsm.pressure_strain")) {
                return Err(vec![diag("turbulence.rsm.pressure_strain", e)]);
            }
        }

        // inlet values
        let cmu_now = model
            .coefficients()
            .iter()
            .find(|c| c.name == "c_mu")
            .map_or(cmu, |c| c.value);
        for (patch, speeds) in inlet_speeds {
            let b = format!("boundaries.{patch}");
            // ADR 0169 D2: an SST / k-epsilon free-stream patch without an inlet specification
            let fallback = freestream_fallback(case, patch);
            let per_scalar: Vec<(&str, Vec<f64>)> = if sa {
                let v = get_f(case, &format!("{b}.nutilde_value"));
                let v = if v > 0.0 { v } else { ctx.nutilde0 };
                vec![("nuTilda", vec![v; speeds.len()])]
            } else if sst {
                let (mut kf, mut wf) = (Vec::new(), Vec::new());
                for &u in speeds {
                    let (k, w) = if fallback {
                        freestream_values_sst(case, nu, u)
                    } else {
                        inlet_values_sst(case, &b, nu, u)
                    };
                    kf.push(k);
                    wf.push(w);
                }
                vec![("k", kf), ("omega", wf)]
            } else if get_s(case, "turbulence.model") == "k_epsilon" {
                let (mut kf, mut ef) = (Vec::new(), Vec::new());
                for &u in speeds {
                    let (k, e) = if fallback {
                        freestream_values(case, cmu_now, nu, u)
                    } else {
                        inlet_values(case, &b, cmu_now, nu, u)
                    };
                    kf.push(k);
                    ef.push(e);
                }
                vec![("k", kf), ("epsilon", ef)]
            } else {
                Vec::new()
            };
            // ADR 0164 amendment 1: a `freestream` turbulence role fixes the values on the inflow faces only (zero gradient
            // on outflow faces, so that what the flow carries can leave); its velocity is a uniform free stream (checked).
            // ADR 0166 D2, ADR 0169 D1: also the default of an SA, SST or k-epsilon free-stream far field (`freestream_role`)
            let freestream = freestream_role(case, patch);
            let u_ref = get_v(case, &format!("{b}.velocity_value"), [0.0; 3]);
            for (name, vals) in per_scalar {
                let bc: Box<dyn rcfd_fv::bc::Bc> = if freestream {
                    Box::new(rcfd_fv::bc::Freestream {
                        u_ref,
                        value: [vals.first().copied().unwrap_or(0.0), 0.0, 0.0],
                        fixed_on_inflow: true,
                    })
                } else {
                    let pf: Vec<[f64; 3]> = vals.iter().map(|&v| [v, 0.0, 0.0]).collect();
                    Box::new(FixedValue(Profile::PerFace(pf)))
                };
                if let Some(f) = model.scalar_mut(name) {
                    let _ = f.set_bc(geom, patch, bc);
                }
            }
        }
        let wall_faces = model.wall_faces();
        let laminar = crate::regions::laminar_cells(case, geom)
            .map_err(|d| vec![d])?
            .map(|m| (m, nu));
        Ok(TurbState {
            model,
            change: f64::INFINITY,
            walls,
            wall_faces,
            inlet_speeds: inlet_speeds.to_vec(),
            geom: geom.clone(),
            laminar,
        })
    }

    /// Face viscosity for the momentum equation: the model's, with the molecular value on faces of laminar regions (ADR 0036).
    pub fn nu_face(&self) -> Vec<f64> {
        let mut nu = self.model.nu_face();
        if let Some((cells, nu_lam)) = &self.laminar {
            crate::regions::mask_faces(&self.geom, cells, *nu_lam, &mut nu);
        }
        nu
    }

    /// Wall-face `y+` of each wall face (aligned with `wall_faces`), from the model.
    pub fn y_plus_faces(&self) -> Vec<f64> {
        self.model.y_plus_faces()
    }

    /// A transported scalar by name (`k`, `epsilon`, `omega`, `nuTilda`, ...).
    pub fn scalar(&self, name: &str) -> Option<&rcfd_fv::ScalarField> {
        self.model
            .scalars()
            .into_iter()
            .find(|(n, _)| *n == name)
            .map(|(_, f)| f)
    }

    /// Cell field `y+` (the largest wall-face value of a wall cell, 0 elsewhere).
    pub fn y_plus_cells(&self) -> Vec<f64> {
        let g = &self.geom;
        let mut out = vec![0.0_f64; g.n_cells()];
        for (&f, y) in self.wall_faces.iter().zip(self.y_plus_faces()) {
            let c = g.mesh.owner()[f];
            out[c] = out[c].max(y);
        }
        out
    }

    /// Boundary-face field `y+` (indexed `f - n_internal`, 0 on non-wall faces).
    pub fn y_plus_boundary(&self) -> Vec<f64> {
        let g = &self.geom;
        let mut out = vec![0.0; g.mesh.n_faces() - g.n_internal];
        for (&f, y) in self.wall_faces.iter().zip(self.y_plus_faces()) {
            out[f - g.n_internal] = y;
        }
        out
    }
}

/// `(k, epsilon)` of an inlet face with speed `u` from the boundary's turbulence settings.
pub fn inlet_values(case: &Case, b: &str, cmu: f64, nu: f64, u: f64) -> (f64, f64) {
    match get_s(case, &format!("{b}.turbulence_spec")).as_str() {
        "values" => (
            get_f(case, &format!("{b}.k_value")),
            get_f(case, &format!("{b}.epsilon_value")),
        ),
        spec => {
            let k = k_from_intensity(get_f(case, &format!("{b}.turbulence_intensity")), u).max(1e-10);
            let e = if spec == "intensity_viscosity_ratio" {
                epsilon_from_viscosity_ratio(cmu, k, nu, get_f(case, &format!("{b}.turbulence_viscosity_ratio")))
            } else {
                epsilon_from_length(cmu, k, get_f(case, &format!("{b}.turbulence_length")).max(1e-30))
            };
            (k, e.max(1e-10))
        }
    }
}

/// `(k, omega)` of an inlet face with speed `u` for k-omega SST: `omega = epsilon / (beta* k)`.
pub fn inlet_values_sst(case: &Case, b: &str, nu: f64, u: f64) -> (f64, f64) {
    if get_s(case, &format!("{b}.turbulence_spec")) == "values" {
        return (
            get_f(case, &format!("{b}.k_value")),
            get_f(case, &format!("{b}.omega_value")),
        );
    }
    let beta_star = rcfd_turb::sst::BETA_STAR;
    let (k, e) = inlet_values(case, b, beta_star, nu, u);
    (k, (e / (beta_star * k)).max(1e-10))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inlet_helpers_match_the_textbook_relations() {
        // I = 5 %, U = 10 m/s: k = 1.5 * 0.25 = 0.375; l = 0.01: eps = 0.09^0.75 * 0.375^1.5 / 0.01
        let k = k_from_intensity(0.05, 10.0);
        assert!((k - 0.375).abs() < 1e-15);
        let e = epsilon_from_length(0.09, k, 0.01);
        assert!((e - 0.09f64.powf(0.75) * 0.375f64.powf(1.5) / 0.01).abs() < 1e-12);
        // viscosity ratio 10 with nu = 1.5e-5: nut = Cmu k^2/eps = 10 nu
        let e2 = epsilon_from_viscosity_ratio(0.09, k, 1.5e-5, 10.0);
        assert!((0.09 * k * k / e2 / 1.5e-5 - 10.0).abs() < 1e-12);
    }

    #[test]
    fn roles_follow_the_velocity_condition() {
        let mut c = Case::new();
        c.set("boundaries.w.velocity", "fixed".into()).unwrap();
        c.set("boundaries.i.velocity", "fixed".into()).unwrap();
        c.set("boundaries.i.velocity_value", serde_json::json!([1.0, 0.0, 0.0]))
            .unwrap();
        c.set("boundaries.o.velocity", "zero_gradient".into()).unwrap();
        for (i, k) in crate::wall_motion::KINDS.iter().enumerate() {
            // D6: the wall-motion kinds are walls even with a non-zero velocity value
            let n = format!("m{i}");
            c.set(&format!("boundaries.{n}.velocity"), (*k).into()).unwrap();
            c.set(
                &format!("boundaries.{n}.velocity_value"),
                serde_json::json!([1.0, 0.0, 0.0]),
            )
            .unwrap();
            assert_eq!(role(&c, &n), Role::Wall, "{k}");
        }
        assert_eq!(role(&c, "w"), Role::Wall);
        assert_eq!(role(&c, "i"), Role::Inlet);
        assert_eq!(role(&c, "o"), Role::ZeroGradient);
        c.set("boundaries.i.turbulence", "wall".into()).unwrap();
        assert_eq!(role(&c, "i"), Role::Wall);
    }

    /// ADR 0169 F7 (D1; replaces ADR 0166 T6's SA-only test): under `auto`, a free-stream far field is `freestream` (an
    /// inlet role) with Spalart-Allmaras, k-omega SST and k-epsilon, and only with a non-zero free stream; the Reynolds
    /// stress model, the algebraic models and laminar cases, and the explicit spellings, resolve as before.
    #[test]
    fn freestream_far_field_default_is_sa_sst_and_k_epsilon() {
        let mut c = Case::new();
        c.set("boundaries.f.velocity", "freestream".into()).unwrap();
        c.set("boundaries.f.velocity_value", serde_json::json!([1.0, 0.0, 0.0]))
            .unwrap();
        c.set("boundaries.z.velocity", "freestream".into()).unwrap();
        // laminar: no turbulence model, the role is the old one
        assert_eq!(role(&c, "f"), Role::ZeroGradient);
        assert!(!freestream_role(&c, "f"));
        for (model, inlet) in [
            ("spalart_allmaras", true),
            ("k_omega_sst", true),
            ("k_epsilon", true),
            ("rsm", false),
            ("smagorinsky", false),
            ("mixing_length", false),
        ] {
            c.set("turbulence.model", model.into()).unwrap();
            let want = if inlet { Role::Inlet } else { Role::ZeroGradient };
            assert_eq!(role(&c, "f"), want, "{model}");
            assert_eq!(freestream_role(&c, "f"), inlet, "{model}");
            // a zero free stream has no inflow faces: the old role
            assert_eq!(role(&c, "z"), Role::ZeroGradient, "{model} zero free stream");
            assert!(!freestream_role(&c, "z"), "{model} zero free stream");
            // ADR 0169 D2: SST and k-epsilon without an inlet specification take the initial-field keys
            let two_eq = matches!(model, "k_omega_sst" | "k_epsilon");
            assert_eq!(freestream_fallback(&c, "f"), two_eq, "{model}");
            assert!(!freestream_fallback(&c, "z"), "{model} zero free stream");
        }
        // an explicit specification is used as given
        c.set("turbulence.model", "k_omega_sst".into()).unwrap();
        c.set("boundaries.f.turbulence_length", serde_json::json!(0.1)).unwrap();
        assert!(freestream_role(&c, "f") && !freestream_fallback(&c, "f"));
        c.set("boundaries.f.turbulence_length", serde_json::json!(0.0)).unwrap();
        c.set("boundaries.f.turbulence_spec", "values".into()).unwrap();
        assert!(!freestream_fallback(&c, "f"));
        c.set("boundaries.f.turbulence_spec", "intensity_length".into())
            .unwrap();
        assert!(freestream_fallback(&c, "f"));
        c.set("turbulence.model", "spalart_allmaras".into()).unwrap();
        // the old behaviour stays spellable; the explicit opt-in is unchanged
        c.set("boundaries.f.turbulence", "zero_gradient".into()).unwrap();
        assert_eq!(role(&c, "f"), Role::ZeroGradient);
        assert!(!freestream_role(&c, "f"));
        c.set("boundaries.f.turbulence", "freestream".into()).unwrap();
        assert_eq!(role(&c, "f"), Role::Inlet);
        assert!(freestream_role(&c, "f"));
        c.set("boundaries.f.turbulence", "inlet".into()).unwrap();
        assert!(!freestream_role(&c, "f"));
    }
}
