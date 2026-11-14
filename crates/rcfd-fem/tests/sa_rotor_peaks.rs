//! F-564 (ADR 0501): damped critical speeds as unbalance-response peaks and checked Campbell crossings of the flexible rotor of
//! ADR 0476. Checks P1 to P5, N1 to N3, K1 to K5, X1 of the registration (B1 is in `sa_rotor_peaks_default_bits.rs`).

use rcfd_fem::beam;
use rcfd_fem::rotating::Whirl;
use rcfd_fem::rotor::{Bearing, Disc, RotorCrossing, RotorModel, Unbalance, C64};
use rcfd_fem::{FemError, Material};
use std::f64::consts::PI;

struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        assert!(value.is_finite(), "{name}: the error is not finite ({value})");
        let ok = value <= limit;
        println!(
            "{name}: {value:.4e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} > {limit:e}"));
        }
    }
    fn above(&mut self, name: &str, value: f64, limit: f64) {
        assert!(value.is_finite(), "{name}: the value is not finite ({value})");
        let ok = value > limit;
        println!(
            "{name}: {value:.4e} (limit > {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} <= {limit:e}"));
        }
    }
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {detail} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name}: {detail}"));
        }
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:#?}", self.0);
    }
}

const E: f64 = 2.0e11;
const NU: f64 = 0.3;
const KAPPA: f64 = 0.9;
const R: f64 = 0.025;
const M: f64 = 20.0;
const B0: f64 = 2.0e6;
const AMOUNT: f64 = 0.01;
const ZETAS: [f64; 4] = [0.05, 0.1, 0.2, 0.3];

fn rel(a: f64, b: f64) -> f64 {
    (a / b - 1.0).abs()
}

fn steel(rho: f64) -> Material {
    Material::isotropic_elastic(E, NU, rho, 0.0).unwrap()
}

fn key_of<T: std::fmt::Debug>(r: &Result<T, FemError>) -> Option<&'static str> {
    match r {
        Err(FemError::InvalidValue { key, .. }) => Some(key),
        _ => None,
    }
}

/// Tip stiffness of the massless Timoshenko cantilever of 0.5 m (ADR 0498 D5).
fn ks() -> f64 {
    let (a, i, g) = (PI * R * R, PI * R.powi(4) / 4.0, E / (2.0 * (1.0 + NU)));
    let l = 0.5_f64;
    1.0 / (l.powi(3) / (3.0 * E * i) + l / (KAPPA * g * a))
}

fn wn() -> f64 {
    ((ks() + B0) / M).sqrt()
}

/// Model Q of ADR 0501 with the damping ratio `zeta`.
fn model_q(zeta: f64) -> RotorModel {
    let mut m = RotorModel::new(vec![0.0, 0.5]).unwrap();
    m.add_shaft(0, 1, beam::circle_section(R, KAPPA).unwrap(), &steel(0.0))
        .unwrap();
    m.clamp(0).unwrap();
    m.add_disc(
        1,
        Disc {
            mass: M,
            transverse: 0.0,
            polar: 0.0,
        },
    )
    .unwrap();
    m.add_bearing(1, Bearing::isotropic(B0, 2.0 * zeta * M * wn())).unwrap();
    m
}

/// Model G of ADR 0501.
fn model_g() -> RotorModel {
    let z: Vec<f64> = (0..5).map(|i| 0.25 * i as f64).collect();
    let mut m = RotorModel::new(z).unwrap();
    let s = beam::circle_section(R, KAPPA).unwrap();
    for i in 0..4 {
        m.add_shaft(i, i + 1, s, &steel(7850.0)).unwrap();
    }
    m.add_disc(
        2,
        Disc {
            mass: 20.0,
            transverse: 0.05,
            polar: 0.1,
        },
    )
    .unwrap();
    m.add_disc(
        3,
        Disc {
            mass: 5.0,
            transverse: 0.01,
            polar: 0.02,
        },
    )
    .unwrap();
    let b = Bearing::isotropic(2.0e6, 0.0);
    m.add_bearing(0, b).unwrap();
    m.add_bearing(4, b).unwrap();
    m
}

fn unbalance(angle: f64) -> [Unbalance; 1] {
    [Unbalance {
        node: 1,
        amount: AMOUNT,
        angle,
    }]
}

// ------------------------------------------------------------------------------------------------- P1 to P4, N1 to N3

#[test]
fn p1_p4_n1_n3_unbalance_peak_of_the_damped_jeffcott_rotor() {
    let mut c = Checks::new();
    let w = wn();
    let e = AMOUNT / M;
    println!("w_n = {w:.9} rad/s (closed form)");
    let (mut speed_err, mut amp_err) = (0.0_f64, 0.0_f64);
    for zeta in ZETAS {
        let q = model_q(zeta);
        let peaks = q.unbalance_peaks(1, &unbalance(0.0), 0.3 * w, 3.0 * w, 600).unwrap();
        c.holds(
            &format!("P3 zeta {zeta}"),
            peaks.len() == 1 && peaks[0].direction == Whirl::Forward,
            &format!("{} peaks", peaks.len()),
        );
        let p = peaks[0];
        let want = w / (1.0 - 2.0 * zeta * zeta).sqrt();
        let want_amp = e / (2.0 * zeta * (1.0 - zeta * zeta).sqrt());
        println!(
            "zeta {zeta}: peak {:.9} rad/s (closed {want:.9}), amplitude {:.9e} (closed {want_amp:.9e})",
            p.speed, p.amplitude
        );
        speed_err = speed_err.max(rel(p.speed, want));
        amp_err = amp_err.max(rel(p.amplitude, want_amp));
        let force_form = w * (1.0 - 2.0 * zeta * zeta).sqrt();
        c.above(
            &format!("N1 zeta {zeta} constant-force form"),
            rel(force_form, p.speed),
            1e-3,
        );
        c.above(&format!("N2 zeta {zeta} undamped w_n"), rel(w, p.speed), 1e-3);
    }
    c.at_most("P1 peak speed", speed_err, 1e-6);
    c.at_most("P2 peak amplitude", amp_err, 1e-6);

    let q = model_q(0.1);
    let peaks = q.unbalance_peaks(1, &unbalance(0.7), 0.3 * w, 3.0 * w, 600).unwrap();
    c.holds("P4 count", peaks.len() == 1, &format!("{}", peaks.len()));
    c.at_most(
        "P4 peak speed at angle 0.7",
        rel(peaks[0].speed, w / (1.0 - 2.0 * 0.01_f64).sqrt()),
        1e-6,
    );

    let over = model_q(0.8)
        .unbalance_peaks(1, &unbalance(0.0), 0.3 * w, 3.0 * w, 600)
        .unwrap();
    c.holds(
        "N3 zeta 0.8 has no peak",
        over.is_empty(),
        &format!("{} peaks", over.len()),
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------- P5

/// Grid plus golden-section maximum of `f` on `[lo, hi]` (the method of ADR 0501 D2, written out for the test).
fn maximum(f: &dyn Fn(f64) -> f64, lo: f64, hi: f64, samples: usize) -> f64 {
    let x: Vec<f64> = (0..=samples)
        .map(|i| lo + (hi - lo) * i as f64 / samples as f64)
        .collect();
    let v: Vec<f64> = x.iter().map(|&w| f(w)).collect();
    let i = (1..samples)
        .find(|&i| v[i] > v[i - 1] && v[i] >= v[i + 1])
        .expect("an interior maximum");
    let gr = 0.5 * (5.0_f64.sqrt() - 1.0);
    let (mut a, mut b) = (x[i - 1], x[i + 1]);
    let mut cc = b - gr * (b - a);
    let mut d = a + gr * (b - a);
    let (mut fc, mut fd) = (f(cc), f(d));
    for _ in 0..200 {
        if b - a <= 4.0 * f64::EPSILON * b {
            break;
        }
        if fc > fd {
            b = d;
            d = cc;
            fd = fc;
            cc = b - gr * (b - a);
            fc = f(cc);
        } else {
            a = cc;
            cc = d;
            fc = fd;
            d = a + gr * (b - a);
            fd = f(d);
        }
    }
    0.5 * (a + b)
}

#[test]
fn p5_constant_force_peak_is_at_wn_sqrt_one_minus_two_zeta_squared() {
    let mut c = Checks::new();
    let w = wn();
    let mut err = 0.0_f64;
    for zeta in [0.1, 0.3] {
        let q = model_q(zeta);
        let load = [(1usize, [C64::new(1.0, 0.0), C64::new(0.0, -1.0), C64::ZERO, C64::ZERO])];
        let f = |om: f64| q.harmonic_response(0.0, om, &load).unwrap().orbits[1].major;
        let peak = maximum(&f, 0.3 * w, 3.0 * w, 600);
        let want = w * (1.0 - 2.0 * zeta * zeta).sqrt();
        println!("zeta {zeta}: constant-force peak {peak:.9} (closed {want:.9})");
        err = err.max(rel(peak, want));
    }
    c.at_most("P5 constant-force peak", err, 1e-6);
    c.finish();
}

// ------------------------------------------------------------------------------------------------- K1, K2

fn range_speeds() -> Vec<f64> {
    (0..=10).map(|i| 100.0 * i as f64).collect()
}

#[test]
fn k1_k2_k4_crossing_of_the_jeffcott_tracks() {
    let mut c = Checks::new();
    let w = wn();
    let q0 = model_q(0.0);
    let crit = q0.critical_speeds().unwrap()[0].speed;
    let cr = q0.campbell_crossings(&range_speeds(), 1).unwrap();
    c.holds("K1 two crossings", cr.len() == 2, &format!("{}", cr.len()));
    let e1 = cr.iter().map(|x| rel(x.speed, w)).fold(0.0_f64, f64::max);
    let e2 = cr.iter().map(|x| rel(x.speed, crit)).fold(0.0_f64, f64::max);
    c.at_most("K1 against the closed form w_n", e1, 1e-6);
    c.at_most("K1 against critical_speeds()", e2, 1e-6);
    let mut res = cr.iter().map(|x| x.residual).fold(0.0_f64, f64::max);
    for zeta in [0.1, 0.3] {
        let q = model_q(zeta);
        let cr = q.campbell_crossings(&range_speeds(), 1).unwrap();
        c.holds(
            &format!("K2 zeta {zeta} two crossings"),
            cr.len() == 2,
            &format!("{}", cr.len()),
        );
        let wd = w * (1.0 - zeta * zeta).sqrt();
        let peak = w / (1.0 - 2.0 * zeta * zeta).sqrt();
        let e = cr.iter().map(|x| rel(x.speed, wd)).fold(0.0_f64, f64::max);
        c.at_most(&format!("K2 zeta {zeta} crossing against w_d"), e, 1e-6);
        let d1 = cr.iter().map(|x| rel(x.speed, w)).fold(f64::INFINITY, f64::min);
        let d2 = cr.iter().map(|x| rel(x.speed, peak)).fold(f64::INFINITY, f64::min);
        c.above(&format!("K2 zeta {zeta} differs from w_n"), d1, 1e-3);
        c.above(&format!("K2 zeta {zeta} differs from the response peak"), d2, 1e-3);
        res = res.max(cr.iter().map(|x| x.residual).fold(0.0_f64, f64::max));
    }
    c.at_most("K4 residual (Jeffcott)", res, 1e-9);
    c.finish();
}

// ------------------------------------------------------------------------------------------------- K3, K3i, K4, K5

fn forward_criticals(g: &RotorModel) -> Vec<f64> {
    g.critical_speeds()
        .unwrap()
        .iter()
        .filter(|s| s.direction == Whirl::Forward)
        .map(|s| s.speed)
        .collect()
}

fn forward(cr: Vec<RotorCrossing>) -> Vec<RotorCrossing> {
    cr.into_iter().filter(|x| x.direction == Whirl::Forward).collect()
}

#[test]
fn k3_k3i_k4_k5_forward_crossings_of_the_gyroscopic_rotor() {
    let mut c = Checks::new();
    let g = model_g();
    let fwd = forward_criticals(&g);
    println!("forward critical speeds of G: {fwd:?}");
    assert!(fwd.len() >= 2, "G needs two forward critical speeds");
    let top = 1.2 * fwd[1];
    let speeds: Vec<f64> = (0..=40).map(|i| top * i as f64 / 40.0).collect();
    let want: Vec<f64> = fwd.iter().copied().filter(|&s| s <= top).collect();
    let cr = forward(g.campbell_crossings(&speeds, 1).unwrap());
    for x in &cr {
        println!(
            "order 1: track {} speed {:.9} direction {:?} residual {:.2e}",
            x.track, x.speed, x.direction, x.residual
        );
    }
    c.holds(
        "K3 count",
        cr.len() == want.len(),
        &format!("{} crossings, {} critical speeds", cr.len(), want.len()),
    );
    let n = cr.len().min(want.len());
    let err = (0..n).map(|i| rel(cr[i].speed, want[i])).fold(0.0_f64, f64::max);
    c.at_most("K3 crossing against critical_speeds()", err, 1e-6);
    let mut res = cr.iter().map(|x| x.residual).fold(0.0_f64, f64::max);

    // K3i: the interpolated crossings of the existing table (recorded, no limit).
    let table = g.campbell(&speeds).unwrap().campbell;
    let mut lin: Vec<(f64, usize)> = table
        .crossings(1)
        .into_iter()
        .filter(|x| table.directions[x.track].iter().flatten().any(|d| *d == Whirl::Forward))
        .map(|x| (x.speed, x.track))
        .collect();
    lin.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (s, t) in &lin {
        let nearest = want.iter().map(|w| rel(*s, *w)).fold(f64::INFINITY, f64::min);
        println!(
            "K3i interpolated crossing {s:.9} (track {t}), error against the nearest critical speed {nearest:.3e}"
        );
    }

    let top2 = fwd[1] * 1.2;
    let two = forward(g.campbell_crossings(&speeds, 2).unwrap());
    for x in &two {
        println!(
            "order 2: track {} speed {:.9} residual {:.2e}",
            x.track, x.speed, x.residual
        );
    }
    c.holds(
        "K5 order 2 crossings exist",
        !two.is_empty(),
        &format!("{} crossings", two.len()),
    );
    res = res.max(two.iter().map(|x| x.residual).fold(0.0_f64, f64::max));
    for x in &two {
        let same = cr.iter().find(|y| y.track == x.track);
        let ok = x.speed <= top2 && same.map_or(true, |y| x.speed < y.speed);
        c.holds(
            &format!("K5 track {} order 2 below order 1", x.track),
            ok,
            &format!("{:.6} rad/s", x.speed),
        );
    }
    c.at_most("K4 residual (gyroscopic)", res, 1e-9);
    c.finish();
}

// ------------------------------------------------------------------------------------------------- X1

#[test]
fn x1_refusals() {
    let mut c = Checks::new();
    let q = model_q(0.1);
    let w = wn();
    let ub = unbalance(0.0);
    let cases: Vec<(&str, Option<&'static str>, &str)> = vec![
        ("lo = 0", key_of(&q.unbalance_peaks(1, &ub, 0.0, w, 10)), "range"),
        ("lo >= hi", key_of(&q.unbalance_peaks(1, &ub, w, w, 10)), "range"),
        ("NaN", key_of(&q.unbalance_peaks(1, &ub, f64::NAN, w, 10)), "range"),
        ("2 samples", key_of(&q.unbalance_peaks(1, &ub, 1.0, w, 2)), "samples"),
        ("node 99", key_of(&q.unbalance_peaks(99, &ub, 1.0, w, 10)), "node"),
        (
            "negative amount",
            key_of(&q.unbalance_peaks(
                1,
                &[Unbalance {
                    node: 1,
                    amount: -1.0,
                    angle: 0.0,
                }],
                1.0,
                w,
                10,
            )),
            "unbalance",
        ),
        ("order 0", key_of(&q.campbell_crossings(&[0.0, 100.0], 0)), "order"),
        (
            "decreasing speeds",
            key_of(&q.campbell_crossings(&[100.0, 50.0], 1)),
            "speeds",
        ),
    ];
    for (name, got, want) in cases {
        c.holds(
            &format!("X1 {name}"),
            got == Some(want),
            &format!("{got:?}, want {want:?}"),
        );
    }
    c.finish();
}
