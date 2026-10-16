//! Effectiveness-NTU library checks (F-217, ADR 0039 checks H1 to H4). The limits are the ones registered in
//! `docs/adr/0039-wave23a-mhd-heat-exchanger.md` before any run. The references are independent numerical integrations of
//! the two-stream balance, not the closed forms under test.

use rcfd_fv::hx::{core_dp, effectiveness, ntu_for_effectiveness, solve_exchanger, vent_dp, Arrangement};

const ALL: [Arrangement; 6] = [
    Arrangement::Counterflow,
    Arrangement::Parallel,
    Arrangement::CrossBothUnmixed,
    Arrangement::CrossCmaxMixed,
    Arrangement::CrossCminMixed,
    Arrangement::ShellAndTube,
];

/// Counterflow / parallel by RK4 marching of `dT_h/dx = -nh (T_h - T_c)`, `dT_c/dx = -+ nc (T_h - T_c)`; the hot stream is
/// the `C_min` stream (`nh = N`, `nc = N Cr`). Hot inlet 1, cold inlet 0. Returns the effectiveness `1 - T_h(1)`.
fn marched(counter: bool, n: f64, cr: f64) -> f64 {
    let (nh, nc) = (n, n * cr);
    let sign = if counter { -1.0 } else { 1.0 };
    let steps = 4000;
    let h = 1.0 / steps as f64;
    let rhs = |th: f64, tc: f64| (-nh * (th - tc), sign * nc * (th - tc));
    let integrate = |tc0: f64| -> (f64, f64) {
        let (mut th, mut tc) = (1.0, tc0);
        for _ in 0..steps {
            let (a1, b1) = rhs(th, tc);
            let (a2, b2) = rhs(th + 0.5 * h * a1, tc + 0.5 * h * b1);
            let (a3, b3) = rhs(th + 0.5 * h * a2, tc + 0.5 * h * b2);
            let (a4, b4) = rhs(th + h * a3, tc + h * b3);
            th += h / 6.0 * (a1 + 2.0 * a2 + 2.0 * a3 + a4);
            tc += h / 6.0 * (b1 + 2.0 * b2 + 2.0 * b3 + b4);
        }
        (th, tc)
    };
    if counter {
        // shooting: T_c(1) = 0 is linear in the guess T_c(0)
        let (_, c_a) = integrate(0.0);
        let (_, c_b) = integrate(1.0);
        let g = -c_a / (c_b - c_a);
        let (th, _) = integrate(g);
        1.0 - th
    } else {
        let (th, _) = integrate(0.0);
        1.0 - th
    }
}

#[test]
fn h1_counterflow_and_parallel_against_marching() {
    let mut worst = 0.0_f64;
    for &n in &[0.2, 1.0, 3.0, 8.0] {
        for &cr in &[0.0, 0.25, 0.75, 1.0] {
            for (arr, counter) in [(Arrangement::Counterflow, true), (Arrangement::Parallel, false)] {
                let want = marched(counter, n, cr);
                let got = effectiveness(arr, n, cr);
                let e = (got / want - 1.0).abs();
                worst = worst.max(e);
                assert!(
                    e <= 1e-5,
                    "{arr:?} N {n} Cr {cr}: {got} vs marched {want} (rel {e:.2e})"
                );
            }
        }
    }
    eprintln!("H1 worst relative error vs marching {worst:.2e} (limit 1e-5)");
    // the Cr = 1 branch is continuous with the general form
    for &n in &[0.5, 2.0, 7.0] {
        let a = effectiveness(Arrangement::Counterflow, n, 1.0);
        let b = effectiveness(Arrangement::Counterflow, n, 1.0 - 1e-7);
        assert!((a / b - 1.0).abs() <= 1e-6, "N {n}: {a} vs {b}");
    }
}

/// WAVE 25B NOTE (ADR 0044): with the exact series in `effectiveness` this test passes when run with `--ignored`; the record is kept
/// ignored as registered, and `h5_*` below assert the same limits as passing tests.
/// KNOWN MISS of the registered H2 small-N slope limit (1e-3 at N = 1e-3) for the crossflow both-unmixed closed form: measured
/// 0.99836 at Cr = 0.5. Root cause: the formula is a curve fit of the series solution, `eps ~ N - (Cr/2) N^1.34` for small N,
/// whose slope tends to 1 only as N^0.34 -> 0; the exact solution has `eps = N - N^2 (1 + Cr)/2 + ...`.
#[test]
#[ignore = "KNOWN MISS: the both-unmixed crossflow closed form is a curve fit, slope at N = 1e-3 is 0.99836 (limit 1e-3)"]
fn h2_cross_unmixed_slope_known_miss() {
    let small = effectiveness(Arrangement::CrossBothUnmixed, 1e-3, 0.5) / 1e-3;
    eprintln!("H2 both-unmixed slope {small:.6}");
    assert!((small - 1.0).abs() <= 1e-3, "slope {small}");
}

/// Registered H2 limits, in full (`full = true`, including the crossflow both-unmixed curve fit) or for the exactly derived
/// arrangements only (`full = false`: the unmixed fit is replaced by the Cmax-mixed crossflow in the ordering).
fn h2_check(full: bool) {
    for arr in ALL {
        for &n in &[0.1_f64, 1.0, 5.0] {
            let want = 1.0 - (-n).exp();
            assert!((effectiveness(arr, n, 0.0) - want).abs() <= 1e-14, "{arr:?} Cr=0 N {n}");
        }
        if !full && arr == Arrangement::CrossBothUnmixed {
            continue;
        }
        let small = effectiveness(arr, 1e-3, 0.5) / 1e-3;
        assert!((small - 1.0).abs() <= 1e-3, "{arr:?} slope {small}");
    }
    assert!((effectiveness(Arrangement::Counterflow, 400.0, 0.5) - 1.0).abs() < 1e-12);
    assert!((effectiveness(Arrangement::Counterflow, 4000.0, 1.0) - 1.0).abs() < 1e-3);
    assert!((effectiveness(Arrangement::Parallel, 400.0, 0.5) - 1.0 / 1.5).abs() < 1e-12);
    // ordering, range, monotonicity on a grid
    let mut violations = 0;
    for i in 1..=200 {
        let n = 0.05 * i as f64;
        for j in 1..=19 {
            let c = 0.05 * j as f64;
            let par = effectiveness(Arrangement::Parallel, n, c);
            let cross = effectiveness(
                if full {
                    Arrangement::CrossBothUnmixed
                } else {
                    Arrangement::CrossCmaxMixed
                },
                n,
                c,
            );
            let counter = effectiveness(Arrangement::Counterflow, n, c);
            let shell = effectiveness(Arrangement::ShellAndTube, n, c);
            if !(par <= cross + 1e-12 && cross <= counter + 1e-12 && shell <= counter + 1e-12) {
                violations += 1;
            }
            for arr in ALL {
                if !full && arr == Arrangement::CrossBothUnmixed {
                    continue;
                }
                let e = effectiveness(arr, n, c);
                if !(0.0..=1.0).contains(&e) {
                    violations += 1;
                }
                if effectiveness(arr, n + 0.05, c) < e - 1e-12 {
                    violations += 1;
                }
                if effectiveness(arr, n, c + 0.05) > e + 1e-12 {
                    violations += 1;
                }
            }
        }
    }
    eprintln!("H2 (full = {full}) ordering/range/monotonicity violations {violations} (limit 0)");
    assert_eq!(violations, 0);
}

#[test]
fn h2_limits_and_ordering_exact_arrangements() {
    h2_check(false);
}

/// WAVE 25B NOTE (ADR 0044): with the exact series in `effectiveness` this test passes when run with `--ignored`; the record is kept
/// ignored as registered, and `h5_*` below assert the same limits as passing tests.
/// KNOWN MISS: the registered H2 limits with the both-unmixed curve fit included (slope 0.99836 at N = 1e-3, and 177 ordering
/// violations where the fit lies below the parallel-flow result at small N, e.g. N = 0.05, Cr = 0.05).
#[test]
#[ignore = "KNOWN MISS: both-unmixed crossflow curve fit violates the small-N slope and the parallel <= cross ordering"]
fn h2_limits_and_ordering_known_miss() {
    h2_check(true);
}

/// Crossflow, stream A (hot, flows in x) unmixed in y, stream B (cold, flows in y) mixed across x: row recursion with the
/// exact channel exchange. `a_is_cmin`: A is the `C_min` stream. Returns the effectiveness.
fn cross_one_mixed(rows: usize, n: f64, cr: f64, a_is_cmin: bool) -> f64 {
    // NTU of an A channel: N_A = UA / C_A
    let (n_a, c_a_over_c_b) = if a_is_cmin { (n, cr) } else { (n * cr, 1.0 / cr) };
    let mut tb = 0.0;
    let mut a_out_sum = 0.0;
    let e = (-n_a).exp();
    for _ in 0..rows {
        let a_out = tb + (1.0 - tb) * e;
        a_out_sum += a_out;
        tb += c_a_over_c_b / rows as f64 * (1.0 - a_out);
    }
    if a_is_cmin {
        1.0 - a_out_sum / rows as f64
    } else {
        tb
    }
}

/// Crossflow, both unmixed: n x n cells, each exchanging as a cocurrent cell; `a_is_cmin` as above.
fn cross_unmixed(cells: usize, n: f64, cr: f64) -> f64 {
    // A = Cmin (hot, flows in x through `cells` channels), B = Cmax (cold, flows in y)
    let (na, nb) = (n / cells as f64, n * cr / cells as f64);
    let s = na + nb;
    let g = (1.0 - (-s).exp()) / s;
    let mut tb = vec![0.0; cells]; // cold temperature of column i, entering from the bottom
    let mut a_out_sum = 0.0;
    for _j in 0..cells {
        let mut ta = 1.0;
        for t in tb.iter_mut() {
            let d = ta - *t;
            ta -= na * g * d;
            *t += nb * g * d;
        }
        a_out_sum += ta;
    }
    1.0 - a_out_sum / cells as f64
}

#[test]
fn h3_crossflow_variants_against_independent_recursions() {
    let richardson = |f: &dyn Fn(usize) -> f64, n: usize| 2.0 * f(2 * n) - f(n);
    for &n in &[0.5, 1.0, 3.0] {
        for &cr in &[0.25, 0.5, 1.0] {
            // C_max mixed (the Cmin stream unmixed)
            let want = richardson(&|r| cross_one_mixed(r, n, cr, true), 4000);
            let got = effectiveness(Arrangement::CrossCmaxMixed, n, cr);
            let e = (got / want - 1.0).abs();
            eprintln!("H3 Cmax mixed N {n} Cr {cr}: {got:.6} vs {want:.6} (rel {e:.2e}, limit 1e-3)");
            assert!(e <= 1e-3);
            // C_min mixed
            let want = richardson(&|r| cross_one_mixed(r, n, cr, false), 4000);
            let got = effectiveness(Arrangement::CrossCminMixed, n, cr);
            let e = (got / want - 1.0).abs();
            eprintln!("H3 Cmin mixed N {n} Cr {cr}: {got:.6} vs {want:.6} (rel {e:.2e}, limit 1e-3)");
            assert!(e <= 1e-3);
        }
    }
}

/// WAVE 25B NOTE (ADR 0044): with the exact series in `effectiveness` this test passes when run with `--ignored`; the record is kept
/// ignored as registered, and `h5_*` below assert the same limits as passing tests.
/// KNOWN MISS of the registered H3 limit (2 percent) for the crossflow both-unmixed closed approximation against the registered
/// reference, the 800 x 800 cell-exact recursion (cross-checked against the Richardson extrapolation of 150 and 300 cells, which
/// agrees to the printed difference): the deviation reaches 3.3 percent at N = 0.5, Cr = 1 (the correlation
/// 1 - exp[N^0.22/Cr (exp(-Cr N^0.78) - 1)] is a curve fit; its error is largest at small NTU and Cr = 1). The mixed variants pass
/// (h3_crossflow_variants_against_independent_recursions).
#[test]
#[ignore = "KNOWN MISS: both-unmixed crossflow curve fit deviates up to 3.3 percent from the exact solution (limit 2e-2)"]
fn h3_cross_unmixed_known_miss() {
    let richardson = |f: &dyn Fn(usize) -> f64, n: usize| 2.0 * f(2 * n) - f(n);
    let mut worst = 0.0_f64;
    for &n in &[0.5, 1.0, 3.0] {
        for &cr in &[0.25, 0.5, 1.0] {
            let want = cross_unmixed(800, n, cr);
            let rich = richardson(&|c| cross_unmixed(c, n, cr), 150);
            let got = effectiveness(Arrangement::CrossBothUnmixed, n, cr);
            let e = (got / want - 1.0).abs();
            eprintln!(
                "H3 both unmixed N {n} Cr {cr}: {got:.6} vs 800x800 {want:.6} (rel {e:.2e}, limit 2e-2); Richardson 150/300 {rich:.6} (reference difference {:.1e})",
                (rich / want - 1.0).abs()
            );
            worst = worst.max(e);
        }
    }
    assert!(worst <= 2e-2, "worst {worst}");
}

#[test]
fn h4_energy_balance_round_trip_and_loss_closures() {
    let mut worst = 0.0_f64;
    for arr in ALL {
        for &(cp, cs) in &[(40.0, 20.0), (20.0, 40.0), (30.0, 30.0), (25.0, 0.0)] {
            let r = solve_exchanger(arr, cp, cs, 35.0, 300.0, 360.0);
            let qp = cp * (r.t_primary_out - 300.0);
            let qs = if cs > 0.0 {
                cs * (360.0 - r.t_secondary_out)
            } else {
                r.q
            };
            worst = worst.max((qp - qs).abs() / r.q.abs());
            assert!((qp - r.q).abs() / r.q.abs() <= 1e-12);
        }
    }
    eprintln!("H4 energy balance worst {worst:.2e} (limit 1e-12)");
    assert!(worst <= 1e-12);
    for arr in ALL {
        for &c in &[0.0, 0.3, 0.9] {
            for &n in &[0.3, 1.5, 4.0] {
                let eps = effectiveness(arr, n, c);
                let back = ntu_for_effectiveness(arr, eps, c).unwrap();
                assert!((back / n - 1.0).abs() <= 1e-9, "{arr:?} Cr {c} N {n}: {back}");
            }
        }
    }
    // loss closures against hand values
    assert!((core_dp(2.0, 5.0, 1.2, 3.0) - (6.0 + 5.0 * 1.2 * 9.0 / 2.0)).abs() <= 1e-14);
    assert!((core_dp(2.0, 5.0, 1.2, -3.0) - (-6.0 - 5.0 * 1.2 * 9.0 / 2.0)).abs() <= 1e-14);
    assert!((vent_dp(2.5, 1.2, 4.0) - 2.5 * 1.2 * 16.0 / 2.0).abs() <= 1e-14);
    // reversed or zero flow gives no heat
    assert_eq!(
        solve_exchanger(Arrangement::Counterflow, 0.0, 20.0, 30.0, 300.0, 350.0).q,
        0.0
    );
    assert_eq!(
        solve_exchanger(Arrangement::Counterflow, -3.0, 20.0, 30.0, 300.0, 350.0).q,
        0.0
    );
}

// ---------------------------------------------------------------------------------------------------------------------
// Wave 25B (ADR 0044 D1): exact both-unmixed crossflow series. Checks X1 to X5 as registered before the implementation.
// ---------------------------------------------------------------------------------------------------------------------

/// The retired curve fit `1 - exp[N^0.22/Cr (exp(-Cr N^0.78) - 1)]`, kept here only as a diagnostic.
fn retired_fit(n: f64, c: f64) -> f64 {
    1.0 - (n.powf(0.22) / c * ((-c * n.powf(0.78)).exp() - 1.0)).exp()
}

/// X1: agreement with the independent marched 2-D solution (cell recursion, Richardson 400/800), limit 1e-4.
#[test]
fn h5_x1_exact_series_against_marched_2d() {
    let mut worst = 0.0_f64;
    let mut worst_ref = 0.0_f64;
    for &n in &[0.1, 0.5, 1.0, 3.0, 8.0] {
        for &cr in &[0.1, 0.25, 0.5, 1.0] {
            let r800 = 2.0 * cross_unmixed(800, n, cr) - cross_unmixed(400, n, cr);
            let r400 = 2.0 * cross_unmixed(400, n, cr) - cross_unmixed(200, n, cr);
            let got = effectiveness(Arrangement::CrossBothUnmixed, n, cr);
            let e = (got / r800 - 1.0).abs();
            let conv = (r800 / r400 - 1.0).abs();
            eprintln!("X1 N {n} Cr {cr}: series {got:.9} marched {r800:.9} rel {e:.2e} (reference convergence {conv:.1e}) fit {:.6}", retired_fit(n, cr));
            worst = worst.max(e);
            worst_ref = worst_ref.max(conv);
        }
    }
    eprintln!("X1 worst relative deviation {worst:.3e} (limit 1e-4); worst reference convergence {worst_ref:.2e}");
    assert!(worst <= 1e-4, "worst {worst}");
}

/// X2: small-N slope and second-order coefficient.
#[test]
fn h5_x2_small_n_slope_and_second_order() {
    for &cr in &[0.25, 0.5, 1.0] {
        let n = 1e-7;
        let s = effectiveness(Arrangement::CrossBothUnmixed, n, cr) / n;
        eprintln!("X2 Cr {cr}: eps/N at 1e-7 = {s:.10} (limit |.-1| <= 1e-6)");
        assert!((s - 1.0).abs() <= 1e-6, "Cr {cr}: {s}");
        let n = 1e-4;
        let e = effectiveness(Arrangement::CrossBothUnmixed, n, cr);
        let c2 = (e - n) / (n * n);
        eprintln!(
            "X2 Cr {cr}: (eps - N)/N^2 at 1e-4 = {c2:.6} vs {:.6}",
            -(1.0 + cr) / 2.0
        );
        assert!(
            (c2 + (1.0 + cr) / 2.0).abs() <= 1e-3 * (1.0 + cr) / 2.0,
            "Cr {cr}: {c2}"
        );
    }
    // the old registered H2 limit
    let small = effectiveness(Arrangement::CrossBothUnmixed, 1e-3, 0.5) / 1e-3;
    eprintln!("X2 old H2 slope at N = 1e-3, Cr = 0.5: {small:.6} (limit 1e-3)");
    assert!((small - 1.0).abs() <= 1e-3, "slope {small}");
}

/// X3: bounds, ordering, monotonicity on the registered H2 grid plus Cr = 1, and the NTU round trip.
#[test]
fn h5_x3_bounds_ordering_monotonicity() {
    let mut violations = 0;
    let crs: Vec<f64> = (1..=19).map(|j| 0.05 * j as f64).chain(std::iter::once(1.0)).collect();
    for i in 1..=200 {
        let n = 0.05 * i as f64;
        for &c in &crs {
            let par = effectiveness(Arrangement::Parallel, n, c);
            let cross = effectiveness(Arrangement::CrossBothUnmixed, n, c);
            let counter = effectiveness(Arrangement::Counterflow, n, c);
            if !(par <= cross + 1e-12 && cross <= counter + 1e-12) {
                violations += 1;
                eprintln!("X3 order violation N {n} Cr {c}: {par} {cross} {counter}");
            }
            if !(0.0..=1.0).contains(&cross) {
                violations += 1;
            }
            if effectiveness(Arrangement::CrossBothUnmixed, n + 0.05, c) < cross - 1e-12 {
                violations += 1;
            }
            if c < 1.0 && effectiveness(Arrangement::CrossBothUnmixed, n, c + 0.05) > cross + 1e-12 {
                violations += 1;
            }
        }
    }
    eprintln!("X3 violations {violations} (limit 0)");
    assert_eq!(violations, 0);
    let (p, x, c) = (
        effectiveness(Arrangement::Parallel, 1.0, 0.5),
        effectiveness(Arrangement::CrossBothUnmixed, 1.0, 0.5),
        effectiveness(Arrangement::Counterflow, 1.0, 0.5),
    );
    assert!(p < x && x < c, "{p} {x} {c}");
    for &cr in &[0.0, 0.3, 0.9, 1.0] {
        for &n in &[0.3, 1.5, 4.0] {
            let eps = effectiveness(Arrangement::CrossBothUnmixed, n, cr);
            let back = ntu_for_effectiveness(Arrangement::CrossBothUnmixed, eps, cr).unwrap();
            assert!((back / n - 1.0).abs() <= 1e-9, "Cr {cr} N {n}: {back}");
        }
    }
}

/// X4: limits, including the derived large-N behaviour.
#[test]
fn h5_x4_limits() {
    for &n in &[0.1, 1.0, 5.0] {
        let e = effectiveness(Arrangement::CrossBothUnmixed, n, 1e-9);
        let d = (e - (1.0 - (-n).exp())).abs();
        eprintln!("X4 Cr 1e-9 N {n}: {e:.12} deviation {d:.2e} (limit 1e-8)");
        assert!(d <= 1e-8, "N {n}: {d}");
    }
    let e = effectiveness(Arrangement::CrossBothUnmixed, 400.0, 0.5);
    eprintln!("X4 Cr 0.5 N 400: 1 - eps = {:.3e} (limit 1e-9)", 1.0 - e);
    assert!(1.0 - e <= 1e-9);
    let t = std::time::Instant::now();
    let e = effectiveness(Arrangement::CrossBothUnmixed, 1e4, 1.0);
    let asy = 1.0 / (std::f64::consts::PI * 1e4).sqrt();
    eprintln!(
        "X4 Cr 1 N 1e4: 1 - eps = {:.5e}, 1/sqrt(pi N) = {asy:.5e}, time {:?}",
        1.0 - e,
        t.elapsed()
    );
    assert!(((1.0 - e) / asy - 1.0).abs() <= 0.05);
    for &cr in &[0.01, 0.5, 1.0] {
        let t = std::time::Instant::now();
        let e = effectiveness(Arrangement::CrossBothUnmixed, 1e6, cr);
        eprintln!("X4 N 1e6 Cr {cr}: eps {e:.8} time {:?}", t.elapsed());
        assert!(e.is_finite() && (0.0..=1.0).contains(&e));
    }
}

/// X5: the registered old limits, re-asserted for the exact series; plus the retired fit's documented deviation.
#[test]
fn h5_x5_old_registered_limits_and_retired_fit() {
    h2_check(true);
    let richardson = |f: &dyn Fn(usize) -> f64, n: usize| 2.0 * f(2 * n) - f(n);
    let mut worst = 0.0_f64;
    for &n in &[0.5, 1.0, 3.0] {
        for &cr in &[0.25, 0.5, 1.0] {
            let want = cross_unmixed(800, n, cr);
            let rich = richardson(&|c| cross_unmixed(c, n, cr), 150);
            let got = effectiveness(Arrangement::CrossBothUnmixed, n, cr);
            worst = worst.max((got / want - 1.0).abs());
            assert!((got / rich - 1.0).abs() <= 2e-2);
        }
    }
    eprintln!("X5 H3 worst vs 800x800 {worst:.3e} (registered limit 2e-2)");
    assert!(worst <= 2e-2);
    let fit = retired_fit(0.5, 1.0);
    let exact = effectiveness(Arrangement::CrossBothUnmixed, 0.5, 1.0);
    eprintln!(
        "X5 retired fit at N 0.5 Cr 1: {fit:.6} vs exact {exact:.6} (rel {:.2e})",
        (fit / exact - 1.0).abs()
    );
    assert!((fit / exact - 1.0).abs() > 2e-2);
}
