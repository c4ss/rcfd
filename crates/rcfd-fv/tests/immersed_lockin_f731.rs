//! F-731 (ADR 0657): moving immersed shapes at lock-in, a cylinder oscillating across a uniform stream at Re 100 (A/D 0.5,
//! f D/U 0.14, about 12 per cent below the natural shedding frequency) in the square `[-8, 8]^2` with a far-field condition, as
//! a moving immersed body on a Cartesian mesh (the F-308 library, `immersed_motion.rs`, unchanged) against the body-fitted
//! moving-mesh run of this repository on an O-grid mapped to the same square (the F-707 set-up of ADR 0648 at Re 100).
//!
//! The comparison allows for the natural-frequency difference of the two discretisations: fixed-cylinder runs of both give
//! their shedding frequency `f0`, two more body-fitted runs at +-3 per cent of the forcing frequency give the slope of each
//! measure with respect to `f / f0`, and each limit is a base plus that slope times the ratio difference (ADR 0657 D4).
//!
//! The limits, set-ups, measures and mutations were registered in ADR 0657 before this file existed. The fast tests T1 to T6 run
//! by default; the heavy record `f731_lockin_record` (nine runs on nine threads, several hours in release) is ignored and was run
//! once.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::ale::{move_mesh, set_follow_conditions};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::diffusion::uniform_gamma;
use rcfd_fv::immersed::{velocity_rate, ImmersedBody, ImmersedForcing, ImmersedSettings, SolidShape};
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
// The case (ADR 0657 arithmetic 1 to 5, D2)
// ---------------------------------------------------------------------------------------------------------------------------

const NU: f64 = 0.01;
const RADIUS: f64 = 0.5;
const EDGE: f64 = 8.0;
const U_INF: Vec3 = [1.0, 0.0, 0.0];
const AMP: f64 = 0.5;
/// The forcing frequency `f D / U` of the compared runs.
const F_FORCE: f64 = 0.14;
/// Steps per forcing period of the compared runs; `dt = T / 700` for every run.
const STEPS_F: usize = 700;
/// Steps per period of the bracket runs (`f-`, `f+`).
const STEPS_MINUS: usize = 720;
const STEPS_PLUS: usize = 680;
/// Each forced run: 20 of its own periods; window W its last 4.
const PERIODS: usize = 20;
const WINDOW_PERIODS: usize = 4;
/// Fixed runs: 14 000 steps, `f0` from the crossings with `t >= 80`.
const FIXED_STEPS: usize = 14_000;
const F0_FROM: f64 = 80.0;
const K_IB: f64 = 1e6;
const GROWTH: f64 = 1.08;
const EXPANSION: f64 = 20.0;

/// The common time step `T / 700` (ADR 0657 arithmetic 2).
fn dt_common() -> f64 {
    (1.0 / F_FORCE) / STEPS_F as f64
}

/// A run specification: the motion amplitude, the time step and the steps per period of its forcing (for a fixed run, the
/// steps per period of the compared forcing, used by X1), and whether it starts from the perturbed stream.
#[derive(Debug, Clone, Copy)]
struct Spec {
    amp: f64,
    dt: f64,
    n_p: usize,
    perturbed: bool,
}

impl Spec {
    fn forced(n_p: usize) -> Spec {
        Spec {
            amp: AMP,
            dt: dt_common(),
            n_p,
            perturbed: false,
        }
    }

    fn fixed() -> Spec {
        Spec {
            amp: 0.0,
            dt: dt_common(),
            n_p: STEPS_F,
            perturbed: true,
        }
    }

    fn period(&self) -> f64 {
        self.n_p as f64 * self.dt
    }

    fn omega(&self) -> f64 {
        TAU / self.period()
    }

    fn freq(&self) -> f64 {
        1.0 / self.period()
    }

    /// `y_c(t) = -A cos(omega t)` across the stream.
    fn motion(&self) -> HarmonicTranslation {
        HarmonicTranslation {
            amplitude: [0.0, self.amp, 0.0],
            omega: self.omega(),
            phase: -FRAC_PI_2,
        }
    }

    fn yc(&self, t: f64) -> f64 {
        self.motion().displacement(t)[1]
    }

    fn steps(&self) -> usize {
        if self.amp == 0.0 {
            FIXED_STEPS
        } else {
            PERIODS * self.n_p
        }
    }

    /// Window W (1-based steps, inclusive): the last four periods.
    fn window(&self) -> (usize, usize) {
        let last = self.steps();
        (last - WINDOW_PERIODS * self.n_p + 1, last)
    }
}

fn spec_f() -> Spec {
    Spec::forced(STEPS_F)
}

fn spec_minus() -> Spec {
    Spec::forced(STEPS_MINUS)
}

fn spec_plus() -> Spec {
    Spec::forced(STEPS_PLUS)
}

/// The start perturbation of the fixed runs (ADR 0657 arithmetic 3).
fn perturbation(c: Vec3) -> Vec3 {
    let blob = 0.1 * (-((c[0] - 2.0).powi(2) + (c[1] - 0.3).powi(2))).exp();
    [1.0, blob, 0.0]
}

fn settings() -> PimpleSettings {
    let mut s = PimpleSettings::piso(NU);
    s.convection = ConvScheme::LinearUpwind;
    s.amg_pressure = true;
    s
}

fn initial_u(geom: &FvGeometry, spec: &Spec) -> VectorField {
    if spec.perturbed {
        VectorField::from_fn("U", geom, perturbation)
    } else {
        VectorField::uniform("U", geom, U_INF)
    }
}

/// The far-field condition on `patches` (ADR 0648 D2).
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

fn raw_ogrid(n_theta: usize, n_r: usize) -> Mesh {
    cylinder_ogrid(n_theta, n_r, RADIUS, EDGE, Grading::Expansion(EXPANSION), 1.0).unwrap()
}

fn bf_base_mesh(n_theta: usize, n_r: usize) -> Mesh {
    warp(&raw_ogrid(n_theta, n_r), square_map).unwrap()
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

fn cylinder() -> ImmersedBody {
    ImmersedBody::fixed(
        "cylinder",
        SolidShape::Cylinder {
            centre: vec3::ZERO,
            axis: [0.0, 0.0, 1.0],
            radius: RADIUS,
        },
    )
}

// ---------------------------------------------------------------------------------------------------------------------------
// The set-ups and their steps (shared by the fast tests and the record)
// ---------------------------------------------------------------------------------------------------------------------------

struct IbCase {
    s: PimpleSolver,
    /// `None` for the fixed run (forcing built once).
    mv: Option<MovingImmersed>,
    spec: Spec,
    n: usize,
}

fn ib_setup(h: f64, spec: Spec, velocity_in_forcing: bool) -> IbCase {
    let geom = Arc::new(FvGeometry::new(Arc::new(ib_mesh(h))).unwrap());
    let mut u = initial_u(&geom, &spec);
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    far_field(&geom, &mut u, &mut p, &["xmin", "xmax", "ymin", "ymax"]);
    let gamma = uniform_gamma(&geom, NU);
    let mut s = PimpleSolver::new(geom.clone(), u, p, settings());
    let mv = if spec.amp == 0.0 {
        s.immersed = Some(ImmersedForcing::build(&geom, &[cylinder()], ImmersedSettings::new(K_IB), &gamma).unwrap());
        None
    } else {
        let mut mv = MovingImmersed::new(
            vec![cylinder()],
            vec![Some(spec.motion())],
            ImmersedSettings::new(K_IB),
            gamma,
        )
        .unwrap();
        mv.velocity_in_forcing = velocity_in_forcing;
        Some(mv)
    };
    IbCase { s, mv, spec, n: 0 }
}

/// One step; the force of ADR 0648 D3.
fn ib_step(c: &mut IbCase) -> Result<(Vec3, StepInfo), String> {
    let dt = c.spec.dt;
    let u_old = c.s.u.values.clone();
    let info = match &c.mv {
        Some(mv) => mv.step(&mut c.s, dt)?,
        None => c.s.step(dt),
    };
    c.n += 1;
    if let Some(d) = &c.s.diverged {
        return Err(d.message(None));
    }
    let t = c.n as f64 * dt;
    let ib = c.s.immersed.as_ref().unwrap();
    let dudt = velocity_rate(&c.s.u.values, &u_old, None, dt);
    let (f, _) = ib.force_moment_unsteady(0, &c.s.u.values, &dudt, 1.0, [0.0, c.spec.yc(t), 0.0]);
    Ok((f, info))
}

struct BfCase {
    s: PimpleSolver,
    x0: Vec<Vec3>,
    w: Vec<f64>,
    spec: Spec,
    n: usize,
}

fn bf_setup(n_theta: usize, n_r: usize, spec: Spec) -> BfCase {
    let mesh0 = bf_base_mesh(n_theta, n_r);
    let x0: Vec<Vec3> = mesh0.points().to_vec();
    let w: Vec<f64> = x0.iter().map(|p| blend((p[0] * p[0] + p[1] * p[1]).sqrt())).collect();
    // ADR 0631 amendment 2, ADR 0657 c7: the solver starts on the grid at the initial pose
    let start = rcfd_mesh::motion::moved_mesh(&mesh0, posed(&x0, &w, spec.yc(0.0))).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(start)).unwrap());
    let mut u = initial_u(&geom, &spec);
    u.set_bc(&geom, "cylinder", Box::new(FixedValue::vector(vec3::ZERO)))
        .unwrap();
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    far_field(&geom, &mut u, &mut p, &["farfield"]);
    BfCase {
        s: PimpleSolver::new(geom, u, p, settings()),
        x0,
        w,
        spec,
        n: 0,
    }
}

fn bf_step(c: &mut BfCase) -> Result<(Vec3, StepInfo), String> {
    let dt = c.spec.dt;
    let t = (c.n + 1) as f64 * dt;
    let yc = c.spec.yc(t);
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
// Measures (ADR 0657 D3 to D5)
// ---------------------------------------------------------------------------------------------------------------------------

/// Mean and harmonic `k` (`(2/N) sum F exp(-i k omega t_n)`, as (re, im)) of `v` (one value per step, step `n` at `t_n = n dt`)
/// over steps `first..=last` (1-based), `omega` and `dt` of `spec`.
fn harmonic(v: &[f64], first: usize, last: usize, k: f64, spec: &Spec) -> (f64, (f64, f64)) {
    let n = (last - first + 1) as f64;
    let (mut mean, mut re, mut im) = (0.0, 0.0, 0.0);
    for step in first..=last {
        let f = v[step - 1];
        let wt = k * spec.omega() * step as f64 * spec.dt;
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

fn wrap_deg(a: f64) -> f64 {
    wrap(a.to_radians()).to_degrees()
}

/// The series of `f(t_n)` for steps `1..=last`.
fn series(last: usize, dt: f64, f: impl Fn(f64) -> f64) -> Vec<f64> {
    (1..=last).map(|n| f(n as f64 * dt)).collect()
}

/// `arg(zhat / Yhat)` in degrees, wrapped, with `Yhat` the first harmonic of the displacement over the same steps.
fn phase_to_displacement(z: (f64, f64), first: usize, last: usize, spec: &Spec) -> f64 {
    let (_, y) = harmonic(&series(last, spec.dt, |t| spec.yc(t)), first, last, 1.0, spec);
    let re = z.0 * y.0 + z.1 * y.1;
    let im = z.1 * y.0 - z.0 * y.1;
    wrap(im.atan2(re)).to_degrees()
}

/// L1 of ADR 0657 D5: the lagged difference over one period `n_p`, relative to the spread about the mean of W.
fn lock_lagged(v: &[f64], first: usize, last: usize, n_p: usize) -> f64 {
    let n_w = (last - first + 1) as f64;
    let mean = v[first - 1..last].iter().sum::<f64>() / n_w;
    let (mut num, mut den) = (0.0, 0.0);
    for n in first + n_p..=last {
        let d = v[n - 1] - v[n - 1 - n_p];
        num += d * d;
        den += (v[n - 1] - mean) * (v[n - 1] - mean);
    }
    (num / den).sqrt()
}

/// L2 of ADR 0657 D5: the rms over W of `v` less its mean and its harmonics 1 to 5, relative to the rms about the mean.
fn lock_spectral(v: &[f64], first: usize, last: usize, spec: &Spec) -> f64 {
    let (mean, _) = harmonic(v, first, last, 1.0, spec);
    let hs: Vec<(f64, f64)> = (1..=5).map(|k| harmonic(v, first, last, k as f64, spec).1).collect();
    let (mut num, mut den) = (0.0, 0.0);
    for n in first..=last {
        let t = n as f64 * spec.dt;
        let mut fit = mean;
        for (k, h) in hs.iter().enumerate() {
            let wt = (k + 1) as f64 * spec.omega() * t;
            fit += h.0 * wt.cos() - h.1 * wt.sin();
        }
        num += (v[n - 1] - fit).powi(2);
        den += (v[n - 1] - mean).powi(2);
    }
    (num / den).sqrt()
}

/// Upward crossings (linear interpolation) of `v - mean` over the samples with `t_n >= from`, `t_n = n dt`.
fn upward_crossings(v: &[f64], dt: f64, from: f64) -> Vec<f64> {
    let idx: Vec<usize> = (1..=v.len()).filter(|&n| n as f64 * dt >= from).collect();
    let mean = idx.iter().map(|&n| v[n - 1]).sum::<f64>() / idx.len() as f64;
    let mut cross = Vec::new();
    for w in idx.windows(2) {
        let (a, b) = (v[w[0] - 1] - mean, v[w[1] - 1] - mean);
        if a < 0.0 && b >= 0.0 {
            let s = -a / (b - a);
            cross.push((w[0] as f64 + s) * dt);
        }
    }
    cross
}

#[derive(Debug, Clone, Copy)]
struct Natural {
    crossings: usize,
    f0: f64,
    /// Frequencies of the first and last four of the eight cycles.
    f_halves: (f64, f64),
    /// Lift half ranges over the same four cycles.
    amp_halves: (f64, f64),
}

/// The natural frequency of ADR 0657 D3 (`None` with fewer than 9 crossings).
fn natural(v: &[f64], dt: f64) -> Option<Natural> {
    let c = upward_crossings(v, dt, F0_FROM);
    let k = c.len();
    if k < 9 {
        return None;
    }
    let (a, m, b) = (c[k - 9], c[k - 5], c[k - 1]);
    let half_range = |lo: f64, hi: f64| {
        let vals: Vec<f64> = (1..=v.len())
            .filter(|&n| (n as f64 * dt) >= lo && (n as f64 * dt) <= hi)
            .map(|n| v[n - 1])
            .collect();
        let mx = vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let mn = vals.iter().cloned().fold(f64::INFINITY, f64::min);
        0.5 * (mx - mn)
    };
    Some(Natural {
        crossings: k,
        f0: 8.0 / (b - a),
        f_halves: (4.0 / (m - a), 4.0 / (b - m)),
        amp_halves: (half_range(a, m), half_range(m, b)),
    })
}

/// The slope of ADR 0657 D4 of a measure from the bracket values `x_minus`, `x_plus` at the ratios `r_minus`, `r_plus`,
/// normalised by `scale`.
fn slope(x_minus: f64, x_plus: f64, scale: f64, r_minus: f64, r_plus: f64) -> f64 {
    (x_plus - x_minus) / (scale * (r_plus - r_minus))
}

/// The allowance of ADR 0657 D4: `abs(slope * delta_r)`.
fn allowance(s: f64, delta_r: f64) -> f64 {
    (s * delta_r).abs()
}

#[derive(Debug, Clone, Copy)]
struct Measures {
    mean_x: f64,
    mean_y: f64,
    lift: (f64, f64),
    drag2: (f64, f64),
    phase: f64,
    l1: f64,
    l2: f64,
}

fn measures(force: &[Vec3], first: usize, last: usize, spec: &Spec) -> Measures {
    let fx: Vec<f64> = force.iter().map(|f| f[0]).collect();
    let fy: Vec<f64> = force.iter().map(|f| f[1]).collect();
    let (mean_x, _) = harmonic(&fx, first, last, 1.0, spec);
    let (_, drag2) = harmonic(&fx, first, last, 2.0, spec);
    let (mean_y, lift) = harmonic(&fy, first, last, 1.0, spec);
    Measures {
        mean_x,
        mean_y,
        lift,
        drag2,
        phase: phase_to_displacement(lift, first, last, spec),
        l1: lock_lagged(&fy, first, last, spec.n_p),
        l2: lock_spectral(&fy, first, last, spec),
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// Fast tests T1 to T6
// ---------------------------------------------------------------------------------------------------------------------------

fn min_volume(mesh: &Mesh, pts: Vec<Vec3>) -> f64 {
    let m = rcfd_mesh::motion::moved_mesh(mesh, pts).unwrap();
    m.cell_volumes().iter().cloned().fold(f64::INFINITY, f64::min)
}

/// ADR 0657 T1.
#[test]
fn t1_the_square_map_and_the_far_field_faces() {
    for (n_theta, n_r) in [(256, 128), (128, 64), (64, 32)] {
        let raw = raw_ogrid(n_theta, n_r);
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
            "T1 ({n_theta} x {n_r}, {} cells): far-field faces on the square within {worst:.1e}, {left} faces on x = -8, \
             {inflow} inflow; smallest cell volume at -A, 0, A: {:.3e} {:.3e} {:.3e}",
            m.n_cells(),
            vols[0],
            vols[1],
            vols[2]
        );
    }
}

/// ADR 0657 T2.
#[test]
fn t2_the_measures_on_synthetic_series() {
    // every run specification: dt bit-equal to T / 700, integer steps per period
    let want_dt = (1.0 / F_FORCE) / 700.0;
    for s in [spec_f(), spec_minus(), spec_plus(), Spec::fixed()] {
        assert_eq!(s.dt.to_bits(), want_dt.to_bits(), "T2 dt of {s:?}");
        assert_eq!(s.steps() % s.n_p, 0, "T2 steps of {s:?}");
    }
    let (mut e_rel, mut e_abs, mut e_ph) = (0.0f64, 0.0f64, 0.0f64);
    for spec in [spec_f(), spec_minus(), spec_plus()] {
        let (first, last) = spec.window();
        let w = spec.omega();
        let (m, a, p) = (1.234, 0.87, 0.6);
        let f = series(last, spec.dt, |t| {
            m + a * (w * t + p).cos() + 0.3 * (2.0 * w * t).cos() + 0.2 * (3.0 * w * t).sin()
        });
        let ms = measures(&f.iter().map(|&y| [y, y, 0.0]).collect::<Vec<_>>(), first, last, &spec);
        e_rel = e_rel.max((ms.mean_x - m).abs() / m);
        e_rel = e_rel.max((cabs(ms.lift) - a).abs() / a);
        e_rel = e_rel.max((cabs(ms.drag2) - 0.3).abs() / 0.3);
        e_ph = e_ph.max(wrap_deg(ms.phase - (p - PI).to_degrees()).abs());
        e_abs = e_abs.max(ms.l1).max(ms.l2);
        for (g, want) in [
            (Box::new(|t: f64| spec.yc(t)) as Box<dyn Fn(f64) -> f64>, 0.0),
            (Box::new(|t: f64| spec.motion().velocity(t)[1]), 90.0),
            (Box::new(|t: f64| spec.motion().acceleration(t)[1]), 180.0),
        ] {
            let (_, z) = harmonic(&series(last, spec.dt, g), first, last, 1.0, &spec);
            e_ph = e_ph.max(wrap_deg(phase_to_displacement(z, first, last, &spec) - want).abs());
        }
        // Yhat = -A exactly up to rounding (ADR 0657 D4)
        let (_, y) = harmonic(&series(last, spec.dt, |t| spec.yc(t)), first, last, 1.0, &spec);
        e_rel = e_rel.max(((y.0 + AMP).abs() + y.1.abs()) / AMP);
    }
    // L1 of a sinusoid at f0 = f / 0.875 = 8 f / 7 against 2 |sin(pi f0 / f)| (arithmetic 6): W of 8 periods, the summed 7 periods
    // hold 8 cycles; the series is odd about the centre of W so that its W mean vanishes
    let spec = spec_f();
    let (first, last) = (1, 8 * STEPS_F);
    let f0 = spec.freq() / 0.875;
    let tc = 0.5 * (first + last) as f64 * spec.dt;
    let s = series(last, spec.dt, |t| (TAU * f0 * (t - tc)).sin());
    let l1 = lock_lagged(&s, first, last, spec.n_p);
    let l1_want = 2.0 * (PI * f0 / spec.freq()).sin().abs();
    let e_l1 = (l1 - l1_want).abs();
    // f0 of D3 on the step grid of a fixed run
    let f0s = 0.16;
    let v = series(FIXED_STEPS, spec.dt, |t| 0.4 + (TAU * f0s * t + 0.3).sin());
    let nat = natural(&v, spec.dt).unwrap();
    let e_f0 = (nat.f0 - f0s).abs() / f0s;
    // the allowance on given numbers against the closed form
    let (xm, xp, scale, rm, rp, dr) = (1.30, 1.36, 1.33, 0.85, 0.90, -0.02);
    let a = allowance(slope(xm, xp, scale, rm, rp), dr);
    let a_want = ((xp - xm) / scale / (rp - rm) * dr).abs();
    let e_allow = (a - a_want).abs() / a_want;
    println!(
        "T2: relative {e_rel:.1e}, L1 and L2 of the periodic series {e_abs:.1e} (limits 1e-12), phases {e_ph:.1e} deg (1e-9); \
         L1 of the f0 sinusoid {l1:.12} against {l1_want:.12} ({e_l1:.1e}, limit 1e-3); f0 {:.10} ({e_f0:.1e}, limit 1e-6, \
         {} crossings); allowance {a:.15} ({e_allow:.1e}, limit 1e-14)",
        nat.f0, nat.crossings
    );
    assert!(e_rel <= 1e-12 && e_abs <= 1e-12, "T2 {e_rel} {e_abs}");
    assert!(e_ph <= 1e-9, "T2 phases {e_ph}");
    assert!(e_l1 <= 1e-3, "T2 L1 {l1} {l1_want}");
    assert!(e_f0 <= 1e-6, "T2 f0 {}", nat.f0);
    assert!(e_allow <= 1e-14, "T2 allowance {a} {a_want}");
}

/// ADR 0657 T3.
#[test]
fn t3_immersed_smoke_in_the_stream() {
    let spec = spec_f();
    let um = AMP * spec.omega();
    for with_velocity in [true, false] {
        let mut c = ib_setup(0.05, spec, with_velocity);
        let mut worst = 0.0f64;
        let mut fmax = 0.0f64;
        for _ in 0..40 {
            let (f, _) = ib_step(&mut c).unwrap_or_else(|e| panic!("T3 stopped: {e}"));
            assert!(f.iter().all(|x| x.is_finite()), "T3 force {f:?}");
            fmax = fmax.max(vec3::norm(f));
            let t = c.s.time;
            assert!((t - c.n as f64 * spec.dt).abs() <= 1e-12, "T3 time {t}");
            let mv = c.mv.as_ref().unwrap();
            assert_eq!(
                c.s.immersed.as_ref().unwrap(),
                &mv.forcing_at(&c.s.geom, t).unwrap(),
                "T3 forcing not at the solver time"
            );
            let ub = if with_velocity {
                [0.0, AMP * spec.omega() * (spec.omega() * t).sin(), 0.0]
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

/// ADR 0657 T4.
#[test]
fn t4_body_fitted_smoke_in_the_stream() {
    let mut c = bf_setup(64, 32, spec_f());
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
    let yc = c.spec.yc(c.n as f64 * c.spec.dt);
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

/// ADR 0657 T5: the body-fitted solver starts on the grid at the initial pose (c7).
#[test]
fn t5_body_fitted_solver_starts_at_the_initial_pose() {
    let c = bf_setup(64, 32, spec_f());
    let pts = c.s.geom.mesh.points();
    let want = posed(&c.x0, &c.w, c.spec.yc(0.0));
    assert!(
        pts.iter()
            .zip(&want)
            .all(|(a, b)| (0..3).all(|k| a[k].to_bits() == b[k].to_bits())),
        "T5 solver mesh not at the initial pose"
    );
    let ids = rcfd_mesh::motion::patch_points(&c.s.geom.mesh, "cylinder").unwrap();
    let mut mean = vec3::ZERO;
    for &i in &ids {
        mean = vec3::add(mean, pts[i]);
    }
    mean = vec3::scale(mean, 1.0 / ids.len() as f64);
    let biggest = pts
        .iter()
        .zip(&c.x0)
        .map(|(a, b)| vec3::norm(vec3::sub(*a, *b)))
        .fold(0.0f64, f64::max);
    println!(
        "T5: cylinder centre ({:+.3e}, {:+.15}) against (0, -A); largest difference to the unmoved points {biggest:.15}",
        mean[0], mean[1]
    );
    assert!(
        mean[0].abs() <= 1e-12 && (mean[1] + AMP).abs() <= 1e-12,
        "T5 centre {mean:?}"
    );
    assert!((biggest - AMP).abs() <= 1e-12, "T5 largest difference {biggest}");
    let f = bf_setup(64, 32, Spec::fixed());
    assert!(
        f.s.geom
            .mesh
            .points()
            .iter()
            .zip(&f.x0)
            .all(|(a, b)| (0..3).all(|k| a[k].to_bits() == b[k].to_bits())),
        "T5 fixed run not on the unmoved points"
    );
}

/// ADR 0657 T6: the fixed set-ups.
#[test]
fn t6_fixed_setups_smoke() {
    let spec = Spec::fixed();
    let mut ib = ib_setup(0.05, spec, true);
    let mut bf = bf_setup(64, 32, spec);
    for (name, s) in [("IB", &ib.s), ("BF", &bf.s)] {
        let cc = s.geom.mesh.cell_centres();
        assert!(
            s.u.values
                .iter()
                .zip(cc)
                .all(|(u, c)| (0..3).all(|k| u[k].to_bits() == perturbation(*c)[k].to_bits())),
            "T6 {name}: initial field is not the perturbation"
        );
    }
    let mut solid_worst = 0.0f64;
    for n in 0..20 {
        let (f, _) = ib_step(&mut ib).unwrap_or_else(|e| panic!("T6 IB stopped: {e}"));
        assert!(f.iter().all(|x| x.is_finite()), "T6 IB force {f:?}");
        if n == 0 {
            for &cell in &ib.s.immersed.as_ref().unwrap().bodies[0].solid_cells {
                solid_worst = solid_worst.max(vec3::norm(ib.s.u.values[cell]));
            }
        }
        let (f, _) = bf_step(&mut bf).unwrap_or_else(|e| panic!("T6 BF stopped: {e}"));
        assert!(f.iter().all(|x| x.is_finite()), "T6 BF force {f:?}");
    }
    println!(
        "T6: IB solid cells after the first step max |u| {solid_worst:.2e} (limit 1e-3); 20 steps of each, no stop"
    );
    assert!(solid_worst <= 1e-3, "T6 solid {solid_worst}");
}

// ---------------------------------------------------------------------------------------------------------------------------
// The heavy record (ADR 0657 F0 to F2, L1, L2, X1, J1, A1 to A5, R1, N1a to N1d, S1, P1)
// ---------------------------------------------------------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Run {
    label: String,
    spec: Spec,
    force: Vec<Vec3>,
    max_continuity: f64,
    max_courant: f64,
    stopped: Option<String>,
    seconds: f64,
    cells: usize,
}

fn record<F: FnMut() -> Result<(Vec3, StepInfo), String>>(label: &str, spec: Spec, cells: usize, mut step: F) -> Run {
    let t0 = Instant::now();
    let steps = spec.steps();
    let mut out = Run {
        label: label.into(),
        spec,
        force: Vec::new(),
        max_continuity: 0.0,
        max_courant: 0.0,
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
                out.max_courant = out.max_courant.max(info.courant);
                out.force.push(f);
                if n % 1000 == 0 {
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

fn run_ib(label: &str, h: f64, spec: Spec, velocity_in_forcing: bool) -> Run {
    let mut c = ib_setup(h, spec, velocity_in_forcing);
    let cells = c.s.geom.n_cells();
    let mut r = record(label, spec, cells, || ib_step(&mut c));
    if r.stopped.is_none() && !c.s.u.values.iter().flatten().all(|x| x.is_finite()) {
        r.stopped = Some("non-finite velocity".into());
    }
    r
}

fn run_bf(label: &str, n_theta: usize, n_r: usize, spec: Spec) -> Run {
    let mut c = bf_setup(n_theta, n_r, spec);
    let cells = c.s.geom.n_cells();
    let mut r = record(label, spec, cells, || bf_step(&mut c));
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
fn lift_parts(l: (f64, f64), first: usize, last: usize, spec: &Spec) -> (f64, f64) {
    let (_, ah) = harmonic(
        &series(last, spec.dt, |t| spec.motion().acceleration(t)[1]),
        first,
        last,
        1.0,
        spec,
    );
    let (_, vh) = harmonic(
        &series(last, spec.dt, |t| spec.motion().velocity(t)[1]),
        first,
        last,
        1.0,
        spec,
    );
    let proj = |z: (f64, f64), w: (f64, f64)| -(z.0 * w.0 + z.1 * w.1) / cabs(w);
    let ca = proj(l, ah) / (PI * RADIUS * RADIUS * cabs(ah));
    let cv = proj(l, vh) / (0.5 * 2.0 * RADIUS * U_INF[0]);
    (ca, cv)
}

/// ADR 0657: the registered heavy record (run once).
#[test]
#[ignore = "heavy record (ADR 0657): nine runs, several hours in release"]
fn f731_lockin_record() {
    let t0 = Instant::now();
    let jobs: Vec<(&str, Job)> = vec![
        ("BF0", Box::new(|| run_bf("BF0", 256, 128, Spec::fixed()))),
        ("BFc0", Box::new(|| run_bf("BFc0", 128, 64, Spec::fixed()))),
        ("IB0", Box::new(|| run_ib("IB0", 0.025, Spec::fixed(), true))),
        ("BF", Box::new(|| run_bf("BF", 256, 128, spec_f()))),
        ("BF-", Box::new(|| run_bf("BF-", 256, 128, spec_minus()))),
        ("BF+", Box::new(|| run_bf("BF+", 256, 128, spec_plus()))),
        ("BFc", Box::new(|| run_bf("BFc", 128, 64, spec_f()))),
        ("IB", Box::new(|| run_ib("IB", 0.025, spec_f(), true))),
        ("N1", Box::new(|| run_ib("N1", 0.025, spec_f(), false))),
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
            "S1 [{}]: {} cells, {} steps, {:.0} s, max continuity {:.3e}, max Courant {:.3}, stopped {:?}, force series hash {:016x}",
            r.label,
            r.cells,
            r.force.len(),
            r.seconds,
            r.max_continuity,
            r.max_courant,
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
    let get = |l: &str| runs.iter().find(|r| r.label == l).unwrap();
    let mut checks: Vec<(String, bool)> = Vec::new();

    // F1, F0, F2, X1: the fixed runs
    let mut nat = std::collections::HashMap::new();
    for l in ["BF0", "BFc0", "IB0"] {
        let r = get(l);
        let fy: Vec<f64> = r.force.iter().map(|f| f[1]).collect();
        let (xs, xe) = (r.force.len() - 4 * STEPS_F + 1, r.force.len());
        let x1 = lock_lagged(&fy, xs, xe, STEPS_F);
        match natural(&fy, r.spec.dt) {
            None => {
                println!("F1 [{l}]: fewer than 9 crossings after t = {F0_FROM}");
                checks.push((format!("F1 {l}"), false));
            }
            Some(n) => {
                let df = (n.f_halves.0 - n.f_halves.1).abs() / n.f0;
                let da = (n.amp_halves.0 - n.amp_halves.1).abs() / n.amp_halves.1;
                let (mean_x, _) = harmonic(&r.force.iter().map(|f| f[0]).collect::<Vec<_>>(), xs, xe, 1.0, &r.spec);
                println!(
                    "F1 [{l}]: {} crossings, f0 = St {:.6}, halves f {:.6} {:.6} ({df:.2e}, limit 0.005), lift half range \
                     {:.6} {:.6} ({da:.2e}, limit 0.02); mean drag of the last 4 forcing periods {mean_x:.6} (reported); \
                     X1 L1 {x1:.4} (must exceed 0.1)",
                    n.crossings, n.f0, n.f_halves.0, n.f_halves.1, n.amp_halves.0, n.amp_halves.1
                );
                checks.push((format!("F1 {l}"), df <= 0.005 && da <= 0.02));
                nat.insert(l, n.f0);
            }
        }
        if l != "BFc0" {
            checks.push((format!("X1 {l}"), x1 > 0.1));
        }
    }
    if nat.len() < 3 {
        for (id, ok) in &checks {
            println!("{id}: {}", if *ok { "met" } else { "MISSED" });
        }
        panic!("F1: no natural frequency for every fixed run, the comparison is not made");
    }
    let (f0_bf, f0_c, f0_ib) = (nat["BF0"], nat["BFc0"], nat["IB0"]);
    let d_ib = f0_ib / f0_bf - 1.0;
    let d_c = f0_c / f0_bf - 1.0;
    let f = spec_f().freq();
    let (r_bf, r_ib, r_c) = (f / f0_bf, f / f0_ib, f / f0_c);
    let (r_m, r_p) = (spec_minus().freq() / f0_bf, spec_plus().freq() / f0_bf);
    println!(
        "F0: f0_IB / f0_BF - 1 = {d_ib:+.4e}, f0_BFc / f0_BF - 1 = {d_c:+.4e} (limit 0.045 each); \
         F2: r_BF {r_bf:.5}, r_IB {r_ib:.5}, r_BFc {r_c:.5}, bracket {r_m:.5} {r_p:.5} (reported, expected in [0.75, 0.97])"
    );
    checks.push(("F0 IB".into(), d_ib.abs() <= 0.045));
    checks.push(("F0 BFc".into(), d_c.abs() <= 0.045));

    // forced runs
    let mut ms = std::collections::HashMap::new();
    for l in ["BF", "BF-", "BF+", "BFc", "IB", "N1"] {
        let r = get(l);
        let (w0, w1) = r.spec.window();
        let m = measures(&r.force, w0, w1, &r.spec);
        let (ca, cv) = lift_parts(m.lift, w0, w1, &r.spec);
        println!(
            "[{l}] f {:.6}: mean drag {:.6}, lift harmonic ({:+.6}, {:+.6}) amplitude {:.6} phase {:+.3} deg, mean lift {:+.3e}; \
             L1 {:.3e}, L2 {:.3e}; drag second harmonic {:.4e}; C_a {:.4}, C_v {:.4} (reported)",
            r.spec.freq(),
            m.mean_x,
            m.lift.0,
            m.lift.1,
            cabs(m.lift),
            m.phase,
            m.mean_y,
            m.l1,
            m.l2,
            cabs(m.drag2),
            ca,
            cv
        );
        let per: Vec<String> = (0..WINDOW_PERIODS)
            .map(|k| {
                let a = w0 + k * r.spec.n_p;
                let p = measures(&r.force, a, a + r.spec.n_p - 1, &r.spec);
                format!("({:.5}, {:.5}, {:+.2})", p.mean_x, cabs(p.lift), p.phase)
            })
            .collect();
        println!(
            "P1 [{l}] per period of W (mean drag, lift amplitude, phase): {}",
            per.join(" ")
        );
        if l != "N1" {
            checks.push((format!("L1 {l}"), m.l1 <= 0.03));
            checks.push((format!("L2 {l}"), m.l2 <= 0.05));
        }
        ms.insert(l, m);
    }
    let (mb, mm, mp, mc, mi, mn) = (ms["BF"], ms["BF-"], ms["BF+"], ms["BFc"], ms["IB"], ms["N1"]);
    let lb = cabs(mb.lift);

    // J1
    let j_phase = wrap_deg(mp.phase - mm.phase).abs();
    let j_amp = (cabs(mp.lift) - cabs(mm.lift)).abs() / lb;
    println!(
        "J1: phase change across the bracket {j_phase:.3} deg (limit 25), amplitude change {j_amp:.4e} (limit 0.25)"
    );
    checks.push(("J1 phase".into(), j_phase <= 25.0));
    checks.push(("J1 amplitude".into(), j_amp <= 0.25));

    // slopes and allowances (D4)
    let s_d = slope(mm.mean_x, mp.mean_x, mb.mean_x, r_m, r_p);
    let s_a = slope(cabs(mm.lift), cabs(mp.lift), lb, r_m, r_p);
    let s_p = wrap_deg(mp.phase - mm.phase) / (r_p - r_m);
    let s_c = cabs((mp.lift.0 - mm.lift.0, mp.lift.1 - mm.lift.1)) / (lb * (r_p - r_m));
    let dr = r_ib - r_bf;
    let dr_c = r_c - r_bf;
    let lim_a1 = 0.04 + allowance(s_d, dr);
    let lim_a2 = 0.08 + allowance(s_a, dr);
    let lim_a3 = 8.0 + allowance(s_p, dr);
    let lim_a4 = 0.12 + allowance(s_c, dr);
    let lim_r1d = 0.03 + allowance(s_d, dr_c);
    let lim_r1l = 0.08 + allowance(s_c, dr_c);
    println!(
        "D4: slopes s_D {s_d:+.4}, s_A {s_a:+.4}, s_P {s_p:+.3} deg, s_C {s_c:.4}; Delta r {dr:+.5}, Delta r_c {dr_c:+.5}; \
         limits A1 {lim_a1:.4}, A2 {lim_a2:.4}, A3 {lim_a3:.3} deg, A4 {lim_a4:.4}, R1 {lim_r1d:.4} {lim_r1l:.4}"
    );
    let drag_rel = |m: &Measures| (m.mean_x - mb.mean_x).abs() / mb.mean_x.abs();
    let amp_rel = |m: &Measures| (cabs(m.lift) - lb).abs() / lb;
    let phase_diff = |m: &Measures| wrap_deg(m.phase - mb.phase).abs();
    let complex_rel = |m: &Measures| cabs((m.lift.0 - mb.lift.0, m.lift.1 - mb.lift.1)) / lb;
    let (a1, a2, a3, a4) = (drag_rel(&mi), amp_rel(&mi), phase_diff(&mi), complex_rel(&mi));
    let a5 = (mi.mean_y - mb.mean_y).abs() / lb;
    let (r1d, r1l) = (drag_rel(&mc), complex_rel(&mc));
    let (n1a, n1b, n1c, n1d) = (drag_rel(&mn), amp_rel(&mn), phase_diff(&mn), complex_rel(&mn));
    // reported: the IB against the reference interpolated linearly to r_IB
    let t = (r_ib - r_m) / (r_p - r_m);
    let lerp = |a: f64, b: f64| a + t * (b - a);
    let bi_drag = lerp(mm.mean_x, mp.mean_x);
    let bi_lift = (lerp(mm.lift.0, mp.lift.0), lerp(mm.lift.1, mp.lift.1));
    println!(
        "reported: IB against BF interpolated to r_IB: mean drag {:.4e}, lift amplitude {:.4e}, complex {:.4e}",
        (mi.mean_x - bi_drag).abs() / bi_drag,
        (cabs(mi.lift) - cabs(bi_lift)).abs() / cabs(bi_lift),
        cabs((mi.lift.0 - bi_lift.0, mi.lift.1 - bi_lift.1)) / cabs(bi_lift)
    );
    println!("A1 mean drag {a1:.4e} (limit {lim_a1:.4e})");
    println!("A2 lift amplitude {a2:.4e} (limit {lim_a2:.4e})");
    println!("A3 lift phase {a3:.4} deg (limit {lim_a3:.4})");
    println!("A4 lift harmonic complex {a4:.4e} (limit {lim_a4:.4e})");
    println!("A5 mean lift {a5:.4e} (limit 0.05)");
    println!(
        "R1 BFc against BF: mean drag {r1d:.4e} (limit {lim_r1d:.4e}), lift harmonic {r1l:.4e} (limit {lim_r1l:.4e})"
    );
    println!(
        "N1 velocity left out: mean drag {n1a:.4e} (must exceed {lim_a1:.4e}), lift amplitude {n1b:.4e} (must exceed \
         {lim_a2:.4e}), lift phase {n1c:.3} deg (must exceed {lim_a3:.3}), lift harmonic {n1d:.4e} (must exceed {lim_a4:.4e}); \
         L1 {:.3e}, L2 {:.3e} (reported)",
        mn.l1, mn.l2
    );
    checks.push(("A1".into(), a1 <= lim_a1));
    checks.push(("A2".into(), a2 <= lim_a2));
    checks.push(("A3".into(), a3 <= lim_a3));
    checks.push(("A4".into(), a4 <= lim_a4));
    checks.push(("A5".into(), a5 <= 0.05));
    checks.push(("R1 drag".into(), r1d <= lim_r1d));
    checks.push(("R1 lift".into(), r1l <= lim_r1l));
    checks.push(("N1a".into(), n1a > lim_a1));
    checks.push(("N1b".into(), n1b > lim_a2));
    checks.push(("N1c".into(), n1c > lim_a3));
    checks.push(("N1d".into(), n1d > lim_a4));
    for (id, ok) in &checks {
        println!("{id}: {}", if *ok { "met" } else { "MISSED" });
        if !ok {
            fails.push(id.clone());
        }
    }
    assert!(fails.is_empty(), "missed: {fails:?}");
}
