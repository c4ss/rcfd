//! Opt-in design-study evaluator on the reduced-order axial-fan model (`rcfd_post::fan_rom`, ADR 0119): a design loop can
//! run on it before any CFD converges. Nothing here is wired into the `design.*` service tools; a caller constructs it.
//!
//! Per condition: the operating point against the system curve `dp = system_k Q^2 + system_l Q` (total-to-static), with the
//! metrics `q`, `dp_s`, `dp_t`, `power`, `torque`, `eta_s`, `eta_t`, `tone1_pa` (steady-loading BPF amplitude at the
//! observer). A stalled or missing operating point is a failed condition with the located reason. Condition keys: `rpm`,
//! `rho`, `nu`, `system_k`, `system_l`.
//!
//! [`FanRomModel`] is the same model as a [`Model`] over manufacturing deviations, and [`fan_rom_bands`] its tolerance
//! bands through the tolerance driver (ADR 0623 D2, D3); both opt-in.

use super::ledger::{Fidelity, Ledger, Model, Outcome, Quantity, Verdict};
use super::store::CondResult;
use super::study::{Condition, Evaluator, StudyDef};
use super::uq::{Affine, Dist, Sampler, Standardized, StopRule, UncertainParam, UqResult, UqSpec};
use rcfd_mesh::fan_design::{Dist as TolDist, Tolerances};
use rcfd_post::fan_rom::{FanRom, Operating, RomFan, RomModel, SystemCurve};
use serde_json::json;
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

// ---------------------------------------------------------------- ADR 0623 D2, D3: the fan as a ledger model, and bands

/// Evaluator id of [`FanRomModel`].
pub const FAN_ROM_MODEL_ID: &str = "rcfd.fan_rom";
/// Evaluator version of [`FanRomModel`] (the ADR 0119 model with the ADR 0623 parameter mapping).
pub const FAN_ROM_MODEL_VERSION: &str = "1 (ADR 0119 model, ADR 0623 mapping)";
/// Parameters of [`FanRomModel`] with their units.
pub const FAN_ROM_PARAMS: [(&str, &str); 3] = [
    ("stagger_common_deg", "deg"),
    ("chord_rel", "1"),
    ("tip_gap_delta", "m"),
];

/// Unit of a metric of [`FanRomEvaluator::metrics`].
pub fn fan_rom_metric_unit(name: &str) -> &'static str {
    match name {
        "q" => "m3/s",
        "dp_s" | "dp_t" | "tone1_pa" => "Pa",
        "power" => "W",
        "torque" => "N m",
        _ => "1",
    }
}

/// The reduced-order fan as a [`Model`] over manufacturing deviations of one blade row (ADR 0623 D2): `stagger_common_deg`
/// [deg] is added to every section's stagger, every chord is multiplied by `1 + chord_rel`, `tip_gap_delta` [m] is added to
/// the tip gap. A missing parameter is 0. The metrics are those of [`FanRomEvaluator::metrics`] for the perturbed fan.
#[derive(Debug, Clone, PartialEq)]
pub struct FanRomModel {
    /// Nominal blade row.
    pub fan: RomFan,
    /// Section and loss model.
    pub model: RomModel,
    /// Condition.
    pub cond: RomCondition,
}

fn fnv1a64(h: &mut u64, x: u64) {
    for b in x.to_le_bytes() {
        *h ^= u64::from(b);
        *h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
}

impl FanRomModel {
    /// The perturbed fan of `params` with the three deviations, or the located problem with them.
    pub fn perturbed(&self, params: &[Quantity]) -> Result<(RomFan, [f64; 3]), String> {
        let mut d = [0.0; 3];
        for (i, q) in params.iter().enumerate() {
            let k = FAN_ROM_PARAMS.iter().position(|(n, _)| *n == q.name).ok_or_else(|| {
                format!(
                    "param '{}': unknown (stagger_common_deg, chord_rel, tip_gap_delta)",
                    q.name
                )
            })?;
            if params[..i].iter().any(|p| p.name == q.name) {
                return Err(format!("param '{}': given twice", q.name));
            }
            if q.unit != FAN_ROM_PARAMS[k].1 {
                return Err(format!(
                    "param '{}': unit must be '{}' (got '{}')",
                    q.name, FAN_ROM_PARAMS[k].1, q.unit
                ));
            }
            if !q.value.is_finite() {
                return Err(format!("param '{}': value {} is not finite", q.name, q.value));
            }
            d[k] = q.value;
        }
        let mut f = self.fan.clone();
        for s in &mut f.sections {
            s.stagger += d[0].to_radians();
            s.chord *= 1.0 + d[1];
        }
        f.tip_gap += d[2];
        Ok((f, d))
    }

    /// FNV-1a 64 of the bit patterns of `fan`, the model and the condition (a change detector for `rerun_check`).
    pub fn hash_of(&self, fan: &RomFan) -> String {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut put = |x: f64| fnv1a64(&mut h, x.to_bits());
        put(fan.n_blades as f64);
        put(fan.r_hub);
        put(fan.r_tip);
        put(fan.tip_gap);
        put(match fan.family {
            rcfd_mesh::fan::SectionFamily::Naca4 => 0.0,
            rcfd_mesh::fan::SectionFamily::CircularArc => 1.0,
        });
        for s in &fan.sections {
            for x in [s.radius, s.chord, s.stagger, s.camber, s.camber_pos] {
                put(x);
            }
        }
        let m = &self.model;
        for x in [
            m.n_strips as f64,
            m.cl_max,
            m.cd_normal,
            f64::from(u8::from(m.friction)),
            m.annulus_drag,
            m.secondary_drag,
            m.tip_cd,
        ] {
            put(x);
        }
        let c = &self.cond;
        for x in [
            c.op.rpm,
            c.op.rho,
            c.op.nu,
            c.system.k,
            c.system.l,
            c.c0,
            c.observer_r,
            c.observer_theta,
            c.n_scan as f64,
        ] {
            put(x);
        }
        format!("fnv1a64:{h:016x}")
    }
}

impl Model for FanRomModel {
    fn id(&self) -> &str {
        FAN_ROM_MODEL_ID
    }
    fn version(&self) -> &str {
        FAN_ROM_MODEL_VERSION
    }
    fn fidelity(&self) -> Fidelity {
        Fidelity::ReducedOrder
    }
    fn evaluate(&self, params: &[Quantity]) -> Outcome {
        let (fan, d) = match self.perturbed(params) {
            Ok(x) => x,
            Err(reason) => {
                return Outcome {
                    verdict: Verdict::NotEvaluated,
                    reason,
                    ..Default::default()
                }
            }
        };
        let c = &self.cond;
        let settings = json!({
            "stagger_common_deg": d[0], "chord_rel": d[1], "tip_gap_delta": d[2],
            "rpm": c.op.rpm, "rho": c.op.rho, "nu": c.op.nu, "system_k": c.system.k, "system_l": c.system.l,
        });
        let geometry_hash = self.hash_of(&fan);
        let ev = FanRomEvaluator {
            geometry: Box::new(|_x: &[f64]| Err("unused".into())),
            model: self.model,
            base: self.cond,
        };
        match ev.metrics(fan, &self.cond) {
            Ok(m) => Outcome {
                verdict: Verdict::Ok,
                reason: String::new(),
                metrics: m
                    .iter()
                    .map(|(k, &v)| Quantity::new(k, v, fan_rom_metric_unit(k)))
                    .collect(),
                geometry_hash,
                settings,
            },
            Err(e) => Outcome {
                verdict: Verdict::NotEvaluated,
                reason: format!("fan_rom: {e}"),
                metrics: Vec::new(),
                geometry_hash,
                settings,
            },
        }
    }
}

/// Fields of [`Tolerances`] the one-row model cannot represent (they must be `Fixed { value: 0 }`, ADR 0623 Q3 (a)).
const NOT_REPRESENTED: [&str; 5] = [
    "stagger_blade_deg",
    "thickness_rel",
    "eccentricity",
    "mass_rel",
    "roughness_ks",
];

/// Standardised inputs of the three representable tolerances (ADR 0623 D2): the affine maps of [`Standardized`] and the
/// distributions of the inputs. `chord_rel` (a per-blade distribution) is applied as one error common to all blades.
pub fn fan_rom_tolerances(tol: &Tolerances) -> Result<(Vec<Affine>, Vec<UncertainParam>), String> {
    if let Some(e) = tol.validate().first() {
        return Err(e.to_string());
    }
    let fields = [
        &tol.stagger_blade_deg,
        &tol.thickness_rel,
        &tol.eccentricity,
        &tol.mass_rel,
        &tol.roughness_ks,
    ];
    for (name, d) in NOT_REPRESENTED.iter().zip(fields) {
        if *d != (TolDist::Fixed { value: 0.0 }) {
            return Err(format!(
                "tolerances.{name}: the one-row reduced-order fan has no input for it (ADR 0623 Q3 (a)); it must be Fixed 0"
            ));
        }
    }
    let mut maps = Vec::new();
    let mut params = Vec::new();
    for ((name, unit), d) in FAN_ROM_PARAMS
        .iter()
        .zip([&tol.stagger_common_deg, &tol.chord_rel, &tol.tip_gap_delta])
    {
        let (offset, width, dist) = match *d {
            TolDist::Fixed { value } => (value, 0.0, Dist::Uniform { lo: 0.0, hi: 1.0 }),
            TolDist::Uniform { lo, hi } => (0.0, 1.0, Dist::Uniform { lo, hi }),
            TolDist::TruncatedNormal { mean, sd, lo, hi } => (
                mean,
                sd,
                Dist::Normal {
                    mean: 0.0,
                    sd: 1.0,
                    lo: (lo - mean) / sd,
                    hi: (hi - mean) / sd,
                },
            ),
        };
        maps.push(Affine {
            name: (*name).to_string(),
            unit: (*unit).to_string(),
            offset,
            width,
        });
        params.push(UncertainParam {
            name: Standardized::input_name(name),
            unit: "1".into(),
            dist,
        });
    }
    Ok((maps, params))
}

/// The tolerance study of the bands (ADR 0623 D3): Monte Carlo, one batch of `n`, quantiles 0.05, 0.5, 0.95 at confidence
/// 0.95, no stopping on the standard error; with the affine maps of the inputs.
pub fn fan_rom_band_spec(
    name: &str,
    tol: &Tolerances,
    n: usize,
    seed: u64,
    workers: usize,
) -> Result<(UqSpec, Vec<Affine>), String> {
    let (maps, params) = fan_rom_tolerances(tol)?;
    Ok((
        UqSpec {
            name: name.to_string(),
            params,
            sampler: Sampler::MonteCarlo,
            seed,
            stop: StopRule {
                batch: n,
                min_batches: 1,
                max_samples: n,
                se_abs: 0.0,
                se_rel: 0.0,
                metrics: Vec::new(),
            },
            quantiles: vec![0.05, 0.5, 0.95],
            confidence: 0.95,
            requirements: Vec::new(),
            workers,
        },
        maps,
    ))
}

/// Bands of the reduced-order fan under `tol` (ADR 0623 D3): [`super::uq::run`] on [`Standardized`] `(model)`. The band of
/// a metric is its 5 and 95 % quantiles with the median, each with its order-statistic interval, read together with the
/// failed fraction `1 - n_ok / n_samples` (statistics are over the ok samples).
pub fn fan_rom_bands(
    model: &FanRomModel,
    name: &str,
    tol: &Tolerances,
    n: usize,
    seed: u64,
    workers: usize,
    ledger: Option<&Ledger>,
) -> Result<UqResult, String> {
    let (spec, maps) = fan_rom_band_spec(name, tol, n, seed, workers)?;
    let m = Standardized::new(model, maps)?;
    super::uq::run(&spec, &m, ledger)
}
