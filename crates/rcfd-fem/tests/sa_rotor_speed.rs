//! F-563 (ADR 0498): speed-dependent bearing coefficients in the flexible rotor of ADR 0476. Checks T1 to T4, L1 to L7, C3,
//! N1, N2, X1 of the registration (B1 is in `sa_rotor_speed_default_bits.rs`).

use rcfd_fem::beam;
use rcfd_fem::rotating::Whirl;
use rcfd_fem::rotor::{Bearing, Disc, RotorModel, SpeedBearing, Unbalance, C64};
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

fn section_constants() -> (f64, f64, f64) {
    (PI * R * R, PI * R.powi(4) / 4.0, E / (2.0 * (1.0 + NU)))
}

/// Tip stiffness of the massless Timoshenko cantilever of length `l` (ADR 0498 D5).
fn ks_cantilever(l: f64) -> f64 {
    let (a, i, g) = section_constants();
    1.0 / (l.powi(3) / (3.0 * E * i) + l / (KAPPA * g * a))
}

/// Midpoint stiffness of the massless simply supported Timoshenko shaft of length `l`.
fn ks_midpoint(l: f64) -> f64 {
    let (a, i, g) = section_constants();
    1.0 / (l.powi(3) / (48.0 * E * i) + l / (4.0 * KAPPA * g * a))
}

const B0: f64 = 2.0e6;
const B1: f64 = 1000.0;
const C0: f64 = 400.0;
const C1: f64 = 0.5;
const SB0: f64 = 3.0e6;
const SB1: f64 = 800.0;
const KNOT_HI: f64 = 1000.0;

fn linear_table(b0: f64, b1: f64, c0: f64, c1: f64) -> SpeedBearing {
    SpeedBearing::new(
        vec![0.0, KNOT_HI],
        vec![
            Bearing::isotropic(b0, c0),
            Bearing::isotropic(b0 + b1 * KNOT_HI, c0 + c1 * KNOT_HI),
        ],
    )
    .unwrap()
}

/// Model Q (ADR 0498 D5): massless cantilever of 0.5 m, a disc and a speed-dependent bearing at the tip.
fn model_q(damped: bool) -> RotorModel {
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
    let (c0, c1) = if damped { (C0, C1) } else { (0.0, 0.0) };
    m.add_speed_bearing(1, linear_table(B0, B1, c0, c1)).unwrap();
    m
}

/// Model S (ADR 0498 D5): massless simply supported shaft of 1 m on two speed-dependent bearings, the disc at the middle.
fn model_s() -> RotorModel {
    let mut m = RotorModel::new(vec![0.0, 0.5, 1.0]).unwrap();
    let s = beam::circle_section(R, KAPPA).unwrap();
    m.add_shaft(0, 1, s, &steel(0.0)).unwrap();
    m.add_shaft(1, 2, s, &steel(0.0)).unwrap();
    m.add_disc(
        1,
        Disc {
            mass: M,
            transverse: 0.0,
            polar: 0.0,
        },
    )
    .unwrap();
    m.add_speed_bearing(0, linear_table(SB0, SB1, 0.0, 0.0)).unwrap();
    m.add_speed_bearing(2, linear_table(SB0, SB1, 0.0, 0.0)).unwrap();
    m
}

fn q_stiffness(w: f64) -> f64 {
    ks_cantilever(0.5) + B0 + B1 * w
}

fn q_damping(w: f64) -> f64 {
    C0 + C1 * w
}

fn q_critical() -> f64 {
    (B1 + (B1 * B1 + 4.0 * M * (ks_cantilever(0.5) + B0)).sqrt()) / (2.0 * M)
}

/// The positive root of the cubic of ADR 0498 D5 by bisection of the polynomial.
fn s_critical() -> f64 {
    let a = 1.0 / ks_midpoint(1.0);
    let p3 = 2.0 * a * M * SB1;
    let p2 = M * (2.0 * a * SB0 + 1.0);
    let p1 = -2.0 * SB1;
    let p0 = -2.0 * SB0;
    let f = |w: f64| ((p3 * w + p2) * w + p1) * w + p0;
    let (mut lo, mut hi) = (0.0_f64, 1.0e5);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if f(mid) < 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

// ------------------------------------------------------------------------------------------------- L1, L2, N1, N2

#[test]
fn l1_l2_n1_n2_linear_table_critical_speed() {
    let mut c = Checks::new();
    let wq = q_critical();
    let ws = s_critical();
    println!("closed forms: Q {wq:.9} rad/s, S {ws:.9} rad/s");
    for (name, model, want) in [("L1 Q", model_q(true), wq), ("L2 S", model_s(), ws)] {
        let roots = model.critical_speeds_over(100.0, 1000.0, 90).unwrap();
        c.holds(
            &format!("{name} two roots"),
            roots.len() == 2,
            &format!("{}", roots.len()),
        );
        let err = roots.iter().map(|r| rel(r.speed, want)).fold(0.0_f64, f64::max);
        c.at_most(&format!("{name} critical speed"), err, 1e-6);
    }
    let solver = model_q(true).critical_speeds_over(100.0, 1000.0, 90).unwrap()[0].speed;
    let const_closed = ((ks_cantilever(0.5) + B0) / M).sqrt();
    let wrong_sign = (-B1 + (B1 * B1 + 4.0 * M * (ks_cantilever(0.5) + B0)).sqrt()) / (2.0 * M);
    c.above(
        "N1 closed form of the constant bearing",
        rel(const_closed, solver),
        1e-2,
    );
    c.above(
        "N2 closed form with the wrong sign of b1",
        rel(wrong_sign, solver),
        1e-2,
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------- L3, L4, L5

#[test]
fn l3_l4_l5_whirl_and_campbell() {
    let mut c = Checks::new();
    let (und, dmp) = (model_q(false), model_q(true));
    let mut e3 = 0.0_f64;
    let mut e4 = 0.0_f64;
    for w in [200.0, 400.0, 600.0] {
        let k = q_stiffness(w);
        let r = und.whirl(w).unwrap();
        c.holds(
            &format!("L3 {w} two modes"),
            r.modes.len() == 2,
            &format!("{}", r.modes.len()),
        );
        for md in &r.modes {
            e3 = e3.max(rel(md.omega, (k / M).sqrt()));
        }
        let cd = q_damping(w);
        let r = dmp.whirl(w).unwrap();
        c.holds(
            &format!("L4 {w} two modes"),
            r.modes.len() == 2,
            &format!("{}", r.modes.len()),
        );
        let od = (k / M - (cd / (2.0 * M)).powi(2)).sqrt();
        let lam = (od * od + (cd / (2.0 * M)).powi(2)).sqrt();
        for md in &r.modes {
            e4 = e4.max(((md.omega - od).abs() + (md.growth + cd / (2.0 * M)).abs()) / lam);
        }
    }
    c.at_most("L3 undamped whirl frequency", e3, 1e-9);
    c.at_most("L4 damped whirl root", e4, 1e-9);

    let speeds: Vec<f64> = (0..=10).map(|j| 100.0 * j as f64).collect();
    let camp = dmp.campbell(&speeds).unwrap();
    let tracks = &camp.campbell.table.tracks;
    c.holds("L5 two tracks", tracks.len() == 2, &format!("{}", tracks.len()));
    let mut e5 = 0.0_f64;
    for tr in tracks {
        for (j, &w) in speeds.iter().enumerate() {
            let f = tr.frequencies[j].expect("a tracked frequency at every speed");
            let (k, cd) = (q_stiffness(w), q_damping(w));
            let od = (k / M - (cd / (2.0 * M)).powi(2)).sqrt();
            e5 = e5.max(rel(2.0 * PI * f, od));
        }
    }
    c.at_most("L5 Campbell frequencies", e5, 1e-9);
    c.finish();
}

// ------------------------------------------------------------------------------------------------- L6

#[test]
fn l6_unbalance_response() {
    let mut c = Checks::new();
    let model = model_q(true);
    let wc = q_critical();
    let u = 0.01;
    let mut e = 0.0_f64;
    for f in [0.5, 1.0, 2.0] {
        let w = f * wc;
        assert!(w < KNOT_HI);
        let r = model
            .unbalance_response(
                w,
                &[Unbalance {
                    node: 1,
                    amount: u,
                    angle: 0.0,
                }],
            )
            .unwrap();
        let k = q_stiffness(w);
        let cd = q_damping(w);
        let radius = u * w * w / ((k - M * w * w).powi(2) + (w * cd).powi(2)).sqrt();
        let o = &r.orbits[1];
        e = e.max(rel(o.major, radius)).max(rel(o.minor, radius));
        c.holds(
            &format!("L6 {f} direction"),
            o.direction == Whirl::Forward,
            &format!("{:?}", o.direction),
        );
    }
    c.at_most("L6 orbit radius", e, 1e-9);
    c.finish();
}

// ------------------------------------------------------------------------------------------------- L7, T3, T4

#[test]
fn l7_t3_t4_table_behaviour() {
    let mut c = Checks::new();
    // L7: outside the table the end bearing.
    let table = SpeedBearing::new(
        vec![100.0, 800.0],
        vec![
            Bearing::isotropic(B0 + B1 * 100.0, C0 + C1 * 100.0),
            Bearing::isotropic(B0 + B1 * 800.0, C0 + C1 * 800.0),
        ],
    )
    .unwrap();
    let build = |b: Option<Bearing>| {
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
        match b {
            Some(b) => m.add_bearing(1, b).unwrap(),
            None => m.add_speed_bearing(1, table.clone()).unwrap(),
        }
        m
    };
    let t = build(None);
    for (w, which) in [(50.0, 0usize), (900.0, 1)] {
        let b = table.bearings()[which];
        let want = build(Some(b)).whirl(w).unwrap();
        c.holds(
            &format!("L7 clamp at {w}"),
            format!("{:?}", t.whirl(w).unwrap()) == format!("{want:?}"),
            "bits",
        );
    }
    // T3: the interpolation.
    let tb = SpeedBearing::new(
        vec![0.0, 10.0],
        vec![Bearing::isotropic(1.0, 2.0), Bearing::isotropic(3.0, 6.0)],
    )
    .unwrap();
    let mid = tb.at(5.0);
    c.holds("T3 midpoint", mid == Bearing::isotropic(2.0, 4.0), &format!("{mid:?}"));
    c.holds(
        "T3 knots",
        tb.at(0.0) == tb.bearings()[0] && tb.at(10.0) == tb.bearings()[1],
        "bits",
    );
    c.holds(
        "T3 covers",
        !tb.covers(-1.0) && !tb.covers(11.0) && tb.covers(5.0),
        "range",
    );
    // T4: matrices() of a table model is matrices_at(0) and the constant bearing at 0.
    let m0 = format!("{:?}", t.matrices());
    c.holds(
        "T4 matrices == matrices_at(0)",
        m0 == format!("{:?}", t.matrices_at(0.0)),
        "bits",
    );
    c.holds(
        "T4 matrices == constant bearing at 0",
        m0 == format!("{:?}", build(Some(table.at(0.0))).matrices()),
        "bits",
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------- T1, T2, C3

fn bearing_x(cross: bool, scale: f64) -> Bearing {
    let k = if cross {
        [[2.0e6, 3.0e5], [-1.0e5, 3.0e6]]
    } else {
        [[2.0e6, 1.0e5], [1.0e5, 3.0e6]]
    };
    Bearing::new(
        [[k[0][0] * scale, k[0][1] * scale], [k[1][0] * scale, k[1][1] * scale]],
        [[200.0 * scale, 20.0], [20.0, 300.0 * scale]],
    )
}

/// Model X or Y of ADR 0498 D7 with the given bearing installer.
fn model_xy(install: &dyn Fn(&mut RotorModel, usize)) -> RotorModel {
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
    install(&mut m, 0);
    install(&mut m, 4);
    m
}

fn dbg<T: std::fmt::Debug>(v: &T) -> String {
    format!("{v:?}")
}

/// Every output of the model that the checks compare, as text (equal text is equal bits).
fn outputs(m: &RotorModel, critical: bool) -> Vec<String> {
    let mut o = Vec::new();
    for w in [0.0, 300.0, 900.0] {
        o.push(dbg(&m.whirl(w).unwrap()));
    }
    let camp = m.campbell(&[0.0, 200.0, 400.0, 600.0, 800.0]).unwrap();
    o.push(dbg(&camp));
    o.push(camp.to_csv());
    let loads = [(2usize, [C64::new(1.0, 0.5), C64::new(-0.3, 0.2), C64::ZERO, C64::ZERO])];
    o.push(dbg(&m.harmonic_response(300.0, 123.4, &loads).unwrap()));
    let ub = [
        Unbalance {
            node: 2,
            amount: 0.01,
            angle: 0.3,
        },
        Unbalance {
            node: 3,
            amount: 0.004,
            angle: -1.0,
        },
    ];
    o.push(dbg(&m.unbalance_response(300.0, &ub).unwrap()));
    o.push(dbg(&m.unbalance_response(900.0, &ub).unwrap()));
    o.push(dbg(&m.matrices()));
    if critical {
        o.push(dbg(&m.critical_speeds().unwrap()));
    }
    o
}

#[test]
fn t1_constant_table_gives_the_bits_of_the_constant_bearing() {
    let mut c = Checks::new();
    for (cross, critical) in [(true, false), (false, true)] {
        let b = bearing_x(cross, 1.0);
        let plain = model_xy(&|m, n| m.add_bearing(n, b).unwrap());
        let want = outputs(&plain, critical);
        for (name, speeds) in [
            ("1 knot", vec![0.0]),
            ("2 knots", vec![0.0, 500.0]),
            ("3 knots", vec![0.0, 500.0, 1500.0]),
        ] {
            let table = SpeedBearing::new(speeds.clone(), vec![b; speeds.len()]).unwrap();
            let tabled = model_xy(&|m, n| m.add_speed_bearing(n, table.clone()).unwrap());
            let got = outputs(&tabled, critical);
            let same = got.len() == want.len() && got.iter().zip(&want).all(|(a, b)| a == b);
            c.holds(
                &format!("T1 {} {name}", if cross { "X" } else { "Y" }),
                same,
                "every output bit-equal",
            );
        }
    }
    c.finish();
}

#[test]
fn t2_table_at_a_knot_gives_the_bits_of_that_bearing() {
    let mut c = Checks::new();
    let knots = vec![100.0, 300.0, 900.0];
    let bs: Vec<Bearing> = (0..3).map(|j| bearing_x(true, 1.0 + 0.1 * j as f64)).collect();
    let table = SpeedBearing::new(knots.clone(), bs.clone()).unwrap();
    let tabled = model_xy(&|m, n| m.add_speed_bearing(n, table.clone()).unwrap());
    let ub = [Unbalance {
        node: 2,
        amount: 0.01,
        angle: 0.3,
    }];
    for (j, &w) in knots.iter().enumerate() {
        let b = bs[j];
        let plain = model_xy(&|m, n| m.add_bearing(n, b).unwrap());
        let same = dbg(&tabled.whirl(w).unwrap()) == dbg(&plain.whirl(w).unwrap())
            && dbg(&tabled.unbalance_response(w, &ub).unwrap()) == dbg(&plain.unbalance_response(w, &ub).unwrap());
        c.holds(&format!("T2 knot {w}"), same, "whirl and unbalance response bit-equal");
    }
    // between the knots the table is not the end bearing (the check can fail)
    let mid = tabled.whirl(200.0).unwrap();
    let low = model_xy(&|m, n| m.add_bearing(n, bs[0]).unwrap()).whirl(200.0).unwrap();
    c.holds("T2 control: between knots differs", dbg(&mid) != dbg(&low), "differs");
    c.finish();
}

#[test]
fn c3_constant_model_agrees_with_critical_speeds() {
    let mut c = Checks::new();
    let b = bearing_x(false, 1.0);
    let y = model_xy(&|m, n| m.add_bearing(n, b).unwrap());
    let all = y.critical_speeds().unwrap();
    let (w1, w2) = (all[0].speed, all[1].speed);
    let (lo, hi) = (0.3 * w1, 3.0 * w2);
    let want: Vec<f64> = all.iter().map(|x| x.speed).filter(|&s| s >= lo && s <= hi).collect();
    let got: Vec<f64> = y
        .critical_speeds_over(lo, hi, 400)
        .unwrap()
        .iter()
        .map(|x| x.speed)
        .collect();
    c.holds(
        "C3 same count",
        got.len() == want.len() && want.len() >= 2,
        &format!("{} / {}", got.len(), want.len()),
    );
    let err = got.iter().zip(&want).map(|(a, b)| rel(*a, *b)).fold(0.0_f64, f64::max);
    c.at_most("C3 critical speeds", err, 1e-12);
    c.finish();
}

// ------------------------------------------------------------------------------------------------- X1

#[test]
fn x1_refusals() {
    let mut c = Checks::new();
    let b = Bearing::isotropic(1.0, 1.0);
    let nan = Bearing::new([[f64::NAN, 0.0], [0.0, 0.0]], [[0.0; 2]; 2]);
    let cases: Vec<(&str, Option<&'static str>)> = vec![
        ("empty", key_of(&SpeedBearing::new(vec![], vec![]))),
        ("lengths", key_of(&SpeedBearing::new(vec![0.0, 1.0], vec![b]))),
        ("decreasing", key_of(&SpeedBearing::new(vec![1.0, 0.0], vec![b, b]))),
        ("repeated", key_of(&SpeedBearing::new(vec![1.0, 1.0], vec![b, b]))),
        ("negative", key_of(&SpeedBearing::new(vec![-1.0, 1.0], vec![b, b]))),
        ("nan speed", key_of(&SpeedBearing::new(vec![f64::NAN], vec![b]))),
        ("nan entry", key_of(&SpeedBearing::new(vec![0.0], vec![nan]))),
    ];
    for (n, k) in cases {
        c.holds(&format!("X1 {n}"), k == Some("speed_bearing"), &format!("{k:?}"));
    }
    let mut m = model_q(true);
    c.holds(
        "X1 node",
        key_of(&m.add_speed_bearing(99, SpeedBearing::constant(b))) == Some("node"),
        "node 99",
    );
    for (n, lo, hi, s, want) in [
        ("lo >= hi", 5.0, 5.0, 10usize, "range"),
        ("nan", f64::NAN, 5.0, 10, "range"),
        ("samples", 1.0, 5.0, 0, "samples"),
    ] {
        c.holds(
            &format!("X1 {n}"),
            key_of(&m.critical_speeds_over(lo, hi, s)) == Some(want),
            want,
        );
    }
    let cross = bearing_x(true, 1.0);
    let x = model_xy(&|m, n| m.add_speed_bearing(n, SpeedBearing::constant(cross)).unwrap());
    c.holds(
        "X1 cross-coupled table",
        key_of(&x.critical_speeds_over(100.0, 1000.0, 10)) == Some("cross_coupling"),
        "cross_coupling",
    );
    c.finish();
}
