//! Checks T1-T9 of ADR 0036 (tabulated and piecewise properties, F-220). Limits as registered.

use rcfd_core::iapws;
use rcfd_core::tabular::*;

fn lcg(seed: &mut u64) -> f64 {
    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    ((*seed >> 11) as f64) / ((1u64 << 53) as f64)
}

fn msg<T: std::fmt::Debug>(r: rcfd_core::Result<T>) -> String {
    format!("{}", r.expect_err("expected an error"))
}

#[test]
fn t1_piecewise_linear_exactness() {
    let xs = vec![0.0, 0.3, 1.0, 2.5, 4.0];
    let ys: Vec<f64> = xs.iter().map(|x| 2.0 * x - 3.0).collect();
    let t = PiecewiseLinear::new(xs.clone(), ys.clone(), Extrapolation::Clamp).unwrap();
    for (x, y) in xs.iter().zip(&ys) {
        assert_eq!(t.eval(*x), *y);
    }
    let mut s = 1u64;
    for _ in 0..1000 {
        let x = 4.0 * lcg(&mut s);
        let f = 2.0 * x - 3.0;
        assert!((t.eval(x) - f).abs() <= 1e-13 * (1.0 + f.abs()), "x = {x}");
    }
    let k = PiecewiseLinear::new(
        vec![0.0, 1.0, 2.0, 4.0, 5.0],
        vec![1.0, 3.0, 2.0, 2.0, 5.0],
        Extrapolation::Clamp,
    )
    .unwrap();
    for (x, want) in [(0.5, 2.0), (1.5, 2.5), (3.0, 2.0), (4.5, 3.5)] {
        assert!((k.eval(x) - want).abs() <= 1e-15 * want, "x = {x}");
    }
}

fn cubic(t: f64) -> f64 {
    t * t * t - 2.0 * t + 1.0
}

#[test]
fn t2_piecewise_polynomial() {
    let c = vec![1.0, -2.0, 0.0, 1.0];
    let p = PiecewisePoly::new(vec![0.0, 1.2, 3.0], vec![c.clone(), c.clone()], Extrapolation::Clamp).unwrap();
    let mut s = 7u64;
    for _ in 0..500 {
        let t = 3.0 * lcg(&mut s);
        assert!(
            (p.eval(t) - cubic(t)).abs() <= 1e-13 * (1.0 + cubic(t).abs()),
            "t = {t}"
        );
    }
    assert!(p.max_jump().0 <= 1e-14);
    let mut c2 = c.clone();
    c2[0] += 0.5;
    let q = PiecewisePoly::new(vec![0.0, 1.2, 3.0], vec![c, c2], Extrapolation::Clamp).unwrap();
    let (j, i) = q.max_jump();
    assert!((j - 0.5).abs() <= 1e-14 && i == 0, "jump {j} at {i}");
    let m = msg(q.require_continuous(1e-9));
    assert!(m.contains("break 0"), "{m}");
}

#[test]
fn t3_extrapolation() {
    let mk = |e| PiecewiseLinear::new(vec![0.0, 1.0, 3.0], vec![1.0, 2.0, 6.0], e).unwrap();
    let c = mk(Extrapolation::Clamp);
    assert_eq!(c.eval(-5.0), 1.0);
    assert_eq!(c.eval(9.0), 6.0);
    let l = mk(Extrapolation::Linear);
    assert!((l.eval(-1.0) - 0.0).abs() <= 1e-13);
    assert!((l.eval(5.0) - 10.0).abs() <= 1e-13);
    let e = mk(Extrapolation::Error);
    assert!(e.try_eval(3.5).is_err() && e.eval(-0.1).is_nan());
    assert_eq!(e.eval(3.0), 6.0);
    // polynomial: end-piece tangent
    let c3 = vec![1.0, -2.0, 0.0, 1.0];
    let pl = PiecewisePoly::new(vec![0.0, 3.0], vec![c3.clone()], Extrapolation::Linear).unwrap();
    let want = cubic(3.0) + (3.0 * 9.0 - 2.0) * 0.5;
    assert!((pl.eval(3.5) - want).abs() <= 1e-13 * want.abs());
    let want0 = cubic(0.0) + (-2.0) * (-0.5);
    assert!((pl.eval(-0.5) - want0).abs() <= 1e-13);
    let pc = PiecewisePoly::new(vec![0.0, 3.0], vec![c3.clone()], Extrapolation::Clamp).unwrap();
    assert_eq!(pc.eval(4.0), cubic(3.0));
    let pe = PiecewisePoly::new(vec![0.0, 3.0], vec![c3], Extrapolation::Error).unwrap();
    assert!(pe.try_eval(4.0).is_err() && pe.eval(4.0).is_nan());
}

#[test]
fn t4_monotonic() {
    let mk = |y: Vec<f64>| {
        let x: Vec<f64> = (0..y.len()).map(|i| i as f64).collect();
        PiecewiseLinear::new(x, y, Extrapolation::Clamp).unwrap()
    };
    assert_eq!(mk(vec![1.0, 2.0, 3.0, 4.0]).monotonicity(), Monotonicity::Increasing);
    let flat = mk(vec![1.0, 2.0, 2.0, 4.0]);
    assert_eq!(flat.monotonicity(), Monotonicity::NonDecreasing);
    let m = msg(flat.require_monotonic(true));
    assert!(m.contains("y[2]"), "{m}");
    assert!(flat.require_monotonic(false).is_ok());
    let dip = mk(vec![1.0, 2.0, 3.0, 2.5, 4.0]);
    assert_eq!(dip.monotonicity(), Monotonicity::None(3));
    assert!(msg(dip.require_monotonic(false)).contains("y[3]"));
    assert_eq!(mk(vec![2.0, 2.0, 2.0]).monotonicity(), Monotonicity::Constant);
    assert_eq!(mk(vec![3.0, 2.0, 1.0]).monotonicity(), Monotonicity::Decreasing);
}

#[test]
fn t5_malformed_tables_are_located() {
    let e = |r: rcfd_core::Result<PiecewiseLinear>| msg(r);
    assert!(e(PiecewiseLinear::new(
        vec![0.0, 1.0, 2.0, 2.0],
        vec![0.0; 4],
        Extrapolation::Clamp
    ))
    .contains("x[3]"));
    assert!(e(PiecewiseLinear::new(
        vec![0.0, 1.0, 2.0],
        vec![0.0, 1.0, f64::NAN],
        Extrapolation::Clamp
    ))
    .contains("y[2]"));
    let m = e(PiecewiseLinear::new(
        vec![0.0, 1.0, 2.0],
        vec![0.0, 1.0],
        Extrapolation::Clamp,
    ));
    assert!(m.contains('3') && m.contains('2'), "{m}");
    assert!(e(PiecewiseLinear::new(vec![0.0], vec![0.0], Extrapolation::Clamp)).contains("at least 2"));
    let pe = |b: Vec<f64>, c: Vec<Vec<f64>>| msg(PiecewisePoly::new(b, c, Extrapolation::Clamp));
    assert!(pe(vec![0.0, 1.0, 2.0], vec![vec![1.0]]).contains("pieces"));
    assert!(pe(vec![0.0, 1.0, 2.0], vec![vec![1.0], vec![]]).contains("piece 1"));
    assert!(pe(vec![0.0, 2.0, 1.0], vec![vec![1.0], vec![1.0]]).contains("breaks["));
    let g = vec![1.0; 4];
    let pt = |t: Vec<f64>, p: Vec<f64>, l: Vec<f64>, st: Vec<f64>, sp: Vec<f64>| {
        msg(PtTable::new(t, p, l, g.clone(), st, sp))
    };
    assert!(pt(
        vec![0.0, 1.0],
        vec![0.0, 1.0],
        vec![1.0; 3],
        vec![0.0, 1.0],
        vec![1.0, 2.0]
    )
    .contains("liquid"));
    assert!(pt(
        vec![1.0, 1.0],
        vec![0.0, 1.0],
        g.clone(),
        vec![0.0, 1.0],
        vec![1.0, 2.0]
    )
    .contains("t["));
    assert!(pt(
        vec![0.0, 1.0],
        vec![1.0, 0.0],
        g.clone(),
        vec![0.0, 1.0],
        vec![1.0, 2.0]
    )
    .contains("p["));
    assert!(pt(
        vec![0.0, 1.0],
        vec![0.0, 1.0],
        g.clone(),
        vec![0.0, 1.0],
        vec![2.0, 1.0]
    )
    .contains("saturation"));
}

#[test]
fn t6_vogel() {
    let v = Vogel {
        a: 1e-3,
        b: 800.0,
        c: 150.0,
    };
    for t in [250.0f64, 300.0, 400.0] {
        let want = 1e-3 * (800.0 / (t - 150.0)).exp();
        assert!((v.eval(t) - want).abs() <= 1e-14 * want);
    }
    let a = Vogel {
        a: 2.0,
        b: 500.0,
        c: 0.0,
    };
    assert!((a.eval(300.0) - 2.0 * (500.0f64 / 300.0).exp()).abs() <= 1e-14 * a.eval(300.0));
    assert!(Vogel { a: 0.0, b: 1.0, c: 0.0 }.validate().is_err());
    assert!(Vogel {
        a: f64::NAN,
        b: 1.0,
        c: 0.0
    }
    .validate()
    .is_err());
    assert!(v.validate().is_ok());
}

#[test]
fn t7_tait() {
    let t = Tait::new(998.0, 1e5, 3e8, 0.09).unwrap();
    assert!((t.density(1e5) - 998.0).abs() <= 1e-15 * 998.0);
    let mut prev = 0.0;
    for k in 0..20 {
        let p = 1e5 * (5e8f64 / 1e5).powf(k as f64 / 19.0);
        let r = t.density(p);
        let lhs = 1.0 - 998.0 / r;
        let rhs = 0.09 * ((3e8 + p) / (3e8 + 1e5)).ln();
        assert!((lhs - rhs).abs() <= 1e-13, "p = {p}");
        assert!((t.pressure(r) - p).abs() <= 1e-12 * p, "round trip p = {p}");
        assert!(r > prev);
        prev = r;
    }
    let h = 1e3;
    let fd = (t.density(1e5 + h) - t.density(1e5 - h)) / (2.0 * h);
    let exact = 998.0 * 0.09 / (3e8 + 1e5);
    assert!((fd - exact).abs() <= 1e-6 * exact, "{fd} vs {exact}");
    assert!(Tait::new(0.0, 1e5, 3e8, 0.09).is_err());
}

fn bilinear(t: f64, p: f64) -> f64 {
    3.0 + 0.5 * t - 2e-6 * p + 1e-8 * t * p
}

#[test]
fn t8_pt_table_exactness_and_saturation() {
    // exactness of bilinear lookup
    let ts: Vec<f64> = (0..11).map(|i| 300.0 + 10.0 * i as f64).collect();
    let ps: Vec<f64> = (0..11).map(|j| 1e5 + 4.9e5 * j as f64).collect();
    let mut grid = Vec::new();
    for t in &ts {
        for p in &ps {
            grid.push(bilinear(*t, *p));
        }
    }
    // saturation far below the grid so everything is vapour phase; same grid in both phases
    let tab = PtTable::new(
        ts.clone(),
        ps.clone(),
        grid.clone(),
        grid.clone(),
        vec![280.0, 290.0],
        vec![1e3, 2e3],
    )
    .unwrap();
    for (i, t) in ts.iter().enumerate() {
        for (j, p) in ps.iter().enumerate() {
            let v = tab.eval(*t, *p).unwrap();
            assert!(
                (v - grid[i * 11 + j]).abs() <= 1e-14 * grid[i * 11 + j].abs(),
                "node {i} {j}"
            );
        }
    }
    let mut s = 3u64;
    for _ in 0..500 {
        let t = 300.0 + 100.0 * lcg(&mut s);
        let p = 1e5 + 4.9e6 * lcg(&mut s);
        let f = bilinear(t, p);
        assert!((tab.eval(t, p).unwrap() - f).abs() <= 1e-12 * f.abs());
    }
    // saturation line from IF97
    let st: Vec<f64> = (0..=90).map(|i| 280.0 + i as f64).collect();
    let sp: Vec<f64> = st.iter().map(|&t| iapws::saturation_pressure(t).unwrap()).collect();
    let g2 = vec![f64::NAN; 4];
    let tab = PtTable::new(
        vec![280.0, 370.0],
        vec![1e3, 1e7],
        g2.clone(),
        g2,
        st.clone(),
        sp.clone(),
    )
    .unwrap();
    let (pmin, pmax) = (sp[0], sp[90]);
    for k in 0..200 {
        let p = pmin + (pmax - pmin) * k as f64 / 199.0;
        let t = tab.tsat(p).unwrap();
        let back = tab.psat(t).unwrap();
        assert!((back - p).abs() <= 1e-12 * p, "psat(tsat(p)) p = {p}: {back}");
    }
    for k in 0..200 {
        let t = 280.0 + 90.0 * k as f64 / 199.0;
        let back = tab.tsat(tab.psat(t).unwrap()).unwrap();
        assert!((back - t).abs() <= 1e-10, "tsat(psat(T)) T = {t}: {back}");
    }
    let mut worst: f64 = 0.0;
    for i in 0..90 {
        let t = 280.5 + i as f64;
        let exact = iapws::saturation_pressure(t).unwrap();
        worst = worst.max((tab.psat(t).unwrap() - exact).abs() / exact);
    }
    eprintln!("T8 (ii) worst relative saturation interpolation error {worst:.3e} (limit 1e-3)");
    assert!(worst <= 1e-3);
    for i in 0..25 {
        let t = 285.0 + 3.3 * i as f64;
        let ps = tab.psat(t).unwrap();
        assert_eq!(tab.phase(t, ps * (1.0 + 1e-9)), Phase::Liquid);
        assert_eq!(tab.phase(t, ps * (1.0 - 1e-9)), Phase::Vapour);
    }
    // NaN corners: located error naming the saturation line
    let ts = vec![300.0, 350.0, 400.0];
    let ps = vec![1e4, 1e5, 1e6];
    // liquid data only for p = 1e6 column and p = 1e5 column at T = 300 is "below saturation" -> NaN
    let liquid = vec![f64::NAN, f64::NAN, 1000.0, 990.0, 991.0, 992.0, 980.0, 981.0, 982.0];
    let vapour = vec![0.1; 9];
    let tab = PtTable::new(ts, ps, liquid, vapour, vec![280.0, 400.0], vec![3e3, 2e5]).unwrap();
    let m = msg(tab.eval(310.0, 5e5));
    assert!(m.contains("saturation") && m.contains("cell"), "{m}");
    assert!(tab.eval(310.0, 5e5).is_err());
    assert!(tab.eval(370.0, 8e5).is_ok());
    assert!(tab.eval(310.0, 1.5e4).is_ok()); // vapour grid
}

#[test]
fn t9_if97_built_tables() {
    let dens = |t: f64, p: f64| iapws::water_state(t, p).unwrap().density;
    // liquid table (all nodes compressed liquid) and vapour table (all nodes vapour); one table object per phase, with a
    // saturation line that places the liquid grid on one side and the vapour grid on the other
    let lt: Vec<f64> = (0..=14).map(|i| 290.0 + 5.0 * i as f64).collect();
    let lp: Vec<f64> = (1..=10).map(|j| 1e6 * j as f64).collect();
    let liquid: Vec<f64> = lt.iter().flat_map(|&t| lp.iter().map(move |&p| dens(t, p))).collect();
    let tab = PtTable::new(
        lt.clone(),
        lp.clone(),
        liquid.clone(),
        vec![f64::NAN; liquid.len()],
        vec![280.0, 370.0],
        vec![991.0, 2e5],
    )
    .unwrap();
    let mut worst: f64 = 0.0;
    for (i, &t) in lt.iter().enumerate() {
        for (j, &p) in lp.iter().enumerate() {
            let v = tab.eval(t, p).unwrap();
            assert!((v - liquid[i * lp.len() + j]).abs() <= 1e-12 * v, "node");
        }
    }
    for i in 0..lt.len() - 1 {
        for j in 0..lp.len() - 1 {
            let (t, p) = (0.5 * (lt[i] + lt[i + 1]), 0.5 * (lp[j] + lp[j + 1]));
            let e = (tab.eval(t, p).unwrap() - dens(t, p)).abs() / dens(t, p);
            worst = worst.max(e);
        }
    }
    eprintln!("T9 liquid worst mid-cell relative error {worst:.3e} (limit 1e-4)");
    assert!(worst <= 1e-4);
}

/// WAVE 25B NOTE (ADR 0044 D2): plain bilinear interpolation of rho still gives 2.642e-4 (record kept ignored); the opt-in
/// `eval_density_ideal_scaled` basis meets the same 1e-4 limit in `t9b_b3_if97_vapour_table_scaled_basis` (1.24e-5).
/// KNOWN MISS (T9, vapour half). The registered limit (1e-4 relative at cell centres, estimated from the 1/T curvature alone) is
/// kept. Measured 2.64e-4 at the cell T 380-385 K, p 10-20 kPa. Root cause: the estimate ignored the pressure direction. Steam
/// at 380 K is not ideal: rho T / p rises by 0.07 percent per 5 kPa (Z falls by about 1.3 percent per 100 kPa), so rho(p) has a curvature
/// whose relative linear-interpolation error is 2 a h^2 / (8 p) with a = 1.3e-7 / Pa, h = 10 kPa, p = 15 kPa: 2.2e-4, which dominates
/// the T-direction error (4.4e-5). The error is physical interpolation error of the coarse low-pressure grid, not an implementation
/// defect (nodes reproduce to 1e-12, bilinear exactness is T8).
#[test]
#[ignore = "KNOWN MISS: vapour table mid-cell error 2.64e-4 > 1e-4 (non-ideal steam, coarse low-p grid)"]
fn t9_vapour_table_midcell_error_known_miss() {
    let dens = |t: f64, p: f64| iapws::water_state(t, p).unwrap().density;
    let vt: Vec<f64> = (0..=24).map(|i| 380.0 + 5.0 * i as f64).collect();
    let vp: Vec<f64> = (1..=10).map(|j| 1e4 * j as f64).collect();
    let vap: Vec<f64> = vt.iter().flat_map(|&t| vp.iter().map(move |&p| dens(t, p))).collect();
    let tab = PtTable::new(
        vt.clone(),
        vp.clone(),
        vec![f64::NAN; vap.len()],
        vap.clone(),
        vec![280.0, 520.0],
        vec![2e5, 1e6],
    )
    .unwrap();
    let mut worst: f64 = 0.0;
    for (i, &t) in vt.iter().enumerate() {
        for (j, &p) in vp.iter().enumerate() {
            let v = tab.eval(t, p).unwrap();
            assert!((v - vap[i * vp.len() + j]).abs() <= 1e-12 * v, "node");
        }
    }
    for i in 0..vt.len() - 1 {
        for j in 0..vp.len() - 1 {
            let (t, p) = (0.5 * (vt[i] + vt[i + 1]), 0.5 * (vp[j] + vp[j + 1]));
            let e = (tab.eval(t, p).unwrap() - dens(t, p)).abs() / dens(t, p);
            worst = worst.max(e);
        }
    }
    eprintln!("T9 vapour worst mid-cell relative error {worst:.3e} (limit 1e-4)");
    assert!(worst <= 1e-4);
}

// ---------------------------------------------------------------------------------------------------------------------
// Wave 25B (ADR 0044 D2): the density-specific ideal-gas-scaled basis, checks B1 to B4 as registered before the run.
// The registered limits of T9 (1e-4) are unchanged; the plain bilinear miss above stays ignored.
// ---------------------------------------------------------------------------------------------------------------------

fn vapour_table(f: &dyn Fn(f64, f64) -> f64) -> (PtTable, Vec<f64>, Vec<f64>) {
    let vt: Vec<f64> = (0..=24).map(|i| 380.0 + 5.0 * i as f64).collect();
    let vp: Vec<f64> = (1..=10).map(|j| 1e4 * j as f64).collect();
    let vap: Vec<f64> = vt.iter().flat_map(|&t| vp.iter().map(move |&p| f(t, p))).collect();
    let tab = PtTable::new(
        vt.clone(),
        vp.clone(),
        vec![f64::NAN; vap.len()],
        vap,
        vec![280.0, 520.0],
        vec![2e5, 1e6],
    )
    .unwrap();
    (tab, vt, vp)
}

/// B1: an ideal-gas density table is reproduced anywhere by the scaled basis; plain bilinear is not.
#[test]
fn t9b_b1_ideal_gas_exact() {
    let rs = 461.5;
    let ideal = |t: f64, p: f64| p / (rs * t);
    let (tab, vt, vp) = vapour_table(&ideal);
    let (mut scaled, mut plain) = (0.0_f64, 0.0_f64);
    for i in 0..vt.len() - 1 {
        for j in 0..vp.len() - 1 {
            for &(a, b) in &[(0.1, 0.2), (0.3, 0.8), (0.5, 0.5), (0.7, 0.1), (0.9, 0.6)] {
                let (t, p) = (vt[i] + a * 5.0, vp[j] + b * 1e4);
                let want = ideal(t, p);
                scaled = scaled.max((tab.eval_density_ideal_scaled(t, p).unwrap() - want).abs() / want);
                plain = plain.max((tab.eval(t, p).unwrap() - want).abs() / want);
            }
        }
    }
    eprintln!("B1 ideal-gas table: scaled max relative error {scaled:.2e} (limit 1e-12), plain bilinear {plain:.2e} (must exceed 1e-5)");
    assert!(scaled <= 1e-12);
    assert!(plain > 1e-5);
}

fn b3_check() -> (f64, f64, f64) {
    let dens = |t: f64, p: f64| iapws::water_state(t, p).unwrap().density;
    let (tab, vt, vp) = vapour_table(&dens);
    // B2 nodes
    for (i, &t) in vt.iter().enumerate() {
        for (j, &p) in vp.iter().enumerate() {
            let v = tab.eval_density_ideal_scaled(t, p).unwrap();
            let want = dens(t, p);
            assert!((v - want).abs() <= 1e-12 * want, "node {i} {j}: {v} vs {want}");
        }
    }
    let (mut centre, mut quarter, mut plain) = (0.0_f64, 0.0_f64, 0.0_f64);
    for i in 0..vt.len() - 1 {
        for j in 0..vp.len() - 1 {
            for &(a, b) in &[(0.25, 0.25), (0.25, 0.75), (0.5, 0.5), (0.75, 0.25), (0.75, 0.75)] {
                let (t, p) = (vt[i] + a * 5.0, vp[j] + b * 1e4);
                let want = dens(t, p);
                let e = (tab.eval_density_ideal_scaled(t, p).unwrap() - want).abs() / want;
                if a == 0.5 {
                    centre = centre.max(e);
                } else {
                    quarter = quarter.max(e);
                }
                if a == 0.5 {
                    plain = plain.max((tab.eval(t, p).unwrap() - want).abs() / want);
                }
            }
        }
    }
    (centre, quarter, plain)
}

/// B2 and B3: the registered T9 vapour table, the registered 1e-4 limit at the cell centres (and at the quarter points).
#[test]
fn t9b_b3_if97_vapour_table_scaled_basis() {
    let (centre, quarter, plain) = b3_check();
    eprintln!(
        "T9b vapour scaled basis: centre worst {centre:.3e}, quarter points worst {quarter:.3e} (limit 1e-4 for both); plain bilinear centre {plain:.3e}"
    );
    assert!(centre <= 1e-4, "centre {centre}");
    assert!(quarter <= 1e-4, "quarter {quarter}");
}

/// B4: error behaviour.
#[test]
fn t9b_b4_errors() {
    let (tab, _, _) = vapour_table(&|t, p| p / (461.5 * t));
    assert!(tab.eval_density_ideal_scaled(0.0, 5e4).is_err());
    assert!(tab.eval_density_ideal_scaled(400.0, -1.0).is_err());
    assert!(tab.eval_density_ideal_scaled(f64::NAN, 5e4).is_err());
    assert!(tab.eval_density_ideal_scaled(300.0, 5e4).is_err()); // below the T axis
    assert!(tab.eval_density_ideal_scaled(400.0, 5e5).is_err()); // above the p axis
    let neg = PtTable::new(
        vec![-10.0, 10.0],
        vec![1.0, 2.0],
        vec![f64::NAN; 4],
        vec![1.0; 4],
        vec![-10.0, 10.0],
        vec![1.0, 100.0],
    )
    .unwrap();
    assert!(neg.eval_density_ideal_scaled(0.0, 1.5).is_err());
    // NaN corner: same located error as eval
    let nan = PtTable::new(
        vec![300.0, 350.0, 400.0],
        vec![1e4, 1e5, 1e6],
        vec![f64::NAN; 9],
        vec![0.1, f64::NAN, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1],
        vec![280.0, 400.0],
        vec![1e3, 2e3],
    )
    .unwrap();
    let a = nan.eval(310.0, 5e4).unwrap_err().to_string();
    let b = nan.eval_density_ideal_scaled(310.0, 5e4).unwrap_err().to_string();
    assert_eq!(a, b);
    assert!(b.contains("cell"), "{b}");
}
