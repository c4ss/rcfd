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
use rcfd_core::vec3;
use rcfd_fv::bc::{FixedValue, Profile};
use rcfd_fv::FvGeometry;
use rcfd_turb::{patch_faces, KEpsilon, KEpsilonVariant, KOmegaSst, WallTreatment};
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

/// Is a RANS turbulence model (k-epsilon or k-omega SST) selected?
pub fn enabled(case: &Case) -> bool {
    matches!(get_s(case, "turbulence.model").as_str(), "k_epsilon" | "k_omega_sst")
}

/// Is k-omega SST selected?
pub fn is_sst(case: &Case) -> bool {
    get_s(case, "turbulence.model") == "k_omega_sst"
}

/// The turbulence model of a running simulation.
pub(crate) enum TurbModel {
    /// k-epsilon family with wall functions.
    Ke(Box<KEpsilon>),
    /// k-omega SST.
    Sst(Box<KOmegaSst>),
}

impl TurbModel {
    /// One sweep; returns the largest relative change of the eddy viscosity.
    pub fn update(&mut self, u: &rcfd_fv::VectorField, phi: &[f64]) -> f64 {
        match self {
            TurbModel::Ke(m) => m.update(u, phi),
            TurbModel::Sst(m) => m.update(u, phi),
        }
    }
    /// Effective face viscosity for the momentum equation.
    pub fn nu_face(&self) -> Vec<f64> {
        match self {
            TurbModel::Ke(m) => m.nu_face(),
            TurbModel::Sst(m) => m.nu_face(),
        }
    }
    /// Turbulent kinetic energy.
    pub fn k(&self) -> &rcfd_fv::ScalarField {
        match self {
            TurbModel::Ke(m) => &m.k,
            TurbModel::Sst(m) => &m.k,
        }
    }
    /// Second transported scalar (epsilon or omega) and its field name.
    pub fn second(&self) -> (&'static str, &rcfd_fv::ScalarField) {
        match self {
            TurbModel::Ke(m) => ("epsilon", &m.eps),
            TurbModel::Sst(m) => ("omega", &m.omega),
        }
    }
    /// Eddy viscosity of the cells.
    pub fn nut(&self) -> &[f64] {
        match self {
            TurbModel::Ke(m) => &m.nut,
            TurbModel::Sst(m) => &m.nut,
        }
    }
    /// Wall distance of the cells.
    pub fn wall_distance(&self) -> &[f64] {
        match self {
            TurbModel::Ke(m) => m.wall_distance(),
            TurbModel::Sst(m) => m.wall_distance(),
        }
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
                    "the turbulence role of a patch needs a RANS turbulence.model (k_epsilon or k_omega_sst); the model is laminar",
                ));
            }
        }
        for r in case.tree.names("reports").unwrap_or_default() {
            if get_s(case, &format!("reports.{r}.type")) == "y_plus" {
                out.push(diag(
                    &format!("reports.{r}.type"),
                    "a y_plus report needs turbulence.model = \"k_epsilon\" or \"k_omega_sst\"",
                ));
            }
        }
        return out;
    }
    let sst = is_sst(case);
    let mname = if sst { "k-omega SST" } else { "k-epsilon" };
    if sst {
        if get_s(case, "turbulence.variant") != "standard" {
            out.push(diag(
                "turbulence.variant",
                "turbulence.variant applies to k_epsilon only; k_omega_sst has a single (Menter 2003) formulation",
            ));
        }
        if get_s(case, "turbulence.wall_treatment") != "smooth" {
            out.push(diag(
                "turbulence.wall_treatment",
                "turbulence.wall_treatment applies to k_epsilon only; k_omega_sst uses its automatic wall treatment",
            ));
        }
        if get_f(case, "turbulence.epsilon_initial") > 0.0 {
            out.push(diag(
                "turbulence.epsilon_initial",
                "k_omega_sst transports omega: use turbulence.omega_initial",
            ));
        }
    } else if get_f(case, "turbulence.omega_initial") > 0.0 {
        out.push(diag(
            "turbulence.omega_initial",
            "turbulence.omega_initial needs turbulence.model = \"k_omega_sst\"; k_epsilon uses epsilon_initial",
        ));
    }
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
    if (k0 > 0.0) != (e0 > 0.0) {
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

/// Runtime state of a RANS turbulence run.
pub(crate) struct TurbState {
    /// The model.
    pub model: TurbModel,
    /// Largest relative change of the eddy viscosity in the last sweep.
    pub change: f64,
    /// Wall patch names.
    pub walls: Vec<String>,
    /// Wall faces of all wall patches (global indices).
    pub wall_faces: Vec<usize>,
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
                "k-epsilon found no wall patch among the mesh patches",
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
        let wall_refs: Vec<&str> = walls.iter().map(String::as_str).collect();
        if sst {
            return Self::new_sst(case, geom, inlet_speeds, nu, &walls, &wall_refs, k0, e0);
        }
        let mut model = KEpsilon::new(variant, geom.clone(), nu, &wall_refs, k0, e0)
            .map_err(|e| vec![diag("turbulence.model", e.to_string())])?;
        model.wc.kappa = get_f(case, "turbulence.kappa");
        model.wc.e = get_f(case, "turbulence.e_wall");
        model.wall_treatment = match get_s(case, "turbulence.wall_treatment").as_str() {
            "switch" => WallTreatment::Switch,
            "hysteresis" => WallTreatment::Hysteresis { band: 0.1 },
            _ => WallTreatment::Smooth,
        };
        model.relax_turb = get_f(case, "turbulence.relax");
        model.relax_nut = get_f(case, "turbulence.relax_nut");
        model.convection = crate::sim::conv_scheme(&get_s(case, "turbulence.convection"))
            .unwrap_or(rcfd_fv::convection::ConvScheme::Upwind);
        for (patch, speeds) in inlet_speeds {
            let b = format!("boundaries.{patch}");
            let (mut kf, mut ef) = (Vec::with_capacity(speeds.len()), Vec::with_capacity(speeds.len()));
            for &u in speeds {
                let (k, e) = inlet_values(case, &b, model.c_mu, nu, u);
                kf.push([k, 0.0, 0.0]);
                ef.push([e, 0.0, 0.0]);
            }
            let _ = model.k.set_bc(geom, patch, Box::new(FixedValue(Profile::PerFace(kf))));
            let _ = model
                .eps
                .set_bc(geom, patch, Box::new(FixedValue(Profile::PerFace(ef))));
        }
        let wall_faces = patch_faces(geom, &wall_refs);
        Ok(TurbState {
            model: TurbModel::Ke(Box::new(model)),
            change: f64::INFINITY,
            walls,
            wall_faces,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn new_sst(
        case: &Case,
        geom: &Arc<FvGeometry>,
        inlet_speeds: &[(String, Vec<f64>)],
        nu: f64,
        walls: &[String],
        wall_refs: &[&str],
        k0: f64,
        w0: f64,
    ) -> Result<Self, Vec<Diagnostic>> {
        let mut model = KOmegaSst::new(geom.clone(), nu, wall_refs, k0, w0)
            .map_err(|e| vec![diag("turbulence.model", e.to_string())])?;
        model.law.kappa = get_f(case, "turbulence.kappa");
        model.law.e = get_f(case, "turbulence.e_wall");
        model.relax_turb = get_f(case, "turbulence.relax");
        model.relax_nut = get_f(case, "turbulence.relax_nut");
        model.convection = crate::sim::conv_scheme(&get_s(case, "turbulence.convection"))
            .unwrap_or(rcfd_fv::convection::ConvScheme::Upwind);
        for (patch, speeds) in inlet_speeds {
            let b = format!("boundaries.{patch}");
            let (mut kf, mut wf) = (Vec::with_capacity(speeds.len()), Vec::with_capacity(speeds.len()));
            for &u in speeds {
                let (k, w) = inlet_values_sst(case, &b, nu, u);
                kf.push([k, 0.0, 0.0]);
                wf.push([w, 0.0, 0.0]);
            }
            let _ = model.k.set_bc(geom, patch, Box::new(FixedValue(Profile::PerFace(kf))));
            let _ = model
                .omega
                .set_bc(geom, patch, Box::new(FixedValue(Profile::PerFace(wf))));
        }
        Ok(TurbState {
            model: TurbModel::Sst(Box::new(model)),
            change: f64::INFINITY,
            walls: walls.to_vec(),
            wall_faces: patch_faces(geom, wall_refs),
        })
    }

    /// Wall-function `y* = Cmu^(1/4) sqrt(k) y / nu` of each wall face's adjacent cell (aligned with `wall_faces`).
    pub fn y_plus_faces(&self) -> Vec<f64> {
        let m = match &self.model {
            TurbModel::Ke(m) => m,
            // SST: y+ = u_tau y / nu from the law of the wall, kept by the model
            TurbModel::Sst(m) => return m.y_plus().to_vec(),
        };
        let g = &m.geom;
        let c25 = m.c_mu.powf(0.25);
        self.wall_faces
            .iter()
            .map(|&f| {
                let c = g.mesh.owner()[f];
                c25 * m.k.values[c].max(0.0).sqrt() * g.dn[f - g.n_internal] / m.nu
            })
            .collect()
    }

    fn geom(&self) -> &Arc<FvGeometry> {
        match &self.model {
            TurbModel::Ke(m) => &m.geom,
            TurbModel::Sst(m) => &m.geom,
        }
    }

    /// Cell field `y+` (the largest wall-face value of a wall cell, 0 elsewhere).
    pub fn y_plus_cells(&self) -> Vec<f64> {
        let g = self.geom();
        let mut out = vec![0.0_f64; g.n_cells()];
        for (&f, y) in self.wall_faces.iter().zip(self.y_plus_faces()) {
            let c = g.mesh.owner()[f];
            out[c] = out[c].max(y);
        }
        out
    }

    /// Boundary-face field `y+` (indexed `f - n_internal`, 0 on non-wall faces).
    pub fn y_plus_boundary(&self) -> Vec<f64> {
        let g = self.geom();
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
