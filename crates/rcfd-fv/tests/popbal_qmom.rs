//! Quadrature method of moments (F-228, ADR 0041): checks Q1-Q8. Limits are those registered in the ADR.

use rcfd_fv::popbal::{
    invert_moments, BinGrid, BinModel, BreakRate, Breakup, CoalKernel, Daughters, MomentModel, PopBalError,
};

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs()
}

fn moments_of(w: &[f64], x: &[f64], n: usize) -> Vec<f64> {
    (0..n)
        .map(|k| w.iter().zip(x).map(|(w, x)| w * x.powi(k as i32)).sum())
        .collect()
}

fn factorial(n: usize) -> f64 {
    (1..=n).map(|i| i as f64).product()
}

#[test]
fn q1_finite_distributions_are_recovered() {
    let (w, x) = ([0.3, 0.7], [1.0, 3.0]);
    let q = invert_moments(&moments_of(&w, &x, 4), 2).unwrap();
    for i in 0..2 {
        assert!(
            rel(q.nodes[i], x[i]) <= 1e-12 && rel(q.weights[i], w[i]) <= 1e-12,
            "{q:?}"
        );
    }
    let (w, x) = ([0.2, 0.5, 0.3], [0.5, 2.0, 5.0]);
    let q = invert_moments(&moments_of(&w, &x, 6), 3).unwrap();
    for i in 0..3 {
        assert!(
            rel(q.nodes[i], x[i]) <= 1e-12 && rel(q.weights[i], w[i]) <= 1e-12,
            "{q:?}"
        );
    }
}

#[test]
fn q2_gamma_and_lognormal() {
    let gamma: Vec<f64> = (0..6).map(|n| factorial(n + 1)).collect();
    let q = invert_moments(&gamma[..4], 2).unwrap();
    let r3 = 3f64.sqrt();
    assert!(
        rel(q.nodes[0], 3.0 - r3) <= 1e-12 && rel(q.nodes[1], 3.0 + r3) <= 1e-12,
        "{q:?}"
    );
    let back = moments_of(&q.weights, &q.nodes, 4);
    for k in 0..4 {
        assert!(rel(back[k], gamma[k]) <= 1e-12, "gamma moment {k}");
    }
    let sigma: f64 = 0.5;
    let logn: Vec<f64> = (0..6).map(|n| (0.5 * (n * n) as f64 * sigma * sigma).exp()).collect();
    for (name, m) in [("gamma", &gamma), ("lognormal", &logn)] {
        let q = invert_moments(m, 3).unwrap();
        let back = moments_of(&q.weights, &q.nodes, 6);
        for k in 0..6 {
            assert!(rel(back[k], m[k]) <= 1e-9, "{name} moment {k}: {} vs {}", back[k], m[k]);
        }
        assert!(
            q.nodes.iter().all(|&x| x > 0.0) && q.nodes.windows(2).all(|w| w[0] < w[1]),
            "{q:?}"
        );
    }
}

#[test]
fn q3_degenerate_and_non_realizable_sets() {
    let c = 1.7;
    let q = invert_moments(&[2.0, 2.0 * c, 2.0 * c * c, 2.0 * c.powi(3)], 2).unwrap();
    assert_eq!(q.nodes.len(), 1);
    assert!(rel(q.nodes[0], c) <= 1e-12 && rel(q.weights[0], 2.0) <= 1e-12, "{q:?}");

    let bad: [(&str, Vec<f64>, usize, usize); 5] = [
        ("negative variance", vec![1.0, 1.0, 0.5, 1.0], 2, 2),
        ("zero m0", vec![0.0, 1.0, 1.0, 1.0], 2, 0),
        ("negative m0", vec![-1.0, 1.0, 1.0, 1.0], 2, 0),
        ("NaN moment", vec![1.0, 1.0, f64::NAN, 1.0], 2, 2),
        // 3-point law with m4 lowered far below its realizable range: second Hankel condition fails
        (
            "second Hankel",
            {
                let mut m = moments_of(&[0.2, 0.5, 0.3], &[0.5, 2.0, 5.0], 6);
                m[4] *= 0.3;
                m
            },
            3,
            4,
        ),
    ];
    for (name, m, n, idx) in bad {
        match invert_moments(&m, n) {
            Err(e @ PopBalError::NotRealizable { index, .. }) => {
                let text = e.to_string();
                eprintln!("Q3 {name}: {text}");
                assert_eq!(index, idx, "{name}");
                assert!(
                    text.starts_with("popbal:") && text.contains(&format!("index {idx}")),
                    "{text}"
                );
            }
            other => panic!("{name}: expected NotRealizable, got {other:?}"),
        }
    }
}

fn exp_moments(n: usize) -> Vec<f64> {
    (0..n).map(factorial).collect()
}

#[test]
fn q4_constant_kernel() {
    let tau_end = 5.0;
    for nodes in [2usize, 3] {
        let model = MomentModel {
            n_nodes: nodes,
            coal: Some(CoalKernel::Constant(1.0)),
            brk: None,
        };
        let mut m = exp_moments(2 * nodes);
        model.advance(&mut m, 2.0 * tau_end, 0.01).unwrap();
        let tau = tau_end;
        let mut worst: f64 = 0.0;
        for k in 0..2 * nodes {
            worst = worst.max(rel(m[k], factorial(k) * (1.0 + tau).powi(k as i32 - 1)));
        }
        let dm1 = (m[1] - 1.0).abs();
        eprintln!("Q4 N={nodes}: worst moment error {worst:.3e}, |m1-1| {dm1:.3e}");
        assert!(worst <= 1e-8 && dm1 <= 1e-12);
    }
}

#[test]
fn q5_sum_kernel() {
    let model = MomentModel {
        n_nodes: 2,
        coal: Some(CoalKernel::Sum(1.0)),
        brk: None,
    };
    let mut m = exp_moments(4);
    model.advance(&mut m, 1.0, 0.002).unwrap();
    let (r0, r2) = (rel(m[0], (-1.0f64).exp()), rel(m[2], 2.0 * 2f64.exp()));
    eprintln!("Q5 m0 {r0:.3e} m2 {r2:.3e}");
    assert!(r0 <= 1e-7 && r2 <= 1e-7);
}

#[test]
fn q6_product_kernel() {
    let model = MomentModel {
        n_nodes: 2,
        coal: Some(CoalKernel::Product(1.0)),
        brk: None,
    };
    for t in [0.25, 0.4] {
        let mut m = exp_moments(4);
        model.advance(&mut m, t, 0.001).unwrap();
        let g = 1.0 - 2.0 * t;
        let (r0, r2, r3) = (rel(m[0], 1.0 - 0.5 * t), rel(m[2], 2.0 / g), rel(m[3], 6.0 / g.powi(3)));
        eprintln!("Q6 t {t}: m0 {r0:.3e} m2 {r2:.3e} m3 {r3:.3e}");
        assert!(r0 <= 1e-7 && r2 <= 1e-7 && r3 <= 1e-7, "t {t}");
    }
}

#[test]
fn q7_breakup() {
    for (name, d, fac) in [
        (
            "uniform",
            Daughters::Uniform,
            (|k: usize| (1.0 - k as f64) / (1.0 + k as f64)) as fn(usize) -> f64,
        ),
        ("binary", Daughters::BinaryEqual, |k| 2f64.powi(1 - k as i32) - 1.0),
    ] {
        let model = MomentModel {
            n_nodes: 2,
            coal: None,
            brk: Some(Breakup {
                rate: BreakRate::Constant(1.0),
                daughters: d,
            }),
        };
        let mut m = exp_moments(4);
        model.advance(&mut m, 1.0, 0.005).unwrap();
        for k in 0..4 {
            let e = rel(m[k], factorial(k) * fac(k).exp());
            eprintln!("Q7 {name} k {k}: {e:.3e}");
            assert!(e <= 1e-8, "{name} k {k}: {e}");
        }
    }
    let model = MomentModel {
        n_nodes: 2,
        coal: None,
        brk: Some(Breakup {
            rate: BreakRate::Power { b: 1.0, alpha: 1.0 },
            daughters: Daughters::Uniform,
        }),
    };
    let mut m = exp_moments(4);
    model.advance(&mut m, 2.0, 0.005).unwrap();
    let (r0, r1) = (rel(m[0], 3.0), (m[1] - 1.0).abs());
    eprintln!("Q7 linear breakup m0 {r0:.3e}, |m1-1| {r1:.3e}");
    assert!(r0 <= 1e-8 && r1 <= 1e-12);
}

#[test]
fn q8_bins_against_qmom() {
    let tau = 2.0;
    let bins = BinModel::new(
        BinGrid::geometric(1e-6, 8, 34).unwrap(),
        Some(CoalKernel::Constant(1.0)),
        None,
    )
    .unwrap();
    let mut n = bins.project(|v| (-v).exp());
    bins.advance(&mut n, 2.0 * tau, 0.005);
    let q = MomentModel {
        n_nodes: 2,
        coal: Some(CoalKernel::Constant(1.0)),
        brk: None,
    };
    let mut m = exp_moments(4);
    q.advance(&mut m, 2.0 * tau, 0.005).unwrap();
    let d0 = rel(bins.moment(&n, 0.0), m[0]);
    let d2 = rel(bins.moment(&n, 2.0), m[2]);
    eprintln!("Q8 bins vs QMOM: m0 {d0:.3e}, m2 {d2:.3e} (reported)");
    assert!(d0 <= 1e-5);
}
