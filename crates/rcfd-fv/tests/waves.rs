//! Checks W1-W7 of ADR 0040: wave kinematics, irregular waves, relaxation weights and the 1-D reflection toy.

use rcfd_fv::waves::*;
use std::f64::consts::{E, PI};

const G: f64 = 9.81;

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs().max(1e-300)
}

/// omega = 1 and depth h such that k h = x.
fn wave_with_kh(x: f64, a: f64, order: u8) -> Wave {
    let h = G * x * x.tanh();
    let w = Wave::new(a, 1.0, h, G, 0.0, order).unwrap();
    assert!((w.k * w.h - x).abs() <= 1e-10 * x, "{} vs {x}", w.k * w.h);
    w
}

#[test]
fn w1_dispersion() {
    let mut n_deep = 0;
    let mut n_shallow = 0;
    let mut worst: f64 = 0.0;
    for h in [0.5, 2.0, 10.0, 50.0] {
        for t in [2.0, 5.0, 9.0, 15.0] {
            let omega = 2.0 * PI / t;
            let k = wavenumber(omega, h, G).unwrap();
            let resid = (omega * omega - G * k * (k * h).tanh()).abs() / (omega * omega);
            worst = worst.max(resid);
            assert!(resid <= 1e-12, "h = {h}, T = {t}: {resid}");
            let kh = k * h;
            if kh >= 6.0 {
                n_deep += 1;
                assert!(
                    (k - omega * omega / G).abs() / k <= 2.1 * (-2.0 * kh).exp(),
                    "deep h = {h}, T = {t}"
                );
            }
            if kh <= 0.2 {
                n_shallow += 1;
                let dev = 1.0 - (omega / k) / (G * h).sqrt();
                assert!(
                    dev <= kh * kh / 6.0 && dev >= kh * kh / 6.0 - 0.06 * kh.powi(4),
                    "shallow h = {h}, T = {t}: {dev}"
                );
                assert!(1.0 - group_velocity(omega, k, h) / (omega / k) <= kh * kh);
            }
            // c_g = d omega / dk by central difference of omega(k) = sqrt(g k tanh(kh))
            let om = |kk: f64| (G * kk * (kk * h).tanh()).sqrt();
            let dk = 1e-5 * k;
            let cg_fd = (om(k + dk) - om(k - dk)) / (2.0 * dk);
            assert!(rel(group_velocity(omega, k, h), cg_fd) <= 1e-8, "cg h = {h}, T = {t}");
        }
    }
    println!("w1: worst dispersion residual {worst:e}, deep cases {n_deep}, shallow cases {n_shallow}");
    assert!(n_deep >= 1 && n_shallow >= 1);
    assert_eq!(wavenumber(1.3, f64::INFINITY, G).unwrap(), 1.3 * 1.3 / G);
    let h = 6.0 * G; // k0 = 1/G, so kh ~ 6
    let k = wavenumber(1.0, h, G).unwrap();
    let cg = group_velocity(1.0, k, h);
    assert!((cg / (1.0 / k) - 0.5).abs() <= 2e-4, "{}", cg / (1.0 / k));
    assert!(wavenumber(0.0, 1.0, G).unwrap_err().contains("omega"));
    assert!(wavenumber(1.0, -1.0, G).unwrap_err().contains('h'));
    assert!(wavenumber(1.0, 1.0, 0.0).unwrap_err().contains('g'));
}

#[test]
fn w2_airy_kinematics_and_invariants() {
    let (a, h, period) = (0.5, 3.0, 6.0);
    let w = Wave::new(a, 2.0 * PI / period, h, G, 0.3, 1).unwrap();
    let (lam, per) = (w.wavelength(), w.period());
    for i in 0..20 {
        let (x, t, z) = (0.37 * i as f64, 0.11 * i as f64, -0.1 * i as f64 * 0.14);
        assert!((w.eta(x + lam, t) - w.eta(x, t)).abs() <= 1e-13);
        assert!((w.eta(x, t + per) - w.eta(x, t)).abs() <= 1e-13);
        let (u, ww) = w.velocity(x, z, t);
        let (u2, w2) = w.velocity(x + lam, z, t + per);
        assert!((u - u2).abs() <= 1e-13 && (ww - w2).abs() <= 1e-13);
        // kinematic boundary condition at z = 0 (linear)
        let (_, w0) = w.velocity(x, 0.0, t);
        assert!((w.eta_t(x, t) - w0).abs() <= 1e-12);
        // eta_t central difference
        let d = 1e-5;
        let fd = (w.eta(x, t + d) - w.eta(x, t - d)) / (2.0 * d);
        assert!((fd - w.eta_t(x, t)).abs() <= 1e-8);
        // continuity and potential relations (central differences, step 1e-5)
        let du_dx = (w.velocity(x + d, z, t).0 - w.velocity(x - d, z, t).0) / (2.0 * d);
        let dw_dz = (w.velocity(x, z + d, t).1 - w.velocity(x, z - d, t).1) / (2.0 * d);
        assert!((du_dx + dw_dz).abs() <= 1e-8 * a * w.omega * w.k, "{}", du_dx + dw_dz);
        assert!((dw_dz - w.dw_dz(x, z, t)).abs() <= 1e-8 * a * w.omega * w.k);
        let phi_x = (w.potential(x + d, z, t) - w.potential(x - d, z, t)) / (2.0 * d);
        let phi_z = (w.potential(x, z + d, t) - w.potential(x, z - d, t)) / (2.0 * d);
        assert!((phi_x - u).abs() <= 1e-8 * a * w.omega && (phi_z - ww).abs() <= 1e-8 * a * w.omega);
    }
    // bed
    for i in 0..10 {
        let (_, wb) = w.velocity(0.7 * i as f64, -h, 0.3 * i as f64);
        assert!(wb.abs() <= 1e-15 * a * w.omega);
    }
    // orbital ratio
    let k = w.k;
    for z in [-0.3, -1.0, -2.0, -2.9] {
        assert!(rel(w.orbital_ratio(z), (k * (z + h)).cosh() / (k * h).cosh()) <= 1e-14);
    }
    let deep = wave_with_kh(6.0, 0.1, 1);
    for f in [0.0, 0.05, 0.1, 0.2, 0.25] {
        let z = -f * deep.h;
        let r = deep.orbital_ratio(z);
        let bound = 2.0 * (-2.0 * deep.k * (z + deep.h)).exp();
        assert!((r - (deep.k * z).exp()).abs() / (deep.k * z).exp() <= bound, "z = {z}");
    }
    // time-mean kinetic energy integrated over depth = rho g a^2 / 4 (rho = 1)
    let n = 2000;
    let dz = h / n as f64;
    let mut ke_mean = 0.0;
    let nt = 64;
    for it in 0..nt {
        let t = per * it as f64 / nt as f64;
        let f = |z: f64| {
            let (u, ww) = w.velocity(0.0, z, t);
            0.5 * (u * u + ww * ww)
        };
        let mut s = f(-h) + f(0.0);
        for i in 1..n {
            s += f(-h + i as f64 * dz) * if i % 2 == 1 { 4.0 } else { 2.0 };
        }
        ke_mean += s * dz / 3.0 / nt as f64;
    }
    assert!(rel(ke_mean, G * a * a / 4.0) <= 1e-8, "{ke_mean} {}", G * a * a / 4.0);
    // mean eta u(0) = E/(rho c)
    let mean: f64 = (0..nt)
        .map(|it| {
            let t = per * it as f64 / nt as f64;
            w.eta(0.0, t) * w.velocity(0.0, 0.0, t).0
        })
        .sum::<f64>()
        / nt as f64;
    assert!(rel(mean, w.energy_density(1.0) / w.celerity()) <= 1e-12, "{mean}");
    assert!(rel(mean, w.mean_volume_flux()) <= 1e-12);
    assert!(
        rel(
            w.dynamic_pressure(1000.0, 0.0, -h, 0.0),
            1000.0 * G * a / (k * h).cosh() * w.phase.cos()
        ) <= 1e-13
    );
    assert!(rel(w.energy_flux(1.0), w.energy_density(1.0) * w.group_velocity()) <= 1e-15);
}

#[test]
fn w3_stokes_second_order() {
    // limits of the second harmonic
    let deep = Wave::new(0.1, 1.0, f64::INFINITY, G, 0.0, 2).unwrap();
    assert!(rel(deep.eta2(), deep.k * 0.01 / 2.0) <= 1e-6);
    let d12 = wave_with_kh(12.0, 0.1, 2);
    assert!(
        rel(d12.eta2(), d12.k * 0.01 / 2.0) <= 1e-6,
        "{}",
        d12.eta2() / (d12.k * 0.005)
    );
    let sh = wave_with_kh(0.05, 0.001, 2);
    let target = 3.0 * 1e-6 / (4.0 * sh.k * sh.k * sh.h.powi(3));
    assert!(rel(sh.eta2(), target) <= 2e-2, "{} {target}", sh.eta2());
    let w = wave_with_kh(1.0, 0.05, 2);
    let crest = w.eta(0.0, 0.0);
    let trough = w.eta(PI / w.k, 0.0);
    assert!(crest + trough > 0.0 && ((crest + trough) - 2.0 * w.eta2()).abs() <= 1e-15);
    // naive hyperbolic formulas at a few points
    let (k, h, a, om) = (w.k, w.h, w.a, w.omega);
    for (x, z, t) in [(0.3, -0.2, 0.1), (1.7, -1.0, 2.2), (-2.0, 0.05, 0.7)] {
        let th = k * x - om * t;
        let e2 = k * a * a / 4.0 * (k * h).cosh() * (2.0 + (2.0 * k * h).cosh()) / (k * h).sinh().powi(3);
        let eta = a * th.cos() + e2 * (2.0 * th).cos();
        let u = a * om * (k * (z + h)).cosh() / (k * h).sinh() * th.cos()
            + 0.75 * a * a * om * k * (2.0 * k * (z + h)).cosh() / (k * h).sinh().powi(4) * (2.0 * th).cos();
        let ww = a * om * (k * (z + h)).sinh() / (k * h).sinh() * th.sin()
            + 0.75 * a * a * om * k * (2.0 * k * (z + h)).sinh() / (k * h).sinh().powi(4) * (2.0 * th).sin();
        let phi = a * G / om * (k * (z + h)).cosh() / (k * h).cosh() * th.sin()
            + 0.375 * a * a * om * (2.0 * k * (z + h)).cosh() / (k * h).sinh().powi(4) * (2.0 * th).sin();
        assert!(rel(w.eta(x, t), eta) <= 1e-12);
        let (uu, wv) = w.velocity(x, z, t);
        assert!(rel(uu, u) <= 1e-12 && rel(wv, ww) <= 1e-12);
        assert!(rel(w.potential(x, z, t), phi) <= 1e-12);
    }
}

/// Free-surface residuals (kinematic, dynamic) of a wave with `k a = ka`, maximum over 720 phases. With `scale_amp` they are
/// divided by `a omega` and `g a` (the registered text of W3, which makes the third-order ratio 4, not 8); without, by the fixed
/// constants `omega/k` and `g/k` (independent of the amplitude).
fn bc_residuals(kh: f64, ka: f64, order: u8, scale_amp: bool) -> (f64, f64) {
    let probe = wave_with_kh(kh, 1.0, order);
    let a = ka / probe.k;
    let w = Wave { a, ..probe };
    let n = 720;
    let mut kin: f64 = 0.0;
    let mut dynv = Vec::new();
    for i in 0..n {
        let x = 2.0 * PI * i as f64 / n as f64 / w.k;
        let eta1 = a * (w.k * x).cos();
        let (u0, w0) = w.velocity(x, 0.0, 0.0);
        let rk = w.eta_t(x, 0.0) + u0 * w.eta_x(x, 0.0) - w0 - eta1 * w.dw_dz(x, 0.0, 0.0);
        kin = kin.max(rk.abs());
        let rd = G * w.eta(x, 0.0)
            + w.potential_t(x, 0.0, 0.0)
            + eta1 * w.potential_tz(x, 0.0, 0.0)
            + 0.5 * (u0 * u0 + w0 * w0);
        dynv.push(rd);
    }
    let mean = dynv.iter().sum::<f64>() / n as f64;
    let dynr = dynv.iter().map(|v| (v - mean).abs()).fold(0.0, f64::max);
    if scale_amp {
        (kin / (a * w.omega), dynr / (G * a))
    } else {
        (kin * w.k / w.omega, dynr * w.k / G)
    }
}

/// Registered W3 (ii)/(iii) as written (residuals scaled by the amplitude): KNOWN MISS of the registered scaling statement.
#[test]
#[ignore = "known miss: scaling by a omega / g a makes the third-order ratio 4, not 8 (registered text defect, see ADR 0040 addendum)"]
fn w3_residual_scaling_amplitude_scaled_known_miss() {
    for kh in [0.5, 1.5, 4.0] {
        let (k1, d1) = bc_residuals(kh, 1e-3, 2, true);
        let (k2, d2) = bc_residuals(kh, 0.5e-3, 2, true);
        println!(
            "w3 (as registered) kh = {kh}: kinematic ratio {}, dynamic ratio {}",
            k1 / k2,
            d1 / d2
        );
        assert!(
            (7.5..=8.5).contains(&(k1 / k2)),
            "kinematic ratio {} at kh = {kh}",
            k1 / k2
        );
        assert!(
            (7.5..=8.5).contains(&(d1 / d2)),
            "dynamic ratio {} at kh = {kh}",
            d1 / d2
        );
        let (_, f1) = bc_residuals(kh, 1e-3, 1, true);
        let (_, f2) = bc_residuals(kh, 0.5e-3, 1, true);
        assert!(
            (3.5..=4.5).contains(&(f1 / f2)),
            "first-order dynamic ratio {} at kh = {kh}",
            f1 / f2
        );
    }
}

/// Corrected set-up of W3 (ii)/(iii): residuals normalised by amplitude-independent constants. Third order: ratio in [7.5, 8.5];
/// first order only: ratio in [3.5, 4.5] (second order is then required to beat first order by a clear margin at the same amplitude).
#[test]
fn w3b_residual_scaling_absolute() {
    for kh in [0.5, 1.5, 4.0] {
        let (k1, d1) = bc_residuals(kh, 1e-3, 2, false);
        let (k2, d2) = bc_residuals(kh, 0.5e-3, 2, false);
        println!(
            "w3b kh = {kh}: kinematic {k1:e} -> {k2:e} (ratio {}), dynamic {d1:e} -> {d2:e} (ratio {})",
            k1 / k2,
            d1 / d2
        );
        assert!(
            (7.5..=8.5).contains(&(k1 / k2)),
            "kinematic ratio {} at kh = {kh}",
            k1 / k2
        );
        assert!(
            (7.5..=8.5).contains(&(d1 / d2)),
            "dynamic ratio {} at kh = {kh}",
            d1 / d2
        );
        let (fk1, f1) = bc_residuals(kh, 1e-3, 1, false);
        let (_, f2) = bc_residuals(kh, 0.5e-3, 1, false);
        println!(
            "w3b kh = {kh}: first-order dynamic {f1:e} (ratio {}), kinematic {fk1:e}",
            f1 / f2
        );
        assert!(
            (3.5..=4.5).contains(&(f1 / f2)),
            "first-order dynamic ratio {} at kh = {kh}",
            f1 / f2
        );
        assert!(f1 > 10.0 * d1, "second order does not beat first order at kh = {kh}");
    }
}

#[test]
fn w4_inlet_and_fractions() {
    assert_eq!(ramp(0.0, 5.0), 0.0);
    assert_eq!(ramp(-1.0, 5.0), 0.0);
    assert_eq!(ramp(5.0, 5.0), 1.0);
    assert_eq!(ramp(9.0, 5.0), 1.0);
    let mut last = 0.0;
    for i in 1..=1000 {
        let r = ramp(5.0 * i as f64 / 1000.0, 5.0);
        assert!(r >= last);
        last = r;
    }
    assert_eq!(liquid_fraction(1.0, 0.1, 2.0), 1.0);
    assert_eq!(liquid_fraction(1.0, 0.1, 0.0), 0.0);
    assert!((liquid_fraction(1.0, 0.1, 1.0) - 0.5).abs() <= 1e-15);
    assert!((liquid_fraction(1.0, 0.1, 1.02) - 0.7).abs() <= 1e-14);
    let (h, dz, eta) = (3.0, 0.1, 0.3737);
    let sum: f64 = (0..40)
        .map(|i| liquid_fraction(-h + (i as f64 + 0.5) * dz, dz, eta) * dz)
        .sum();
    assert!((sum - (eta + h)).abs() <= 1e-14, "{sum}");
    let w = Wave::new(0.4, 0.9, 5.0, G, 0.2, 1).unwrap();
    let (x, z, t, tr) = (1.3, -0.7, 4.1, 10.0);
    let (eta, u, ww) = w.inlet_state(x, z, t, tr);
    let r = ramp(t, tr);
    let th = w.k * x - w.omega * t + 0.2;
    assert!(rel(eta, r * 0.4 * th.cos()) <= 1e-14);
    assert!(
        rel(
            u,
            r * 0.4 * 0.9 * (w.k * (z + 5.0)).cosh() / (w.k * 5.0).sinh() * th.cos()
        ) <= 1e-13
    );
    assert!(
        rel(
            ww,
            r * 0.4 * 0.9 * (w.k * (z + 5.0)).sinh() / (w.k * 5.0).sinh() * th.sin()
        ) <= 1e-13
    );
}

#[test]
fn w5_irregular_waves() {
    let wp = 2.0 * PI / 9.0;
    let hs = 2.5;
    let pm = Spectrum::pierson_moskowitz(hs, wp).unwrap();
    let wave = IrregularWave::new(&pm, f64::INFINITY, G, 0.4 * wp, 6.0 * wp, 200, 12345).unwrap();
    let hs_rec = wave.significant_height();
    println!(
        "w5 PM: recovered Hs = {hs_rec:.6} (target 2.5, rel {:e})",
        rel(hs_rec, hs)
    );
    assert!(rel(hs_rec, hs) <= 1e-3, "{hs_rec}");
    let m1 = spectral_moment(&pm, 1, 0.3 * wp, 40.0 * wp, 20000);
    let m1c = 5.0 / 64.0 * hs * hs * wp * 1.25f64.powf(-0.75) * 1.2254167024651776;
    let m2 = spectral_moment(&pm, 2, 0.3 * wp, 40.0 * wp, 20000);
    let m2c = 5.0 / 64.0 * hs * hs * wp * wp * 1.25f64.powf(-0.5) * PI.sqrt();
    println!("w5 moments: m1 rel {:e}, m2 rel {:e}", rel(m1, m1c), rel(m2, m2c));
    assert!(rel(m1, m1c) <= 2e-3 && rel(m2, m2c) <= 2e-3);
    let (mut best, mut arg) = (0.0, 0.0);
    for i in 0..15000 {
        let w = wp * (0.5 + 1e-4 * i as f64);
        let s = pm.density(w);
        if s > best {
            best = s;
            arg = w;
        }
    }
    assert!((arg / wp - 1.0).abs() <= 1e-3, "{}", arg / wp);
    // JONSWAP
    let js = Spectrum::jonswap(hs, wp, 3.3).unwrap();
    let wj = IrregularWave::new(&js, f64::INFINITY, G, 0.4 * wp, 6.0 * wp, 200, 12345).unwrap();
    println!(
        "w5 JONSWAP: recovered Hs = {:.6} (rel {:e})",
        wj.significant_height(),
        rel(wj.significant_height(), hs)
    );
    assert!(rel(wj.significant_height(), hs) <= 2e-3);
    let (mut best, mut arg) = (0.0, 0.0);
    for i in 0..15000 {
        let w = wp * (0.5 + 1e-4 * i as f64);
        let s = js.density(w);
        if s > best {
            best = s;
            arg = w;
        }
    }
    assert!(arg >= wp && arg <= 1.05 * wp, "{}", arg / wp);
    let j1 = Spectrum::jonswap(hs, wp, 1.0).unwrap();
    for i in 0..50 {
        let w = wp * (0.5 + 0.05 * i as f64);
        assert!(rel(j1.density(w), pm.density(w)) <= 1e-6, "w = {w}");
    }
    // reproducibility
    let w2 = IrregularWave::new(&pm, f64::INFINITY, G, 0.4 * wp, 6.0 * wp, 200, 12345).unwrap();
    let w3 = IrregularWave::new(&pm, f64::INFINITY, G, 0.4 * wp, 6.0 * wp, 200, 54321).unwrap();
    let mut differ = false;
    for i in 0..200 {
        assert_eq!(wave.components[i].phase.to_bits(), w2.components[i].phase.to_bits());
        differ |= wave.components[i].phase != w3.components[i].phase;
        assert!((0.0..2.0 * PI).contains(&wave.components[i].phase));
    }
    assert!(differ);
    assert_eq!(wave.eta(10.0, 3.0).to_bits(), w2.eta(10.0, 3.0).to_bits());
    let mc: f64 = wave.components.iter().map(|c| c.phase.cos()).sum::<f64>() / 200.0;
    let ms: f64 = wave.components.iter().map(|c| c.phase.sin()).sum::<f64>() / 200.0;
    println!("w5 phase means: cos {mc:.4}, sin {ms:.4}");
    assert!(mc.abs() <= 0.15 && ms.abs() <= 0.15);
    // dispersion of every component
    let fin = IrregularWave::new(&pm, 15.0, G, 0.4 * wp, 6.0 * wp, 200, 1).unwrap();
    for c in &fin.components {
        assert!((c.omega * c.omega - G * c.k * (c.k * 15.0).tanh()).abs() <= 1e-12 * c.omega * c.omega);
    }
    // variance over one repetition period on a commensurate grid
    let dw = 0.02;
    let n = 150;
    let wl = 20.0 * dw;
    let pm2 = Spectrum::pierson_moskowitz(2.0, 0.7).unwrap();
    let cw = IrregularWave::new(&pm2, 20.0, G, wl, wl + n as f64 * dw, n, 777).unwrap();
    assert!((cw.d_omega - dw).abs() <= 1e-15);
    let period = cw.repetition_period();
    let ns = 4096;
    let ser: Vec<f64> = (0..ns).map(|i| cw.eta(0.0, period * i as f64 / ns as f64)).collect();
    let mean = ser.iter().sum::<f64>() / ns as f64;
    let var = ser.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / ns as f64;
    assert!(rel(var, cw.variance()) <= 1e-10, "{var} {}", cw.variance());
    assert!(mean.abs() <= 1e-10);
    assert!(rel(4.0 * var.sqrt(), cw.significant_height()) <= 1e-10);
    // deep-water velocity against the independent e^{kz} form
    let (x, z, t) = (12.0, -2.5, 3.3);
    let (u, ww) = wave.velocity(x, z, t);
    let (mut us, mut ws) = (0.0, 0.0);
    for c in &wave.components {
        let th = c.k * x - c.omega * t + c.phase;
        us += c.a * c.omega * (c.k * z).exp() * th.cos();
        ws += c.a * c.omega * (c.k * z).exp() * th.sin();
    }
    assert!(
        (u - us).abs() <= 1e-13 * us.abs().max(1.0) && (ww - ws).abs() <= 1e-13 * ws.abs().max(1.0),
        "{u} {us} {ww} {ws}"
    );
}

#[test]
fn w6_relaxation_weights() {
    let ex = Blend::exponential();
    assert_eq!(ex.weight(0.0), 1.0);
    assert_eq!(ex.weight(1.0), 0.0);
    assert_eq!(ex.weight(1.5), 0.0);
    assert_eq!(ex.weight(-0.5), 1.0);
    let mut last = 1.0;
    for i in 1..10000 {
        let w = ex.weight(i as f64 / 10000.0);
        assert!(w < last, "not strictly decreasing at {i}");
        last = w;
    }
    let hand = 1.0 - (0.5f64.powf(3.5).exp() - 1.0) / (E - 1.0);
    assert!((ex.weight(0.5) - hand).abs() <= 1e-14);
    assert!(1.0 - ex.weight(0.01) <= 1e-6);
    let po = Blend::Polynomial { n: 3.0 };
    assert_eq!(po.weight(0.0), 1.0);
    assert_eq!(po.weight(1.0), 0.0);
    let mut last = 1.0;
    for i in 1..1000 {
        let c = i as f64 / 1000.0;
        let w = po.weight(c);
        assert!((w - (1.0 - c).powi(3)).abs() <= 1e-14 && w < last);
        last = w;
    }
    assert_eq!(zone_chi(2.0, 2.0, 6.0), 0.0);
    assert_eq!(zone_chi(4.0, 2.0, 6.0), 0.5);
    assert_eq!(zone_chi(9.0, 2.0, 6.0), 1.0);
    assert_eq!(zone_chi(4.0, 6.0, 2.0), 0.5);
    assert_eq!(zone_chi(1.0, 6.0, 2.0), 1.0);
    assert_eq!(zone_chi(7.0, 6.0, 2.0), 0.0);
    assert_eq!(relax(1.0, 3.0, 5.0), 3.0);
    assert_eq!(relax(0.0, 3.0, 5.0), 5.0);
    // sponge profile integral
    assert_eq!(sponge_rate(0.0, 2.0, 3.0), 0.0);
    assert_eq!(sponge_rate(1.0, 2.0, 3.0), 2.0);
    let (smax, len, n) = (2.0, 7.0, 3.0);
    let m = 1000;
    let hh = len / m as f64;
    let f = |x: f64| sponge_rate(x / len, smax, n);
    let mut s = f(0.0) + f(len);
    for i in 1..m {
        s += f(i as f64 * hh) * if i % 2 == 1 { 4.0 } else { 2.0 };
    }
    assert!(rel(s * hh / 3.0, smax * len / (n + 1.0)) <= 1e-12);
}

/// Run the tank; returns the gauge series sampled after every step.
fn run_tank(cfg: TankConfig, gauge: usize, steps: usize) -> Vec<f64> {
    let mut tank = WaveTank::new(cfg).unwrap();
    let mut out = Vec::with_capacity(steps);
    for _ in 0..steps {
        tank.step();
        out.push(tank.eta[gauge]);
    }
    out
}

struct Setup {
    lambda: f64,
    cfg: TankConfig,
    omega: f64,
    c: f64,
}

fn setup() -> Setup {
    let (g, h, period): (f64, f64, f64) = (9.81, 1.0, 8.0);
    let c = (g * h).sqrt();
    let lambda = c * period;
    let cfg = TankConfig {
        g,
        h,
        dx: lambda / 40.0,
        dt: period / 80.0,
        n_cells: 0,
        amplitude: 0.01,
        period,
        ramp_time: 2.0 * period,
        zone: TankZone::None,
    };
    Setup {
        lambda,
        cfg,
        omega: 2.0 * PI / period,
        c,
    }
}

/// Reflection coefficient of the zone described by `zone` over a domain of `domain_lambda` wavelengths.
fn reflection(s: &Setup, zone: TankZone, domain_lambda: f64, x_e_lambda: f64, x_s_lambda: f64) -> (f64, f64) {
    let (xg, per) = (9.0, s.cfg.period);
    let gauge = (xg * 40.0) as usize;
    // window per the ADR: starts after every reflection from the zone has become steady, ends before the left re-reflection
    let t0 = ((2.0 * x_e_lambda - xg) + 3.0) * per;
    let t1 = t0 + 3.0 * per;
    let t_rr = (2.0 * x_s_lambda + xg) * per;
    assert!(t1 < t_rr, "window {t1} overlaps the left re-reflection at {t_rr}");
    let steps = (t1 / s.cfg.dt).ceil() as usize + 2;
    let n_beach = (domain_lambda * 40.0).round() as usize;
    let beach = run_tank(
        TankConfig {
            n_cells: n_beach,
            zone,
            ..s.cfg
        },
        gauge,
        steps,
    );
    let reference = run_tank(
        TankConfig {
            n_cells: 80 * 40,
            zone: TankZone::None,
            ..s.cfg
        },
        gauge,
        steps,
    );
    let i0 = (t0 / s.cfg.dt).round() as usize - 1;
    let nw = (3.0 * per / s.cfg.dt).round() as usize;
    let ts = (i0 + 1) as f64 * s.cfg.dt;
    let diff: Vec<f64> = (i0..i0 + nw).map(|i| beach[i] - reference[i]).collect();
    let d = harmonic_amplitude(&diff, s.cfg.dt, ts, s.omega);
    let inc = harmonic_amplitude(&reference[i0..i0 + nw], s.cfg.dt, ts, s.omega);
    (d / inc, inc / s.cfg.amplitude)
}

#[test]
fn w7_reflection_toy() {
    let s = setup();
    assert!((s.c * s.cfg.dt / s.cfg.dx - 0.5).abs() <= 1e-12);
    // (a) rigid wall at 14 lambda
    let (r_wall, inc) = reflection(&s, TankZone::None, 14.0, 14.0, 14.0);
    println!("w7 incident amplitude ratio {inc:.5}, rigid wall R = {r_wall:.5}");
    assert!((inc - 1.0).abs() <= 0.02, "{inc}");
    assert!((0.995..=1.005).contains(&r_wall), "{r_wall}");
    // (b) exponential beach, L = 2 lambda
    let zone2 = TankZone::Relaxation {
        x_start: 12.0 * s.lambda,
        x_end: 14.0 * s.lambda,
        blend: Blend::exponential(),
    };
    let (r2, _) = reflection(&s, zone2, 14.0, 14.0, 12.0);
    println!("w7 exponential beach L = 2 lambda: R = {r2:.5}");
    // (c) L = 4 lambda
    let zone4 = TankZone::Relaxation {
        x_start: 12.0 * s.lambda,
        x_end: 16.0 * s.lambda,
        blend: Blend::exponential(),
    };
    let (r4, _) = reflection(&s, zone4, 16.0, 16.0, 12.0);
    println!("w7 exponential beach L = 4 lambda: R = {r4:.5}");
    // (d) abrupt zone
    let zone0 = TankZone::Relaxation {
        x_start: 12.0 * s.lambda,
        x_end: 12.05 * s.lambda,
        blend: Blend::exponential(),
    };
    let (r0, _) = reflection(&s, zone0, 12.05, 12.05, 12.0);
    println!("w7 abrupt zone: R = {r0:.5}");
    // sponge variant, reported only
    let sig = 3.0 * s.c / (2.0 * s.lambda);
    let zs = TankZone::Sponge {
        x_start: 12.0 * s.lambda,
        x_end: 14.0 * s.lambda,
        sigma_max: sig,
        n: 2.0,
    };
    let (rs, _) = reflection(&s, zs, 14.0, 14.0, 12.0);
    println!("w7 sponge (sigma_max = {sig:.4}, n = 2, L = 2 lambda): R = {rs:.5} (reported only)");
    assert!(r2 <= 0.05, "beach L = 2 lambda: R = {r2}");
    assert!(r4 <= r2, "L = 4 lambda R = {r4} exceeds L = 2 lambda R = {r2}");
}

fn abrupt_and_beach_r(s: &Setup) -> (f64, f64) {
    let zone2 = TankZone::Relaxation {
        x_start: 12.0 * s.lambda,
        x_end: 14.0 * s.lambda,
        blend: Blend::exponential(),
    };
    let (r2, _) = reflection(s, zone2, 14.0, 14.0, 12.0);
    let zone0 = TankZone::Relaxation {
        x_start: 12.0 * s.lambda,
        x_end: 12.05 * s.lambda,
        blend: Blend::exponential(),
    };
    let (r0, _) = reflection(s, zone0, 12.05, 12.05, 12.0);
    (r0, r2)
}

/// WAVE 25B NOTE (ADR 0044 D3): the premise is shown ill-posed by `w8_*` below (the independent discrete frequency-domain solution gives the
/// same 0.2144; the thin zone's reflection is set by its integrated per-second damping); this record stays ignored with its limit.
/// Registered W7 (d): the abrupt zone reflects at least half. KNOWN MISS (the premise was wrong, see ADR 0040 addendum).
#[test]
#[ignore = "known miss: abrupt relaxation zone R = 0.21 < 0.5 (premise of the registered check)"]
fn w7d_abrupt_zone_reflects_half_known_miss() {
    let s = setup();
    let (r0, _) = abrupt_and_beach_r(&s);
    assert!(r0 >= 0.5, "abrupt zone R = {r0}");
}

/// Corrected discrimination check (written AFTER the numbers above were seen; stated as such in the addendum): the abrupt zone
/// reflects much more than the 2 lambda beach.
#[test]
fn w7e_abrupt_zone_vs_beach() {
    let s = setup();
    let (r0, r2) = abrupt_and_beach_r(&s);
    println!("w7e abrupt R = {r0:.5}, beach R = {r2:.6}");
    assert!(r0 >= 0.1 && r0 >= 50.0 * r2, "abrupt {r0} beach {r2}");
}

// ---------------------------------------------------------------------------------------------------------------------
// Wave 25B (ADR 0044 D3): the cause of the W7 (d) miss tested separately. Checks W8-1 to W8-5 as registered before the runs.
// ---------------------------------------------------------------------------------------------------------------------

/// Per-step factors of the tank (the same position rule as `WaveTank::new`): eta at cell centres, u at faces.
fn tank_weights(cfg: &TankConfig) -> (Vec<f64>, Vec<f64>) {
    let factor = |x: f64| -> f64 {
        match cfg.zone {
            TankZone::None => 1.0,
            TankZone::Relaxation { x_start, x_end, blend } => blend.weight(zone_chi(x, x_start, x_end)),
            TankZone::Sponge {
                x_start,
                x_end,
                sigma_max,
                n,
            } => (-sponge_rate(zone_chi(x, x_start, x_end), sigma_max, n) * cfg.dt).exp(),
        }
    };
    let n = cfg.n_cells;
    (
        (0..n).map(|i| factor((i as f64 + 0.5) * cfg.dx)).collect(),
        (0..=n).map(|j| factor(j as f64 * cfg.dx)).collect(),
    )
}

/// Minimal complex number (the test must not depend on the library).
#[derive(Clone, Copy)]
struct Cx(f64, f64);
impl Cx {
    fn add(self, o: Cx) -> Cx {
        Cx(self.0 + o.0, self.1 + o.1)
    }
    fn sub(self, o: Cx) -> Cx {
        Cx(self.0 - o.0, self.1 - o.1)
    }
    fn mul(self, o: Cx) -> Cx {
        Cx(self.0 * o.0 - self.1 * o.1, self.0 * o.1 + self.1 * o.0)
    }
    fn scale(self, k: f64) -> Cx {
        Cx(self.0 * k, self.1 * k)
    }
    fn div(self, o: Cx) -> Cx {
        let d = o.0 * o.0 + o.1 * o.1;
        Cx((self.0 * o.0 + self.1 * o.1) / d, (self.1 * o.0 - self.0 * o.1) / d)
    }
    fn abs(self) -> f64 {
        self.0.hypot(self.1)
    }
}

/// Independent steady-harmonic solution of the SAME discrete scheme (not a time-domain run). One step is
/// `u* = u - G d(eta)` (interior faces), `eta* = eta - H d(u*)`, `u' = wu u*`, `eta' = we eta*` with `G = g dt/dx`, `H = h dt/dx`.
/// With the time factor `z^n`, `z = exp(-i omega dt)`: `u*_j (z - wu_j) = -z G (E_j - E_{j-1})` and
/// `E_i (z - we_i) = -we_i H (u*_{i+1} - u*_i)`. Marching from the wall (`u*_N = 0`, `E_{N-1} = 1`) towards the source gives the standing
/// field; in the lossless region it is `I q^i + R q^-i` with the discrete dispersion `q + 1/q = 2 - 4 sin^2(omega dt/2)/(c dt/dx)^2`.
/// Returns `|R/I|`, read from the two cells `i_probe`, `i_probe + 1` (both with lossless stencils).
fn discrete_reflection(cfg: &TankConfig, i_probe: usize) -> f64 {
    let (we, wu) = tank_weights(cfg);
    let n = cfg.n_cells;
    let omega = 2.0 * PI / cfg.period;
    let z = Cx((omega * cfg.dt).cos(), -(omega * cfg.dt).sin());
    let (g_, h_) = (cfg.g * cfg.dt / cfg.dx, cfg.h * cfg.dt / cfg.dx);
    let cfl2 = g_ * h_;
    let cos_k = 1.0 - 2.0 * (0.5 * omega * cfg.dt).sin().powi(2) / cfl2;
    assert!(cos_k.abs() < 1.0);
    let k_dx = cos_k.acos();
    let q = Cx(k_dx.cos(), k_dx.sin());
    let q_inv = Cx(k_dx.cos(), -k_dx.sin());
    let mut e = vec![Cx(0.0, 0.0); n];
    let mut ustar = vec![Cx(0.0, 0.0); n + 1];
    e[n - 1] = Cx(1.0, 0.0);
    for i in (i_probe + 1..n).rev() {
        // u*_i from the eta equation of cell i
        ustar[i] = ustar[i + 1].add(e[i].mul(z.sub(Cx(we[i], 0.0))).scale(1.0 / (we[i] * h_)));
        // eta_{i-1} from the u equation of face i
        e[i - 1] = e[i].add(ustar[i].mul(z.sub(Cx(wu[i], 0.0))).div(z).scale(1.0 / g_));
    }
    let (e0, e1) = (e[i_probe], e[i_probe + 1]);
    // e0 = a + b, e1 = a q + b / q
    let a = e1.sub(e0.mul(q_inv)).div(q.sub(q_inv));
    let b = e0.sub(a);
    b.abs() / a.abs()
}

/// Continuum estimate (ODE integration of the layer with rates `-ln(w)/dt`, piecewise constant on the cells and the face volumes).
fn continuum_reflection(cfg: &TankConfig, x_probe: f64, linear_rate: bool) -> f64 {
    let (we, wu) = tank_weights(cfg);
    let n = cfg.n_cells;
    let omega = 2.0 * PI / cfg.period;
    let c = (cfg.g * cfg.h).sqrt();
    let rate = |w: f64| {
        if w >= 1.0 {
            0.0
        } else if linear_rate {
            (1.0 - w) / cfg.dt
        } else {
            -w.ln() / cfg.dt
        }
    };
    // e^{-i w t} convention: d eta/dx = -(-i w + a_u) u / g, d u/dx = -(-i w + a_e) eta / h
    let rhs = |x: f64, eta: Cx, u: Cx| -> (Cx, Cx) {
        let ie = ((x / cfg.dx).floor() as usize).min(n - 1);
        let iu = ((x / cfg.dx).round() as usize).min(n);
        let a_e = rate(we[ie]);
        let a_u = if iu == n { 0.0 } else { rate(wu[iu]) };
        let deta = Cx(a_u, -omega).mul(u).scale(-1.0 / cfg.g);
        let du = Cx(a_e, -omega).mul(eta).scale(-1.0 / cfg.h);
        (deta, du)
    };
    let (mut eta, mut u) = (Cx(1.0, 0.0), Cx(0.0, 0.0));
    let mut x = n as f64 * cfg.dx;
    let half = 0.5 * cfg.dx;
    let sub = 8;
    while x > x_probe + 1e-9 {
        // one half-cell piece with constant coefficients (evaluated at the piece midpoint)
        let xm = x - 0.5 * half;
        let hh = -half / sub as f64;
        for _ in 0..sub {
            let (k1e, k1u) = rhs(xm, eta, u);
            let (k2e, k2u) = rhs(xm, eta.add(k1e.scale(0.5 * hh)), u.add(k1u.scale(0.5 * hh)));
            let (k3e, k3u) = rhs(xm, eta.add(k2e.scale(0.5 * hh)), u.add(k2u.scale(0.5 * hh)));
            let (k4e, k4u) = rhs(xm, eta.add(k3e.scale(hh)), u.add(k3u.scale(hh)));
            eta = eta.add(k1e.add(k2e.scale(2.0)).add(k3e.scale(2.0)).add(k4e).scale(hh / 6.0));
            u = u.add(k1u.add(k2u.scale(2.0)).add(k3u.scale(2.0)).add(k4u).scale(hh / 6.0));
        }
        x -= half;
    }
    let hu = u.scale(cfg.h / c);
    eta.sub(hu).abs() / eta.add(hu).abs()
}

fn zone_exp(s: &Setup, m_cells: usize) -> (TankZone, f64) {
    let ext = m_cells as f64 / 40.0;
    (
        TankZone::Relaxation {
            x_start: 12.0 * s.lambda,
            x_end: (12.0 + ext) * s.lambda,
            blend: Blend::exponential(),
        },
        12.0 + ext,
    )
}

fn zone_uniform(s: &Setup, m_cells: usize, w: f64, dt: f64) -> (TankZone, f64) {
    let ext = m_cells as f64 / 40.0;
    (
        TankZone::Sponge {
            x_start: 12.0 * s.lambda,
            x_end: (12.0 + ext) * s.lambda,
            sigma_max: -w.ln() / dt,
            n: 1e-9,
        },
        12.0 + ext,
    )
}

/// Time-domain R and the discrete frequency-domain R of the same zone.
fn both_r(s: &Setup, zone: TankZone, domain_lambda: f64, x_s_lambda: f64) -> (f64, f64) {
    let (r_time, _) = reflection(s, zone, domain_lambda, domain_lambda, x_s_lambda);
    let cfg = TankConfig {
        n_cells: (domain_lambda * 40.0).round() as usize,
        zone,
        ..s.cfg
    };
    (r_time, discrete_reflection(&cfg, 470))
}

/// Sanity of the solver itself: a lossless wall gives |R| = 1.
#[test]
fn w8_0_discrete_solver_lossless() {
    let s = setup();
    let cfg = TankConfig {
        n_cells: 560,
        zone: TankZone::None,
        ..s.cfg
    };
    let r = discrete_reflection(&cfg, 470);
    eprintln!("W8-0 lossless wall |R| = {r:.12}");
    assert!((r - 1.0).abs() <= 1e-9);
}

/// W8-1 and W8-4: the time-domain R equals the independent discrete frequency-domain R within 5e-3 absolute; the continuum
/// estimate is within 25 percent of it for the registered abrupt zone.
#[test]
fn w8_1_time_domain_equals_discrete_solution() {
    let s = setup();
    let mut worst = 0.0_f64;
    for m in [1usize, 2, 4, 8] {
        let (zone, dom) = zone_exp(&s, m);
        let (rt, rd) = both_r(&s, zone, dom, 12.0);
        let cfg = TankConfig {
            n_cells: (dom * 40.0).round() as usize,
            zone,
            ..s.cfg
        };
        let rc = continuum_reflection(&cfg, 12.0 * s.lambda, false);
        let rl = continuum_reflection(&cfg, 12.0 * s.lambda, true);
        eprintln!(
            "W8-1 exponential zone of {m} cells (L = {:.4} lambda): time-domain R {rt:.5}, discrete {rd:.5}, continuum (-ln w/dt) {rc:.5}, continuum ((1-w)/dt) {rl:.5}",
            m as f64 / 40.0
        );
        worst = worst.max((rt - rd).abs());
        assert!((rt - rd).abs() <= 5e-3, "m {m}: {rt} vs {rd}");
    }
    // registered sponge, L = 2 lambda
    let sig = 3.0 * s.c / (2.0 * s.lambda);
    let zs = TankZone::Sponge {
        x_start: 12.0 * s.lambda,
        x_end: 14.0 * s.lambda,
        sigma_max: sig,
        n: 2.0,
    };
    let (rt, rd) = both_r(&s, zs, 14.0, 12.0);
    eprintln!("W8-1 registered sponge: time-domain R {rt:.5}, discrete {rd:.5}");
    worst = worst.max((rt - rd).abs());
    assert!((rt - rd).abs() <= 5e-3);
    eprintln!("W8-1 worst |time - discrete| {worst:.2e} (limit 5e-3)");
}

/// W8-2: the per-step weight is a rate `-ln(w)/dt`; the same weights at half the time step change R by more than 0.1.
#[test]
fn w8_2_dt_dependence_of_the_same_weights() {
    let s = setup();
    let (zone, dom) = zone_exp(&s, 2);
    let (r1_t, r1_d) = both_r(&s, zone, dom, 12.0);
    let mut s2 = setup();
    s2.cfg.dt = s2.cfg.period / 160.0;
    let (r2_t, r2_d) = both_r(&s2, zone, dom, 12.0);
    eprintln!(
        "W8-2 abrupt zone, dt = T/80: time {r1_t:.5} discrete {r1_d:.5}; dt = T/160: time {r2_t:.5} discrete {r2_d:.5}"
    );
    assert!((r1_t - r1_d).abs() <= 5e-3 && (r2_t - r2_d).abs() <= 5e-3);
    assert!((r2_t - r1_t).abs() > 0.1, "R(dt/2) {r2_t} vs R(dt) {r1_t}");
}

/// W8-3: the sponge reflection is round-trip attenuation, R = exp(-2 int sigma dx / c).
#[test]
fn w8_3_sponge_is_round_trip_attenuation() {
    let s = setup();
    let sig = 3.0 * s.c / (2.0 * s.lambda);
    let len = 2.0 * s.lambda;
    let m = 2000;
    let hh = len / m as f64;
    let f = |x: f64| sponge_rate(x / len, sig, 2.0);
    let mut integral = f(0.0) + f(len);
    for i in 1..m {
        integral += f(i as f64 * hh) * if i % 2 == 1 { 4.0 } else { 2.0 };
    }
    integral *= hh / 3.0;
    let pred = (-2.0 * integral / s.c).exp();
    let zs = TankZone::Sponge {
        x_start: 12.0 * s.lambda,
        x_end: 14.0 * s.lambda,
        sigma_max: sig,
        n: 2.0,
    };
    let (r, _) = reflection(&s, zs, 14.0, 14.0, 12.0);
    eprintln!(
        "W8-3 sponge R {r:.5}, exp(-2 int sigma dx/c) = {pred:.5} (int sigma dx / c = {:.5}; limit 5e-3)",
        integral / s.c
    );
    assert!((r - pred).abs() <= 5e-3, "{r} vs {pred}");
}

/// W8-5: corrected, well-posed thin-zone expectation. A thin zone (L = 0.05 lambda) is a near-rigid reflector only when its integrated
/// damping is small: uniform w = 0.99 gives R >= 0.8, uniform w = 0.8 gives R <= 0.4 (time domain and discrete solution).
#[test]
fn w8_5_thin_zone_reflects_iff_weakly_damped() {
    let s = setup();
    let (weak, dom) = zone_uniform(&s, 2, 0.99, s.cfg.dt);
    let (strong, _) = zone_uniform(&s, 2, 0.8, s.cfg.dt);
    let (rw_t, rw_d) = both_r(&s, weak, dom, 12.0);
    let (rs_t, rs_d) = both_r(&s, strong, dom, 12.0);
    eprintln!(
        "W8-5 thin zone w = 0.99: time {rw_t:.5} discrete {rw_d:.5} (limit >= 0.8); w = 0.8: time {rs_t:.5} discrete {rs_d:.5} (limit <= 0.4)"
    );
    assert!(rw_t >= 0.8 && rw_d >= 0.8);
    assert!(rs_t <= 0.4 && rs_d <= 0.4);
}

/// Registered W8-4: the continuum estimate (rates `-ln(w)/dt`) is within 25 percent of the discrete value for the registered abrupt
/// zone. KNOWN MISS: measured 0.274 against 0.214 (+27.8 percent). The diagnosis is in `w8_4b_*` and ADR 0044 addendum.
#[test]
#[ignore = "KNOWN MISS: continuum estimate of the abrupt zone is 27.8 percent above the discrete value (limit 25 percent)"]
fn w8_4_continuum_estimate_known_miss() {
    let s = setup();
    let (zone, dom) = zone_exp(&s, 2);
    let (_, rd) = both_r(&s, zone, dom, 12.0);
    let cfg = TankConfig {
        n_cells: (dom * 40.0).round() as usize,
        zone,
        ..s.cfg
    };
    let rc = continuum_reflection(&cfg, 12.0 * s.lambda, false);
    eprintln!(
        "W8-4 continuum {rc:.5} vs discrete {rd:.5} (relative {:.3}, limit 0.25)",
        (rc - rd).abs() / rd
    );
    assert!((rc - rd).abs() <= 0.25 * rd, "continuum {rc} vs discrete {rd}");
}

/// Diagnosis of the W8-4 miss (limits written AFTER seeing the W8-4 numbers, labelled post-hoc): the continuum estimate is a statement
/// about small per-step damping (sigma dt << 1). For uniform thin layers with w = 0.99 and 0.97 (sigma dt = 0.010, 0.030) it must agree
/// with the discrete value within 5 percent; the registered exponential zone has w down to 0.74 (sigma dt = 0.30) in its outer cell, where
/// it does not.
#[test]
fn w8_4b_continuum_estimate_holds_for_small_per_step_damping_post_hoc() {
    let s = setup();
    for &w in &[0.99, 0.97] {
        let (zone, dom) = zone_uniform(&s, 2, w, s.cfg.dt);
        let cfg = TankConfig {
            n_cells: (dom * 40.0).round() as usize,
            zone,
            ..s.cfg
        };
        let rd = discrete_reflection(&cfg, 470);
        let rc = continuum_reflection(&cfg, 12.0 * s.lambda, false);
        eprintln!("W8-4b uniform thin layer w = {w}: discrete {rd:.5}, continuum {rc:.5} (relative {:.3}, post-hoc limit 0.05)", (rc - rd).abs() / rd);
        assert!((rc - rd).abs() <= 0.05 * rd, "w {w}: {rc} vs {rd}");
    }
}
