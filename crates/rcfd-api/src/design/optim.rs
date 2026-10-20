//! Bound-aware optimisers. Gradient-free and seeded (ADR 0067): Nelder-Mead (Nelder and Mead 1965) with clamping into the box
//! and restarts, and differential evolution (Storn and Price 1997, DE/rand/1/bin). Constraints use Deb's feasibility rules
//! (Deb 2000): feasible beats infeasible, two infeasible points compare by total violation, two feasible by objective. One
//! gradient method (ADR 0091): projected steepest descent with Armijo backtracking on a caller-supplied gradient.
//!
//! The optimisers are deterministic functions of (bounds, options, seed, evaluation results). They ask for evaluations in
//! batches through a closure, so a caller can evaluate a batch concurrently, answer from a store (resume) or stop the run
//! (`Stop`), and the same code serves "propose the next candidates" by replaying against a store.

use super::rng::Rng;

/// Why an optimiser run ended early without a result.
#[derive(Debug, Clone, PartialEq)]
pub enum Stop {
    /// The caller asked to stop between batches (evaluation allowance used up).
    Interrupted,
    /// The batch contains points that have no stored evaluation (propose mode); the points are listed.
    Missing(Vec<Vec<f64>>),
    /// The evaluator failed in a way that cannot be turned into a fitness.
    Error(String),
}

/// Fitness of a point: objective value and total constraint violation (0 = feasible).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fitness {
    /// Objective (minimised); `+inf` for a failed evaluation.
    pub f: f64,
    /// Total constraint violation, `>= 0`; `+inf` for a failed evaluation.
    pub violation: f64,
}

impl Fitness {
    /// Build a fitness; NaN becomes `+inf`, a negative violation 0.
    pub fn new(f: f64, violation: f64) -> Self {
        Fitness {
            f: if f.is_nan() { f64::INFINITY } else { f },
            violation: if violation.is_nan() {
                f64::INFINITY
            } else {
                violation.max(0.0)
            },
        }
    }

    /// The fitness of a failed evaluation.
    pub const FAILED: Fitness = Fitness {
        f: f64::INFINITY,
        violation: f64::INFINITY,
    };

    /// Whether the point satisfies all constraints.
    pub fn feasible(&self) -> bool {
        self.violation <= 0.0
    }

    /// Deb's rules: strictly better than `o`.
    pub fn better(&self, o: &Fitness) -> bool {
        match (self.feasible(), o.feasible()) {
            (true, true) => self.f < o.f,
            (true, false) => true,
            (false, true) => false,
            (false, false) => self.violation < o.violation,
        }
    }
}

/// Batch evaluation callback of the optimisers.
pub type Eval<'a> = dyn FnMut(&[Vec<f64>]) -> Result<Vec<Fitness>, Stop> + 'a;

/// Result of an optimiser run.
#[derive(Debug, Clone)]
pub struct OptResult {
    /// Best point found.
    pub x: Vec<f64>,
    /// Its fitness.
    pub fitness: Fitness,
    /// Evaluations requested.
    pub evaluations: usize,
    /// Whether the run ended by its own convergence test (not by the budget).
    pub converged: bool,
}

/// Box constraints (lo, hi) per parameter.
pub type Bounds = [(f64, f64)];

fn clamp_into(x: &mut [f64], b: &Bounds) {
    for (v, &(lo, hi)) in x.iter_mut().zip(b) {
        *v = v.clamp(lo, hi);
    }
}

/// A uniformly random point in the box.
pub fn random_in_box(b: &Bounds, rng: &mut Rng) -> Vec<f64> {
    b.iter().map(|&(lo, hi)| rng.range(lo, hi)).collect()
}

/// Options of [`nelder_mead`].
#[derive(Debug, Clone)]
pub struct NmOptions {
    /// Evaluation budget.
    pub max_evals: usize,
    /// Restarts after the simplex has converged (each with half the previous initial simplex size).
    pub restarts: usize,
    /// Initial simplex edge as a fraction of the range of each parameter.
    pub initial_step: f64,
    /// Convergence: largest vertex distance to the best vertex relative to the range.
    pub xtol: f64,
    /// Start point (default: random in the box from `seed`).
    pub x0: Option<Vec<f64>>,
    /// Seed (start point and restart orientations).
    pub seed: u64,
}

impl Default for NmOptions {
    fn default() -> Self {
        NmOptions {
            max_evals: 200,
            restarts: 2,
            initial_step: 0.05,
            xtol: 1e-9,
            x0: None,
            seed: 1,
        }
    }
}

type Vertex = (Vec<f64>, Fitness);

fn one(eval: &mut Eval<'_>, x: Vec<f64>, evals: &mut usize) -> Result<Vertex, Stop> {
    let f = eval(std::slice::from_ref(&x))?;
    *evals += 1;
    Ok((x, f[0]))
}

fn sort_vertices(v: &mut [Vertex]) {
    v.sort_by(|a, b| {
        if a.1.better(&b.1) {
            std::cmp::Ordering::Less
        } else if b.1.better(&a.1) {
            std::cmp::Ordering::Greater
        } else {
            std::cmp::Ordering::Equal
        }
    });
}

/// Nelder-Mead with clamping into the box and restarts. Coefficients 1, 2, 0.5, 0.5.
pub fn nelder_mead(b: &Bounds, opts: &NmOptions, eval: &mut Eval<'_>) -> Result<OptResult, Stop> {
    let d = b.len();
    if d == 0 {
        return Err(Stop::Error("no parameters".into()));
    }
    let range: Vec<f64> = b.iter().map(|&(lo, hi)| hi - lo).collect();
    let mut rng = Rng::new(opts.seed);
    let mut x0 = match &opts.x0 {
        Some(x) => x.clone(),
        None => random_in_box(b, &mut rng),
    };
    clamp_into(&mut x0, b);
    let mut evals = 0usize;
    let mut best: Option<Vertex> = None;
    let mut size = opts.initial_step;
    let mut converged = false;
    for restart in 0..=opts.restarts {
        let centre = best.as_ref().map_or_else(|| x0.clone(), |v| v.0.clone());
        let mut pts = vec![centre.clone()];
        for i in 0..d {
            let mut p = centre.clone();
            let mut step = size * range[i];
            if restart > 0 && rng.uniform() < 0.5 {
                step = -step;
            }
            if p[i] + step > b[i].1 || p[i] + step < b[i].0 {
                step = -step;
            }
            p[i] = (p[i] + step).clamp(b[i].0, b[i].1);
            pts.push(p);
        }
        if evals + pts.len() > opts.max_evals {
            break;
        }
        let fits = eval(&pts)?;
        evals += pts.len();
        let mut s: Vec<Vertex> = pts.into_iter().zip(fits).collect();
        let mut budget_hit = false;
        let mut conv_this = false;
        loop {
            sort_vertices(&mut s);
            let diam = (1..=d)
                .map(|j| {
                    (0..d)
                        .map(|i| (s[j].0[i] - s[0].0[i]).abs() / range[i])
                        .fold(0.0, f64::max)
                })
                .fold(0.0, f64::max);
            if diam <= opts.xtol {
                conv_this = true;
                break;
            }
            if evals >= opts.max_evals {
                budget_hit = true;
                break;
            }
            let mut c = vec![0.0; d];
            for v in &s[..d] {
                for i in 0..d {
                    c[i] += v.0[i] / d as f64;
                }
            }
            let along = |t: f64| -> Vec<f64> {
                let mut p: Vec<f64> = (0..d).map(|i| c[i] + t * (s[d].0[i] - c[i])).collect();
                clamp_into(&mut p, b);
                p
            };
            let xr = along(-1.0);
            let fr = one(eval, xr, &mut evals)?;
            if fr.1.better(&s[d - 1].1) && !fr.1.better(&s[0].1) {
                s[d] = fr;
                continue;
            }
            if fr.1.better(&s[0].1) {
                if evals >= opts.max_evals {
                    s[d] = fr;
                    budget_hit = true;
                    break;
                }
                let xe = along(-2.0);
                let fe = one(eval, xe, &mut evals)?;
                s[d] = if fe.1.better(&fr.1) { fe } else { fr };
                continue;
            }
            // contraction
            let outside = fr.1.better(&s[d].1);
            if evals >= opts.max_evals {
                budget_hit = true;
                break;
            }
            let xc = if outside { along(-0.5) } else { along(0.5) };
            let fc = one(eval, xc, &mut evals)?;
            let accept = if outside {
                !fr.1.better(&fc.1)
            } else {
                fc.1.better(&s[d].1)
            };
            if accept {
                s[d] = fc;
                continue;
            }
            // shrink towards the best vertex
            if evals + d > opts.max_evals {
                budget_hit = true;
                break;
            }
            let b0 = s[0].0.clone();
            let pts: Vec<Vec<f64>> = (1..=d)
                .map(|j| {
                    let mut p: Vec<f64> = (0..d).map(|i| b0[i] + 0.5 * (s[j].0[i] - b0[i])).collect();
                    clamp_into(&mut p, b);
                    p
                })
                .collect();
            let fits = eval(&pts)?;
            evals += d;
            for (j, (p, f)) in pts.into_iter().zip(fits).enumerate() {
                s[j + 1] = (p, f);
            }
        }
        converged = conv_this;
        sort_vertices(&mut s);
        let top = s[0].clone();
        if best
            .as_ref()
            .map_or(true, |bv| top.1.better(&bv.1) || !bv.1.better(&top.1))
        {
            best = Some(top);
        }
        size *= 0.5;
        if budget_hit || restart == opts.restarts {
            break;
        }
    }
    let (x, fitness) =
        best.ok_or_else(|| Stop::Error("the evaluation budget is smaller than the initial simplex".into()))?;
    Ok(OptResult {
        x,
        fitness,
        evaluations: evals,
        converged,
    })
}

/// Options of [`differential_evolution`].
#[derive(Debug, Clone)]
pub struct DeOptions {
    /// Evaluation budget (whole generations only).
    pub max_evals: usize,
    /// Population size (default `max(10 d, 20)`).
    pub pop: Option<usize>,
    /// Differential weight F.
    pub f: f64,
    /// Crossover rate CR.
    pub cr: f64,
    /// Seed.
    pub seed: u64,
    /// A point injected as the first member of the initial population.
    pub x0: Option<Vec<f64>>,
}

impl Default for DeOptions {
    fn default() -> Self {
        DeOptions {
            max_evals: 2000,
            pop: None,
            f: 0.5,
            cr: 0.9,
            seed: 1,
            x0: None,
        }
    }
}

fn reflect(v: f64, lo: f64, hi: f64) -> f64 {
    let r = if v < lo {
        lo + (lo - v)
    } else if v > hi {
        hi - (v - hi)
    } else {
        v
    };
    r.clamp(lo, hi)
}

/// Differential evolution, DE/rand/1/bin, generation-synchronous; bounds by reflection.
pub fn differential_evolution(b: &Bounds, opts: &DeOptions, eval: &mut Eval<'_>) -> Result<OptResult, Stop> {
    let d = b.len();
    if d == 0 {
        return Err(Stop::Error("no parameters".into()));
    }
    let np = opts.pop.unwrap_or_else(|| (10 * d).max(20)).max(4);
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
    let mut fit = eval(&pop)?;
    let mut evals = np;
    let mut converged = false;
    while evals + np <= opts.max_evals {
        let mut trials = Vec::with_capacity(np);
        for i in 0..np {
            let mut pick = [0usize; 3];
            for k in 0..3 {
                loop {
                    let r = rng.below(np);
                    if r != i && !pick[..k].contains(&r) {
                        pick[k] = r;
                        break;
                    }
                }
            }
            let jrand = rng.below(d);
            let mut t = pop[i].clone();
            for j in 0..d {
                if rng.uniform() < opts.cr || j == jrand {
                    let v = pop[pick[0]][j] + opts.f * (pop[pick[1]][j] - pop[pick[2]][j]);
                    t[j] = reflect(v, b[j].0, b[j].1);
                }
            }
            trials.push(t);
        }
        let ft = eval(&trials)?;
        evals += np;
        for (i, (t, f)) in trials.into_iter().zip(ft).enumerate() {
            if !fit[i].better(&f) {
                pop[i] = t;
                fit[i] = f;
            }
        }
        if fit.iter().all(Fitness::feasible) {
            let (lo, hi) = fit
                .iter()
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), f| (l.min(f.f), h.max(f.f)));
            if hi - lo <= 1e-14 {
                converged = true;
                break;
            }
        }
    }
    let mut bi = 0;
    for i in 1..np {
        if fit[i].better(&fit[bi]) {
            bi = i;
        }
    }
    Ok(OptResult {
        x: pop[bi].clone(),
        fitness: fit[bi],
        evaluations: evals,
        converged,
    })
}

/// Options of [`projected_gradient`].
#[derive(Debug, Clone)]
pub struct PgOptions {
    /// Gradient iterations (accepted steps).
    pub max_iters: usize,
    /// First trial step of every iteration: the parameter with the largest gradient relative to its range moves by this
    /// fraction of its range.
    pub step: f64,
    /// Armijo sufficient-decrease constant.
    pub armijo: f64,
    /// Step halvings before an iteration gives up.
    pub max_halvings: usize,
    /// Start point (clamped into the box).
    pub x0: Vec<f64>,
}

/// Value callback of [`projected_gradient`]: `(f, gradient)` at `x`; the gradient may be empty when the flag is `false`.
pub type GradEval<'a> = dyn FnMut(&[f64], bool) -> Result<(f64, Vec<f64>), Stop> + 'a;

/// Bound-constrained steepest descent (ADR 0091): the step `x - t g` is projected onto the box and accepted by Armijo
/// backtracking along the projection arc. The gradient comes from the caller (for example the adjoint of `design::adjoint`);
/// trial points ask for the value only. Converged when the projected step no longer decreases the linear model.
pub fn projected_gradient(b: &Bounds, opts: &PgOptions, eval: &mut GradEval<'_>) -> Result<OptResult, Stop> {
    let range: Vec<f64> = b.iter().map(|&(lo, hi)| hi - lo).collect();
    let mut x = opts.x0.clone();
    if x.len() != b.len() {
        return Err(Stop::Error("x0 does not match the bounds".into()));
    }
    clamp_into(&mut x, b);
    let (mut f, mut g) = eval(&x, true)?;
    let mut evals = 1;
    let mut converged = false;
    for _ in 0..opts.max_iters {
        if g.len() != x.len() {
            return Err(Stop::Error("gradient size does not match the bounds".into()));
        }
        let gmax = g.iter().zip(&range).map(|(gi, r)| gi.abs() / r).fold(0.0, f64::max);
        if gmax.is_nan() || gmax <= 0.0 {
            converged = true;
            break;
        }
        let mut t = opts.step / gmax;
        let mut accepted = false;
        for _ in 0..=opts.max_halvings {
            let mut xt: Vec<f64> = x.iter().zip(&g).map(|(xi, gi)| xi - t * gi).collect();
            clamp_into(&mut xt, b);
            let dec: f64 = g.iter().zip(x.iter().zip(&xt)).map(|(gi, (a, c))| gi * (a - c)).sum();
            if dec <= 0.0 {
                converged = true;
                break;
            }
            let (ft, _) = eval(&xt, false)?;
            evals += 1;
            if ft <= f - opts.armijo * dec {
                x = xt;
                accepted = true;
                break;
            }
            t *= 0.5;
        }
        if !accepted {
            break;
        }
        (f, g) = eval(&x, true)?;
        evals += 1;
    }
    Ok(OptResult {
        x,
        fitness: Fitness::new(f, 0.0),
        evaluations: evals,
        converged,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rosenbrock(x: &[f64]) -> f64 {
        100.0 * (x[1] - x[0] * x[0]).powi(2) + (1.0 - x[0]).powi(2)
    }

    fn rastrigin(x: &[f64]) -> f64 {
        10.0 * x.len() as f64
            + x.iter()
                .map(|v| v * v - 10.0 * (2.0 * std::f64::consts::PI * v).cos())
                .sum::<f64>()
    }

    /// Recording evaluator: history of all requested points and fitnesses.
    fn recorder<'a>(
        f: impl Fn(&[f64]) -> Fitness + 'a,
        hist: &'a mut Vec<(Vec<f64>, Fitness)>,
        bounds: &'a [(f64, f64)],
    ) -> impl FnMut(&[Vec<f64>]) -> Result<Vec<Fitness>, Stop> + 'a {
        move |xs: &[Vec<f64>]| {
            let mut out = Vec::new();
            for x in xs {
                for (v, &(lo, hi)) in x.iter().zip(bounds) {
                    assert!(*v >= lo && *v <= hi, "O5: {v} outside [{lo}, {hi}]");
                }
                let fit = f(x);
                hist.push((x.clone(), fit));
                out.push(fit);
            }
            Ok(out)
        }
    }

    #[test]
    fn o1_nelder_mead_finds_the_rosenbrock_minimum() {
        let bounds = [(-2.0, 2.0), (-2.0, 2.0)];
        let mut ok = 0;
        for seed in 1..=20u64 {
            let mut hist = Vec::new();
            let mut ev = recorder(|x| Fitness::new(rosenbrock(x), 0.0), &mut hist, &bounds);
            let opts = NmOptions {
                max_evals: 800,
                restarts: 3,
                seed,
                ..Default::default()
            };
            let r = nelder_mead(&bounds, &opts, &mut ev).unwrap();
            assert!(r.evaluations <= 800);
            let good = r.fitness.f <= 1e-6 && (r.x[0] - 1.0).abs() <= 1e-3 && (r.x[1] - 1.0).abs() <= 1e-3;
            eprintln!(
                "seed {seed}: f = {:.3e} after {} evaluations",
                r.fitness.f, r.evaluations
            );
            ok += usize::from(good);
        }
        assert!(ok >= 18, "{ok} of 20 seeds reached the minimum (limit 18)");
    }

    #[test]
    fn o2_differential_evolution_on_rastrigin() {
        let bounds = [(-5.12, 5.12); 4];
        let (mut exact, mut basin) = (0, 0);
        for seed in 1..=20u64 {
            let mut hist = Vec::new();
            let mut ev = recorder(|x| Fitness::new(rastrigin(x), 0.0), &mut hist, &bounds);
            let opts = DeOptions {
                max_evals: 20000,
                pop: Some(40),
                seed,
                ..Default::default()
            };
            let r = differential_evolution(&bounds, &opts, &mut ev).unwrap();
            assert!(r.evaluations <= 20000);
            eprintln!(
                "seed {seed}: f = {:.3e} after {} evaluations",
                r.fitness.f, r.evaluations
            );
            exact += usize::from(r.fitness.f <= 1e-6);
            basin += usize::from(r.fitness.f <= 1.0);
        }
        assert!(exact >= 12, "{exact} of 20 seeds reached f <= 1e-6 (limit 12)");
        assert!(basin >= 19, "{basin} of 20 seeds reached f <= 1 (limit 19)");
    }

    fn quad(x: &[f64]) -> Fitness {
        Fitness::new(
            (x[0] - 3.0).powi(2) + (x[1] - 2.0).powi(2),
            (x[0] + x[1] - 4.0).max(0.0),
        )
    }

    #[test]
    fn o3_constrained_quadratic_with_both_optimisers() {
        let bounds = [(0.0, 5.0), (0.0, 5.0)];
        for seed in 1..=20u64 {
            let mut hist = Vec::new();
            let mut ev = recorder(quad, &mut hist, &bounds);
            let nm = nelder_mead(
                &bounds,
                &NmOptions {
                    max_evals: 600,
                    restarts: 3,
                    seed,
                    ..Default::default()
                },
                &mut ev,
            )
            .unwrap();
            assert!(
                nm.fitness.f - 0.5 <= 1e-3 && nm.fitness.violation <= 1e-6,
                "NM seed {seed}: f {} violation {}",
                nm.fitness.f,
                nm.fitness.violation
            );
            let mut hist = Vec::new();
            let mut ev = recorder(quad, &mut hist, &bounds);
            let de = differential_evolution(
                &bounds,
                &DeOptions {
                    max_evals: 3000,
                    seed,
                    ..Default::default()
                },
                &mut ev,
            )
            .unwrap();
            assert!(
                (de.fitness.f - 0.5).abs() <= 1e-4 && de.fitness.violation <= 1e-6,
                "DE seed {seed}: f {} violation {}",
                de.fitness.f,
                de.fitness.violation
            );
        }
    }

    #[test]
    fn o4_same_seed_same_history_bit_for_bit() {
        let bounds = [(-2.0, 2.0), (-2.0, 2.0)];
        let run_nm = |seed: u64| {
            let mut hist = Vec::new();
            let mut ev = recorder(|x| Fitness::new(rosenbrock(x), 0.0), &mut hist, &bounds);
            let o = NmOptions {
                max_evals: 300,
                seed,
                ..Default::default()
            };
            nelder_mead(&bounds, &o, &mut ev).unwrap();
            drop(ev);
            hist
        };
        let run_de = |seed: u64| {
            let mut hist = Vec::new();
            let mut ev = recorder(|x| Fitness::new(rosenbrock(x), 0.0), &mut hist, &bounds);
            let o = DeOptions {
                max_evals: 400,
                seed,
                ..Default::default()
            };
            differential_evolution(&bounds, &o, &mut ev).unwrap();
            drop(ev);
            hist
        };
        let bits = |h: &[(Vec<f64>, Fitness)]| {
            h.iter()
                .map(|(x, f)| (x.iter().map(|v| v.to_bits()).collect::<Vec<_>>(), f.f.to_bits()))
                .collect::<Vec<_>>()
        };
        assert_eq!(bits(&run_nm(3)), bits(&run_nm(3)));
        assert_eq!(bits(&run_de(3)), bits(&run_de(3)));
        assert_ne!(bits(&run_nm(3)), bits(&run_nm(4)));
        assert_ne!(bits(&run_de(3)), bits(&run_de(4)));
    }

    #[test]
    fn failed_evaluations_are_worst_and_budget_is_respected() {
        let bounds = [(-1.0, 1.0), (-1.0, 1.0)];
        // the half plane x > 0.5 "fails"
        let mut hist = Vec::new();
        let mut ev = recorder(
            |x| {
                if x[0] > 0.5 {
                    Fitness::FAILED
                } else {
                    Fitness::new(x[0] * x[0] + x[1] * x[1], 0.0)
                }
            },
            &mut hist,
            &bounds,
        );
        let r = nelder_mead(
            &bounds,
            &NmOptions {
                max_evals: 100,
                x0: Some(vec![0.4, 0.4]),
                ..Default::default()
            },
            &mut ev,
        )
        .unwrap();
        drop(ev);
        assert!(r.evaluations <= 100 && hist.len() <= 100);
        assert!(r.fitness.f.is_finite());
    }

    #[test]
    fn stop_propagates() {
        let bounds = [(0.0, 1.0)];
        let mut n = 0;
        let mut ev = |xs: &[Vec<f64>]| -> Result<Vec<Fitness>, Stop> {
            n += xs.len();
            if n > 5 {
                Err(Stop::Interrupted)
            } else {
                Ok(xs.iter().map(|x| Fitness::new(x[0], 0.0)).collect())
            }
        };
        assert_eq!(
            nelder_mead(&bounds, &NmOptions::default(), &mut ev).unwrap_err(),
            Stop::Interrupted
        );
    }

    #[test]
    fn projected_gradient_reaches_a_bound_constrained_minimum() {
        // (x - 3)^2 + 2 (y + 1)^2 on [0, 2] x [-5, 5]: minimum (2, -1) with x on its bound
        let bounds = [(0.0, 2.0), (-5.0, 5.0)];
        let mut calls = 0;
        let mut ev = |x: &[f64], grad: bool| -> Result<(f64, Vec<f64>), Stop> {
            calls += 1;
            let f = (x[0] - 3.0).powi(2) + 2.0 * (x[1] + 1.0).powi(2);
            let g = if grad {
                vec![2.0 * (x[0] - 3.0), 4.0 * (x[1] + 1.0)]
            } else {
                vec![]
            };
            Ok((f, g))
        };
        let o = PgOptions {
            max_iters: 100,
            step: 0.1,
            armijo: 1e-4,
            max_halvings: 30,
            x0: vec![0.5, 4.0],
        };
        let r = projected_gradient(&bounds, &o, &mut ev).unwrap();
        assert!((r.x[0] - 2.0).abs() < 1e-12 && (r.x[1] + 1.0).abs() < 1e-6, "{:?}", r.x);
        assert_eq!(r.evaluations, calls);
    }
}
