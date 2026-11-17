//! Long-horizon design loop (ADR 0624, part H6 of `docs/fan-project-breakdown.md`): a durable hypothesis record in the design
//! ledger, tests registered before their runs, verdicts recomputed from their evidence and scoped to the evaluator and its tier,
//! and a lessons query. Opt-in, library only: nothing else calls it.
//!
//! * [`register`] appends a `hypothesis` entry (the claim, one factor at two levels, the response, the direction, the smallest
//!   effect that matters, at most one held metric, the domain, the paired design with its size and seed, the evaluator id,
//!   version and tier, and the level spent) before any evidence exists. The family is the ledger file; hypothesis `i` of the
//!   family is tested at `a_i = 0.05 * 6 / (pi^2 i^2)`, so the levels of any number of hypotheses sum to at most 0.05 (union
//!   bound). [`register_batch`] registers several hypotheses in one slot of the series, judged together by Holm's procedure.
//! * [`register_with_floor`] (ADR 0665) registers a hypothesis with one or two held metrics (two are solved together by a damped
//!   Newton iteration) and refuses a `min_effect` below [`K_RES`] times the held-tolerance floor of [`held_floor`]: the first-order
//!   change of the paired difference that the held tolerances allow, at the domain midpoint. A record with a [`FloorSample`]
//!   (ADR 0670) takes the largest of that floor over the midpoint and a seeded sample of its design points.
//! * [`run`] evaluates exactly the registered pairs (a seeded Latin hypercube of the domain, the factor at its low and high level,
//!   the held metric matched by bisection on its compensating parameter) with a [`Model`], appends them as evidence entries in
//!   pair order (whatever the worker count), resumes after a kill, and appends a `hypothesis_result` entry.
//! * [`verify`] and [`lessons`] never trust a stored verdict: they recompute it from the registration and the evidence and
//!   report every rule broken (order, alteration, evaluator version, pair count, design, an optional re-run). A registration
//!   may carry a seeded re-run sample of k pairs (ADR 0662): `verify` re-runs exactly those pairs and the result records them.
//! * [`verify_drawn`] (ADR 0693) re-runs k pairs drawn from a verifier's seed that is not in the file (caller-supplied or
//!   [`entropy_seed`]) after the evidence exists, then appends the seed in a `hypothesis_drawn_check` record; an evaluator that
//!   reads the file cannot be honest exactly on those pairs. [`drawn_record_problems`] redoes a recorded check.
//! * [`lessons_drawn`] (ADR 0695) is [`lessons`] plus the drawn checks: a row whose latest consistent drawn record did not
//!   reproduce is dropped, and every row reports the number of its drawn checks.
//! * Verdicts: `confirmed` when the one-sided lower bound of the mean paired difference is at least the smallest effect that
//!   matters, `refuted` when the upper bound is below it, `inconclusive` otherwise, `void` when too many pairs failed or a held
//!   metric was not matched. A verdict is a statement about the evaluator at its tier, never about a product.

use super::ledger::{
    entry_ref, rerun_check, resolve, Entry, EntryRef, Fidelity, Filter, Identity, Ledger, Model, Outcome, Quantity,
    Verdict,
};
use super::sampling::latin_hypercube;
use super::study::parallel_map;
use super::uq::{beta_inv, binom_cdf, clopper_pearson};
use rcfd_core::hash::fnv64;
use rcfd_core::rng::Rng;
use rcfd_reactor::mixfrac::reg_inc_beta;
use serde_json::{json, Value};
use std::panic::{catch_unwind, AssertUnwindSafe};

/// Evaluator id of the driver's own entries (registrations, withdrawals, results).
pub const DRIVER_ID: &str = "rcfd.hypothesis";
/// Version of the driver's entries.
pub const DRIVER_VERSION: &str = "1";
/// Family error level (ADR 0624 decision B2).
pub const FAMILY_LEVEL: f64 = 0.05;
/// Smallest number of pairs a registration may ask for.
pub const MIN_PAIRS: usize = 16;
/// `min_effect` must be at least this multiple of the declared resolution of the response.
pub const K_RES: f64 = 10.0;
/// Largest number of re-tests of one claim through `supersedes` (Q10 (c)).
pub const MAX_RETESTS: usize = 2;
/// Kind of a registration entry.
pub const KIND_HYPOTHESIS: &str = "hypothesis";
/// Kind of a result entry.
pub const KIND_RESULT: &str = "hypothesis_result";
/// Kind of a withdrawal entry.
pub const KIND_WITHDRAWN: &str = "hypothesis_withdrawn";
/// Largest number of held metrics, solved together by [`register_with_floor`]'s records (ADR 0665).
pub const MAX_HELD: usize = 2;
/// Newton iterations of a coupled hold (ADR 0665 D1).
const NEWTON_MAX_ITER: usize = 50;
/// Step halvings of one Newton iteration.
const NEWTON_HALVINGS: usize = 30;

fn num(v: f64) -> Value {
    serde_json::Number::from_f64(v).map_or(Value::Null, Value::Number)
}

// ---------------------------------------------------------------- statistics

/// Level of hypothesis slot `i >= 1` of a family: `0.05 * 6 / (pi^2 i^2)`.
pub fn series_level(i: usize) -> f64 {
    let i = i.max(1) as f64;
    FAMILY_LEVEL * 6.0 / (std::f64::consts::PI * std::f64::consts::PI * i * i)
}

/// Upper tail `P(T > t)` of Student's t with `nu` degrees of freedom.
pub fn student_t_sf(t: f64, nu: f64) -> f64 {
    if t.is_nan() {
        return f64::NAN;
    }
    if t == f64::INFINITY {
        return 0.0;
    }
    if t == f64::NEG_INFINITY {
        return 1.0;
    }
    let half = 0.5 * reg_inc_beta(nu / (nu + t * t), 0.5 * nu, 0.5);
    if t >= 0.0 {
        half
    } else {
        1.0 - half
    }
}

/// Distribution function of Student's t with `nu` degrees of freedom.
pub fn student_t_cdf(t: f64, nu: f64) -> f64 {
    1.0 - student_t_sf(t, nu)
}

/// Quantile of Student's t for `0 < p < 1`: for `p > 1/2`, `t = sqrt(nu (1 - x) / x)` with `I_x(nu/2, 1/2) = 2 (1 - p)`.
pub fn student_t_quantile(p: f64, nu: f64) -> f64 {
    if p == 0.5 {
        return 0.0;
    }
    if p < 0.5 {
        return -student_t_quantile(1.0 - p, nu);
    }
    let x = beta_inv(2.0 * (1.0 - p), 0.5 * nu, 0.5);
    (nu * (1.0 - x) / x).sqrt()
}

/// Upper tail `P(B >= k)` of `B ~ Bin(n, 1/2)`.
fn binom_half_sf(k: usize, n: usize) -> f64 {
    if k == 0 {
        1.0
    } else {
        1.0 - binom_cdf(k as i64 - 1, n, 0.5)
    }
}

/// Claimed direction of the change of the response from the low to the high level of the factor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// The response rises.
    Increases,
    /// The response falls.
    Decreases,
    /// The change lies inside `(-min_effect, min_effect)` (an equivalence claim).
    NoEffect,
}

impl Direction {
    /// Text form.
    pub fn as_str(self) -> &'static str {
        match self {
            Direction::Increases => "increases",
            Direction::Decreases => "decreases",
            Direction::NoEffect => "no_effect",
        }
    }
    /// Parse the text form.
    pub fn parse(s: &str) -> Option<Direction> {
        Some(match s {
            "increases" => Direction::Increases,
            "decreases" => Direction::Decreases,
            "no_effect" => Direction::NoEffect,
            _ => return None,
        })
    }
    /// Sign `s` of `d = s (y_high - y_low)`.
    pub fn sign(self) -> f64 {
        if self == Direction::Decreases {
            -1.0
        } else {
            1.0
        }
    }
}

/// Test statistic (Q8 (c)).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Statistic {
    /// Paired Student t on the mean difference.
    #[default]
    PairedT,
    /// Exact sign test on the pairs above (below) `min_effect`.
    Sign,
}

impl Statistic {
    /// Text form.
    pub fn as_str(self) -> &'static str {
        match self {
            Statistic::PairedT => "paired_t",
            Statistic::Sign => "sign",
        }
    }
    /// Parse the text form.
    pub fn parse(s: &str) -> Option<Statistic> {
        match s {
            "paired_t" => Some(Statistic::PairedT),
            "sign" => Some(Statistic::Sign),
            _ => None,
        }
    }
}

/// Outcome of a test.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Finding {
    /// The claim holds at the registered size and level.
    Confirmed,
    /// The claim fails: the effect is shown to be below the smallest effect that matters (or outside it, for no effect).
    Refuted,
    /// Neither.
    Inconclusive,
    /// Too many failed pairs, a held metric not matched, or fewer than 2 ok pairs.
    Void,
}

impl Finding {
    /// Text form.
    pub fn as_str(self) -> &'static str {
        match self {
            Finding::Confirmed => "confirmed",
            Finding::Refuted => "refuted",
            Finding::Inconclusive => "inconclusive",
            Finding::Void => "void",
        }
    }
    /// Parse the text form.
    pub fn parse(s: &str) -> Option<Finding> {
        Some(match s {
            "confirmed" => Finding::Confirmed,
            "refuted" => Finding::Refuted,
            "inconclusive" => Finding::Inconclusive,
            "void" => Finding::Void,
            _ => return None,
        })
    }
}

/// Statistics of a paired test.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Stats {
    /// Number of differences.
    pub n: usize,
    /// Mean difference `m`.
    pub mean: f64,
    /// Standard deviation (divisor `n - 1`).
    pub sd: f64,
    /// Standard error `sd / sqrt(n)`.
    pub se: f64,
    /// Quantile `t_{1 - a, n - 1}` at the level used.
    pub t: f64,
    /// One-sided lower bound `m - t SE`.
    pub lower: f64,
    /// One-sided upper bound `m + t SE`.
    pub upper: f64,
    /// p-value of the confirm test (no effect: the larger of the two one-sided p-values).
    pub p_value: f64,
    /// Level used.
    pub level: f64,
    /// Refuted with the opposite sign shown.
    pub opposite: bool,
    /// Share of differences above 0.
    pub sign_share: f64,
    /// Clopper-Pearson 0.95 interval of the share.
    pub sign_ci: (f64, f64),
    /// Every difference is exactly 0.
    pub all_zero: bool,
}

/// The test of `d` (differences in the claimed direction, `d = s (y_high - y_low)`) at `level`. With fewer than 2 values the
/// finding is `Void`.
pub fn paired_test(
    d: &[f64],
    level: f64,
    min_effect: f64,
    direction: Direction,
    statistic: Statistic,
) -> (Finding, Stats) {
    let n = d.len();
    let mut st = Stats {
        n,
        level,
        ..Default::default()
    };
    if n < 2 {
        return (Finding::Void, st);
    }
    let nf = n as f64;
    let mean = d.iter().sum::<f64>() / nf;
    let sd = (d.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / (nf - 1.0)).sqrt();
    let se = sd / nf.sqrt();
    let t = student_t_quantile(1.0 - level, nf - 1.0);
    let (lower, upper) = if se > 0.0 {
        (mean - t * se, mean + t * se)
    } else {
        (mean, mean)
    };
    let k_pos = d.iter().filter(|&&x| x > 0.0).count();
    st.mean = mean;
    st.sd = sd;
    st.se = se;
    st.t = t;
    st.lower = lower;
    st.upper = upper;
    st.sign_share = k_pos as f64 / nf;
    st.sign_ci = clopper_pearson(k_pos, n, 0.95);
    st.all_zero = d.iter().all(|&x| x == 0.0);
    // p-value of "mean > c" by t (SE = 0: 0 when the bound rule holds, else 1)
    let p_gt = |c: f64, holds: bool| -> f64 {
        if se > 0.0 {
            student_t_sf((mean - c) / se, nf - 1.0)
        } else if holds {
            0.0
        } else {
            1.0
        }
    };
    let p_lt = |c: f64, holds: bool| -> f64 {
        if se > 0.0 {
            student_t_sf((c - mean) / se, nf - 1.0)
        } else if holds {
            0.0
        } else {
            1.0
        }
    };
    let finding = match (statistic, direction) {
        (Statistic::PairedT, Direction::NoEffect) => {
            st.p_value = p_gt(-min_effect, mean > -min_effect).max(p_lt(min_effect, mean < min_effect));
            if -min_effect < lower && upper < min_effect {
                Finding::Confirmed
            } else if lower >= min_effect || upper <= -min_effect {
                Finding::Refuted
            } else {
                Finding::Inconclusive
            }
        }
        (Statistic::PairedT, _) => {
            st.p_value = p_gt(min_effect, mean >= min_effect);
            if lower >= min_effect {
                Finding::Confirmed
            } else if upper < min_effect {
                st.opposite = upper < 0.0;
                Finding::Refuted
            } else {
                Finding::Inconclusive
            }
        }
        (Statistic::Sign, Direction::NoEffect) => {
            let above_lo = d.iter().filter(|&&x| x > -min_effect).count();
            let below_hi = d.iter().filter(|&&x| x < min_effect).count();
            let above_hi = d.iter().filter(|&&x| x > min_effect).count();
            let below_lo = d.iter().filter(|&&x| x < -min_effect).count();
            st.p_value = binom_half_sf(above_lo, n).max(binom_half_sf(below_hi, n));
            if st.p_value <= level {
                Finding::Confirmed
            } else if binom_half_sf(above_hi, n) <= level || binom_half_sf(below_lo, n) <= level {
                Finding::Refuted
            } else {
                Finding::Inconclusive
            }
        }
        (Statistic::Sign, _) => {
            let k_up = d.iter().filter(|&&x| x > min_effect).count();
            let k_dn = d.iter().filter(|&&x| x < min_effect).count();
            let k_neg = d.iter().filter(|&&x| x < 0.0).count();
            st.p_value = binom_half_sf(k_up, n);
            if st.p_value <= level {
                Finding::Confirmed
            } else if binom_half_sf(k_dn, n) <= level {
                st.opposite = binom_half_sf(k_neg, n) <= level;
                Finding::Refuted
            } else {
                Finding::Inconclusive
            }
        }
    };
    (finding, st)
}

// ---------------------------------------------------------------- the record

/// The factor and its two levels.
#[derive(Debug, Clone, PartialEq)]
pub struct Factor {
    /// Parameter name.
    pub name: String,
    /// Unit.
    pub unit: String,
    /// Low level.
    pub low: f64,
    /// High level.
    pub high: f64,
}

/// The response metric.
#[derive(Debug, Clone, PartialEq)]
pub struct Response {
    /// Metric name.
    pub name: String,
    /// Unit.
    pub unit: String,
    /// Numerical resolution of the response declared for the evaluator (response unit, `>= 0`).
    pub resolution: f64,
}

/// Target of a held metric.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HeldTarget {
    /// Both arms are brought to this value.
    Value(f64),
    /// The low arm is evaluated with the compensating parameter at `at`; the high arm is brought to the low arm's value.
    AsLowArm {
        /// Value of the compensating parameter in the low arm.
        at: f64,
    },
}

/// A metric kept equal between the arms by solving one compensating parameter ("at equal flow").
#[derive(Debug, Clone, PartialEq)]
pub struct Held {
    /// Metric name.
    pub metric: String,
    /// Metric unit.
    pub unit: String,
    /// Target.
    pub target: HeldTarget,
    /// Absolute tolerance on the metric (metric unit).
    pub tolerance: f64,
    /// Compensating parameter.
    pub by: String,
    /// Its unit.
    pub by_unit: String,
    /// Bisection bracket of the compensating parameter.
    pub bracket: (f64, f64),
}

/// Bounds of a parameter of the domain.
#[derive(Debug, Clone, PartialEq)]
pub struct Bound {
    /// Parameter name.
    pub name: String,
    /// Unit.
    pub unit: String,
    /// Lower bound.
    pub lo: f64,
    /// Upper bound.
    pub hi: f64,
}

impl Bound {
    /// A bound.
    pub fn new(name: &str, unit: &str, lo: f64, hi: f64) -> Bound {
        Bound {
            name: name.into(),
            unit: unit.into(),
            lo,
            hi,
        }
    }
}

/// The evaluator a hypothesis is about.
#[derive(Debug, Clone, PartialEq)]
pub struct Scope {
    /// Evaluator id.
    pub id: String,
    /// Evaluator version.
    pub version: String,
    /// Tier.
    pub tier: Fidelity,
    /// Whether a repeat gives the same bits (then the spread of `d` is domain spread, not noise).
    pub deterministic: bool,
    /// Objective identity when an objective is the response.
    pub objective: Option<Identity>,
}

impl Scope {
    /// The scope of `model`.
    pub fn of(model: &dyn Model, deterministic: bool) -> Scope {
        Scope {
            id: model.id().into(),
            version: model.version().into(),
            tier: model.fidelity(),
            deterministic,
            objective: None,
        }
    }
}

/// What the claim was formed from.
#[derive(Debug, Clone, PartialEq)]
pub enum Origin {
    /// Entries of the ledger read before the registration (never evidence).
    Exploratory {
        /// The entries.
        entries: Vec<EntryRef>,
    },
    /// Literature or first principles.
    Prior {
        /// Citation.
        citation: String,
    },
}

/// A registered seeded re-run sample of the evidence (ADR 0662 D1): `pairs` pairs drawn from the record hash and seed, both arms
/// re-evaluated by [`verify`] with the trusted model at the tolerances given.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RerunSample {
    /// Number of pairs re-run (`1..=n_pairs`).
    pub pairs: usize,
    /// Relative tolerance of the re-run comparison.
    pub rtol: f64,
    /// Absolute tolerance of the re-run comparison.
    pub atol: f64,
}

/// A registered seeded sample of the design for the held-tolerance floor (ADR 0670 D1): `points` of the registered pairs, drawn
/// by [`floor_sample_pairs`]; the floor of [`held_floor`] is then the largest over the domain midpoint and their design points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FloorSample {
    /// Number of design pairs sampled (`1..=n_pairs`).
    pub points: usize,
}

/// The re-run sample of a hypothesis as [`verify`] derives it (ADR 0662 D2, D4).
#[derive(Debug, Clone, PartialEq)]
pub struct RerunPlan {
    /// Number of pairs.
    pub k: usize,
    /// Seed of the draw ([`rerun_seed`]).
    pub seed: u64,
    /// Sampled pairs, ascending.
    pub pairs: Vec<usize>,
    /// Relative tolerance.
    pub rtol: f64,
    /// Absolute tolerance.
    pub atol: f64,
}

/// A hypothesis as registered.
#[derive(Debug, Clone, PartialEq)]
pub struct Hypothesis {
    /// The claim in plain words, stored verbatim and never interpreted.
    pub claim_text: String,
    /// Factor and levels.
    pub factor: Factor,
    /// Response.
    pub response: Response,
    /// Direction.
    pub direction: Direction,
    /// Predicted mean change high minus low (response unit), optional.
    pub predicted_size: Option<(f64, f64)>,
    /// Smallest change that matters (response unit, `> 0`).
    pub min_effect: f64,
    /// Held metrics (at most one is supported).
    pub held: Vec<Held>,
    /// The other parameters and their bounds.
    pub domain: Vec<Bound>,
    /// Number of pairs.
    pub n_pairs: usize,
    /// Caller seed (the design seed is derived from it and the record hash).
    pub seed: u64,
    /// Evaluator.
    pub evaluator: Scope,
    /// Statistic.
    pub statistic: Statistic,
    /// Largest share of failed pairs before the result is void.
    pub max_failed_share: f64,
    /// Origin.
    pub origin: Origin,
    /// A larger re-test of this hypothesis id.
    pub supersedes: Option<String>,
    /// The same claim at another tier or evaluator version.
    pub replicates: Option<String>,
    /// Registered re-run sample (ADR 0662); `None` keeps the record of ADR 0624 bit for bit.
    pub rerun_sample: Option<RerunSample>,
    /// Registered sample of the design for the held-tolerance floor (ADR 0670); `None` keeps the midpoint rule of ADR 0665 and
    /// the record bit for bit.
    pub floor_sample: Option<FloorSample>,
}

impl Hypothesis {
    /// A hypothesis with no held metric, PairedT, failed share 0.2, origin prior (citation empty), no links.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        claim_text: &str,
        factor: Factor,
        response: Response,
        direction: Direction,
        min_effect: f64,
        domain: Vec<Bound>,
        n_pairs: usize,
        seed: u64,
        evaluator: Scope,
    ) -> Hypothesis {
        Hypothesis {
            claim_text: claim_text.into(),
            factor,
            response,
            direction,
            predicted_size: None,
            min_effect,
            held: Vec::new(),
            domain,
            n_pairs,
            seed,
            evaluator,
            statistic: Statistic::PairedT,
            max_failed_share: 0.2,
            origin: Origin::Prior {
                citation: String::new(),
            },
            supersedes: None,
            replicates: None,
            rerun_sample: None,
            floor_sample: None,
        }
    }

    /// Canonical JSON (keys sorted; numbers round-trip bit for bit).
    pub fn to_json(&self) -> Value {
        let target = |t: &HeldTarget| match t {
            HeldTarget::Value(v) => json!({"value": num(*v)}),
            HeldTarget::AsLowArm { at } => json!({"as_low_arm": num(*at)}),
        };
        let mut v = json!({
            "claim_text": self.claim_text,
            "factor": {"name": self.factor.name, "unit": self.factor.unit, "low": num(self.factor.low), "high": num(self.factor.high)},
            "response": {"name": self.response.name, "unit": self.response.unit, "resolution": num(self.response.resolution)},
            "direction": self.direction.as_str(),
            "predicted_size": self.predicted_size.map(|(a, b)| json!([num(a), num(b)])),
            "min_effect": num(self.min_effect),
            "held": self.held.iter().map(|h| json!({
                "metric": h.metric, "unit": h.unit, "target": target(&h.target), "tolerance": num(h.tolerance),
                "by": h.by, "by_unit": h.by_unit, "bracket": [num(h.bracket.0), num(h.bracket.1)],
            })).collect::<Vec<_>>(),
            "domain": self.domain.iter().map(|b| json!({"name": b.name, "unit": b.unit, "lo": num(b.lo), "hi": num(b.hi)})).collect::<Vec<_>>(),
            "design": {"kind": "paired", "n_pairs": self.n_pairs, "sampler": "latin_hypercube", "seed": self.seed},
            "evaluator": {
                "id": self.evaluator.id, "version": self.evaluator.version, "tier": self.evaluator.tier.as_str(),
                "deterministic": self.evaluator.deterministic,
                "objective": self.evaluator.objective.as_ref().map(|o| json!({"id": o.id, "version": o.version, "hash": o.hash})),
            },
            "test": {"statistic": self.statistic.as_str(), "max_failed_share": num(self.max_failed_share)},
            "origin": match &self.origin {
                Origin::Exploratory { entries } => json!({"kind": "exploratory", "entries": entries.iter().map(|r| json!({"index": r.index, "hash": r.hash})).collect::<Vec<_>>()}),
                Origin::Prior { citation } => json!({"kind": "prior", "citation": citation}),
            },
            "supersedes": self.supersedes,
            "replicates": self.replicates,
        });
        // written only when registered, so a record without a sample keeps its text and hash (ADR 0662 D1)
        if let (Some(r), Some(o)) = (&self.rerun_sample, v.as_object_mut()) {
            o.insert(
                "rerun_sample".into(),
                json!({"pairs": r.pairs, "rtol": num(r.rtol), "atol": num(r.atol)}),
            );
        }
        // the same rule for the floor sample (ADR 0670 D1)
        if let (Some(f), Some(o)) = (&self.floor_sample, v.as_object_mut()) {
            o.insert("floor_sample".into(), json!({"points": f.points}));
        }
        v
    }

    /// Parse [`Hypothesis::to_json`].
    pub fn from_json(v: &Value) -> Result<Hypothesis, String> {
        let s = |v: &Value, k: &str| -> Result<String, String> {
            v.get(k)
                .and_then(Value::as_str)
                .map(str::to_string)
                .ok_or_else(|| format!("record: '{k}' missing"))
        };
        let f = |v: &Value, k: &str| -> Result<f64, String> {
            v.get(k)
                .and_then(Value::as_f64)
                .ok_or_else(|| format!("record: '{k}' missing or not a number"))
        };
        let pair = |v: &Value, k: &str| -> Result<(f64, f64), String> {
            match v.get(k).and_then(Value::as_array).map(|a| a.as_slice()) {
                Some([a, b]) => Ok((
                    a.as_f64().ok_or_else(|| format!("record: '{k}' not numbers"))?,
                    b.as_f64().ok_or_else(|| format!("record: '{k}' not numbers"))?,
                )),
                _ => Err(format!("record: '{k}' is not a pair")),
            }
        };
        let get = |k: &str| v.get(k).ok_or_else(|| format!("record: '{k}' missing"));
        let fa = get("factor")?;
        let re = get("response")?;
        let de = get("design")?;
        let ev = get("evaluator")?;
        let te = get("test")?;
        let or = get("origin")?;
        let tier = s(ev, "tier")?;
        let held = v
            .get("held")
            .and_then(Value::as_array)
            .ok_or("record: 'held' missing")?
            .iter()
            .map(|h| {
                let t = h.get("target").ok_or("record: held target missing")?;
                let target = if let Some(x) = t.get("value").and_then(Value::as_f64) {
                    HeldTarget::Value(x)
                } else if let Some(x) = t.get("as_low_arm").and_then(Value::as_f64) {
                    HeldTarget::AsLowArm { at: x }
                } else {
                    return Err("record: held target is neither value nor as_low_arm".to_string());
                };
                Ok(Held {
                    metric: s(h, "metric")?,
                    unit: s(h, "unit")?,
                    target,
                    tolerance: f(h, "tolerance")?,
                    by: s(h, "by")?,
                    by_unit: s(h, "by_unit")?,
                    bracket: pair(h, "bracket")?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let domain = v
            .get("domain")
            .and_then(Value::as_array)
            .ok_or("record: 'domain' missing")?
            .iter()
            .map(|b| Ok(Bound::new(&s(b, "name")?, &s(b, "unit")?, f(b, "lo")?, f(b, "hi")?)))
            .collect::<Result<Vec<_>, String>>()?;
        let origin = match s(or, "kind")?.as_str() {
            "exploratory" => Origin::Exploratory {
                entries: or
                    .get("entries")
                    .and_then(Value::as_array)
                    .ok_or("record: origin entries missing")?
                    .iter()
                    .map(|r| {
                        Ok(EntryRef {
                            index: r.get("index").and_then(Value::as_u64).ok_or("record: origin index")? as usize,
                            hash: r.get("hash").and_then(Value::as_str).map(str::to_string),
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()?,
            },
            "prior" => Origin::Prior {
                citation: s(or, "citation")?,
            },
            k => return Err(format!("record: origin kind '{k}' unknown")),
        };
        let opt_id = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
        let rerun_sample = match v.get("rerun_sample") {
            None | Some(Value::Null) => None,
            Some(r) => Some(RerunSample {
                pairs: r
                    .get("pairs")
                    .and_then(Value::as_u64)
                    .ok_or("record: rerun_sample pairs missing")? as usize,
                rtol: f(r, "rtol")?,
                atol: f(r, "atol")?,
            }),
        };
        let floor_sample = match v.get("floor_sample") {
            None | Some(Value::Null) => None,
            Some(r) => Some(FloorSample {
                points: r
                    .get("points")
                    .and_then(Value::as_u64)
                    .ok_or("record: floor_sample points missing")? as usize,
            }),
        };
        Ok(Hypothesis {
            claim_text: s(v, "claim_text")?,
            factor: Factor {
                name: s(fa, "name")?,
                unit: s(fa, "unit")?,
                low: f(fa, "low")?,
                high: f(fa, "high")?,
            },
            response: Response {
                name: s(re, "name")?,
                unit: s(re, "unit")?,
                resolution: f(re, "resolution")?,
            },
            direction: Direction::parse(&s(v, "direction")?).ok_or("record: direction unknown")?,
            predicted_size: match v.get("predicted_size") {
                None | Some(Value::Null) => None,
                Some(_) => Some(pair(v, "predicted_size")?),
            },
            min_effect: f(v, "min_effect")?,
            held,
            domain,
            n_pairs: de
                .get("n_pairs")
                .and_then(Value::as_u64)
                .ok_or("record: n_pairs missing")? as usize,
            seed: de.get("seed").and_then(Value::as_u64).ok_or("record: seed missing")?,
            evaluator: Scope {
                id: s(ev, "id")?,
                version: s(ev, "version")?,
                tier: Fidelity::parse_any(&tier).ok_or_else(|| format!("record: tier '{tier}' unknown"))?,
                deterministic: ev.get("deterministic").and_then(Value::as_bool).unwrap_or(false),
                objective: match ev.get("objective") {
                    None | Some(Value::Null) => None,
                    Some(o) => Some(Identity {
                        id: s(o, "id")?,
                        version: s(o, "version")?,
                        hash: s(o, "hash")?,
                    }),
                },
            },
            statistic: Statistic::parse(&s(te, "statistic")?).ok_or("record: statistic unknown")?,
            max_failed_share: f(te, "max_failed_share")?,
            origin,
            supersedes: opt_id("supersedes"),
            replicates: opt_id("replicates"),
            rerun_sample,
            floor_sample,
        })
    }

    /// `fnv1a64:` and 16 hex digits of FNV-1a 64 of the canonical JSON text.
    pub fn hash(&self) -> String {
        hash_text(&self.to_json().to_string())
    }
}

fn hash_text(t: &str) -> String {
    format!("fnv1a64:{:016x}", fnv64(t.as_bytes()))
}

/// Design seed: FNV-1a 64 of `<record hash>:<seed>`.
pub fn derived_seed(hash: &str, seed: u64) -> u64 {
    fnv64(format!("{hash}:{seed}").as_bytes())
}

/// Seed of the re-run sample: FNV-1a 64 of `<record hash>:rerun:<seed>` (ADR 0662 D2).
pub fn rerun_seed(hash: &str, seed: u64) -> u64 {
    fnv64(format!("{hash}:rerun:{seed}").as_bytes())
}

/// `k` distinct pairs of `0..n`, ascending: the first `k` steps of the Fisher-Yates shuffle with `Rng::new(seed)` (ADR 0662 D2).
pub fn rerun_pairs(n: usize, k: usize, seed: u64) -> Vec<usize> {
    let k = k.min(n);
    let mut idx: Vec<usize> = (0..n).collect();
    let mut r = Rng::new(seed);
    for i in 0..k {
        let j = i + r.below(n - i);
        idx.swap(i, j);
    }
    let mut s = idx[..k].to_vec();
    s.sort_unstable();
    s
}

/// Probability that a uniform sample of `k` of `n` pairs contains none of `bad` pairs: `C(n - bad, k) / C(n, k)` as the product
/// `prod_{i<k} (n - bad - i) / (n - i)` (ADR 0662 D3).
pub fn rerun_miss_probability(n: usize, bad: usize, k: usize) -> f64 {
    if k > n || bad > n {
        return f64::NAN;
    }
    let mut p = 1.0;
    for i in 0..k {
        if n - i <= bad {
            return 0.0;
        }
        p *= (n - bad - i) as f64 / (n - i) as f64;
    }
    p
}

/// Smallest `k` with [`rerun_miss_probability`]`(n, bad, k) <= miss`; `None` for `bad = 0`, `bad > n`, `n = 0` or `miss` outside
/// `(0, 1)` (ADR 0662 D3).
pub fn rerun_sample_size(n: usize, bad: usize, miss: f64) -> Option<usize> {
    if n == 0 || bad == 0 || bad > n || !(miss > 0.0 && miss < 1.0) {
        return None;
    }
    (1..=n).find(|&k| rerun_miss_probability(n, bad, k) <= miss)
}

/// Seed of the floor sample: FNV-1a 64 of `<record hash>:floor:<seed>` (ADR 0670 D1).
pub fn floor_seed(hash: &str, seed: u64) -> u64 {
    fnv64(format!("{hash}:floor:{seed}").as_bytes())
}

/// The design pairs of the floor sample of `h`, ascending ([`rerun_pairs`] with [`floor_seed`]); `None` without a sample.
pub fn floor_sample_pairs(h: &Hypothesis) -> Option<Vec<usize>> {
    h.floor_sample
        .map(|f| rerun_pairs(h.n_pairs, f.points, floor_seed(&h.hash(), h.seed)))
}

/// The re-run plan of a registered record (`None` without a sample).
fn rerun_plan(h: &Hypothesis, hash: &str) -> Option<RerunPlan> {
    h.rerun_sample.map(|r| {
        let seed = rerun_seed(hash, h.seed);
        RerunPlan {
            k: r.pairs,
            seed,
            pairs: rerun_pairs(h.n_pairs, r.pairs, seed),
            rtol: r.rtol,
            atol: r.atol,
        }
    })
}

/// The registered design: `n_pairs` points of the domain (a Latin hypercube from the derived seed, scaled to the bounds).
pub fn design_points(h: &Hypothesis, seed: u64) -> Vec<Vec<f64>> {
    latin_hypercube(h.n_pairs, h.domain.len(), seed)
        .into_iter()
        .map(|u| h.domain.iter().zip(u).map(|(b, u)| b.lo + (b.hi - b.lo) * u).collect())
        .collect()
}

// ---------------------------------------------------------------- registrations in a ledger

/// Batch membership (Holm within a batch, B2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Batch {
    /// Id of the first member.
    pub first: String,
    /// Number of members.
    pub size: usize,
    /// 0-based member index.
    pub member: usize,
}

/// A registration as stored.
#[derive(Debug, Clone, PartialEq)]
pub struct Registered {
    /// Entry index.
    pub index: usize,
    /// `H-<n>`.
    pub id: String,
    /// Stored record hash.
    pub hash: String,
    /// Family slot.
    pub slot: usize,
    /// Level of the slot.
    pub level: f64,
    /// Batch membership.
    pub batch: Option<Batch>,
    /// Design seed.
    pub derived_seed: u64,
    /// The record (or why it does not parse).
    pub record: Result<Hypothesis, String>,
    /// Hash of the stored record text (recomputed).
    pub recomputed_hash: String,
}

fn is_registration(e: &Entry) -> bool {
    e.kind == KIND_HYPOTHESIS && e.evaluator == DRIVER_ID
}

fn parse_registration(i: usize, e: &Entry) -> Registered {
    let st = &e.settings;
    let rec_v = st.get("record").cloned().unwrap_or(Value::Null);
    let batch = st.get("family").and_then(|f| f.get("batch")).and_then(|b| {
        Some(Batch {
            first: b.get("first")?.as_str()?.to_string(),
            size: b.get("size")?.as_u64()? as usize,
            member: b.get("member")?.as_u64()? as usize,
        })
    });
    Registered {
        index: i,
        id: st.get("id").and_then(Value::as_str).unwrap_or("").to_string(),
        hash: st.get("hash").and_then(Value::as_str).unwrap_or("").to_string(),
        slot: st
            .get("family")
            .and_then(|f| f.get("slot"))
            .and_then(Value::as_u64)
            .unwrap_or(0) as usize,
        level: st
            .get("family")
            .and_then(|f| f.get("level"))
            .and_then(Value::as_f64)
            .unwrap_or(f64::NAN),
        batch,
        derived_seed: st.get("derived_seed").and_then(Value::as_u64).unwrap_or(0),
        record: Hypothesis::from_json(&rec_v),
        recomputed_hash: hash_text(&rec_v.to_string()),
    }
}

/// Every registration of the ledger, in order.
pub fn registrations(entries: &[Entry]) -> Vec<Registered> {
    entries
        .iter()
        .enumerate()
        .filter(|(_, e)| is_registration(e))
        .map(|(i, e)| parse_registration(i, e))
        .collect()
}

fn opens_slot(r: &Registered) -> bool {
    r.batch.as_ref().is_none_or(|b| b.member == 0)
}

/// Slot of each registration recounted from the file order: (expected id, expected slot).
fn recount(regs: &[Registered]) -> Vec<(String, usize)> {
    let mut slot = 0;
    regs.iter()
        .enumerate()
        .map(|(j, r)| {
            if opens_slot(r) {
                slot += 1;
            }
            (format!("H-{}", j + 1), slot)
        })
        .collect()
}

fn validate(h: &Hypothesis, entries: &[Entry], regs: &[Registered], max_held: usize) -> Result<(), String> {
    let finite = |x: f64, what: &str| -> Result<(), String> {
        if x.is_finite() {
            Ok(())
        } else {
            Err(format!("{what}: {x} is not finite"))
        }
    };
    if h.claim_text.trim().is_empty() {
        return Err("claim_text: empty".into());
    }
    if h.factor.name.is_empty() || h.factor.unit.is_empty() {
        return Err("factor: name and unit must be given".into());
    }
    finite(h.factor.low, "factor.low")?;
    finite(h.factor.high, "factor.high")?;
    if h.factor.low >= h.factor.high {
        return Err(format!(
            "factor: low {} must be below high {}",
            h.factor.low, h.factor.high
        ));
    }
    if h.response.name.is_empty() || h.response.unit.is_empty() {
        return Err("response: name and unit must be given".into());
    }
    if !(h.response.resolution.is_finite() && h.response.resolution >= 0.0) {
        return Err(format!(
            "response.resolution: {} is not a finite non-negative number",
            h.response.resolution
        ));
    }
    if !(h.min_effect.is_finite() && h.min_effect > 0.0) {
        return Err(format!(
            "min_effect: {} must be finite and positive (a claim needs a size that matters; Schuirmann 1987 for no effect)",
            h.min_effect
        ));
    }
    if h.min_effect < K_RES * h.response.resolution {
        return Err(format!(
            "min_effect: {} is below {K_RES} times the declared resolution {} of '{}'",
            h.min_effect, h.response.resolution, h.response.name
        ));
    }
    if h.n_pairs < MIN_PAIRS {
        return Err(format!(
            "design.n_pairs: {} is below the smallest {MIN_PAIRS}",
            h.n_pairs
        ));
    }
    for (i, b) in h.domain.iter().enumerate() {
        if b.name.is_empty() || b.unit.is_empty() {
            return Err(format!("domain[{i}]: name and unit must be given"));
        }
        if !(b.lo.is_finite() && b.hi.is_finite() && b.lo < b.hi) {
            return Err(format!(
                "domain '{}': bounds [{}, {}] must be finite with lo < hi",
                b.name, b.lo, b.hi
            ));
        }
        if h.domain[..i].iter().any(|c| c.name == b.name) {
            return Err(format!("domain '{}': given twice", b.name));
        }
        if b.name == h.factor.name {
            return Err(format!("domain '{}': the factor cannot be in the domain", b.name));
        }
    }
    if h.held.len() > max_held {
        return Err(if max_held <= 1 {
            format!(
                "held: {} held metrics; `register` takes one, `register_with_floor` takes up to {MAX_HELD} (ADR 0665)",
                h.held.len()
            )
        } else {
            format!(
                "held: {} held metrics; at most {MAX_HELD} are supported (ADR 0665)",
                h.held.len()
            )
        });
    }
    for (i, x) in h.held.iter().enumerate() {
        if h.held[..i].iter().any(|y| y.metric == x.metric) {
            return Err(format!("held '{}': given twice", x.metric));
        }
        if h.held[..i].iter().any(|y| y.by == x.by) {
            return Err(format!(
                "held '{}': the compensating parameter '{}' is given twice",
                x.metric, x.by
            ));
        }
    }
    if h.held.len() > 1 && !same_target_kind(&h.held) {
        return Err("held: targets must be all values or all as_low_arm (a mix is not covered, ADR 0665)".into());
    }
    for x in &h.held {
        if x.metric.is_empty() || x.unit.is_empty() || x.by.is_empty() || x.by_unit.is_empty() {
            return Err("held: metric, unit, by and by_unit must be given".into());
        }
        if x.metric == h.response.name {
            return Err(format!("held '{}': the response cannot be held", x.metric));
        }
        if x.by == h.factor.name || h.domain.iter().any(|b| b.name == x.by) {
            return Err(format!(
                "held '{}': the compensating parameter '{}' is the factor or in the domain",
                x.metric, x.by
            ));
        }
        if !(x.tolerance.is_finite() && x.tolerance > 0.0) {
            return Err(format!(
                "held '{}': tolerance {} must be finite and positive",
                x.metric, x.tolerance
            ));
        }
        if !(x.bracket.0.is_finite() && x.bracket.1.is_finite() && x.bracket.0 < x.bracket.1) {
            return Err(format!(
                "held '{}': bracket [{}, {}] must be finite with lo < hi",
                x.metric, x.bracket.0, x.bracket.1
            ));
        }
        match x.target {
            HeldTarget::Value(t) => finite(t, "held.target")?,
            HeldTarget::AsLowArm { at } => {
                if !(at >= x.bracket.0 && at <= x.bracket.1) {
                    return Err(format!(
                        "held '{}': as_low_arm at {at} lies outside the bracket [{}, {}]",
                        x.metric, x.bracket.0, x.bracket.1
                    ));
                }
            }
        }
    }
    if !(h.max_failed_share >= 0.0 && h.max_failed_share < 1.0) {
        return Err(format!(
            "test.max_failed_share: {} must lie in [0, 1)",
            h.max_failed_share
        ));
    }
    if h.evaluator.id.trim().is_empty() || h.evaluator.version.trim().is_empty() {
        return Err("evaluator: id and version must be given".into());
    }
    if let Some((a, b)) = h.predicted_size {
        if !(a.is_finite() && b.is_finite() && a <= b) {
            return Err(format!("predicted_size: [{a}, {b}] must be finite with lo <= hi"));
        }
    }
    if let Some(r) = &h.rerun_sample {
        if r.pairs == 0 || r.pairs > h.n_pairs {
            return Err(format!(
                "rerun_sample: {} pairs; it must lie in 1..={} (the registered pairs)",
                r.pairs, h.n_pairs
            ));
        }
        if !(r.rtol.is_finite() && r.rtol >= 0.0 && r.atol.is_finite() && r.atol >= 0.0) {
            return Err(format!(
                "rerun_sample: tolerances rtol {} and atol {} must be finite and non-negative",
                r.rtol, r.atol
            ));
        }
        if h.evaluator.tier == Fidelity::Measured {
            return Err("rerun_sample: tier measured cannot be re-run".into());
        }
        if !h.evaluator.deterministic && r.rtol == 0.0 && r.atol == 0.0 {
            return Err(
                "rerun_sample: the evaluator is not declared deterministic, so a re-run at zero tolerances cannot reproduce it"
                    .into(),
            );
        }
    }
    if let Some(f) = &h.floor_sample {
        if f.points == 0 || f.points > h.n_pairs {
            return Err(format!(
                "floor_sample: {} points; it must lie in 1..={} (the registered pairs)",
                f.points, h.n_pairs
            ));
        }
        if max_held <= 1 {
            return Err(
                "floor_sample: `register` computes no held-tolerance floor; use `register_with_floor` (ADR 0670)"
                    .into(),
            );
        }
        if h.held.is_empty() {
            return Err("floor_sample: no held metric, so there is no held-tolerance floor to sample".into());
        }
    }
    if let Origin::Exploratory { entries: refs } = &h.origin {
        for r in refs {
            resolve(entries, r).map_err(|e| format!("origin: {e}"))?;
        }
    }
    let known = |id: &str| regs.iter().find(|r| r.id == id);
    if let Some(s) = &h.supersedes {
        let mut depth = 1;
        let mut cur = known(s).ok_or_else(|| format!("supersedes: no hypothesis '{s}' in this ledger"))?;
        while let Some(next) = cur.record.as_ref().ok().and_then(|r| r.supersedes.clone()) {
            depth += 1;
            match known(&next) {
                Some(n) => cur = n,
                None => break,
            }
        }
        if depth > MAX_RETESTS {
            return Err(format!(
                "supersedes: '{s}' is already re-test {} of its claim; at most {MAX_RETESTS} re-tests (Q10 (c))",
                depth - 1
            ));
        }
    }
    if let Some(s) = &h.replicates {
        known(s).ok_or_else(|| format!("replicates: no hypothesis '{s}' in this ledger"))?;
    }
    Ok(())
}

fn registration_entry(h: &Hypothesis, id: &str, slot: usize, batch: Option<&Batch>) -> Entry {
    let hash = h.hash();
    let ds = derived_seed(&hash, h.seed);
    let parents = match &h.origin {
        Origin::Exploratory { entries } => entries.clone(),
        Origin::Prior { .. } => Vec::new(),
    };
    Entry {
        kind: KIND_HYPOTHESIS.into(),
        study: "hypothesis".into(),
        variant: id.into(),
        evaluator: DRIVER_ID.into(),
        evaluator_version: DRIVER_VERSION.into(),
        fidelity: h.evaluator.tier,
        verdict: Verdict::Ok,
        settings: json!({
            "record": h.to_json(), "hash": hash, "id": id, "derived_seed": ds,
            "family": {"slot": slot, "level": num(series_level(slot)),
                       "batch": batch.map(|b| json!({"first": b.first, "size": b.size, "member": b.member}))},
        }),
        seed: Some(ds),
        provenance: json!({"driver": "hypothesis", "id": id}),
        parents,
        ..Default::default()
    }
}

/// Register one hypothesis (validated; nothing is appended when it is refused).
pub fn register(ledger: &Ledger, h: &Hypothesis) -> Result<Registered, String> {
    register_batch(ledger, std::slice::from_ref(h)).map(|mut v| v.remove(0))
}

/// Register `hs` in one write and one slot of the series; with more than one, the members are judged together by Holm's
/// procedure at the slot's level.
pub fn register_batch(ledger: &Ledger, hs: &[Hypothesis]) -> Result<Vec<Registered>, String> {
    if hs.is_empty() {
        return Err("register: no hypothesis".into());
    }
    let entries = ledger.read()?.entries;
    let regs = registrations(&entries);
    let slot = recount(&regs).last().map_or(0, |x| x.1) + 1;
    let first = format!("H-{}", regs.len() + 1);
    let mut out = Vec::new();
    for (j, h) in hs.iter().enumerate() {
        validate(h, &entries, &regs, 1).map_err(|e| format!("hypothesis {}: {e}", regs.len() + j + 1))?;
        let id = format!("H-{}", regs.len() + j + 1);
        let batch = (hs.len() > 1).then(|| Batch {
            first: first.clone(),
            size: hs.len(),
            member: j,
        });
        out.push(registration_entry(h, &id, slot, batch.as_ref()));
    }
    ledger.append(&out)?;
    let entries = ledger.read()?.entries;
    let base = entries.len() - out.len();
    Ok((0..out.len())
        .map(|j| parse_registration(base + j, &entries[base + j]))
        .collect())
}

/// Withdraw a hypothesis before its first evidence entry (it keeps its slot).
pub fn withdraw(ledger: &Ledger, id: &str, reason: &str) -> Result<(), String> {
    let entries = ledger.read()?.entries;
    let regs = registrations(&entries);
    let r = regs
        .iter()
        .find(|r| r.id == id)
        .ok_or_else(|| format!("withdraw: no hypothesis '{id}'"))?;
    if let Some((k, _)) = entries.iter().enumerate().find(|(_, e)| is_evidence_of(e, id)) {
        return Err(format!(
            "withdraw: {id} has evidence (entry {k}); a registration with evidence cannot be withdrawn"
        ));
    }
    if reason.trim().is_empty() {
        return Err("withdraw: a reason is needed".into());
    }
    ledger.append(&[Entry {
        kind: KIND_WITHDRAWN.into(),
        study: "hypothesis".into(),
        variant: id.into(),
        evaluator: DRIVER_ID.into(),
        evaluator_version: DRIVER_VERSION.into(),
        fidelity: r.record.as_ref().map_or(Fidelity::Analytic, |h| h.evaluator.tier),
        verdict: Verdict::Ok,
        settings: json!({"id": id, "hash": r.hash, "reason": reason}),
        provenance: json!({"driver": "hypothesis", "hypothesis": id}),
        parents: vec![entry_ref(r.index, &entries[r.index])],
        ..Default::default()
    }])
}

// ---------------------------------------------------------------- evidence

/// Arm of a pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arm {
    /// Factor at its low level.
    Low,
    /// Factor at its high level.
    High,
}

impl Arm {
    fn as_str(self) -> &'static str {
        match self {
            Arm::Low => "low",
            Arm::High => "high",
        }
    }
    fn parse(s: &str) -> Option<Arm> {
        match s {
            "low" => Some(Arm::Low),
            "high" => Some(Arm::High),
            _ => None,
        }
    }
    fn slot(self) -> usize {
        match self {
            Arm::Low => 0,
            Arm::High => 1,
        }
    }
}

fn is_evidence_of(e: &Entry, id: &str) -> bool {
    e.kind == "evaluation"
        && e.provenance.get("driver").and_then(Value::as_str) == Some("hypothesis")
        && e.provenance.get("hypothesis").and_then(Value::as_str) == Some(id)
}

fn params_for(h: &Hypothesis, x: &[f64], arm: Arm, by: &[f64]) -> Vec<Quantity> {
    let mut p = vec![Quantity::new(
        &h.factor.name,
        if arm == Arm::Low { h.factor.low } else { h.factor.high },
        &h.factor.unit,
    )];
    p.extend(h.domain.iter().zip(x).map(|(b, &v)| Quantity::new(&b.name, v, &b.unit)));
    for (hd, &v) in h.held.iter().zip(by) {
        p.push(Quantity::new(&hd.by, v, &hd.by_unit));
    }
    p
}

fn same_target_kind(held: &[Held]) -> bool {
    held.iter().all(|x| matches!(x.target, HeldTarget::Value(_)))
        || held.iter().all(|x| matches!(x.target, HeldTarget::AsLowArm { .. }))
}

fn safe_eval(model: &dyn Model, p: &[Quantity]) -> Outcome {
    match catch_unwind(AssertUnwindSafe(|| model.evaluate(p))) {
        Ok(o) => o,
        Err(_) => Outcome {
            verdict: Verdict::Diverged,
            reason: "the evaluation panicked".into(),
            ..Default::default()
        },
    }
}

fn metric_of(o: &Outcome, name: &str, unit: &str) -> Option<f64> {
    o.metrics
        .iter()
        .find(|q| q.name == name && q.unit == unit)
        .map(|q| q.value)
}

/// Evaluate one arm; with a held metric, solve the compensating parameter by bisection. Returns the evidence parameters, the
/// outcome and the number of evaluations.
fn eval_arm(
    model: &dyn Model,
    h: &Hypothesis,
    x: &[f64],
    arm: Arm,
    low_held: Option<&[f64]>,
) -> (Vec<Quantity>, Outcome, usize) {
    if h.held.len() > 1 {
        return eval_arm_coupled(model, h, x, arm, low_held);
    }
    let Some(hd) = h.held.first() else {
        let p = params_for(h, x, arm, &[]);
        let o = safe_eval(model, &p);
        return (p, o, 1);
    };
    let target = match (hd.target, arm) {
        (HeldTarget::AsLowArm { at }, Arm::Low) => {
            let p = params_for(h, x, arm, &[at]);
            let o = safe_eval(model, &p);
            return (p, o, 1);
        }
        (HeldTarget::AsLowArm { at }, Arm::High) => match low_held {
            Some(t) => t[0],
            None => {
                // the low arm failed, so the pair is failed; the high arm is evaluated at the low arm's setting for the record
                let p = params_for(h, x, arm, &[at]);
                let o = safe_eval(model, &p);
                return (p, o, 1);
            }
        },
        (HeldTarget::Value(t), _) => t,
    };
    let tol = hd.tolerance;
    let mut it = 0;
    let mut f = |v: f64| {
        it += 1;
        let p = params_for(h, x, arm, &[v]);
        let o = safe_eval(model, &p);
        let r = if o.verdict == Verdict::Ok {
            metric_of(&o, &hd.metric, &hd.unit).map(|m| m - target)
        } else {
            None
        };
        (p, o, r)
    };
    let (lo, hi) = hd.bracket;
    let (pl, ol, rl) = f(lo);
    let Some(rl) = rl else { return (pl, ol, it) };
    if rl.abs() <= tol {
        return (pl, ol, it);
    }
    let (ph, oh, rh) = f(hi);
    let Some(rh) = rh else { return (ph, oh, it) };
    if rh.abs() <= tol || rl.signum() == rh.signum() {
        return (ph, oh, it);
    }
    let (mut a, mut ra, mut b) = (lo, rl, hi);
    let mut last = (ph, oh);
    for _ in 0..200 {
        let m = a + 0.5 * (b - a);
        if m <= a || m >= b {
            break;
        }
        let (pm, om, rm) = f(m);
        let Some(rm) = rm else { return (pm, om, it) };
        if rm.abs() <= tol {
            return (pm, om, it);
        }
        if rm.signum() == ra.signum() {
            a = m;
            ra = rm;
        } else {
            b = m;
        }
        last = (pm, om);
    }
    (last.0, last.1, it)
}

/// Evaluate one arm of a record with two or more held metrics (ADR 0665 D1): the compensating parameters are solved together
/// by a damped Newton iteration on the held residuals. `low_held` is the low arm's held metric values (as_low_arm targets).
fn eval_arm_coupled(
    model: &dyn Model,
    h: &Hypothesis,
    x: &[f64],
    arm: Arm,
    low_held: Option<&[f64]>,
) -> (Vec<Quantity>, Outcome, usize) {
    let ats: Option<Vec<f64>> = h
        .held
        .iter()
        .map(|hd| match hd.target {
            HeldTarget::AsLowArm { at } => Some(at),
            HeldTarget::Value(_) => None,
        })
        .collect();
    let single = |by: &[f64]| {
        let p = params_for(h, x, arm, by);
        let o = safe_eval(model, &p);
        (p, o, 1)
    };
    match (&ats, arm) {
        (Some(at), Arm::Low) => single(at),
        (Some(at), Arm::High) => match low_held {
            Some(t) => solve_holds(model, h, x, arm, t, at.clone()),
            // the low arm failed, so the pair is failed; the high arm is evaluated at the low arm's setting for the record
            None => single(at),
        },
        (None, _) => {
            // a mixed record is refused at registration; one written by hand never matches (NaN target)
            let t: Vec<f64> = h
                .held
                .iter()
                .map(|hd| match hd.target {
                    HeldTarget::Value(t) => t,
                    HeldTarget::AsLowArm { .. } => f64::NAN,
                })
                .collect();
            let start = h
                .held
                .iter()
                .map(|hd| hd.bracket.0 + 0.5 * (hd.bracket.1 - hd.bracket.0))
                .collect();
            solve_holds(model, h, x, arm, &t, start)
        }
    }
}

/// One evaluation of an arm at the compensating values `u`: the parameters, the outcome and the held residuals (`None` when
/// the evaluation failed or a held metric is missing).
fn eval_holds(
    model: &dyn Model,
    h: &Hypothesis,
    (x, arm): (&[f64], Arm),
    targets: &[f64],
    u: &[f64],
    count: &mut usize,
) -> (Vec<Quantity>, Outcome, Option<Vec<f64>>) {
    *count += 1;
    let p = params_for(h, x, arm, u);
    let o = safe_eval(model, &p);
    let r = if o.verdict == Verdict::Ok {
        h.held
            .iter()
            .zip(targets)
            .map(|(hd, t)| metric_of(&o, &hd.metric, &hd.unit).map(|g| g - t))
            .collect()
    } else {
        None
    };
    (p, o, r)
}

/// Damped, projected Newton iteration on the held residuals `r_j = g_j(u) - t_j` (ADR 0665 D1; Dennis and Schnabel 1983,
/// chapters 5 and 6): forward-difference Jacobian, step `J delta = -r`, the trial `P(u + lambda delta)` (P the bracket box)
/// accepted when the tolerance-scaled residual norm falls below `(1 - 1e-4 lambda)` times its value; `lambda` halved at most
/// 30 times; at most 50 iterations. Returns the evidence (the accepted iterate's evaluation) and the number of evaluations.
fn solve_holds(
    model: &dyn Model,
    h: &Hypothesis,
    x: &[f64],
    arm: Arm,
    targets: &[f64],
    start: Vec<f64>,
) -> (Vec<Quantity>, Outcome, usize) {
    let held = &h.held;
    let m = held.len();
    let boxed = |k: usize, v: f64| v.max(held[k].bracket.0).min(held[k].bracket.1);
    let scaled = |r: &[f64]| {
        r.iter()
            .zip(held)
            .map(|(r, hd)| (r / hd.tolerance) * (r / hd.tolerance))
            .sum::<f64>()
            .sqrt()
    };
    let matched = |r: &[f64]| r.iter().zip(held).all(|(r, hd)| r.abs() <= hd.tolerance);
    let mut it = 0;
    let mut u: Vec<f64> = start.iter().enumerate().map(|(k, &v)| boxed(k, v)).collect();
    let (mut p, mut o, r) = eval_holds(model, h, (x, arm), targets, &u, &mut it);
    let Some(mut r) = r else { return (p, o, it) };
    for _ in 0..NEWTON_MAX_ITER {
        if matched(&r) {
            return (p, o, it);
        }
        let mut jac = vec![vec![0.0; m]; m];
        for k in 0..m {
            let (lo, hi) = held[k].bracket;
            let mut step = f64::EPSILON.sqrt() * u[k].abs().max(hi - lo);
            if u[k] + step > hi {
                step = -step;
            }
            let mut up = u.clone();
            up[k] += step;
            let (_, _, rp) = eval_holds(model, h, (x, arm), targets, &up, &mut it);
            let Some(rp) = rp else { return (p, o, it) };
            for j in 0..m {
                jac[j][k] = (rp[j] - r[j]) / step;
            }
        }
        let Some(delta) = gauss_solve(jac, r.iter().map(|v| -v).collect()) else {
            return (p, o, it);
        };
        let m0 = scaled(&r);
        let mut lambda = 1.0;
        let mut accepted = false;
        for _ in 0..=NEWTON_HALVINGS {
            let ut: Vec<f64> = (0..m).map(|k| boxed(k, u[k] + lambda * delta[k])).collect();
            let (pt, ot, rt) = eval_holds(model, h, (x, arm), targets, &ut, &mut it);
            let Some(rt) = rt else { return (pt, ot, it) };
            if scaled(&rt) < (1.0 - 1e-4 * lambda) * m0 {
                (u, p, o, r) = (ut, pt, ot, rt);
                accepted = true;
                break;
            }
            lambda *= 0.5;
        }
        if !accepted {
            return (p, o, it);
        }
    }
    (p, o, it)
}

/// Solve `a x = b` by Gaussian elimination with partial pivoting; `None` when singular or not finite.
fn gauss_solve(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> Option<Vec<f64>> {
    let n = b.len();
    for c in 0..n {
        let piv = (c..n).max_by(|&i, &j| a[i][c].abs().total_cmp(&a[j][c].abs()))?;
        if a[piv][c] == 0.0 || !a[piv][c].is_finite() {
            return None;
        }
        a.swap(c, piv);
        b.swap(c, piv);
        for i in c + 1..n {
            let f = a[i][c] / a[c][c];
            for k in c..n {
                a[i][k] -= f * a[c][k];
            }
            b[i] -= f * b[c];
        }
    }
    let mut x = vec![0.0; n];
    for i in (0..n).rev() {
        let s: f64 = (i + 1..n).map(|k| a[i][k] * x[k]).sum();
        x[i] = (b[i] - s) / a[i][i];
    }
    x.iter().all(|v| v.is_finite()).then_some(x)
}

// ---------------------------------------------------------------- the held-tolerance floor (ADR 0665 D2)

/// The floor of one solved arm at the probe point.
#[derive(Debug, Clone, PartialEq)]
pub struct ArmFloor {
    /// Arm.
    pub arm: Arm,
    /// Solved compensating parameters (held order).
    pub by: Vec<f64>,
    /// `S_j = dy / dg_j` along the hold (response unit per held-metric unit).
    pub sensitivity: Vec<f64>,
    /// `phi = sum_j |S_j| tolerance_j` (response unit).
    pub bound: f64,
}

/// The held-tolerance floor of a record and model (ADR 0665 D2).
#[derive(Debug, Clone, PartialEq)]
pub struct HeldFloor {
    /// `phi_low + phi_high` (value targets) or `phi_high` (as_low_arm targets), response unit.
    pub value: f64,
    /// The probe point: the domain midpoint, in registered order.
    pub probe: Vec<f64>,
    /// The solved arms.
    pub arms: Vec<ArmFloor>,
    /// The floor sample (ADR 0670); `None` for the midpoint rule of ADR 0665.
    pub sample: Option<SampledFloor>,
}

/// The floor over a sample of the design (ADR 0670 D2).
#[derive(Debug, Clone, PartialEq)]
pub struct SampledFloor {
    /// Number of sampled pairs.
    pub points: usize,
    /// Seed of the draw ([`floor_seed`]).
    pub seed: u64,
    /// Sampled pairs, ascending.
    pub pairs: Vec<usize>,
    /// The floor at the domain midpoint.
    pub midpoint: f64,
    /// The floor at the design point of each sampled pair (`pairs` order).
    pub values: Vec<f64>,
    /// The pair of the largest point (`None`: the midpoint).
    pub largest: Option<usize>,
}

impl HeldFloor {
    fn to_json(&self) -> Value {
        let nums = |v: &[f64]| v.iter().map(|x| num(*x)).collect::<Vec<_>>();
        let mut v = json!({
            "value": num(self.value), "k": num(K_RES), "probe": nums(&self.probe),
            "arms": self.arms.iter().map(|a| json!({
                "arm": a.arm.as_str(), "by": nums(&a.by), "sensitivity": nums(&a.sensitivity), "bound": num(a.bound),
            })).collect::<Vec<_>>(),
        });
        // written only with a sample, so a midpoint floor keeps its bytes (ADR 0670 D3)
        if let (Some(f), Some(o)) = (&self.sample, v.as_object_mut()) {
            o.insert(
                "sample".into(),
                json!({
                    "points": f.points, "seed": f.seed, "pairs": f.pairs, "midpoint": num(f.midpoint),
                    "values": nums(&f.values), "largest": f.largest,
                }),
            );
        }
        v
    }
}

fn held_values(o: &Outcome, h: &Hypothesis) -> Option<Vec<f64>> {
    if o.verdict != Verdict::Ok {
        return None;
    }
    h.held.iter().map(|hd| metric_of(o, &hd.metric, &hd.unit)).collect()
}

/// The response and the held metrics at compensating values `u` (`None` when the evaluation fails or a metric is missing).
fn probe_eval(model: &dyn Model, h: &Hypothesis, x: &[f64], arm: Arm, u: &[f64]) -> Option<(f64, Vec<f64>)> {
    let o = safe_eval(model, &params_for(h, x, arm, u));
    let y = (o.verdict == Verdict::Ok)
        .then(|| metric_of(&o, &h.response.name, &h.response.unit))
        .flatten()?;
    Some((y, held_values(&o, h)?))
}

fn arm_floor(
    model: &dyn Model,
    h: &Hypothesis,
    (x, arm, place): (&[f64], Arm, &str),
    (p, o): (&[Quantity], &Outcome),
    targets: &[f64],
) -> Result<ArmFloor, String> {
    let where_ = format!("held: the floor probe at {place}, arm {}", arm.as_str());
    let g = held_values(o, h).ok_or_else(|| format!("{where_}: the evaluation failed or a held metric is missing"))?;
    if metric_of(o, &h.response.name, &h.response.unit).is_none() {
        return Err(format!("{where_}: the response '{}' is missing", h.response.name));
    }
    for ((hd, g), t) in h.held.iter().zip(&g).zip(targets) {
        let within = (g - t).abs() <= hd.tolerance; // false for NaN
        if !within {
            return Err(format!(
                "{where_}: held metric '{}' = {g} not within {} of the target {t}",
                hd.metric, hd.tolerance
            ));
        }
    }
    let m = h.held.len();
    let n0 = 1 + h.domain.len();
    let u: Vec<f64> = p[n0..n0 + m].iter().map(|q| q.value).collect();
    let mut jt = vec![vec![0.0; m]; m]; // jt[k][j] = d g_j / d u_k (the transpose of J)
    let mut gy = vec![0.0; m];
    for k in 0..m {
        let (lo, hi) = h.held[k].bracket;
        let scale = u[k].abs().max(hi - lo);
        let hc = f64::EPSILON.cbrt() * scale;
        let hs = f64::EPSILON.sqrt() * scale;
        let (a, b) = if u[k] - hc >= lo && u[k] + hc <= hi {
            (u[k] - hc, u[k] + hc)
        } else if u[k] + hs <= hi {
            (u[k], u[k] + hs)
        } else {
            (u[k] - hs, u[k])
        };
        let at = |v: f64| {
            let mut w = u.clone();
            w[k] = v;
            probe_eval(model, h, x, arm, &w)
                .ok_or_else(|| format!("{where_}: the evaluation at a difference point failed"))
        };
        let (ya, ga) = at(a)?;
        let (yb, gb) = at(b)?;
        let den = b - a;
        gy[k] = (yb - ya) / den;
        for j in 0..m {
            jt[k][j] = (gb[j] - ga[j]) / den;
        }
    }
    let s = gauss_solve(jt, gy).ok_or_else(|| {
        format!("{where_}: the compensating parameters do not control the held metrics (singular Jacobian)")
    })?;
    let bound = s.iter().zip(&h.held).map(|(s, hd)| s.abs() * hd.tolerance).sum();
    Ok(ArmFloor {
        arm,
        by: u,
        sensitivity: s,
        bound,
    })
}

/// The held-tolerance floor of `h` with `model` (ADR 0665 D2): at the domain midpoint, each arm solved as its evidence would be,
/// `phi = sum_j |S_j| tolerance_j` with `S = J^{-T} grad_u y` by differences; `phi_low + phi_high` for value targets, `phi_high`
/// for as_low_arm targets (the low arm has no residual). First order: the change of the paired difference that the held
/// tolerances allow. With a registered [`FloorSample`] (ADR 0670 D2) the largest of that floor over the domain midpoint and the
/// design points of the sampled pairs ([`floor_sample_pairs`]); the first point reaching it gives `probe` and `arms`.
pub fn held_floor(h: &Hypothesis, model: &dyn Model) -> Result<HeldFloor, String> {
    let mid: Vec<f64> = h.domain.iter().map(|b| b.lo + 0.5 * (b.hi - b.lo)).collect();
    let Some(fs) = h.floor_sample else {
        return floor_at(h, model, &mid, "the domain midpoint");
    };
    let hash = h.hash();
    let seed = floor_seed(&hash, h.seed);
    let pairs = rerun_pairs(h.n_pairs, fs.points, seed);
    let pts = design_points(h, derived_seed(&hash, h.seed));
    let first = floor_at(h, model, &mid, "the domain midpoint")?;
    let midpoint = first.value;
    let mut best = first;
    let mut largest = None;
    let mut values = Vec::with_capacity(pairs.len());
    for &i in &pairs {
        let f = floor_at(h, model, &pts[i], &format!("design pair {i} of the floor sample"))?;
        values.push(f.value);
        if f.value > best.value {
            best = f;
            largest = Some(i);
        }
    }
    best.sample = Some(SampledFloor {
        points: fs.points,
        seed,
        pairs,
        midpoint,
        values,
        largest,
    });
    Ok(best)
}

/// The held-tolerance floor of ADR 0665 D2 at one point `x` of the domain (domain order); `sample` is `None` (ADR 0670 D3).
pub fn held_floor_at(h: &Hypothesis, model: &dyn Model, x: &[f64]) -> Result<HeldFloor, String> {
    if x.len() != h.domain.len() {
        return Err(format!(
            "held: the floor point has {} coordinates, the domain {}",
            x.len(),
            h.domain.len()
        ));
    }
    floor_at(h, model, x, "the given point")
}

fn floor_at(h: &Hypothesis, model: &dyn Model, x: &[f64], place: &str) -> Result<HeldFloor, String> {
    if h.held.is_empty() {
        return Err("held: no held metric, so there is no held-tolerance floor".into());
    }
    if !same_target_kind(&h.held) {
        return Err("held: targets must be all values or all as_low_arm (a mix is not covered, ADR 0665)".into());
    }
    let x = x.to_vec();
    let as_low = matches!(h.held[0].target, HeldTarget::AsLowArm { .. });
    let values: Vec<f64> = h
        .held
        .iter()
        .map(|hd| match hd.target {
            HeldTarget::Value(t) => t,
            HeldTarget::AsLowArm { .. } => f64::NAN,
        })
        .collect();
    let (pl, ol, _) = eval_arm(model, h, &x, Arm::Low, None);
    let low = held_values(&ol, h).ok_or_else(|| {
        format!("held: the floor probe at {place}, arm low: the evaluation failed or a held metric is missing")
    })?;
    let mut arms = Vec::new();
    if !as_low {
        arms.push(arm_floor(model, h, (&x, Arm::Low, place), (&pl, &ol), &values)?);
    }
    let (ph, oh, _) = eval_arm(model, h, &x, Arm::High, Some(&low));
    let high_targets = if as_low { low.clone() } else { values };
    arms.push(arm_floor(model, h, (&x, Arm::High, place), (&ph, &oh), &high_targets)?);
    Ok(HeldFloor {
        value: arms.iter().map(|a| a.bound).sum(),
        probe: x,
        arms,
        sample: None,
    })
}

/// Register one hypothesis with one or two held metrics and its held-tolerance floor (ADR 0665): ADR 0624's validation with up
/// to [`MAX_HELD`] holds, the model's identity equal to the record's, the floor of [`held_floor`], and `min_effect` at least
/// [`K_RES`] times the floor. The registration entry carries `settings.held_floor`; the record and its hash are not changed.
pub fn register_with_floor(ledger: &Ledger, h: &Hypothesis, model: &dyn Model) -> Result<Registered, String> {
    let entries = ledger.read()?.entries;
    let regs = registrations(&entries);
    let slot = recount(&regs).last().map_or(0, |x| x.1) + 1;
    let n = regs.len() + 1;
    let fail = |e: String| format!("hypothesis {n}: {e}");
    validate(h, &entries, &regs, MAX_HELD).map_err(fail)?;
    if h.held.is_empty() {
        return Err(fail(
            "held: no held metric; register_with_floor computes the floor of one or two (use register)".into(),
        ));
    }
    if model.id() != h.evaluator.id || model.version() != h.evaluator.version || model.fidelity() != h.evaluator.tier {
        return Err(fail(format!(
            "evaluator: the model is {} version '{}' tier {}, the registration names {} version '{}' tier {}",
            model.id(),
            model.version(),
            model.fidelity().as_str(),
            h.evaluator.id,
            h.evaluator.version,
            h.evaluator.tier.as_str()
        )));
    }
    let floor = held_floor(h, model).map_err(fail)?;
    if let (Some(f), true) = (&floor.sample, h.min_effect < K_RES * floor.value) {
        let at = f.largest.map_or("the midpoint".to_string(), |i| format!("pair {i}"));
        return Err(fail(format!(
            "min_effect: {} is below {K_RES} times the held-tolerance floor {} of '{}' over the domain midpoint and {} sampled \
             design pairs (largest at {at}; the midpoint floor is {}; ADR 0670)",
            h.min_effect, floor.value, h.response.name, f.points, f.midpoint
        )));
    }
    if h.min_effect < K_RES * floor.value {
        return Err(fail(format!(
            "min_effect: {} is below {K_RES} times the held-tolerance floor {} of '{}' (the change of the paired difference \
             that the held tolerances allow, first order, at the domain midpoint; ADR 0665)",
            h.min_effect, floor.value, h.response.name
        )));
    }
    let id = format!("H-{n}");
    let mut e = registration_entry(h, &id, slot, None);
    if let Some(o) = e.settings.as_object_mut() {
        o.insert("held_floor".into(), floor.to_json());
    }
    ledger.append(&[e])?;
    let entries = ledger.read()?.entries;
    let k = entries.len() - 1;
    Ok(parse_registration(k, &entries[k]))
}

fn evidence_entry(
    model: &dyn Model,
    reg: &Registered,
    reg_ref: &EntryRef,
    pair: usize,
    arm: Arm,
    (p, o, it): (Vec<Quantity>, Outcome, usize),
) -> Entry {
    let mut e = super::ledger::entry_of(model, p, o);
    e.kind = "evaluation".into();
    e.study = "hypothesis".into();
    e.variant = format!("{}#p{pair}.{}", reg.id, arm.as_str());
    e.seed = Some(reg.derived_seed);
    e.parents = vec![reg_ref.clone()];
    e.provenance = json!({"driver": "hypothesis", "hypothesis": reg.id, "hash": reg.hash, "pair": pair, "arm": arm.as_str(), "iterations": it});
    e
}

// ---------------------------------------------------------------- verification

/// Result of a hypothesis test, recomputed from the evidence.
#[derive(Debug, Clone, PartialEq)]
pub struct TestResult {
    /// Hypothesis id.
    pub id: String,
    /// Finding.
    pub finding: Finding,
    /// Why it is void.
    pub void_reason: Option<String>,
    /// Statistics at the effective level (counts only when void).
    pub stats: Stats,
    /// Paired differences in the claimed direction, per pair (`None` for a failed pair).
    pub d: Vec<Option<f64>>,
    /// Effective level (the slot level; within a batch the Holm step level).
    pub level: f64,
    /// Family slot.
    pub slot: usize,
    /// Pairs whose low arm failed.
    pub n_failed_low: usize,
    /// Pairs whose high arm failed.
    pub n_failed_high: usize,
    /// Smallest and largest `d` with their pair.
    pub d_extremes: Option<((f64, usize), (f64, usize))>,
    /// Whether `s [L, U]` overlaps the predicted size.
    pub predicted_overlap: Option<bool>,
}

/// What [`verify`] found.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Verification {
    /// Hypothesis id.
    pub id: String,
    /// Registration entry index.
    pub registration: Option<usize>,
    /// Broken rules (the hypothesis is refused when not empty); each names the entry.
    pub problems: Vec<String>,
    /// Evidence entry per (pair, arm): index `2 pair + arm`.
    pub evidence: Vec<Option<usize>>,
    /// Every (pair, arm) has evidence.
    pub complete: bool,
    /// Withdrawn.
    pub withdrawn: bool,
    /// The recomputed result (when the evidence of the slot is complete and nothing is broken).
    pub result: Option<TestResult>,
    /// Index of the genuine result entry (equal to the recomputation).
    pub genuine: Option<usize>,
    /// Result entries of this id that are not genuine, with the reason.
    pub flagged: Vec<(usize, String)>,
    /// `Some(true)` when every evidence entry was re-run and reproduced (with a registered sample: every entry of the sampled
    /// pairs); `None` when not re-run.
    pub evidence_rerun: Option<bool>,
    /// The registered re-run sample (ADR 0662), reported whether or not a model was given.
    pub rerun_sample: Option<RerunPlan>,
}

struct Member {
    reg: Registered,
    rec: Option<Hypothesis>,
    v: Verification,
}

fn result_entries_of<'a>(entries: &'a [Entry], id: &'a str) -> impl Iterator<Item = (usize, &'a Entry)> + 'a {
    entries.iter().enumerate().filter(move |(_, e)| {
        e.kind == KIND_RESULT && (e.variant == id || e.provenance.get("hypothesis").and_then(Value::as_str) == Some(id))
    })
}

fn check_member(entries: &[Entry], regs: &[Registered], counts: &[(String, usize)], j: usize) -> Member {
    let reg = regs[j].clone();
    let id = reg.id.clone();
    let mut v = Verification {
        id: id.clone(),
        registration: Some(reg.index),
        ..Default::default()
    };
    let at = |k: usize| format!("entry {k}");
    if regs.iter().filter(|r| r.id == id).count() > 1 {
        v.problems
            .push(format!("{}: {id} is registered more than once", at(reg.index)));
    }
    let (eid, eslot) = &counts[j];
    if *eid != id {
        v.problems.push(format!(
            "{}: registration id {id}, the file order gives {eid}",
            at(reg.index)
        ));
    }
    if reg.slot != *eslot {
        v.problems.push(format!(
            "{}: registration of {id} stores family slot {}, the file order gives {eslot}",
            at(reg.index),
            reg.slot
        ));
    }
    if reg.level.to_bits() != series_level(*eslot).to_bits() {
        v.problems.push(format!(
            "{}: registration of {id} stores level {}, slot {eslot} has {}",
            at(reg.index),
            reg.level,
            series_level(*eslot)
        ));
    }
    if reg.hash != reg.recomputed_hash {
        v.problems.push(format!(
            "{}: the registration of {id} was altered: stored hash {}, hash of the stored record {}",
            at(reg.index),
            reg.hash,
            reg.recomputed_hash
        ));
    }
    let rec = match &reg.record {
        Ok(h) => Some(h.clone()),
        Err(e) => {
            v.problems.push(format!("{}: record of {id}: {e}", at(reg.index)));
            None
        }
    };
    if let Some(h) = &rec {
        if reg.derived_seed != derived_seed(&reg.hash, h.seed) {
            v.problems.push(format!(
                "{}: derived seed of {id} does not match its record",
                at(reg.index)
            ));
        }
    }
    if let Some(h) = &rec {
        // the held-tolerance floor (ADR 0665): required with two holds, checked against min_effect when stored
        match entries[reg.index].settings.get("held_floor") {
            None | Some(Value::Null) => {
                if h.held.len() > 1 {
                    v.problems.push(format!(
                        "{}: registration of {id} holds {} metrics and carries no held-tolerance floor (register_with_floor, ADR 0665)",
                        at(reg.index),
                        h.held.len()
                    ));
                }
            }
            Some(f) => match f.get("value").and_then(Value::as_f64) {
                Some(x) if x.is_finite() && x >= 0.0 => {
                    if h.min_effect < K_RES * x {
                        v.problems.push(format!(
                            "{}: min_effect {} of {id} is below {K_RES} times its stored held-tolerance floor {x}",
                            at(reg.index),
                            h.min_effect
                        ));
                    }
                }
                _ => v.problems.push(format!(
                    "{}: the held-tolerance floor of {id} is not a finite non-negative number",
                    at(reg.index)
                )),
            },
        }
        // the floor sample (ADR 0670 D3): required by a record that registers one, its pairs and its largest checked
        let stored = entries[reg.index].settings.get("held_floor").filter(|f| !f.is_null());
        let sample = stored.and_then(|f| f.get("sample")).filter(|x| !x.is_null());
        match (h.floor_sample.is_some(), stored, sample) {
            (true, None, _) => v.problems.push(format!(
                "{}: the record of {id} registers a floor sample and its registration carries no held-tolerance floor (ADR 0670)",
                at(reg.index)
            )),
            (true, Some(_), None) => v.problems.push(format!(
                "{}: the record of {id} registers a floor sample and its stored held-tolerance floor carries none (ADR 0670)",
                at(reg.index)
            )),
            (false, _, Some(_)) => v.problems.push(format!(
                "{}: the stored held-tolerance floor of {id} carries a floor sample its record does not register (ADR 0670)",
                at(reg.index)
            )),
            (true, Some(f), Some(x)) => {
                let pairs: Option<Vec<usize>> = x
                    .get("pairs")
                    .and_then(Value::as_array)
                    .and_then(|a| a.iter().map(|q| q.as_u64().map(|q| q as usize)).collect());
                if pairs != floor_sample_pairs(h) {
                    v.problems.push(format!(
                        "{}: the sampled pairs of the held-tolerance floor of {id} do not match its record (ADR 0670)",
                        at(reg.index)
                    ));
                }
                let mut all: Vec<Option<f64>> = vec![x.get("midpoint").and_then(Value::as_f64)];
                if let Some(a) = x.get("values").and_then(Value::as_array) {
                    all.extend(a.iter().map(Value::as_f64));
                }
                let largest = all
                    .iter()
                    .copied()
                    .collect::<Option<Vec<f64>>>()
                    .and_then(|a| a.into_iter().reduce(f64::max));
                let value = f.get("value").and_then(Value::as_f64);
                let same = matches!((value, largest), (Some(a), Some(b)) if a.to_bits() == b.to_bits());
                if !same {
                    v.problems.push(format!(
                        "{}: the stored held-tolerance floor of {id} ({value:?}) is not the largest of its sample ({largest:?}; ADR 0670)",
                        at(reg.index)
                    ));
                }
            }
            (false, _, None) => {}
        }
    }
    v.withdrawn = entries
        .iter()
        .enumerate()
        .any(|(k, e)| k > reg.index && e.kind == KIND_WITHDRAWN && e.evaluator == DRIVER_ID && e.variant == id);
    let Some(h) = rec.clone() else {
        return Member { reg, rec, v };
    };
    let n = h.n_pairs;
    v.evidence = vec![None; 2 * n];
    let pts = design_points(&h, reg.derived_seed);
    let reg_ref = entry_ref(reg.index, &entries[reg.index]);
    let withdrawn_at = entries
        .iter()
        .enumerate()
        .find(|(k, e)| *k > reg.index && e.kind == KIND_WITHDRAWN && e.evaluator == DRIVER_ID && e.variant == id)
        .map(|(k, _)| k);
    for (k, e) in entries.iter().enumerate().filter(|(_, e)| is_evidence_of(e, &id)) {
        if k <= reg.index {
            v.problems.push(format!(
                "{}: evidence of {id} precedes its registration (entry {})",
                at(k),
                reg.index
            ));
        }
        if let Some(w) = withdrawn_at {
            if k > w {
                v.problems
                    .push(format!("{}: evidence of {id} after its withdrawal (entry {w})", at(k)));
            }
        }
        if e.provenance.get("hash").and_then(Value::as_str) != Some(reg.hash.as_str()) {
            v.problems
                .push(format!("{}: evidence of {id} carries another registration hash", at(k)));
        }
        if e.evaluator != h.evaluator.id || e.evaluator_version != h.evaluator.version || e.fidelity != h.evaluator.tier
        {
            v.problems.push(format!(
                "{}: evidence evaluator {} version '{}' tier {}, registered {} version '{}' tier {}",
                at(k),
                e.evaluator,
                e.evaluator_version,
                e.fidelity.as_str(),
                h.evaluator.id,
                h.evaluator.version,
                h.evaluator.tier.as_str()
            ));
        }
        if e.parents.first() != Some(&reg_ref) {
            v.problems.push(format!(
                "{}: evidence of {id} does not reference its registration (entry {}, its hash)",
                at(k),
                reg.index
            ));
        }
        let pair = e.provenance.get("pair").and_then(Value::as_u64).map(|p| p as usize);
        let arm = e.provenance.get("arm").and_then(Value::as_str).and_then(Arm::parse);
        let (Some(pair), Some(arm)) = (pair, arm) else {
            v.problems
                .push(format!("{}: evidence of {id} without a pair and arm", at(k)));
            continue;
        };
        if pair >= n {
            v.problems.push(format!(
                "{}: evidence of {id} for pair {pair}, outside the registered {n} pairs",
                at(k)
            ));
            continue;
        }
        let s = 2 * pair + arm.slot();
        if let Some(prev) = v.evidence[s] {
            v.problems.push(format!(
                "{}: second evidence of {id} for pair {pair} arm {} (first at entry {prev})",
                at(k),
                arm.as_str()
            ));
            continue;
        }
        v.evidence[s] = Some(k);
        // the parameters must be the design's
        let want = params_for(&h, &pts[pair], arm, &[]);
        let extra = h.held.len();
        if e.params.len() != want.len() + extra {
            v.problems.push(format!(
                "{}: evidence of {id} pair {pair} has {} parameters, the design {}",
                at(k),
                e.params.len(),
                want.len() + extra
            ));
            continue;
        }
        for (got, w) in e.params.iter().zip(&want) {
            if got.name != w.name || got.unit != w.unit || got.value.to_bits() != w.value.to_bits() {
                v.problems.push(format!(
                    "{}: evidence of {id} pair {pair} arm {}: parameter '{}' = {} {}, the design has '{}' = {} {}",
                    at(k),
                    arm.as_str(),
                    got.name,
                    got.value,
                    got.unit,
                    w.name,
                    w.value,
                    w.unit
                ));
            }
        }
        for (i, hd) in h.held.iter().enumerate() {
            let b = &e.params[want.len() + i];
            let in_bracket = b.value >= hd.bracket.0 && b.value <= hd.bracket.1;
            let at_ok = match (hd.target, arm) {
                (HeldTarget::AsLowArm { at }, Arm::Low) => b.value.to_bits() == at.to_bits(),
                _ => true,
            };
            if b.name != hd.by || b.unit != hd.by_unit || !in_bracket || !at_ok {
                v.problems.push(format!(
                    "{}: evidence of {id} pair {pair} arm {}: compensating parameter '{}' = {} outside the registered rule",
                    at(k),
                    arm.as_str(),
                    b.name,
                    b.value
                ));
            }
        }
    }
    v.complete = v.evidence.iter().all(Option::is_some);
    Member { reg, rec, v }
}

/// Compute the result of one member from its evidence at `level` (no Holm).
fn member_result(entries: &[Entry], m: &Member, level: f64) -> TestResult {
    let h = m.rec.as_ref().expect("checked");
    let n = h.n_pairs;
    let s = h.direction.sign();
    let mut d = vec![None; n];
    let mut failed_low = 0;
    let mut failed_high = 0;
    let mut failed = 0;
    let mut unmatched: Option<String> = None;
    for (p, dp) in d.iter_mut().enumerate() {
        let lo = &entries[m.v.evidence[2 * p].expect("complete")];
        let hi = &entries[m.v.evidence[2 * p + 1].expect("complete")];
        let ylo = (lo.verdict == Verdict::Ok)
            .then(|| {
                lo.metric(&h.response.name)
                    .filter(|q| q.unit == h.response.unit)
                    .map(|q| q.value)
            })
            .flatten();
        let yhi = (hi.verdict == Verdict::Ok)
            .then(|| {
                hi.metric(&h.response.name)
                    .filter(|q| q.unit == h.response.unit)
                    .map(|q| q.value)
            })
            .flatten();
        failed_low += usize::from(ylo.is_none());
        failed_high += usize::from(yhi.is_none());
        let (Some(a), Some(b)) = (ylo, yhi) else {
            failed += 1;
            continue;
        };
        for hd in &h.held {
            let ml = lo.metric(&hd.metric).filter(|q| q.unit == hd.unit).map(|q| q.value);
            let mh = hi.metric(&hd.metric).filter(|q| q.unit == hd.unit).map(|q| q.value);
            let check = |arm: &str, got: Option<f64>, target: Option<f64>| -> Option<String> {
                match (got, target) {
                    (Some(g), Some(t)) if (g - t).abs() <= hd.tolerance => None,
                    (g, t) => Some(format!(
                        "pair {p} arm {arm}: held metric '{}' = {} not within {} of the target {}",
                        hd.metric,
                        g.map_or("missing".to_string(), |x| x.to_string()),
                        hd.tolerance,
                        t.map_or("missing".to_string(), |x| x.to_string())
                    )),
                }
            };
            let bad = match hd.target {
                HeldTarget::Value(t) => check("low", ml, Some(t)).or_else(|| check("high", mh, Some(t))),
                HeldTarget::AsLowArm { .. } => check("high", mh, ml),
            };
            if let Some(r) = bad {
                unmatched.get_or_insert(r);
                break;
            }
        }
        *dp = Some(s * (b - a));
    }
    let ok: Vec<f64> = d.iter().flatten().copied().collect();
    let void_reason = if let Some(u) = unmatched {
        Some(u)
    } else if failed as f64 > h.max_failed_share * n as f64 {
        Some(format!(
            "{failed} of {n} pairs failed ({failed_low} low arms, {failed_high} high arms), above the registered share {}",
            h.max_failed_share
        ))
    } else if ok.len() < 2 {
        Some(format!("{} ok pairs; at least 2 are needed", ok.len()))
    } else {
        None
    };
    let (finding, stats) = if void_reason.is_some() {
        (
            Finding::Void,
            Stats {
                n: ok.len(),
                level,
                ..Default::default()
            },
        )
    } else {
        paired_test(&ok, level, h.min_effect, h.direction, h.statistic)
    };
    let d_extremes = (finding != Finding::Void).then(|| {
        let mut mn = (f64::INFINITY, 0);
        let mut mx = (f64::NEG_INFINITY, 0);
        for (p, x) in d.iter().enumerate() {
            if let Some(x) = *x {
                if x < mn.0 {
                    mn = (x, p);
                }
                if x > mx.0 {
                    mx = (x, p);
                }
            }
        }
        (mn, mx)
    });
    let predicted_overlap = (finding != Finding::Void)
        .then(|| {
            h.predicted_size.map(|(a, b)| {
                let (l, u) = (s * stats.lower, s * stats.upper);
                let (l, u) = (l.min(u), l.max(u));
                l <= b && a <= u
            })
        })
        .flatten();
    TestResult {
        id: m.reg.id.clone(),
        finding,
        void_reason,
        stats,
        d,
        level,
        slot: m.reg.slot,
        n_failed_low: failed_low,
        n_failed_high: failed_high,
        d_extremes,
        predicted_overlap,
    }
}

/// Results of every member of a slot: single members at the slot level; a batch by Holm's step-down procedure.
fn slot_results(entries: &[Entry], members: &[Member]) -> Vec<TestResult> {
    let a = series_level(members[0].reg.slot);
    let k = members.len();
    if k == 1 {
        return vec![member_result(entries, &members[0], a)];
    }
    let base: Vec<Option<TestResult>> = members
        .iter()
        .map(|m| (!m.v.withdrawn).then(|| member_result(entries, m, a / k as f64)))
        .collect();
    let mut order: Vec<usize> = (0..k)
        .filter(|&j| base[j].as_ref().is_some_and(|r| r.finding != Finding::Void))
        .collect();
    order.sort_by(|&x, &y| {
        let px = base[x].as_ref().map_or(1.0, |r| r.stats.p_value);
        let py = base[y].as_ref().map_or(1.0, |r| r.stats.p_value);
        px.total_cmp(&py).then(x.cmp(&y))
    });
    let mut out: Vec<Option<TestResult>> = base;
    let mut confirmed = vec![false; k];
    for (r, &j) in order.iter().enumerate() {
        let step = a / (k - r) as f64;
        let res = member_result(entries, &members[j], step);
        if res.finding == Finding::Confirmed {
            confirmed[j] = true;
            out[j] = Some(res);
        } else {
            break;
        }
    }
    // members the step-down did not confirm keep their a / k judgement (refuted or inconclusive), never a confirmation
    for (j, o) in out.iter_mut().enumerate() {
        if let Some(r) = o {
            if !confirmed[j] && r.finding == Finding::Confirmed {
                r.finding = Finding::Inconclusive;
            }
        }
    }
    out.into_iter()
        .zip(members)
        .map(|(o, m)| {
            o.unwrap_or_else(|| TestResult {
                id: m.reg.id.clone(),
                finding: Finding::Void,
                void_reason: Some("withdrawn".into()),
                stats: Stats::default(),
                d: Vec::new(),
                level: a / k as f64,
                slot: m.reg.slot,
                n_failed_low: 0,
                n_failed_high: 0,
                d_extremes: None,
                predicted_overlap: None,
            })
        })
        .collect()
}

fn result_entry(entries: &[Entry], m: &Member, r: &TestResult) -> Entry {
    let h = m.rec.as_ref().expect("checked");
    let u = &h.response.unit;
    let mut metrics = vec![
        Quantity::new("level", r.level, "1"),
        Quantity::new("n_ok", r.stats.n as f64, "1"),
        Quantity::new("n_failed_low", r.n_failed_low as f64, "1"),
        Quantity::new("n_failed_high", r.n_failed_high as f64, "1"),
    ];
    if r.finding != Finding::Void {
        let st = &r.stats;
        for (k, v, unit) in [
            ("mean", st.mean, u.as_str()),
            ("sd", st.sd, u),
            ("se", st.se, u),
            ("t", st.t, "1"),
            ("lower", st.lower, u),
            ("upper", st.upper, u),
            ("p_value", st.p_value, "1"),
            ("sign_share", st.sign_share, "1"),
            ("sign_ci_lo", st.sign_ci.0, "1"),
            ("sign_ci_hi", st.sign_ci.1, "1"),
        ] {
            metrics.push(Quantity::new(k, v, unit));
        }
        if let Some(((a, pa), (b, pb))) = r.d_extremes {
            metrics.push(Quantity::new("d_min", a, u));
            metrics.push(Quantity::new("d_max", b, u));
            metrics.push(Quantity::new("d_min_pair", pa as f64, "1"));
            metrics.push(Quantity::new("d_max_pair", pb as f64, "1"));
        }
    }
    let mut parents = vec![entry_ref(m.reg.index, &entries[m.reg.index])];
    parents.extend(m.v.evidence.iter().map(|k| {
        let k = k.expect("complete");
        entry_ref(k, &entries[k])
    }));
    let mut e = Entry {
        kind: KIND_RESULT.into(),
        study: "hypothesis".into(),
        variant: m.reg.id.clone(),
        evaluator: DRIVER_ID.into(),
        evaluator_version: DRIVER_VERSION.into(),
        fidelity: h.evaluator.tier,
        verdict: Verdict::Ok,
        metrics,
        settings: json!({
            "verdict": r.finding.as_str(), "opposite": r.stats.opposite, "void_reason": r.void_reason,
            "all_zero": r.stats.all_zero, "predicted_overlap": r.predicted_overlap,
            "scope": {"id": h.evaluator.id, "version": h.evaluator.version, "tier": h.evaluator.tier.as_str()},
            "family": {"slot": m.reg.slot, "slot_level": num(m.reg.level), "batch": m.reg.batch.as_ref().map(|b| json!({"first": b.first, "size": b.size, "member": b.member}))},
            "record_hash": m.reg.hash, "statistic": h.statistic.as_str(),
            "spread": if h.evaluator.deterministic { "domain" } else { "noise and domain" },
        }),
        parents,
        provenance: json!({"driver": "hypothesis", "hypothesis": m.reg.id, "hash": m.reg.hash}),
        ..Default::default()
    };
    // k recorded in the result (ADR 0662 D5); absent without a sample
    if let (Some(p), Some(o)) = (rerun_plan(h, &m.reg.hash), e.settings.as_object_mut()) {
        o.insert(
            "rerun_sample".into(),
            json!({"k": p.k, "seed": p.seed, "pairs": p.pairs, "rtol": num(p.rtol), "atol": num(p.atol)}),
        );
    }
    e
}

fn comparable(e: &Entry) -> Value {
    let mut v = e.to_json();
    if let Some(o) = v.as_object_mut() {
        o.remove("recorded_at_ms");
        o.remove("solver_version");
    }
    v
}

/// Verify every member of the slot of `regs[j]`; returns the members and the expected result entries (when computable).
fn verify_slot(
    entries: &[Entry],
    regs: &[Registered],
    j: usize,
    rerun: Option<&dyn Model>,
) -> (Vec<Member>, Vec<Option<Entry>>) {
    let counts = recount(regs);
    let target = &regs[j];
    let idx: Vec<usize> = match &target.batch {
        Some(b) => (0..regs.len())
            .filter(|&q| regs[q].batch.as_ref().is_some_and(|c| c.first == b.first))
            .collect(),
        None => vec![j],
    };
    let mut members: Vec<Member> = idx.iter().map(|&q| check_member(entries, regs, &counts, q)).collect();
    if let Some(b) = &target.batch {
        if members.len() != b.size {
            let msg = format!(
                "batch {}: {} members found, {} registered",
                b.first,
                members.len(),
                b.size
            );
            members.iter_mut().for_each(|m| m.v.problems.push(msg.clone()));
        }
    }
    // re-run (Q14 (b)); with a registered sample, exactly the sampled pairs (ADR 0662 D4)
    for m in &mut members {
        let Some(h) = &m.rec else {
            continue;
        };
        let plan = rerun_plan(h, &m.reg.hash);
        m.v.rerun_sample = plan.clone();
        let Some(model) = rerun else {
            continue;
        };
        let tier_ok = match &plan {
            Some(_) => matches!(
                h.evaluator.tier,
                Fidelity::Analytic | Fidelity::ReducedOrder | Fidelity::Cfd
            ),
            None => matches!(h.evaluator.tier, Fidelity::Analytic | Fidelity::ReducedOrder),
        };
        if model.id() != h.evaluator.id || model.version() != h.evaluator.version || !tier_ok {
            continue;
        }
        let stored_floor = entries[m.reg.index]
            .settings
            .get("held_floor")
            .and_then(|f| f.get("value"))
            .and_then(Value::as_f64);
        if let Some(stored) = stored_floor {
            match held_floor(h, model) {
                Ok(f) if f.value.to_bits() == stored.to_bits() => {}
                Ok(f) => m.v.problems.push(format!(
                    "entry {}: held-tolerance floor of {} stored {stored}, re-computed {} with {} {}: not reproduced",
                    m.reg.index,
                    m.reg.id,
                    f.value,
                    model.id(),
                    model.version()
                )),
                Err(e) => m.v.problems.push(format!(
                    "entry {}: held-tolerance floor of {} could not be re-computed: {e}",
                    m.reg.index, m.reg.id
                )),
            }
        }
        let (targets, rtol, atol): (Vec<usize>, f64, f64) = match &plan {
            Some(p) => (
                p.pairs
                    .iter()
                    .flat_map(|&q| [2 * q, 2 * q + 1])
                    .filter_map(|s| m.v.evidence.get(s).copied().flatten())
                    .collect(),
                p.rtol,
                p.atol,
            ),
            None => (m.v.evidence.iter().flatten().copied().collect(), 0.0, 0.0),
        };
        let mut all = true;
        for k in &targets {
            let r = rerun_check(&entries[*k], model, rtol, atol);
            if !r.pass {
                all = false;
                m.v.problems.push(format!(
                    "entry {k}: evidence of {} not reproduced by a re-run of {} {}: {}",
                    m.reg.id,
                    model.id(),
                    model.version(),
                    r.problems
                        .iter()
                        .cloned()
                        .chain(
                            r.metrics
                                .iter()
                                .filter(|x| !x.4)
                                .map(|x| format!("'{}' stored {} re-run {}", x.0, x.1, x.2))
                        )
                        .collect::<Vec<_>>()
                        .join("; ")
                ));
            }
        }
        m.v.evidence_rerun = Some(all);
    }
    let broken: Vec<String> = members
        .iter()
        .filter(|m| !m.v.problems.is_empty())
        .map(|m| m.reg.id.clone())
        .collect();
    if members.len() > 1 && !broken.is_empty() {
        for m in &mut members {
            if !broken.contains(&m.reg.id) {
                m.v.problems
                    .push(format!("batch member(s) {} do not verify", broken.join(", ")));
            }
        }
    }
    let ready = members
        .iter()
        .all(|m| m.v.problems.is_empty() && (m.v.complete || m.v.withdrawn));
    let single_withdrawn = members.len() == 1 && members[0].v.withdrawn;
    let expected: Vec<Option<Entry>> = if ready && !single_withdrawn {
        let res = slot_results(entries, &members);
        members
            .iter_mut()
            .zip(res)
            .map(|(m, r)| {
                if m.v.withdrawn {
                    None
                } else {
                    let e = result_entry(entries, m, &r);
                    m.v.result = Some(r);
                    Some(e)
                }
            })
            .collect()
    } else {
        members.iter().map(|_| None).collect()
    };
    for (m, exp) in members.iter_mut().zip(&expected) {
        let lines: Vec<(usize, &Entry)> = result_entries_of(entries, &m.reg.id).collect();
        let n_ev = m.v.evidence.iter().filter(|x| x.is_some()).count();
        let n_pairs = m.rec.as_ref().map_or(0, |h| h.n_pairs);
        for (k, e) in lines {
            if !m.v.complete && !m.v.withdrawn {
                m.v.problems.push(format!(
                    "entry {k}: result of {} with evidence for {n_ev} of the registered {} (pair, arm) entries ({n_pairs} pairs)",
                    m.reg.id,
                    2 * n_pairs
                ));
            }
            match exp {
                Some(x) if m.v.genuine.is_none() && comparable(x) == comparable(e) => m.v.genuine = Some(k),
                Some(x) => {
                    let sv = e.settings.get("verdict").and_then(Value::as_str).unwrap_or("?");
                    let xv = x.settings.get("verdict").and_then(Value::as_str).unwrap_or("?");
                    let why = if m.v.genuine.is_some() && comparable(x) == comparable(e) {
                        "a second copy of the genuine result".to_string()
                    } else {
                        format!("differs from the recomputation (stored verdict {sv}, recomputed {xv})")
                    };
                    m.v.flagged.push((k, why));
                }
                None => {
                    m.v.flagged
                        .push((k, "no verified and complete evidence to recompute it from".to_string()))
                }
            }
        }
    }
    (members, expected)
}

/// Verify hypothesis `id`: the registration, the order and design of its evidence, the result lines (recomputed, never trusted)
/// and, with `rerun` (same id and version, tier analytic or reduced_order), the evidence values by re-evaluation.
pub fn verify(entries: &[Entry], id: &str, rerun: Option<&dyn Model>) -> Verification {
    let regs = registrations(entries);
    let Some(j) = regs.iter().position(|r| r.id == id) else {
        return Verification {
            id: id.into(),
            problems: vec![format!("no registration of {id} in the ledger")],
            ..Default::default()
        };
    };
    let (members, _) = verify_slot(entries, &regs, j, rerun);
    members
        .into_iter()
        .find(|m| m.reg.id == id)
        .map(|m| m.v)
        .unwrap_or_default()
}

// ---------------------------------------------------------------- verifier-drawn re-run sample (ADR 0693)

/// Kind of the record of a verifier-drawn re-run check (ADR 0693 D3).
pub const KIND_DRAWN: &str = "hypothesis_drawn_check";

/// Source of the verifier's seed (ADR 0693 D2): drawn after the ledger is read, never stored in it before the check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawSeed {
    /// Supplied by the caller (reproducible runs, or a verifier's own generator).
    Given(u64),
    /// [`entropy_seed`].
    Entropy,
}

/// A seed from outside the file: std's `RandomState` (keys from the operating system's random source) hashing the system time,
/// the process id and a process counter. Unpredictable to an evaluator outside the verifier's process; not a cryptographic
/// generator (ADR 0693 D2).
pub fn entropy_seed() -> u64 {
    use std::hash::{BuildHasher, Hasher};
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    h.write_u128(t);
    h.write_u32(std::process::id());
    h.write_u64(COUNTER.fetch_add(1, Ordering::Relaxed));
    h.finish()
}

/// What a verifier-drawn re-run check found (ADR 0693 D3).
#[derive(Debug, Clone, PartialEq)]
pub struct DrawnCheck {
    /// Hypothesis id.
    pub id: String,
    /// Registration entry index.
    pub registration: usize,
    /// Number of pairs drawn.
    pub k: usize,
    /// The verifier's seed.
    pub seed: u64,
    /// `given` or `entropy` (`given` for [`check_drawn`]).
    pub seed_source: &'static str,
    /// Drawn pairs, ascending ([`rerun_pairs`] with the verifier's seed).
    pub pairs: Vec<usize>,
    /// Relative tolerance.
    pub rtol: f64,
    /// Absolute tolerance.
    pub atol: f64,
    /// Evidence entries re-run (both arms of each drawn pair), in pair and arm order.
    pub rerun: Vec<usize>,
    /// Re-run entries not reproduced.
    pub mismatched: Vec<usize>,
    /// One located problem per mismatched entry.
    pub problems: Vec<String>,
    /// Every re-run entry reproduced.
    pub reproduced: bool,
    /// Number of ledger entries the check read.
    pub entries_seen: usize,
    /// Index of the appended record ([`verify_drawn`]); `None` for [`check_drawn`].
    pub record: Option<usize>,
}

/// Re-run both arms of `k` pairs drawn from the verifier's `seed` with the trusted `model` (ADR 0693 D2, D3); pure. Refused (the
/// message names `drawn`): an unknown id, an unreadable record, `k` outside `1..=n_pairs`, tolerances not finite and
/// non-negative, a model of another id or version, tier measured, a non-deterministic evaluator at zero tolerances, incomplete
/// evidence.
pub fn check_drawn(
    entries: &[Entry],
    id: &str,
    model: &dyn Model,
    sample: &RerunSample,
    seed: u64,
) -> Result<DrawnCheck, String> {
    let regs = registrations(entries);
    let reg = regs
        .iter()
        .find(|r| r.id == id)
        .ok_or_else(|| format!("drawn check: no registration of {id} in the ledger"))?;
    let h = reg
        .record
        .as_ref()
        .map_err(|e| format!("drawn check: record of {id}: {e}"))?;
    let n = h.n_pairs;
    let k = sample.pairs;
    if !(1..=n).contains(&k) {
        return Err(format!(
            "drawn check: {k} pairs; it must lie in 1..={n} (the registered pairs)"
        ));
    }
    let (rtol, atol) = (sample.rtol, sample.atol);
    if !(rtol.is_finite() && rtol >= 0.0 && atol.is_finite() && atol >= 0.0) {
        return Err(format!(
            "drawn check: tolerances rtol {rtol} and atol {atol} must be finite and non-negative"
        ));
    }
    if model.id() != h.evaluator.id || model.version() != h.evaluator.version {
        return Err(format!(
            "drawn check: model {} version '{}', registered {} version '{}'",
            model.id(),
            model.version(),
            h.evaluator.id,
            h.evaluator.version
        ));
    }
    if h.evaluator.tier == Fidelity::Measured {
        return Err("drawn check: tier measured cannot be re-run".into());
    }
    if !h.evaluator.deterministic && rtol == 0.0 && atol == 0.0 {
        return Err(
            "drawn check: the evaluator is not declared deterministic, so a re-run at zero tolerances cannot reproduce it".into(),
        );
    }
    let v = verify(entries, id, None);
    if !v.complete {
        return Err(format!(
            "drawn check: the evidence of {id} is incomplete ({} of {} (pair, arm) entries)",
            v.evidence.iter().filter(|x| x.is_some()).count(),
            2 * n
        ));
    }
    let pairs = rerun_pairs(n, k, seed);
    let rerun: Vec<usize> = pairs
        .iter()
        .flat_map(|&q| [2 * q, 2 * q + 1])
        .filter_map(|s| v.evidence.get(s).copied().flatten())
        .collect();
    let mut mismatched = Vec::new();
    let mut problems = Vec::new();
    for &e in &rerun {
        let r = rerun_check(&entries[e], model, rtol, atol);
        if !r.pass {
            mismatched.push(e);
            problems.push(format!(
                "entry {e}: evidence of {id} not reproduced by a verifier-drawn re-run of {} {}: {}",
                model.id(),
                model.version(),
                r.problems
                    .iter()
                    .cloned()
                    .chain(
                        r.metrics
                            .iter()
                            .filter(|x| !x.4)
                            .map(|x| format!("'{}' stored {} re-run {}", x.0, x.1, x.2))
                    )
                    .collect::<Vec<_>>()
                    .join("; ")
            ));
        }
    }
    Ok(DrawnCheck {
        id: id.into(),
        registration: reg.index,
        k,
        seed,
        seed_source: "given",
        pairs,
        rtol,
        atol,
        reproduced: mismatched.is_empty(),
        rerun,
        mismatched,
        problems,
        entries_seen: entries.len(),
        record: None,
    })
}

fn drawn_entry(entries: &[Entry], c: &DrawnCheck, model: &dyn Model) -> Entry {
    let reg = &entries[c.registration];
    let hash = reg
        .provenance
        .get("hash")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let mut parents = vec![entry_ref(c.registration, reg)];
    parents.extend(c.rerun.iter().map(|&k| entry_ref(k, &entries[k])));
    Entry {
        kind: KIND_DRAWN.into(),
        study: "hypothesis".into(),
        variant: c.id.clone(),
        evaluator: DRIVER_ID.into(),
        evaluator_version: DRIVER_VERSION.into(),
        fidelity: reg.fidelity,
        verdict: Verdict::Ok,
        metrics: vec![
            Quantity::new("k", c.k as f64, "1"),
            Quantity::new("n_rerun", c.rerun.len() as f64, "1"),
            Quantity::new("n_mismatched", c.mismatched.len() as f64, "1"),
        ],
        settings: json!({
            "seed": c.seed, "seed_source": c.seed_source, "k": c.k, "pairs": c.pairs,
            "rtol": num(c.rtol), "atol": num(c.atol),
            "rerun_by": {"id": model.id(), "version": model.version()},
            "reproduced": c.reproduced, "mismatched": c.mismatched, "entries_seen": c.entries_seen,
            "record_hash": hash,
        }),
        parents,
        provenance: json!({"driver": "hypothesis_drawn", "hypothesis": c.id, "hash": hash}),
        ..Default::default()
    }
}

/// Read the ledger, draw the verifier's seed, run [`check_drawn`], then append one `hypothesis_drawn_check` record with the seed
/// (ADR 0693 D3). Nothing is appended when the check is refused.
pub fn verify_drawn(
    ledger: &Ledger,
    id: &str,
    model: &dyn Model,
    sample: &RerunSample,
    seed: DrawSeed,
) -> Result<DrawnCheck, String> {
    let entries = ledger.read()?.entries;
    let (s, source) = match seed {
        DrawSeed::Given(s) => (s, "given"),
        DrawSeed::Entropy => (entropy_seed(), "entropy"),
    };
    let mut d = check_drawn(&entries, id, model, sample, s)?;
    d.seed_source = source;
    let e = drawn_entry(&entries, &d, model);
    ledger.append(&[e])?;
    d.record = Some(entries.len());
    Ok(d)
}

/// Re-check the drawn record at `index` with the trusted `model`: its parents resolve and [`check_drawn`] on the entries it saw,
/// with its seed and sample, gives its pairs, mismatched entries and outcome (ADR 0693 D3). Empty when consistent.
pub fn drawn_record_problems(entries: &[Entry], index: usize, model: &dyn Model) -> Vec<String> {
    let at = format!("entry {index}");
    let Some(e) = entries.get(index) else {
        return vec![format!("{at}: no such entry")];
    };
    if e.kind != KIND_DRAWN {
        return vec![format!("{at}: not a {KIND_DRAWN} record")];
    }
    let mut p = Vec::new();
    for r in &e.parents {
        if let Err(m) = resolve(entries, r) {
            p.push(format!("{at}: parent {} does not resolve: {m}", r.index));
        }
    }
    let s = &e.settings;
    let uz = |key: &str| s.get(key).and_then(Value::as_u64);
    let list = |key: &str| -> Option<Vec<usize>> {
        s.get(key)
            .and_then(Value::as_array)
            .and_then(|a| a.iter().map(|q| q.as_u64().map(|q| q as usize)).collect())
    };
    let (Some(seed), Some(k), Some(seen), Some(rtol), Some(atol)) = (
        uz("seed"),
        uz("k"),
        uz("entries_seen"),
        s.get("rtol").and_then(Value::as_f64),
        s.get("atol").and_then(Value::as_f64),
    ) else {
        p.push(format!(
            "{at}: drawn record without seed, k, entries_seen or tolerances"
        ));
        return p;
    };
    let seen = seen as usize;
    if seen > index {
        p.push(format!(
            "{at}: drawn record claims {seen} entries seen, it is entry {index}"
        ));
        return p;
    }
    let sample = RerunSample {
        pairs: k as usize,
        rtol,
        atol,
    };
    match check_drawn(&entries[..seen], &e.variant, model, &sample, seed) {
        Err(m) => p.push(format!("{at}: the recorded check cannot be redone: {m}")),
        Ok(c) => {
            if list("pairs") != Some(c.pairs.clone()) {
                p.push(format!(
                    "{at}: recorded pairs are not the draw of the recorded seed {seed}"
                ));
            }
            if list("mismatched") != Some(c.mismatched.clone()) {
                p.push(format!(
                    "{at}: recorded mismatched entries differ from the re-run ({:?})",
                    c.mismatched
                ));
            }
            if s.get("reproduced").and_then(Value::as_bool) != Some(c.reproduced) {
                p.push(format!(
                    "{at}: recorded outcome differs from the re-run (reproduced {})",
                    c.reproduced
                ));
            }
        }
    }
    p
}

// ---------------------------------------------------------------- run

/// Options of [`run`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunOptions {
    /// Worker threads.
    pub workers: usize,
    /// Pairs per append (crash granularity; the bytes do not depend on it).
    pub chunk: usize,
}

impl Default for RunOptions {
    fn default() -> Self {
        RunOptions { workers: 1, chunk: 8 }
    }
}

/// What [`run`] did.
#[derive(Debug, Clone, PartialEq)]
pub struct RunReport {
    /// Evidence entries appended by this call.
    pub appended_evidence: usize,
    /// Result entries appended by this call (several for a batch).
    pub appended_results: usize,
    /// The slot is a batch whose other members are not complete yet.
    pub pending: bool,
    /// The verification after the run.
    pub verification: Verification,
}

/// Run hypothesis `id` with `model`: evaluate the missing registered pairs, append them in pair and arm order, and append the
/// result entries of the slot when its evidence is complete. Refused, with the located problems, when the hypothesis does not
/// verify, is withdrawn, or the model is not the registered evaluator.
pub fn run(ledger: &Ledger, id: &str, model: &dyn Model, opts: &RunOptions) -> Result<RunReport, String> {
    let entries = ledger.read()?.entries;
    let regs = registrations(&entries);
    let j = regs
        .iter()
        .position(|r| r.id == id)
        .ok_or_else(|| format!("run: no registration of {id} in {}", ledger.path().display()))?;
    let (members, _) = verify_slot(&entries, &regs, j, None);
    let me = members.iter().find(|m| m.reg.id == id).expect("member");
    if !me.v.problems.is_empty() {
        return Err(format!("run {id}: refused: {}", me.v.problems.join("; ")));
    }
    if me.v.withdrawn {
        return Err(format!("run {id}: refused: the hypothesis was withdrawn"));
    }
    let h = me.rec.clone().expect("verified");
    if model.id() != h.evaluator.id || model.version() != h.evaluator.version || model.fidelity() != h.evaluator.tier {
        return Err(format!(
            "run {id}: refused: the model is {} version '{}' tier {}, the registration names {} version '{}' tier {}",
            model.id(),
            model.version(),
            model.fidelity().as_str(),
            h.evaluator.id,
            h.evaluator.version,
            h.evaluator.tier.as_str()
        ));
    }
    let reg = me.reg.clone();
    let reg_ref = entry_ref(reg.index, &entries[reg.index]);
    let pts = design_points(&h, reg.derived_seed);
    let have: Vec<Option<usize>> = me.v.evidence.clone();
    let missing: Vec<usize> = (0..h.n_pairs)
        .filter(|&p| have[2 * p].is_none() || have[2 * p + 1].is_none())
        .collect();
    let mut appended = 0;
    for chunk in missing.chunks(opts.chunk.max(1)) {
        let out: Vec<Vec<Entry>> = parallel_map(chunk, opts.workers, &|&p: &usize| {
            let mut es = Vec::new();
            let mut low_held: Option<Vec<f64>> = None;
            let held = &h.held;
            match have[2 * p] {
                Some(k) => {
                    if !held.is_empty() {
                        let e = &entries[k];
                        low_held = (e.verdict == Verdict::Ok)
                            .then(|| {
                                held.iter()
                                    .map(|hd| e.metric(&hd.metric).filter(|q| q.unit == hd.unit).map(|q| q.value))
                                    .collect::<Option<Vec<f64>>>()
                            })
                            .flatten();
                    }
                }
                None => {
                    let r = eval_arm(model, &h, &pts[p], Arm::Low, None);
                    if !held.is_empty() {
                        low_held = held_values(&r.1, &h);
                    }
                    es.push(evidence_entry(model, &reg, &reg_ref, p, Arm::Low, r));
                }
            }
            if have[2 * p + 1].is_none() {
                let r = eval_arm(model, &h, &pts[p], Arm::High, low_held.as_deref());
                es.push(evidence_entry(model, &reg, &reg_ref, p, Arm::High, r));
            }
            es
        });
        let flat: Vec<Entry> = out.into_iter().flatten().collect();
        appended += flat.len();
        ledger.append(&flat)?;
    }
    let (entries, appended_results, pending) = finish_slot(ledger, id)?;
    let v = verify(&entries, id, None);
    Ok(RunReport {
        appended_evidence: appended,
        appended_results,
        pending,
        verification: v,
    })
}

/// Append the missing result entries of the slot of `id` when its evidence is complete.
fn finish_slot(ledger: &Ledger, id: &str) -> Result<(Vec<Entry>, usize, bool), String> {
    let entries = ledger.read()?.entries;
    let regs = registrations(&entries);
    let j = regs.iter().position(|r| r.id == id).expect("registered");
    let (members, expected) = verify_slot(&entries, &regs, j, None);
    if let Some(m) = members.iter().find(|m| !m.v.problems.is_empty()) {
        return Err(format!("run {id}: {}: {}", m.reg.id, m.v.problems.join("; ")));
    }
    if expected.iter().all(Option::is_none) {
        return Ok((entries, 0, true));
    }
    let new: Vec<Entry> = members
        .iter()
        .zip(expected)
        .filter(|(m, _)| m.v.genuine.is_none())
        .filter_map(|(_, e)| e)
        .collect();
    if new.is_empty() {
        return Ok((entries, 0, false));
    }
    ledger.append(&new)?;
    Ok((ledger.read()?.entries, new.len(), false))
}

// ---------------------------------------------------------------- lessons

/// Rank of a tier: analytic < reduced_order < cfd < measured.
pub fn tier_rank(f: Fidelity) -> u8 {
    match f {
        Fidelity::Analytic => 0,
        Fidelity::ReducedOrder => 1,
        Fidelity::Cfd => 2,
        Fidelity::Measured => 3,
    }
}

fn next_tier(f: Fidelity) -> Option<Fidelity> {
    match f {
        Fidelity::Analytic => Some(Fidelity::ReducedOrder),
        Fidelity::ReducedOrder => Some(Fidelity::Cfd),
        Fidelity::Cfd => Some(Fidelity::Measured),
        Fidelity::Measured => None,
    }
}

/// A model error declared by the caller for an evaluator and a metric (Q13 (b)).
#[derive(Debug, Clone, PartialEq)]
pub struct DeclaredError {
    /// Evaluator id.
    pub id: String,
    /// Evaluator version.
    pub version: String,
    /// Metric.
    pub metric: String,
    /// Error (metric unit).
    pub value: f64,
}

/// The lessons query.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LessonQuery {
    /// Only this factor.
    pub factor: Option<String>,
    /// Only this response.
    pub response: Option<String>,
    /// Only this finding.
    pub finding: Option<Finding>,
    /// Smallest tier returned.
    pub min_tier: Option<Fidelity>,
    /// Also return stale rows (labelled).
    pub include_stale: bool,
    /// Evaluator (id, version) pairs the caller holds now; a row whose scope is not among them is stale.
    pub current: Vec<(String, String)>,
    /// Declared model errors.
    pub declared_errors: Vec<DeclaredError>,
    /// NEGATIVE-CONTROL SWITCH (ADR 0624 NC5): read the stored verdict of the last result line, without recomputation. Never
    /// set it outside a check.
    pub trust_stored: bool,
}

/// Known model error of the tier for the response.
#[derive(Debug, Clone, PartialEq)]
pub enum ModelError {
    /// Neither a fidelity summary nor a declared value.
    Unknown,
    /// Known.
    Known {
        /// Value (response unit).
        value: f64,
        /// Where it comes from.
        source: String,
        /// `|mean| <` value.
        below: bool,
    },
}

impl ModelError {
    /// `model_error_unknown`, `below_model_error` or `above_model_error`.
    pub fn flag(&self) -> &'static str {
        match self {
            ModelError::Unknown => "model_error_unknown",
            ModelError::Known { below: true, .. } => "below_model_error",
            ModelError::Known { below: false, .. } => "above_model_error",
        }
    }
}

/// One row of [`lessons`].
#[derive(Debug, Clone, PartialEq)]
pub struct Lesson {
    /// Hypothesis id.
    pub id: String,
    /// Statement generated from the record and the result (not from the claim text).
    pub statement: String,
    /// The author's words.
    pub claim_text: String,
    /// Finding.
    pub finding: Finding,
    /// Refuted with the opposite sign.
    pub opposite: bool,
    /// Why void.
    pub void_reason: Option<String>,
    /// Factor name.
    pub factor: String,
    /// Response name.
    pub response: String,
    /// Scope.
    pub scope: Scope,
    /// Model error.
    pub model_error: ModelError,
    /// Family slot and level.
    pub slot: usize,
    /// Effective level.
    pub level: f64,
    /// Registration entry.
    pub registration: usize,
    /// Result entry.
    pub result: usize,
    /// Evidence entries.
    pub evidence: Vec<usize>,
    /// Stale against the caller's evaluators.
    pub stale: bool,
    /// The claim this one replicates.
    pub replicates: Option<String>,
    /// Hypotheses that replicate this one.
    pub replicated_by: Vec<String>,
    /// Whether the evidence was re-run.
    pub evidence_rerun: Option<bool>,
    /// Statistics.
    pub stats: Stats,
}

/// A result line or a hypothesis that is not reported, with the reason.
#[derive(Debug, Clone, PartialEq)]
pub struct Flag {
    /// Entry index.
    pub index: usize,
    /// Hypothesis id.
    pub hypothesis: String,
    /// Reason.
    pub problem: String,
}

/// Output of [`lessons`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Lessons {
    /// Rows, in registration order.
    pub rows: Vec<Lesson>,
    /// Result lines and hypotheses not reported.
    pub flagged: Vec<Flag>,
}

fn model_error(entries: &[Entry], h: &Hypothesis, mean_change: f64, declared: &[DeclaredError]) -> ModelError {
    let m = &h.response.name;
    for (k, e) in entries.iter().enumerate().rev() {
        if e.kind != "fidelity_summary" {
            continue;
        }
        let low = e.settings.get("correction").and_then(|c| c.get("low"));
        let same = low.and_then(|l| l.get("id")).and_then(Value::as_str) == Some(h.evaluator.id.as_str())
            && low.and_then(|l| l.get("version")).and_then(Value::as_str) == Some(h.evaluator.version.as_str());
        let n = e.metric(&format!("{m}.n_pairs")).map_or(0.0, |q| q.value);
        if let (true, true, Some(q)) = (same, n >= 1.0, e.metric(&format!("{m}.oos_rmse"))) {
            return ModelError::Known {
                value: q.value,
                source: format!("fidelity_summary at entry {k}"),
                below: mean_change.abs() < q.value,
            };
        }
    }
    if let Some(d) = declared
        .iter()
        .find(|d| d.id == h.evaluator.id && d.version == h.evaluator.version && d.metric == *m)
    {
        return ModelError::Known {
            value: d.value,
            source: "declared by the caller".into(),
            below: mean_change.abs() < d.value,
        };
    }
    ModelError::Unknown
}

fn fmt_num(x: f64) -> String {
    format!("{x:.4e}")
}

fn statement(h: &Hypothesis, l: &Lesson, higher: Option<(&str, Fidelity)>) -> String {
    let s = h.direction.sign();
    let st = &l.stats;
    let mut t = format!(
        "{} ({} {}): raising `{}` from {} to {} {} ",
        h.evaluator.tier.as_str(),
        h.evaluator.id,
        h.evaluator.version,
        h.factor.name,
        h.factor.low,
        h.factor.high,
        h.factor.unit
    );
    if l.finding == Finding::Void {
        t.push_str(&format!(
            "on `{}`: void ({}).",
            h.response.name,
            l.void_reason.clone().unwrap_or_default()
        ));
    } else {
        let (a, b) = (s * st.lower, s * st.upper);
        t.push_str(&format!(
            "changed `{}` by {} {} on average (one-sided bounds {}, {}; {} of {} pairs with d > 0)",
            h.response.name,
            fmt_num(s * st.mean),
            h.response.unit,
            fmt_num(a.min(b)),
            fmt_num(a.max(b)),
            (st.sign_share * st.n as f64).round() as usize,
            st.n
        ));
        if !h.held.is_empty() {
            let holds: Vec<String> = h
                .held
                .iter()
                .map(|hd| {
                    format!(
                        "`{}` held within {} {} by `{}`",
                        hd.metric, hd.tolerance, hd.unit, hd.by
                    )
                })
                .collect();
            t.push_str(&format!(" with {}", holds.join(", ")));
        }
        t.push_str(" over the registered domain.");
        if st.all_zero {
            t.push_str(&format!(
                " Every paired difference is exactly 0: the model does not respond to `{}` over the domain (a property of the \
                 model, not of the product).",
                h.factor.name
            ));
        }
    }
    t.push_str(&format!(
        " {} as hypothesis {} of the family (slot {}, level {:.3e}{}).",
        match l.finding {
            Finding::Confirmed => "Confirmed",
            Finding::Refuted =>
                if l.opposite {
                    "Refuted (opposite sign)"
                } else {
                    "Refuted"
                },
            Finding::Inconclusive => "Inconclusive",
            Finding::Void => "Void",
        },
        l.id,
        l.slot,
        l.level,
        if h.direction == Direction::NoEffect {
            ", equivalence claim"
        } else {
            ""
        }
    ));
    t.push_str(&match &l.model_error {
        ModelError::Unknown => " Model error of this tier unknown.".to_string(),
        ModelError::Known { value, source, below } => format!(
            " Model error of this tier {} ({source}); the mean change is {} it.",
            fmt_num(*value),
            if *below { "below" } else { "above" }
        ),
    });
    match (higher, next_tier(h.evaluator.tier)) {
        (Some((id, tier)), _) => t.push_str(&format!(" Re-tested at {} as {id}.", tier.as_str())),
        (None, Some(n)) => t.push_str(&format!(" Not tested at {}.", n.as_str())),
        (None, None) => {}
    }
    t
}

fn stats_from_entry(e: &Entry) -> Stats {
    let g = |k: &str| e.metric(k).map_or(f64::NAN, |q| q.value);
    Stats {
        n: e.metric("n_ok").map_or(0, |q| q.value as usize),
        mean: g("mean"),
        sd: g("sd"),
        se: g("se"),
        t: g("t"),
        lower: g("lower"),
        upper: g("upper"),
        p_value: g("p_value"),
        level: g("level"),
        opposite: e.settings.get("opposite").and_then(Value::as_bool).unwrap_or(false),
        sign_share: g("sign_share"),
        sign_ci: (g("sign_ci_lo"), g("sign_ci_hi")),
        all_zero: e.settings.get("all_zero").and_then(Value::as_bool).unwrap_or(false),
    }
}

/// What is currently known: one row per hypothesis that verifies and has a genuine result (recomputed, never the stored
/// verdict), with scope, model error and staleness. Tier analytic is never returned. `rerun` models re-run the evidence of the
/// hypotheses whose evaluator they are (Q14 (b)).
pub fn lessons(entries: &[Entry], q: &LessonQuery, rerun: &[&dyn Model]) -> Lessons {
    let regs = registrations(entries);
    let mut out = Lessons::default();
    let mut rows: Vec<(Lesson, Hypothesis)> = Vec::new();
    for r in &regs {
        let Ok(h) = r.record.clone() else {
            out.flagged.push(Flag {
                index: r.index,
                hypothesis: r.id.clone(),
                problem: "the record does not parse".into(),
            });
            continue;
        };
        let (finding, stats, result_idx, evidence, evidence_rerun, void_reason, level) = if q.trust_stored {
            let Some((k, e)) = result_entries_of(entries, &r.id).last() else {
                continue;
            };
            let f = e
                .settings
                .get("verdict")
                .and_then(Value::as_str)
                .and_then(Finding::parse)
                .unwrap_or(Finding::Void);
            let st = stats_from_entry(e);
            (
                f,
                st,
                k,
                e.parents.iter().skip(1).map(|p| p.index).collect::<Vec<_>>(),
                None,
                e.settings
                    .get("void_reason")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                st.level,
            )
        } else {
            let model = rerun
                .iter()
                .copied()
                .find(|m| m.id() == h.evaluator.id && m.version() == h.evaluator.version);
            let v = verify(entries, &r.id, model);
            for (k, why) in &v.flagged {
                out.flagged.push(Flag {
                    index: *k,
                    hypothesis: r.id.clone(),
                    problem: why.clone(),
                });
            }
            if !v.problems.is_empty() {
                out.flagged.push(Flag {
                    index: r.index,
                    hypothesis: r.id.clone(),
                    problem: v.problems.join("; "),
                });
                continue;
            }
            let (Some(k), Some(res)) = (v.genuine, v.result.clone()) else {
                continue;
            };
            (
                res.finding,
                res.stats,
                k,
                v.evidence.iter().flatten().copied().collect(),
                v.evidence_rerun,
                res.void_reason.clone(),
                res.level,
            )
        };
        if h.evaluator.tier == Fidelity::Analytic {
            continue;
        }
        let mean_change = h.direction.sign() * stats.mean;
        let lesson = Lesson {
            id: r.id.clone(),
            statement: String::new(),
            claim_text: h.claim_text.clone(),
            finding,
            opposite: stats.opposite,
            void_reason,
            factor: h.factor.name.clone(),
            response: h.response.name.clone(),
            scope: h.evaluator.clone(),
            model_error: model_error(entries, &h, mean_change, &q.declared_errors),
            slot: r.slot,
            level,
            registration: r.index,
            result: result_idx,
            evidence,
            stale: !q
                .current
                .iter()
                .any(|(i, v)| *i == h.evaluator.id && *v == h.evaluator.version),
            replicates: h.replicates.clone(),
            replicated_by: Vec::new(),
            evidence_rerun,
            stats,
        };
        rows.push((lesson, h));
    }
    // links between replicates (rows only)
    let links: Vec<(String, String, Fidelity)> = rows
        .iter()
        .filter_map(|(l, h)| l.replicates.clone().map(|t| (t, l.id.clone(), h.evaluator.tier)))
        .collect();
    for (l, h) in &mut rows {
        l.replicated_by = links
            .iter()
            .filter(|(t, _, _)| *t == l.id)
            .map(|x| x.1.clone())
            .collect();
        let higher = links
            .iter()
            .find(|(t, _, tier)| *t == l.id && tier_rank(*tier) > tier_rank(h.evaluator.tier))
            .map(|(_, id, tier)| (id.as_str(), *tier));
        l.statement = statement(h, l, higher);
    }
    out.rows = rows
        .into_iter()
        .map(|(l, _)| l)
        .filter(|l| q.include_stale || !l.stale)
        .filter(|l| q.factor.as_ref().is_none_or(|f| *f == l.factor))
        .filter(|l| q.response.as_ref().is_none_or(|f| *f == l.response))
        .filter(|l| q.finding.is_none_or(|f| f == l.finding))
        .filter(|l| q.min_tier.is_none_or(|t| tier_rank(l.scope.tier) >= tier_rank(t)))
        .collect();
    out
}

// ---------------------------------------------------------------- lessons that read the drawn checks (ADR 0695)

/// A [`Lesson`] with the verifier-drawn checks of its hypothesis counted (ADR 0695 D2).
#[derive(Debug, Clone, PartialEq)]
pub struct DrawnRow {
    /// The row [`lessons`] gives.
    pub lesson: Lesson,
    /// Consistent drawn records of the hypothesis (reproduced or not).
    pub drawn_checks: usize,
    /// Of those, the ones that did not reproduce.
    pub drawn_failed: usize,
    /// Drawn records of the hypothesis whose redo is inconsistent (ignored).
    pub drawn_ignored: usize,
    /// Drawn records that could not be redone because the caller holds no model of the evaluator (ignored).
    pub drawn_unchecked: usize,
    /// Entry index of the latest consistent drawn record.
    pub drawn_latest: Option<usize>,
}

/// A row dropped because its latest consistent drawn record did not reproduce.
#[derive(Debug, Clone, PartialEq)]
pub struct DrawnDrop {
    /// Hypothesis id.
    pub id: String,
    /// Registration entry.
    pub registration: usize,
    /// The drawn record.
    pub record: usize,
    /// Evidence entries the record names as not reproduced.
    pub mismatched: Vec<usize>,
}

/// Output of [`lessons_drawn`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DrawnLessons {
    /// Surviving rows, in the order of [`lessons`].
    pub rows: Vec<DrawnRow>,
    /// Exactly the flags of [`lessons`].
    pub flagged: Vec<Flag>,
    /// Ignored drawn records and dropped rows, each with its reason.
    pub drawn_flags: Vec<Flag>,
    /// Rows dropped.
    pub dropped: Vec<DrawnDrop>,
}

impl DrawnLessons {
    /// The surviving rows and the flags of [`lessons`] as a [`Lessons`].
    pub fn plain(&self) -> Lessons {
        Lessons {
            rows: self.rows.iter().map(|r| r.lesson.clone()).collect(),
            flagged: self.flagged.clone(),
        }
    }
}

/// [`lessons`] (unchanged) followed by the verifier-drawn checks (ADR 0695 D2): a row whose latest consistent drawn record (ledger
/// order; consistent = [`drawn_record_problems`] with the caller's model is empty and the record names the row's registration
/// hash) did not reproduce is dropped; every row carries the number of its drawn checks. Records that are inconsistent, or that
/// cannot be redone for lack of a model among `rerun`, are ignored and counted. Without drawn records the result is that of
/// [`lessons`].
pub fn lessons_drawn(entries: &[Entry], q: &LessonQuery, rerun: &[&dyn Model]) -> DrawnLessons {
    let base = lessons(entries, q, rerun);
    let mut out = DrawnLessons {
        flagged: base.flagged,
        ..Default::default()
    };
    for lesson in base.rows {
        let hash = entries
            .get(lesson.registration)
            .and_then(|e| e.provenance.get("hash"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let model = rerun
            .iter()
            .copied()
            .find(|m| m.id() == lesson.scope.id && m.version() == lesson.scope.version);
        let mut row = DrawnRow {
            lesson,
            drawn_checks: 0,
            drawn_failed: 0,
            drawn_ignored: 0,
            drawn_unchecked: 0,
            drawn_latest: None,
        };
        let mut latest: Option<(usize, bool, Vec<usize>)> = None;
        for (k, e) in entries.iter().enumerate() {
            let of_row = e.kind == KIND_DRAWN
                && e.variant == row.lesson.id
                && e.parents.first().map(|p| p.index) == Some(row.lesson.registration);
            if !of_row {
                continue;
            }
            let Some(model) = model else {
                row.drawn_unchecked += 1;
                continue;
            };
            let mut why = drawn_record_problems(entries, k, model);
            if e.settings.get("record_hash").and_then(Value::as_str) != Some(hash.as_str()) {
                why.push(format!("entry {k}: the record names another registration hash"));
            }
            if !why.is_empty() {
                row.drawn_ignored += 1;
                out.drawn_flags.push(Flag {
                    index: k,
                    hypothesis: row.lesson.id.clone(),
                    problem: format!("drawn record ignored: {}", why.join("; ")),
                });
                continue;
            }
            let reproduced = e.settings.get("reproduced").and_then(Value::as_bool) == Some(true);
            let mismatched: Vec<usize> = e
                .settings
                .get("mismatched")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(|x| x.as_u64().map(|x| x as usize)).collect())
                .unwrap_or_default();
            row.drawn_checks += 1;
            if !reproduced {
                row.drawn_failed += 1;
            }
            latest = Some((k, reproduced, mismatched));
        }
        row.drawn_latest = latest.as_ref().map(|l| l.0);
        match latest {
            Some((k, false, mismatched)) => {
                out.drawn_flags.push(Flag {
                    index: k,
                    hypothesis: row.lesson.id.clone(),
                    problem: format!("row dropped: its latest drawn check did not reproduce (entries {mismatched:?})"),
                });
                out.dropped.push(DrawnDrop {
                    id: row.lesson.id.clone(),
                    registration: row.lesson.registration,
                    record: k,
                    mismatched,
                });
            }
            _ => out.rows.push(row),
        }
    }
    out
}

// ---------------------------------------------------------------- explore

/// One (parameter, metric) cell of [`explore`].
#[derive(Debug, Clone, PartialEq)]
pub struct Cell {
    /// Parameter.
    pub param: String,
    /// Metric.
    pub metric: String,
    /// Spearman rank correlation.
    pub rho: f64,
    /// Entries used.
    pub n: usize,
}

/// Output of [`explore`]: a reading aid, never evidence.
#[derive(Debug, Clone, PartialEq)]
pub struct Exploration {
    /// Always true: these numbers are exploratory.
    pub exploratory: bool,
    /// Cells by `|rho|` descending (ties: parameter, then metric, in first-seen order).
    pub cells: Vec<Cell>,
}

fn ranks(v: &[f64]) -> Vec<f64> {
    let mut o: Vec<usize> = (0..v.len()).collect();
    o.sort_by(|&a, &b| v[a].total_cmp(&v[b]).then(a.cmp(&b)));
    let mut r = vec![0.0; v.len()];
    let mut i = 0;
    while i < o.len() {
        let mut j = i;
        while j + 1 < o.len() && v[o[j + 1]] == v[o[i]] {
            j += 1;
        }
        let avg = 0.5 * (i + j) as f64;
        for &k in &o[i..=j] {
            r[k] = avg;
        }
        i = j + 1;
    }
    r
}

fn pearson(x: &[f64], y: &[f64]) -> f64 {
    let n = x.len() as f64;
    let mx = x.iter().sum::<f64>() / n;
    let my = y.iter().sum::<f64>() / n;
    let (mut sxy, mut sxx, mut syy) = (0.0, 0.0, 0.0);
    for (a, b) in x.iter().zip(y) {
        sxy += (a - mx) * (b - my);
        sxx += (a - mx) * (a - mx);
        syy += (b - my) * (b - my);
    }
    sxy / (sxx * syy).sqrt()
}

/// Spearman rank correlation of every parameter with every metric over the `ok` entries that `filter` selects (a reading aid
/// for forming hypotheses; marked exploratory, never evidence).
pub fn explore(entries: &[Entry], filter: &Filter) -> Exploration {
    let idx: Vec<usize> = filter
        .apply(entries)
        .into_iter()
        .filter(|&i| entries[i].verdict == Verdict::Ok)
        .collect();
    let mut params: Vec<String> = Vec::new();
    let mut metrics: Vec<String> = Vec::new();
    for &i in &idx {
        for p in &entries[i].params {
            if !params.contains(&p.name) {
                params.push(p.name.clone());
            }
        }
        for m in &entries[i].metrics {
            if !metrics.contains(&m.name) {
                metrics.push(m.name.clone());
            }
        }
    }
    let mut cells = Vec::new();
    for (pi, p) in params.iter().enumerate() {
        for (mi, m) in metrics.iter().enumerate() {
            let (xs, ys): (Vec<f64>, Vec<f64>) = idx
                .iter()
                .filter_map(|&i| Some((entries[i].param(p)?.value, entries[i].metric(m)?.value)))
                .filter(|(a, b)| a.is_finite() && b.is_finite())
                .unzip();
            if xs.len() < 3 {
                continue;
            }
            let rho = pearson(&ranks(&xs), &ranks(&ys));
            if rho.is_finite() {
                cells.push((
                    pi,
                    mi,
                    Cell {
                        param: p.clone(),
                        metric: m.clone(),
                        rho,
                        n: xs.len(),
                    },
                ));
            }
        }
    }
    cells.sort_by(|a, b| {
        b.2.rho
            .abs()
            .total_cmp(&a.2.rho.abs())
            .then((a.0, a.1).cmp(&(b.0, b.1)))
    });
    Exploration {
        exploratory: true,
        cells: cells.into_iter().map(|c| c.2).collect(),
    }
}
