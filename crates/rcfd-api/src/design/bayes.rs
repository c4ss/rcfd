//! Expected-improvement proposal of the next variant over a Gaussian-process surrogate (ADR 0621, F-698): efficient global
//! optimisation after Jones, Schonlau and Welch (1998), with failed evaluations imputed at the predicted mean plus one
//! standard deviation (Forrester, Sobester and Keane 2006), the probability of feasibility for declared constraints (Schonlau,
//! Welch and Jones 1998), and the kriging-believer rule for batches and pending entries (Ginsbourger, Le Riche and Carraro
//! 2010).
//!
//! [`propose`] is a deterministic function of (bounds, observations as a set, options): the observations are put in a
//! canonical order first, so the order in which sessions appended them to the ledger does not matter. [`minimise`] has the
//! batch interface of the other optimisers (`optim::Eval`); [`run`] drives a `ledger::Model` and records every proposal as
//! a ledger entry with its predicted mean and standard deviation. Library only (no service method, no case key).

use super::ledger::{
    entry_of, entry_ref, training_set, Entry, EntryRef, Identity, Ledger, Model, Outcome, Quantity, Verdict,
};
use super::optim::{nelder_mead, Bounds, Eval, Fitness, NmOptions, OptResult, Stop};
use super::rng::Rng;
use super::sampling::latin_hypercube;
use super::study::{parallel_map, Cmp, Sense};
use super::surrogate::{Gp, GpOptions, Noise};
use super::uq::{evaluate_guarded, norm_cdf};
use serde_json::{json, Value};

/// Standard normal density.
fn norm_pdf(z: f64) -> f64 {
    (-0.5 * z * z).exp() / (2.0 * std::f64::consts::PI).sqrt()
}

/// Expected improvement of a minimisation below `incumbent` under `N(mean, sd^2)` (Jones et al. 1998 eq. 15):
/// `(f_min - m) Phi(z) + s phi(z)`, `z = (f_min - m) / s`; `max(f_min - m, 0)` when `sd = 0`.
pub fn expected_improvement(mean: f64, sd: f64, incumbent: f64) -> f64 {
    let d = incumbent - mean;
    if sd.is_nan() || sd <= 0.0 {
        return d.max(0.0);
    }
    let z = d / sd;
    (d * norm_cdf(z) + sd * norm_pdf(z)).max(0.0)
}

/// Probability that a constraint `c <= 0` holds under `N(mean, sd^2)`.
pub fn prob_feasible(mean: f64, sd: f64) -> f64 {
    if sd.is_nan() || sd <= 0.0 {
        return if mean <= 0.0 { 1.0 } else { 0.0 };
    }
    norm_cdf(-mean / sd)
}

/// What is known at one point.
#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    /// Evaluated: the minimised objective and the constraint values (`c_k <= 0` holds).
    Ok {
        /// Objective (minimised).
        f: f64,
        /// Constraint values.
        c: Vec<f64>,
    },
    /// Evaluated and failed (diverged, refused by a constraint before evaluation, or a non-finite objective).
    Failed,
    /// Proposed and not yet evaluated.
    Pending,
}

/// One observation, in parameter space.
#[derive(Debug, Clone, PartialEq)]
pub struct Observation {
    /// Parameters.
    pub x: Vec<f64>,
    /// Status.
    pub status: Status,
}

/// Options of [`propose`], [`minimise`] and [`run`].
#[derive(Debug, Clone, PartialEq)]
pub struct BayesOptions {
    /// Gaussian-process options (its seed is replaced by one derived from `seed` and the number of observations).
    pub gp: GpOptions,
    /// Initial design size (default `max(5 d, 10)`); existing observations count toward it.
    pub n0: Option<usize>,
    /// Points per proposal (kriging believer beyond the first).
    pub batch: usize,
    /// Seed.
    pub seed: u64,
    /// Threads for the candidate scoring, the likelihood starts and the evaluations of `run` (any count gives the same bits).
    pub workers: usize,
    /// Impute failed points at `m + s` (default true; false drops them and keeps only the exclusion radius).
    pub impute: bool,
    /// Candidates per parameter of the acquisition search.
    pub candidates_per_dim: usize,
    /// Exclusion radius in the unit cube around evaluated and pending points.
    pub exclusion: f64,
    /// Model the success of an evaluation: a Gaussian process on the label (+1 failed, -1 ok) whose probability of a
    /// negative value multiplies the acquisition (least-squares classification, Rasmussen and Williams 2006 section 6.5).
    pub classify_failures: bool,
}

impl Default for BayesOptions {
    fn default() -> Self {
        BayesOptions {
            gp: GpOptions::default(),
            n0: None,
            batch: 1,
            seed: 1,
            workers: 1,
            impute: true,
            candidates_per_dim: 1000,
            exclusion: 1e-6,
            classify_failures: true,
        }
    }
}

/// How a point was chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickKind {
    /// A point of the initial Latin hypercube (no model).
    Initial,
    /// Space filling because too few evaluations succeeded for a fit.
    Filling,
    /// Expected improvement (times the probability of feasibility when constraints are declared).
    Ei,
    /// Probability of feasibility alone (no feasible point yet).
    Feasibility,
}

impl PickKind {
    /// Text form.
    pub fn as_str(self) -> &'static str {
        match self {
            PickKind::Initial => "initial",
            PickKind::Filling => "filling",
            PickKind::Ei => "ei",
            PickKind::Feasibility => "pof",
        }
    }
}

/// Record of one proposed point.
#[derive(Debug, Clone, PartialEq)]
pub struct Pick {
    /// The point, parameter space.
    pub x: Vec<f64>,
    /// How it was chosen.
    pub kind: PickKind,
    /// Acquisition value (expected improvement, times the feasibility probability), NaN without a model.
    pub acquisition: f64,
    /// Expected improvement alone, NaN without a model or an incumbent.
    pub ei: f64,
    /// Predicted mean of the minimised objective, NaN without a model.
    pub mean: f64,
    /// Predicted standard deviation, NaN without a model.
    pub sd: f64,
    /// Probability of feasibility (1 without constraints), NaN without a model.
    pub feasibility: f64,
    /// Incumbent (minimised), when any.
    pub incumbent: Option<f64>,
    /// Fitted length scales (empty without a model).
    pub length_scales: Vec<f64>,
    /// Fitted nugget ratio.
    pub nugget: f64,
    /// Jitter of the objective fit.
    pub jitter: f64,
    /// Evaluated observations used (after merging duplicates).
    pub n_obs: usize,
    /// Failed points imputed.
    pub n_imputed: usize,
    /// Pending points (and earlier points of the batch) believed.
    pub n_pending: usize,
}

/// Result of [`propose`].
#[derive(Debug, Clone, PartialEq)]
pub struct Proposal {
    /// The proposed points with their records, in batch order.
    pub picks: Vec<Pick>,
}

fn mix(seed: u64, a: u64, b: u64) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for x in [seed, a, b] {
        for byte in x.to_le_bytes() {
            h ^= u64::from(byte);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    h
}

fn check_bounds(b: &Bounds) -> Result<(), String> {
    if b.is_empty() {
        return Err("bayes: no parameters".into());
    }
    for (i, &(lo, hi)) in b.iter().enumerate() {
        if !(lo.is_finite() && hi.is_finite() && lo < hi) {
            return Err(format!(
                "bayes: parameter {i}: the bounds must be finite with lo < hi (got {lo}, {hi})"
            ));
        }
    }
    Ok(())
}

fn to_unit(x: &[f64], b: &Bounds) -> Vec<f64> {
    x.iter().zip(b).map(|(v, (lo, hi))| (v - lo) / (hi - lo)).collect()
}

fn from_unit(u: &[f64], b: &Bounds) -> Vec<f64> {
    u.iter().zip(b).map(|(v, (lo, hi))| lo + (hi - lo) * v).collect()
}

fn dist(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| (x - y) * (x - y)).sum::<f64>().sqrt()
}

fn rank(s: &Status) -> u8 {
    match s {
        Status::Ok { .. } => 0,
        Status::Failed => 1,
        Status::Pending => 2,
    }
}

fn cmp_lex(a: &[f64], b: &[f64]) -> std::cmp::Ordering {
    for (x, y) in a.iter().zip(b) {
        let o = x.total_cmp(y);
        if o != std::cmp::Ordering::Equal {
            return o;
        }
    }
    a.len().cmp(&b.len())
}

/// Observations in the unit cube, canonical order, duplicates of an evaluated point merged (noise off).
struct Canon {
    ok_u: Vec<Vec<f64>>,
    ok_f: Vec<f64>,
    ok_c: Vec<Vec<f64>>,
    failed: Vec<Vec<f64>>,
    pending: Vec<Vec<f64>>,
    n_constraints: usize,
}

fn canonical(b: &Bounds, obs: &[Observation], merge: bool) -> Result<Canon, String> {
    let d = b.len();
    let mut items: Vec<(Vec<f64>, Status)> = Vec::with_capacity(obs.len());
    let mut nc: Option<usize> = None;
    for (i, o) in obs.iter().enumerate() {
        if o.x.len() != d {
            return Err(format!(
                "bayes: observation {i} has {} parameters, the bounds {d}",
                o.x.len()
            ));
        }
        if let Some(k) = o.x.iter().position(|v| !v.is_finite()) {
            return Err(format!("bayes: observation {i} parameter {k} is not finite"));
        }
        let st = match &o.status {
            Status::Ok { f, c } => {
                match nc {
                    None => nc = Some(c.len()),
                    Some(n) if n != c.len() => {
                        return Err(format!(
                            "bayes: observation {i} has {} constraint values, earlier ones {n}",
                            c.len()
                        ))
                    }
                    _ => {}
                }
                if f.is_finite() && c.iter().all(|v| v.is_finite()) {
                    o.status.clone()
                } else {
                    Status::Failed
                }
            }
            s => s.clone(),
        };
        items.push((to_unit(&o.x, b), st));
    }
    // canonical order: coordinates, then status, then values (bits)
    items.sort_by(|a, b| {
        cmp_lex(&a.0, &b.0)
            .then(rank(&a.1).cmp(&rank(&b.1)))
            .then_with(|| match (&a.1, &b.1) {
                (Status::Ok { f: fa, c: ca }, Status::Ok { f: fb, c: cb }) => fa.total_cmp(fb).then(cmp_lex(ca, cb)),
                _ => std::cmp::Ordering::Equal,
            })
    });
    let n_constraints = nc.unwrap_or(0);
    let mut c = Canon {
        ok_u: Vec::new(),
        ok_f: Vec::new(),
        ok_c: Vec::new(),
        failed: Vec::new(),
        pending: Vec::new(),
        n_constraints,
    };
    let mut i = 0;
    while i < items.len() {
        match &items[i].1 {
            Status::Ok { .. } => {
                let mut j = i + 1;
                if merge {
                    while j < items.len()
                        && matches!(items[j].1, Status::Ok { .. })
                        && cmp_lex(&items[j].0, &items[i].0) == std::cmp::Ordering::Equal
                    {
                        j += 1;
                    }
                }
                let k = (j - i) as f64;
                let mut f = 0.0;
                let mut cs = vec![0.0; n_constraints];
                for it in &items[i..j] {
                    if let Status::Ok { f: fv, c: cv } = &it.1 {
                        f += fv;
                        for (s, v) in cs.iter_mut().zip(cv) {
                            *s += v;
                        }
                    }
                }
                c.ok_u.push(items[i].0.clone());
                c.ok_f.push(if j - i == 1 { f } else { f / k });
                c.ok_c.push(if j - i == 1 {
                    cs
                } else {
                    cs.iter().map(|v| v / k).collect()
                });
                i = j;
            }
            Status::Failed => {
                c.failed.push(items[i].0.clone());
                i += 1;
            }
            Status::Pending => {
                c.pending.push(items[i].0.clone());
                i += 1;
            }
        }
    }
    Ok(c)
}

/// The fitted models of one proposal step.
struct Models {
    f: Gp,
    c: Vec<Gp>,
    success: Option<Gp>,
    incumbent: Option<f64>,
    n_imputed: usize,
}

impl Models {
    fn acquisition(&self, u: &[f64]) -> (f64, f64, f64, f64, f64) {
        let (m, s) = self.f.predict(u);
        let mut pof: f64 = self
            .c
            .iter()
            .map(|g| {
                let (mc, sc) = g.predict(u);
                prob_feasible(mc, sc)
            })
            .product();
        if let Some(g) = &self.success {
            let (mc, sc) = g.predict(u);
            pof *= prob_feasible(mc, sc);
        }
        match self.incumbent {
            Some(fmin) => {
                let ei = expected_improvement(m, s, fmin);
                (ei * pof, ei, m, s, pof)
            }
            None => (pof, f64::NAN, m, s, pof),
        }
    }

    fn believe(
        &mut self,
        u: &[f64],
        us: &mut Vec<Vec<f64>>,
        fs: &mut Vec<f64>,
        cs: &mut [Vec<f64>],
        cus: &mut Vec<Vec<f64>>,
    ) -> Result<(), String> {
        let (m, _) = self.f.predict(u);
        us.push(u.to_vec());
        fs.push(m);
        self.f = Gp::with_hyper(us, fs, self.f.kernel(), self.f.length_scales(), self.f.nugget())?;
        let mut feasible = true;
        if !self.c.is_empty() {
            cus.push(u.to_vec());
            for (k, g) in self.c.iter_mut().enumerate() {
                let (mc, _) = g.predict(u);
                feasible &= mc <= 0.0;
                cs[k].push(mc);
                *g = Gp::with_hyper(cus, &cs[k], g.kernel(), g.length_scales(), g.nugget())?;
            }
        }
        // the believed value is data: it enters the incumbent (ADR 0621 amendment 2)
        if feasible {
            self.incumbent = Some(self.incumbent.map_or(m, |f| f.min(m)));
        }
        Ok(())
    }
}

/// Propose the next `o.batch` points (ADR 0621 D6 to D8).
pub fn propose(b: &Bounds, obs: &[Observation], o: &BayesOptions) -> Result<Proposal, String> {
    check_bounds(b)?;
    let d = b.len();
    if o.batch == 0 {
        return Err("bayes: batch must be >= 1".into());
    }
    let noisy = !matches!(o.gp.noise, Noise::None);
    let cn = canonical(b, obs, !noisy)?;
    let n_total = obs.len();
    let n0 = o.n0.unwrap_or((5 * d).max(10));
    let mut taken: Vec<Vec<f64>> = cn.ok_u.iter().chain(&cn.failed).chain(&cn.pending).cloned().collect();
    let free = |p: &Vec<f64>, taken: &[Vec<f64>]| taken.iter().all(|t| dist(t, p) > 1e-12);
    let no_model = |x: Vec<f64>, kind: PickKind| Pick {
        x,
        kind,
        acquisition: f64::NAN,
        ei: f64::NAN,
        mean: f64::NAN,
        sd: f64::NAN,
        feasibility: f64::NAN,
        incumbent: None,
        length_scales: Vec::new(),
        nugget: 0.0,
        jitter: 0.0,
        n_obs: cn.ok_u.len(),
        n_imputed: 0,
        n_pending: cn.pending.len(),
    };
    if n_total < n0 {
        let mut picks = Vec::new();
        for p in latin_hypercube(n0, d, o.seed) {
            if picks.len() == o.batch.min(n0 - n_total) {
                break;
            }
            if free(&p, &taken) {
                taken.push(p.clone());
                picks.push(no_model(from_unit(&p, b), PickKind::Initial));
            }
        }
        if !picks.is_empty() {
            return Ok(Proposal { picks });
        }
    }
    if cn.ok_u.len() < d + 2 {
        let mut picks = Vec::new();
        let mut round = 1u64;
        while picks.len() < o.batch {
            for p in latin_hypercube(n0, d, mix(o.seed, n_total as u64, 100 + round)) {
                if picks.len() == o.batch {
                    break;
                }
                if free(&p, &taken) {
                    taken.push(p.clone());
                    picks.push(no_model(from_unit(&p, b), PickKind::Filling));
                }
            }
            round += 1;
        }
        return Ok(Proposal { picks });
    }
    // models
    let mut gpo = o.gp.clone();
    gpo.seed = mix(o.seed, n_total as u64, 1);
    gpo.workers = o.workers;
    let f0 = Gp::fit(&cn.ok_u, &cn.ok_f, &gpo)?;
    let mut us = cn.ok_u.clone();
    let mut fs = cn.ok_f.clone();
    let mut n_imputed = 0;
    let f_model = if o.impute && !cn.failed.is_empty() {
        for u in &cn.failed {
            let (m, s) = f0.predict(u);
            us.push(u.clone());
            fs.push(m + s);
            n_imputed += 1;
        }
        Gp::with_hyper(&us, &fs, f0.kernel(), f0.length_scales(), f0.nugget())?
    } else {
        f0
    };
    let mut cs: Vec<Vec<f64>> = (0..cn.n_constraints)
        .map(|k| cn.ok_c.iter().map(|c| c[k]).collect())
        .collect();
    let mut cus = cn.ok_u.clone();
    let mut c_models = Vec::with_capacity(cn.n_constraints);
    for (k, ck) in cs.iter().enumerate() {
        let mut go = gpo.clone();
        go.seed = mix(o.seed, n_total as u64, 10 + k as u64);
        c_models.push(Gp::fit(&cn.ok_u, ck, &go).map_err(|e| format!("constraint {k}: {e}"))?);
    }
    let feasible: Vec<bool> = cn.ok_c.iter().map(|c| c.iter().all(|v| *v <= 0.0)).collect();
    let incumbent = if noisy {
        (0..cn.ok_u.len())
            .filter(|&i| feasible[i])
            .map(|i| f_model.predict(&cn.ok_u[i]).0)
            .fold(None, |a: Option<f64>, v| Some(a.map_or(v, |a| a.min(v))))
    } else {
        (0..cn.ok_u.len())
            .filter(|&i| feasible[i])
            .map(|i| cn.ok_f[i])
            .fold(None, |a: Option<f64>, v| Some(a.map_or(v, |a| a.min(v))))
    };
    let success = if o.classify_failures && !cn.failed.is_empty() {
        let mut su = cn.ok_u.clone();
        su.extend(cn.failed.iter().cloned());
        let mut sl = vec![-1.0; cn.ok_u.len()];
        sl.extend(std::iter::repeat_n(1.0, cn.failed.len()));
        let mut go = gpo.clone();
        go.seed = mix(o.seed, n_total as u64, 9);
        Some(Gp::fit(&su, &sl, &go).map_err(|e| format!("failure model: {e}"))?)
    } else {
        None
    };
    let mut models = Models {
        f: f_model,
        c: c_models,
        success,
        incumbent,
        n_imputed,
    };
    let mut n_pending = 0;
    for u in &cn.pending {
        models.believe(u, &mut us, &mut fs, &mut cs, &mut cus)?;
        n_pending += 1;
    }
    // best evaluated points (feasible first, then by objective; canonical order breaks ties)
    let mut best: Vec<usize> = (0..cn.ok_u.len()).collect();
    best.sort_by(|&a, &bb| {
        feasible[bb]
            .cmp(&feasible[a])
            .then(cn.ok_f[a].total_cmp(&cn.ok_f[bb]))
            .then(a.cmp(&bb))
    });
    best.truncate(5);
    let mut picks = Vec::new();
    for q in 0..o.batch {
        let mut cand = latin_hypercube(
            o.candidates_per_dim * d,
            d,
            mix(o.seed, n_total as u64, 2 + 1000 * q as u64),
        );
        let mut rng = Rng::new(mix(o.seed, n_total as u64, 3 + 1000 * q as u64));
        for &i in &best {
            for p in 0..200 {
                let s = if p < 100 { 0.02 } else { 0.1 };
                cand.push(
                    cn.ok_u[i]
                        .iter()
                        .map(|v| (v + s * super::cmaes::normal(&mut rng)).clamp(0.0, 1.0))
                        .collect(),
                );
            }
        }
        let excl = o.exclusion;
        let score = |u: &[f64]| -> f64 {
            if taken.iter().any(|t| dist(t, u) < excl) {
                return f64::NEG_INFINITY;
            }
            let a = models.acquisition(u).0;
            if a.is_nan() {
                f64::NEG_INFINITY
            } else {
                a
            }
        };
        let scores = parallel_map(&cand, o.workers, &|u: &Vec<f64>| score(u));
        let mut order: Vec<usize> = (0..cand.len()).collect();
        order.sort_by(|&a, &bb| scores[bb].total_cmp(&scores[a]).then(a.cmp(&bb)));
        order.truncate(5);
        let unit: Vec<(f64, f64)> = vec![(0.0, 1.0); d];
        let refine = |&i: &usize| -> (Vec<f64>, f64) {
            let nm = NmOptions {
                max_evals: 100 * d,
                restarts: 0,
                initial_step: 0.05,
                xtol: 1e-9,
                x0: Some(cand[i].clone()),
                seed: 1,
                ..Default::default()
            };
            let mut ev = |xs: &[Vec<f64>]| -> Result<Vec<Fitness>, Stop> {
                Ok(xs
                    .iter()
                    .map(|u| {
                        let s = score(u);
                        if s.is_finite() {
                            Fitness::new(-s, 0.0)
                        } else {
                            Fitness::FAILED
                        }
                    })
                    .collect())
            };
            match nelder_mead(&unit, &nm, &mut ev) {
                Ok(r) if r.fitness.f.is_finite() && -r.fitness.f >= scores[i] => (r.x, -r.fitness.f),
                _ => (cand[i].clone(), scores[i]),
            }
        };
        let refined = parallel_map(&order, o.workers, &refine);
        let mut win: Option<(Vec<f64>, f64)> = None;
        for (u, s) in refined {
            if win.as_ref().is_none_or(|w| s > w.1) {
                win = Some((u, s));
            }
        }
        let (u, s) = win.expect("five candidates");
        if !s.is_finite() {
            return Err("bayes: every candidate lies inside the exclusion radius of an evaluated point".into());
        }
        let kind = if models.incumbent.is_some() {
            PickKind::Ei
        } else {
            PickKind::Feasibility
        };
        let (acq, ei, m, sd, pof) = models.acquisition(&u);
        picks.push(Pick {
            x: from_unit(&u, b),
            kind,
            acquisition: acq,
            ei,
            mean: m,
            sd,
            feasibility: pof,
            incumbent: models.incumbent,
            length_scales: models.f.length_scales().to_vec(),
            nugget: models.f.nugget(),
            jitter: models.f.jitter(),
            n_obs: cn.ok_u.len(),
            n_imputed: models.n_imputed,
            n_pending,
        });
        taken.push(u.clone());
        if q + 1 < o.batch {
            models.believe(&u, &mut us, &mut fs, &mut cs, &mut cus)?;
            n_pending += 1;
        }
    }
    Ok(Proposal { picks })
}

/// Minimise through the batch interface of the optimisers: propose, evaluate, repeat to `budget` evaluations. A failed
/// evaluation (`f` not finite) and an infeasible one (`violation > 0`) are both treated as failed points (imputed); declared
/// constraints are modelled by [`run`] only. Deterministic in (bounds, options, budget, results).
pub fn minimise(b: &Bounds, o: &BayesOptions, budget: usize, eval: &mut Eval<'_>) -> Result<OptResult, Stop> {
    check_bounds(b).map_err(Stop::Error)?;
    let mut hist: Vec<Observation> = Vec::new();
    let mut best: Option<(Vec<f64>, Fitness)> = None;
    while hist.len() < budget {
        let mut oo = o.clone();
        oo.batch = o.batch.min(budget - hist.len());
        let p = propose(b, &hist, &oo).map_err(Stop::Error)?;
        let xs: Vec<Vec<f64>> = p.picks.iter().map(|k| k.x.clone()).collect();
        let fits = eval(&xs)?;
        for (x, f) in xs.into_iter().zip(fits) {
            let status = if f.f.is_finite() && f.feasible() {
                Status::Ok { f: f.f, c: Vec::new() }
            } else {
                Status::Failed
            };
            if best.as_ref().is_none_or(|bb| f.better(&bb.1)) {
                best = Some((x.clone(), f));
            }
            hist.push(Observation { x, status });
        }
    }
    let (x, fitness) = best.ok_or_else(|| Stop::Error("bayes: budget 0".into()))?;
    Ok(OptResult {
        x,
        fitness,
        evaluations: hist.len(),
        converged: false,
    })
}

/// A parameter of [`run`].
#[derive(Debug, Clone, PartialEq)]
pub struct BayesParam {
    /// Name.
    pub name: String,
    /// Unit.
    pub unit: String,
    /// Lower bound.
    pub lo: f64,
    /// Upper bound.
    pub hi: f64,
}

/// A constraint of [`run`]: `metric cmp limit`.
#[derive(Debug, Clone, PartialEq)]
pub struct BayesConstraint {
    /// Metric.
    pub metric: String,
    /// Comparison.
    pub cmp: Cmp,
    /// Limit.
    pub limit: f64,
}

/// Specification of [`run`].
#[derive(Debug, Clone, PartialEq)]
pub struct BayesSpec {
    /// Study name of the entries read and written.
    pub study: String,
    /// Parameters.
    pub params: Vec<BayesParam>,
    /// Objective metric.
    pub objective: String,
    /// Its sense.
    pub sense: Sense,
    /// Constraints.
    pub constraints: Vec<BayesConstraint>,
    /// Evaluations made by this run.
    pub budget: usize,
    /// Options.
    pub options: BayesOptions,
    /// Fixed clock stamp of the written entries (`None`: the ledger stamps the time).
    pub clock_ms: Option<u64>,
}

impl BayesSpec {
    fn bounds(&self) -> Vec<(f64, f64)> {
        self.params.iter().map(|p| (p.lo, p.hi)).collect()
    }

    /// Validation with located messages.
    pub fn validate(&self) -> Result<(), String> {
        if self.params.is_empty() {
            return Err("bayes: no parameters".into());
        }
        for (i, p) in self.params.iter().enumerate() {
            if p.name.is_empty() || p.unit.is_empty() {
                return Err(format!("bayes: parameter {i}: empty name or unit"));
            }
            if self.params[..i].iter().any(|q| q.name == p.name) {
                return Err(format!("bayes: parameter '{}' given twice", p.name));
            }
            if !(p.lo.is_finite() && p.hi.is_finite() && p.lo < p.hi) {
                return Err(format!(
                    "bayes: parameter '{}': bounds ({}, {}) need lo < hi, finite",
                    p.name, p.lo, p.hi
                ));
            }
        }
        if self.objective.is_empty() {
            return Err("bayes: no objective metric".into());
        }
        for (k, c) in self.constraints.iter().enumerate() {
            if c.metric.is_empty() || !c.limit.is_finite() {
                return Err(format!("bayes: constraint {k}: empty metric or non-finite limit"));
            }
        }
        if self.options.batch == 0 {
            return Err("bayes: batch must be >= 1".into());
        }
        Ok(())
    }
}

/// Observations read from ledger entries.
#[derive(Debug, Clone, PartialEq)]
pub struct LedgerObservations {
    /// Observations (entry order).
    pub obs: Vec<Observation>,
    /// Entry index of each observation.
    pub rows: Vec<usize>,
    /// Entries skipped because a declared parameter is missing.
    pub skipped_missing_param: usize,
    /// Evaluator id, version of the rows.
    pub evaluator: String,
    /// Evaluator version.
    pub evaluator_version: String,
}

fn minimised(spec: &BayesSpec, v: f64) -> f64 {
    if spec.sense == Sense::Max {
        -v
    } else {
        v
    }
}

fn constraint_value(c: &BayesConstraint, v: f64) -> f64 {
    match c.cmp {
        Cmp::Le => v - c.limit,
        Cmp::Ge => c.limit - v,
    }
}

/// Observations of `spec` among the `evaluation` entries `idx` (ADR 0621 D9): one evaluator, version and fidelity level (the
/// F-700 training-set query refuses a mix), the declared parameters in their units; an entry without a declared parameter is
/// skipped and counted; `diverged` and `constraint_rejected` entries are failed points, `not_evaluated` ones pending.
pub fn observations(entries: &[Entry], idx: &[usize], spec: &BayesSpec) -> Result<LedgerObservations, String> {
    let names: Vec<&str> = spec.params.iter().map(|p| p.name.as_str()).collect();
    let mut metrics: Vec<&str> = vec![spec.objective.as_str()];
    metrics.extend(spec.constraints.iter().map(|c| c.metric.as_str()));
    let mut keep = Vec::new();
    let mut skipped = 0;
    for &i in idx {
        let e = &entries[i];
        if e.kind != "evaluation" {
            continue;
        }
        if names.iter().all(|n| e.param(n).is_some()) {
            keep.push(i);
        } else {
            skipped += 1;
        }
    }
    let ts = training_set(entries, &keep, &names, &metrics)?;
    for (c, p) in spec.params.iter().enumerate() {
        if !ts.params[c].1.is_empty() && ts.params[c].1 != p.unit {
            return Err(format!(
                "bayes: parameter '{}' has unit '{}' in the ledger, '{}' declared",
                p.name, ts.params[c].1, p.unit
            ));
        }
    }
    let mut obs = Vec::new();
    let mut rows = Vec::new();
    let x_of = |e: &Entry| -> Vec<f64> { names.iter().map(|n| e.param(n).expect("kept").value).collect() };
    for (r, &i) in ts.rows.iter().enumerate() {
        let f = minimised(spec, ts.y[r][0]);
        let c: Vec<f64> = spec
            .constraints
            .iter()
            .enumerate()
            .map(|(k, cc)| constraint_value(cc, ts.y[r][k + 1]))
            .collect();
        obs.push(Observation {
            x: ts.x[r].clone(),
            status: Status::Ok { f, c },
        });
        rows.push(i);
    }
    for &i in &ts.failed {
        let e = &entries[i];
        let status = if e.verdict == Verdict::NotEvaluated {
            Status::Pending
        } else {
            Status::Failed
        };
        obs.push(Observation { x: x_of(e), status });
        rows.push(i);
    }
    Ok(LedgerObservations {
        obs,
        rows,
        skipped_missing_param: skipped,
        evaluator: ts.evaluator,
        evaluator_version: ts.evaluator_version,
    })
}

/// The proposal of [`run`]'s next step from ledger entries: the `evaluation` entries of `spec.study` from `model_id`.
pub fn propose_from_entries(entries: &[Entry], spec: &BayesSpec, model_id: &str) -> Result<Proposal, String> {
    spec.validate()?;
    let idx: Vec<usize> = (0..entries.len())
        .filter(|&i| entries[i].study == spec.study && entries[i].evaluator == model_id)
        .collect();
    let lo = observations(entries, &idx, spec)?;
    propose(&spec.bounds(), &lo.obs, &spec.options)
}

/// Result of [`run`].
#[derive(Debug, Clone, PartialEq)]
pub struct BayesResult {
    /// The entries this run evaluated (in evaluation order; also appended to the ledger when given).
    pub entries: Vec<Entry>,
    /// The record of every proposed point (same order).
    pub picks: Vec<Pick>,
    /// Index into `entries` of the best feasible `ok` entry of this run and the ones read, when any (`None` if it was read).
    pub best: Option<usize>,
    /// Best feasible objective value over the read and new observations (natural sign), when any.
    pub best_value: Option<f64>,
    /// Failed evaluations of this run.
    pub failed: usize,
    /// Summary entry (kind `surrogate_summary`).
    pub summary: Entry,
}

fn fnv(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Run the proposal loop on `model` for `spec.budget` evaluations, continuing from the entries of `spec.study` from this
/// model already in `ledger` (ADR 0621 D10). Every evaluation is appended as an `evaluation` entry whose provenance carries
/// the pick record (driver `gp_ei`, acquisition, expected improvement, predicted mean and standard deviation, incumbent,
/// length scales, nugget, jitter, observation counts); one `surrogate_summary` entry closes the run.
pub fn run(spec: &BayesSpec, model: &dyn Model, ledger: Option<&Ledger>) -> Result<BayesResult, String> {
    spec.validate()?;
    let b = spec.bounds();
    let mut read: Vec<Entry> = match ledger {
        Some(l) => l.read()?.entries,
        None => Vec::new(),
    };
    let base_len = read.len();
    let sel = |entries: &[Entry]| -> Vec<usize> {
        (0..entries.len())
            .filter(|&i| entries[i].study == spec.study && entries[i].evaluator == model.id())
            .collect()
    };
    let mut new_entries: Vec<Entry> = Vec::new();
    let mut picks_all: Vec<Pick> = Vec::new();
    let mut failed = 0;
    let named = |x: &[f64]| -> Vec<Quantity> {
        spec.params
            .iter()
            .zip(x)
            .map(|(p, &v)| Quantity::new(&p.name, v, &p.unit))
            .collect()
    };
    while new_entries.len() < spec.budget {
        let idx = sel(&read);
        let lo = observations(&read, &idx, spec)?;
        let mut oo = spec.options.clone();
        oo.batch = oo.batch.min(spec.budget - new_entries.len());
        let p = propose(&b, &lo.obs, &oo)?;
        let xs: Vec<Vec<f64>> = p.picks.iter().map(|k| k.x.clone()).collect();
        let outs: Vec<Outcome> = parallel_map(&xs, spec.options.workers, &|x: &Vec<f64>| {
            evaluate_guarded(model, &named(x))
        });
        let mut batch = Vec::new();
        for (k, (pk, out)) in p.picks.iter().zip(outs).enumerate() {
            let sign = if spec.sense == Sense::Max { -1.0 } else { 1.0 };
            let prov = json!({
                "driver": "gp_ei",
                "iteration": lo.obs.len(),
                "batch_index": k,
                "acquisition": pk.kind.as_str(),
                "acquisition_value": num(pk.acquisition),
                "ei": num(pk.ei),
                "predicted_mean": num(sign * pk.mean),
                "predicted_sd": num(pk.sd),
                "feasibility": num(pk.feasibility),
                "incumbent": pk.incumbent.map_or(Value::Null, |v| num(sign * v)),
                "objective": spec.objective,
                "sense": if spec.sense == Sense::Max { "max" } else { "min" },
                "length_scales": pk.length_scales.iter().map(|v| num(*v)).collect::<Vec<_>>(),
                "nugget": num(pk.nugget),
                "jitter": num(pk.jitter),
                "n_obs": pk.n_obs,
                "n_imputed": pk.n_imputed,
                "n_pending": pk.n_pending,
            });
            if out.verdict != Verdict::Ok {
                failed += 1;
            }
            let e = Entry {
                study: spec.study.clone(),
                variant: format!("{}/gp_ei/{}", spec.study, lo.obs.len() + k),
                seed: Some(spec.options.seed),
                provenance: prov,
                recorded_at_ms: spec.clock_ms.unwrap_or(0),
                ..entry_of(model, named(&pk.x), out)
            };
            batch.push(e);
        }
        if let Some(l) = ledger {
            l.append(&batch)?;
        }
        for e in &batch {
            let mut e2 = e.clone();
            if e2.solver_version.is_empty() {
                e2.solver_version = super::ledger::solver_version();
            }
            read.push(e2);
        }
        new_entries.extend(batch);
        picks_all.extend(p.picks);
    }
    // best over everything of this study and model, and the final fit for the summary
    let idx = sel(&read);
    let lo = observations(&read, &idx, spec)?;
    let mut best_value: Option<f64> = None;
    let mut best_row: Option<usize> = None;
    for (o, &row) in lo.obs.iter().zip(&lo.rows) {
        if let Status::Ok { f, c } = &o.status {
            if c.iter().all(|v| *v <= 0.0) && best_value.is_none_or(|bv| *f < bv) {
                best_value = Some(*f);
                best_row = Some(row);
            }
        }
    }
    let best = best_row.and_then(|r| r.checked_sub(base_len));
    let sign = if spec.sense == Sense::Max { -1.0 } else { 1.0 };
    let ok: Vec<(Vec<f64>, f64, usize)> = lo
        .obs
        .iter()
        .zip(&lo.rows)
        .filter_map(|(o, &r)| match &o.status {
            Status::Ok { f, .. } => Some((to_unit(&o.x, &b), *f, r)),
            _ => None,
        })
        .collect();
    let mut metrics = vec![
        Quantity::new("evaluations", new_entries.len() as f64, "1"),
        Quantity::new("failed", failed as f64, "1"),
        Quantity::new("observations", lo.obs.len() as f64, "1"),
    ];
    if let Some(v) = best_value {
        metrics.push(Quantity::new("best", sign * v, "1"));
    }
    let mut fitted = None;
    let mut parents: Vec<EntryRef> = Vec::new();
    let d = b.len();
    if ok.len() >= d + 2 {
        let mut gpo = spec.options.gp.clone();
        gpo.seed = mix(spec.options.seed, lo.obs.len() as u64, 1);
        gpo.workers = spec.options.workers;
        let us: Vec<Vec<f64>> = ok.iter().map(|t| t.0.clone()).collect();
        let fs: Vec<f64> = ok.iter().map(|t| t.1).collect();
        if let Ok(gp) = Gp::fit(&us, &fs, &gpo) {
            let loo = gp.loo();
            let z: Vec<f64> = loo
                .iter()
                .zip(&fs)
                .map(|((m, s), y)| if *s > 0.0 { (y - m) / s } else { 0.0 })
                .collect();
            let rms = (z.iter().map(|v| v * v).sum::<f64>() / z.len() as f64).sqrt();
            let cover = z.iter().filter(|v| v.abs() <= 1.96).count() as f64 / z.len() as f64;
            metrics.push(Quantity::new("loo_rms_z", rms, "1"));
            metrics.push(Quantity::new("loo_cover95", cover, "1"));
            let text = json!({
                "kernel": format!("{:?}", gp.kernel()),
                "length_scales": gp.length_scales().iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                "nugget": gp.nugget().to_bits(), "jitter": gp.jitter().to_bits(), "n": gp.n(),
            })
            .to_string();
            fitted = Some(Identity {
                id: "rcfd.design.surrogate.gp".into(),
                version: "1".into(),
                hash: format!("fnv1a64:{:016x}", fnv(text.as_bytes())),
            });
            if ledger.is_some() {
                parents = ok.iter().map(|t| entry_ref(t.2, &read[t.2])).collect();
            }
        }
    }
    let summary = Entry {
        kind: "surrogate_summary".into(),
        study: spec.study.clone(),
        variant: format!("{}/gp_ei/summary", spec.study),
        evaluator: "rcfd.design.bayes".into(),
        evaluator_version: "1".into(),
        fidelity: model.fidelity(),
        verdict: Verdict::Ok,
        metrics,
        seed: Some(spec.options.seed),
        recorded_at_ms: spec.clock_ms.unwrap_or(0),
        provenance: json!({"driver": "gp_ei", "objective": spec.objective, "model": model.id(), "model_version": model.version()}),
        fitted,
        parents,
        ..Default::default()
    };
    if let Some(l) = ledger {
        l.append(std::slice::from_ref(&summary))?;
    }
    Ok(BayesResult {
        entries: new_entries,
        picks: picks_all,
        best,
        best_value: best_value.map(|v| sign * v),
        failed,
        summary,
    })
}

fn num(v: f64) -> Value {
    serde_json::Number::from_f64(v).map_or(Value::Null, Value::Number)
}
