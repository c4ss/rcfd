//! Opt-in design-study evaluator on the reduced-order axial-fan model (`rcfd_post::fan_rom`, ADR 0119): a design loop can
//! run on it before any CFD converges. Nothing here is wired into the `design.*` service tools; a caller constructs it.
//!
//! Per condition: the operating point against the system curve `dp = system_k Q^2 + system_l Q` (total-to-static), with the
//! metrics `q`, `dp_s`, `dp_t`, `power`, `torque`, `eta_s`, `eta_t`, `tone1_pa` (steady-loading BPF amplitude at the
//! observer). A stalled or missing operating point is a failed condition with the located reason. Condition keys: `rpm`,
//! `rho`, `nu`, `system_k`, `system_l`.

use super::store::CondResult;
use super::study::{Condition, Evaluator, StudyDef};
use rcfd_post::fan_rom::{FanRom, Operating, RomFan, RomModel, SystemCurve};
use std::collections::BTreeMap;

/// Maps a candidate (parameter values in study order) to a blade row.
pub type FanGeometry = Box<dyn Fn(&[f64]) -> Result<RomFan, String> + Send + Sync>;

/// Base condition of the evaluator (conditions override it by key).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RomCondition {
    /// Air and rotor speed.
    pub op: Operating,
    /// System curve.
    pub system: SystemCurve,
    /// Speed of sound for the tone [m/s].
    pub c0: f64,
    /// Observer distance [m].
    pub observer_r: f64,
    /// Observer polar angle from the axis toward the thrust on the fluid [rad].
    pub observer_theta: f64,
    /// Scan intervals over `(0, A U_tip]` for the operating point.
    pub n_scan: usize,
}

/// Evaluator of study candidates on the reduced-order fan.
pub struct FanRomEvaluator {
    /// Candidate -> blade row.
    pub geometry: FanGeometry,
    /// Section and loss model.
    pub model: RomModel,
    /// Base condition.
    pub base: RomCondition,
}

impl FanRomEvaluator {
    /// Metrics of `fan` at condition `c`, or the located failure.
    pub fn metrics(&self, fan: RomFan, c: &RomCondition) -> Result<BTreeMap<String, f64>, String> {
        let rom = FanRom::new(fan, self.model)?;
        let q_hi = rom.area() * c.op.omega() * rom.fan.r_tip;
        let (_, p) = rom.operating_point(&c.op, &c.system, 1e-3 * q_hi, q_hi, c.n_scan)?;
        let tone = rom.tone(&c.op, &p, c.c0, 1, c.observer_r, c.observer_theta)?;
        let m: BTreeMap<String, f64> = [
            ("q", p.q),
            ("dp_s", p.dp_s),
            ("dp_t", p.dp_t),
            ("power", p.power),
            ("torque", p.torque),
            ("eta_s", p.eta_s),
            ("eta_t", p.eta_t),
            ("tone1_pa", tone),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
        if let Some((k, v)) = m.iter().find(|(_, v)| !v.is_finite()) {
            return Err(format!("metric {k} is not finite ({v})"));
        }
        Ok(m)
    }

    fn condition(&self, cond: &Condition) -> Result<RomCondition, String> {
        let mut c = self.base;
        for (key, v) in &cond.set {
            let x = v.as_f64().ok_or_else(|| format!("{key}: not a number ({v})"))?;
            match key.as_str() {
                "rpm" => c.op.rpm = x,
                "rho" => c.op.rho = x,
                "nu" => c.op.nu = x,
                "system_k" => c.system.k = x,
                "system_l" => c.system.l = x,
                _ => return Err(format!("{key}: unknown key (rpm, rho, nu, system_k, system_l)")),
            }
        }
        Ok(c)
    }
}

impl Evaluator for FanRomEvaluator {
    fn evaluate(&self, def: &StudyDef, x: &[f64]) -> Vec<CondResult> {
        def.conditions
            .iter()
            .map(|cond| {
                let r = self
                    .condition(cond)
                    .map_err(|e| format!("condition '{}': {e}", cond.label))
                    .and_then(|c| (self.geometry)(x).map_err(|e| format!("geometry: {e}")).map(|f| (c, f)))
                    .and_then(|(c, f)| self.metrics(f, &c));
                match r {
                    Ok(m) => CondResult::success(&cond.label, m),
                    Err(e) => CondResult::failure(&cond.label, "failed", e),
                }
            })
            .collect()
    }
}
