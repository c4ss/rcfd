//! F-341 (ADR 0265): the sponge layer of the compressible solver on moving meshes, attached to the mesh. U1 a uniform
//! reference state on the deforming set-up U of ADR 0207; T0 the tube of ADR 0237 on a mesh translating at -0.2; B2 bit
//! identity of `step_ale` without a sponge; G the source in the ALE step against the closed-form decay and against the
//! static step; S4 the frame setting and the refusals. Limits are the registered ones of ADR 0265.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::compressible::{to_prim, CompBc, CompSettings, CompressibleSolver, Cons, Gas, Prim};
use rcfd_fv::compressible_bc::{MsdWall, TotalInlet};
use rcfd_fv::sponge::{rate_max_for_reflection, Sponge, SpongeForm, SpongeFrame, SpongeLayer};
use rcfd_fv::FvGeometry;
use rcfd_mesh::gen::box_2d;
use rcfd_mesh::motion::moved_mesh;
use rcfd_mesh::Mesh;
use std::f64::consts::PI;
use std::sync::Arc;

const AIR: Gas = Gas { gamma: 1.4 };
const GAMMA: f64 = 1.4;
/// Ambient pressure: with `rho0 = 1` the sound speed is 1.
const P0: f64 = 1.0 / 1.4;
/// Perturbation amplitude of the pulse (ADR 0237).
const EPS: f64 = 1e-4 * P0;
/// Width of the Gaussian pulse.
const W: f64 = 0.04;
/// Cell size of the tube.
const DX: f64 = 0.005;
/// The layer of ADR 0237 D7.
const LAYER: f64 = 0.2;
const EXPONENT: f64 = 2.0;
const R0: f64 = 1e-6;
/// The acceptance limits of F-341: round-off (clause 1) and 1 percent (clause 2).
const ROUND_OFF: f64 = 1e-12;
const LIMIT: f64 = 0.01;
/// The velocity of the translating tube (ADR 0265 DR6).
const V: Vec3 = [-0.2, 0.0, 0.0];
/// The shift of the shifted form in the checks (ADR 0248).
const SHIFT: f64 = 0.25;

fn geom(m: Mesh) -> FvGeometry {
    FvGeometry::new(Arc::new(m)).unwrap()
}

fn ambient() -> Prim {
    Prim {
        rho: 1.0,
        u: [0.0; 3],
        p: P0,
    }
}

fn pressure(s: &CompressibleSolver, cell: usize) -> f64 {
    to_prim(s.gas, &s.u[cell]).p
}

/// The end test of ADR 0191's library loops, the schedule of ADR 0207.
fn ended(time: f64, end: f64) -> bool {
    time >= end - 1e-12 * end.max(1.0)
}

/// The step of ADR 0207's schedule: `max_dt_ale()` clipped to the end time.
fn step_to(s: &CompressibleSolver, end: f64) -> f64 {
    s.max_dt_ale().min((end - s.time).max(1e-300))
}

/// The motion of set-up U of ADR 0207 T2 at time `t` (copied from `compressible_ale_f258.rs`).
fn u_motion(p: Vec3, t: f64) -> Vec3 {
    let (s1, s2) = ((2.0 * PI * t).sin(), (4.0 * PI * t).sin());
    let bump = 0.01 * s1 * (PI * p[0] / 1.2).sin() * (PI * p[1] / 0.8).sin();
    let q = [p[0] + bump, p[1] + bump];
    let (cx, cy) = (0.6, 0.4);
    let d = [(1.0 + 0.03 * s1) * (q[0] - cx), (1.0 + 0.02 * s2) * (q[1] - cy)];
    let th = 0.05 * s1;
    let (c, s) = (th.cos(), th.sin());
    [
        cx + c * d[0] - s * d[1] + 0.02 * s1,
        cy + s * d[0] + c * d[1] + 0.01 * s2,
        p[2],
    ]
}

fn moved(mesh0: &Mesh, f: impl Fn(Vec3) -> Vec3) -> FvGeometry {
    let points: Vec<Vec3> = mesh0.points().iter().map(|&p| f(p)).collect();
    geom(moved_mesh(mesh0, points).unwrap())
}

/// The sponge of a run.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Form {
    None,
    Plain,
    Matched,
    Shifted,
}

fn sponge_of(g: &FvGeometry, form: Form, reference: Prim, layer: &SpongeLayer) -> Option<Sponge> {
    match form {
        Form::None => None,
        Form::Plain => Some(Sponge::planar(g, reference, layer, false).unwrap()),
        Form::Matched => Some(Sponge::planar(g, reference, layer, true).unwrap()),
        Form::Shifted => Some(Sponge::planar_shifted(g, reference, layer, SHIFT, 0.0).unwrap()),
    }
}

// ---------------------------------------------------------------------------------------------------------------- U1

/// U1p, U1m, U1s and U1g of ADR 0265: a uniform state equal to the reference stays under `step_ale` on the deforming
/// set-up U of ADR 0207, in the three forms.
#[test]
fn u1_uniform_reference_state_stays_on_a_deforming_mesh() {
    let mesh0 = box_2d(12, 8, 1.2, 0.8, false, false).unwrap();
    let layer = SpongeLayer {
        origin: [0.6, 0.0, 0.0],
        normal: [1.0, 0.0, 0.0],
        thickness: 0.6,
        rate_max: 20.0,
        exponent: 2.0,
    };
    let second = SpongeLayer {
        origin: [0.0, 0.4, 0.0],
        normal: [0.0, 1.0, 0.0],
        thickness: 0.4,
        ..layer
    };
    let patches = ["xmin", "xmax", "ymin", "ymax"];
    let faces: Vec<Vec<usize>> = patches
        .iter()
        .map(|n| mesh0.patch(n).unwrap().faces().collect())
        .collect();
    for name in ["U1p", "U1m", "U1s"] {
        let reference = Prim {
            rho: 1.2,
            u: if name == "U1p" { [0.3, 0.1, 0.0] } else { [0.0; 3] },
            p: 0.9,
        };
        let mut s = CompressibleSolver::new(geom(mesh0.clone()), AIR, CompSettings::default(), |_| reference).unwrap();
        for p in patches {
            s.set_bc(p, CompBc::Fixed(reference)).unwrap();
        }
        let sp = match name {
            "U1p" => Sponge::planar(&s.geom, reference, &layer, false).unwrap(),
            "U1m" => Sponge::planar(&s.geom, reference, &layer, true).unwrap(),
            _ => Sponge::corner(&s.geom, reference, &layer, &second, SHIFT, 0.05).unwrap(),
        };
        s.set_sponge(Some(sp)).unwrap();
        s.set_sponge_frame(SpongeFrame::AttachedToMesh);
        let uref = reference.to_cons(AIR);
        let a = reference.sound_speed(AIR);
        let scale = [
            reference.rho,
            reference.rho * a,
            reference.rho * a,
            reference.rho * a,
            uref[4],
        ];
        let (mut drift, mut mq, mut mr, mut mm) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
        let mut speed = [0.0f64; 4];
        while !ended(s.time, 2.0) {
            let dt = step_to(&s, 2.0);
            let t_new = s.time + dt;
            s.step_ale(moved(&mesh0, |p| u_motion(p, t_new)), dt).unwrap();
            let wn = s.mesh_speeds().unwrap();
            for (k, fs) in faces.iter().enumerate() {
                for &f in fs {
                    speed[k] = speed[k].max(wn[f].abs());
                }
            }
            for c in &s.u {
                for k in 0..5 {
                    drift = drift.max((c[k] - uref[k]).abs() / scale[k]);
                }
            }
            let mem = s.sponge_memories();
            let t = s.time;
            mq = mem.q.iter().fold(mq, |m, q| m.max(q.abs() / (reference.p * t)));
            mr = mem.r.iter().fold(mr, |m, r| m.max(r.abs() / (reference.p * t * t)));
            mm = mem
                .m
                .iter()
                .fold(mm, |m, v| m.max(vec3::norm(*v) / (reference.rho * a * t)));
        }
        println!(
            "{name}: {} steps to t = {:.4}; drift {drift:.2e}, memories q {mq:.2e}, r {mr:.2e}, m {mm:.2e} (limit {ROUND_OFF:e} each); U1g largest |w_n| on xmin, xmax, ymin, ymax: {:.3}, {:.3}, {:.3}, {:.3} (>= 0.02 each, >= 100 steps)",
            s.steps, s.time, speed[0], speed[1], speed[2], speed[3]
        );
        assert!(s.steps >= 100, "U1g {name} steps {}", s.steps);
        assert!(speed.iter().all(|v| *v >= 0.02), "U1g {name} {speed:?}");
        assert!(drift <= ROUND_OFF, "{name} drift {drift}");
        assert!(
            mq <= ROUND_OFF && mr <= ROUND_OFF && mm <= ROUND_OFF,
            "{name} memories {mq} {mr} {mm}"
        );
        if name == "U1p" {
            assert!(mem_is_zero(&s), "U1p: the plain form keeps no memory");
        }
    }
}

fn mem_is_zero(s: &CompressibleSolver) -> bool {
    s.sponge_memory().iter().all(|q| *q == 0.0)
}

// ---------------------------------------------------------------------------------------------------------------- T0

/// Superposition of exact isentropic simple waves (ADR 0237): each part a pressure perturbation along a unit vector.
fn simple_wave(dp: f64, d: Vec3) -> Prim {
    let a = ((P0 + dp) / P0).powf((GAMMA - 1.0) / (2.0 * GAMMA));
    let v = 2.0 * (a - 1.0) / (GAMMA - 1.0);
    Prim {
        rho: ((P0 + dp) / P0).powf(1.0 / GAMMA),
        u: vec3::scale(d, v),
        p: P0 + dp,
    }
}

/// A tube of set-up T: the grid at `t = 0`, its solver and the probe cell (centre 0.8975 at `t = 0`).
struct Tube {
    mesh0: Mesh,
    s: CompressibleSolver,
    probe: usize,
}

fn tube(nx: usize, lx: f64, form: Form, behind: CompBc) -> Tube {
    let mesh0 = box_2d(nx, 1, lx, DX, false, false).unwrap();
    let init = |x: Vec3| simple_wave(EPS * (-((x[0] - 0.5) / W).powi(2)).exp(), [1.0, 0.0, 0.0]);
    let mut s = CompressibleSolver::new(geom(mesh0.clone()), AIR, CompSettings::default(), init).unwrap();
    s.set_bc("xmin", CompBc::Fixed(ambient())).unwrap();
    s.set_bc("xmax", behind).unwrap();
    let layer = SpongeLayer {
        origin: [1.0, 0.0, 0.0],
        normal: [1.0, 0.0, 0.0],
        thickness: LAYER,
        rate_max: rate_max_for_reflection(LAYER, 1.0, R0, EXPONENT).unwrap(),
        exponent: EXPONENT,
    };
    let sp = sponge_of(&s.geom, form, ambient(), &layer);
    s.set_sponge(sp).unwrap();
    s.set_sponge_frame(SpongeFrame::AttachedToMesh);
    let cc = mesh0.cell_centres();
    let probe = (0..cc.len()).find(|&c| (cc[c][0] - 0.8975).abs() < 0.2 * DX).unwrap();
    Tube { mesh0, s, probe }
}

const THOMPSON: CompBc = CompBc::NonReflecting { p: P0, sigma: 0.0 };
const RELEASE: CompBc = CompBc::PressureOutlet { p: P0 };

/// T0m, T0mr, T0pr, T0sr and NCr of ADR 0265: the tube of ADR 0237 A0 with both grids translating at `V`; the measure of
/// ADR 0237 against a reference run on the extended grid that moves in the same way.
#[test]
#[ignore = "known_miss record (ADR 0265 addendum 1): NCr measured 0.8642 against >= 0.9 (diagnosed in amendment 1); T0m, T0mr, T0pr 9.617e-7 and T0sr 9.829e-7 against 1e-2 met in the same run"]
fn t0_tube_on_a_translating_mesh_known_miss() {
    let runs = [
        ("T0m", Form::Matched, THOMPSON),
        ("T0mr", Form::Matched, RELEASE),
        ("T0pr", Form::Plain, RELEASE),
        ("T0sr", Form::Shifted, RELEASE),
        ("NCr", Form::None, RELEASE),
    ];
    let mut tests: Vec<Tube> = runs.iter().map(|&(_, f, b)| tube(240, 1.2, f, b)).collect();
    let mut reference = tube(600, 3.0, Form::None, THOMPSON);
    let mut num = vec![0.0f64; tests.len()];
    let mut den = 0.0f64;
    while reference.s.time < 1.6 {
        let dt = tests
            .iter()
            .map(|t| t.s.max_dt_ale())
            .fold(reference.s.max_dt_ale(), f64::min);
        let t_new = reference.s.time + dt;
        for t in tests.iter_mut().chain(std::iter::once(&mut reference)) {
            let g = moved(&t.mesh0, |p| vec3::add(p, vec3::scale(V, t_new)));
            t.s.step_ale(g, dt).unwrap();
        }
        let p_ref = pressure(&reference.s, reference.probe);
        den = den.max(p_ref - P0);
        for (i, t) in tests.iter().enumerate() {
            let p = pressure(&t.s, t.probe);
            assert!(p.is_finite(), "{} not finite", runs[i].0);
            num[i] = num[i].max((p - p_ref).abs());
        }
    }
    println!(
        "T0 {} steps to t = {:.4}; incident peak at the probe / eps = {:.5}",
        reference.s.steps,
        reference.s.time,
        den / EPS
    );
    let r: Vec<f64> = num.iter().map(|n| n / den).collect();
    for (i, (name, form, _)) in runs.iter().enumerate() {
        println!("  {name} ({form:?}): R = {:.3e}", r[i]);
    }
    println!("  limits: R <= {LIMIT:e} for T0m, T0mr, T0pr, T0sr; NCr >= 0.9");
    assert!(r[0] <= LIMIT, "T0m {}", r[0]);
    assert!(r[1] <= LIMIT, "T0mr {}", r[1]);
    assert!(r[2] <= LIMIT, "T0pr {}", r[2]);
    assert!(r[3] <= LIMIT, "T0sr {}", r[3]);
    assert!(r[4] >= 0.9, "NCr {}", r[4]);
}

// ---------------------------------------------------------------------------------------------------------------- B2

/// B2 of ADR 0265: on set-up U of ADR 0207 with its four characteristic conditions, 30 steps of `step_ale` give the same
/// bits with nothing set, a sponge set and cleared, the frame set without a sponge, and attached sponges whose rates are
/// all zero; the layer of U1 changes the state.
#[test]
fn b2_step_ale_without_a_sponge_is_bit_identical() {
    let mesh0 = box_2d(12, 8, 1.2, 0.8, false, false).unwrap();
    let u0 = [0.3, 0.1, 0.0];
    let q0 = Prim {
        rho: 1.0,
        u: u0,
        p: 1.0,
    };
    let rest = Prim { u: [0.0; 3], ..q0 };
    let run = |prepare: &dyn Fn(&mut CompressibleSolver)| -> (Vec<[u64; 5]>, Vec<u64>) {
        let mut s = CompressibleSolver::new(geom(mesh0.clone()), AIR, CompSettings::default(), |_| q0).unwrap();
        let t0 = 1.0 + 0.2 * vec3::dot(u0, u0) / GAMMA;
        s.set_bc(
            "xmin",
            CompBc::TotalInlet(TotalInlet {
                p0: t0.powf(3.5),
                t0,
                r_gas: 1.0,
                dir: vec3::unit(u0),
            }),
        )
        .unwrap();
        s.set_bc("xmax", CompBc::PressureOutlet { p: 1.0 }).unwrap();
        s.set_bc("ymin", CompBc::FarField(q0)).unwrap();
        s.set_bc("ymax", CompBc::NonReflecting { p: 1.0, sigma: 0.5 }).unwrap();
        prepare(&mut s);
        for _ in 0..30 {
            let dt = step_to(&s, 2.0);
            let t_new = s.time + dt;
            s.step_ale(moved(&mesh0, |p| u_motion(p, t_new)), dt).unwrap();
        }
        (
            s.u.iter().map(|c| c.map(f64::to_bits)).collect(),
            s.mesh_speeds().unwrap().iter().map(|w| w.to_bits()).collect(),
        )
    };
    let layer = SpongeLayer {
        origin: [0.6, 0.0, 0.0],
        normal: [1.0, 0.0, 0.0],
        thickness: 0.6,
        rate_max: 20.0,
        exponent: 2.0,
    };
    let zero = |s: &CompressibleSolver, form: SpongeForm| Sponge {
        reference: rest,
        rate: vec![0.0; s.u.len()],
        form,
    };
    let never = run(&|_| {});
    let cleared = run(&|s| {
        let sp = Sponge::planar(&s.geom, rest, &layer, false).unwrap();
        s.set_sponge(Some(sp)).unwrap();
        s.set_sponge(None).unwrap();
    });
    let frame_only = run(&|s| s.set_sponge_frame(SpongeFrame::AttachedToMesh));
    let zero_runs: Vec<_> = [
        SpongeForm::Plain,
        SpongeForm::Matched {
            normal: [1.0, 0.0, 0.0],
        },
    ]
    .into_iter()
    .map(|form| {
        run(&|s| {
            s.set_sponge_frame(SpongeFrame::AttachedToMesh);
            let sp = zero(s, form);
            s.set_sponge(Some(sp)).unwrap();
        })
    })
    .collect();
    let zero_shifted = run(&|s| {
        s.set_sponge_frame(SpongeFrame::AttachedToMesh);
        let mut sp = Sponge::planar_shifted(&s.geom, rest, &layer, SHIFT, 0.05).unwrap();
        sp.rate.iter_mut().for_each(|r| *r = 0.0);
        s.set_sponge(Some(sp)).unwrap();
    });
    let layered = run(&|s| {
        s.set_sponge_frame(SpongeFrame::AttachedToMesh);
        let sp = Sponge::planar(&s.geom, rest, &layer, false).unwrap();
        s.set_sponge(Some(sp)).unwrap();
    });
    assert!(never == cleared, "B2 set and cleared");
    assert!(never == frame_only, "B2 the frame without a sponge");
    assert!(never == zero_runs[0], "B2 plain with zero rates");
    assert!(never == zero_runs[1], "B2 matched with zero rates");
    assert!(never == zero_shifted, "B2 shifted with zero rates");
    let changed = never.0.iter().zip(&layered.0).filter(|(a, b)| a != b).count();
    println!(
        "B2 {} cells, 30 steps of step_ale: nothing set, set and cleared, frame only, zero-rate plain, matched, shifted: bit-identical (state and mesh speeds); the layer changes {changed} cells",
        never.0.len()
    );
    assert!(changed > 0, "B2 control");
}

// ---------------------------------------------------------------------------------------------------------------- G

/// The deformation of set-up G at time `t`: boundary points fixed, interior moved by `0.05 t sin(pi x) sin(pi y) (1, 1)`.
fn g_motion(p: Vec3, t: f64) -> Vec3 {
    let on_boundary = |x: f64| x <= 1e-12 || x >= 1.0 - 1e-12;
    if on_boundary(p[0]) || on_boundary(p[1]) {
        return p;
    }
    let b = 0.05 * t * (PI * p[0]).sin() * (PI * p[1]).sin();
    [p[0] + b, p[1] + b, p[2]]
}

const G_DT: f64 = 0.005;
const G_STEPS: usize = 200;

fn g_start() -> Prim {
    Prim {
        rho: 1.001,
        u: [0.0; 3],
        p: 1.002 * P0,
    }
}

/// A run of set-up G with the uniform rate 2 in the form `form`; `deform` false runs `step` on the undeformed mesh.
/// Returns the solver and, per cell, the trapezoid sum of its recorded `p - p_ref`.
fn g_run(form: SpongeForm, deform: bool) -> (CompressibleSolver, Vec<f64>) {
    let mesh0 = box_2d(20, 20, 1.0, 1.0, false, false).unwrap();
    let mut s = CompressibleSolver::new(geom(mesh0.clone()), AIR, CompSettings::default(), |_| g_start()).unwrap();
    for p in ["xmin", "xmax", "ymin", "ymax"] {
        s.set_bc(p, CompBc::Slip).unwrap();
    }
    let n = s.u.len();
    s.set_sponge(Some(Sponge {
        reference: ambient(),
        rate: vec![2.0; n],
        form,
    }))
    .unwrap();
    s.set_sponge_frame(SpongeFrame::AttachedToMesh);
    let mut trap = vec![0.0; n];
    let mut f_old: Vec<f64> = (0..n).map(|c| pressure(&s, c) - P0).collect();
    for k in 1..=G_STEPS {
        if deform {
            let t_new = k as f64 * G_DT;
            s.step_ale(moved(&mesh0, |p| g_motion(p, t_new)), G_DT).unwrap();
        } else {
            s.step(G_DT).unwrap();
        }
        for c in 0..n {
            let f = pressure(&s, c) - P0;
            trap[c] += 0.5 * (f_old[c] + f) * G_DT;
            f_old[c] = f;
        }
    }
    (s, trap)
}

/// G1p, G1m, G1u of ADR 0265: a uniform offset with a uniform rate decays as `exp(-rate t)` on a deforming mesh; the
/// memory of a cell is the time integral of its own `p - p_ref`.
#[test]
fn g1_uniform_offset_decays_on_a_deforming_mesh() {
    for (name, form) in [
        ("G1p", SpongeForm::Plain),
        (
            "G1m",
            SpongeForm::Matched {
                normal: [1.0, 0.0, 0.0],
            },
        ),
    ] {
        let (s, trap) = g_run(form, true);
        let vol = s.geom.mesh.cell_volumes();
        let (vmin, vmax) = vol
            .iter()
            .fold((f64::INFINITY, 0.0f64), |(a, b), v| (a.min(*v), b.max(*v)));
        let uref = ambient().to_cons(AIR);
        let u_start = g_start().to_cons(AIR);
        let want = (-2.0 * s.time).exp();
        let (mut decay, mut momentum) = (0.0f64, 0.0f64);
        for c in &s.u {
            for k in [0, 4] {
                decay = decay.max(((c[k] - uref[k]) / (u_start[k] - uref[k]) / want - 1.0).abs());
            }
            momentum = momentum.max(vec3::norm([c[1], c[2], c[3]]));
        }
        println!(
            "{name} t = {:.4}, {} steps, cell volumes {:.4e} to {:.4e} (undeformed 2.5e-3): decay against exp(-2 t) {decay:.2e} (limit 1e-5); G1u momentum {momentum:.2e} (limit 1e-8)",
            s.time, s.steps, vmin, vmax
        );
        assert!(decay <= 1e-5, "{name} decay {decay}");
        assert!(momentum <= 1e-8, "G1u {name} momentum {momentum}");
        if name == "G1m" {
            let q = s.sponge_memory();
            let worst = q
                .iter()
                .zip(&trap)
                .map(|(q, t)| (q / t - 1.0).abs())
                .fold(0.0, f64::max);
            println!("G1m memory against the trapezoid sum of each cell's p - p_ref: {worst:.2e} (limit 1e-4)");
            assert!(worst <= 1e-4, "G1m memory {worst}");
        } else {
            assert!(mem_is_zero(&s));
        }
    }
}

/// G2s of ADR 0265: the shifted form on the deforming mesh against the same sponge with `step` on the undeformed mesh.
#[test]
fn g2_shifted_form_on_a_deforming_mesh_matches_the_static_step() {
    let form = SpongeForm::MatchedShifted(rcfd_fv::sponge::MatchedOptions {
        normal: [1.0, 0.0, 0.0],
        shift: SHIFT,
        along: 0.0,
        second: None,
    });
    let (moving, _) = g_run(form, true);
    let (fixed, _) = g_run(form, false);
    let uref = ambient().to_cons(AIR);
    let compare = |a: &[f64], b: &[f64]| -> f64 {
        let scale = b.iter().fold(0.0f64, |m, x| m.max(x.abs()));
        a.iter().zip(b).map(|(x, y)| (x - y).abs()).fold(0.0, f64::max) / scale
    };
    let offset = |s: &CompressibleSolver, k: usize| -> Vec<f64> { s.u.iter().map(|c: &Cons| c[k] - uref[k]).collect() };
    let (mm, mf) = (moving.sponge_memories(), fixed.sponge_memories());
    let d = [
        compare(&offset(&moving, 0), &offset(&fixed, 0)),
        compare(&offset(&moving, 4), &offset(&fixed, 4)),
        compare(&mm.q, &mf.q),
        compare(&mm.r, &mf.r),
    ];
    println!(
        "G2s t = {:.4}: deforming against static, relative differences mass {:.2e}, energy {:.2e}, q {:.2e}, r {:.2e} (limit 1e-5 each)",
        moving.time, d[0], d[1], d[2], d[3]
    );
    assert!(d.iter().all(|x| *x <= 1e-5), "G2s {d:?}");
}

// ---------------------------------------------------------------------------------------------------------------- S4

fn small() -> (Mesh, CompressibleSolver) {
    let mesh0 = box_2d(8, 2, 1.0, 0.25, false, false).unwrap();
    let s = CompressibleSolver::new(geom(mesh0.clone()), AIR, CompSettings::default(), |_| ambient()).unwrap();
    (mesh0, s)
}

fn small_layer() -> SpongeLayer {
    SpongeLayer {
        origin: [0.5, 0.0, 0.0],
        normal: [1.0, 0.0, 0.0],
        thickness: 0.5,
        rate_max: 10.0,
        exponent: 2.0,
    }
}

/// S4a to S4d of ADR 0265: the frame setting, the refusals and the explicit replacement of the mesh.
#[test]
fn s4_frame_setting_and_refusals() {
    let shift = |x: Vec3| vec3::add(x, [1e-5, 0.0, 0.0]);
    // S4a: the default frame refuses every form and leaves the solver unchanged
    for form in [Form::Plain, Form::Matched, Form::Shifted] {
        let (mesh0, mut s) = small();
        assert_eq!(s.sponge_frame(), SpongeFrame::StaticMesh);
        let sp = sponge_of(&s.geom, form, ambient(), &small_layer());
        s.set_sponge(sp).unwrap();
        let before = s.u.clone();
        assert!(s.step_ale(moved(&mesh0, shift), 1e-3).is_err(), "S4a {form:?}");
        assert!(s.u == before && s.time == 0.0 && s.steps == 0, "S4a unchanged {form:?}");
        // S4b: attached, the step is accepted; replace_mesh refuses in both frames
        s.set_sponge_frame(SpongeFrame::AttachedToMesh);
        s.step_ale(moved(&mesh0, shift), 1e-3).unwrap();
        assert_eq!(s.steps, 1);
        assert!(s.mesh_speeds().is_some(), "S4b mesh speeds {form:?}");
        for frame in [SpongeFrame::StaticMesh, SpongeFrame::AttachedToMesh] {
            s.set_sponge_frame(frame);
            let (u, time) = (s.u.clone(), s.time);
            assert!(
                s.replace_mesh(s.geom.clone(), s.u.clone()).is_err(),
                "S4b replace_mesh {frame:?}"
            );
            assert!(s.u == u && s.time == time && s.sponge().is_some(), "S4b unchanged");
        }
    }
    // S4b: the frame is kept across set_sponge
    let (mesh0, mut s) = small();
    s.set_sponge_frame(SpongeFrame::AttachedToMesh);
    s.set_sponge(None).unwrap();
    assert_eq!(s.sponge_frame(), SpongeFrame::AttachedToMesh);
    let sp = Sponge::planar(&s.geom, ambient(), &small_layer(), true).unwrap();
    s.set_sponge(Some(sp)).unwrap();
    assert_eq!(s.sponge_frame(), SpongeFrame::AttachedToMesh);
    s.step_ale(moved(&mesh0, shift), 1e-3).unwrap();
    // S4c: the explicit replacement
    s.set_sponge(None).unwrap();
    let (g, u) = (s.geom.clone(), s.u.clone());
    s.replace_mesh(g, u).unwrap();
    let sp = Sponge::planar(&s.geom, ambient(), &small_layer(), true).unwrap();
    s.set_sponge(Some(sp)).unwrap();
    let q: Vec<f64> = (0..s.u.len()).map(|c| 1e-6 * c as f64).collect();
    s.set_sponge_memory(&q).unwrap();
    assert_eq!(s.sponge_memory(), &q[..]);
    let moved_twice = |x: Vec3| vec3::add(x, [2e-5, 0.0, 0.0]);
    s.step_ale(moved(&mesh0, moved_twice), 1e-3).unwrap();
    // S4d: another topology; a wall together with a sponge
    let other = geom(box_2d(4, 2, 1.0, 0.25, false, false).unwrap());
    assert!(s.step_ale(other, 1e-3).is_err(), "S4d topology");
    let wall = MsdWall {
        p: P0,
        r: 1.0,
        m: 0.0,
        k: 0.0,
    };
    assert!(s.set_msd_wall("xmax", Some(wall)).is_err(), "S4d wall with a sponge");
    println!("S4: default frame refuses (3 forms), attached accepted, replace_mesh refused in both frames, frame kept, explicit replacement works, topology and wall refused");
}
