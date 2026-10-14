//! ADR 0013 checks R1-R7 for the 0D reactor toolkit. Limits are those registered in the ADR before any run.

use rcfd_reactor::equil::{equilibrium, Constraint};
use rcfd_reactor::mech::{Mechanism, P_STD, R_GAS};
use rcfd_reactor::mole_fractions;
use rcfd_reactor::ode::{integrate, Flow, Options};
use rcfd_reactor::reactors::*;
use std::path::PathBuf;

// ---------- R1, R2: ODE ----------
#[test]
fn r1_prothero_robinson_and_decay() {
    let lambda = 1.0e6;
    let mut rhs = |t: f64, y: &[f64], f: &mut [f64]| {
        f[0] = -lambda * (y[0] - t.sin()) + t.cos();
    };
    let opt = Options::new(1e-8, 1e-12, 1);
    let mut worst = 0.0_f64;
    let (y, st) = integrate(&mut rhs, 0.0, 1.0, &[0.0], &opt, &mut |s| {
        worst = worst.max((s.y1[0] - s.t1.sin()).abs());
        Flow::Continue
    })
    .unwrap();
    println!(
        "R1 PR: max node error {worst:.3e}, final {:.3e}, steps {}",
        (y[0] - 1f64.sin()).abs(),
        st.accepted
    );
    assert!(worst <= 1e-6, "R1 PR error {worst:e}");
    let mut rhs = |_t: f64, y: &[f64], f: &mut [f64]| f[0] = -y[0];
    let (y, _) = integrate(&mut rhs, 0.0, 2.0, &[1.0], &Options::new(1e-8, 1e-12, 1), &mut |_| {
        Flow::Continue
    })
    .unwrap();
    let rel = (y[0] - (-2f64).exp()).abs() / (-2f64).exp();
    println!("R1 decay rel err {rel:.3e}");
    assert!(rel <= 1e-7, "R1 decay {rel:e}");
}

#[test]
fn r2_robertson() {
    let mut rhs = |_t: f64, y: &[f64], f: &mut [f64]| {
        f[0] = -0.04 * y[0] + 1e4 * y[1] * y[2];
        f[2] = 3e7 * y[1] * y[1];
        f[1] = -f[0] - f[2];
    };
    let mut opt = Options::new(1e-10, 1e-14, 3);
    opt.atol[1] = 1e-18;
    let (y, st) = integrate(&mut rhs, 0.0, 40.0, &[1.0, 0.0, 0.0], &opt, &mut |_| Flow::Continue).unwrap();
    let reference = [0.7158270687, 9.185534764e-6, 0.2841637457];
    let lim = [1e-5, 1e-4, 1e-5];
    for i in 0..3 {
        let rel = (y[i] - reference[i]).abs() / reference[i];
        println!(
            "R2 y{} = {:.10e} ref {:.10e} rel {rel:.3e} (limit {:.0e})",
            i + 1,
            y[i],
            reference[i],
            lim[i]
        );
        assert!(rel <= lim[i], "R2 y{}: {rel:e}", i + 1);
    }
    let s = (y[0] + y[1] + y[2] - 1.0).abs();
    println!("R2 sum error {s:.3e}, steps {}", st.accepted);
    assert!(s <= 1e-12);
}

// ---------- toy mechanisms ----------
fn thermo_record(name: &str, atoms: &[(&str, f64)], low: [f64; 7], high: [f64; 7]) -> String {
    let mut el = String::new();
    for i in 0..4 {
        if let Some((n, c)) = atoms.get(i) {
            el.push_str(&format!("{:<2}{:>3}", n, *c as i64));
        } else {
            el.push_str("     ");
        }
    }
    // columns: 1-18 name, 19-24 date, 25-44 elements, 45 phase, 46-55 Tlow, 56-65 Thigh, 66-73 Tmid, 80 line number
    let h = format!(
        "{:<18}{:<6}{}G{:>10.3}{:>10.3}{:>8.2}",
        name, "TOY", el, 200.0, 6000.0, 1000.0
    );
    let h = format!("{h:<79}1");
    let f = |v: f64| format!("{v:>15.8e}");
    let l2 = format!("{}{}{}{}{}", f(high[0]), f(high[1]), f(high[2]), f(high[3]), f(high[4]));
    let l2 = format!("{l2:<79}2");
    let l3 = format!("{}{}{}{}{}", f(high[5]), f(high[6]), f(low[0]), f(low[1]), f(low[2]));
    let l3 = format!("{l3:<79}3");
    let l4 = format!("{}{}{}{}", f(low[3]), f(low[4]), f(low[5]), f(low[6]));
    let l4 = format!("{l4:<79}4");
    format!("{h}\n{l2}\n{l3}\n{l4}\n")
}

/// constant-cp thermo: cp/R = a1, h/RT = a1 + h0/T, s/R = a1 ln T + s0
fn cc(a1: f64, h0: f64, s0: f64) -> [f64; 7] {
    [a1, 0.0, 0.0, 0.0, 0.0, h0, s0]
}

const A2: (f64, f64, f64) = (3.5, 0.0, 3.0);
const A1: (f64, f64, f64) = (2.5, 30000.0, 4.0);

fn toy_dissociation() -> Mechanism {
    let mech = "ELEMENTS X/20.0/ END\nSPECIES A2 A END\nREACTIONS\nA2 <=> 2A  1.0E13 0.0 0.0\nEND\n";
    let thermo = format!(
        "THERMO ALL\n   200.000  1000.000  6000.000\n{}{}END\n",
        thermo_record("A2", &[("X", 2.0)], cc(A2.0, A2.1, A2.2), cc(A2.0, A2.1, A2.2)),
        thermo_record("A", &[("X", 1.0)], cc(A1.0, A1.1, A1.2), cc(A1.0, A1.1, A1.2))
    );
    Mechanism::from_chemkin(mech, Some(&thermo)).unwrap()
}

fn toy_isomer() -> Mechanism {
    let mech = "ELEMENTS X/20.0/ END\nSPECIES A B END\nREACTIONS\nA => B  50.0 0.0 0.0\nEND\n";
    let t = |n: &str| thermo_record(n, &[("X", 1.0)], cc(2.5, 0.0, 4.0), cc(2.5, 0.0, 4.0));
    let thermo = format!("THERMO ALL\n   200.000  1000.000  6000.000\n{}{}END\n", t("A"), t("B"));
    Mechanism::from_chemkin(mech, Some(&thermo)).unwrap()
}

/// closed-form ln(Kp) for A2 <=> 2A at temperature t
fn ln_kp(t: f64) -> f64 {
    let g = |c: (f64, f64, f64)| c.0 + c.1 / t - (c.0 * t.ln() + c.2);
    -(2.0 * g(A1) - g(A2))
}

#[test]
fn r4_dissociation_equilibrium_analytic() {
    let m = toy_dissociation();
    assert!(m.max_element_imbalance() == 0.0);
    for (t, p_atm) in [(3000.0, 1.0), (3000.0, 10.0), (2500.0, 1.0)] {
        let p = p_atm * P_STD;
        let x0 = [1.0, 0.0];
        let eq = equilibrium(&m, &x0, t, p, Constraint::TP).unwrap();
        let kprime = ln_kp(t).exp() * P_STD / p;
        let xi = (kprime / (4.0 + kprime)).sqrt();
        let xa = 2.0 * xi / (1.0 + xi);
        let rel = (eq.x[1] - xa).abs() / xa;
        println!(
            "R4 TP T={t} P={p_atm} atm: x_A {:.10} analytic {xa:.10} rel {rel:.3e}",
            eq.x[1]
        );
        assert!(rel <= 1e-9, "R4 TP: {rel:e}");
    }
    // HP: enthalpy conserved (closed-form enthalpy)
    let h = |n: &[f64], t: f64| n[0] * R_GAS * (A2.0 * t + A2.1) + n[1] * R_GAS * (A1.0 * t + A1.1);
    let (t0, p) = (2500.0, P_STD);
    let eq = equilibrium(&m, &[1.0, 0.0], t0, p, Constraint::HP).unwrap();
    let h0 = h(&[1.0, 0.0], t0);
    let rel = (h(&eq.n, eq.t) - h0).abs() / (h0.abs() + 3.5 * R_GAS * t0);
    println!("R4 HP: T = {:.6}, enthalpy rel err {rel:.3e}", eq.t);
    assert!(rel <= 1e-9, "R4 HP {rel:e}");
}

#[test]
fn r4_isothermal_cv_relaxes_to_equilibrium() {
    let m = toy_dissociation();
    let t = 3000.0;
    let y0 = m.x_to_y(&[1.0, 0.0]);
    let mut s0 = vec![t];
    s0.extend(&y0);
    let tr = run_batch(
        &m,
        BatchKind::ConstantVolume,
        Energy::Isothermal,
        P_STD,
        &s0,
        1e-3,
        Tol::default(),
        Stop::EndTime,
    )
    .unwrap();
    let y = &tr.last()[1..];
    let x = m.y_to_x(y);
    let v0 = R_GAS * t / P_STD;
    let kc = ln_kp(t).exp() * P_STD / (R_GAS * t);
    let k = kc * v0;
    let xi = (-k + (k * k + 16.0 * k).sqrt()) / 8.0;
    let xa = 2.0 * xi / (1.0 + xi);
    let rel = (x[1] - xa).abs() / xa;
    println!("R4 CV isothermal: x_A {:.10} analytic {xa:.10} rel {rel:.3e}", x[1]);
    assert!(rel <= 1e-6);
}

fn invariants(m: &Mechanism, tr: &Trajectory) -> (f64, f64) {
    let e0 = m.element_mass_fractions(&tr.state[0][1..]);
    let (mut de, mut ds) = (0.0_f64, 0.0_f64);
    for s in &tr.state {
        let e = m.element_mass_fractions(&s[1..]);
        for j in 0..e.len() {
            if e0[j] != 0.0 {
                de = de.max(((e[j] - e0[j]) / e0[j]).abs());
            }
        }
        ds = ds.max((s[1..].iter().sum::<f64>() - 1.0).abs());
    }
    (de, ds)
}

#[test]
fn r5_adiabatic_conservation_toy() {
    let m = toy_dissociation();
    let tol = Tol {
        rtol: 1e-10,
        atol_t: 1e-8,
        atol_y: 1e-14,
    };
    let y0 = m.x_to_y(&[1.0, 0.0]);
    let mut s0 = vec![3000.0];
    s0.extend(&y0);
    for kind in [BatchKind::ConstantPressure, BatchKind::ConstantVolume] {
        let tr = run_batch(&m, kind, Energy::Adiabatic, P_STD, &s0, 1e-4, tol, Stop::EndTime).unwrap();
        let q = |s: &[f64]| match kind {
            BatchKind::ConstantPressure => m.h_mass(s[0], &s[1..]),
            BatchKind::ConstantVolume => m.u_mass(s[0], &s[1..]),
        };
        let q0 = q(&tr.state[0]);
        let scale = q0.abs() + m.cp_mass(s0[0], &y0) * s0[0];
        let dq = tr.state.iter().map(|s| (q(s) - q0).abs() / scale).fold(0.0, f64::max);
        let (de, ds) = invariants(&m, &tr);
        println!(
            "R5 {kind:?}: energy drift {dq:.3e}, element drift {de:.3e}, sum Y drift {ds:.3e}, T_end {:.3}",
            tr.last()[0]
        );
        assert!(tr.last()[0] < 2990.0, "the run must actually react");
        assert!(dq <= 1e-8, "R5 energy {dq:e}");
        assert!(de <= 1e-12, "R5 element {de:e}");
        assert!(ds <= 1e-12, "R5 sum {ds:e}");
    }
}

#[test]
fn r6_plug_flow_and_psr_first_order() {
    let m = toy_isomer();
    let tol = Tol {
        rtol: 1e-10,
        atol_t: 1e-8,
        atol_y: 1e-14,
    };
    let t = 1000.0;
    let s0 = [t, 1.0, 0.0];
    let tr = run_plug_flow(&m, Energy::Isothermal, P_STD, &s0, 2.0, 0.1, tol).unwrap();
    let conv = 1.0 - tr.last()[1];
    let exact = 1.0 - (-50.0 * 0.1 / 2.0_f64).exp();
    let rel = (conv - exact).abs() / exact;
    println!("R6 PFR conversion {conv:.12} exact {exact:.12} rel {rel:.3e}");
    assert!(rel <= 1e-8);
    let spec = Psr {
        tau: 0.01,
        p: P_STD,
        t_in: t,
        y_in: vec![1.0, 0.0],
        energy: Energy::Isothermal,
    };
    let (st, ok) = psr_steady(&m, &spec, &[t, 0.5, 0.5], tol).unwrap();
    assert!(ok);
    let conv = 1.0 - st[1];
    let exact = 0.5 / 1.5;
    let rel = (conv - exact).abs() / exact;
    println!("R6 PSR conversion {conv:.12} exact {exact:.12} rel {rel:.3e}");
    assert!(rel <= 1e-8);
}

// ---------- GRI-Mech 3.0 (fetched data, skipped when absent) ----------
fn gri() -> Option<Mechanism> {
    let d = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../validation-data/data/gri-mech-30");
    let m = std::fs::read_to_string(d.join("berkeley-grimech30.dat")).ok()?;
    let t = std::fs::read_to_string(d.join("berkeley-thermo30.dat")).ok()?;
    Some(Mechanism::from_chemkin(&m, Some(&t)).unwrap_or_else(|e| panic!("GRI-Mech read failed: {e}")))
}

fn mix(m: &Mechanism, pairs: &[(&str, f64)]) -> Vec<f64> {
    let v: Vec<(String, f64)> = pairs.iter().map(|(n, a)| (n.to_string(), *a)).collect();
    mole_fractions(m, &v).unwrap()
}

fn state(m: &Mechanism, x: &[f64], t: f64) -> Vec<f64> {
    let mut s = vec![t];
    s.extend(m.x_to_y(x));
    s
}

fn h2_air(phi: f64) -> Vec<(&'static str, f64)> {
    vec![("H2", 2.0 * phi), ("O2", 1.0), ("N2", 3.76)]
}

#[test]
fn r3_gri_reads() {
    let Some(m) = gri() else {
        return println!("R3 skipped: GRI-Mech data not fetched");
    };
    println!(
        "R3 species {} reactions {} elements {:?}",
        m.species.len(),
        m.reactions.len(),
        m.elements
    );
    assert_eq!(m.species.len(), 53);
    assert_eq!(m.reactions.len(), 325);
    assert_eq!(m.elements.len(), 5);
    assert_eq!(m.max_element_imbalance(), 0.0);
}

/// Registered limit 1e-3 relative; measured 5.1e-3 in h (GRI-Mech's own polynomials are not continuous to that
/// level at Tmid), so the check lives here as a known miss with the limit untouched (ADR 0013, R3).
#[test]
#[ignore = "known miss: GRI-Mech thermo continuity at Tmid exceeds the registered 1e-3 (measured 5.1e-3 in h)"]
fn r3_gri_nasa_continuity_known_miss() {
    let Some(m) = gri() else {
        return println!("R3 skipped: GRI-Mech data not fetched");
    };
    let mut worst = (0.0_f64, 0.0_f64);
    for s in &m.species {
        let mut a = s.clone();
        let mut b = s.clone();
        a.high = s.low;
        b.low = s.high;
        let (hl, hh) = (a.h_rt(s.t_mid - 1e-9), b.h_rt(s.t_mid));
        let (sl, sh) = (a.s_r(s.t_mid - 1e-9), b.s_r(s.t_mid));
        worst.0 = worst.0.max(((hl - hh) / hh.abs().max(1e-3)).abs());
        worst.1 = worst.1.max(((sl - sh) / sh.abs().max(1e-3)).abs());
    }
    println!("R3 NASA continuity at Tmid: h {:.3e}, s {:.3e}", worst.0, worst.1);
    assert!(worst.0 <= 1e-3 && worst.1 <= 1e-3);
}

#[test]
#[ignore = "heavy: GRI-Mech integration; run with --release --ignored"]
fn r7_gri_ignition_delay_trends_and_conservation() {
    let Some(m) = gri() else {
        return println!("R7 skipped: GRI-Mech data not fetched");
    };
    let x = mix(&m, &h2_air(1.0));
    let tol = Tol::default();
    for kind in [BatchKind::ConstantVolume, BatchKind::ConstantPressure] {
        let mut prev = f64::INFINITY;
        for t0 in [1000.0, 1100.0, 1200.0] {
            let s0 = state(&m, &x, t0);
            let tr = run_batch(
                &m,
                kind,
                Energy::Adiabatic,
                P_STD,
                &s0,
                1.0,
                tol,
                Stop::AfterIgnition { rise: 400.0 },
            )
            .unwrap();
            let tau = tr.time_of_max_dtdt().expect("ignites");
            let (de, ds) = invariants(&m, &tr);
            println!(
                "R7 {kind:?} T0 = {t0}: tau_ign (max dT/dt) = {tau:.6e} s, T_end {:.1}, element drift {de:.2e}, sumY {ds:.2e}, steps {}",
                tr.last()[0],
                tr.t.len()
            );
            assert!(tau.is_finite() && tau > 0.0);
            assert!(tau < prev, "ignition delay must decrease with T0");
            assert!(de <= 1e-12, "element drift {de:e}");
            prev = tau;
        }
    }
}

fn element_atoms(m: &Mechanism, x: &[f64]) -> Vec<f64> {
    (0..m.elements.len())
        .map(|j| m.species.iter().zip(x).map(|(s, x)| x * s.atoms[j]).sum())
        .collect()
}

#[test]
#[ignore = "heavy: GRI-Mech equilibrium and long integrations; run with --release --ignored"]
fn r7_gri_equilibrium_consistency() {
    let Some(m) = gri() else {
        return println!("R7 skipped: GRI-Mech data not fetched");
    };
    // CH4/air adiabatic flame temperature
    let xc = mix(&m, &[("CH4", 1.0), ("O2", 2.0), ("N2", 7.52)]);
    let eq = equilibrium(&m, &xc, 300.0, P_STD, Constraint::HP).unwrap();
    let b0 = element_atoms(&m, &xc);
    let mut worst = 0.0_f64;
    for j in 0..b0.len() {
        if b0[j] > 0.0 {
            let atoms: f64 = m.species.iter().zip(&eq.n).map(|(s, n)| n * s.atoms[j]).sum();
            worst = worst.max(((atoms - b0[j]) / b0[j]).abs());
        }
    }
    println!(
        "R7 CH4/air HP: T_ad = {:.2} K (reference 2225, unverified), element error {worst:.2e}",
        eq.t
    );
    assert!((eq.t - 2225.0).abs() / 2225.0 <= 0.01);
    assert!(worst <= 1e-10);
    // kinetics/equilibrium consistency, H2/air
    let x = mix(&m, &h2_air(1.0));
    let tol = Tol::default();
    let s0 = state(&m, &x, 1200.0);
    let uv = equilibrium(&m, &x, 1200.0, P_STD, Constraint::UV).unwrap();
    let tr = run_batch(
        &m,
        BatchKind::ConstantVolume,
        Energy::Adiabatic,
        P_STD,
        &s0,
        1.0,
        tol,
        Stop::EndTime,
    )
    .unwrap();
    let rel = (tr.last()[0] - uv.t).abs() / uv.t;
    println!(
        "R7 CV 1 s: T = {:.2}, UV equilibrium T = {:.2}, rel {rel:.3e}",
        tr.last()[0],
        uv.t
    );
    assert!(rel <= 0.01);
    let hp = equilibrium(&m, &x, 1200.0, P_STD, Constraint::HP).unwrap();
    let tr = run_batch(
        &m,
        BatchKind::ConstantPressure,
        Energy::Adiabatic,
        P_STD,
        &s0,
        1.0,
        tol,
        Stop::EndTime,
    )
    .unwrap();
    let rel = (tr.last()[0] - hp.t).abs() / hp.t;
    println!(
        "R7 CP 1 s: T = {:.2}, HP equilibrium T = {:.2}, rel {rel:.3e}",
        tr.last()[0],
        hp.t
    );
    assert!(rel <= 0.01);
}

fn blowout_tau(m: &Mechanism, phi: f64, t_in: f64) -> Blowout {
    let x = mix(m, &h2_air(phi));
    let eq = equilibrium(m, &x, t_in, P_STD, Constraint::HP).unwrap();
    let mut guess = vec![eq.t];
    guess.extend(&eq.y);
    let spec = Psr {
        tau: 1e-2,
        p: P_STD,
        t_in,
        y_in: m.x_to_y(&x),
        energy: Energy::Adiabatic,
    };
    let t_burn = t_in + 0.4 * (eq.t - t_in);
    let b = psr_blowout(m, &spec, 1e-2, &guess, t_burn, 0.05, Tol::default()).unwrap();
    println!(
        "R7 PSR phi {phi} T_in {t_in}: T_ad {:.1}, blowout tau in [{:.4e}, {:.4e}], T at burning end {:.1}",
        eq.t, b.tau_extinct, b.tau_burning, b.state[0]
    );
    b
}

#[test]
#[ignore = "heavy: GRI-Mech PSR continuation; run with --release --ignored"]
fn r7_gri_psr_blowout_trends() {
    let Some(m) = gri() else {
        return println!("R7 skipped: GRI-Mech data not fetched");
    };
    let cold = blowout_tau(&m, 1.0, 300.0);
    let hot = blowout_tau(&m, 1.0, 600.0);
    let lean = blowout_tau(&m, 0.5, 300.0);
    assert!(cold.tau_burning / cold.tau_extinct <= 1.05);
    assert!(
        hot.tau_burning < cold.tau_extinct,
        "preheat must shorten the blowout residence time"
    );
    assert!(
        lean.tau_extinct > cold.tau_burning,
        "lean mixture must blow out at a longer residence time"
    );
}
