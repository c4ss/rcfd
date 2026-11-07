//! NSGA-II (Deb, Pratap, Agarwal and Meyarivan 2002), the multi-objective optimiser of the design loop (ADR 0209 D1).
//!
//! Elitist and generation-synchronous: each generation is one batch of `pop` evaluations, asked for through a closure as the
//! other optimisers do (so evaluation, resume and proposal from a store work the same way). Parents are chosen by binary
//! tournaments with the crowded comparison (rank, then crowding distance), crossed by simulated binary crossover (Deb and
//! Agrawal 1995) and mutated by polynomial mutation (Deb and Goyal 1996), both with distribution index 20; the next population
//! is filled front by front from parents and children, the last front by crowding distance. Constraints use constrained
//! domination (`pareto::dominates`, Deb 2002). The constants are those of the experiments of Deb et al. (2002); they and the
//! operator formulas are from memory, not from a reading of the papers (ADR 0209 D1, question 5).
//!
//! The run is a deterministic function of (bounds, options, evaluation results); the random draws come in a fixed order
//! (ADR 0209 D1 step 3).

use super::optim::{clamp_into, random_in_box, Bounds, Stop};
use super::pareto;
use super::rng::Rng;

/// Crossover probability of a pair.
pub const P_CROSSOVER: f64 = 0.9;
/// Distribution index of the simulated binary crossover.
pub const ETA_C: f64 = 20.0;
/// Distribution index of the polynomial mutation.
pub const ETA_M: f64 = 20.0;

/// Objective values (minimised) and total constraint violation of one point.
#[derive(Debug, Clone, PartialEq)]
pub struct MoFitness {
    /// Objectives, minimised; `+inf` for a failed evaluation.
    pub objectives: Vec<f64>,
    /// Total constraint violation, `>= 0` (0 = feasible); `+inf` for a failed evaluation.
    pub violation: f64,
}

impl MoFitness {
    /// Build a fitness; a NaN objective or violation becomes `+inf`, a negative violation 0.
    pub fn new(objectives: Vec<f64>, violation: f64) -> Self {
        MoFitness {
            objectives: objectives
                .into_iter()
                .map(|x| if x.is_nan() { f64::INFINITY } else { x })
                .collect(),
            violation: if violation.is_nan() {
                f64::INFINITY
            } else {
                violation.max(0.0)
            },
        }
    }

    /// The fitness of a failed evaluation with `m` objectives.
    pub fn failed(m: usize) -> Self {
        MoFitness {
            objectives: vec![f64::INFINITY; m],
            violation: f64::INFINITY,
        }
    }

    /// Whether every constraint holds and every objective is finite.
    pub fn feasible(&self) -> bool {
        self.violation <= 0.0 && self.objectives.iter().all(|x| x.is_finite())
    }
}

/// Batch evaluation callback of [`run`].
pub type MoEval<'a> = dyn FnMut(&[Vec<f64>]) -> Result<Vec<MoFitness>, Stop> + 'a;

/// Options of [`run`].
#[derive(Debug, Clone)]
pub struct Options {
    /// Evaluation budget (whole generations only).
    pub max_evals: usize,
    /// Population size (even, at least 4).
    pub pop: usize,
    /// Seed.
    pub seed: u64,
    /// A point that replaces the first member of the initial population.
    pub x0: Option<Vec<f64>>,
}

/// Result of [`run`].
#[derive(Debug, Clone)]
pub struct MoResult {
    /// The last population with its fitness.
    pub population: Vec<(Vec<f64>, MoFitness)>,
    /// Front rank of each member (0 = non-dominated) in the last selection.
    pub rank: Vec<usize>,
    /// Evaluations requested (`pop * generations`).
    pub evaluations: usize,
    /// Generations evaluated, the initial population included.
    pub generations: usize,
}

/// The spread factor of simulated binary crossover for the uniform draw `u` in [0, 1) and the distribution index `eta`.
pub fn sbx_beta(u: f64, eta: f64) -> f64 {
    let e = 1.0 / (eta + 1.0);
    if u <= 0.5 {
        (2.0 * u).powf(e)
    } else {
        (1.0 / (2.0 * (1.0 - u))).powf(e)
    }
}

/// The step (in units of the parameter range) of polynomial mutation for the uniform draw `u` in [0, 1) and the index `eta`.
pub fn poly_delta(u: f64, eta: f64) -> f64 {
    let e = 1.0 / (eta + 1.0);
    if u < 0.5 {
        (2.0 * u).powf(e) - 1.0
    } else {
        1.0 - (2.0 * (1.0 - u)).powf(e)
    }
}

/// Objective vectors with every non-finite value replaced by `f64::MAX`, so that crowding distances stay finite.
fn crowding_objs(objs: &[Vec<f64>]) -> Vec<Vec<f64>> {
    objs.iter()
        .map(|o| o.iter().map(|x| if x.is_finite() { *x } else { f64::MAX }).collect())
        .collect()
}

/// Front rank and crowding distance (within its front) of every point.
fn rank_and_crowd(objs: &[Vec<f64>], viol: &[f64]) -> (Vec<Vec<usize>>, Vec<usize>, Vec<f64>) {
    let fronts = pareto::non_dominated_sort(objs, viol);
    let cobjs = crowding_objs(objs);
    let mut rank = vec![0usize; objs.len()];
    let mut crowd = vec![0.0; objs.len()];
    for (r, f) in fronts.iter().enumerate() {
        let cd = pareto::crowding_distance(&cobjs, f);
        for (k, &i) in f.iter().enumerate() {
            rank[i] = r;
            crowd[i] = cd[k];
        }
    }
    (fronts, rank, crowd)
}

/// The survivors of a union of points: fronts whole in ascending index order while they fit, the first front that does not
/// fit by descending crowding distance (ties by ascending index), until `n` members (ADR 0209 D1 step 4).
pub fn select(objs: &[Vec<f64>], violation: &[f64], n: usize) -> Vec<usize> {
    let (fronts, _, crowd) = rank_and_crowd(objs, violation);
    survivors(&fronts, &crowd, n)
}

fn survivors(fronts: &[Vec<usize>], crowd: &[f64], n: usize) -> Vec<usize> {
    let mut out = Vec::with_capacity(n);
    for f in fronts {
        if out.len() >= n {
            break;
        }
        if out.len() + f.len() <= n {
            out.extend_from_slice(f);
            continue;
        }
        let mut order = f.clone();
        order.sort_by(|&a, &b| crowd[b].total_cmp(&crowd[a]).then(a.cmp(&b)));
        let need = n - out.len();
        out.extend_from_slice(&order[..need]);
    }
    out
}

/// Binary tournament with the crowded comparison: the lower rank wins, then the larger crowding distance, then the first drawn.
fn tournament(rng: &mut Rng, rank: &[usize], crowd: &[f64]) -> usize {
    let n = rank.len();
    let i = rng.below(n);
    let mut j = rng.below(n);
    while j == i {
        j = rng.below(n);
    }
    if rank[i] != rank[j] {
        return if rank[i] < rank[j] { i } else { j };
    }
    if crowd[j] > crowd[i] {
        j
    } else {
        i
    }
}

fn checked(f: Vec<MoFitness>, n: usize) -> Result<Vec<MoFitness>, Stop> {
    if f.len() == n {
        Ok(f)
    } else {
        Err(Stop::Error(format!(
            "the evaluator returned {} results for {n} points",
            f.len()
        )))
    }
}

fn split(f: &[MoFitness]) -> (Vec<Vec<f64>>, Vec<f64>) {
    (
        f.iter().map(|x| x.objectives.clone()).collect(),
        f.iter().map(|x| x.violation).collect(),
    )
}

/// Run NSGA-II in the box `b` (ADR 0209 D1).
pub fn run(b: &Bounds, opts: &Options, eval: &mut MoEval<'_>) -> Result<MoResult, Stop> {
    let d = b.len();
    if d == 0 {
        return Err(Stop::Error("no parameters".into()));
    }
    let np = opts.pop;
    if np < 4 || np % 2 != 0 {
        return Err(Stop::Error(format!(
            "the population must be even and at least 4, got {np}"
        )));
    }
    if opts.max_evals < np {
        return Err(Stop::Error(format!(
            "the evaluation budget {} is smaller than the population {np}",
            opts.max_evals
        )));
    }
    let mut rng = Rng::new(opts.seed);
    let mut pop: Vec<Vec<f64>> = (0..np).map(|_| random_in_box(b, &mut rng)).collect();
    if let Some(x0) = &opts.x0 {
        let mut x = x0.clone();
        clamp_into(&mut x, b);
        pop[0] = x;
    }
    let mut fit = checked(eval(&pop)?, np)?;
    let mut evals = np;
    let mut generations = 1;
    let (o, v) = split(&fit);
    let (_, mut rank, mut crowd) = rank_and_crowd(&o, &v);
    let pm = 1.0 / d as f64;
    while evals + np <= opts.max_evals {
        let mut kids: Vec<Vec<f64>> = Vec::with_capacity(np);
        for _ in 0..np / 2 {
            let a = tournament(&mut rng, &rank, &crowd);
            let c = tournament(&mut rng, &rank, &crowd);
            let (mut c1, mut c2) = (pop[a].clone(), pop[c].clone());
            if rng.uniform() < P_CROSSOVER {
                for j in 0..d {
                    let beta = sbx_beta(rng.uniform(), ETA_C);
                    let (p1, p2) = (pop[a][j], pop[c][j]);
                    c1[j] = 0.5 * ((1.0 + beta) * p1 + (1.0 - beta) * p2);
                    c2[j] = 0.5 * ((1.0 - beta) * p1 + (1.0 + beta) * p2);
                }
            }
            for child in [&mut c1, &mut c2] {
                for j in 0..d {
                    if rng.uniform() < pm {
                        child[j] += poly_delta(rng.uniform(), ETA_M) * (b[j].1 - b[j].0);
                    }
                }
                clamp_into(child, b);
            }
            kids.push(c1);
            kids.push(c2);
        }
        let kf = checked(eval(&kids)?, np)?;
        evals += np;
        generations += 1;
        let mut all_x = std::mem::take(&mut pop);
        all_x.extend(kids);
        let mut all_f = std::mem::take(&mut fit);
        all_f.extend(kf);
        let (o, v) = split(&all_f);
        let (fronts, r_all, c_all) = rank_and_crowd(&o, &v);
        let keep = survivors(&fronts, &c_all, np);
        pop = keep.iter().map(|&i| all_x[i].clone()).collect();
        fit = keep.iter().map(|&i| all_f[i].clone()).collect();
        rank = keep.iter().map(|&i| r_all[i]).collect();
        crowd = keep.iter().map(|&i| c_all[i]).collect();
    }
    Ok(MoResult {
        population: pop.into_iter().zip(fit).collect(),
        rank,
        evaluations: evals,
        generations,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn survivors_fill_fronts_then_crowding() {
        // fronts {0, 1} and {2, 3, 4}: n = 4 takes 0, 1 and the two boundary members of the second front
        let objs = vec![
            vec![0.0, 1.0],
            vec![1.0, 0.0],
            vec![0.5, 2.0],
            vec![1.0, 1.5],
            vec![2.0, 0.5],
        ];
        let s = select(&objs, &[0.0; 5], 4);
        assert_eq!(s, vec![0, 1, 2, 4]);
    }

    #[test]
    fn refusals_evaluate_nothing() {
        let mut calls = 0;
        let mut ev = |xs: &[Vec<f64>]| -> Result<Vec<MoFitness>, Stop> {
            calls += xs.len();
            Ok(xs.iter().map(|x| MoFitness::new(vec![x[0]], 0.0)).collect())
        };
        let b = [(0.0, 1.0)];
        for (pop, max_evals) in [(7, 100), (2, 100), (20, 10)] {
            let o = Options {
                max_evals,
                pop,
                seed: 1,
                x0: None,
            };
            assert!(matches!(run(&b, &o, &mut ev), Err(Stop::Error(_))));
        }
        let o = Options {
            max_evals: 100,
            pop: 10,
            seed: 1,
            x0: None,
        };
        assert!(matches!(run(&[], &o, &mut ev), Err(Stop::Error(_))));
        assert_eq!(calls, 0);
    }
}
