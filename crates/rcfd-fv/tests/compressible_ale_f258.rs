//! F-258 (ADR 0207): the compressible ALE step with characteristic conditions on moving faces and the mesh-speed time step.
//! T1 the time step against its closed form, T2 uniform flow with moving characteristic faces, T3 a piston-generated simple
//! wave leaving through a moving outlet, T4 the face state of a moving face. The limits are the ones registered in ADR 0207;
//! the records R1 to R4 of its defaults are in `compressible_ale_f258_records.rs`.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::compressible::{characteristic_face_state, CompBc, CompSettings, CompressibleSolver, Gas, Prim};
use rcfd_fv::compressible_bc::{self, TotalInlet};
use rcfd_fv::FvGeometry;
use rcfd_mesh::gen::{box_2d, box_mesh, Grading};
use rcfd_mesh::motion::moved_mesh;
use rcfd_mesh::Mesh;
use std::f64::consts::PI;
use std::sync::Arc;

const AIR: Gas = Gas::AIR;
const GAMMA: f64 = 1.4;
/// T1a, T1b: "to round-off" (ADR 0207 reading 2).
const ROUND_OFF: f64 = 1e-12;
/// T1n: the absolute step must differ from the closed form by at least this.
const T1N_MIN: f64 = 0.05;
/// U1: uniform flow on the moving mesh.
const UNIFORM: f64 = 1e-12;
/// U2: steps and the largest normal mesh speed on each characteristic patch.
const U2_STEPS: usize = 100;
const U2_SPEED: f64 = 0.02;
/// S0, S1, S2: against the incident pressure jump.
const S_LIMIT: f64 = 1e-2;
/// SN: the fixed-pressure outlet must reflect at least this much.
const SN_MIN: f64 = 0.5;
/// T4b, T4c tolerances.
const T4_TOL: f64 = 1e-14;
const T4_ENTROPY: f64 = 1e-12;

fn geom(m: Mesh) -> FvGeometry {
    FvGeometry::new(Arc::new(m)).unwrap()
}

/// The end test of ADR 0191's library loops, the schedule of ADR 0207.
fn ended(time: f64, end: f64) -> bool {
    time >= end - 1e-12 * end.max(1.0)
}

/// The step of ADR 0207's schedule: `max_dt_ale()` clipped to the end time.
fn step_to(s: &CompressibleSolver, end: f64) -> f64 {
    s.max_dt_ale().min((end - s.time).max(1e-300))
}

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs()
}

// ---------------------------------------------------------------------------------------------------------------- T1

/// A uniform gas on a mesh translating at `w`, every patch fixed to the state.
struct Translating {
    s: CompressibleSolver,
    mesh0: Mesh,
    w: Vec3,
    u0: Vec3,
    h: Vec<f64>,
}

impl Translating {
    fn new(mesh0: Mesh, u0: Vec3, w: Vec3, h: Vec<f64>) -> Self {
        let st = Prim {
            rho: 1.0,
            u: u0,
            p: 1.0,
        };
        let mut s = CompressibleSolver::new(geom(mesh0.clone()), AIR, CompSettings::default(), |_| st).unwrap();
        let names: Vec<String> = mesh0.patches().iter().map(|p| p.name.clone()).collect();
        for n in names {
            if n != "frontAndBack" {
                s.set_bc(&n, CompBc::Fixed(st)).unwrap();
            }
        }
        Translating { s, mesh0, w, u0, h }
    }

    /// The closed form of ADR 0207 M1, `cfl / sum_d (|u_d - w_d| + a) / h_d` over the directions in `h`.
    fn closed(&self, w: Vec3) -> f64 {
        let a = GAMMA.sqrt();
        let sum: f64 = (0..self.h.len())
            .map(|d| ((self.u0[d] - w[d]).abs() + a) / self.h[d])
            .sum();
        0.4 / sum
    }

    fn step(&mut self) {
        let dt = self.s.max_dt_ale();
        let t_new = self.s.time + dt;
        let points: Vec<Vec3> = self
            .mesh0
            .points()
            .iter()
            .map(|&p| vec3::add(p, vec3::scale(self.w, t_new)))
            .collect();
        self.s
            .step_ale(geom(moved_mesh(&self.mesh0, points).unwrap()), dt)
            .unwrap();
    }
}

/// T1a, T1b, T1n of ADR 0207 on one set-up.
fn t1_run(name: &str, mut m: Translating) {
    let zeros = |s: &CompressibleSolver| vec![0.0; s.geom.mesh.n_faces()];
    let dt_w = m.closed(m.w);
    let dt_0 = m.closed([0.0; 3]);
    // T1b before step 1
    let z = m.s.max_dt_moving(&zeros(&m.s)).unwrap();
    assert_eq!(
        z.to_bits(),
        m.s.max_dt().to_bits(),
        "T1b {name}: zero speeds against max_dt before step 1"
    );
    let b0 = rel(m.s.max_dt(), dt_0);
    let mut worst = 0.0f64;
    let mut neg = f64::NAN;
    for step in 1..=10 {
        if step >= 2 {
            let a = m.s.max_dt_ale();
            let b = m.s.max_dt_moving(m.s.mesh_speeds().unwrap()).unwrap();
            worst = worst.max(rel(a, dt_w)).max(rel(b, dt_w));
            if step == 2 {
                neg = rel(m.s.max_dt(), dt_w);
            }
        }
        m.step();
    }
    let z = m.s.max_dt_moving(&zeros(&m.s)).unwrap();
    let zero_bits = z.to_bits() == m.s.max_dt().to_bits();
    let b10 = rel(m.s.max_dt(), dt_0);
    println!(
        "T1 {name}: closed form {dt_w:.12e}; T1a worst relative {worst:.2e} (limit {ROUND_OFF:e}); T1b zero speeds bit for bit after step 10: {zero_bits}, max_dt against the closed form with w = 0 {b0:.2e} / {b10:.2e}; T1n max_dt against dt_w {neg:.4} (>= {T1N_MIN})"
    );
    assert!(worst <= ROUND_OFF, "T1a {name} {worst}");
    assert!(zero_bits, "T1b {name} after step 10");
    assert!(b0 <= ROUND_OFF && b10 <= ROUND_OFF, "T1b {name} {b0} {b10}");
    assert!(neg >= T1N_MIN, "T1n {name} {neg}");
}

/// T1a, T1b, T1n of ADR 0207: the mesh-speed step of a uniform gas on a translating mesh against its closed form.
#[test]
fn t1_mesh_speed_time_step_against_the_closed_form() {
    let m3 = box_mesh([5, 4, 3], [0.5, 0.6, 0.6], [Grading::Uniform; 3]).unwrap();
    t1_run(
        "M3D",
        Translating::new(m3, [0.3, -0.2, 0.1], [-0.5, 0.25, 0.4], vec![0.1, 0.15, 0.2]),
    );
    let m2 = box_2d(6, 4, 0.6, 0.4, false, false).unwrap();
    t1_run(
        "M2D",
        Translating::new(m2, [0.3, -0.2, 0.0], [-0.6, 0.3, 0.0], vec![0.1, 0.1]),
    );
}

/// T1c of ADR 0207: the API of the kept mesh speeds.
#[test]
fn t1c_mesh_speeds_api() {
    let m3 = box_mesh([5, 4, 3], [0.5, 0.6, 0.6], [Grading::Uniform; 3]).unwrap();
    let mut m = Translating::new(m3, [0.3, -0.2, 0.1], [-0.5, 0.25, 0.4], vec![0.1, 0.15, 0.2]);
    assert!(m.s.mesh_speeds().is_none(), "T1c: None on a new solver");
    assert_eq!(
        m.s.max_dt_ale().to_bits(),
        m.s.max_dt().to_bits(),
        "T1c: max_dt_ale without speeds"
    );
    m.step();
    let nf = m.s.geom.mesh.n_faces();
    assert_eq!(
        m.s.mesh_speeds().map(<[f64]>::len),
        Some(nf),
        "T1c: Some after step_ale"
    );
    let mut short = m.s.mesh_speeds().unwrap().to_vec();
    short.pop();
    assert!(m.s.max_dt_moving(&short).is_err(), "T1c: a list one short");
    let mut nan = m.s.mesh_speeds().unwrap().to_vec();
    nan[3] = f64::NAN;
    assert!(m.s.max_dt_moving(&nan).is_err(), "T1c: a NaN");
    let mut after_step = m.s.clone();
    let dt = after_step.max_dt();
    after_step.step(dt).unwrap();
    assert!(after_step.mesh_speeds().is_none(), "T1c: None after step");
    assert_eq!(
        after_step.max_dt_ale().to_bits(),
        after_step.max_dt().to_bits(),
        "T1c: max_dt_ale after step"
    );
    let mut replaced = m.s.clone();
    let g = replaced.geom.clone();
    let u = replaced.u.clone();
    replaced.replace_mesh(g, u).unwrap();
    assert!(replaced.mesh_speeds().is_none(), "T1c: None after replace_mesh");
    println!("T1c: mesh_speeds None / Some({nf}) / None after step / None after replace_mesh; Err for a short list and a NaN");
}

// ---------------------------------------------------------------------------------------------------------------- T2

/// The motion of set-up U of ADR 0207 T2 at time `t`.
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

/// U1, U2 of ADR 0207: uniform flow with the four characteristic conditions on moving faces.
#[test]
fn u_uniform_flow_with_moving_characteristic_faces() {
    let mesh0 = box_2d(12, 8, 1.2, 0.8, false, false).unwrap();
    let u0 = [0.3, 0.1, 0.0];
    let q0 = Prim {
        rho: 1.0,
        u: u0,
        p: 1.0,
    };
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
    let patches = ["xmin", "xmax", "ymin", "ymax"];
    let faces: Vec<Vec<usize>> = patches
        .iter()
        .map(|n| mesh0.patch(n).unwrap().faces().collect())
        .collect();
    let mut speed = [0.0f64; 4];
    let mut u1 = 0.0f64;
    let un0 = vec3::norm(u0);
    while !ended(s.time, 2.0) {
        let dt = step_to(&s, 2.0);
        let t_new = s.time + dt;
        let points: Vec<Vec3> = mesh0.points().iter().map(|&p| u_motion(p, t_new)).collect();
        s.step_ale(geom(moved_mesh(&mesh0, points).unwrap()), dt).unwrap();
        let wn = s.mesh_speeds().unwrap();
        for (k, fs) in faces.iter().enumerate() {
            for &f in fs {
                speed[k] = speed[k].max(wn[f].abs());
            }
        }
        for p in s.primitives() {
            u1 = u1
                .max((p.rho / q0.rho - 1.0).abs())
                .max(vec3::norm(vec3::sub(p.u, u0)) / un0)
                .max((p.p / q0.p - 1.0).abs());
        }
    }
    println!(
        "U1 largest deviation over the run {u1:.2e} (limit {UNIFORM:e}); U2 {} steps (>= {U2_STEPS}), largest |w_n| on {patches:?}: {speed:?} (>= {U2_SPEED})",
        s.steps
    );
    assert!(s.steps >= U2_STEPS, "U2 steps {}", s.steps);
    assert!(speed.iter().all(|&w| w >= U2_SPEED), "U2 speeds {speed:?}");
    assert!(u1 <= UNIFORM, "U1 {u1}");
}

// ---------------------------------------------------------------------------------------------------------------- T3

const U_P: f64 = 0.1;
const TAU: f64 = 0.25;
const W_O: f64 = -0.2;

fn x_piston(t: f64) -> f64 {
    if t <= TAU {
        0.5 * U_P * (t - TAU / PI * (PI * t / TAU).sin())
    } else {
        0.5 * U_P * TAU + U_P * (t - TAU)
    }
}

fn x_outlet(t: f64) -> f64 {
    1.0 + W_O * t
}

/// The column of set-up S at time `t`: the initial `x` in `[0, 1]` mapped between the piston and the outlet.
fn s_points(mesh0: &Mesh, t: f64) -> Vec<Vec3> {
    let (xp, xo) = (x_piston(t), x_outlet(t));
    mesh0
        .points()
        .iter()
        .map(|p| [xp + (xo - xp) * p[0], p[1], p[2]])
        .collect()
}

/// The post-wave state of ADR 0207 M5: `p1`.
fn p1() -> f64 {
    let a1 = 1.0 + 0.5 * (GAMMA - 1.0) * U_P;
    a1.powf(2.0 * GAMMA / (GAMMA - 1.0))
}

/// What one run of set-up S gives: the reflection `R` over the window, and the S0 values at `t = 0.7`.
struct SimpleWave {
    r: f64,
    plateau: f64,
    ahead: f64,
    steps: usize,
}

/// One run of set-up S with the outlet condition `bc`.
fn simple_wave(bc: CompBc) -> SimpleWave {
    let mesh0 = box_2d(200, 1, 1.0, 0.1, false, false).unwrap();
    let rest = Prim {
        rho: 1.4,
        u: [0.0; 3],
        p: 1.0,
    };
    let mut s = CompressibleSolver::new(geom(mesh0.clone()), AIR, CompSettings::default(), |_| rest).unwrap();
    s.set_bc("xmax", bc).unwrap();
    let (p0, p1) = (1.0, p1());
    let dp = p1 - p0;
    let (mut r, mut plateau, mut ahead) = (0.0f64, f64::NAN, f64::NAN);
    for target in [0.7, 1.5] {
        while !ended(s.time, target) {
            let dt = step_to(&s, target);
            let t_new = s.time + dt;
            s.step_ale(geom(moved_mesh(&mesh0, s_points(&mesh0, t_new)).unwrap()), dt)
                .unwrap();
            if s.time >= 1.1 && s.time <= 1.5 + 1e-12 {
                for p in s.primitives() {
                    r = r.max((p.p - p1).abs() / dp);
                }
            }
        }
        if target == 0.7 {
            let (xp, cc) = (x_piston(0.7), s.geom.mesh.cell_centres());
            let prim = s.primitives();
            let (mut a, mut b) = (0.0f64, 0.0f64);
            for (c, p) in cc.iter().zip(&prim) {
                if c[0] >= xp && c[0] <= 0.4 {
                    a = a.max((p.p - p1).abs() / dp);
                }
                if c[0] >= 0.75 {
                    b = b.max((p.p - p0).abs() / dp);
                }
            }
            plateau = a;
            ahead = b;
        }
    }
    SimpleWave {
        r,
        plateau,
        ahead,
        steps: s.steps,
    }
}

/// S0, S1, S2, SN of ADR 0207: a piston-generated simple wave leaving through a moving characteristic outlet.
#[test]
fn s_simple_wave_through_a_moving_outlet() {
    let rest = Prim {
        rho: 1.4,
        u: [0.0; 3],
        p: 1.0,
    };
    let ff = simple_wave(CompBc::FarField(rest));
    let nr = simple_wave(CompBc::NonReflecting { p: 1.0, sigma: 0.0 });
    let po = simple_wave(CompBc::PressureOutlet { p: 1.0 });
    let mid = simple_wave(CompBc::NonReflecting { p: 1.0, sigma: 0.25 });
    println!("S: p1 = {:.6}, dp = {:.6}", p1(), p1() - 1.0);
    println!(
        "S0 far field at t = 0.7: plateau {:.2e}, ahead of the front {:.2e} (limit {S_LIMIT:e} each)",
        ff.plateau, ff.ahead
    );
    println!("S1 far field R = {:.3e} (limit {S_LIMIT:e}), {} steps", ff.r, ff.steps);
    println!(
        "S2 non-reflecting sigma 0 R = {:.3e} (limit {S_LIMIT:e}), {} steps",
        nr.r, nr.steps
    );
    println!("SN pressure outlet R = {:.3e} (>= {SN_MIN}), {} steps", po.r, po.steps);
    println!(
        "printed: non-reflecting sigma 0.25 R = {:.3e}, {} steps",
        mid.r, mid.steps
    );
    assert!(
        ff.plateau <= S_LIMIT && ff.ahead <= S_LIMIT,
        "S0 {} {}",
        ff.plateau,
        ff.ahead
    );
    assert!(po.r >= SN_MIN, "SN {}", po.r);
    assert!(ff.r <= S_LIMIT, "S1 {}", ff.r);
    assert!(nr.r <= S_LIMIT, "S2 {}", nr.r);
}

// ---------------------------------------------------------------------------------------------------------------- T4

/// Deterministic pseudo-random numbers in [0, 1).
fn rnd(seed: &mut u64) -> f64 {
    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    ((*seed >> 11) as f64) / ((1u64 << 53) as f64)
}

fn bits(p: &Prim) -> [u64; 5] {
    [p.rho, p.u[0], p.u[1], p.u[2], p.p].map(f64::to_bits)
}

/// T4a of ADR 0207: with `wn = 0` the face state is the static one of `compressible_bc`, bit for bit.
#[test]
fn t4a_zero_speed_is_the_static_face_state() {
    let mut seed = 2027u64;
    for i in 0..20 {
        let a = rnd(&mut seed) * 2.0 * PI;
        let b = (rnd(&mut seed) - 0.5) * PI;
        let n = [b.cos() * a.cos(), b.cos() * a.sin(), b.sin()];
        let rho = 0.5 + rnd(&mut seed);
        let p = 0.5 + rnd(&mut seed);
        let c = (GAMMA * p / rho).sqrt();
        // speeds up to 2 c: sub- and supersonic normal components
        let u = [0, 1, 2].map(|_| (rnd(&mut seed) - 0.5) * 4.0 * c);
        let int = Prim { rho, u, p };
        let fs = Prim {
            rho: 0.8 + 0.4 * rnd(&mut seed),
            u: [0, 1, 2].map(|_| (rnd(&mut seed) - 0.5) * 2.0 * c),
            p: 0.8 + 0.4 * rnd(&mut seed),
        };
        let ti = TotalInlet {
            p0: 1.5 + rnd(&mut seed),
            t0: 1.2 + rnd(&mut seed),
            r_gas: 1.0,
            dir: vec3::scale(n, if i % 2 == 0 { -1.0 } else { 1.0 }),
        };
        let sigma = rnd(&mut seed);
        let cases = [
            (CompBc::TotalInlet(ti), compressible_bc::total_inlet(AIR, &ti, &int, n)),
            (
                CompBc::PressureOutlet { p: 0.9 },
                compressible_bc::pressure_outlet(AIR, 0.9, 1.0, &int, n),
            ),
            (
                CompBc::NonReflecting { p: 0.9, sigma },
                compressible_bc::pressure_outlet(AIR, 0.9, sigma, &int, n),
            ),
            (CompBc::FarField(fs), compressible_bc::far_field(AIR, &fs, &int, n)),
        ];
        for (bc, want) in cases {
            let got = characteristic_face_state(AIR, &bc, &int, n, 0.0).unwrap();
            assert_eq!(bits(&got), bits(&want), "T4a state {i} {bc:?}");
        }
        for bc in [CompBc::Slip, CompBc::Extrapolate, CompBc::Fixed(fs)] {
            assert!(characteristic_face_state(AIR, &bc, &int, n, 0.0).is_none());
            assert!(characteristic_face_state(AIR, &bc, &int, n, 0.3).is_none());
        }
    }
    println!("T4a: 20 states x 4 conditions bit for bit at wn = 0; slip, extrapolate and fixed give None");
}

/// T4b of ADR 0207: the switch of the pressure outlet reads the normal velocity relative to the face.
#[test]
fn t4b_pressure_outlet_switch_in_the_frame_of_the_face() {
    let a = GAMMA.sqrt();
    let n = [1.0, 0.0, 0.0];
    let bc = CompBc::PressureOutlet { p: 0.5 };
    let sub = Prim {
        rho: 1.0,
        u: [0.5 * a, 0.2, 0.0],
        p: 1.0,
    };
    let s0 = characteristic_face_state(AIR, &bc, &sub, n, 0.0).unwrap();
    let s1 = characteristic_face_state(AIR, &bc, &sub, n, -0.6 * a).unwrap();
    let sup = Prim {
        rho: 1.0,
        u: [1.2 * a, 0.2, 0.0],
        p: 1.0,
    };
    let s2 = characteristic_face_state(AIR, &bc, &sup, n, 0.5 * a).unwrap();
    let e0 = rel(s0.p, 0.5);
    let e1 = [
        rel(s1.rho, sub.rho),
        (s1.u[0] - sub.u[0]).abs(),
        (s1.u[1] - sub.u[1]).abs(),
        (s1.u[2] - sub.u[2]).abs(),
        rel(s1.p, sub.p),
    ];
    let e2 = rel(s2.p, 0.5);
    println!("T4b: wn = 0 face p {} ({e0:.1e}); wn = -0.6 a the interior state ({e1:?}); supersonic interior, wn = +0.5 a face p {} ({e2:.1e})", s0.p, s2.p);
    assert!(e0 <= T4_TOL, "T4b static {e0}");
    assert!(
        e1.iter().all(|&e| e <= T4_TOL),
        "T4b relative supersonic outflow {e1:?}"
    );
    assert!(e2 <= T4_TOL, "T4b relative subsonic {e2}");
}

/// T4c of ADR 0207: the far field seen from a receding face takes the free stream's entropy and tangential velocity.
#[test]
fn t4c_far_field_in_the_frame_of_the_face() {
    let a = GAMMA.sqrt();
    let n = [1.0, 0.0, 0.0];
    let fs = Prim {
        rho: 1.0,
        u: [0.0; 3],
        p: 1.0,
    };
    let int = Prim {
        rho: 1.1,
        u: [0.3 * a, 0.4, 0.0],
        p: 1.0,
    };
    let q = characteristic_face_state(AIR, &CompBc::FarField(fs), &int, n, 0.5 * a).unwrap();
    let tangential = q.u[1].abs().max(q.u[2].abs());
    let entropy = rel(q.p / q.rho.powf(GAMMA), fs.p / fs.rho.powf(GAMMA));
    let lab = compressible_bc::far_field(AIR, &fs, &int, n);
    println!(
        "T4c: face state {q:?}: tangential {tangential:.1e} (limit {T4_TOL:e}), entropy {entropy:.1e} (limit {T4_ENTROPY:e}); the lab-frame state for comparison {lab:?}"
    );
    assert!(tangential <= T4_TOL, "T4c tangential {tangential}");
    assert!(entropy <= T4_ENTROPY, "T4c entropy {entropy}");
}

/// T4d of ADR 0207: the total inlet's lab-frame data give the static face state on a moving face.
#[test]
fn t4d_total_inlet_face_state_does_not_depend_on_the_mesh_speed() {
    let n = [1.0, 0.0, 0.0];
    let bc = CompBc::TotalInlet(TotalInlet {
        p0: 1.2,
        t0: 1.05,
        r_gas: 1.0,
        dir: [-1.0, 0.0, 0.0],
    });
    let int = Prim {
        rho: 1.0,
        u: [-0.2, 0.05, 0.0],
        p: 1.0,
    };
    let s0 = characteristic_face_state(AIR, &bc, &int, n, 0.0).unwrap();
    for wn in [-0.3, 0.2] {
        let s = characteristic_face_state(AIR, &bc, &int, n, wn).unwrap();
        assert_eq!(bits(&s), bits(&s0), "T4d wn {wn}");
    }
    println!("T4d: face state {s0:?} for wn 0, -0.3 and 0.2, bit for bit");
}
