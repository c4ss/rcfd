//! Electric potential and Joule heating in the settings tree (F-213, ADR 0035 D2).
//!
//! `electric.enabled` solves `div(sigma grad phi) = 0` (quasi-static conduction) on every cell, with `sigma` from
//! `electric.conductivity` (or `solids.<n>.electrical_conductivity`) and the optional linear resistivity law
//! `sigma = sigma_0 / (1 + a (T - T_ref))`. The Joule heat enters the energy equation as a source through the
//! prescribed-flow paths (`solver.frozen_flow = true`); the coupling is lagged (potential from the previous temperature).

use crate::case::{Case, Diagnostic};
use crate::cht::ChtModel;
use crate::sources::{gf, gs};
use rcfd_core::settings::Schema;
use rcfd_fv::bc::{FixedGradient, FixedValue, Profile};
use rcfd_fv::electric as lib;
use rcfd_fv::{FvGeometry, ScalarField};
use rcfd_linalg::SolveControl;

/// Sentinel of the signed numeric keys (a legal potential can be any real, so the sentinel is out of range).
pub const UNSET: f64 = -1.0e30;
/// Sentinel of the unsigned keys.
const UNSET_POS: f64 = -1.0;

fn d(path: impl Into<String>, message: impl Into<String>) -> Diagnostic {
    Diagnostic {
        path: path.into(),
        line: None,
        message: message.into(),
    }
}

fn flag(case: &Case, path: &str) -> bool {
    case.tree.get(path).ok().and_then(|v| v.as_bool()).unwrap_or(false)
}

/// Is the electric potential solved?
pub fn enabled(case: &Case) -> bool {
    flag(case, "electric.enabled")
}

/// Schema of the `electric` section.
pub fn schema() -> Schema {
    Schema::object()
        .doc("Electric potential and Joule heating (F-213, ADR 0035): quasi-static conduction div(sigma grad phi) = 0 with the heat sigma |grad phi|^2 in the energy equation. Needs solver.frozen_flow = true and energy.enabled = true")
        .field("enabled", Schema::boolean(false).doc("Solve the electric potential"))
        .field(
            "conductivity",
            Schema::float(UNSET_POS).doc("Electrical conductivity sigma_0 [S/m] of the fluid cells and of the solids without electrical_conductivity; must be set (> 0) when enabled, -1 = unset"),
        )
        .field(
            "temperature_coefficient",
            Schema::float(0.0).doc("Coefficient a [1/K] of the linear resistivity law sigma = sigma_0 / (1 + a (T - reference_temperature)); 0 = constant conductivity"),
        )
        .field(
            "reference_temperature",
            Schema::float(UNSET_POS).doc("T_ref [K] of the resistivity law; required when temperature_coefficient is not 0, -1 = unset"),
        )
        .field(
            "joule_heating",
            Schema::boolean(true).doc("Add the Joule heat to the energy equation (false: potential only)"),
        )
}

/// Extra fields of `boundaries.<patch>` (added to the boundary schema).
pub fn boundary_fields(b: Schema) -> Schema {
    b.field(
        "electric",
        Schema::enumeration("insulated", &["insulated", "potential", "current"]).doc("Electric condition (electric.enabled): insulated (zero current, default), potential (fixed electric_potential) or current (total electric_current into the domain, uniform density)"),
    )
    .field(
        "electric_potential",
        Schema::float(UNSET).doc("electric = potential: potential [V] (any real; -1e30 = unset, an error)"),
    )
    .field(
        "electric_current",
        Schema::float(UNSET).doc("electric = current: total current [A] flowing INTO the domain through the patch, uniform density (any real; -1e30 = unset, an error)"),
    )
}

/// Settings checks (no mesh needed).
pub fn check(case: &Case) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let on = enabled(case);
    let patches = case.tree.names("boundaries").unwrap_or_default();
    let solids = crate::cht::solid_names(case);
    for s in &solids {
        let p = format!("solids.{s}.electrical_conductivity");
        let v = gf(case, &p);
        if v != UNSET_POS && !(v >= 0.0 && v.is_finite()) {
            out.push(d(
                p,
                format!("must be >= 0 (0 = insulator), or -1 for electric.conductivity (found {v})"),
            ));
        } else if v != UNSET_POS && !on {
            out.push(d(p, "only used with electric.enabled = true"));
        }
    }
    let mut n_potential = 0;
    for b in &patches {
        let p = |f: &str| format!("boundaries.{b}.{f}");
        let kind = gs(case, &p("electric"));
        let (v, i) = (gf(case, &p("electric_potential")), gf(case, &p("electric_current")));
        if !on && !crate::sources::lorentz_potential(case) && kind != "insulated" {
            out.push(d(p("electric"), "only used with electric.enabled = true"));
        }
        match kind.as_str() {
            "potential" => {
                n_potential += 1;
                if v == UNSET {
                    out.push(d(
                        p("electric_potential"),
                        "set the potential [V] of an electric = \"potential\" patch",
                    ));
                }
                if i != UNSET {
                    out.push(d(p("electric_current"), "only used with electric = \"current\""));
                }
            }
            "current" => {
                if i == UNSET {
                    out.push(d(
                        p("electric_current"),
                        "set the total current [A] (into the domain) of an electric = \"current\" patch",
                    ));
                } else if !i.is_finite() {
                    out.push(d(p("electric_current"), "must be finite"));
                }
                if v != UNSET {
                    out.push(d(p("electric_potential"), "only used with electric = \"potential\""));
                }
            }
            _ => {
                if v != UNSET {
                    out.push(d(p("electric_potential"), "only used with electric = \"potential\""));
                }
                if i != UNSET {
                    out.push(d(p("electric_current"), "only used with electric = \"current\""));
                }
            }
        }
    }
    if !on {
        return out;
    }
    let sigma0 = gf(case, "electric.conductivity");
    if !(sigma0 > 0.0 && sigma0.is_finite()) {
        out.push(d(
            "electric.conductivity",
            format!("set the electrical conductivity [S/m] (> 0); found {sigma0} (-1 means unset)"),
        ));
    }
    let a = gf(case, "electric.temperature_coefficient");
    let tref = gf(case, "electric.reference_temperature");
    if !a.is_finite() {
        out.push(d("electric.temperature_coefficient", "must be finite"));
    }
    if a != 0.0 && !(tref > 0.0 && tref.is_finite()) {
        out.push(d(
            "electric.reference_temperature",
            "set the reference temperature [K] (> 0) of the resistivity law; -1 means unset",
        ));
    }
    if a == 0.0 && tref != UNSET_POS {
        out.push(d(
            "electric.reference_temperature",
            "has no effect with temperature_coefficient = 0; remove it or set a coefficient",
        ));
    }
    if n_potential == 0 {
        out.push(d(
            "electric",
            "no boundary has electric = \"potential\": the potential is undefined (set one patch to a fixed potential, e.g. ground)",
        ));
    }
    if !flag(case, "solver.frozen_flow") {
        out.push(d(
            "electric.enabled",
            "the electric potential is available with solver.frozen_flow = true (prescribed flow); solved-flow paths are not implemented (ADR 0035)",
        ));
    }
    if !flag(case, "energy.enabled") {
        out.push(d(
            "electric.enabled",
            "needs energy.enabled = true (Joule heating is an energy source)",
        ));
    }
    out
}

/// The electric state of a simulation.
pub struct ElectricState {
    phi: ScalarField,
    /// Cell conductivity at the reference temperature.
    sigma0: Vec<f64>,
    a: f64,
    t_ref: f64,
    joule: bool,
    rho_cp: f64,
    /// Current patches: name, uniform density into the domain (A/m^2).
    current: Vec<(String, f64)>,
    sigma_face: Vec<f64>,
    heat: Vec<f64>,
    ctl: SolveControl,
}

impl ElectricState {
    /// Build from the case; `None` when the potential is not solved.
    pub fn from_case(
        case: &Case,
        arc: &std::sync::Arc<FvGeometry>,
        cht: Option<&ChtModel>,
    ) -> Result<Option<ElectricState>, Vec<Diagnostic>> {
        if !enabled(case) {
            return Ok(None);
        }
        let geom: &FvGeometry = arc;
        let n = geom.n_cells();
        let sigma_fluid = gf(case, "electric.conductivity");
        let mut sigma0 = vec![sigma_fluid; n];
        if let Some(m) = cht {
            for (c, &l) in m.label.iter().enumerate() {
                if l > 0 {
                    let v = gf(case, &format!("solids.{}.electrical_conductivity", m.names[l - 1]));
                    if v != UNSET_POS {
                        sigma0[c] = v;
                    }
                }
            }
        }
        let mut phi = ScalarField::uniform("phi", arc, 0.0);
        let mut current = Vec::new();
        let mut errs = Vec::new();
        let sf = geom.mesh.face_area_vectors();
        for patch in geom.mesh.patches() {
            let b = format!("boundaries.{}", patch.name);
            match gs(case, &format!("{b}.electric")).as_str() {
                "potential" => {
                    let v = gf(case, &format!("{b}.electric_potential"));
                    let _ = phi.set_bc(arc, &patch.name, Box::new(FixedValue::scalar(v)));
                }
                "current" => {
                    let i = gf(case, &format!("{b}.electric_current"));
                    let area: f64 = patch.faces().map(|f| rcfd_core::vec3::norm(sf[f])).sum();
                    if area.is_nan() || area <= 0.0 {
                        errs.push(d(format!("{b}.electric_current"), "the patch has no area"));
                        continue;
                    }
                    current.push((patch.name.clone(), i / area));
                }
                _ => {}
            }
        }
        if !errs.is_empty() {
            return Err(errs);
        }
        Ok(Some(ElectricState {
            phi,
            sigma0,
            a: gf(case, "electric.temperature_coefficient"),
            t_ref: gf(case, "electric.reference_temperature"),
            joule: flag(case, "electric.joule_heating"),
            rho_cp: gf(case, "physics.rho") * gf(case, "physics.cp"),
            current,
            sigma_face: vec![0.0; geom.mesh.n_faces()],
            heat: vec![0.0; n],
            ctl: SolveControl {
                tol_abs: 1e-300,
                tol_rel: 1e-13,
                max_iter: 5000,
            },
        }))
    }

    /// Solve the potential for the temperature field `t` and return the Joule source in K/s per cell (`None` when
    /// `joule_heating` is false). `Err` carries a located failure message (non-positive conductivity).
    pub fn update(&mut self, geom: &std::sync::Arc<FvGeometry>, t: &[f64]) -> Result<Option<Vec<f64>>, String> {
        let mut sigma = Vec::with_capacity(t.len());
        for (c, (&s0, &tc)) in self.sigma0.iter().zip(t).enumerate() {
            if s0 <= 0.0 {
                sigma.push(0.0);
                continue;
            }
            match lib::sigma_of_t(s0, self.a, self.t_ref, tc) {
                Some(s) => sigma.push(s),
                None => {
                    return Err(format!(
                        "electric.temperature_coefficient: the resistivity 1 + a (T - T_ref) = {} is not positive in cell {c} (T = {tc} K)",
                        1.0 + self.a * (tc - self.t_ref)
                    ))
                }
            }
        }
        let own = geom.mesh.owner();
        for (name, j) in &self.current {
            let prof = Profile::PerFace(
                geom.mesh
                    .patch(name)
                    .expect("known patch")
                    .faces()
                    .map(|f| [if sigma[own[f]] > 0.0 { j / sigma[own[f]] } else { 0.0 }, 0.0, 0.0])
                    .collect(),
            );
            let _ = self.phi.set_bc(geom, name, Box::new(FixedGradient(prof)));
        }
        let (sf, _) = lib::solve(geom, &mut self.phi, &sigma, &self.ctl, 20);
        self.sigma_face = sf;
        self.heat = lib::joule_heat(geom, &self.phi, &self.sigma_face, true);
        Ok(self
            .joule
            .then(|| self.heat.iter().map(|q| q / self.rho_cp).collect::<Vec<f64>>()))
    }

    /// Potential (V) per cell.
    pub fn potential(&self) -> &[f64] {
        &self.phi.values
    }

    /// Potential at the boundary faces.
    pub fn boundary_potential(&self, geom: &FvGeometry) -> Vec<f64> {
        self.phi.boundary_values(geom)
    }

    /// Joule heat (W/m^3) per cell.
    pub fn heat(&self) -> &[f64] {
        &self.heat
    }

    /// Conductivity (S/m) of every cell for the last update (face values are harmonic means; cells are `sigma0` law values
    /// reconstructed from the face data of the owner for reporting only).
    pub fn conductivity_cells(&self, t: &[f64]) -> Vec<f64> {
        self.sigma0
            .iter()
            .zip(t)
            .map(|(&s0, &tc)| {
                if s0 <= 0.0 {
                    0.0
                } else {
                    lib::sigma_of_t(s0, self.a, self.t_ref, tc).unwrap_or(f64::NAN)
                }
            })
            .collect()
    }

    /// Current density INTO the domain (A/m^2) at the boundary faces: `sigma_f dphi/dn` (outward gradient).
    pub fn boundary_current_density(&self, geom: &FvGeometry) -> Vec<f64> {
        let flows = lib::face_currents(geom, &self.phi, &self.sigma_face, true);
        let sf = geom.mesh.face_area_vectors();
        (geom.n_internal..geom.mesh.n_faces())
            .map(|f| {
                let a = rcfd_core::vec3::norm(sf[f]);
                if geom.empty[f] || a == 0.0 {
                    0.0
                } else {
                    -flows[f] / a
                }
            })
            .collect()
    }
}
