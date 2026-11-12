//! F-331 (ADR 0237): the sponge layer of the compressible solver. A0, P0 normal incidence with an isolated pulse in a tube;
//! A30, A45, A60 oblique pulse trains in a duct (heavy); X1 one isolated oblique pulse in a half space (heavy); U the
//! uniform states; S2 the refusals of the solver; B2 bit identity without a sponge; NC the negative controls. The measure
//! is the largest difference to a reference run on an extended grid against the incident peak. Limits are the registered
//! ones of ADR 0237.

use rcfd_fv::compressible::{to_prim, CompBc, CompSettings, CompressibleSolver, Gas, Prim};
use rcfd_fv::sponge::{rate_max_for_reflection, Sponge, SpongeForm, SpongeLayer};
use rcfd_fv::FvGeometry;
use rcfd_mesh::gen::box_2d;
use std::sync::Arc;

const AIR: Gas = Gas { gamma: 1.4 };
const GAMMA: f64 = 1.4;
/// Ambient pressure: with `rho0 = 1` the sound speed is 1.
const P0: f64 = 1.0 / 1.4;
/// Perturbation amplitude of the waves.
const EPS: f64 = 1e-4 * P0;
/// Width of the Gaussian pulses.
const W: f64 = 0.04;
/// Cell size of the reflection checks.
const DX: f64 = 0.005;
/// The registered layer (ADR 0237 D7): thickness, exponent, two-way attenuation at normal incidence.
const LAYER: f64 = 0.2;
const EXPONENT: f64 = 2.0;
const R0: f64 = 1e-6;
/// The acceptance limit of F-331.
const LIMIT: f64 = 0.01;

fn ambient() -> Prim {
    Prim {
        rho: 1.0,
        u: [0.0; 3],
        p: P0,
    }
}

/// Superposition of exact isentropic simple waves: each part is a pressure perturbation travelling along a unit vector.
fn superpose(parts: &[(f64, [f64; 3])]) -> Prim {
    let mut u = [0.0; 3];
    let mut dp_sum = 0.0;
    for &(dp, d) in parts {
        let a = ((P0 + dp) / P0).powf((GAMMA - 1.0) / (2.0 * GAMMA));
        let v = 2.0 * (a - 1.0) / (GAMMA - 1.0);
        for k in 0..3 {
            u[k] += v * d[k];
        }
        dp_sum += dp;
    }
    Prim {
        rho: ((P0 + dp_sum) / P0).powf(1.0 / GAMMA),
        u,
        p: P0 + dp_sum,
    }
}

fn gauss(xi: f64) -> f64 {
    (-(xi / W).powi(2)).exp()
}

fn solver(nx: usize, ny: usize, lx: f64, ly: f64, init: impl Fn([f64; 3]) -> Prim) -> CompressibleSolver {
    let mesh = Arc::new(box_2d(nx, ny, lx, ly, false, false).unwrap());
    let geom = FvGeometry::new(mesh).unwrap();
    CompressibleSolver::new(geom, AIR, CompSettings::default(), init).unwrap()
}

fn pressure(s: &CompressibleSolver, cell: usize) -> f64 {
    to_prim(s.gas, &s.u[cell]).p
}

/// The registered layer with its front at `x = x_front`, normal `+x`.
fn registered_layer(x_front: f64) -> SpongeLayer {
    SpongeLayer {
        origin: [x_front, 0.0, 0.0],
        normal: [1.0, 0.0, 0.0],
        thickness: LAYER,
        rate_max: rate_max_for_reflection(LAYER, 1.0, R0, EXPONENT).unwrap(),
        exponent: EXPONENT,
    }
}

/// The sponge of a run: none, plain or matched.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Form {
    None,
    Plain,
    Matched,
}

fn apply(s: &mut CompressibleSolver, form: Form, x_front: f64) {
    let sponge = match form {
        Form::None => None,
        Form::Plain => Some(Sponge::planar(&s.geom, ambient(), &registered_layer(x_front), false).unwrap()),
        Form::Matched => Some(Sponge::planar(&s.geom, ambient(), &registered_layer(x_front), true).unwrap()),
    };
    s.set_sponge(sponge).unwrap();
}

/// The cells of the column with the centre `x`, sorted by `y`.
fn column(s: &CompressibleSolver, x: f64) -> Vec<usize> {
    let cc = s.geom.mesh.cell_centres();
    let mut v: Vec<usize> = (0..cc.len()).filter(|&c| (cc[c][0] - x).abs() < 0.2 * DX).collect();
    v.sort_by(|&a, &b| cc[a][1].total_cmp(&cc[b][1]));
    assert!(!v.is_empty(), "no cell at x = {x}");
    v
}

/// The measure of ADR 0237: the runs `tests` and the reference run advance in lockstep with the smallest `max_dt()` until
/// `t >= t_end`; over the steps with `t_start <= t <= t_window_end` and the probe cells (`probes[i]` of run `i`, the last
/// entry of the reference; the same number of cells, in the same order) the result is, per test run,
/// `max |p_test - p_ref| / max (p_ref - p0)`.
fn reflection(
    tests: &mut [CompressibleSolver],
    reference: &mut CompressibleSolver,
    probes: &[Vec<usize>],
    t_start: f64,
    t_window_end: f64,
    t_end: f64,
) -> Vec<f64> {
    assert_eq!(probes.len(), tests.len() + 1);
    let pr = probes.last().unwrap();
    assert!(probes.iter().all(|p| p.len() == pr.len()));
    let mut num = vec![0.0f64; tests.len()];
    let mut den = 0.0f64;
    while reference.time < t_end {
        let dt = tests.iter().map(|s| s.max_dt()).fold(reference.max_dt(), f64::min);
        reference.step(dt).unwrap();
        for s in tests.iter_mut() {
            s.step(dt).unwrap();
        }
        let t = reference.time;
        if t < t_start || t > t_window_end {
            continue;
        }
        let p_ref: Vec<f64> = pr.iter().map(|&c| pressure(reference, c)).collect();
        den = p_ref.iter().fold(den, |m, p| m.max(p - P0));
        for (i, s) in tests.iter().enumerate() {
            for (k, &c) in probes[i].iter().enumerate() {
                num[i] = num[i].max((pressure(s, c) - p_ref[k]).abs());
            }
        }
    }
    println!("  incident peak at the probes / eps = {:.5}", den / EPS);
    num.iter().map(|n| n / den).collect()
}

/// Set-up of A0, P0 and NC0 (ADR 0237): an isolated pulse in a tube; the runs are (form, boundary behind the layer).
fn tube(runs: &[(Form, CompBc)]) -> Vec<f64> {
    let init = |x: [f64; 3]| superpose(&[(EPS * gauss(x[0] - 0.5), [1.0, 0.0, 0.0])]);
    let mut tests: Vec<CompressibleSolver> = runs
        .iter()
        .map(|&(form, bc)| {
            let mut s = solver(240, 1, 1.2, DX, init);
            s.set_bc("xmin", CompBc::Fixed(ambient())).unwrap();
            s.set_bc("xmax", bc).unwrap();
            apply(&mut s, form, 1.0);
            s
        })
        .collect();
    let mut reference = solver(600, 1, 3.0, DX, init);
    reference.set_bc("xmin", CompBc::Fixed(ambient())).unwrap();
    reference
        .set_bc("xmax", CompBc::NonReflecting { p: P0, sigma: 0.0 })
        .unwrap();
    let mut probes: Vec<Vec<usize>> = tests.iter().map(|s| column(s, 0.8975)).collect();
    probes.push(column(&reference, 0.8975));
    reflection(&mut tests, &mut reference, &probes, 0.0, f64::INFINITY, 1.6)
}

const THOMPSON: CompBc = CompBc::NonReflecting { p: P0, sigma: 0.0 };

/// A0, A0s, P0 and NC0 of ADR 0237: normal incidence. The layer absorbs the pulse in front of the Thompson boundary and
/// in front of a rigid wall, in both forms; the rigid wall alone reflects it.
#[test]
fn a0_p0_normal_incidence_reflects_less_than_one_percent() {
    let r = tube(&[
        (Form::Matched, THOMPSON),
        (Form::Matched, CompBc::Slip),
        (Form::Plain, CompBc::Slip),
        (Form::None, CompBc::Slip),
    ]);
    println!(
        "A0 matched, Thompson: R = {:.3e}; A0s matched, Slip: R = {:.3e}; P0 plain, Slip: R = {:.3e} (limit {LIMIT:e}); NC0 no sponge, Slip: R = {:.5} (>= 0.9)",
        r[0], r[1], r[2], r[3]
    );
    assert!(r[0] <= LIMIT, "A0 {}", r[0]);
    assert!(r[1] <= LIMIT, "A0s {}", r[1]);
    assert!(r[2] <= LIMIT, "P0 {}", r[2]);
    assert!(r[3] >= 0.9, "NC0 {}", r[3]);
}

/// The periodic, zero-mean train of set-up D (ADR 0237) in a duct of height `h`: two families of plane Gaussian pulses
/// along `(cos, +sin)` and `(cos, -sin)`, period `2 h sin` along each direction.
fn train(x: [f64; 3], theta: f64, h: f64) -> Prim {
    let (c, s) = (theta.cos(), theta.sin());
    let lam = 2.0 * h * s;
    let mean = W * std::f64::consts::PI.sqrt() / lam;
    let g = |xi: f64| {
        let r = xi - lam * (xi / lam).round();
        (-1..=1).map(|m| gauss(r - m as f64 * lam)).sum::<f64>() - mean
    };
    superpose(&[
        (EPS * g(x[0] * c + x[1] * s), [c, s, 0.0]),
        (EPS * g(x[0] * c - x[1] * s), [c, -s, 0.0]),
    ])
}

/// One row of the table of set-up D.
struct Duct {
    theta_deg: f64,
    h: f64,
    ny: usize,
    t_s: f64,
    t_end: f64,
    x_i: f64,
    nx: usize,
    nx_ref: usize,
}

const D30: Duct = Duct {
    theta_deg: 30.0,
    h: 0.64,
    ny: 128,
    t_s: 0.84,
    t_end: 1.53,
    x_i: 1.75,
    nx: 390,
    nx_ref: 660,
};
const D45: Duct = Duct {
    theta_deg: 45.0,
    h: 0.455,
    ny: 91,
    t_s: 1.05,
    t_end: 1.75,
    x_i: 1.95,
    nx: 430,
    nx_ref: 740,
};
const D60: Duct = Duct {
    theta_deg: 60.0,
    h: 0.37,
    ny: 74,
    t_s: 1.58,
    t_end: 2.28,
    x_i: 2.5,
    nx: 540,
    nx_ref: 960,
};

/// Set-up D of ADR 0237 for the given sponge forms (Thompson boundary behind): the measure per form.
fn duct(d: &Duct, forms: &[Form]) -> Vec<f64> {
    let th = d.theta_deg.to_radians();
    let make = |nx: usize| {
        let mut s = solver(nx, d.ny, nx as f64 * DX, d.h, |x| train(x, th, d.h));
        s.set_bc("ymin", CompBc::Slip).unwrap();
        s.set_bc("ymax", CompBc::Slip).unwrap();
        s.set_bc("xmin", CompBc::Fixed(ambient())).unwrap();
        s.set_bc("xmax", THOMPSON).unwrap();
        s
    };
    assert!((d.nx as f64 * DX - (d.x_i + LAYER)).abs() < 1e-9);
    let mut tests: Vec<CompressibleSolver> = forms
        .iter()
        .map(|&form| {
            let mut s = make(d.nx);
            apply(&mut s, form, d.x_i);
            s
        })
        .collect();
    let mut reference = make(d.nx_ref);
    let x_probe = d.x_i - 0.1025;
    let mut probes: Vec<Vec<usize>> = tests.iter().map(|s| column(s, x_probe)).collect();
    probes.push(column(&reference, x_probe));
    assert_eq!(probes[0].len(), d.ny);
    reflection(&mut tests, &mut reference, &probes, d.t_s, d.t_end, d.t_end)
}

/// A30 and NC30 of ADR 0237.
#[test]
#[ignore = "heavy: three duct runs of 50 000 to 85 000 cells; run in the release profile"]
fn a30_oblique_train_30_degrees() {
    let r = duct(&D30, &[Form::Matched, Form::None]);
    println!(
        "A30 matched: R = {:.3e} (limit {LIMIT:e}); NC30 no sponge: R = {:.5} ([0.04, 0.10])",
        r[0], r[1]
    );
    assert!(r[0] <= LIMIT, "A30 {}", r[0]);
    assert!((0.04..=0.10).contains(&r[1]), "NC30 {}", r[1]);
}

/// A45, NC45 and NCP45 of ADR 0237.
#[test]
#[ignore = "heavy: four duct runs of 39 000 to 67 000 cells; run in the release profile"]
fn a45_oblique_train_45_degrees() {
    let r = duct(&D45, &[Form::Matched, Form::None, Form::Plain]);
    println!(
        "A45 matched: R = {:.3e} (limit {LIMIT:e}); NC45 no sponge: R = {:.5} ([0.12, 0.21]); NCP45 plain: R = {:.5} (>= 0.05)",
        r[0], r[1], r[2]
    );
    assert!(r[0] <= LIMIT, "A45 {}", r[0]);
    assert!((0.12..=0.21).contains(&r[1]), "NC45 {}", r[1]);
    assert!(r[2] >= 0.05, "NCP45 {}", r[2]);
}

/// A60 and NC60 of ADR 0237.
#[test]
#[ignore = "heavy: three duct runs of 40 000 to 71 000 cells; run in the release profile"]
fn a60_oblique_train_60_degrees() {
    let r = duct(&D60, &[Form::Matched, Form::None]);
    println!(
        "A60 matched: R = {:.3e} (limit {LIMIT:e}); NC60 no sponge: R = {:.5} ([0.25, 0.40])",
        r[0], r[1]
    );
    assert!(r[0] <= LIMIT, "A60 {}", r[0]);
    assert!((0.25..=0.40).contains(&r[1]), "NC60 {}", r[1]);
}

/// X1 of ADR 0237: one isolated plane pulse at 30 degrees and its mirror image in a half space above a rigid wall.
#[test]
#[ignore = "heavy: two runs of 151 000 and 259 000 cells; run in the release profile"]
fn x1_isolated_oblique_pulse_30_degrees() {
    let th = 30f64.to_radians();
    let (c, s) = (th.cos(), th.sin());
    let init = move |x: [f64; 3]| {
        superpose(&[
            (EPS * gauss(x[0] * c + x[1] * s - 0.545), [c, s, 0.0]),
            (EPS * gauss(x[0] * c - x[1] * s - 0.545), [c, -s, 0.0]),
        ])
    };
    let make = |nx: usize| {
        let mut sv = solver(nx, 360, nx as f64 * DX, 1.8, init);
        sv.set_bc("ymin", CompBc::Slip).unwrap();
        sv.set_bc("ymax", CompBc::Extrapolate).unwrap();
        sv.set_bc("xmin", CompBc::Extrapolate).unwrap();
        sv.set_bc("xmax", THOMPSON).unwrap();
        sv
    };
    let mut tests = vec![make(420)];
    apply(&mut tests[0], Form::Matched, 1.9);
    let mut reference = make(720);
    let probes = vec![vec![column(&tests[0], 1.7975)[0]], vec![column(&reference, 1.7975)[0]]];
    let r = reflection(&mut tests, &mut reference, &probes, 0.0, f64::INFINITY, 1.75);
    println!(
        "X1 matched, isolated pulse at 30 degrees: R = {:.3e} (limit {LIMIT:e})",
        r[0]
    );
    assert!(r[0] <= LIMIT, "X1 {}", r[0]);
}

/// U1p and U1m of ADR 0237: a uniform state equal to the reference stays in the layer.
#[test]
fn u1_uniform_reference_state_does_not_drift() {
    for matched in [false, true] {
        let reference = Prim {
            rho: 1.2,
            u: if matched { [0.0; 3] } else { [0.3, 0.1, 0.0] },
            p: 0.9,
        };
        let mut s = solver(40, 20, 1.0, 0.5, |_| reference);
        for patch in ["xmin", "xmax", "ymin", "ymax"] {
            let bc = if matched {
                CompBc::Slip
            } else {
                CompBc::Fixed(reference)
            };
            s.set_bc(patch, bc).unwrap();
        }
        let layer = SpongeLayer {
            origin: [0.5, 0.0, 0.0],
            normal: [1.0, 0.0, 0.0],
            thickness: 0.5,
            rate_max: 50.0,
            exponent: 2.0,
        };
        s.set_sponge(Some(Sponge::planar(&s.geom, reference, &layer, matched).unwrap()))
            .unwrap();
        assert_eq!(s.sponge().unwrap().rate.iter().filter(|r| **r > 0.0).count(), 400);
        for _ in 0..200 {
            let dt = s.max_dt();
            s.step(dt).unwrap();
        }
        let uref = reference.to_cons(AIR);
        let a = reference.sound_speed(AIR);
        let scale = [
            reference.rho,
            reference.rho * a,
            reference.rho * a,
            reference.rho * a,
            uref[4],
        ];
        let drift =
            s.u.iter()
                .flat_map(|c| (0..5).map(move |k| (c[k] - uref[k]).abs() / scale[k]))
                .fold(0.0, f64::max);
        let q = s.sponge_memory().iter().fold(0.0f64, |m, q| m.max(q.abs())) / (reference.p * s.time);
        println!(
            "U1{} after 200 steps (t = {:.4}): drift {drift:.2e}, memory {q:.2e} (limit 1e-12 each)",
            if matched { "m" } else { "p" },
            s.time
        );
        assert!(drift <= 1e-12, "{drift}");
        assert!(q <= 1e-12, "{q}");
    }
}

/// U2p and U2m of ADR 0237: a uniform state away from the reference decays as `exp(-rate t)` in mass and energy; the
/// memory of the matched form is the time integral of `p - p_ref`.
#[test]
fn u2_uniform_offset_decays_exponentially() {
    for matched in [false, true] {
        let rate = 2.0;
        let start = Prim {
            rho: 1.001,
            u: [0.0; 3],
            p: 1.002 * P0,
        };
        let mut s = solver(20, 20, 1.0, 1.0, |_| start);
        let n = s.u.len();
        s.set_sponge(Some(Sponge {
            reference: ambient(),
            rate: vec![rate; n],
            form: if matched {
                SpongeForm::Matched {
                    normal: [1.0, 0.0, 0.0],
                }
            } else {
                SpongeForm::Plain
            },
        }))
        .unwrap();
        let uref = ambient().to_cons(AIR);
        let u_start = start.to_cons(AIR);
        let (mut trapezoid, mut f_old) = (0.0, pressure(&s, 0) - P0);
        while s.time < 1.0 {
            let dt = s.max_dt();
            s.step(dt).unwrap();
            let f = pressure(&s, 0) - P0;
            trapezoid += 0.5 * (f_old + f) * dt;
            f_old = f;
        }
        let want = (-rate * s.time).exp();
        let mut decay = 0.0f64;
        let mut momentum = 0.0f64;
        for c in &s.u {
            for k in [0, 4] {
                decay = decay.max(((c[k] - uref[k]) / (u_start[k] - uref[k]) / want - 1.0).abs());
            }
            momentum = momentum.max(c[1].abs()).max(c[2].abs()).max(c[3].abs());
        }
        let name = if matched { "U2m" } else { "U2p" };
        println!(
            "{name} t = {:.4}, {} steps: decay against exp(-2 t) {decay:.2e} (limit 1e-5), momentum {momentum:.2e} (1e-12)",
            s.time, s.steps
        );
        assert!(decay <= 1e-5, "{decay}");
        assert!(momentum <= 1e-12, "{momentum}");
        if matched {
            let q = s.sponge_memory();
            let worst = q.iter().map(|q| (q / trapezoid - 1.0).abs()).fold(0.0, f64::max);
            println!(
                "U2m memory against the trapezoid sum of p - p_ref: {worst:.2e} (limit 1e-4), Q = {:.6e}",
                q[0]
            );
            assert!(worst <= 1e-4, "{worst}");
        } else {
            assert!(s.sponge_memory().iter().all(|q| *q == 0.0));
        }
    }
}

/// S2 of ADR 0237 (the solver's part): an invalid sponge is refused and leaves the solver unchanged; a sponge blocks the
/// moving-mesh step and the mesh replacement; clearing it is accepted.
#[test]
fn s2_solver_refusals() {
    let mut s = solver(8, 2, 1.0, 0.25, |_| ambient());
    let good = Sponge::planar(&s.geom, ambient(), &registered_layer(0.5), true).unwrap();
    let mut short = good.clone();
    short.rate.pop();
    let mut negative = good.clone();
    negative.rate[5] = -1.0;
    let mut nan = good.clone();
    nan.rate[5] = f64::NAN;
    let mut vacuum = good.clone();
    vacuum.reference.p = 0.0;
    let mut moving = good.clone();
    moving.reference.u = [0.1, 0.0, 0.0];
    let mut skew = good.clone();
    skew.form = SpongeForm::Matched { normal: [0.0; 3] };
    for bad in [short, negative, nan, vacuum, moving, skew] {
        assert!(s.set_sponge(Some(bad)).is_err());
        assert!(s.sponge().is_none() && s.sponge_memory().is_empty());
    }
    s.set_sponge(Some(good.clone())).unwrap();
    assert_eq!(s.sponge(), Some(&good));
    assert_eq!(s.sponge_memory().len(), s.u.len());
    assert!(s.step_ale(s.geom.clone(), 1e-3).is_err(), "step_ale with a sponge");
    assert!(
        s.replace_mesh(s.geom.clone(), s.u.clone()).is_err(),
        "replace_mesh with a sponge"
    );
    assert_eq!(s.steps, 0);
    s.set_sponge(None).unwrap();
    assert!(s.sponge().is_none() && s.sponge_memory().is_empty());
    s.step_ale(s.geom.clone(), 1e-3).unwrap();
    s.replace_mesh(s.geom.clone(), s.u.clone()).unwrap();
}

/// B2 of ADR 0237: without a sponge, after a sponge was set and cleared, and with a sponge whose rates are all zero the
/// solver produces the same bits; with the registered layer it does not.
#[test]
fn b2_without_a_sponge_the_state_is_bit_identical() {
    let th = 45f64.to_radians();
    let run = |prepare: &dyn Fn(&mut CompressibleSolver)| -> Vec<[u64; 5]> {
        let mut s = solver(60, 20, 0.3, 0.1, |x| train(x, th, 0.1));
        s.set_bc("xmin", CompBc::Fixed(ambient())).unwrap();
        s.set_bc("xmax", THOMPSON).unwrap();
        prepare(&mut s);
        for _ in 0..30 {
            let dt = s.max_dt();
            s.step(dt).unwrap();
        }
        s.u.iter().map(|c| c.map(f64::to_bits)).collect()
    };
    let zero = |s: &CompressibleSolver, form: SpongeForm| Sponge {
        reference: ambient(),
        rate: vec![0.0; s.u.len()],
        form,
    };
    let never = run(&|_| {});
    let cleared = run(&|s| {
        apply(s, Form::Matched, 0.1);
        s.set_sponge(None).unwrap();
    });
    let zero_plain = run(&|s| {
        let sp = zero(s, SpongeForm::Plain);
        s.set_sponge(Some(sp)).unwrap();
    });
    let zero_matched = run(&|s| {
        let sp = zero(
            s,
            SpongeForm::Matched {
                normal: [1.0, 0.0, 0.0],
            },
        );
        s.set_sponge(Some(sp)).unwrap();
    });
    let layered = run(&|s| apply(s, Form::Matched, 0.1));
    assert!(never == cleared, "set and cleared");
    assert!(never == zero_plain, "plain sponge with zero rates");
    assert!(never == zero_matched, "matched sponge with zero rates");
    let changed = never.iter().zip(&layered).filter(|(a, b)| a != b).count();
    println!(
        "B2 {} cells: no sponge, cleared sponge and zero-rate sponges are bit-identical; the registered layer changes {changed} cells",
        never.len()
    );
    assert!(changed > 0, "the control run must differ");
}
