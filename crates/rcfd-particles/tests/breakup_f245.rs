//! F-245 (ADR 0224): TAB droplet breakup in the particle library. Registered checks B (BU1 to BU4, NB1 to NB3), RF2 and
//! IN1. The limits are those of ADR 0224, fixed before this code ran. The references are this file's own closed forms (DB).

use rcfd_mesh::gen::{box_mesh, Grading};
use rcfd_particles::*;
use std::f64::consts::PI;
use std::sync::Arc;

/// A change of a spec (the refusal lists).
type Mutator<T> = Box<dyn Fn(&mut T)>;

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs()
}

/// Data B of ADR 0224.
const SIGMA: f64 = 0.02;
const RHO_L: f64 = 700.0;
const RHO_G: f64 = 1.0;
const D0: f64 = 2e-4;
const R0: f64 = 1e-4;
const N0: f64 = 100.0;

fn omega0() -> f64 {
    (8.0 * SIGMA / (RHO_L * R0 * R0 * R0)).sqrt()
}

struct Drop {
    tr: Tracker,
    u: Vec<[f64; 3]>,
    speed: f64,
}

/// One drop of data B at rest in the uniform gas of Weber number `we`, released at `1/rate` (`t_end` as given).
fn drop(we: f64, mu_l: f64, c_v: f64, rate: f64, t_end: f64) -> Drop {
    let mesh = Arc::new(box_mesh([1, 1, 1], [1.0, 1.0, 1.0], [Grading::Uniform; 3]).unwrap());
    let opts = Options {
        drag: DragLaw::None,
        scheme: Scheme::Analytic,
        interp: Interp::CellCentre,
        gravity: [0.0; 3],
        ..Options::default()
    };
    let mut tr = Tracker::new(mesh, opts, &[]).unwrap();
    tr.add_injector(InjectorSpec {
        name: "d".into(),
        kind: InjKind::Point([0.5; 3]),
        rate,
        t_start: 0.0,
        t_end,
        velocity: VelMode::Given([0.0; 3]),
        dist: SizeDist::Constant(D0),
        density: RHO_L,
        parcel_count: N0,
        seed: 3,
    })
    .unwrap();
    let mut spec = BreakupSpec::tab(SIGMA, mu_l, 7);
    spec.c_v = c_v;
    tr.set_breakup(spec).unwrap();
    let speed = (we * SIGMA / (RHO_G * R0)).sqrt();
    Drop {
        tr,
        u: vec![[speed, 0.0, 0.0]],
        speed,
    }
}

impl Drop {
    fn step(&mut self, dt: f64) {
        let flow = Flow {
            u: &self.u,
            grad: None,
            rho: RHO_G,
            mu: 1.8e-5,
        };
        self.tr.advance(&flow, dt).unwrap();
    }
}

/// BU1 (acceptance A2) with NB1 and NB2.
#[test]
fn bu1_breakup_time_and_child_size_from_rest() {
    let om = omega0();
    for we in [6.1, 12.0, 60.0, 1000.0] {
        let mut d = drop(we, 0.0, 0.0, 1e5, 1e-5);
        for _ in 0..60 {
            d.step(1e-5);
        }
        let (n_bu, age) = d.tr.breakups(0);
        let p = &d.tr.particles[0];
        let w = we / 12.0;
        let t_bu = (1.0 - 1.0 / w).acos() / om;
        let ratio = 4.0 / 3.0 + we / 6.0;
        let e_t = rel(age, t_bu);
        let e_r = rel(p.d / 2.0, R0 / ratio);
        let e_n = rel(p.parcel, N0 * ratio * ratio * ratio);
        let e_m = rel(p.parcel * p.d * p.d * p.d, N0 * D0 * D0 * D0);
        println!(
            "BU1 We {we}: U {:.8} m/s, breakups {n_bu}, age {age:.12e} against {t_bu:.12e}: {e_t:.2e}; r {:.12e} against {:.12e}: {e_r:.2e}; n {:.10e}: {e_n:.2e}; n d^3: {e_m:.2e}; v {:?}",
            d.speed,
            p.d / 2.0,
            R0 / ratio,
            p.parcel,
            p.v
        );
        assert_eq!(n_bu, 1, "BU1 We {we}: one breakup");
        assert!(e_t <= 0.02, "BU1 We {we} time: {e_t:e}");
        assert!(e_r <= 0.02, "BU1 We {we} child radius: {e_r:e}");
        assert!(e_n <= 1e-12, "BU1 We {we} count: {e_n:e}");
        assert!(e_m <= 1e-14, "BU1 We {we} liquid mass: {e_m:e}");
        assert!(p.v == [0.0; 3], "BU1 We {we} velocity {:?}", p.v);
        if we == 12.0 {
            let nb1 = rel(age, PI / om);
            println!("NB1 We 12: age against the half period {:.6e}: {nb1:.4} off", PI / om);
            assert!(nb1 >= 0.4, "NB1 must be at least 40 percent off: {nb1}");
        }
        if we == 60.0 {
            let nb2 = rel(R0 / (p.d / 2.0), 7.0 / 3.0);
            println!(
                "NB2 We 60: r0/r_child {:.6} against 7/3: {nb2:.4} off",
                R0 / (p.d / 2.0)
            );
            assert!(nb2 >= 0.5, "NB2 must be at least 50 percent off: {nb2}");
        }
    }
}

/// BU2: below the critical Weber number the drop oscillates and does not break.
#[test]
fn bu2_subcritical_drop_does_not_break() {
    let mut d = drop(5.9, 0.0, 0.0, 1e5, 1e-5);
    let mut ymax = f64::NEG_INFINITY;
    for _ in 0..500 {
        d.step(1e-5);
        ymax = ymax.max(d.tr.distortion(0).0);
    }
    let p = &d.tr.particles[0];
    let (n_bu, _) = d.tr.breakups(0);
    println!(
        "BU2 We 5.9: breakups {n_bu}, largest sampled y {ymax:.8} (2W = {:.8}), d {:e}, n {}",
        2.0 * 5.9 / 12.0,
        p.d,
        p.parcel
    );
    assert_eq!(n_bu, 0, "BU2 no breakup");
    assert_eq!(p.d.to_bits(), D0.to_bits(), "BU2 diameter");
    assert_eq!(p.parcel.to_bits(), N0.to_bits(), "BU2 count");
    assert!((0.9..=1.0).contains(&ymax), "BU2 largest y {ymax}");
}

/// The damped closed form (eq. 4 from rest) and its derivative at `t`.
fn damped(w: f64, inv_td: f64, om: f64, t: f64) -> (f64, f64) {
    let a = -w;
    let b = a * inv_td / om;
    let e = (-t * inv_td).exp();
    let y = w + e * (a * (om * t).cos() + b * (om * t).sin());
    let yd = (w - y) * inv_td + om * e * (b * (om * t).cos() - a * (om * t).sin());
    (y, yd)
}

/// BU3 (viscous, the breakup in the first sub-step) with NB3.
#[test]
fn bu3_viscous_breakup_time_and_child_size() {
    let mu_l = 0.03;
    let mut d = drop(12.0, mu_l, 0.0, 1e8, 1e-8);
    d.step(3e-4);
    let (n_bu, age) = d.tr.breakups(0);
    let p = &d.tr.particles[0];
    let w: f64 = 1.0;
    let inv_td = 5.0 * mu_l / (2.0 * RHO_L * R0 * R0);
    let om = (omega0() * omega0() - inv_td * inv_td).sqrt();
    let t_bu = (1.0 - 1.0 / w).acos() / om;
    let (y, yd) = damped(w, inv_td, om, t_bu);
    let k = 10.0 / 3.0;
    let r32 = R0 / (1.0 + 8.0 * k / 20.0 + RHO_L * R0 * R0 * R0 * yd * yd * (6.0 * k - 5.0) / (120.0 * SIGMA));
    let e_t = rel(age, t_bu);
    let e_r = rel(p.d / 2.0, r32);
    let nb3 = rel(age, (1.0 - 1.0 / w).acos() / omega0());
    println!(
        "BU3: 1/t_d {inv_td:.6}, omega {om:.6}, breakups {n_bu}, age {age:.12e} against {t_bu:.12e}: {e_t:.2e}; y(t_bu) {y:.8}, yd {yd:.8e}, r {:.12e} against {r32:.12e} (r0/r32 {:.8}): {e_r:.2e}; NB3 against the inviscid time: {nb3:.4} off",
        p.d / 2.0,
        R0 / r32
    );
    assert_eq!(n_bu, 1, "BU3 one breakup");
    assert!(e_t <= 0.02, "BU3 time: {e_t:e}");
    assert!(e_r <= 0.02, "BU3 child radius: {e_r:e}");
    assert!(nb3 >= 0.2, "NB3 must be at least 20 percent off: {nb3}");
}

/// BU3s (record, no limit): BU3 with steps of 1e-5 s, the source's per-step re-estimate under damping.
#[test]
fn bu3s_viscous_breakup_with_steps_record() {
    let mut d = drop(12.0, 0.03, 0.0, 1e5, 1e-5);
    for _ in 0..60 {
        d.step(1e-5);
    }
    let (n_bu, age) = d.tr.breakups(0);
    println!("BU3s: breakups {n_bu}, age at the last {age:.12e}");
}

/// BU3o: an overdamped drop never breaks and holds no distortion.
#[test]
fn bu3o_overdamped_drop() {
    let mut d = drop(60.0, 0.05, 0.0, 1e5, 1e-5);
    for step in 1..=100 {
        d.step(1e-5);
        let (y, yd) = d.tr.distortion(0);
        assert!(
            y.to_bits() == 0 && yd.to_bits() == 0,
            "BU3o step {step}: y {y:e} yd {yd:e}"
        );
    }
    let (n_bu, _) = d.tr.breakups(0);
    println!("BU3o: breakups {n_bu}, y and yd 0 after every step");
    assert_eq!(n_bu, 0, "BU3o no breakup");
}

/// BU4: the source's own limits (eqs. 12a, 13, 14, 16, 23 and the bag value 7/3).
#[test]
fn bu4_limits_of_the_source() {
    let om = omega0();
    // We = 1000
    let we = 1000.0;
    let mut d = drop(we, 0.0, 1.0, 1e5, 1e-5);
    for _ in 0..60 {
        d.step(1e-5);
    }
    let (n_bu, age) = d.tr.breakups(0);
    let p = &d.tr.particles[0];
    let u = d.speed;
    let w = we / 12.0;
    let yd_b = w * om * (2.0 / w - 1.0 / (w * w)).sqrt();
    let dv = p.v;
    let dvn = (dv[0] * dv[0] + dv[1] * dv[1] + dv[2] * dv[2]).sqrt();
    let e12a = rel(age, 3f64.sqrt() * (RHO_L / RHO_G).sqrt() * R0 / u);
    let e23 = rel(R0 / (p.d / 2.0), we / 6.0);
    let e14 = rel(dvn, 0.5 * R0 * yd_b);
    let ex = dv[0].abs();
    let e16 = rel(dvn / u, 3f64.sqrt() / 3.0 * (RHO_G / RHO_L).sqrt());
    println!(
        "BU4 We 1000: breakups {n_bu}, age {age:.10e}: eq. 12a {e12a:.4e}; r0/r_child {:.8}: eq. 23 {e23:.4e}; |dv| {dvn:.10} against {:.10}: {e14:.2e}; dv.x {ex:.2e}; |dv|/U {:.6e}: eq. 16 {e16:.4e}",
        R0 / (p.d / 2.0),
        0.5 * R0 * yd_b,
        dvn / u
    );
    assert_eq!(n_bu, 1, "BU4 We 1000 one breakup");
    assert!(e12a <= 0.02, "BU4 eq. 12a: {e12a:e}");
    assert!(e23 <= 0.02, "BU4 eq. 23: {e23:e}");
    assert!(e14 <= 1e-12, "BU4 eq. 14: {e14:e}");
    assert!(ex <= 1e-12 * dvn, "BU4 normal direction: {ex:e}");
    assert!(e16 <= 0.02, "BU4 eq. 16: {e16:e}");
    // We = 6.001
    let we = 6.001;
    let mut d = drop(we, 0.0, 1.0, 1e5, 1e-5);
    for _ in 0..60 {
        d.step(1e-5);
    }
    let (n_bu, age) = d.tr.breakups(0);
    let p = &d.tr.particles[0];
    let e13 = rel(age, PI * (RHO_L * R0 * R0 * R0 / (8.0 * SIGMA)).sqrt());
    let e73 = rel(R0 / (p.d / 2.0), 7.0 / 3.0);
    println!(
        "BU4 We 6.001: breakups {n_bu}, age {age:.10e}: eq. 13 {e13:.4e}; r0/r_child {:.8}: 7/3 {e73:.4e}",
        R0 / (p.d / 2.0)
    );
    assert_eq!(n_bu, 1, "BU4 We 6.001 one breakup");
    assert!(e13 <= 0.02, "BU4 eq. 13: {e13:e}");
    assert!(e73 <= 0.02, "BU4 7/3: {e73:e}");
}

/// RF2: the refusals of breakup and the control.
#[test]
fn rf2_breakup_refusals() {
    let mesh = Arc::new(box_mesh([1, 1, 1], [1.0, 1.0, 1.0], [Grading::Uniform; 3]).unwrap());
    let fresh = || Tracker::new(mesh.clone(), Options::default(), &[]).unwrap();
    let ok = || BreakupSpec::tab(0.02, 1e-3, 7);
    let mut results: Vec<(String, bool)> = Vec::new();
    let mut push = |what: &str, r: std::result::Result<(), ParticleError>| {
        println!("RF2 {what}: {r:?}");
        results.push((what.to_string(), r.is_err()));
    };
    let bad: Vec<(&str, Mutator<BreakupSpec>)> = vec![
        ("sigma = 0", Box::new(|s| s.surface_tension = 0.0)),
        ("sigma = NaN", Box::new(|s| s.surface_tension = f64::NAN)),
        ("mu_l = -1e-3", Box::new(|s| s.liquid_viscosity = -1e-3)),
        ("C_F = 0", Box::new(|s| s.c_f = 0.0)),
        ("C_k = 0", Box::new(|s| s.c_k = 0.0)),
        ("C_b = 0", Box::new(|s| s.c_b = 0.0)),
        ("C_d = -1", Box::new(|s| s.c_d = -1.0)),
        ("C_v = -1", Box::new(|s| s.c_v = -1.0)),
        ("K = 0.5", Box::new(|s| s.k = 0.5)),
    ];
    for (what, f) in bad {
        let mut s = ok();
        f(&mut s);
        push(what, fresh().set_breakup(s));
    }
    let mut t = fresh();
    t.set_breakup(ok()).unwrap();
    push("second set_breakup", t.set_breakup(ok()));
    let rx = ReactionSpec {
        solid_molar_mass: 0.012,
        solid_coefficient: 1.0,
        gas: vec![
            GasSpecies {
                name: "O".into(),
                molar_mass: 0.032,
                coefficient: -1.0,
            },
            GasSpecies {
                name: "P".into(),
                molar_mass: 0.044,
                coefficient: 1.0,
            },
        ],
        oxidiser: 0,
        rate: 0.01,
        reacting_fraction: 0.875,
    };
    push("set_reaction with breakup set", t.set_reaction(rx.clone(), vec![0.2]));
    let mut t2 = fresh();
    t2.set_reaction(rx, vec![0.2]).unwrap();
    push("set_breakup with a reaction set", t2.set_breakup(ok()));
    let mut t3 = fresh();
    t3.set_breakup(ok()).unwrap();
    t3.enable_coupling();
    let u = vec![[0.0; 3]];
    let grad = vec![[[0.0; 3]; 3]];
    let flow = Flow {
        u: &u,
        grad: Some(&grad),
        rho: 1.2,
        mu: 1.8e-5,
    };
    push("advance with breakup and the exchange record", t3.advance(&flow, 0.1));
    let failed: Vec<&String> = results.iter().filter(|r| !r.1).map(|r| &r.0).collect();
    assert!(failed.is_empty(), "RF2 accepted: {failed:?}");
    assert_eq!(results.len(), 13, "RF2 registered refusals");
    let control = fresh().set_breakup(ok());
    println!("RF2 control: {control:?}");
    assert!(control.is_ok(), "RF2 control");
}

/// The RX2 scenario of ADR 0208 (`reaction_f255.rs`) without the reaction, with breakup when `breakup` is given.
fn rx2_scenario(breakup: Option<BreakupSpec>) -> Tracker {
    let mesh = Arc::new(box_mesh([6, 5, 4], [1.2, 1.0, 0.8], [Grading::Uniform; 3]).unwrap());
    let opts = Options {
        drag: DragLaw::SchillerNaumann,
        scheme: Scheme::Rk4,
        interp: Interp::Gradient,
        gravity: [0.0, 0.0, -9.81],
        default_wall: WallAction::Reflect(0.8),
        ..Options::default()
    };
    let overrides = [
        ("zmin".to_string(), WallAction::Stick),
        ("zmax".to_string(), WallAction::Escape),
    ];
    let mut tr = Tracker::new(mesh.clone(), opts, &overrides).unwrap();
    tr.add_injector(InjectorSpec {
        name: "a".into(),
        kind: InjKind::Point([0.3, 0.4, 0.5]),
        rate: 400.0,
        t_start: 0.0,
        t_end: 0.4,
        velocity: VelMode::Given([0.2, 0.0, 0.1]),
        dist: SizeDist::Constant(1e-4),
        density: 1500.0,
        parcel_count: 10.0,
        seed: 11,
    })
    .unwrap();
    tr.add_injector(InjectorSpec {
        name: "b".into(),
        kind: InjKind::Surface("zmax".into()),
        rate: 150.0,
        t_start: 0.1,
        t_end: 0.5,
        velocity: VelMode::Fluid,
        dist: SizeDist::Constant(6e-5),
        density: 1400.0,
        parcel_count: 1.0,
        seed: 12,
    })
    .unwrap();
    tr.set_injector_ext(
        "a",
        InjectorExt {
            cone: Some(ConeSpec {
                axis: [1.0, 0.2, 0.3],
                half_angle: 0.4,
                inner_half_angle: 0.1,
                speed: 0.5,
            }),
            mass_flow: Some(2e-6),
            temperature: None,
        },
    )
    .unwrap();
    let n = mesh.n_cells();
    tr.set_dispersion(Dispersion::default(), vec![0.02; n], vec![0.05; n], 99)
        .unwrap();
    if let Some(b) = breakup {
        tr.set_breakup(b).unwrap();
    }
    let u: Vec<[f64; 3]> = mesh
        .cell_centres()
        .iter()
        .map(|c| [0.3 + 0.2 * c[1], -0.1 * c[0], 0.05 * c[2]])
        .collect();
    let grad = vec![[[0.0, 0.2, 0.0], [-0.1, 0.0, 0.0], [0.0, 0.0, 0.05]]; n];
    let flow = Flow {
        u: &u,
        grad: Some(&grad),
        rho: 1.2,
        mu: 1.8e-5,
    };
    for _ in 0..300 {
        tr.advance(&flow, 0.01).unwrap();
    }
    tr
}

/// IN1: breakup set but every Weber number far below critical: the parcels are those of the run without breakup.
#[test]
fn in1_inert_breakup() {
    let without = rx2_scenario(None).fingerprint();
    let tr = rx2_scenario(Some(BreakupSpec::tab(1e6, 1e-3, 5)));
    let with = tr.fingerprint();
    println!(
        "IN1 fingerprint words: without {}, with {}; breakup events {}",
        without.len(),
        with.len(),
        tr.breakup().unwrap().events()
    );
    assert!(with.len() > without.len(), "IN1 the breakup part is appended");
    assert_eq!(&with[..without.len()], &without[..], "IN1 parcels unchanged");
}
