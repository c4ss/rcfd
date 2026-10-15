//! ADR 0027 library-level checks K2a, K2c (library), K3, K6, K6b, K7, using the toy mechanisms of `rcfd_reactor::toy`.
//! Limits are those registered in the ADR before any run.

use rcfd_reactor::cell::*;
use rcfd_reactor::mech::{Mechanism, R_GAS};
use rcfd_reactor::reactors::*;
use rcfd_reactor::toy;

const W: f64 = 0.020;

fn m2() -> Mechanism {
    toy::isomer(1.0e8, 19872.0, -3000.0, false)
}

fn tool_state_after(mech: &Mechanism, rho: f64, y0: &[f64], dt: f64) -> Vec<f64> {
    // constant volume at density rho: p = rho R T / W_mean
    let p = rho * R_GAS * y0[0] / mech.mean_w(&y0[1..]);
    let tr = run_batch(
        mech,
        BatchKind::ConstantVolume,
        Energy::Adiabatic,
        p,
        y0,
        dt,
        Tol::default(),
        Stop::EndTime,
    )
    .expect("toolkit step");
    tr.last().to_vec()
}

#[test]
fn k2a_cell_steps_equal_the_toolkit() {
    let mech = m2();
    let rho = 1.5;
    let cv = 1.5 * R_GAS / W;
    let dt = 1e-5;
    let n = 200;
    let t0 = 800.0;
    let dt_ad = R_GAS * 3000.0 / (W * cv);
    assert!((dt_ad - 2000.0).abs() < 1e-9, "hand DeltaT_ad {dt_ad}");
    let cell = CellModel {
        mech: &mech,
        rho,
        cp: Some(cv),
        closure: Closure::Kinetic,
        tol: Tol::default(),
    };
    // toolkit trajectory with Hermite dense output
    let p = rho * R_GAS * t0 / W;
    let tr = run_batch(
        &mech,
        BatchKind::ConstantVolume,
        Energy::Adiabatic,
        p,
        &[t0, 1.0, 0.0],
        n as f64 * dt,
        Tol::default(),
        Stop::EndTime,
    )
    .unwrap();
    let hermite = |t: f64, comp: usize| -> f64 {
        let i = tr.t.partition_point(|&x| x < t).clamp(1, tr.t.len() - 1);
        let (t0, t1) = (tr.t[i - 1], tr.t[i]);
        let mut f0 = vec![0.0; 3];
        let mut f1 = vec![0.0; 3];
        batch_rhs(
            &mech,
            BatchKind::ConstantVolume,
            Energy::Adiabatic,
            rho,
            &tr.state[i - 1],
            &mut f0,
        );
        batch_rhs(
            &mech,
            BatchKind::ConstantVolume,
            Energy::Adiabatic,
            rho,
            &tr.state[i],
            &mut f1,
        );
        let h = t1 - t0;
        let s = (t - t0) / h;
        let (s2, s3) = (s * s, s * s * s);
        (2.0 * s3 - 3.0 * s2 + 1.0) * tr.state[i - 1][comp]
            + (s3 - 2.0 * s2 + s) * h * f0[comp]
            + (-2.0 * s3 + 3.0 * s2) * tr.state[i][comp]
            + (s3 - s2) * h * f1[comp]
    };
    let mut st = vec![t0, 1.0, 0.0];
    let mut st_restart = st.clone();
    let (mut worst_t, mut worst_y, mut worst_tr, mut worst_yr) = (0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64);
    let (mut idx_cell, mut idx_restart) = (None, None);
    for step in 1..=n {
        cell.advance(&mut st, dt).unwrap();
        st_restart = tool_state_after(&mech, rho, &st_restart, dt);
        let t = step as f64 * dt;
        worst_t = worst_t.max((st[0] - hermite(t, 0)).abs() / dt_ad);
        worst_y = worst_y.max((st[1] - hermite(t, 1)).abs());
        worst_tr = worst_tr.max((st[0] - st_restart[0]).abs() / dt_ad);
        worst_yr = worst_yr.max((st[1] - st_restart[1]).abs());
        if idx_cell.is_none() && st[0] > t0 + 0.5 * dt_ad {
            idx_cell = Some(step);
        }
        if idx_restart.is_none() && st_restart[0] > t0 + 0.5 * dt_ad {
            idx_restart = Some(step);
        }
    }
    println!(
        "K2a Hermite: max dT/DeltaT {worst_t:.3e} (limit 1e-5), max dY {worst_y:.3e} (limit 1e-6); restart: dT/DeltaT {worst_tr:.3e} (limit 1e-7), dY {worst_yr:.3e}; half-rise step cell {idx_cell:?} restart {idx_restart:?}"
    );
    assert!(idx_cell.is_some(), "no ignition in the run");
    assert_eq!(idx_cell, idx_restart);
    assert!(worst_t <= 1e-5, "K2a Hermite T {worst_t:e}");
    assert!(worst_y <= 1e-6, "K2a Hermite Y {worst_y:e}");
    assert!(worst_tr <= 1e-7, "K2a restart T {worst_tr:e}");
}

#[test]
fn k2c_adiabatic_flame_temperature_library() {
    let mech = toy::isomer(1.0e8, 19872.0, -3000.0, true);
    let cp = 1000.0;
    let cell = CellModel {
        mech: &mech,
        rho: 1.0,
        cp: Some(cp),
        closure: Closure::Kinetic,
        tol: Tol::default(),
    };
    let mut st = vec![900.0, 0.2, 0.0, 0.8];
    cell.advance(&mut st, 0.05).unwrap();
    let hand = 900.0 + 0.2 * R_GAS * 3000.0 / (W * cp);
    println!("K2c T_end {:.8} hand {hand:.8} Y_A {:.3e}", st[0], st[1]);
    assert!((hand - 1149.4339).abs() < 1e-3, "hand arithmetic {hand}");
    assert!((st[0] - hand).abs() <= 1e-4, "K2c T_end");
    assert!(st[1] <= 1e-12);
}

fn conservation(mech: &Mechanism, mut st: Vec<f64>, rho: f64, cp: Option<f64>, dt: f64, n: usize) -> (f64, f64) {
    let cell = CellModel {
        mech,
        rho,
        cp,
        closure: Closure::Kinetic,
        tol: Tol::default(),
    };
    let z0 = mech.element_mass_fractions(&st[1..]);
    let (mut worst_sum, mut worst_el) = (0.0_f64, 0.0_f64);
    for _ in 0..n {
        cell.advance(&mut st, dt).unwrap();
        let s: f64 = st[1..].iter().sum();
        worst_sum = worst_sum.max((s - 1.0).abs());
        let z = mech.element_mass_fractions(&st[1..]);
        for (a, b) in z.iter().zip(&z0) {
            if *b > 0.0 {
                worst_el = worst_el.max((a - b).abs() / b);
            }
        }
    }
    (worst_sum, worst_el)
}

#[test]
fn k3_mass_and_element_conservation() {
    let (m, t) = toy::exchange_texts();
    let m3 = Mechanism::from_chemkin(&m, Some(&t)).unwrap();
    assert_eq!(m3.max_element_imbalance(), 0.0);
    let (s, e) = conservation(&m3, vec![1500.0, 0.3, 0.4, 0.1, 0.2], 1.2, Some(1000.0), 1e-4, 50);
    println!("K3 M3: sum drift {s:.3e}, element drift {e:.3e} (limit 1e-10)");
    assert!(s <= 1e-10 && e <= 1e-10, "K3 M3 {s:e} {e:e}");
    let m2i = toy::isomer(1.0e8, 19872.0, -3000.0, true);
    let (s, e) = conservation(&m2i, vec![900.0, 0.2, 0.0, 0.8], 1.0, Some(1000.0), 1e-3, 50);
    println!("K3 M2i: sum drift {s:.3e}, element drift {e:.3e} (limit 1e-10)");
    assert!(s <= 1e-10 && e <= 1e-10, "K3 M2i {s:e} {e:e}");
}

fn burner() -> Mechanism {
    let (m, t) = toy::burner_texts();
    Mechanism::from_chemkin(&m, Some(&t)).unwrap()
}

fn ed(a: f64, b: f64, e: f64) -> EdParams {
    EdParams {
        a,
        b,
        eps_over_k: e,
        product_limit: true,
    }
}

#[test]
fn k6_eddy_dissipation_equals_hand_formula() {
    let m = burner();
    assert_eq!(m.max_element_imbalance(), 0.0);
    let p = ed(4.0, 0.5, 50.0);
    let rho = 1.2;
    let (wf, wo, wp) = (0.024, 0.032, 0.060);
    for (y, expect, label) in [
        ([0.01, 0.3, 0.2, 0.49], 100.0, "fuel"),
        ([0.2, 0.03, 0.2, 0.57], 75.0, "oxidiser"),
        ([0.2, 0.3, 0.01, 0.49], 10.0, "product"),
    ] {
        let q = eddy_dissipation_progress(&m, 0, rho, &y, &p);
        // independent hand formula
        let hand = 4.0 * rho * 50.0 * f64::min(f64::min(y[0] / (1.0 * wf), y[1] / (3.0 * wo)), 0.5 * y[2] / (2.0 * wp));
        let rel = (q - hand).abs() / hand;
        let rel2 = (q - expect).abs() / expect;
        println!(
            "K6 {label}-limited: q {q:.12e} hand {hand:.12e} rel {rel:.2e}, vs ADR number {expect} rel {rel2:.2e}"
        );
        assert!(rel <= 1e-12 && rel2 <= 1e-12);
        // net production rates through the cell model
        let cell = CellModel {
            mech: &m,
            rho,
            cp: None,
            closure: Closure::EddyDissipation(p),
            tol: Tol::default(),
        };
        let st = [1000.0, y[0], y[1], y[2], y[3]];
        let mut wdot = [0.0; 4];
        let mut qq = [0.0; 1];
        cell.production(&st, &mut wdot, &mut qq);
        assert!((wdot[0] + q).abs() <= 1e-12 * q);
        assert!((wdot[1] + 3.0 * q).abs() <= 1e-12 * q);
        assert!((wdot[2] - 2.0 * q).abs() <= 1e-12 * q);
        assert_eq!(wdot[3], 0.0);
        let dm: f64 = wdot.iter().zip(&m.species).map(|(w, s)| w * s.w).sum();
        assert!(dm.abs() <= 1e-12 * q * wp, "mass {dm:e}");
    }
    // zero mixing rate: exactly zero
    assert_eq!(
        eddy_dissipation_progress(&m, 0, rho, &[0.2, 0.3, 0.1, 0.4], &ed(4.0, 0.5, 0.0)),
        0.0
    );
    // no products: zero with the product term, reactant-only form gives the other two terms
    let y = [0.2, 0.3, 0.0, 0.5];
    assert_eq!(eddy_dissipation_progress(&m, 0, rho, &y, &p), 0.0);
    let mut p2 = p;
    p2.product_limit = false;
    let q = eddy_dissipation_progress(&m, 0, rho, &y, &p2);
    let hand = 4.0 * rho * 50.0 * f64::min(0.2 / wf, 0.3 / (3.0 * wo));
    assert!((q - hand).abs() <= 1e-12 * hand);
}

#[test]
fn k6b_eddy_dissipation_decay_is_exponential() {
    let m = burner();
    let p = ed(4.0, 0.5, 50.0);
    let cell = CellModel {
        mech: &m,
        rho: 1.2,
        cp: None,
        closure: Closure::EddyDissipation(p),
        tol: Tol::default(),
    };
    let mut st = vec![1000.0, 0.01, 0.3, 0.2, 0.49];
    let mut worst = 0.0_f64;
    let dt = 0.002;
    for i in 1..=10 {
        cell.advance(&mut st, dt).unwrap();
        let exact = 0.01 * (-4.0 * 50.0 * i as f64 * dt).exp();
        worst = worst.max((st[1] - exact).abs());
    }
    println!("K6b max |Y_FU - exact| {worst:.3e} (limit 1e-9)");
    assert!(worst <= 1e-9);
}

#[test]
fn k7_edc_scales_and_relaxation() {
    let (nu, k, eps) = (1.5e-5, 1.0, 10.0);
    let (xi, tau, m) = edc_scales(nu, k, eps, 2.1377, 0.4082).unwrap();
    // independently coded
    let xi2 = 2.1377 * (nu * eps / (k * k)).powf(0.25);
    let tau2 = 0.4082 * (nu / eps).powf(0.5);
    let m2 = xi2 * xi2 / (tau2 * (1.0 - xi2.powi(3)));
    println!("K7a xi {xi:.10} tau {tau:.10e} m {m:.8}");
    assert!((xi - xi2).abs() <= 1e-12 * xi2 && (tau - tau2).abs() <= 1e-12 * tau2 && (m - m2).abs() <= 1e-12 * m2);
    assert!((xi - 0.23658).abs() <= 1e-3 * 0.23658);
    assert!((tau - 4.999e-4).abs() <= 1e-3 * 4.999e-4);
    assert!((m - 113.5).abs() <= 1e-3 * 113.5);
    assert!(edc_scales(nu, 0.0, eps, 2.1377, 0.4082).is_none());
    assert!(edc_scales(nu, k, 0.0, 2.1377, 0.4082).is_none());
    // K7b: first-order isothermal, Y* = Y exp(-k tau*)
    let mech = toy::isomer(5.0, 0.0, 0.0, false);
    let cell = CellModel {
        mech: &mech,
        rho: 1.3,
        cp: None,
        closure: Closure::Kinetic,
        tol: Tol::default(),
    };
    let st = vec![1000.0, 0.6, 0.4];
    let fs = cell.fine_structure(&st, tau).unwrap();
    let exact = 0.6 * (-5.0 * tau).exp();
    let rel = (fs[1] - exact).abs() / exact;
    // source rho m (Y* - Y)
    let src = 1.3 * m * (fs[1] - 0.6);
    let src_exact = 1.3 * m * 0.6 * ((-5.0 * tau).exp() - 1.0);
    let rel_src = (src - src_exact).abs() / src_exact.abs();
    println!("K7b Y* rel {rel:.3e}, source rel {rel_src:.3e} (limit 1e-6)");
    assert!(rel <= 1e-6 && rel_src <= 1e-6);
    // K7c
    let y1 = relax_towards(0.3, 0.1, m, 0.01);
    let hand = 0.1 + 0.2 * (-m * 0.01).exp();
    assert!((y1 - hand).abs() <= 1e-14);
    assert!((0.0..=1.0).contains(&y1));
    assert_eq!(relax_towards(0.37, 0.37, m, 0.5).to_bits(), 0.37_f64.to_bits());
}
