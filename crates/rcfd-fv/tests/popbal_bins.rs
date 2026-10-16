//! Discrete-bin population balance (F-228, ADR 0041): checks B1-B8 and B10. Limits are those registered in the ADR.

use rcfd_fv::popbal::{BinGrid, BinModel, BreakRate, Breakup, CoalKernel, Daughters};

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs()
}

fn model(grid: BinGrid, coal: Option<CoalKernel>, brk: Option<Breakup>) -> BinModel {
    BinModel::new(grid, coal, brk).unwrap()
}

#[test]
fn b1_linear_grid_reproduces_the_flory_distribution() {
    let m = model(
        BinGrid::linear(400, 1.0).unwrap(),
        Some(CoalKernel::Constant(1.0)),
        None,
    );
    let mut n = m.monodisperse(1.0, 1.0);
    let mut worst_pos = f64::INFINITY;
    let mut t = 0.0;
    for tau in [1.0, 4.0] {
        let target = 2.0 * tau;
        worst_pos = worst_pos.min(m.advance(&mut n, target - t, 0.01));
        t = target;
        let err = (0..400)
            .map(|k| {
                let ex = tau.powi(k as i32) / (1.0 + tau).powi(k as i32 + 2);
                (n[k] - ex).abs()
            })
            .fold(0.0, f64::max);
        let dv = (m.volume(&n) - 1.0).abs();
        eprintln!("B1 tau {tau}: max |N_k - exact| {err:.3e}, |m1-1| {dv:.3e}");
        assert!(err <= 1e-7, "B1 tau {tau}: {err}");
        assert!(dv <= 1e-12, "B1 volume {dv}");
    }
    eprintln!("B1/B9 positivity ratio {worst_pos:.3e}");
    assert!(worst_pos >= -1e-10);
}

#[test]
fn b2_geometric_grid_closes_the_number_equation() {
    let m = model(
        BinGrid::geometric(1.0, 4, 40).unwrap(),
        Some(CoalKernel::Constant(1.0)),
        None,
    );
    let mut n = m.monodisperse(1.0, 1.0);
    let mut t = 0.0;
    for tau in [1.0, 4.0] {
        m.advance(&mut n, 2.0 * tau - t, 0.01);
        t = 2.0 * tau;
        let e0 = rel(m.moment(&n, 0.0), 1.0 / (1.0 + tau));
        let dv = (m.volume(&n) - 1.0).abs();
        eprintln!("B2 tau {tau}: m0 rel err {e0:.3e}, |m1-1| {dv:.3e}");
        assert!(e0 <= 1e-8, "B2 m0 {e0}");
        assert!(dv <= 1e-12, "B2 volume {dv}");
    }
}

#[test]
fn b3_exponential_initial_density_converges_in_bins_per_doubling() {
    let tau = 2.0;
    let mut e2 = Vec::new();
    let mut e3 = Vec::new();
    for s in [2usize, 4, 8] {
        let m = model(
            BinGrid::geometric(1e-6, s, 34).unwrap(),
            Some(CoalKernel::Constant(1.0)),
            None,
        );
        let mut n = m.project(|v| (-v).exp());
        m.advance(&mut n, 2.0 * tau, 0.005);
        let r0 = rel(m.moment(&n, 0.0), 1.0 / (1.0 + tau));
        let r1 = rel(m.moment(&n, 1.0), 1.0);
        let r2 = rel(m.moment(&n, 2.0), 2.0 * (1.0 + tau));
        let r3 = rel(m.moment(&n, 3.0), 6.0 * (1.0 + tau).powi(2));
        eprintln!("B3 s {s}: m0 {r0:.3e} m1 {r1:.3e} m2 {r2:.3e} m3 {r3:.3e}");
        assert!(r0 <= 1e-5, "m0 {r0}");
        assert!(r1 <= 1e-8, "m1 {r1}");
        if s == 4 {
            assert!(r2 <= 0.08, "m2 s=4 {r2}");
        }
        if s == 8 {
            assert!(r2 <= 0.03, "m2 s=8 {r2}");
            assert!(r3 <= 0.10, "m3 s=8 {r3}");
        }
        e2.push(r2);
        e3.push(r3);
    }
    for e in [&e2, &e3] {
        assert!(e[1] < e[0] && e[2] < e[1], "errors must fall with s: {e:?}");
    }
    eprintln!(
        "B3 observed orders (bin width halves): m2 {:.2} {:.2}, m3 {:.2} {:.2}",
        (e2[0] / e2[1]).log2(),
        (e2[1] / e2[2]).log2(),
        (e3[0] / e3[1]).log2(),
        (e3[1] / e3[2]).log2()
    );
}

#[test]
fn b4_sum_kernel_on_the_linear_grid() {
    let m = model(BinGrid::linear(600, 1.0).unwrap(), Some(CoalKernel::Sum(1.0)), None);
    let mut n = m.monodisperse(1.0, 1.0);
    m.advance(&mut n, 1.0, 0.002);
    let r0 = rel(m.moment(&n, 0.0), (-1.0f64).exp());
    let r2 = rel(m.moment(&n, 2.0), 2.0f64.exp());
    eprintln!("B4 m0 {r0:.3e} m2 {r2:.3e}");
    assert!(r0 <= 1e-6 && r2 <= 1e-6);
}

#[test]
fn b5_product_kernel_on_the_linear_grid() {
    let m = model(BinGrid::linear(600, 1.0).unwrap(), Some(CoalKernel::Product(1.0)), None);
    let mut n = m.monodisperse(1.0, 1.0);
    m.advance(&mut n, 0.5, 0.002);
    let r0 = rel(m.moment(&n, 0.0), 0.75);
    let r2 = rel(m.moment(&n, 2.0), 2.0);
    eprintln!("B5 m0 {r0:.3e} m2 {r2:.3e}");
    assert!(r0 <= 1e-6 && r2 <= 1e-6);
}

#[test]
fn b6_binary_equal_breakup_generations() {
    let g = BinGrid::geometric(1.0, 2, 20).unwrap();
    let top = *g.x.last().unwrap();
    let nb = g.len();
    let m = model(
        g,
        None,
        Some(Breakup {
            rate: BreakRate::Constant(1.0),
            daughters: Daughters::BinaryEqual,
        }),
    );
    let mut n = m.monodisperse(top, 1.0);
    m.advance(&mut n, 1.0, 0.01);
    let mut worst: f64 = 0.0;
    for gen in 0..=15usize {
        let fact: f64 = (1..=gen).map(|i| i as f64).product();
        let ex = (-1.0f64).exp() * 2f64.powi(gen as i32) / fact;
        worst = worst.max((n[nb - 1 - 2 * gen] - ex).abs());
    }
    let r0 = rel(m.moment(&n, 0.0), 1f64.exp());
    let dv = rel(m.volume(&n), top);
    eprintln!("B6 generation abs err {worst:.3e}, m0 rel {r0:.3e}, volume rel {dv:.3e}");
    assert!(worst <= 1e-9 && r0 <= 1e-9 && dv <= 1e-12);
}

#[test]
fn b7_uniform_daughters() {
    let mut errs = Vec::new();
    for s in [4usize, 8] {
        let g = BinGrid::geometric(2f64.powi(-40), s, 40).unwrap();
        let top = *g.x.last().unwrap();
        assert!((top - 1.0).abs() < 1e-12);
        let m = model(
            g,
            None,
            Some(Breakup {
                rate: BreakRate::Constant(1.0),
                daughters: Daughters::Uniform,
            }),
        );
        let mut n = m.monodisperse(top, 1.0);
        m.advance(&mut n, 1.0, 0.01);
        let r0 = rel(m.moment(&n, 0.0), 1f64.exp());
        let r1 = rel(m.moment(&n, 1.0), top);
        let r2 = rel(m.moment(&n, 2.0), top * top * (-1.0f64 / 3.0).exp());
        let r3 = rel(m.moment(&n, 3.0), top.powi(3) * (-0.5f64).exp());
        eprintln!("B7 s {s}: m0 {r0:.3e} m1 {r1:.3e} m2 {r2:.3e} m3 {r3:.3e}");
        assert!(r0 <= 1e-8, "m0 {r0}");
        assert!(r1 <= 1e-12, "m1 {r1}");
        if s == 8 {
            assert!(r2 <= 0.05 && r3 <= 0.10, "m2 {r2} m3 {r3}");
        }
        errs.push((r2, r3));
    }
    assert!(errs[1].0 < errs[0].0 && errs[1].1 < errs[0].1, "{errs:?}");
}

/// Deterministic pseudo-random numbers in (0, 1].
fn lcg(state: &mut u64) -> f64 {
    *state = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    ((*state >> 11) as f64 + 1.0) / (1u64 << 53) as f64
}

#[test]
fn b8_rhs_conserves_volume_and_number_to_round_off() {
    let kernels = [
        CoalKernel::Constant(1.0),
        CoalKernel::Sum(1.0),
        CoalKernel::Product(1.0),
        CoalKernel::Brownian { kt_over_mu: 1.0 },
        CoalKernel::Shear { gamma_dot: 1.0 },
        CoalKernel::Turbulent { epsilon: 0.1, nu: 1e-2 },
    ];
    let breaks = [
        Breakup {
            rate: BreakRate::Constant(1.0),
            daughters: Daughters::Uniform,
        },
        Breakup {
            rate: BreakRate::Power { b: 0.3, alpha: 0.5 },
            daughters: Daughters::SymBeta(2),
        },
        Breakup {
            rate: BreakRate::Constant(2.0),
            daughters: Daughters::BinaryEqual,
        },
    ];
    let mut seed = 12345u64;
    let mut worst_v: f64 = 0.0;
    let mut worst_n: f64 = 0.0;
    let grid = || BinGrid::geometric(1.0, 3, 30).unwrap();
    let mb = grid().len();
    for k in kernels.iter().map(|k| Some(*k)).chain([None]) {
        for b in breaks.iter().map(|b| Some(*b)).chain([None]) {
            if k.is_none() && b.is_none() {
                continue;
            }
            let m = model(grid(), k, b);
            let n: Vec<f64> = (0..mb).map(|_| lcg(&mut seed)).collect();
            let mut f = vec![0.0; mb];
            m.rhs(&n, &mut f);
            let x = &m.grid().x;
            let num: f64 = x.iter().zip(&f).map(|(x, f)| x * f).sum();
            let den: f64 = x.iter().zip(&f).map(|(x, f)| (x * f).abs()).sum();
            worst_v = worst_v.max(num.abs() / den);
        }
    }
    // number: coalescence with support in the lower half
    for k in kernels {
        let m = model(grid(), Some(k), None);
        let n: Vec<f64> = (0..mb).map(|i| if i < mb / 2 { lcg(&mut seed) } else { 0.0 }).collect();
        let mut f = vec![0.0; mb];
        m.rhs(&n, &mut f);
        let x = &m.grid().x;
        let mut half = 0.0;
        for j in 0..mb {
            for l in 0..mb {
                half += 0.5 * k.eval(x[j], x[l]) * n[j] * n[l];
            }
        }
        let got: f64 = f.iter().sum();
        worst_n = worst_n.max((got + half).abs() / half);
    }
    {
        let m = model(
            grid(),
            None,
            Some(Breakup {
                rate: BreakRate::Constant(1.7),
                daughters: Daughters::BinaryEqual,
            }),
        );
        let n: Vec<f64> = (0..mb).map(|i| if i >= 4 { lcg(&mut seed) } else { 0.0 }).collect();
        let mut f = vec![0.0; mb];
        m.rhs(&n, &mut f);
        let got: f64 = f.iter().sum();
        let want = 1.7 * n.iter().sum::<f64>();
        worst_n = worst_n.max((got - want).abs() / want);
    }
    eprintln!("B8 worst volume {worst_v:.3e}, worst number {worst_n:.3e}");
    assert!(worst_v <= 1e-13 && worst_n <= 1e-13);
}

// Set-up defect, kept as a record (ADR 0041 addendum): the bound `num > 1.0` is wrong for bin 1, whose uniform daughters all fall
// below the first pivot and carry exactly number 1 (volume-conserving pile-up). Corrected in `b10_kernel_and_daughter_identities_v2`.
#[test]
#[ignore = "set-up defect, see v2"]
fn b10_kernel_and_daughter_identities_v1_setup_defect() {
    let kt = 0.37;
    let b = CoalKernel::Brownian { kt_over_mu: kt };
    let v = 3.7;
    assert!(rel(b.eval(v, v), 8.0 * kt / 3.0) <= 1e-14);
    let gd = 2.5;
    let sh = CoalKernel::Shear { gamma_dot: gd };
    assert!(rel(sh.eval(v, v), 8.0 * gd * v / std::f64::consts::PI) <= 1e-14);
    let (eps, nu) = (0.4, 1.3e-3);
    let st = CoalKernel::Turbulent { epsilon: eps, nu };
    let eq_exact = CoalKernel::Shear {
        gamma_dot: (8.0 * std::f64::consts::PI / 15.0).sqrt() / (4.0 / 3.0) * (eps / nu).sqrt(),
    };
    assert!(rel(st.eval(0.3, 5.0), eq_exact.eval(0.3, 5.0)) <= 1e-14);
    for k in [b, sh, st, CoalKernel::Sum(1.0), CoalKernel::Product(2.0)] {
        for (u, w) in [(0.1, 7.0), (2.0, 2.5), (30.0, 0.4)] {
            assert!(k.eval(u, w) > 0.0 && k.eval(u, w) == k.eval(w, u), "{k:?}");
        }
    }
    // initial number rate of a monodisperse bin under the Brownian kernel
    let grid = BinGrid::geometric(1.0, 2, 10).unwrap();
    let m = model(grid, Some(b), None);
    let nn = 1.7;
    let mut n = vec![0.0; m.len()];
    n[5] = nn;
    let mut f = vec![0.0; m.len()];
    m.rhs(&n, &mut f);
    let rate: f64 = f.iter().sum();
    let want = -(4.0 * kt / 3.0) * nn * nn;
    eprintln!("B10 Brownian initial number rate {rate:.12e} vs {want:.12e}");
    assert!(rel(rate, want) <= 1e-12);

    // daughter rows: volume exactly, number up to the part below the first pivot
    let g = BinGrid::geometric(1e-3, 2, 20).unwrap();
    for d in [Daughters::Uniform, Daughters::SymBeta(1), Daughters::SymBeta(3)] {
        let m = model(
            g.clone(),
            None,
            Some(Breakup {
                rate: BreakRate::Constant(1.0),
                daughters: d,
            }),
        );
        for k in 0..m.len() {
            let row = m.daughter_row(k);
            let vol: f64 = row.iter().map(|&(i, n)| g.x[i] * n).sum();
            assert!(rel(vol, g.x[k]) <= 1e-13, "{d:?} k {k}: volume {vol} vs {}", g.x[k]);
            let num: f64 = row.iter().map(|&(_, n)| n).sum();
            assert!(num <= 2.0 + 1e-13 && num > 1.0, "{d:?} k {k}: number {num}");
            if d == Daughters::Uniform {
                // number deficiency is int_0^{x1} (2/x)(1 - v/x1) dv = x1/x_k
                assert!((2.0 - num - g.x[0] / g.x[k]).abs() <= 1e-13, "uniform k {k}: {num}");
            }
        }
        // moment factor against numerical integration of the density
        for kk in 0..5u32 {
            let nsteps = 4000;
            let h = 1.0 / nsteps as f64;
            let mut acc = 0.0;
            for i in 0..=nsteps {
                let v = i as f64 * h;
                let w = if i == 0 || i == nsteps {
                    1.0
                } else if i % 2 == 1 {
                    4.0
                } else {
                    2.0
                };
                acc += w * v.powi(kk as i32) * d.density(v, 1.0).unwrap();
            }
            acc *= h / 3.0;
            assert!(
                rel(acc, d.moment_factor(kk)) <= 1e-9,
                "{d:?} k {kk}: {acc} vs {}",
                d.moment_factor(kk)
            );
        }
    }
    // binary equal: number exactly 2 where the daughters stay on the grid
    let m = model(
        g.clone(),
        None,
        Some(Breakup {
            rate: BreakRate::Constant(1.0),
            daughters: Daughters::BinaryEqual,
        }),
    );
    for k in 2..m.len() {
        let row = m.daughter_row(k);
        let num: f64 = row.iter().map(|&(_, n)| n).sum();
        let vol: f64 = row.iter().map(|&(i, n)| g.x[i] * n).sum();
        assert!(rel(num, 2.0) <= 1e-13 && rel(vol, g.x[k]) <= 1e-13, "binary k {k}");
    }
}

#[test]
fn b10_kernel_and_daughter_identities_v2() {
    let kt = 0.37;
    let b = CoalKernel::Brownian { kt_over_mu: kt };
    let v = 3.7;
    assert!(rel(b.eval(v, v), 8.0 * kt / 3.0) <= 1e-14);
    let gd = 2.5;
    let sh = CoalKernel::Shear { gamma_dot: gd };
    assert!(rel(sh.eval(v, v), 8.0 * gd * v / std::f64::consts::PI) <= 1e-14);
    let (eps, nu) = (0.4, 1.3e-3);
    let st = CoalKernel::Turbulent { epsilon: eps, nu };
    let eq_exact = CoalKernel::Shear {
        gamma_dot: (8.0 * std::f64::consts::PI / 15.0).sqrt() / (4.0 / 3.0) * (eps / nu).sqrt(),
    };
    assert!(rel(st.eval(0.3, 5.0), eq_exact.eval(0.3, 5.0)) <= 1e-14);
    for k in [b, sh, st, CoalKernel::Sum(1.0), CoalKernel::Product(2.0)] {
        for (u, w) in [(0.1, 7.0), (2.0, 2.5), (30.0, 0.4)] {
            assert!(k.eval(u, w) > 0.0 && k.eval(u, w) == k.eval(w, u), "{k:?}");
        }
    }
    // initial number rate of a monodisperse bin under the Brownian kernel
    let grid = BinGrid::geometric(1.0, 2, 10).unwrap();
    let m = model(grid, Some(b), None);
    let nn = 1.7;
    let mut n = vec![0.0; m.len()];
    n[5] = nn;
    let mut f = vec![0.0; m.len()];
    m.rhs(&n, &mut f);
    let rate: f64 = f.iter().sum();
    let want = -(4.0 * kt / 3.0) * nn * nn;
    eprintln!("B10 Brownian initial number rate {rate:.12e} vs {want:.12e}");
    assert!(rel(rate, want) <= 1e-12);

    // daughter rows: volume exactly, number up to the part below the first pivot
    let g = BinGrid::geometric(1e-3, 2, 20).unwrap();
    for d in [Daughters::Uniform, Daughters::SymBeta(1), Daughters::SymBeta(3)] {
        let m = model(
            g.clone(),
            None,
            Some(Breakup {
                rate: BreakRate::Constant(1.0),
                daughters: d,
            }),
        );
        for k in 0..m.len() {
            let row = m.daughter_row(k);
            let vol: f64 = row.iter().map(|&(i, n)| g.x[i] * n).sum();
            assert!(rel(vol, g.x[k]) <= 1e-13, "{d:?} k {k}: volume {vol} vs {}", g.x[k]);
            let num: f64 = row.iter().map(|&(_, n)| n).sum();
            assert!((1.0 - 1e-13..=2.0 + 1e-13).contains(&num), "{d:?} k {k}: number {num}");
            if d == Daughters::Uniform {
                // number deficiency is int_0^{x1} (2/x)(1 - v/x1) dv = x1/x_k
                assert!((2.0 - num - g.x[0] / g.x[k]).abs() <= 1e-13, "uniform k {k}: {num}");
            }
        }
        // moment factor against numerical integration of the density
        for kk in 0..5u32 {
            let nsteps = 4000;
            let h = 1.0 / nsteps as f64;
            let mut acc = 0.0;
            for i in 0..=nsteps {
                let v = i as f64 * h;
                let w = if i == 0 || i == nsteps {
                    1.0
                } else if i % 2 == 1 {
                    4.0
                } else {
                    2.0
                };
                acc += w * v.powi(kk as i32) * d.density(v, 1.0).unwrap();
            }
            acc *= h / 3.0;
            assert!(
                rel(acc, d.moment_factor(kk)) <= 1e-9,
                "{d:?} k {kk}: {acc} vs {}",
                d.moment_factor(kk)
            );
        }
    }
    // binary equal: number exactly 2 where the daughters stay on the grid
    let m = model(
        g.clone(),
        None,
        Some(Breakup {
            rate: BreakRate::Constant(1.0),
            daughters: Daughters::BinaryEqual,
        }),
    );
    for k in 2..m.len() {
        let row = m.daughter_row(k);
        let num: f64 = row.iter().map(|&(_, n)| n).sum();
        let vol: f64 = row.iter().map(|&(i, n)| g.x[i] * n).sum();
        assert!(rel(num, 2.0) <= 1e-13 && rel(vol, g.x[k]) <= 1e-13, "binary k {k}");
    }
}
