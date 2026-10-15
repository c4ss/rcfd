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

/// Is a RANS turbulence model (k-epsilon, k-omega SST, Spalart-Allmaras or a plugin) selected?
pub fn enabled(case: &Case) -> bool {
    matches!(
        get_s(case, "turbulence.model").as_str(),
        "k_epsilon" | "k_omega_sst" | "spalart_allmaras" | "plugin"
    )
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

/// Turbulence role of a boundary entry (`turbulence = auto` is resolved from the velocity condition).
pub fn role(case: &Case, patch: &str) -> Role {
    let b = format!("boundaries.{patch}");
    match get_s(case, &format!("{b}.turbulence")).as_str() {
        "wall" => Role::Wall,
        "inlet" => Role::Inlet,
        "zero_gradient" => Role::ZeroGradient,
        _ => {
            if get_s(case, &format!("{b}.velocity")) == "fixed" {
                if velocity_is_zero(case, &b) {
                    Role::Wall
                } else {
                    Role::Inlet
                }
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
fn reference_speed(case: &Case) -> f64 {
    let mut u = vec3::norm(get_v(case, "initial.velocity", [0.0; 3]));
    for n in case.tree.names("boundaries").unwrap_or_default() {
        if role(case, &n) == Role::Inlet {
            u = u.max(vec3::norm(get_v(
                case,
                &format!("boundaries.{n}.velocity_value"),
                [0.0; 3],
            )));
        }
    }
    u
}

/// Semantic checks of the turbulence settings and of the combinations they cannot be used in.
pub fn check(case: &Case) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let names = case.tree.names("boundaries").unwrap_or_default();
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
    let plugin = get_s(case, "turbulence.model") == "plugin";
    let mname = match get_s(case, "turbulence.model").as_str() {
        "k_omega_sst" => "k-omega SST".to_string(),
        "spalart_allmaras" => "Spalart-Allmaras".to_string(),
        "plugin" => format!("plugin model '{}'", get_s(case, "turbulence.plugin")),
        _ => "k-epsilon".to_string(),
    };
    let mname = mname.as_str();
    let ke = !sst && !sa && !plugin;
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
    if (sst || sa || plugin) && get_f(case, "turbulence.epsilon_initial") > 0.0 {
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
    check_coefficients(case, &mut out);
    let alg = get_s(case, "solver.algorithm");
    if !matches!(alg.as_str(), "simple" | "simplec") {
        out.push(diag(
            "turbulence.model",
            format!(
                "{mname} is available with the steady algorithms (simple, simplec) only, not with solver.algorithm = \"{alg}\""
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
    if !(ke || sst) {
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
    } else if k0 <= 0.0 && reference_speed(case) <= 0.0 {
        out.push(diag(
            "turbulence.k_initial",
            "the initial k cannot be derived (no initial or boundary velocity): set turbulence.k_initial and the initial epsilon / omega",
        ));
    }
    let mut walls = 0;
    for n in &names {
        let b = format!("boundaries.{n}");
        match role(case, n) {
            Role::Wall => {
                walls += 1;
                if !velocity_is_zero(case, &b) && get_s(case, &format!("{b}.velocity")) == "fixed" {
                    out.push(diag(
                        &format!("{b}.turbulence"),
                        "moving walls are not supported by the wall functions (the wall velocity must be zero)",
                    ));
                }
            }
            Role::Inlet if sa || plugin => {}
            Role::Inlet => {
                let spec = get_s(case, &format!("{b}.turbulence_spec"));
                let speed = vec3::norm(get_v(case, &format!("{b}.velocity_value"), [0.0; 3]));
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
                        if other == "intensity_length" && get_f(case, &format!("{b}.turbulence_length")) <= 0.0 {
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
    if walls == 0 {
        out.push(diag(
            "turbulence.model",
            format!("{mname} needs at least one wall patch (a boundary with a fixed zero velocity or turbulence = \"wall\")"),
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

/// Coefficient overrides in `turbulence.coefficients.*`: every group is validated; only the active model's group may differ
/// from the defaults (k-epsilon: 0 = variant default).
fn check_coefficients(case: &Case, out: &mut Vec<Diagnostic>) {
    let active = get_s(case, "turbulence.model");
    let groups: [(&str, &str, Vec<rcfd_turb::CoefficientInfo>, bool); 3] = [
        ("k_epsilon", "k_epsilon", rcfd_turb::ke_coefficient_table(), true),
        ("k_omega_sst", "k_omega_sst", rcfd_turb::sst_coefficient_table(), false),
        (
            "sa",
            "spalart_allmaras",
            rcfd_turb::sa_infos(rcfd_turb::SaCoefficients::default()),
            false,
        ),
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
        if walls.is_empty() {
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
            k0 = k_from_intensity(get_f(case, "turbulence.initial_intensity"), reference_speed(case));
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
            _ => ("", false),
        };
        if !group.is_empty() {
            for c in model.coefficients() {
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

        // inlet values
        let cmu_now = model
            .coefficients()
            .iter()
            .find(|c| c.name == "c_mu")
            .map_or(cmu, |c| c.value);
        for (patch, speeds) in inlet_speeds {
            let b = format!("boundaries.{patch}");
            let per_scalar: Vec<(&str, Vec<f64>)> = if sa {
                let v = get_f(case, &format!("{b}.nutilde_value"));
                let v = if v > 0.0 { v } else { ctx.nutilde0 };
                vec![("nuTilda", vec![v; speeds.len()])]
            } else if sst {
                let (mut kf, mut wf) = (Vec::new(), Vec::new());
                for &u in speeds {
                    let (k, w) = inlet_values_sst(case, &b, nu, u);
                    kf.push(k);
                    wf.push(w);
                }
                vec![("k", kf), ("omega", wf)]
            } else if get_s(case, "turbulence.model") == "k_epsilon" {
                let (mut kf, mut ef) = (Vec::new(), Vec::new());
                for &u in speeds {
                    let (k, e) = inlet_values(case, &b, cmu_now, nu, u);
                    kf.push(k);
                    ef.push(e);
                }
                vec![("k", kf), ("epsilon", ef)]
            } else {
                Vec::new()
            };
            for (name, vals) in per_scalar {
                let pf: Vec<[f64; 3]> = vals.iter().map(|&v| [v, 0.0, 0.0]).collect();
                if let Some(f) = model.scalar_mut(name) {
                    let _ = f.set_bc(geom, patch, Box::new(FixedValue(Profile::PerFace(pf))));
                }
            }
        }
        let wall_faces = model.wall_faces();
        Ok(TurbState {
            model,
            change: f64::INFINITY,
            walls,
            wall_faces,
            inlet_speeds: inlet_speeds.to_vec(),
            geom: geom.clone(),
        })
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
        assert_eq!(role(&c, "w"), Role::Wall);
        assert_eq!(role(&c, "i"), Role::Inlet);
        assert_eq!(role(&c, "o"), Role::ZeroGradient);
        c.set("boundaries.i.turbulence", "wall".into()).unwrap();
        assert_eq!(role(&c, "i"), Role::Wall);
    }
}
