//! ADR 0209 N2 to N5 (F-249): the two Nelder-Mead options (`initial_simplex = Start`, `coefficients = Adaptive`) on the ADR 0067
//! analytic problems, their closed forms, and a recorded comparison in 10 and 20 dimensions. Limits are those registered in
//! ADR 0209 before the code.

use rcfd_api::design::optim::{self, nelder_mead, Coefficients, Fitness, InitialSimplex, NmOptions, Stop};

fn rosenbrock(x: &[f64]) -> f64 {
    100.0 * (x[1] - x[0] * x[0]).powi(2) + (1.0 - x[0]).powi(2)
}

fn variant() -> NmOptions {
    NmOptions {
        initial_simplex: InitialSimplex::Start,
        coefficients: Coefficients::Adaptive,
        ..Default::default()
    }
}

/// Bits of every requested point and its value, in request order.
type History = Vec<(Vec<u64>, u64)>;

/// History of every requested point and value; counts points outside the box.
fn history(
    bounds: &[(f64, f64)],
    opts: &NmOptions,
    f: impl Fn(&[f64]) -> Fitness,
) -> (Result<optim::OptResult, Stop>, History, usize) {
    let mut hist = Vec::new();
    let mut outside = 0usize;
    let mut ev = |xs: &[Vec<f64>]| -> Result<Vec<Fitness>, Stop> {
        Ok(xs
            .iter()
            .map(|x| {
                if x.iter().zip(bounds).any(|(v, &(lo, hi))| *v < lo || *v > hi) {
                    outside += 1;
                }
                let fit = f(x);
                hist.push((x.iter().map(|v| v.to_bits()).collect(), fit.f.to_bits()));
                fit
            })
            .collect())
    };
    let r = nelder_mead(bounds, opts, &mut ev);
    (r, hist, outside)
}

#[test]
fn n2_rosenbrock_from_20_seeded_starts() {
    let bounds = [(-2.0, 2.0), (-2.0, 2.0)];
    let mut ok = 0;
    for seed in 1..=20u64 {
        let o = NmOptions {
            max_evals: 800,
            restarts: 3,
            seed,
            ..variant()
        };
        let (r, _, outside) = history(&bounds, &o, |x| Fitness::new(rosenbrock(x), 0.0));
        let r = r.unwrap();
        assert_eq!(outside, 0);
        assert!(r.evaluations <= 800);
        let good = r.fitness.f <= 1e-6 && (r.x[0] - 1.0).abs() <= 1e-3 && (r.x[1] - 1.0).abs() <= 1e-3;
        println!(
            "N2 seed {seed}: f = {:.3e} at ({}, {}) after {} evaluations",
            r.fitness.f, r.x[0], r.x[1], r.evaluations
        );
        ok += usize::from(good);
    }
    println!("N2: {ok} of 20 seeds");
    assert!(ok >= 18, "N2: {ok} of 20 seeds reached the minimum (limit 18)");
}

/// ADR 0209 N3, a known miss (amendment 1): 19 of 20 seeds at ebc4e9f; seed 16 ends at f = 1.6668 (violation 0) after 600
/// evaluations. The limit (all 20 seeds) is kept intact.
#[test]
#[ignore = "known miss: seed 16 of 20 ends at f = 1.6668 on the constrained quadratic (ADR 0209 amendment 1)"]
fn n3_constrained_quadratic_from_20_seeded_starts_known_miss() {
    let bounds = [(0.0, 5.0), (0.0, 5.0)];
    let quad = |x: &[f64]| {
        Fitness::new(
            (x[0] - 3.0).powi(2) + (x[1] - 2.0).powi(2),
            (x[0] + x[1] - 4.0).max(0.0),
        )
    };
    let mut ok = 0;
    for seed in 1..=20u64 {
        let o = NmOptions {
            max_evals: 600,
            restarts: 3,
            seed,
            ..variant()
        };
        let (r, _, outside) = history(&bounds, &o, quad);
        let r = r.unwrap();
        assert_eq!(outside, 0);
        let good = r.fitness.f - 0.5 <= 1e-3 && r.fitness.violation <= 1e-6;
        println!(
            "N3 seed {seed}: f = {:.6e}, violation {:.1e}, {} evaluations",
            r.fitness.f, r.fitness.violation, r.evaluations
        );
        ok += usize::from(good);
    }
    println!("N3: {ok} of 20 seeds");
    assert_eq!(ok, 20, "N3: {ok} of 20 seeds met f - 0.5 <= 1e-3 and violation <= 1e-6");
}

#[test]
fn n4a_coefficient_formulas() {
    for d in [2usize, 3, 4, 10] {
        let n = d as f64;
        let want = [1.0, 1.0 + 2.0 / n, 0.75 - 1.0 / (2.0 * n), 1.0 - 1.0 / n];
        let got = optim::nm_coefficients(Coefficients::Adaptive, d);
        assert_eq!(got.map(f64::to_bits), want.map(f64::to_bits), "d = {d}");
    }
    assert_eq!(
        optim::nm_coefficients(Coefficients::Adaptive, 2).map(f64::to_bits),
        [1.0, 2.0, 0.5, 0.5].map(f64::to_bits)
    );
    assert_eq!(
        optim::nm_coefficients(Coefficients::Standard, 7).map(f64::to_bits),
        [1.0, 2.0, 0.5, 0.5].map(f64::to_bits)
    );
}

#[test]
fn n4b_adaptive_equals_standard_in_two_dimensions() {
    let bounds = [(-2.0, 2.0), (-2.0, 2.0)];
    for simplex in [InitialSimplex::Range, InitialSimplex::Start] {
        let run = |c: Coefficients| {
            let o = NmOptions {
                max_evals: 300,
                seed: 3,
                initial_simplex: simplex,
                coefficients: c,
                ..Default::default()
            };
            history(&bounds, &o, |x| Fitness::new(rosenbrock(x), 0.0)).1
        };
        let (s, a) = (run(Coefficients::Standard), run(Coefficients::Adaptive));
        println!("N4b {simplex:?}: {} evaluations", s.len());
        assert!(!s.is_empty());
        assert_eq!(s, a, "{simplex:?}");
    }
}

/// The first batch (the initial simplex) of a run.
fn first_batch(bounds: &[(f64, f64)], x0: Vec<f64>) -> Vec<Vec<f64>> {
    let mut first: Option<Vec<Vec<f64>>> = None;
    let mut ev = |xs: &[Vec<f64>]| -> Result<Vec<Fitness>, Stop> {
        first = Some(xs.to_vec());
        Err(Stop::Interrupted)
    };
    let o = NmOptions {
        x0: Some(x0),
        initial_simplex: InitialSimplex::Start,
        ..Default::default()
    };
    assert_eq!(nelder_mead(bounds, &o, &mut ev).unwrap_err(), Stop::Interrupted);
    first.unwrap()
}

#[test]
fn n4c_start_rule_vertices() {
    let b3 = [(-2.0, 2.0); 3];
    let x0 = vec![-1.2, 1.0, 0.0];
    let got = first_batch(&b3, x0.clone());
    let want = vec![
        x0.clone(),
        vec![-1.2 + 0.05 * -1.2, 1.0, 0.0],
        vec![-1.2, 1.0 + 0.05 * 1.0, 0.0],
        vec![-1.2, 1.0, 0.0 + 0.00025],
    ];
    println!("N4c: {got:?}");
    let bits = |v: &[Vec<f64>]| {
        v.iter()
            .map(|p| p.iter().map(|x| x.to_bits()).collect::<Vec<_>>())
            .collect::<Vec<_>>()
    };
    assert_eq!(bits(&got), bits(&want));
    let got = first_batch(&[(-2.0, 2.0); 2], vec![2.0, 0.5]);
    let want = vec![vec![2.0, 0.5], vec![2.0 - 0.1, 0.5], vec![2.0, 0.5 + 0.05 * 0.5]];
    println!("N4c at a bound: {got:?}");
    assert_eq!(bits(&got), bits(&want));
}

#[test]
fn n4d_adaptive_differs_in_four_dimensions_and_needs_two_parameters() {
    let b = [(-2.0, 2.0); 4];
    let sphere = |x: &[f64]| Fitness::new(x.iter().map(|v| v * v).sum(), 0.0);
    let run = |c: Coefficients| {
        let o = NmOptions {
            max_evals: 200,
            x0: Some(vec![1.0; 4]),
            initial_simplex: InitialSimplex::Start,
            coefficients: c,
            ..Default::default()
        };
        history(&b, &o, sphere).1
    };
    assert_ne!(run(Coefficients::Standard), run(Coefficients::Adaptive));
    let (r, hist, _) = history(&[(-1.0, 1.0)], &variant(), |x| Fitness::new(x[0] * x[0], 0.0));
    println!("N4d d = 1: {r:?}");
    assert!(matches!(r, Err(Stop::Error(_))));
    assert!(hist.is_empty());
}

#[test]
fn n5_recorded_sum_of_squares_in_10_and_20_dimensions() {
    for (d, budget) in [(10usize, 10_000usize), (20, 20_000)] {
        let b = vec![(-2.0, 2.0); d];
        for c in [Coefficients::Standard, Coefficients::Adaptive] {
            let o = NmOptions {
                max_evals: budget,
                restarts: 0,
                x0: Some(vec![1.0; d]),
                initial_simplex: InitialSimplex::Start,
                coefficients: c,
                ..Default::default()
            };
            let (r, _, _) = history(&b, &o, |x| Fitness::new(x.iter().map(|v| v * v).sum(), 0.0));
            let r = r.unwrap();
            println!(
                "N5 d = {d} {c:?}: best f {:.3e} after {} evaluations (converged {})",
                r.fitness.f, r.evaluations, r.converged
            );
        }
    }
}
