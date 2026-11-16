//! F-707 (ADR 0648): moving immersed shapes, a cylinder oscillating across a uniform stream (Re 20, A/D 0.5, f D/U 0.25) in
//! the square `[-8, 8]^2` with a far-field condition, as a moving immersed body on a Cartesian mesh (the F-308 library,
//! `immersed_motion.rs`, unchanged) against the body-fitted moving-mesh run of this repository on an O-grid mapped to the same
//! square.
//!
//! The limits, set-ups, measures and mutations were registered in ADR 0648 before this file existed. The fast tests T1 to T4 run
//! by default; the heavy record `f707_transverse_cylinder_record` (five runs on five threads, about 30 minutes in release) is
//! ignored and was run once.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::ale::{move_mesh, set_follow_conditions};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::diffusion::uniform_gamma;
use rcfd_fv::immersed::{velocity_rate, ImmersedBody, ImmersedSettings, SolidShape};
use rcfd_fv::immersed_motion::{HarmonicTranslation, MovingImmersed};
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver, StepInfo};
use rcfd_fv::sixdof::patch_force_moment_wall_gradient;
use rcfd_fv::{FixedValue, Freestream, FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::{box_2d_coords, cylinder_ogrid, warp, Grading};
use rcfd_mesh::Mesh;
use std::f64::consts::{FRAC_PI_2, PI, TAU};
use std::sync::Arc;
use std::time::Instant;

// ---------------------------------------------------------------------------------------------------------------------------
// The case (ADR 0648 arithmetic 1 to 8, D2)
// ---------------------------------------------------------------------------------------------------------------------------

const NU: f64 = 0.05;
const RADIUS: f64 = 0.5;
const EDGE: f64 = 8.0;
const U_INF: Vec3 = [1.0, 0.0, 0.0];
const AMP: f64 = 0.5;
const PERIOD: f64 = 4.0;
const STEPS_PER_PERIOD: usize = 500;
const PERIODS: usize = 10;
const K_IB: f64 = 1e6;
const GROWTH: f64 = 1.08;

fn omega() -> f64 {
    TAU / PERIOD
}

fn dt() -> f64 {
    PERIOD / STEPS_PER_PERIOD as f64
}

/// `y_c(t) = -A cos(omega t)` across the stream.
fn motion() -> HarmonicTranslation {
    HarmonicTranslation {
        amplitude: [0.0, AMP, 0.0],
        omega: omega(),
        phase: -FRAC_PI_2,
    }
}

fn yc_at(t: f64) -> f64 {
    motion().displacement(t)[1]
}

fn settings() -> PimpleSettings {
    let mut s = PimpleSettings::piso(NU);
    s.convection = ConvScheme::LinearUpwind;
    s.amg_pressure = true;
    s
}

/// The far-field condition on `patches` (ADR 0648 D2): velocity fixed to the stream on inflow faces, pressure fixed to 0 on
/// the others.
fn far_field(geom: &FvGeometry, u: &mut VectorField, p: &mut ScalarField, patches: &[&str]) {
    for name in patches {
        u.set_bc(
            geom,
            name,
            Box::new(Freestream {
                u_ref: U_INF,
                value: U_INF,
                fixed_on_inflow: true,
            }),
        )
        .unwrap();
        p.set_bc(
            geom,
            name,
            Box::new(Freestream {
                u_ref: U_INF,
                value: [0.0; 3],
                fixed_on_inflow: false,
            }),
        )
        .unwrap();
    }
}

/// Cell sizes growing by `growth` from `h`, scaled so that they sum to `len` (ADR 0631 arithmetic 3, per side).
fn side(h: f64, len: f64, growth: f64) -> Vec<f64> {
    let mut sizes = Vec::new();
    let (mut s, mut sum) = (h, 0.0);
    while sum < len {
        s *= growth;
        sizes.push(s);
        sum += s;
    }
    let f = len / sum;
    sizes.iter().map(|s| s * f).collect()
}

/// Coordinates from `-EDGE` to `EDGE`: uniform cells of size `h` on `[lo, hi]`, stretched cells outside (ADR 0648 arithmetic 4).
fn axis(h: f64, lo: f64, hi: f64) -> Vec<f64> {
    let n_core = ((hi - lo) / h).round() as usize;
    let mut left = vec![lo];
    for s in side(h, lo + EDGE, GROWTH) {
        let x = left.last().unwrap() - s;
        left.push(x);
    }
    *left.last_mut().unwrap() = -EDGE;
    let mut xs: Vec<f64> = left.into_iter().rev().collect();
    for i in 1..n_core {
        xs.push(lo + i as f64 * h);
    }
    xs.push(hi);
    for s in side(h, EDGE - hi, GROWTH) {
        let x = xs.last().unwrap() + s;
        xs.push(x);
    }
    *xs.last_mut().unwrap() = EDGE;
    xs
}

fn ib_mesh(h: f64) -> Mesh {
    box_2d_coords(&axis(h, -1.25, 2.5), &axis(h, -1.5, 1.5), 1.0).unwrap()
}

/// The square map of the O-grid (ADR 0648 arithmetic 6).
fn square_map(p: Vec3) -> Vec3 {
    let rho = (p[0] * p[0] + p[1] * p[1]).sqrt();
    if rho <= 2.0 {
        return p;
    }
    let (c, s) = (p[0] / rho, p[1] / rho);
    if rho >= EDGE - 1e-9 {
        // the outer ring, placed exactly on the square
        let corner = (c.abs() - s.abs()).abs() < 1e-12;
        if corner {
            return [EDGE * c.signum(), EDGE * s.signum(), p[2]];
        }
        if c.abs() >= s.abs() {
            return [EDGE * c.signum(), EDGE * s / c.abs(), p[2]];
        }
        return [EDGE * c / s.abs(), EDGE * s.signum(), p[2]];
    }
    let m = c.abs().max(s.abs());
    let b = ((rho - 2.0) / (EDGE - 2.0)).powi(2);
    let k = 1.0 + b * (1.0 / m - 1.0);
    [p[0] * k, p[1] * k, p[2]]
}

fn bf_base_mesh(n_theta: usize, n_r: usize) -> Mesh {
    let m = cylinder_ogrid(n_theta, n_r, RADIUS, EDGE, Grading::Expansion(12.0), 1.0).unwrap();
    warp(&m, square_map).unwrap()
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

fn posed(x0: &[Vec3], w: &[f64], yc: f64) -> Vec<Vec3> {
    x0.iter().zip(w).map(|(p, w)| [p[0], p[1] + yc * w, p[2]]).collect()
}

// ---------------------------------------------------------------------------------------------------------------------------
// The two set-ups and their steps (shared by the fast tests and the record)
// ---------------------------------------------------------------------------------------------------------------------------

struct IbCase {
    s: PimpleSolver,
    mv: MovingImmersed,
    n: usize,
}

fn ib_setup(h: f64, velocity_in_forcing: bool) -> IbCase {
    let geom = Arc::new(FvGeometry::new(Arc::new(ib_mesh(h))).unwrap());
    let mut u = VectorField::uniform("U", &geom, U_INF);
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    far_field(&geom, &mut u, &mut p, &["xmin", "xmax", "ymin", "ymax"]);
    let cyl = ImmersedBody::fixed(
        "cylinder",
        SolidShape::Cylinder {
            centre: vec3::ZERO,
            axis: [0.0, 0.0, 1.0],
            radius: RADIUS,
        },
    );
    let mut mv = MovingImmersed::new(
        vec![cyl],
        vec![Some(motion())],
        ImmersedSettings::new(K_IB),
        uniform_gamma(&geom, NU),
    )
    .unwrap();
    mv.velocity_in_forcing = velocity_in_forcing;
    IbCase {
        s: PimpleSolver::new(geom, u, p, settings()),
        mv,
        n: 0,
    }
}

/// One step; the force of ADR 0648 D3.
fn ib_step(c: &mut IbCase) -> Result<(Vec3, StepInfo), String> {
    let dt = dt();
    let u_old = c.s.u.values.clone();
    let info = c.mv.step(&mut c.s, dt)?;
    c.n += 1;
    if let Some(d) = &c.s.diverged {
        return Err(d.message(None));
    }
    let t = c.n as f64 * dt;
    let ib = c.s.immersed.as_ref().unwrap();
    let dudt = velocity_rate(&c.s.u.values, &u_old, None, dt);
    let (f, _) = ib.force_moment_unsteady(0, &c.s.u.values, &dudt, 1.0, [0.0, yc_at(t), 0.0]);
    Ok((f, info))
}

struct BfCase {
    s: PimpleSolver,
    x0: Vec<Vec3>,
    w: Vec<f64>,
    n: usize,
}

fn bf_setup(n_theta: usize, n_r: usize) -> BfCase {
    let mesh0 = bf_base_mesh(n_theta, n_r);
    let x0: Vec<Vec3> = mesh0.points().to_vec();
    let w: Vec<f64> = x0.iter().map(|p| blend((p[0] * p[0] + p[1] * p[1]).sqrt())).collect();
    // ADR 0631 amendment 2: the solver starts on the grid at the initial pose
    let start = rcfd_mesh::motion::moved_mesh(&mesh0, posed(&x0, &w, yc_at(0.0))).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(start)).unwrap());
    let mut u = VectorField::uniform("U", &geom, U_INF);
    u.set_bc(&geom, "cylinder", Box::new(FixedValue::vector(vec3::ZERO)))
        .unwrap();
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    far_field(&geom, &mut u, &mut p, &["farfield"]);
    BfCase {
        s: PimpleSolver::new(geom, u, p, settings()),
        x0,
        w,
        n: 0,
    }
}

fn bf_step(c: &mut BfCase) -> Result<(Vec3, StepInfo), String> {
    let dt = dt();
    let t = (c.n + 1) as f64 * dt;
    let yc = yc_at(t);
    let st = move_mesh(&c.s.geom, posed(&c.x0, &c.w, yc), dt)?;
    let old = c.s.geom.mesh.clone();
    let names = vec!["cylinder".to_string()];
    let walls = set_follow_conditions(&mut c.s.u, &st, &old, &names, dt)?;
    let info = c.s.step_ale_with_walls(dt, st.geom, st.mesh_flux, walls.clone())?;
    c.n += 1;
    if let Some(d) = &c.s.diverged {
        return Err(d.message(None));
    }
    let (f, _) = patch_force_moment_wall_gradient(&c.s.geom, &c.s.u, &c.s.p, 1.0, NU, &names, [0.0, yc, 0.0], &walls)?;
    Ok((f, info))
}

// ---------------------------------------------------------------------------------------------------------------------------
// Measures (ADR 0648 D4)
// ---------------------------------------------------------------------------------------------------------------------------

/// Mean and harmonic `k` (`(2/N) sum F exp(-i k omega t_n)`, as (re, im)) of `v` (one value per step, step `n` at `t_n = n dt`)
/// over steps `first..=last` (1-based).
fn harmonic(v: &[f64], first: usize, last: usize, k: f64) -> (f64, (f64, f64)) {
    let n = (last - first + 1) as f64;
    let (mut mean, mut re, mut im) = (0.0, 0.0, 0.0);
    for step in first..=last {
        let f = v[step - 1];
        let wt = k * omega() * step as f64 * dt();
        mean += f;
        re += f * wt.cos();
        im -= f * wt.sin();
    }
    (mean / n, (2.0 * re / n, 2.0 * im / n))
}

fn cabs(z: (f64, f64)) -> f64 {
    (z.0 * z.0 + z.1 * z.1).sqrt()
}

fn wrap(a: f64) -> f64 {
    let mut a = a % TAU;
    if a > PI {
        a -= TAU;
    }
    if a <= -PI {
        a += TAU;
    }
    a
}

/// The series of `f(t_n)` for steps `1..=last`.
fn series(last: usize, f: impl Fn(f64) -> f64) -> Vec<f64> {
    (1..=last).map(|n| f(n as f64 * dt())).collect()
}

/// `arg(zhat / Yhat)` in degrees, wrapped, with `Yhat` the first harmonic of the displacement over the same steps.
fn phase_to_displacement(z: (f64, f64), first: usize, last: usize) -> f64 {
    let (_, y) = harmonic(&series(last, yc_at), first, last, 1.0);
    let re = z.0 * y.0 + z.1 * y.1;
    let im = z.1 * y.0 - z.0 * y.1;
    wrap(im.atan2(re)).to_degrees()
}

#[derive(Debug, Clone, Copy)]
struct Measures {
    mean_x: f64,
    mean_y: f64,
    lift: (f64, f64),
    drag2: (f64, f64),
}

fn measures(force: &[Vec3], first: usize, last: usize) -> Measures {
    let fx: Vec<f64> = force.iter().map(|f| f[0]).collect();
    let fy: Vec<f64> = force.iter().map(|f| f[1]).collect();
    let (mean_x, _) = harmonic(&fx, first, last, 1.0);
    let (_, drag2) = harmonic(&fx, first, last, 2.0);
    let (mean_y, lift) = harmonic(&fy, first, last, 1.0);
    Measures {
        mean_x,
        mean_y,
        lift,
        drag2,
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// Fast tests T1 to T4
// ---------------------------------------------------------------------------------------------------------------------------

fn min_volume(mesh: &Mesh, pts: Vec<Vec3>) -> f64 {
    let m = rcfd_mesh::motion::moved_mesh(mesh, pts).unwrap();
    m.cell_volumes().iter().cloned().fold(f64::INFINITY, f64::min)
}

/// ADR 0648 T1 as first run: defective face selection (ADR 0648 amendment 2), kept as the record.
#[test]
#[ignore = "ADR 0648 amendment 2: the first T1, defective (faces selected by their centre)"]
fn t1_the_square_map_and_the_far_field_faces_first_run() {
    for (n_theta, n_r) in [(256, 96), (128, 48)] {
        let raw = cylinder_ogrid(n_theta, n_r, RADIUS, EDGE, Grading::Expansion(12.0), 1.0).unwrap();
        let m = bf_base_mesh(n_theta, n_r);
        for (a, b) in raw.points().iter().zip(m.points()) {
            if (a[0] * a[0] + a[1] * a[1]).sqrt() <= 2.0 {
                assert_eq!(a, b, "T1 point inside rho 2 moved");
            }
        }
        let far = m.patch("farfield").unwrap();
        let (fc, sf) = (m.face_centres(), m.face_area_vectors());
        let (mut worst, mut inflow, mut left) = (0.0f64, 0usize, 0usize);
        for f in far.start..far.start + far.size {
            worst = worst.max((fc[f][0].abs().max(fc[f][1].abs()) - EDGE).abs());
            let on_left = fc[f][0] == -EDGE;
            let on_top = fc[f][1].abs() == EDGE;
            if on_left {
                left += 1;
                assert!(
                    sf[f][1] == 0.0 && sf[f][2] == 0.0 && sf[f][0] < 0.0,
                    "T1 left face {f}: {:?}",
                    sf[f]
                );
            }
            if on_top {
                assert!(sf[f][0] == 0.0, "T1 top/bottom face {f}: {:?}", sf[f]);
            }
            if vec3::dot(sf[f], U_INF) < 0.0 {
                inflow += 1;
                assert!(on_left, "T1 inflow face {f} not on x = -8: {:?} {:?}", fc[f], sf[f]);
            }
        }
        assert!(worst <= 1e-12, "T1 far-field face off the square by {worst}");
        assert_eq!(left, n_theta / 4, "T1 faces on x = -8");
        assert_eq!(inflow, n_theta / 4, "T1 inflow faces");
        let x0: Vec<Vec3> = m.points().to_vec();
        let w: Vec<f64> = x0.iter().map(|p| blend((p[0] * p[0] + p[1] * p[1]).sqrt())).collect();
        let vols: Vec<f64> = [-AMP, 0.0, AMP]
            .iter()
            .map(|&yc| min_volume(&m, posed(&x0, &w, yc)))
            .collect();
        assert!(vols.iter().all(|&v| v > 0.0), "T1 volumes {vols:?}");
        println!(
            "T1 ({n_theta} x {n_r}): far-field faces on the square within {worst:.1e}, {left} faces on x = -8, {inflow} inflow; \
             smallest cell volume at -A, 0, A: {:.3e} {:.3e} {:.3e}",
            vols[0], vols[1], vols[2]
        );
    }
    let b = ib_mesh(0.025);
    let (fc, sf) = (b.face_centres(), b.face_area_vectors());
    let xmin = b.patch("xmin").unwrap();
    let mut inflow = 0usize;
    for name in ["xmin", "xmax", "ymin", "ymax"] {
        let p = b.patch(name).unwrap();
        for f in p.start..p.start + p.size {
            assert!((fc[f][0].abs().max(fc[f][1].abs()) - EDGE).abs() <= 1e-12);
            if vec3::dot(sf[f], U_INF) < 0.0 {
                inflow += 1;
                assert_eq!(name, "xmin", "T1 box inflow face on {name}");
            }
        }
    }
    assert_eq!(inflow, xmin.size, "T1 box inflow faces");
    println!("T1 box: {} cells, {} inflow faces, all on xmin", b.n_cells(), inflow);
}

/// ADR 0648 T1 (corrected by amendment 2).
#[test]
fn t1_the_square_map_and_the_far_field_faces() {
    for (n_theta, n_r) in [(256, 96), (128, 48)] {
        let raw = cylinder_ogrid(n_theta, n_r, RADIUS, EDGE, Grading::Expansion(12.0), 1.0).unwrap();
        let m = bf_base_mesh(n_theta, n_r);
        for (a, b) in raw.points().iter().zip(m.points()) {
            if (a[0] * a[0] + a[1] * a[1]).sqrt() <= 2.0 {
                assert_eq!(a, b, "T1 point inside rho 2 moved");
            }
        }
        let far = m.patch("farfield").unwrap();
        let (fc, sf) = (m.face_centres(), m.face_area_vectors());
        let (mut worst, mut inflow, mut left) = (0.0f64, 0usize, 0usize);
        for f in far.start..far.start + far.size {
            worst = worst.max((fc[f][0].abs().max(fc[f][1].abs()) - EDGE).abs());
            let pts = m.face_points(f);
            let on_left = pts.iter().all(|&i| m.points()[i][0] == -EDGE);
            let on_top = pts.iter().all(|&i| m.points()[i][1].abs() == EDGE);
            if on_left {
                left += 1;
                assert!(
                    sf[f][1] == 0.0 && sf[f][2] == 0.0 && sf[f][0] < 0.0,
                    "T1 left face {f}: {:?}",
                    sf[f]
                );
            }
            if on_top {
                assert!(sf[f][0] == 0.0, "T1 top/bottom face {f}: {:?}", sf[f]);
            }
            if vec3::dot(sf[f], U_INF) < 0.0 {
                inflow += 1;
                assert!(on_left, "T1 inflow face {f} not on x = -8: {:?} {:?}", fc[f], sf[f]);
            }
        }
        assert!(worst <= 1e-12, "T1 far-field face off the square by {worst}");
        assert_eq!(left, n_theta / 4, "T1 faces on x = -8");
        assert_eq!(inflow, n_theta / 4, "T1 inflow faces");
        let x0: Vec<Vec3> = m.points().to_vec();
        let w: Vec<f64> = x0.iter().map(|p| blend((p[0] * p[0] + p[1] * p[1]).sqrt())).collect();
        let vols: Vec<f64> = [-AMP, 0.0, AMP]
            .iter()
            .map(|&yc| min_volume(&m, posed(&x0, &w, yc)))
            .collect();
        assert!(vols.iter().all(|&v| v > 0.0), "T1 volumes {vols:?}");
        println!(
            "T1 ({n_theta} x {n_r}): far-field faces on the square within {worst:.1e}, {left} faces on x = -8, {inflow} inflow; \
             smallest cell volume at -A, 0, A: {:.3e} {:.3e} {:.3e}",
            vols[0], vols[1], vols[2]
        );
    }
    let b = ib_mesh(0.025);
    let (fc, sf) = (b.face_centres(), b.face_area_vectors());
    let xmin = b.patch("xmin").unwrap();
    let mut inflow = 0usize;
    for name in ["xmin", "xmax", "ymin", "ymax"] {
        let p = b.patch(name).unwrap();
        for f in p.start..p.start + p.size {
            assert!((fc[f][0].abs().max(fc[f][1].abs()) - EDGE).abs() <= 1e-12);
            if vec3::dot(sf[f], U_INF) < 0.0 {
                inflow += 1;
                assert_eq!(name, "xmin", "T1 box inflow face on {name}");
            }
        }
    }
    assert_eq!(inflow, xmin.size, "T1 box inflow faces");
    println!("T1 box: {} cells, {} inflow faces, all on xmin", b.n_cells(), inflow);
}

/// ADR 0648 T2 (with amendment 1).
#[test]
fn t2_the_measures_on_synthetic_series() {
    let (first, last) = ((PERIODS - 2) * STEPS_PER_PERIOD + 1, PERIODS * STEPS_PER_PERIOD);
    let w = omega();
    let (m, a, p) = (1.234, 0.87, 0.6);
    let f = series(last, |t| {
        m + a * (w * t + p).cos() + 0.3 * (2.0 * w * t).cos() + 0.2 * (3.0 * w * t).sin()
    });
    let (mean, h) = harmonic(&f, first, last, 1.0);
    let e_mean = (mean - m).abs() / m;
    let e_amp = (cabs(h) - a).abs() / a;
    let ph = phase_to_displacement(h, first, last);
    let e_ph = (wrap((ph - (p - PI).to_degrees()).to_radians())).abs().to_degrees();
    let (_, h2) = harmonic(&f, first, last, 2.0);
    let e_h2 = (cabs(h2) - 0.3).abs() / 0.3;
    let mut e_phases = 0.0f64;
    for (name, g, want) in [
        ("y_c", Box::new(yc_at) as Box<dyn Fn(f64) -> f64>, 0.0),
        ("ydot", Box::new(|t: f64| motion().velocity(t)[1]), 90.0),
        ("yddot", Box::new(|t: f64| motion().acceleration(t)[1]), 180.0),
        ("-yddot", Box::new(|t: f64| -motion().acceleration(t)[1]), 0.0),
    ] {
        let (_, z) = harmonic(&series(last, g), first, last, 1.0);
        let got = phase_to_displacement(z, first, last);
        let e = wrap((got - want).to_radians()).abs().to_degrees();
        println!("T2: phase of {name} {got:+.12} deg (want {want})");
        e_phases = e_phases.max(e);
    }
    println!(
        "T2: mean {e_mean:.1e}, amplitude {e_amp:.1e}, second harmonic {e_h2:.1e} (limit 1e-12), phase {e_ph:.1e} deg, \
         motion phases {e_phases:.1e} deg (limit 1e-9)"
    );
    assert!(
        e_mean <= 1e-12 && e_amp <= 1e-12 && e_h2 <= 1e-12,
        "T2 {e_mean} {e_amp} {e_h2}"
    );
    assert!(e_ph <= 1e-9 && e_phases <= 1e-9, "T2 phases {e_ph} {e_phases}");
}

/// ADR 0648 T3 as first run: compared with the forcing at `n dt` (ADR 0648 amendment 2), kept as the record.
#[test]
#[ignore = "ADR 0648 amendment 2: the first T3, defective (time n dt in place of the solver time)"]
fn t3_immersed_smoke_in_the_stream_first_run() {
    let um = AMP * omega();
    for with_velocity in [true, false] {
        let mut c = ib_setup(0.05, with_velocity);
        let mut worst = 0.0f64;
        let mut fmax = 0.0f64;
        for _ in 0..40 {
            let (f, _) = ib_step(&mut c).unwrap_or_else(|e| panic!("T3 stopped: {e}"));
            assert!(f.iter().all(|x| x.is_finite()), "T3 force {f:?}");
            fmax = fmax.max(vec3::norm(f));
            let t = c.n as f64 * dt();
            assert_eq!(
                c.s.immersed.as_ref().unwrap(),
                &c.mv.forcing_at(&c.s.geom, t).unwrap(),
                "T3 forcing not at t_n"
            );
            match c.mv.bodies_at(t)[0].shape {
                SolidShape::Cylinder { centre, .. } => {
                    let want = [0.0, -AMP * (omega() * t).cos(), 0.0];
                    assert!(
                        (0..3).all(|k| (centre[k] - want[k]).abs() <= 1e-14),
                        "T3 centre {centre:?} want {want:?}"
                    );
                }
                _ => unreachable!(),
            }
            let ub = if with_velocity {
                [0.0, AMP * omega() * (omega() * t).sin(), 0.0]
            } else {
                vec3::ZERO
            };
            for &cell in &c.s.immersed.as_ref().unwrap().bodies[0].solid_cells {
                worst = worst.max(vec3::norm(vec3::sub(c.s.u.values[cell], ub)) / um);
            }
        }
        assert!(c.s.u.values.iter().flatten().all(|x| x.is_finite()));
        println!(
            "T3 (velocity in forcing {with_velocity}): max |u_c - u_b| / (A omega) {worst:.2e} (limit 1e-3), max |F| {fmax:.4}"
        );
        assert!(worst <= 1e-3, "T3 {with_velocity}: {worst}");
    }
}

/// ADR 0648 T3 (corrected by amendment 2).
#[test]
fn t3_immersed_smoke_in_the_stream() {
    let um = AMP * omega();
    for with_velocity in [true, false] {
        let mut c = ib_setup(0.05, with_velocity);
        let mut worst = 0.0f64;
        let mut fmax = 0.0f64;
        for _ in 0..40 {
            let (f, _) = ib_step(&mut c).unwrap_or_else(|e| panic!("T3 stopped: {e}"));
            assert!(f.iter().all(|x| x.is_finite()), "T3 force {f:?}");
            fmax = fmax.max(vec3::norm(f));
            let t = c.s.time;
            assert!((t - c.n as f64 * dt()).abs() <= 1e-12, "T3 time {t}");
            assert_eq!(
                c.s.immersed.as_ref().unwrap(),
                &c.mv.forcing_at(&c.s.geom, t).unwrap(),
                "T3 forcing not at t_n"
            );
            match c.mv.bodies_at(t)[0].shape {
                SolidShape::Cylinder { centre, .. } => {
                    let want = [0.0, -AMP * (omega() * t).cos(), 0.0];
                    assert!(
                        (0..3).all(|k| (centre[k] - want[k]).abs() <= 1e-14),
                        "T3 centre {centre:?} want {want:?}"
                    );
                }
                _ => unreachable!(),
            }
            let ub = if with_velocity {
                [0.0, AMP * omega() * (omega() * t).sin(), 0.0]
            } else {
                vec3::ZERO
            };
            for &cell in &c.s.immersed.as_ref().unwrap().bodies[0].solid_cells {
                worst = worst.max(vec3::norm(vec3::sub(c.s.u.values[cell], ub)) / um);
            }
        }
        assert!(c.s.u.values.iter().flatten().all(|x| x.is_finite()));
        println!(
            "T3 (velocity in forcing {with_velocity}): max |u_c - u_b| / (A omega) {worst:.2e} (limit 1e-3), max |F| {fmax:.4}"
        );
        assert!(worst <= 1e-3, "T3 {with_velocity}: {worst}");
    }
}

/// ADR 0648 T4.
#[test]
fn t4_body_fitted_smoke_in_the_stream() {
    let mut c = bf_setup(64, 24);
    let far0: Vec<Vec3> = {
        let ids = rcfd_mesh::motion::patch_points(&c.s.geom.mesh, "farfield").unwrap();
        ids.iter().map(|&i| c.s.geom.mesh.points()[i]).collect()
    };
    let mut cont = 0.0f64;
    for _ in 0..40 {
        let (f, info) = bf_step(&mut c).unwrap_or_else(|e| panic!("T4 stopped: {e}"));
        assert!(f.iter().all(|x| x.is_finite()), "T4 force {f:?}");
        cont = cont.max(info.continuity);
    }
    let mesh = &c.s.geom.mesh;
    let ids = rcfd_mesh::motion::patch_points(mesh, "cylinder").unwrap();
    let mut mean = vec3::ZERO;
    for &i in &ids {
        mean = vec3::add(mean, mesh.points()[i]);
    }
    mean = vec3::scale(mean, 1.0 / ids.len() as f64);
    let yc = yc_at(c.n as f64 * dt());
    let far: Vec<Vec3> = rcfd_mesh::motion::patch_points(mesh, "farfield")
        .unwrap()
        .iter()
        .map(|&i| mesh.points()[i])
        .collect();
    println!(
        "T4: max continuity {cont:.2e} (limit 1e-6), cylinder centre ({:+.3e}, {:+.15}) against y_c {yc:+.15}",
        mean[0], mean[1]
    );
    assert!(cont <= 1e-6, "T4 continuity {cont}");
    assert!(
        mean[0].abs() <= 1e-12 && (mean[1] - yc).abs() <= 1e-12,
        "T4 centre {mean:?}"
    );
    assert_eq!(far, far0, "T4 far field moved");
}

// ---------------------------------------------------------------------------------------------------------------------------
// The heavy record (ADR 0648 A1 to A5, R1, N1a to N1d, S1, C1, P1)
// ---------------------------------------------------------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Run {
    label: String,
    force: Vec<Vec3>,
    max_continuity: f64,
    stopped: Option<String>,
    seconds: f64,
    cells: usize,
}

fn record<F: FnMut() -> Result<(Vec3, StepInfo), String>>(label: &str, cells: usize, mut step: F) -> Run {
    let t0 = Instant::now();
    let steps = PERIODS * STEPS_PER_PERIOD;
    let mut out = Run {
        label: label.into(),
        force: Vec::new(),
        max_continuity: 0.0,
        stopped: None,
        seconds: 0.0,
        cells,
    };
    for n in 1..=steps {
        match step() {
            Ok((f, info)) => {
                if !f.iter().all(|x| x.is_finite()) {
                    out.stopped = Some(format!("step {n}: non-finite force"));
                    break;
                }
                out.max_continuity = out.max_continuity.max(info.continuity);
                out.force.push(f);
                if n % 500 == 0 {
                    println!(
                        "[{label}] step {n}/{steps}, {:.0} s, F ({:+.5}, {:+.5}), continuity {:.2e}, Courant {:.3}",
                        t0.elapsed().as_secs_f64(),
                        f[0],
                        f[1],
                        info.continuity,
                        info.courant
                    );
                }
            }
            Err(e) => {
                out.stopped = Some(format!("step {n}: {e}"));
                break;
            }
        }
    }
    out.seconds = t0.elapsed().as_secs_f64();
    out
}

fn run_ib(label: &str, h: f64, velocity_in_forcing: bool) -> Run {
    let mut c = ib_setup(h, velocity_in_forcing);
    let cells = c.s.geom.n_cells();
    let mut r = record(label, cells, || ib_step(&mut c));
    if r.stopped.is_none() && !c.s.u.values.iter().flatten().all(|x| x.is_finite()) {
        r.stopped = Some("non-finite velocity".into());
    }
    r
}

fn run_bf(label: &str, n_theta: usize, n_r: usize) -> Run {
    let mut c = bf_setup(n_theta, n_r);
    let cells = c.s.geom.n_cells();
    let mut r = record(label, cells, || bf_step(&mut c));
    if r.stopped.is_none() && !c.s.u.values.iter().flatten().all(|x| x.is_finite()) {
        r.stopped = Some("non-finite velocity".into());
    }
    r
}

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

type Job = Box<dyn FnOnce() -> Run + Send>;

/// Reported only: the parts of the lift harmonic in phase with `-yddot` (as `C_a`) and with `-ydot` (as `C_v`).
fn lift_parts(l: (f64, f64), first: usize, last: usize) -> (f64, f64) {
    let (_, ah) = harmonic(&series(last, |t| motion().acceleration(t)[1]), first, last, 1.0);
    let (_, vh) = harmonic(&series(last, |t| motion().velocity(t)[1]), first, last, 1.0);
    let proj = |z: (f64, f64), w: (f64, f64)| -(z.0 * w.0 + z.1 * w.1) / cabs(w);
    let ca = proj(l, ah) / (PI * RADIUS * RADIUS * cabs(ah));
    let cv = proj(l, vh) / (0.5 * 2.0 * RADIUS * U_INF[0]);
    (ca, cv)
}

/// ADR 0648 A1 to A5, R1, N1a to N1d, S1, C1, P1: the registered heavy record (run once).
#[test]
#[ignore = "heavy record (ADR 0648): five runs, about 30 minutes in release"]
fn f707_transverse_cylinder_record() {
    let t0 = Instant::now();
    let jobs: Vec<(&str, Job)> = vec![
        ("BF", Box::new(|| run_bf("BF", 256, 96))),
        ("BFc", Box::new(|| run_bf("BFc", 128, 48))),
        ("IB40", Box::new(|| run_ib("IB40", 0.025, true))),
        ("IB20", Box::new(|| run_ib("IB20", 0.05, true))),
        ("N1", Box::new(|| run_ib("N1", 0.025, false))),
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
            "S1 [{}]: {} cells, {:.0} s, max continuity {:.3e}, stopped {:?}, force series hash {:016x}",
            r.label,
            r.cells,
            r.seconds,
            r.max_continuity,
            r.stopped,
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
    let (w0, w1) = ((PERIODS - 2) * STEPS_PER_PERIOD + 1, PERIODS * STEPS_PER_PERIOD);
    let mid = (PERIODS - 1) * STEPS_PER_PERIOD;
    let all: Vec<(&Run, Measures)> = runs.iter().map(|r| (r, measures(&r.force, w0, w1))).collect();
    for (r, m) in &all {
        let (ca, cv) = lift_parts(m.lift, w0, w1);
        let p9 = measures(&r.force, w0, mid);
        let p10 = measures(&r.force, mid + 1, w1);
        println!(
            "[{}] mean drag {:.6}, lift harmonic ({:+.6}, {:+.6}) amplitude {:.6} phase {:+.3} deg, mean lift {:+.3e}; \
             drag second harmonic {:.4e}; C_a {:.4}, C_v {:.4} (reported)",
            r.label,
            m.mean_x,
            m.lift.0,
            m.lift.1,
            cabs(m.lift),
            phase_to_displacement(m.lift, w0, w1),
            m.mean_y,
            cabs(m.drag2),
            ca,
            cv
        );
        println!(
            "P1 [{}] periods 9 and 10: mean drag {:.6} {:.6}, lift amplitude {:.6} {:.6}, lift phase {:+.3} {:+.3} deg",
            r.label,
            p9.mean_x,
            p10.mean_x,
            cabs(p9.lift),
            cabs(p10.lift),
            phase_to_displacement(p9.lift, w0, mid),
            phase_to_displacement(p10.lift, mid + 1, w1)
        );
    }
    let get = |r: &Run| all.iter().find(|(x, _)| x.label == r.label).unwrap().1;
    let (mb, mc, mi, m20, mn) = (get(bf), get(bfc), get(ib40), get(ib20), get(n1));
    let lb = cabs(mb.lift);
    let ph = |m: &Measures| phase_to_displacement(m.lift, w0, w1);
    let drag_rel = |m: &Measures| (m.mean_x - mb.mean_x).abs() / mb.mean_x.abs();
    let amp_rel = |m: &Measures| (cabs(m.lift) - lb).abs() / lb;
    let phase_diff = |m: &Measures| wrap((ph(m) - ph(&mb)).to_radians()).abs().to_degrees();
    let complex_rel = |m: &Measures| cabs((m.lift.0 - mb.lift.0, m.lift.1 - mb.lift.1)) / lb;
    let a1 = drag_rel(&mi);
    let a2 = amp_rel(&mi);
    let a3 = phase_diff(&mi);
    let a4 = complex_rel(&mi);
    let a5 = (mi.mean_y - mb.mean_y).abs() / lb;
    let r1d = drag_rel(&mc);
    let r1l = complex_rel(&mc);
    let (n1a, n1b, n1c, n1d) = (drag_rel(&mn), amp_rel(&mn), phase_diff(&mn), complex_rel(&mn));
    let (c1d, c1l) = (drag_rel(&m20), complex_rel(&m20));
    println!("A1 mean drag {a1:.4e} (limit 0.03)");
    println!("A2 lift amplitude {a2:.4e} (limit 0.04)");
    println!("A3 lift phase {a3:.4} deg (limit 3)");
    println!("A4 lift harmonic complex {a4:.4e} (limit 0.06)");
    println!("A5 mean lift {a5:.4e} (limit 0.02)");
    println!("R1 BFc against BF: mean drag {r1d:.4e} (limit 0.02), lift harmonic {r1l:.4e} (limit 0.03)");
    println!(
        "N1 velocity left out: mean drag {n1a:.4e} (must exceed 0.03), lift amplitude {n1b:.4e} (must exceed 0.04), \
         lift phase {n1c:.3} deg (must exceed 3), lift harmonic {n1d:.4e} (must exceed 0.5)"
    );
    println!(
        "C1 G20: mean drag {c1d:.4e} against G40 {a1:.4e}; lift harmonic {c1l:.4e} against {a4:.4e}; larger: {} (reported)",
        c1d > a1 && c1l > a4
    );
    let checks = [
        ("A1", a1 <= 0.03),
        ("A2", a2 <= 0.04),
        ("A3", a3 <= 3.0),
        ("A4", a4 <= 0.06),
        ("A5", a5 <= 0.02),
        ("R1 drag", r1d <= 0.02),
        ("R1 lift", r1l <= 0.03),
        ("N1a", n1a > 0.03),
        ("N1b", n1b > 0.04),
        ("N1c", n1c > 3.0),
        ("N1d", n1d > 0.5),
    ];
    for (id, ok) in checks {
        println!("{id}: {}", if ok { "met" } else { "MISSED" });
        if !ok {
            fails.push(id.to_string());
        }
    }
    assert!(fails.is_empty(), "missed: {fails:?}");
}
