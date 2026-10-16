//! Melting and solidification in the settings tree (F-212, ADR 0035 D3).
//!
//! `phase_change.enabled` replaces the linear temperature step of the prescribed-flow transient path
//! (`solver.frozen_flow = true`, piso or pimple, zero prescribed flux) by the enthalpy-method step of
//! [`rcfd_fv::solidification`]: `h(T) = T + (L/cp) f(T)` with the linear liquid fraction between the solidus and the
//! liquidus, implicit Euler, Newton on the apparent capacity. The Carman-Kozeny momentum damping exists in the library only
//! (the melt does not flow here: the flux is zero).

use crate::case::{Case, Diagnostic};
use crate::cht::ChtModel;
use crate::sources::{gf, gs};
use rcfd_core::settings::Schema;
use rcfd_fv::solidification::{PhaseChange, PhaseLaw, StepStats};
use rcfd_fv::{FvGeometry, ScalarField};

/// Sentinel of the unset positive numbers.
const UNSET: f64 = -1.0;

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

/// Is the phase change active?
pub fn enabled(case: &Case) -> bool {
    flag(case, "phase_change.enabled")
}

/// Schema of the `phase_change` section.
pub fn schema() -> Schema {
    Schema::object()
        .doc("Enthalpy-method melting and solidification (F-212, ADR 0035): h(T) = T + (L/cp) f(T), linear liquid fraction between solidus and liquidus, equal phase properties, implicit Euler with Newton on the apparent capacity. Prescribed flow only (solver.frozen_flow = true with zero flux), piso or pimple, no solids, no electric")
        .field("enabled", Schema::boolean(false).doc("Solve the enthalpy equation with a phase change"))
        .field(
            "solidus",
            Schema::float(UNSET).doc("Solidus temperature [K]: below it the material is solid; must be set (> 0), -1 = unset"),
        )
        .field(
            "liquidus",
            Schema::float(UNSET).doc("Liquidus temperature [K]: above it the material is liquid; must exceed solidus, -1 = unset"),
        )
        .field(
            "latent_heat",
            Schema::float(UNSET).doc("Latent heat L [J/kg] (>= 0; 0 = sensible only); must be set, -1 = unset"),
        )
        .field(
            "momentum_constant",
            Schema::float(UNSET).doc("Carman-Kozeny mushy constant [kg/m^3/s]: reserved, the damping is not in the tree (library only); setting it is an error. -1 = unset"),
        )
        .field(
            "newton_max",
            Schema::int(100).range(1.0, 1e6).doc("Newton iteration cap per time step; a step that does not converge within it stops the run with a located message"),
        )
}

/// Settings checks (no mesh needed).
pub fn check(case: &Case) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    if !enabled(case) {
        for k in ["solidus", "liquidus", "latent_heat", "momentum_constant"] {
            let p = format!("phase_change.{k}");
            if gf(case, &p) != UNSET {
                out.push(d(p, "only used with phase_change.enabled = true"));
            }
        }
        return out;
    }
    let (ts, tl, l) = (
        gf(case, "phase_change.solidus"),
        gf(case, "phase_change.liquidus"),
        gf(case, "phase_change.latent_heat"),
    );
    if !(ts > 0.0 && ts.is_finite()) {
        out.push(d(
            "phase_change.solidus",
            format!("set the solidus temperature [K] (> 0); found {ts} (-1 means unset)"),
        ));
    }
    if !(tl > 0.0 && tl.is_finite()) {
        out.push(d(
            "phase_change.liquidus",
            format!("set the liquidus temperature [K] (> 0); found {tl} (-1 means unset)"),
        ));
    } else if ts > 0.0 && tl <= ts {
        out.push(d(
            "phase_change.liquidus",
            format!("must exceed phase_change.solidus ({ts} K); found {tl} K (a sharp front needs a small positive interval)"),
        ));
    }
    if !(l >= 0.0 && l.is_finite()) {
        out.push(d(
            "phase_change.latent_heat",
            format!("set the latent heat [J/kg] (>= 0); found {l} (-1 means unset)"),
        ));
    }
    if gf(case, "phase_change.momentum_constant") != UNSET {
        out.push(d(
            "phase_change.momentum_constant",
            "the Carman-Kozeny damping is not available in the case tree (library only, ADR 0035 D3); remove it",
        ));
    }
    let here = "phase_change.enabled";
    if !flag(case, "solver.frozen_flow") {
        out.push(d(
            here,
            "the phase change is available with solver.frozen_flow = true (prescribed zero flux); melt flow is not implemented (ADR 0035)",
        ));
    }
    if !flag(case, "energy.enabled") {
        out.push(d(here, "needs energy.enabled = true"));
    }
    let alg = gs(case, "solver.algorithm");
    if alg != "piso" && alg != "pimple" {
        out.push(d(
            "solver.algorithm",
            format!("the phase change needs a transient algorithm (piso or pimple), not \"{alg}\" (no steady solidification)"),
        ));
    }
    if gs(case, "solver.time_scheme") != "euler" {
        out.push(d(
            "solver.time_scheme",
            "the phase change is implicit Euler only; set solver.time_scheme = \"euler\"",
        ));
    }
    if !crate::cht::solid_names(case).is_empty() {
        out.push(d(
            here,
            "cannot be combined with solids (one material with equal phase properties)",
        ));
    }
    if crate::electric::enabled(case) {
        out.push(d(here, "cannot be combined with electric.enabled"));
    }
    for z in case.tree.names("zone_sources").unwrap_or_default() {
        if gf(case, &format!("zone_sources.{z}.linear_coefficient")) != 0.0 {
            out.push(d(
                format!("zone_sources.{z}.linear_coefficient"),
                "an implicit linear part is not combined with phase_change.enabled (use a constant source)",
            ));
        }
        if gs(case, &format!("zone_sources.{z}.kind")) == "fixed_value" {
            out.push(d(
                format!("zone_sources.{z}.kind"),
                "fixed_value zone sources cannot be combined with phase_change.enabled (use a source)",
            ));
        }
    }
    out
}

/// The phase-change state of a simulation.
pub struct PhaseState {
    law: PhaseLaw,
    alpha: f64,
    newton_max: usize,
    pc: Option<PhaseChange>,
    /// Statistics of the last step.
    pub last: Option<StepStats>,
}

impl PhaseState {
    /// Build from the case; `None` when the phase change is off.
    pub fn from_case(case: &Case, cht: Option<&ChtModel>) -> Result<Option<PhaseState>, Vec<Diagnostic>> {
        if !enabled(case) {
            return Ok(None);
        }
        let cp = gf(case, "physics.cp");
        let rho = gf(case, "physics.rho");
        let k = gf(case, "physics.conductivity");
        let mut errs = Vec::new();
        if !(cp > 0.0 && rho > 0.0 && k > 0.0) {
            errs.push(d(
                "phase_change.enabled",
                "needs physics.rho, physics.cp and physics.conductivity > 0 (the thermal diffusivity k/(rho cp))",
            ));
        }
        if let Some(flux) = cht.and_then(|m| m.frozen_flux.as_ref()) {
            let big = flux.iter().fold(0.0_f64, |m, x| m.max(x.abs()));
            if big > 0.0 {
                errs.push(d(
                    "phase_change.enabled",
                    format!(
                        "the prescribed flux must be zero (the melt does not flow); the largest face flux is {big:.3e}"
                    ),
                ));
            }
        }
        if !errs.is_empty() {
            return Err(errs);
        }
        Ok(Some(PhaseState {
            law: PhaseLaw {
                solidus: gf(case, "phase_change.solidus"),
                liquidus: gf(case, "phase_change.liquidus"),
                latent_k: gf(case, "phase_change.latent_heat") / cp,
            },
            alpha: k / (rho * cp),
            newton_max: case
                .tree
                .get("phase_change.newton_max")
                .ok()
                .and_then(|v| v.as_i64())
                .map_or(100, |v| v.max(1) as usize),
            pc: None,
            last: None,
        }))
    }

    /// Liquid fraction per cell of the temperatures `t`.
    pub fn liquid_fraction(&self, t: &[f64]) -> Vec<f64> {
        t.iter().map(|&x| self.law.liquid_fraction(x)).collect()
    }

    /// One implicit-Euler step of the temperature field `t` (boundary conditions set), the face diffusivity `alpha_face`
    /// (`None` = the uniform diffusivity) and the source `source` (K/s). `Err` when Newton did not converge.
    pub fn advance(
        &mut self,
        geom: &FvGeometry,
        t: &mut ScalarField,
        alpha_face: Option<&[f64]>,
        dt: f64,
        source: Option<&[f64]>,
    ) -> Result<f64, String> {
        if self.pc.is_none() {
            let gamma = match alpha_face {
                Some(a) => a.to_vec(),
                None => vec![self.alpha; geom.mesh.n_faces()],
            };
            let mut pc = PhaseChange::new(self.law, gamma, &t.values);
            pc.newton_max = self.newton_max;
            self.pc = Some(pc);
        }
        let pc = self.pc.as_mut().expect("just built");
        let st = pc.advance(geom, t, dt, source);
        self.last = Some(st);
        if st.converged {
            Ok(st.residual)
        } else {
            Err(format!(
                "phase_change.newton_max: the enthalpy Newton iteration did not converge in {} iterations (relative residual {:.3e}); the mushy interval is too thin for the time step and cell size",
                st.iterations, st.residual
            ))
        }
    }
}
