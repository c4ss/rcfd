//! ADR 0027 library-level checks K10 (thermal NO), K11 (sensitivity, PSR to plug network), K12 (NASA-9, JSON mechanism).
//! Limits are those registered in the ADR before any run.

use rcfd_reactor::equil::equilibrium_tp;
use rcfd_reactor::json::{from_json, from_json_str, to_json};
use rcfd_reactor::mech::{Mechanism, Nasa9Range, R_GAS};
use rcfd_reactor::network::{psr_then_plug, sensitivities};
use rcfd_reactor::ode::{integrate, Flow, Options};
use rcfd_reactor::pollutants::*;
use rcfd_reactor::reactors::*;
use rcfd_reactor::toy;

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs().max(1e-300)
}

// ---------- K10 ----------

fn k10_rates() -> ZeldovichRates {
    // invented constants (SI), chosen only to separate the time scales; they are not literature values
    let c = |a: f64| Arrhenius { a, b: 0.0, ta: 0.0 };
    ZeldovichRates {
        k1f: c(1.0e2),
        k1r: c(1.0e6),
        k2f: c(1.0e4),
        k2r: c(1.0e-2),
    }
}

/// Registered K10 (ii) comparison: integrated two-reaction system against the closed form, worst relative difference.
fn k10_quasi_steady(k1f: f64, no0: f64) -> f64 {
    let mut rates = k10_rates();
    rates.k1f.a = k1f;
    let k = rates.at(2000.0);
    let (o, n2, o2) = (1.0e-2, 10.0, 1.0);
    // integrated two-reaction system, N and NO evolving
    let rhs = |_t: f64, y: &[f64], f: &mut [f64]| {
        let (n, no) = (y[0], y[1]);
        let r1 = k.k1f * o * n2 - k.k1r * no * n;
        let r2 = k.k2f * n * o2 - k.k2r * no * o;
        f[0] = r1 - r2;
        f[1] = r1 + r2;
    };
    let mut worst = 0.0_f64;
    for mult in [20.0, 50.0] {
        let tau_n = 1.0 / (k.k1r * no0 + k.k2f * o2);
        let t_end = mult * tau_n;
        let opt = Options::new(1e-12, 1e-18, 2);
        let mut f = rhs;
        let (y, _) = integrate(&mut f, 0.0, t_end, &[0.0, no0], &opt, &mut |_| Flow::Continue).unwrap();
        let mut d = [0.0; 2];
        rhs(0.0, &y, &mut d);
        let model = thermal_no_rate(&k, o, n2, o2, y[1]);
        let e = rel(model, d[1]);
        println!(
            "K10 (ii) t = {mult} tau_N: integrated dNO/dt {:.6e}, closed form {model:.6e}, relative {e:.3e} (limit 1e-3)",
            d[1]
        );
        worst = worst.max(e);
    }
    worst
}

/// Registered set-up (k1f = 1e2, [NO]_0 = 1e-3): the N relaxation time 1/(k1r[NO] + k2f[O2]) = 1e-4 s is not short against the
/// NO growth time (about 1.4e-4 s), so the quasi-steady premise fails. Kept as a known miss (ADR 0027 addendum A1).
#[test]
#[ignore = "known miss: scale separation of the registered set-up is insufficient, see ADR 0027 addendum A1"]
fn k10_known_miss_quasi_steady_registered_setup() {
    let w = k10_quasi_steady(1.0e2, 1.0e-3);
    assert!(w <= 1e-3, "K10 quasi-steady {w:e}");
}

/// Corrected set-up (k1f = 1e-1, [NO]_0 = 1e-1): N relaxes in about 1e-5 s while NO changes on a time of seconds.
#[test]
fn k10_quasi_steady_with_scale_separation() {
    let w = k10_quasi_steady(1.0e-1, 1.0e-1);
    println!("K10 (ii) corrected set-up: worst relative difference {w:.3e} (limit 1e-3)");
    assert!(w <= 1e-3, "K10 quasi-steady {w:e}");
}

#[test]
fn k10_thermal_no_closed_form_quasi_steady_and_balance() {
    let k = k10_rates().at(2000.0);
    let (o, n2, o2) = (1.0e-2, 10.0, 1.0);
    // (i) closed form
    for no in [1.0e-6, 1.0e-3, 0.05] {
        let f = 2.0 * (k.k1f * k.k2f * o * n2 * o2 - k.k1r * k.k2r * no * no * o) / (k.k1r * no + k.k2f * o2);
        let r = thermal_no_rate(&k, o, n2, o2, no);
        assert!(rel(r, f) <= 1e-12, "closed form {r} {f}");
    }
    // (iii) detailed balance
    let nob = no_balance(&k, n2, o2);
    let r = thermal_no_rate(&k, o, n2, o2, nob);
    let fwd = 2.0 * k.k1f * k.k2f * o * n2 * o2 / (k.k1r * nob + k.k2f * o2);
    println!(
        "K10 (iii) rate at balance {r:.3e}, relative to forward term {:.3e} (limit 1e-12)",
        r.abs() / fwd
    );
    assert!(r.abs() / fwd <= 1e-12);
    // N steady value consistent with the rate
    let n = n_steady(&k, o, n2, o2, 1.0e-3);
    let r1 = k.k1f * o * n2 - k.k1r * 1.0e-3 * n;
    assert!(rel(2.0 * r1, thermal_no_rate(&k, o, n2, o2, 1.0e-3)) <= 1e-12);
}

// ---------- K11 ----------

fn chain_mech() -> Mechanism {
    let mech =
        "ELEMENTS X/20.0/ END\nSPECIES A B C END\nREACTIONS\nA => B  1.0E8 0.0 19872.0\nB => C  1.0E8 0.0 19872.0\nEND\n";
    let e = vec![
        ("A", vec![("X", 1.0)], 2.5, 0.0, 4.0),
        ("B", vec![("X", 1.0)], 2.5, -3000.0, 4.0),
        ("C", vec![("X", 1.0)], 2.5, -3000.0, 4.0),
    ];
    Mechanism::from_chemkin(mech, Some(&toy::thermo_text(&e))).unwrap()
}

fn delay(m: &Mechanism) -> Result<f64, ReactorError> {
    let rho = 1.5;
    let y0 = vec![800.0, 1.0, 0.0, 0.0];
    let p = rho * R_GAS * 800.0 / m.mean_w(&y0[1..]);
    let tol = Tol {
        rtol: 1e-12,
        atol_t: 1e-9,
        atol_y: 1e-16,
    };
    let tr = run_batch(
        m,
        BatchKind::ConstantVolume,
        Energy::Adiabatic,
        p,
        &y0,
        6.0e-4,
        tol,
        Stop::EndTime,
    )?;
    tr.time_of_rise(1000.0)
        .ok_or_else(|| ReactorError::Input("no ignition".into()))
}

#[test]
fn k11a_sensitivity_of_the_ignition_delay() {
    let m = chain_mech();
    let s = sensitivities(&m, 1.0e-3, &delay).unwrap();
    println!(
        "K11a sensitivities (reaction, S): {:?}; delay {:.6e} s",
        s,
        delay(&m).unwrap()
    );
    let s1 = s.iter().find(|x| x.reaction == 0).unwrap().coefficient;
    let s2 = s.iter().find(|x| x.reaction == 1).unwrap().coefficient;
    println!(
        "K11a |S1 + 1| = {:.3e} (limit 1e-4), |S2| = {:.3e} (limit 1e-4)",
        (s1 + 1.0).abs(),
        s2.abs()
    );
    assert_eq!(s[0].reaction, 0, "ranking");
    assert!((s1 + 1.0).abs() <= 1e-4);
    assert!(s2.abs() <= 1e-4);
}

#[test]
fn k11b_psr_into_plug_flow() {
    let m = toy::isomer(5.0, 0.0, 0.0, false);
    let tol = Tol::default();
    for tau in [0.1, 0.5] {
        let psr = Psr {
            tau,
            p: 1.0e5,
            t_in: 1000.0,
            y_in: vec![1.0, 0.0],
            energy: Energy::Isothermal,
        };
        let net = psr_then_plug(&m, &psr, &[1000.0, 1.0, 0.0], 1.0, 0.2, tol).unwrap();
        let ya = 1.0 / (1.0 + 5.0 * tau);
        let e1 = rel(net.psr[1], ya);
        let end = net.plug.last();
        let e2 = rel(end[1], ya * (-5.0 * 0.2_f64).exp());
        println!(
            "K11b tau {tau}: PSR rel err {e1:.3e}, plug rel err {e2:.3e} (limit 1e-8), residence time {:.12}",
            end[end.len() - 1]
        );
        assert!(e1 <= 1e-8 && e2 <= 1e-8);
    }
}

// ---------- K12 ----------

#[test]
fn k12a_nasa9_consistency_and_mapping() {
    let r = Nasa9Range {
        t_low: 200.0,
        t_high: 6000.0,
        a: [1.2e3, -5.0e1, 3.1, 2.0e-3, -1.5e-7, 4.0e-11, -2.0e-15],
        b: [-8.0e3, 4.5],
    };
    let mut worst = 0.0_f64;
    for t in [400.0, 1500.0, 4000.0] {
        let h = 1.0e-3;
        let dh = ((t + h) * r.h_rt(t + h) - (t - h) * r.h_rt(t - h)) / (2.0 * h);
        let ds = (r.s_r(t + h) - r.s_r(t - h)) / (2.0 * h);
        let e1 = rel(dh, r.cp_r(t));
        let e2 = rel(ds, r.cp_r(t) / t);
        println!("K12a T {t}: d(h/RT*T)/dT vs cp/R {e1:.3e}, d(s/R)/dT vs cp/(R T) {e2:.3e} (limit 1e-7)");
        worst = worst.max(e1).max(e2);
    }
    assert!(worst <= 1e-7);
    // mapping of a NASA-7 species
    let mut m = toy::isomer(1.0, 0.0, 0.0, false);
    let sp = &mut m.species[0];
    sp.low = [3.5, -1.0e-3, 2.0e-6, -1.0e-9, 3.0e-13, -1.0e3, 3.0];
    sp.high = [2.9, 1.5e-3, -5.0e-7, 4.0e-11, -2.0e-15, -9.0e2, 5.0];
    let mut s9 = sp.clone();
    s9.nasa9 = Some(sp.to_nasa9());
    let mut worst = 0.0_f64;
    for t in [300.0, 1000.0, 3000.0] {
        worst = worst
            .max(rel(s9.cp_r(t), sp.cp_r(t)))
            .max(rel(s9.h_rt(t), sp.h_rt(t)))
            .max(rel(s9.s_r(t), sp.s_r(t)));
    }
    println!("K12a mapping: max relative difference {worst:.3e} (limit 1e-14)");
    assert!(worst <= 1e-14);
}

fn json_mech() -> Mechanism {
    let mech = "ELEMENTS X/20.0/ Y/30.0/ END\nSPECIES X2 Y2 XY X END\nREACTIONS\n\
X2 + Y2 <=> 2XY  1.0E9 0.0 0.0\n\
X2 + M <=> 2X + M  1.0E10 0.5 5000.0\n X2/2.0/ XY/0.5/\n\
2X (+M) <=> X2 (+M)  1.0E10 0.0 0.0\n LOW / 1.0E12 0.0 0.0 /\n TROE / 0.6 100.0 1000.0 /\n X2/2.0/\n\
END\n";
    let e = vec![
        ("X2", vec![("X", 2.0)], 2.5, 0.0, 4.0),
        ("Y2", vec![("Y", 2.0)], 2.5, 0.0, 4.0),
        ("XY", vec![("X", 1.0), ("Y", 1.0)], 2.5, -2000.0, 4.0),
        ("X", vec![("X", 1.0)], 2.5, 3000.0, 3.0),
    ];
    Mechanism::from_chemkin(mech, Some(&toy::thermo_text(&e))).expect("mechanism with third body and falloff")
}

#[test]
fn k12b_json_round_trip() {
    let m = json_mech();
    let text = to_json(&m).to_string();
    let back = from_json_str(&text).unwrap();
    let back2 = from_json(&to_json(&m)).unwrap();
    assert_eq!(back.species.len(), m.species.len());
    assert_eq!(back.reactions.len(), m.reactions.len());
    assert_eq!(back2.reactions.len(), m.reactions.len());
    let x0 = [0.3, 0.3, 0.2, 0.2];
    let mut worst_q = 0.0_f64;
    for (t, c) in [(1000.0, 5.0), (1800.0, 20.0), (2600.0, 1.0)] {
        let cs: Vec<f64> = x0.iter().map(|x| x * c).collect();
        let (mut q0, mut q1) = (vec![0.0; 3], vec![0.0; 3]);
        m.rates_of_progress(t, &cs, &mut q0);
        back.rates_of_progress(t, &cs, &mut q1);
        for (a, b) in q0.iter().zip(&q1) {
            worst_q = worst_q.max(rel(*b, *a));
        }
    }
    let mut worst_e = 0.0_f64;
    for (t, p) in [(1500.0, 1.0e5), (2500.0, 1.0e6), (1000.0, 2.0e5)] {
        let a = equilibrium_tp(&m, &x0, t, p).unwrap();
        let b = equilibrium_tp(&back, &x0, t, p).unwrap();
        for (u, v) in a.x.iter().zip(&b.x) {
            worst_e = worst_e.max(rel(*v, *u));
        }
    }
    println!(
        "K12b: counts {}/{} species/reactions equal; rates_of_progress rel {worst_q:.3e} (limit 1e-14); equilibrium rel {worst_e:.3e} (limit 1e-12)",
        m.species.len(),
        m.reactions.len()
    );
    assert!(worst_q <= 1e-14 && worst_e <= 1e-12);
    assert_eq!(back, m);
    // a bad document is refused with a message
    assert!(from_json_str("{\"format\": \"other\"}").is_err());
}
