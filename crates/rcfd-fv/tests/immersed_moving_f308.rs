//! F-308 (ADR 0631): moving immersed shapes. An in-line oscillating cylinder (KC 5, Re 100) in fluid at rest inside a fixed
//! circular wall, as a moving immersed body on a Cartesian mesh against the body-fitted moving-mesh run of this repository.
//!
//! The limits, set-ups, measures and mutations were registered in ADR 0631 before this file existed. The fast tests T1 to T7 run
//! by default; the heavy record `f308_oscillating_cylinder_record` (five runs on five threads, about 20 minutes in release) is
//! ignored and was run once.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::ale::{move_mesh, set_follow_conditions};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::diffusion::uniform_gamma;
use rcfd_fv::gradient::least_squares;
use rcfd_fv::immersed::{velocity_rate, ImmersedBody, ImmersedForcing, ImmersedSettings, SolidShape};
use rcfd_fv::immersed_motion::{translate_shape, HarmonicTranslation, MovingImmersed};
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::sixdof::patch_force_moment_wall_gradient;
use rcfd_fv::{FixedValue, FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::{box_2d, box_2d_coords, cylinder_ogrid, Grading};
use std::f64::consts::{FRAC_PI_2, PI, TAU};
use std::sync::Arc;
use std::time::Instant;

// ---------------------------------------------------------------------------------------------------------------------------
// Fast tests T1 to T7
// ---------------------------------------------------------------------------------------------------------------------------

fn fnv(words: impl Iterator<Item = u64>) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for w in words {
        for b in w.to_le_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}

fn bits(s: &PimpleSolver) -> u64 {
    fnv(s
        .u
        .values
        .iter()
        .flat_map(|v| v.iter().map(|x| x.to_bits()))
        .chain(s.p.values.iter().map(|x| x.to_bits()))
        .chain(s.phi.iter().map(|x| x.to_bits())))
}

const SMALL_NU: f64 = 0.01;
const SMALL_K: f64 = 1e6;

fn small_disc() -> ImmersedBody {
    ImmersedBody::fixed(
        "disc",
        SolidShape::Cylinder {
            centre: [1.5, 1.5, 0.0],
            axis: [0.0, 0.0, 1.0],
            radius: 0.25,
        },
    )
}

fn small_motion() -> HarmonicTranslation {
    HarmonicTranslation {
        amplitude: [0.2, 0.0, 0.0],
        omega: TAU,
        phase: -FRAC_PI_2,
    }
}

/// A 48 x 48 box of side 3, no-slip walls; `lid` sets the top wall moving (flow without a body motion).
fn small_solver(lid: bool) -> PimpleSolver {
    let geom = Arc::new(FvGeometry::new(Arc::new(box_2d(48, 48, 3.0, 3.0, false, false).unwrap())).unwrap());
    let mut u = VectorField::uniform("U", &geom, vec3::ZERO);
    u.set_bc(
        &geom,
        "ymax",
        Box::new(FixedValue::vector([if lid { 1.0 } else { 0.0 }, 0.0, 0.0])),
    )
    .unwrap();
    for w in ["xmin", "xmax", "ymin"] {
        u.set_bc(&geom, w, Box::new(FixedValue::vector(vec3::ZERO))).unwrap();
    }
    let p = ScalarField::uniform("p", &geom, 0.0);
    PimpleSolver::new(geom, u, p, PimpleSettings::piso(SMALL_NU))
}

fn small_moving(s: &PimpleSolver, motion: Option<HarmonicTranslation>) -> MovingImmersed {
    MovingImmersed::new(
        vec![small_disc()],
        vec![motion],
        ImmersedSettings::new(SMALL_K),
        uniform_gamma(&s.geom, SMALL_NU),
    )
    .unwrap()
}

/// ADR 0631 T1.
#[test]
fn t1_harmonic_translation_closed_forms() {
    let m = HarmonicTranslation {
        amplitude: [0.3, -0.2, 0.1],
        omega: 2.7,
        phase: 0.4,
    };
    let scale_d = 0.3;
    let scale_v = 0.3 * 2.7;
    let scale_a = 0.3 * 2.7 * 2.7;
    let (mut wd, mut wv, mut wa, mut fv, mut fa) = (0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for t in [0.0, 0.13, 0.5, 1.0, 2.2, 3.7, 10.1] {
        let x = 2.7 * t + 0.4;
        for k in 0..3 {
            let a = m.amplitude[k];
            wd = wd.max((m.displacement(t)[k] - a * (x - FRAC_PI_2).cos()).abs() / scale_d);
            wv = wv.max((m.velocity(t)[k] - a * 2.7 * (x + FRAC_PI_2).sin()).abs() / scale_v);
            wa = wa.max((m.acceleration(t)[k] - a * 2.7 * 2.7 * (x + PI).sin()).abs() / scale_a);
        }
        let h = 1e-5;
        let dv = vec3::scale(vec3::sub(m.displacement(t + h), m.displacement(t - h)), 0.5 / h);
        let da = vec3::scale(vec3::sub(m.velocity(t + h), m.velocity(t - h)), 0.5 / h);
        for k in 0..3 {
            fv = fv.max((dv[k] - m.velocity(t)[k]).abs() / scale_v);
            fa = fa.max((da[k] - m.acceleration(t)[k]).abs() / scale_a);
        }
    }
    println!("T1: closed forms {wd:.2e} {wv:.2e} {wa:.2e} (limit 1e-14), differences {fv:.2e} {fa:.2e} (limit 1e-8)");
    assert!(
        wd <= 1e-14 && wv <= 1e-14 && wa <= 1e-14,
        "T1 closed forms {wd} {wv} {wa}"
    );
    assert!(fv <= 1e-8 && fa <= 1e-8, "T1 differences {fv} {fa}");
}

/// ADR 0631 T2.
#[test]
fn t2_bodies_at_translate_the_shape_the_origin_and_the_velocity() {
    let m = HarmonicTranslation {
        amplitude: [0.1, 0.05, -0.02],
        omega: 3.0,
        phase: 0.25,
    };
    let mut spin = ImmersedBody::fixed(
        "spin",
        SolidShape::Cylinder {
            centre: [0.4, 0.6, 0.0],
            axis: [0.0, 0.0, 1.0],
            radius: 0.1,
        },
    );
    spin.velocity = [0.3, -0.1, 0.0];
    spin.omega = [0.0, 0.0, 2.0];
    spin.origin = [0.4, 0.6, 0.0];
    let ball = ImmersedBody::fixed(
        "ball",
        SolidShape::Sphere {
            centre: [1.0, 1.0, 1.0],
            radius: 0.2,
        },
    );
    let brick = ImmersedBody::fixed(
        "brick",
        SolidShape::Cuboid {
            lo: [0.0, 0.0, 0.0],
            hi: [0.1, 0.2, 0.3],
        },
    );
    let fixed = ImmersedBody::fixed(
        "fixed",
        SolidShape::Sphere {
            centre: [5.0, 5.0, 5.0],
            radius: 0.5,
        },
    );
    let bodies = vec![spin.clone(), ball.clone(), brick.clone(), fixed.clone()];
    let mut mv = MovingImmersed::new(
        bodies,
        vec![Some(m), Some(m), Some(m), None],
        ImmersedSettings::new(1e6),
        Vec::new(),
    )
    .unwrap();
    for t in [0.0, 0.7, 1.9] {
        let d = m.displacement(t);
        let v = m.velocity(t);
        let at = mv.bodies_at(t);
        assert_eq!(at[0].shape, translate_shape(spin.shape, d));
        match at[0].shape {
            SolidShape::Cylinder { centre, .. } => assert_eq!(centre, vec3::add([0.4, 0.6, 0.0], d)),
            _ => unreachable!(),
        }
        assert_eq!(at[0].origin, vec3::add(spin.origin, d), "T2 origin");
        assert_eq!(at[0].velocity, vec3::add(spin.velocity, v), "T2 velocity");
        assert_eq!(at[0].omega, spin.omega);
        // the rigid velocity at the moved centre is the translation
        assert_eq!(
            at[0].velocity_at(vec3::add(spin.origin, d)),
            vec3::add(spin.velocity, v)
        );
        match at[1].shape {
            SolidShape::Sphere { centre, radius } => {
                assert_eq!(centre, vec3::add([1.0, 1.0, 1.0], d));
                assert_eq!(radius, 0.2);
            }
            _ => unreachable!(),
        }
        match at[2].shape {
            SolidShape::Cuboid { lo, hi } => {
                assert_eq!(lo, vec3::add([0.0, 0.0, 0.0], d));
                assert_eq!(hi, vec3::add([0.1, 0.2, 0.3], d));
            }
            _ => unreachable!(),
        }
        assert_eq!(at[1].velocity, v);
        assert_eq!(at[3], fixed, "T2 fixed body changed");
    }
    mv.velocity_in_forcing = false;
    let at = mv.bodies_at(0.7);
    assert_eq!(at[0].velocity, spin.velocity, "T2 velocity left out");
    assert_eq!(at[1].velocity, vec3::ZERO);
    assert_eq!(at[0].origin, vec3::add(spin.origin, m.displacement(0.7)));
    println!("T2: shapes, origins and velocities translated exactly; fixed body unchanged");
}

/// ADR 0631 T3.
#[test]
fn t3_without_motion_the_default_bits() {
    let dt = 0.02;
    for motion in [
        None,
        Some(HarmonicTranslation {
            amplitude: vec3::ZERO,
            omega: TAU,
            phase: 0.3,
        }),
    ] {
        let mut a = small_solver(true);
        let mut b = small_solver(true);
        let mv = small_moving(&b, motion);
        let built = ImmersedForcing::build(
            &a.geom,
            &[small_disc()],
            ImmersedSettings::new(SMALL_K),
            &uniform_gamma(&a.geom, SMALL_NU),
        )
        .unwrap();
        assert_eq!(mv.forcing_at(&b.geom, 0.37).unwrap(), built, "T3 forcing");
        a.immersed = Some(built);
        for _ in 0..5 {
            a.step(dt);
            mv.step(&mut b, dt).unwrap();
        }
        assert!(a.diverged.is_none() && b.diverged.is_none());
        let (ha, hb) = (bits(&a), bits(&b));
        println!("T3 ({motion:?}): {ha:016x} {hb:016x}");
        assert_eq!(ha, hb, "T3 bits differ");
    }
}

/// ADR 0631 T4.
#[test]
fn t4_each_step_uses_the_pose_of_the_new_time_level() {
    let dt = 0.02;
    let mut s = small_solver(false);
    let mv = small_moving(&s, Some(small_motion()));
    mv.step(&mut s, dt).unwrap();
    let first = s.immersed.as_ref().unwrap().bodies[0].solid_cells.clone();
    assert_eq!(s.immersed.as_ref().unwrap(), &mv.forcing_at(&s.geom, s.time).unwrap());
    for _ in 1..5 {
        mv.step(&mut s, dt).unwrap();
    }
    assert!((s.time - 5.0 * dt).abs() <= 1e-15, "T4 time {}", s.time);
    assert_eq!(
        s.immersed.as_ref().unwrap(),
        &mv.forcing_at(&s.geom, s.time).unwrap(),
        "T4 forcing not at the new time level"
    );
    let last = &s.immersed.as_ref().unwrap().bodies[0].solid_cells;
    println!(
        "T4: solid cells at step 1 {}, at step 5 {}, time {}",
        first.len(),
        last.len(),
        s.time
    );
    assert_ne!(&first, last, "T4 the solid did not move");
}

/// ADR 0631 T5.
#[test]
fn t5_solid_cells_hold_the_rigid_velocity_and_the_negative_control_does_not() {
    let dt = 0.02;
    let m = small_motion();
    let um = 0.2 * TAU;
    for with_velocity in [true, false] {
        let mut s = small_solver(false);
        let mut mv = small_moving(&s, Some(m));
        mv.velocity_in_forcing = with_velocity;
        let mut worst = 0.0f64;
        let mut speed = 0.0f64;
        for _ in 0..5 {
            mv.step(&mut s, dt).unwrap();
            let ib = s.immersed.as_ref().unwrap();
            let ub = if with_velocity { m.velocity(s.time) } else { vec3::ZERO };
            for t in &ib.bodies[0].terms {
                if t.face.is_none() && !with_velocity {
                    assert_eq!(t.source, vec3::ZERO, "T5 source of the negative control");
                }
            }
            for &c in &ib.bodies[0].solid_cells {
                worst = worst.max(vec3::norm(vec3::sub(s.u.values[c], ub)) / um);
            }
            speed = speed.max(vec3::norm(ub));
        }
        println!("T5 (velocity in forcing {with_velocity}): max |u_c - u_b| / U_m {worst:.2e} (limit 1e-3), |u_b| {speed:.3}");
        assert!(worst <= 1e-3, "T5 {with_velocity}: {worst}");
    }
}

/// ADR 0631 T6.
#[test]
fn t6_errors_are_located() {
    let s = small_solver(false);
    let g = uniform_gamma(&s.geom, SMALL_NU);
    let e = MovingImmersed::new(vec![small_disc()], vec![], ImmersedSettings::new(1e6), g.clone()).unwrap_err();
    assert!(e.contains("ADR 0631") && e.contains("0 motions for 1 bodies"), "{e}");
    for bad in [
        HarmonicTranslation {
            amplitude: [f64::NAN, 0.0, 0.0],
            omega: 1.0,
            phase: 0.0,
        },
        HarmonicTranslation {
            amplitude: [0.1, 0.0, 0.0],
            omega: f64::INFINITY,
            phase: 0.0,
        },
        HarmonicTranslation {
            amplitude: [0.1, 0.0, 0.0],
            omega: 1.0,
            phase: f64::NAN,
        },
    ] {
        let e = MovingImmersed::new(
            vec![small_disc()],
            vec![Some(bad)],
            ImmersedSettings::new(1e6),
            g.clone(),
        )
        .unwrap_err();
        assert!(
            e.contains("ADR 0631") && e.contains("'disc'") && e.contains("finite"),
            "{e}"
        );
    }
    let mv = MovingImmersed::new(
        vec![small_disc()],
        vec![Some(small_motion())],
        ImmersedSettings::new(1e6),
        vec![0.0; 3],
    )
    .unwrap();
    let e = mv.forcing_at(&s.geom, 0.1).unwrap_err();
    assert!(e.contains("ADR 0631") && e.contains("gamma"), "{e}");
    println!("T6: {e}");
}

/// ADR 0631 T7.
#[test]
fn t7_oscillating_disc_smoke() {
    let dt = 0.02;
    let mut s = small_solver(false);
    let mv = small_moving(&s, Some(small_motion()));
    let mut fmax = 0.0f64;
    let mut cont = 0.0f64;
    for _ in 0..50 {
        let u_old = s.u.values.clone();
        let info = mv.step(&mut s, dt).unwrap();
        assert!(s.diverged.is_none(), "T7 stopped: {:?}", s.diverged);
        cont = cont.max(info.continuity);
        let ib = s.immersed.as_ref().unwrap();
        let dudt = velocity_rate(&s.u.values, &u_old, None, dt);
        let (f, _) = ib.force_moment_unsteady(0, &s.u.values, &dudt, 1.0, vec3::ZERO);
        assert!(f.iter().all(|x| x.is_finite()), "T7 force");
        fmax = fmax.max(f[0].abs());
    }
    assert!(s.u.values.iter().flatten().all(|x| x.is_finite()));
    println!("T7: one period, max |F_x| {fmax:.4}, max continuity {cont:.2e}");
}

// ---------------------------------------------------------------------------------------------------------------------------
// The heavy record (ADR 0631 A1 to A7, R1, C1, N1, S1)
// ---------------------------------------------------------------------------------------------------------------------------

const NU: f64 = 0.01;
const RADIUS: f64 = 0.5;
const R_WALL: f64 = 8.0;
const U_M: f64 = 1.0;
const PERIOD: f64 = 5.0;
const STEPS_PER_PERIOD: usize = 1000;
const K_IB: f64 = 1e6;

fn omega() -> f64 {
    TAU / PERIOD
}

fn amp() -> f64 {
    U_M / omega()
}

fn motion() -> HarmonicTranslation {
    HarmonicTranslation {
        amplitude: [amp(), 0.0, 0.0],
        omega: omega(),
        phase: -FRAC_PI_2,
    }
}

fn dt() -> f64 {
    PERIOD / STEPS_PER_PERIOD as f64
}

fn heavy_settings() -> PimpleSettings {
    let mut s = PimpleSettings::piso(NU);
    s.convection = ConvScheme::Central;
    s.amg_pressure = true;
    s
}

/// Coordinates from `-edge` to `edge`: uniform cells of size `h` on `[-core, core]`, then cells growing by `growth` to the edge,
/// the stretched sizes scaled so that the last face lands on the edge (ADR 0631 arithmetic 3).
fn stretched_axis(h: f64, core: f64, edge: f64, growth: f64) -> Vec<f64> {
    let n_core = (2.0 * core / h).round() as usize;
    let mut sizes = Vec::new();
    let (mut s, mut sum) = (h, 0.0);
    while sum < edge - core {
        s *= growth;
        sizes.push(s);
        sum += s;
    }
    let f = (edge - core) / sum;
    let mut right = vec![core];
    for s in &sizes {
        let x = right.last().unwrap() + s * f;
        right.push(x);
    }
    *right.last_mut().unwrap() = edge;
    let mut xs: Vec<f64> = right.iter().rev().map(|x| -x).collect();
    for i in 1..n_core {
        xs.push(-core + i as f64 * h);
    }
    xs.extend(right);
    xs
}

/// The registered wake points of a phase with the body centre at `xc` (ADR 0631 D4).
fn wake_points(xc: f64) -> Vec<Vec3> {
    let mut out = Vec::new();
    for xs in [-0.6, 0.0, 0.6, 1.2] {
        for j in 0..=30 {
            let y = 0.05 * j as f64;
            if ((xs - xc) * (xs - xc) + y * y).sqrt() >= RADIUS + 0.05 {
                out.push([xs, y, 0.5]);
            }
        }
    }
    out
}

/// The velocity at `pts`: nearest cell centre plus its least-squares gradient times the offset.
fn sample(geom: &FvGeometry, u: &VectorField, pts: &[Vec3]) -> Vec<Vec3> {
    let cc = geom.mesh.cell_centres();
    let bv = u.boundary_values(geom);
    let grads: Vec<Vec<Vec3>> = (0..2)
        .map(|k| {
            let cells: Vec<f64> = u.values.iter().map(|v| v[k]).collect();
            let b: Vec<f64> = bv.iter().map(|v| v[k]).collect();
            least_squares(geom, &cells, &b)
        })
        .collect();
    pts.iter()
        .map(|&x| {
            let mut best = (0usize, f64::INFINITY);
            for (c, y) in cc.iter().enumerate() {
                let d = (x[0] - y[0]).powi(2) + (x[1] - y[1]).powi(2);
                if d < best.1 {
                    best = (c, d);
                }
            }
            let c = best.0;
            let off = [x[0] - cc[c][0], x[1] - cc[c][1], 0.0];
            [
                u.values[c][0] + vec3::dot(grads[0][c], off),
                u.values[c][1] + vec3::dot(grads[1][c], off),
                0.0,
            ]
        })
        .collect()
}

/// Step numbers (1-based) of the wake samples: phases pi/2, 3 pi/4, pi of each period.
fn sample_steps(periods: usize) -> Vec<usize> {
    let mut v = Vec::new();
    for p in 0..periods {
        for q in [250, 375, 500] {
            v.push(p * STEPS_PER_PERIOD + q);
        }
    }
    v
}

#[derive(Debug, Clone)]
struct Run {
    label: String,
    /// Force on the cylinder after every step.
    force: Vec<Vec3>,
    /// Wake samples at `sample_steps`, by step.
    wake: Vec<(usize, Vec<Vec3>)>,
    max_continuity: f64,
    stopped: Option<String>,
    seconds: f64,
    cells: usize,
}

fn xc_at(t: f64) -> f64 {
    motion().displacement(t)[0]
}

fn run_ib(label: &str, h: f64, periods: usize, velocity_in_forcing: bool) -> Run {
    let t0 = Instant::now();
    let xs = stretched_axis(h, 2.0, 8.4, 1.08);
    let mesh = box_2d_coords(&xs, &xs, 1.0).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::uniform("U", &geom, vec3::ZERO);
    for w in ["xmin", "xmax", "ymin", "ymax"] {
        u.set_bc(&geom, w, Box::new(FixedValue::vector(vec3::ZERO))).unwrap();
    }
    let p = ScalarField::uniform("p", &geom, 0.0);
    let cyl = ImmersedBody::fixed(
        "cylinder",
        SolidShape::Cylinder {
            centre: vec3::ZERO,
            axis: [0.0, 0.0, 1.0],
            radius: RADIUS,
        },
    );
    let mut wall = ImmersedBody::fixed(
        "wall",
        SolidShape::Cylinder {
            centre: vec3::ZERO,
            axis: [0.0, 0.0, 1.0],
            radius: R_WALL,
        },
    );
    wall.inverted = true;
    let mut mv = MovingImmersed::new(
        vec![cyl, wall],
        vec![Some(motion()), None],
        ImmersedSettings::new(K_IB),
        uniform_gamma(&geom, NU),
    )
    .unwrap();
    mv.velocity_in_forcing = velocity_in_forcing;
    let cells = geom.n_cells();
    let mut s = PimpleSolver::new(geom, u, p, heavy_settings());
    let steps = periods * STEPS_PER_PERIOD;
    let samples = sample_steps(periods);
    let mut out = Run {
        label: label.into(),
        force: Vec::new(),
        wake: Vec::new(),
        max_continuity: 0.0,
        stopped: None,
        seconds: 0.0,
        cells,
    };
    let dt = dt();
    for n in 1..=steps {
        let u_old = s.u.values.clone();
        let info = match mv.step(&mut s, dt) {
            Ok(i) => i,
            Err(e) => {
                out.stopped = Some(format!("step {n}: {e}"));
                break;
            }
        };
        if let Some(d) = &s.diverged {
            out.stopped = Some(format!("step {n}: {}", d.message(None)));
            break;
        }
        out.max_continuity = out.max_continuity.max(info.continuity);
        let t = n as f64 * dt;
        let ib = s.immersed.as_ref().unwrap();
        let dudt = velocity_rate(&s.u.values, &u_old, None, dt);
        let (f, _) = ib.force_moment_unsteady(0, &s.u.values, &dudt, 1.0, [xc_at(t), 0.0, 0.0]);
        out.force.push(f);
        if samples.contains(&n) {
            out.wake.push((n, sample(&s.geom, &s.u, &wake_points(xc_at(t)))));
        }
        if n % 500 == 0 {
            println!(
                "[{label}] step {n}/{steps}, {:.0} s, F_x {:.5}, continuity {:.2e}",
                t0.elapsed().as_secs_f64(),
                f[0],
                info.continuity
            );
        }
    }
    if !s.u.values.iter().flatten().all(|x| x.is_finite()) && out.stopped.is_none() {
        out.stopped = Some("non-finite velocity".into());
    }
    out.seconds = t0.elapsed().as_secs_f64();
    out
}

/// The blend of the body-fitted mesh motion (ADR 0631 arithmetic 5).
fn blend(rho: f64) -> f64 {
    if rho <= 1.5 {
        1.0
    } else if rho >= 6.0 {
        0.0
    } else {
        0.5 * (1.0 + (PI * (rho - 1.5) / 4.5).cos())
    }
}

fn run_bf(label: &str, n_theta: usize, n_r: usize, periods: usize) -> Run {
    let t0 = Instant::now();
    let mesh0 = cylinder_ogrid(n_theta, n_r, RADIUS, R_WALL, Grading::Expansion(35.0), 1.0).unwrap();
    let x0: Vec<Vec3> = mesh0.points().to_vec();
    let w: Vec<f64> = x0.iter().map(|p| blend((p[0] * p[0] + p[1] * p[1]).sqrt())).collect();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh0)).unwrap());
    let cells = geom.n_cells();
    let mut u = VectorField::uniform("U", &geom, vec3::ZERO);
    for name in ["cylinder", "farfield"] {
        u.set_bc(&geom, name, Box::new(FixedValue::vector(vec3::ZERO))).unwrap();
    }
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut s = PimpleSolver::new(geom, u, p, heavy_settings());
    let names = vec!["cylinder".to_string()];
    let steps = periods * STEPS_PER_PERIOD;
    let samples = sample_steps(periods);
    let mut out = Run {
        label: label.into(),
        force: Vec::new(),
        wake: Vec::new(),
        max_continuity: 0.0,
        stopped: None,
        seconds: 0.0,
        cells,
    };
    let dt = dt();
    for n in 1..=steps {
        let t = n as f64 * dt;
        let xc = xc_at(t);
        let pts: Vec<Vec3> = x0.iter().zip(&w).map(|(p, w)| [p[0] + xc * w, p[1], p[2]]).collect();
        let st = match move_mesh(&s.geom, pts, dt) {
            Ok(st) => st,
            Err(e) => {
                out.stopped = Some(format!("step {n}: {e}"));
                break;
            }
        };
        let old = s.geom.mesh.clone();
        let walls = set_follow_conditions(&mut s.u, &st, &old, &names, dt).unwrap();
        let info = match s.step_ale_with_walls(dt, st.geom, st.mesh_flux, walls.clone()) {
            Ok(i) => i,
            Err(e) => {
                out.stopped = Some(format!("step {n}: {e}"));
                break;
            }
        };
        if let Some(d) = &s.diverged {
            out.stopped = Some(format!("step {n}: {}", d.message(None)));
            break;
        }
        out.max_continuity = out.max_continuity.max(info.continuity);
        let (f, _) =
            patch_force_moment_wall_gradient(&s.geom, &s.u, &s.p, 1.0, NU, &names, [xc, 0.0, 0.0], &walls).unwrap();
        out.force.push(f);
        if samples.contains(&n) {
            out.wake.push((n, sample(&s.geom, &s.u, &wake_points(xc))));
        }
        if n % 500 == 0 {
            println!(
                "[{label}] step {n}/{steps}, {:.0} s, F_x {:.5}, continuity {:.2e}",
                t0.elapsed().as_secs_f64(),
                f[0],
                info.continuity
            );
        }
    }
    if !s.u.values.iter().flatten().all(|x| x.is_finite()) && out.stopped.is_none() {
        out.stopped = Some("non-finite velocity".into());
    }
    out.seconds = t0.elapsed().as_secs_f64();
    out
}

/// `run_bf` with the solver built on the grid deformed to the initial pose `x0 + x_c(0) w` (ADR 0631 amendment 2).
fn run_bf_corrected(label: &str, n_theta: usize, n_r: usize, periods: usize) -> Run {
    let t0 = Instant::now();
    let mesh0 = cylinder_ogrid(n_theta, n_r, RADIUS, R_WALL, Grading::Expansion(35.0), 1.0).unwrap();
    let x0: Vec<Vec3> = mesh0.points().to_vec();
    let w: Vec<f64> = x0.iter().map(|p| blend((p[0] * p[0] + p[1] * p[1]).sqrt())).collect();
    let start: Vec<Vec3> = x0
        .iter()
        .zip(&w)
        .map(|(p, w)| [p[0] + xc_at(0.0) * w, p[1], p[2]])
        .collect();
    let mesh_start = rcfd_mesh::motion::moved_mesh(&mesh0, start).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh_start)).unwrap());
    let cells = geom.n_cells();
    let mut u = VectorField::uniform("U", &geom, vec3::ZERO);
    for name in ["cylinder", "farfield"] {
        u.set_bc(&geom, name, Box::new(FixedValue::vector(vec3::ZERO))).unwrap();
    }
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut s = PimpleSolver::new(geom, u, p, heavy_settings());
    let names = vec!["cylinder".to_string()];
    let steps = periods * STEPS_PER_PERIOD;
    let samples = sample_steps(periods);
    let mut out = Run {
        label: label.into(),
        force: Vec::new(),
        wake: Vec::new(),
        max_continuity: 0.0,
        stopped: None,
        seconds: 0.0,
        cells,
    };
    let dt = dt();
    for n in 1..=steps {
        let t = n as f64 * dt;
        let xc = xc_at(t);
        let pts: Vec<Vec3> = x0.iter().zip(&w).map(|(p, w)| [p[0] + xc * w, p[1], p[2]]).collect();
        let st = match move_mesh(&s.geom, pts, dt) {
            Ok(st) => st,
            Err(e) => {
                out.stopped = Some(format!("step {n}: {e}"));
                break;
            }
        };
        let old = s.geom.mesh.clone();
        let walls = set_follow_conditions(&mut s.u, &st, &old, &names, dt).unwrap();
        let info = match s.step_ale_with_walls(dt, st.geom, st.mesh_flux, walls.clone()) {
            Ok(i) => i,
            Err(e) => {
                out.stopped = Some(format!("step {n}: {e}"));
                break;
            }
        };
        if let Some(d) = &s.diverged {
            out.stopped = Some(format!("step {n}: {}", d.message(None)));
            break;
        }
        out.max_continuity = out.max_continuity.max(info.continuity);
        let (f, _) =
            patch_force_moment_wall_gradient(&s.geom, &s.u, &s.p, 1.0, NU, &names, [xc, 0.0, 0.0], &walls).unwrap();
        out.force.push(f);
        if samples.contains(&n) {
            out.wake.push((n, sample(&s.geom, &s.u, &wake_points(xc))));
        }
        if n % 500 == 0 {
            println!(
                "[{label}] step {n}/{steps}, {:.0} s, F_x {:.5}, continuity {:.2e}",
                t0.elapsed().as_secs_f64(),
                f[0],
                info.continuity
            );
        }
    }
    if !s.u.values.iter().flatten().all(|x| x.is_finite()) && out.stopped.is_none() {
        out.stopped = Some("non-finite velocity".into());
    }
    out.seconds = t0.elapsed().as_secs_f64();
    out
}

/// Mean and first harmonic `(2/N) sum F exp(-i omega t)` (as (re, im)) of component `k` over steps `first..=last` (1-based).
fn harmonic(v: &[f64], first: usize, last: usize) -> (f64, (f64, f64)) {
    let n = (last - first + 1) as f64;
    let (mut mean, mut re, mut im) = (0.0, 0.0, 0.0);
    for step in first..=last {
        let f = v[step - 1];
        let wt = omega() * step as f64 * dt();
        mean += f;
        re += f * wt.cos();
        im -= f * wt.sin();
    }
    (mean / n, (2.0 * re / n, 2.0 * im / n))
}

fn comp(r: &Run, k: usize) -> Vec<f64> {
    r.force.iter().map(|f| f[k]).collect()
}

fn cabs(z: (f64, f64)) -> f64 {
    (z.0 * z.0 + z.1 * z.1).sqrt()
}

fn wrap(a: f64) -> f64 {
    let mut a = a % TAU;
    if a > PI {
        a -= TAU;
    }
    if a < -PI {
        a += TAU;
    }
    a
}

/// Wake differences over the samples at `steps`: (e_max, e_rms) over U_m.
fn wake_diff(a: &Run, b: &Run, steps: &[usize]) -> (f64, f64) {
    let (mut emax, mut sum, mut n) = (0.0f64, 0.0, 0usize);
    for &st in steps {
        let wa = &a.wake.iter().find(|w| w.0 == st).unwrap().1;
        let wb = &b.wake.iter().find(|w| w.0 == st).unwrap().1;
        assert_eq!(wa.len(), wb.len());
        for (x, y) in wa.iter().zip(wb) {
            for k in 0..2 {
                let d = (x[k] - y[k]).abs() / U_M;
                emax = emax.max(d);
                sum += d * d;
                n += 1;
            }
        }
    }
    (emax, (sum / n as f64).sqrt())
}

type Job = Box<dyn FnOnce() -> Run + Send>;

struct Harm {
    mean_x: f64,
    hx: (f64, f64),
    mean_y: f64,
    hy: (f64, f64),
}

fn harm(r: &Run, first: usize, last: usize) -> Harm {
    let (mean_x, hx) = harmonic(&comp(r, 0), first, last);
    let (mean_y, hy) = harmonic(&comp(r, 1), first, last);
    Harm { mean_x, hx, mean_y, hy }
}

/// Morison-type parts of the drag harmonic (reported only): in phase with the acceleration and with the velocity.
fn morison(h: &Harm, first: usize, last: usize) -> (f64, f64) {
    let m = motion();
    let acc: Vec<f64> = (1..=last).map(|n| m.acceleration(n as f64 * dt())[0]).collect();
    let vel: Vec<f64> = (1..=last).map(|n| m.velocity(n as f64 * dt())[0]).collect();
    let (_, ah) = harmonic(&acc, first, last);
    let (_, vh) = harmonic(&vel, first, last);
    let proj = |z: (f64, f64), w: (f64, f64)| (z.0 * w.0 + z.1 * w.1) / cabs(w);
    let ca = -proj(h.hx, ah) / (PI * RADIUS * RADIUS * cabs(ah));
    let cv = -proj(h.hx, vh) / (RADIUS * U_M * U_M);
    (ca, cv)
}

/// ADR 0631 A1 to A7, R1, C1, N1, S1: the registered heavy record (run once).
#[test]
#[ignore = "heavy record (ADR 0631): five runs, about 20 minutes in release"]
fn f308_oscillating_cylinder_record() {
    let t0 = Instant::now();
    let jobs: Vec<(&str, Job)> = vec![
        ("BF", Box::new(|| run_bf("BF", 256, 128, 4))),
        ("BFc", Box::new(|| run_bf("BFc", 128, 64, 4))),
        ("IB40", Box::new(|| run_ib("IB40", 0.025, 4, true))),
        ("IB20", Box::new(|| run_ib("IB20", 0.05, 4, true))),
        ("N1", Box::new(|| run_ib("N1", 0.025, 2, false))),
    ];
    let runs: Vec<Run> = std::thread::scope(|sc| {
        let hs: Vec<_> = jobs
            .into_iter()
            .map(|(name, job)| {
                std::thread::Builder::new()
                    .name(name.into())
                    .stack_size(64 << 20)
                    .spawn_scoped(sc, job)
                    .unwrap()
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    println!("all runs done in {:.0} s", t0.elapsed().as_secs_f64());
    let mut fails: Vec<String> = Vec::new();
    for r in &runs {
        println!(
            "S1 [{}]: {} cells, {:.0} s, max continuity {:.3e}, stopped {:?}",
            r.label, r.cells, r.seconds, r.max_continuity, r.stopped
        );
        if r.stopped.is_some() {
            fails.push(format!("S1 {} stopped: {:?}", r.label, r.stopped));
        }
    }
    if !fails.is_empty() {
        panic!("{fails:#?}");
    }
    let [bf, bfc, ib40, ib20, n1] = [&runs[0], &runs[1], &runs[2], &runs[3], &runs[4]];
    let (w0, w1) = (2 * STEPS_PER_PERIOD + 1, 4 * STEPS_PER_PERIOD);
    let hb = harm(bf, w0, w1);
    let amp_bf = cabs(hb.hx);
    for (r, h) in [
        (bf, &hb),
        (bfc, &harm(bfc, w0, w1)),
        (ib40, &harm(ib40, w0, w1)),
        (ib20, &harm(ib20, w0, w1)),
    ] {
        let (ca, cv) = morison(h, w0, w1);
        println!(
            "[{}] drag mean {:+.5e}, harmonic ({:+.6}, {:+.6}) amplitude {:.6} phase {:+.3} deg; lift mean {:+.3e} amplitude {:.3e}; \
             C_a {:.4}, C_v {:.4} (reported only)",
            r.label,
            h.mean_x,
            h.hx.0,
            h.hx.1,
            cabs(h.hx),
            h.hx.1.atan2(h.hx.0).to_degrees(),
            h.mean_y,
            cabs(h.hy),
            ca,
            cv
        );
        // periodicity, reported only
        let p3 = harm(r, w0, 3 * STEPS_PER_PERIOD);
        let p4 = harm(r, 3 * STEPS_PER_PERIOD + 1, w1);
        println!(
            "[{}] periods 3 and 4 drag amplitude {:.6} {:.6}",
            r.label,
            cabs(p3.hx),
            cabs(p4.hx)
        );
    }
    let hi = harm(ib40, w0, w1);
    let a1 = (cabs(hi.hx) - amp_bf).abs() / amp_bf;
    let a2 = wrap(hi.hx.1.atan2(hi.hx.0) - hb.hx.1.atan2(hb.hx.0)).abs().to_degrees();
    let a3 = cabs((hi.hx.0 - hb.hx.0, hi.hx.1 - hb.hx.1)) / amp_bf;
    let a4 = (hi.mean_x - hb.mean_x).abs() / amp_bf;
    let lifts = [
        hi.mean_y.abs() / amp_bf,
        cabs(hi.hy) / amp_bf,
        hb.mean_y.abs() / amp_bf,
        cabs(hb.hy) / amp_bf,
    ];
    let a5 = lifts.iter().cloned().fold(0.0, f64::max);
    let p4_steps: Vec<usize> = sample_steps(4)
        .into_iter()
        .filter(|&s| s > 3 * STEPS_PER_PERIOD)
        .collect();
    let (a6, a7) = wake_diff(ib40, bf, &p4_steps);
    let hc = harm(bfc, w0, w1);
    let r1f = cabs((hc.hx.0 - hb.hx.0, hc.hx.1 - hb.hx.1)) / amp_bf;
    let (_, r1w) = wake_diff(bfc, bf, &p4_steps);
    let h20 = harm(ib20, w0, w1);
    let c1f = cabs((h20.hx.0 - hb.hx.0, h20.hx.1 - hb.hx.1)) / amp_bf;
    let (_, c1w) = wake_diff(ib20, bf, &p4_steps);
    let (q0, q1) = (STEPS_PER_PERIOD + 1, 2 * STEPS_PER_PERIOD);
    let hb2 = harm(bf, q0, q1);
    let hn = harm(n1, q0, q1);
    let n1f = cabs((hn.hx.0 - hb2.hx.0, hn.hx.1 - hb2.hx.1)) / cabs(hb2.hx);
    let p2_steps: Vec<usize> = sample_steps(2).into_iter().filter(|&s| s > STEPS_PER_PERIOD).collect();
    let (_, n1w) = wake_diff(n1, bf, &p2_steps);
    println!("A1 drag amplitude {a1:.4e} (limit 0.05)");
    println!("A2 drag phase {a2:.4} deg (limit 3)");
    println!("A3 drag harmonic complex {a3:.4e} (limit 0.06)");
    println!("A4 mean drag {a4:.4e} (limit 0.02)");
    println!(
        "A5 lift (IB mean, IB amplitude, BF mean, BF amplitude) {:.3e} {:.3e} {:.3e} {:.3e} (limit 1e-3 each)",
        lifts[0], lifts[1], lifts[2], lifts[3]
    );
    println!("A6 wake e_max {a6:.4e} (limit 0.15)");
    println!("A7 wake e_rms {a7:.4e} (limit 0.05)");
    println!("R1 BFc against BF: drag harmonic {r1f:.4e} (limit 0.04), wake e_rms {r1w:.4e} (limit 0.025)");
    println!("C1 G20: drag harmonic {c1f:.4e} against G40 {a3:.4e}; wake e_rms {c1w:.4e} against {a7:.4e} (G20 larger: reported)");
    println!("N1 velocity left out (period 2): drag harmonic {n1f:.4e} (must exceed 0.3), wake e_rms {n1w:.4e} (must exceed 0.15)");
    let checks = [
        ("A1", a1 <= 0.05),
        ("A2", a2 <= 3.0),
        ("A3", a3 <= 0.06),
        ("A4", a4 <= 0.02),
        ("A5", a5 <= 1e-3),
        ("A6", a6 <= 0.15),
        ("A7", a7 <= 0.05),
        ("R1 force", r1f <= 0.04),
        ("R1 wake", r1w <= 0.025),
        ("N1 force", n1f >= 0.3),
        ("N1 wake", n1w >= 0.15),
    ];
    println!("C1 met: {}", c1f > a3 && c1w > a7);
    for (id, ok) in checks {
        println!("{id}: {}", if ok { "met" } else { "MISSED" });
        if !ok {
            fails.push(id.to_string());
        }
    }
    assert!(fails.is_empty(), "missed: {fails:?}");
}

/// ADR 0631 A1 to A7, R1, C1, N1, S1 with the body-fitted start of amendment 2: the corrected heavy record (run once).
#[test]
#[ignore = "heavy record (ADR 0631 amendment 2): five runs, about 25 minutes in release"]
fn f308_oscillating_cylinder_record_corrected() {
    let t0 = Instant::now();
    let jobs: Vec<(&str, Job)> = vec![
        ("BF", Box::new(|| run_bf_corrected("BF", 256, 128, 4))),
        ("BFc", Box::new(|| run_bf_corrected("BFc", 128, 64, 4))),
        ("IB40", Box::new(|| run_ib("IB40", 0.025, 4, true))),
        ("IB20", Box::new(|| run_ib("IB20", 0.05, 4, true))),
        ("N1", Box::new(|| run_ib("N1", 0.025, 2, false))),
    ];
    let runs: Vec<Run> = std::thread::scope(|sc| {
        let hs: Vec<_> = jobs
            .into_iter()
            .map(|(name, job)| {
                std::thread::Builder::new()
                    .name(name.into())
                    .stack_size(64 << 20)
                    .spawn_scoped(sc, job)
                    .unwrap()
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    println!("all runs done in {:.0} s", t0.elapsed().as_secs_f64());
    let mut fails: Vec<String> = Vec::new();
    for r in &runs {
        println!(
            "S1 [{}]: {} cells, {:.0} s, max continuity {:.3e}, stopped {:?}",
            r.label, r.cells, r.seconds, r.max_continuity, r.stopped
        );
        println!(
            "[{}] force series hash {:016x}",
            r.label,
            fnv(r.force.iter().flat_map(|f| f.iter().map(|x| x.to_bits())))
        );
        if r.stopped.is_some() {
            fails.push(format!("S1 {} stopped: {:?}", r.label, r.stopped));
        }
    }
    if !fails.is_empty() {
        panic!("{fails:#?}");
    }
    let [bf, bfc, ib40, ib20, n1] = [&runs[0], &runs[1], &runs[2], &runs[3], &runs[4]];
    let (w0, w1) = (2 * STEPS_PER_PERIOD + 1, 4 * STEPS_PER_PERIOD);
    let hb = harm(bf, w0, w1);
    let amp_bf = cabs(hb.hx);
    for (r, h) in [
        (bf, &hb),
        (bfc, &harm(bfc, w0, w1)),
        (ib40, &harm(ib40, w0, w1)),
        (ib20, &harm(ib20, w0, w1)),
    ] {
        let (ca, cv) = morison(h, w0, w1);
        println!(
            "[{}] drag mean {:+.5e}, harmonic ({:+.6}, {:+.6}) amplitude {:.6} phase {:+.3} deg; lift mean {:+.3e} amplitude {:.3e}; \
             C_a {:.4}, C_v {:.4} (reported only)",
            r.label,
            h.mean_x,
            h.hx.0,
            h.hx.1,
            cabs(h.hx),
            h.hx.1.atan2(h.hx.0).to_degrees(),
            h.mean_y,
            cabs(h.hy),
            ca,
            cv
        );
        // periodicity, reported only
        let p3 = harm(r, w0, 3 * STEPS_PER_PERIOD);
        let p4 = harm(r, 3 * STEPS_PER_PERIOD + 1, w1);
        println!(
            "[{}] periods 3 and 4 drag amplitude {:.6} {:.6}",
            r.label,
            cabs(p3.hx),
            cabs(p4.hx)
        );
    }
    let hi = harm(ib40, w0, w1);
    let a1 = (cabs(hi.hx) - amp_bf).abs() / amp_bf;
    let a2 = wrap(hi.hx.1.atan2(hi.hx.0) - hb.hx.1.atan2(hb.hx.0)).abs().to_degrees();
    let a3 = cabs((hi.hx.0 - hb.hx.0, hi.hx.1 - hb.hx.1)) / amp_bf;
    let a4 = (hi.mean_x - hb.mean_x).abs() / amp_bf;
    let lifts = [
        hi.mean_y.abs() / amp_bf,
        cabs(hi.hy) / amp_bf,
        hb.mean_y.abs() / amp_bf,
        cabs(hb.hy) / amp_bf,
    ];
    let a5 = lifts.iter().cloned().fold(0.0, f64::max);
    let p4_steps: Vec<usize> = sample_steps(4)
        .into_iter()
        .filter(|&s| s > 3 * STEPS_PER_PERIOD)
        .collect();
    let (a6, a7) = wake_diff(ib40, bf, &p4_steps);
    let hc = harm(bfc, w0, w1);
    let r1f = cabs((hc.hx.0 - hb.hx.0, hc.hx.1 - hb.hx.1)) / amp_bf;
    let (_, r1w) = wake_diff(bfc, bf, &p4_steps);
    let h20 = harm(ib20, w0, w1);
    let c1f = cabs((h20.hx.0 - hb.hx.0, h20.hx.1 - hb.hx.1)) / amp_bf;
    let (_, c1w) = wake_diff(ib20, bf, &p4_steps);
    let (q0, q1) = (STEPS_PER_PERIOD + 1, 2 * STEPS_PER_PERIOD);
    let hb2 = harm(bf, q0, q1);
    let hn = harm(n1, q0, q1);
    let n1f = cabs((hn.hx.0 - hb2.hx.0, hn.hx.1 - hb2.hx.1)) / cabs(hb2.hx);
    let p2_steps: Vec<usize> = sample_steps(2).into_iter().filter(|&s| s > STEPS_PER_PERIOD).collect();
    let (_, n1w) = wake_diff(n1, bf, &p2_steps);
    println!("A1 drag amplitude {a1:.4e} (limit 0.05)");
    println!("A2 drag phase {a2:.4} deg (limit 3)");
    println!("A3 drag harmonic complex {a3:.4e} (limit 0.06)");
    println!("A4 mean drag {a4:.4e} (limit 0.02)");
    println!(
        "A5 lift (IB mean, IB amplitude, BF mean, BF amplitude) {:.3e} {:.3e} {:.3e} {:.3e} (limit 1e-3 each)",
        lifts[0], lifts[1], lifts[2], lifts[3]
    );
    println!("A6 wake e_max {a6:.4e} (limit 0.15)");
    println!("A7 wake e_rms {a7:.4e} (limit 0.05)");
    println!("R1 BFc against BF: drag harmonic {r1f:.4e} (limit 0.04), wake e_rms {r1w:.4e} (limit 0.025)");
    println!("C1 G20: drag harmonic {c1f:.4e} against G40 {a3:.4e}; wake e_rms {c1w:.4e} against {a7:.4e} (G20 larger: reported)");
    println!("N1 velocity left out (period 2): drag harmonic {n1f:.4e} (must exceed 0.3), wake e_rms {n1w:.4e} (must exceed 0.15)");
    let checks = [
        ("A1", a1 <= 0.05),
        ("A2", a2 <= 3.0),
        ("A3", a3 <= 0.06),
        ("A4", a4 <= 0.02),
        ("A5", a5 <= 1e-3),
        ("A6", a6 <= 0.15),
        ("A7", a7 <= 0.05),
        ("R1 force", r1f <= 0.04),
        ("R1 wake", r1w <= 0.025),
        ("N1 force", n1f >= 0.3),
        ("N1 wake", n1w >= 0.15),
    ];
    println!("C1 met: {}", c1f > a3 && c1w > a7);
    for (id, ok) in checks {
        println!("{id}: {}", if ok { "met" } else { "MISSED" });
        if !ok {
            fails.push(id.to_string());
        }
    }
    assert!(fails.is_empty(), "missed: {fails:?}");
}
