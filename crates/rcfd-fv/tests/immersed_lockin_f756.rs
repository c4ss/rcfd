//! F-756 (ADR 0677): moving immersed shapes at lock-in near the natural frequency at the smaller amplitude. A cylinder oscillating
//! across a uniform stream at Re 100 with A/D 0.25 in the square `[-8, 8]^2` with a far-field condition, as a moving immersed
//! body on a Cartesian mesh (the F-308 library, `immersed_motion.rs`, unchanged) against the body-fitted moving-mesh run of this
//! repository on an O-grid mapped to the same square: the set-up, measures and allowance of F-731 and F-750 (ADR 0657, ADR 0671;
//! the helpers of `immersed_lockin_f750.rs` are copied here, that file and the F-731 file are not edited) at a forcing point
//! between the natural frequency and 10 per cent above it, chosen by a registered body-fitted scan whose conditioning is measured
//! over the comparison's own frequency bracket (the runs at 720 and 680 steps of the point's time step).
//!
//! Two ignored heavy records, each run once: `f756_scan_record` (stage S of ADR 0677 D2: at each of three candidates the coarse
//! body-fitted run and its two bracket runs, and a coarse negative control; the selection rule; the reuse check SR of the F-750
//! series) and `f756_lockin_record` (stage M, D3: the nine runs of ADR 0671 D3 at the selected point). Every run's force series
//! is written to `RCFD_F756_SERIES_DIR` (default `CARGO_TARGET_TMPDIR`), read back bit for bit, and its hash printed (K1). The
//! fast tests T1 to T8 run by default.

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
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

// ---------------------------------------------------------------------------------------------------------------------------
// The case (ADR 0657 arithmetic 1 to 5, ADR 0671 arithmetic 2 and 4)
// ---------------------------------------------------------------------------------------------------------------------------

const NU: f64 = 0.01;
const RADIUS: f64 = 0.5;
const EDGE: f64 = 8.0;
const U_INF: Vec3 = [1.0, 0.0, 0.0];
/// Steps per forcing period of the compared runs; `dt = T / 700` for every run of one point.
const STEPS_F: usize = 700;
/// Steps per period of the bracket runs (`f-`, `f+`).
const STEPS_MINUS: usize = 720;
const STEPS_PLUS: usize = 680;
/// Each forced run: 20 of its own periods; window W its last 4.
const PERIODS: usize = 20;
const WINDOW_PERIODS: usize = 4;
/// Fixed runs: `f0` from the crossings with `t >= 80`.
const F0_FROM: f64 = 80.0;
const K_IB: f64 = 1e6;
const GROWTH: f64 = 1.08;
const EXPANSION: f64 = 20.0;
/// The screening scale of stage S: the F-731 fixed body-fitted lift half range (ADR 0657 addendum 1).
const S731: f64 = 0.177620;
/// The F-731 natural frequency of the fine body-fitted reference (ADR 0657 addendum 1), used by the screen S0 only.
const F0_731: f64 = 0.164794;
/// Immersed core sizes: G40 (the compared runs) and G20 (the screening of the negative control).
const H_G40: f64 = 0.025;
const H_G20: f64 = 0.05;
/// The motion amplitude A / D of every forced run of F-756.
const AMP: f64 = 0.25;

/// A forcing point: the motion amplitude `A / D` and the forcing frequency `f D / U`.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Point {
    amp: f64,
    f: f64,
}

/// The candidates C1 to C3 of ADR 0677 arithmetic 2 (f D / U at A/D 0.25).
const CANDIDATES: [f64; 3] = [0.17, 0.175, 0.18];

/// The forcing point selected by the scan (ADR 0677 D2); set by amendment 1, committed before the main record is run.
const SELECTED: Option<Point> = None;

fn candidate(k: usize) -> Point {
    Point {
        amp: AMP,
        f: CANDIDATES[k],
    }
}

/// The common time step `(1 / f) / 700` of every run of the point.
fn dt_of(p: Point) -> f64 {
    (1.0 / p.f) / STEPS_F as f64
}

/// Fixed runs: `700 ceil(20 f / 0.14)` steps (ADR 0671 arithmetic 4); the `1e-9` keeps an exact integer quotient (f 0.14) from
/// rounding up.
fn fixed_steps(f: f64) -> usize {
    STEPS_F * ((PERIODS as f64 * f / 0.14) - 1e-9).ceil() as usize
}

/// The fixed force scale `F_L = S + m_a A omega^2` (ADR 0671 readings), `m_a = rho pi r^2`.
fn force_scale(s: f64, p: Point) -> f64 {
    let w = TAU * p.f;
    s + PI * RADIUS * RADIUS * p.amp * w * w
}

/// A run specification: the motion amplitude, the time step, the steps per period of its forcing (for a fixed run, those of the
/// compared forcing, used by X1), whether it starts from the perturbed stream, and the steps of a fixed run.
#[derive(Debug, Clone, Copy)]
struct Spec {
    amp: f64,
    dt: f64,
    n_p: usize,
    perturbed: bool,
    steps_fixed: usize,
}

impl Spec {
    fn forced(p: Point, n_p: usize) -> Spec {
        Spec {
            amp: p.amp,
            dt: dt_of(p),
            n_p,
            perturbed: false,
            steps_fixed: fixed_steps(p.f),
        }
    }

    fn fixed(p: Point) -> Spec {
        Spec {
            amp: 0.0,
            dt: dt_of(p),
            n_p: STEPS_F,
            perturbed: true,
            steps_fixed: fixed_steps(p.f),
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
            self.steps_fixed
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
// Measures (ADR 0657 D3 to D5, ADR 0671 D3)
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

fn csub(a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    (a.0 - b.0, a.1 - b.1)
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

/// L2 of ADR 0657 D5 and the residual `R2` of ADR 0671 D3: the rms over W of `v` less its mean and its harmonics 1 to 5, relative
/// to the rms about the mean (first) and absolute (second).
fn lock_spectral(v: &[f64], first: usize, last: usize, spec: &Spec) -> (f64, f64) {
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
    ((num / den).sqrt(), (num / (last - first + 1) as f64).sqrt())
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
    r2: f64,
}

fn measures(force: &[Vec3], first: usize, last: usize, spec: &Spec) -> Measures {
    let fx: Vec<f64> = force.iter().map(|f| f[0]).collect();
    let fy: Vec<f64> = force.iter().map(|f| f[1]).collect();
    let (mean_x, _) = harmonic(&fx, first, last, 1.0, spec);
    let (_, drag2) = harmonic(&fx, first, last, 2.0, spec);
    let (mean_y, lift) = harmonic(&fy, first, last, 1.0, spec);
    let (l2, r2) = lock_spectral(&fy, first, last, spec);
    Measures {
        mean_x,
        mean_y,
        lift,
        drag2,
        phase: phase_to_displacement(lift, first, last, spec),
        l1: lock_lagged(&fy, first, last, spec.n_p),
        l2,
        r2,
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// The force series files (ADR 0671 K1)
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

fn series_hash(force: &[Vec3]) -> u64 {
    fnv(force.iter().flat_map(|f| f.iter().map(|x| x.to_bits())))
}

fn series_dir() -> PathBuf {
    match std::env::var_os("RCFD_F756_SERIES_DIR") {
        Some(d) => PathBuf::from(d),
        None => PathBuf::from(env!("CARGO_TARGET_TMPDIR")),
    }
}

/// One line per step: `n F_x F_y F_z`, shortest round-trip decimals in exponent form.
fn write_series(path: &Path, force: &[Vec3]) -> std::io::Result<()> {
    let mut s = String::with_capacity(force.len() * 72);
    for (n, f) in force.iter().enumerate() {
        s.push_str(&format!("{} {:e} {:e} {:e}\n", n + 1, f[0], f[1], f[2]));
    }
    std::fs::write(path, s)
}

fn read_series(path: &Path) -> Result<Vec<Vec3>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut out = Vec::new();
    for (k, line) in text.lines().enumerate() {
        let w: Vec<&str> = line.split_whitespace().collect();
        if w.len() != 4 || w[0].parse::<usize>().ok() != Some(k + 1) {
            return Err(format!("line {}: {line:?}", k + 1));
        }
        let p = |s: &str| s.parse::<f64>().map_err(|e| format!("line {}: {e}", k + 1));
        out.push([p(w[1])?, p(w[2])?, p(w[3])?]);
    }
    Ok(out)
}

/// K1: write the series of `run` to `<dir>/<stage>_<label>.txt`, read it back, compare bit for bit; the hash of the bits.
fn keep_series(stage: &str, label: &str, force: &[Vec3]) -> Result<(PathBuf, u64), String> {
    let dir = series_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let name: String = label
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '.' { c } else { '_' })
        .collect();
    let path = dir.join(format!("{stage}_{name}.txt"));
    write_series(&path, force).map_err(|e| format!("{}: {e}", path.display()))?;
    let back = read_series(&path)?;
    let same = back.len() == force.len()
        && back
            .iter()
            .zip(force)
            .all(|(a, b)| (0..3).all(|k| a[k].to_bits() == b[k].to_bits()));
    if !same {
        return Err(format!("{}: read back differs", path.display()));
    }
    Ok((path, series_hash(&back)))
}

// ---------------------------------------------------------------------------------------------------------------------------
// The selection rule of ADR 0677 D2
// ---------------------------------------------------------------------------------------------------------------------------

/// The screening numbers of one scan run: L1, the lift harmonic amplitude and its phase (degrees).
#[derive(Debug, Clone, Copy)]
struct Row {
    l1: f64,
    amp: f64,
    phase: f64,
}

/// The scan of one candidate: BFc at the candidate (`c`), at its bracket runs of 720 (`minus`) and 680 (`plus`) steps per
/// period of the candidate's time step, and the coarse negative control (`n1`, G20) at the candidate.
#[derive(Debug, Clone, Copy)]
struct Cand {
    f: f64,
    c: Row,
    minus: Row,
    plus: Row,
    n1: Row,
}

/// The verdict on one candidate: S0 to S4 and the conditioning measure `m_c`.
#[derive(Debug, Clone, Copy)]
struct Verdict {
    s0: bool,
    s1: bool,
    s2: bool,
    s3: bool,
    s4: bool,
    m: f64,
    dphase: f64,
}

impl Verdict {
    fn qualifies(&self) -> bool {
        self.s0 && self.s1 && self.s2 && self.s3 && self.s4
    }
}

/// S0: between the natural frequency and 10 per cent above it, with a margin of half a per cent, against `F0_731`.
fn s0(f: f64) -> bool {
    let r = f / F0_731;
    (1.005..=1.095).contains(&r)
}

fn verdict(k: &Cand) -> Verdict {
    let c = k.c;
    let s1 = c.l1 <= 0.01 && k.minus.l1 <= 0.01 && k.plus.l1 <= 0.01;
    let s2 = c.amp >= 0.75 * S731;
    let m = (k.plus.amp - k.minus.amp).abs() / c.amp;
    let dphase = wrap_deg(k.plus.phase - k.minus.phase);
    let s3 = m <= 0.20 && dphase.abs() <= 20.0;
    let s4 = wrap_deg(k.n1.phase - c.phase).abs() >= 24.0 && (k.n1.amp - c.amp).abs() / c.amp >= 0.24;
    Verdict {
        s0: s0(k.f),
        s1,
        s2,
        s3,
        s4,
        m,
        dphase,
    }
}

/// The selected candidate (index): the qualifying one with the smallest `m_c`, a tie within 1e-12 to the lower frequency
/// (the earlier in `CANDIDATES`); `None` if none qualifies.
fn select(table: &[Cand]) -> Option<usize> {
    let mut best: Option<(usize, f64)> = None;
    for (k, v) in table.iter().map(verdict).enumerate() {
        if !v.qualifies() {
            continue;
        }
        match best {
            Some((_, m)) if v.m >= m - 1e-12 => {}
            _ => best = Some((k, v.m)),
        }
    }
    best.map(|(k, _)| k)
}

// ---------------------------------------------------------------------------------------------------------------------------
// Fast tests T1 to T8
// ---------------------------------------------------------------------------------------------------------------------------

fn min_volume(mesh: &Mesh, pts: Vec<Vec3>) -> f64 {
    let m = rcfd_mesh::motion::moved_mesh(mesh, pts).unwrap();
    m.cell_volumes().iter().cloned().fold(f64::INFINITY, f64::min)
}

/// Every run specification of the scan and of the main record at every candidate: the candidate, its bracket, its fixed run.
fn all_specs() -> Vec<(Point, Spec)> {
    (0..CANDIDATES.len())
        .flat_map(|k| {
            let p = candidate(k);
            [
                (p, Spec::forced(p, STEPS_F)),
                (p, Spec::forced(p, STEPS_MINUS)),
                (p, Spec::forced(p, STEPS_PLUS)),
                (p, Spec::fixed(p)),
            ]
        })
        .collect()
}

/// ADR 0677 T1.
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
             {inflow} inflow; smallest cell volume at y_c -0.25, 0, 0.25: {}",
            m.n_cells(),
            vols.iter().map(|v| format!("{v:.3e}")).collect::<Vec<_>>().join(" ")
        );
    }
}

/// ADR 0677 T2.
#[test]
fn t2_the_specifications_and_the_measures_on_synthetic_series() {
    // every run specification of every candidate: dt bit-equal to the candidate's (1/f)/700, also for the bracket runs;
    // integer steps per period; the bracket frequencies; the fixed steps; the amplitude
    let mut e_br = 0.0f64;
    for (p, s) in all_specs() {
        let want_dt = (1.0 / p.f) / 700.0;
        assert_eq!(s.dt.to_bits(), want_dt.to_bits(), "T2 dt of {s:?}");
        assert_eq!(s.steps() % s.n_p, 0, "T2 steps of {s:?}");
        if s.amp != 0.0 {
            assert_eq!(s.motion().amplitude, [0.0, AMP, 0.0], "T2 amplitude of {s:?}");
            let want_f = p.f * STEPS_F as f64 / s.n_p as f64;
            e_br = e_br.max((s.freq() - want_f).abs() / want_f);
        } else {
            assert!(s.steps() as f64 * want_dt >= 142.857, "T2 fixed run length of {p:?}");
        }
    }
    assert!(e_br <= 1e-15, "T2 bracket frequencies {e_br}");
    for (f, want) in [(0.17, 17_500), (0.175, 17_500), (0.18, 18_200), (0.14, 14_000)] {
        assert_eq!(fixed_steps(f), want, "T2 fixed steps at f {f}");
    }
    let (mut e_rel, mut e_abs, mut e_ph) = (0.0f64, 0.0f64, 0.0f64);
    for (p, spec) in all_specs() {
        if spec.amp == 0.0 {
            continue;
        }
        let (first, last) = spec.window();
        let w = spec.omega();
        let (m, a, ph) = (1.234, 0.87, 0.6);
        let f = series(last, spec.dt, |t| {
            m + a * (w * t + ph).cos() + 0.3 * (2.0 * w * t).cos() + 0.2 * (3.0 * w * t).sin()
        });
        let ms = measures(&f.iter().map(|&y| [y, y, 0.0]).collect::<Vec<_>>(), first, last, &spec);
        e_rel = e_rel.max((ms.mean_x - m).abs() / m);
        e_rel = e_rel.max((cabs(ms.lift) - a).abs() / a);
        e_rel = e_rel.max((cabs(ms.drag2) - 0.3).abs() / 0.3);
        e_ph = e_ph.max(wrap_deg(ms.phase - (ph - PI).to_degrees()).abs());
        e_abs = e_abs.max(ms.l1).max(ms.l2).max(ms.r2);
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
        e_rel = e_rel.max(((y.0 + p.amp).abs() + y.1.abs()) / p.amp);
        // R2 of a known sixth harmonic: b / sqrt 2
        let b = 0.05;
        let g = series(last, spec.dt, |t| m + a * (w * t + ph).cos() + b * (6.0 * w * t).cos());
        let (_, r2) = lock_spectral(&g, first, last, &spec);
        e_abs = e_abs.max((r2 - b / 2f64.sqrt()).abs());
    }
    // L1 of a sinusoid at f0 = f / 0.875 against 2 |sin(pi f0 / f)| (ADR 0657 arithmetic 6), on the grid of C1
    let spec = Spec::forced(candidate(0), STEPS_F);
    let (first, last) = (1, 8 * STEPS_F);
    let f0 = spec.freq() / 0.875;
    let tc = 0.5 * (first + last) as f64 * spec.dt;
    let s = series(last, spec.dt, |t| (TAU * f0 * (t - tc)).sin());
    let l1 = lock_lagged(&s, first, last, spec.n_p);
    let l1_want = 2.0 * (PI * f0 / spec.freq()).sin().abs();
    let e_l1 = (l1 - l1_want).abs();
    // f0 of D3 on the step grid of the fixed run of C1
    let fx = Spec::fixed(candidate(0));
    let f0s = 0.165;
    let v = series(fx.steps(), fx.dt, |t| 0.4 + (TAU * f0s * t + 0.3).sin());
    let nat = natural(&v, fx.dt).unwrap();
    let e_f0 = (nat.f0 - f0s).abs() / f0s;
    // the allowance and the fixed force scale on given numbers against the closed forms
    let (xm, xp, scale, rm, rp, dr) = (1.30, 1.36, 1.33, 1.00, 1.06, -0.02);
    let al = allowance(slope(xm, xp, scale, rm, rp), dr);
    let al_want = ((xp - xm) / scale / (rp - rm) * dr).abs();
    let mut e_allow = (al - al_want).abs() / al_want;
    for (s_fix, p) in [
        (0.1776, candidate(0)),
        (0.18, candidate(2)),
        (0.2, Point { amp: 0.0, f: 0.3 }),
    ] {
        let want = s_fix + 0.25 * PI * p.amp * (2.0 * PI * p.f).powi(2);
        e_allow = e_allow.max((force_scale(s_fix, p) - want).abs() / want);
    }
    println!(
        "T2: bracket frequencies {e_br:.1e} (limit 1e-15); relative {e_rel:.1e}, L1, L2 and R2 of the periodic series \
         {e_abs:.1e} (limits 1e-12), phases {e_ph:.1e} deg (1e-9); L1 of the f0 sinusoid {l1:.12} against {l1_want:.12} \
         ({e_l1:.1e}, limit 1e-3); f0 {:.10} ({e_f0:.1e}, limit 1e-6, {} crossings); allowance and F_L {e_allow:.1e} (limit \
         1e-14); F_L with S731 at C1, C2, C3 {:.6} {:.6} {:.6}",
        nat.f0,
        nat.crossings,
        force_scale(S731, candidate(0)),
        force_scale(S731, candidate(1)),
        force_scale(S731, candidate(2))
    );
    assert!(e_rel <= 1e-12 && e_abs <= 1e-12, "T2 {e_rel} {e_abs}");
    assert!(e_ph <= 1e-9, "T2 phases {e_ph}");
    assert!(e_l1 <= 1e-3, "T2 L1 {l1} {l1_want}");
    assert!(e_f0 <= 1e-6, "T2 f0 {}", nat.f0);
    assert!(e_allow <= 1e-14, "T2 allowance, F_L {e_allow}");
}

/// ADR 0677 T3: IB smoke at the candidates, the solid cells within `1e-3 U` (the absolute limit of ADR 0671 amendment T3).
#[test]
fn t3_immersed_smoke_at_the_candidates() {
    for k in 0..CANDIDATES.len() {
        let p = candidate(k);
        let spec = Spec::forced(p, STEPS_F);
        let um = p.amp * spec.omega();
        for with_velocity in [true, false] {
            let mut c = ib_setup(H_G20, spec, with_velocity);
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
                    [0.0, p.amp * spec.omega() * (spec.omega() * t).sin(), 0.0]
                } else {
                    vec3::ZERO
                };
                for &cell in &c.s.immersed.as_ref().unwrap().bodies[0].solid_cells {
                    worst = worst.max(vec3::norm(vec3::sub(c.s.u.values[cell], ub)));
                }
            }
            assert!(c.s.u.values.iter().flatten().all(|x| x.is_finite()));
            let measured = worst / vec3::norm(U_INF);
            println!(
                "T3 C{} {p:?} (velocity in forcing {with_velocity}): max |u_c - u_b| {worst:.3e}, over U {measured:.2e} \
                 (limit 1e-3), over A omega {:.2e} (reported), max |F| {fmax:.4}",
                k + 1,
                worst / um
            );
            assert!(measured <= 1e-3, "T3 C{} {with_velocity}: {measured}", k + 1);
        }
    }
}

/// ADR 0677 T4.
#[test]
fn t4_body_fitted_smoke_at_the_candidates() {
    for k in [0, 2] {
        let mut c = bf_setup(64, 32, Spec::forced(candidate(k), STEPS_F));
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
            "T4 C{}: max continuity {cont:.2e} (limit 1e-6), cylinder centre ({:+.3e}, {:+.15}) against y_c {yc:+.15}",
            k + 1,
            mean[0],
            mean[1]
        );
        assert!(cont <= 1e-6, "T4 continuity {cont}");
        assert!(
            mean[0].abs() <= 1e-12 && (mean[1] - yc).abs() <= 1e-12,
            "T4 centre {mean:?}"
        );
        assert_eq!(far, far0, "T4 far field moved");
    }
}

/// ADR 0677 T5: the body-fitted solver starts on the grid at the initial pose (C1, C3, and the 720-step bracket run of C1).
#[test]
fn t5_body_fitted_solver_starts_at_the_initial_pose() {
    for (name, spec) in [
        ("C1", Spec::forced(candidate(0), STEPS_F)),
        ("C3", Spec::forced(candidate(2), STEPS_F)),
        ("C1-", Spec::forced(candidate(0), STEPS_MINUS)),
    ] {
        let c = bf_setup(64, 32, spec);
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
            "T5 {name}: cylinder centre ({:+.3e}, {:+.15}) against (0, -{AMP}); largest difference to the unmoved points \
             {biggest:.15}",
            mean[0], mean[1]
        );
        assert!(
            mean[0].abs() <= 1e-12 && (mean[1] + AMP).abs() <= 1e-12,
            "T5 centre {mean:?}"
        );
        assert!((biggest - AMP).abs() <= 1e-12, "T5 largest difference {biggest}");
    }
    let f = bf_setup(64, 32, Spec::fixed(candidate(0)));
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

/// ADR 0677 T6: the fixed set-ups.
#[test]
fn t6_fixed_setups_smoke() {
    let spec = Spec::fixed(candidate(0));
    let mut ib = ib_setup(H_G20, spec, true);
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

/// ADR 0677 T7: the series file round trip.
#[test]
fn t7_the_series_file_round_trip() {
    let mut force: Vec<Vec3> = vec![
        [1.0 / 3.0, -0.0, 1e-300],
        [1.234_567_890_123_456_7e17, -2.0 / 7.0, f64::MIN_POSITIVE],
        [0.1 + 0.2, -1e-17, 5e-324],
    ];
    for n in 0..500 {
        let t = n as f64 * 0.01;
        force.push([
            0.8 + 0.01 * (3.0 * t).sin(),
            0.2 * (t * 1.1).cos(),
            (t * 0.7).sin() * 1e-9,
        ]);
    }
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("f756_t7_series.txt");
    write_series(&path, &force).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert_eq!(text.lines().count(), force.len(), "T7 one line per step");
    let back = read_series(&path).unwrap();
    assert_eq!(back.len(), force.len(), "T7 length");
    let same = back
        .iter()
        .zip(&force)
        .all(|(a, b)| (0..3).all(|k| a[k].to_bits() == b[k].to_bits()));
    assert!(same, "T7 read back differs");
    assert_eq!(series_hash(&back), series_hash(&force), "T7 hash");
    println!(
        "T7: {} steps written and read back bit for bit, hash {:016x}",
        force.len(),
        series_hash(&force)
    );
}

/// The synthetic scan table of T8 (hand values: m_c of C1 0.04 / 0.30 = 0.1333, C2 0.02 / 0.35 = 0.0571, C3 0.06 / 0.40 =
/// 0.15; the control 60, 50, 40 deg off; every candidate qualifies).
fn t8_table() -> Vec<Cand> {
    let row = |amp: f64, phase: f64| Row { l1: 1e-6, amp, phase };
    let cand = |k: usize, c: Row, minus: Row, plus: Row| Cand {
        f: CANDIDATES[k],
        c,
        minus,
        plus,
        n1: row(1.0, 0.0),
    };
    vec![
        cand(0, row(0.30, 60.0), row(0.28, 65.0), row(0.32, 55.0)),
        cand(1, row(0.35, 50.0), row(0.34, 55.0), row(0.36, 45.0)),
        cand(2, row(0.40, 40.0), row(0.37, 45.0), row(0.43, 35.0)),
    ]
}

/// ADR 0677 T8: the selection rule on synthetic tables.
#[test]
fn t8_the_selection_rule_on_synthetic_tables() {
    // S0 on frequencies
    for (f, want) in [
        (0.165, false),
        (0.17, true),
        (0.175, true),
        (0.18, true),
        (0.181, false),
    ] {
        assert_eq!(s0(f), want, "T8 S0 at f {f}");
    }
    // all qualify: the smallest m_c (C2)
    let t = t8_table();
    assert!(t.iter().map(verdict).all(|v| v.qualifies()), "T8 base table");
    assert_eq!(select(&t), Some(1), "T8 base");
    // a tie (C1 with the amplitudes of C2): the lower frequency
    let mut tt = t8_table();
    tt[0].c.amp = tt[1].c.amp;
    tt[0].minus.amp = tt[1].minus.amp;
    tt[0].plus.amp = tt[1].plus.amp;
    assert_eq!(select(&tt), Some(0), "T8 tie");
    // S1: a bracket run of C2 not locked (L1 0.011), the lower and the upper one: C1
    for side in 0..2 {
        let mut t1 = t8_table();
        if side == 0 {
            t1[1].minus.l1 = 0.011;
        } else {
            t1[1].plus.l1 = 0.011;
        }
        assert!(!verdict(&t1[1]).s1, "T8 S1 side {side}");
        assert_eq!(select(&t1), Some(0), "T8 S1 pick side {side}");
    }
    // S2: the harmonic of C2 just below 0.75 S731 (0.133, m_c kept): C1; just above (0.1333): C2
    let set = |t: &mut Vec<Cand>, a: f64| {
        t[1].c.amp = a;
        t[1].minus.amp = a * 0.34 / 0.35;
        t[1].plus.amp = a * 0.36 / 0.35;
    };
    let mut t2 = t8_table();
    set(&mut t2, 0.133);
    assert!(!verdict(&t2[1]).s2, "T8 S2");
    assert_eq!(select(&t2), Some(0), "T8 S2 pick");
    set(&mut t2, 0.1333);
    assert_eq!(select(&t2), Some(1), "T8 S2 above");
    // S3 amplitude: the bracket of C2 changes by 0.21 of |Lhat_c|, mostly on the lower side: C1
    let mut t3 = t8_table();
    t3[1].c.amp = 0.30;
    t3[1].minus.amp = 0.243;
    t3[1].plus.amp = 0.306;
    assert!(!verdict(&t3[1]).s3, "T8 S3 amplitude");
    assert_eq!(select(&t3), Some(0), "T8 S3 amplitude pick");
    // S3 phase: the bracket of C2 changes by 21 deg: C1
    let mut t3p = t8_table();
    t3p[1].minus.phase = 50.0 + 10.5;
    t3p[1].plus.phase = 50.0 - 10.5;
    assert!(!verdict(&t3p[1]).s3, "T8 S3 phase");
    assert_eq!(select(&t3p), Some(0), "T8 S3 phase pick");
    // S4: the coarse negative control at C2 only 23 deg off: C1
    let mut t4 = t8_table();
    t4[1].n1.phase = 50.0 - 23.0;
    assert!(!verdict(&t4[1]).s4, "T8 S4");
    assert_eq!(select(&t4), Some(0), "T8 S4 pick");
    // S0: a table whose best candidate sits at 0.165 (outside the margin): C1
    let mut t0 = t8_table();
    t0[1].f = 0.165;
    assert!(!verdict(&t0[1]).s0, "T8 S0 table");
    assert_eq!(select(&t0), Some(0), "T8 S0 pick");
    // none qualifies: the negative control everywhere in phase with the reference
    let mut t5 = t8_table();
    for k in &mut t5 {
        k.n1.phase = k.c.phase;
    }
    assert_eq!(select(&t5), None, "T8 none");
    let m: Vec<f64> = t.iter().map(|k| verdict(k).m).collect();
    println!(
        "T8: m_c of the base table {m:.4?}; S0, base, tie, S1 (both sides), S2, S3 (amplitude, phase), S4, none as expected"
    );
}

// ---------------------------------------------------------------------------------------------------------------------------
// The heavy records (ADR 0677 D2, D3)
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

type Job = Box<dyn FnOnce() -> Run + Send>;

fn run_all(jobs: Vec<(String, Job)>) -> Vec<Run> {
    std::thread::scope(|sc| {
        let hs: Vec<_> = jobs
            .into_iter()
            .map(|(name, job)| {
                std::thread::Builder::new()
                    .name(name)
                    .stack_size(64 << 20)
                    .spawn_scoped(sc, job)
                    .unwrap()
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    })
}

/// S1 and K1 for every run: prints, writes and reads back the series; the failures.
fn stop_and_keep(stage: &str, runs: &[Run]) -> Vec<String> {
    let mut fails = Vec::new();
    for r in runs {
        let kept = keep_series(stage, &r.label, &r.force);
        println!(
            "S1 [{}]: {} cells, {} steps, {:.0} s, max continuity {:.3e}, max Courant {:.3}, stopped {:?}; K1 {}",
            r.label,
            r.cells,
            r.force.len(),
            r.seconds,
            r.max_continuity,
            r.max_courant,
            r.stopped,
            match &kept {
                Ok((p, h)) => format!("series {} read back bit for bit, hash {h:016x}", p.display()),
                Err(e) => format!("FAILED {e}"),
            }
        );
        if r.stopped.is_some() {
            fails.push(format!("S1 {} stopped: {:?}", r.label, r.stopped));
        }
        if let Err(e) = kept {
            fails.push(format!("K1 {}: {e}", r.label));
        }
    }
    fails
}

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

fn print_forced(l: &str, r: &Run) -> Measures {
    let (w0, w1) = r.spec.window();
    let m = measures(&r.force, w0, w1, &r.spec);
    let (ca, cv) = lift_parts(m.lift, w0, w1, &r.spec);
    println!(
        "[{l}] A {} f {:.6}: mean drag {:.6}, lift harmonic ({:+.6}, {:+.6}) amplitude {:.6} phase {:+.3} deg, mean lift {:+.3e}; \
         L1 {:.3e}, L2 {:.3e}, R2 {:.3e}; drag second harmonic {:.4e}; C_a {:.4}, C_v {:.4} (reported)",
        r.spec.amp,
        r.spec.freq(),
        m.mean_x,
        m.lift.0,
        m.lift.1,
        cabs(m.lift),
        m.phase,
        m.mean_y,
        m.l1,
        m.l2,
        m.r2,
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
    m
}

/// The labels of the scan runs of candidate `k`: BFc at the candidate, at its bracket runs, and the coarse control.
fn scan_labels(k: usize) -> [String; 4] {
    let f = CANDIDATES[k];
    [
        format!("BFc-f{f:.3}"),
        format!("BFc-f{f:.3}-m720"),
        format!("BFc-f{f:.3}-p680"),
        format!("N1g-C{}", k + 1),
    ]
}

/// The F-750 series and their hashes (ADR 0671 amendment 1), reused as data by SR.
const F750_SERIES: [(&str, &str, Option<f64>); 12] = [
    ("scan_BFc_A50_f0.17.txt", "383ecfcb802eea27", None),
    ("scan_BFc_A50_f0.18.txt", "232e417fbbe7b96a", None),
    ("scan_BFc_A50_f0.19.txt", "b68fe1109cbb771f", None),
    ("scan_BFc_A50_f0.20.txt", "9e0e5ba41560dfe5", None),
    ("scan_BFc_A25_f0.13.txt", "f2d02cf1e90eeef9", Some(0.13)),
    ("scan_BFc_A25_f0.14.txt", "5957d2499f7d31bd", Some(0.14)),
    ("scan_BFc_A25_f0.15.txt", "969cbe4ca2528289", Some(0.15)),
    ("scan_BFc_A25_f0.16.txt", "b9c5ea2e1b480351", Some(0.16)),
    ("scan_N1g_C1.txt", "298ba6edc9914642", None),
    ("scan_N1g_C2.txt", "b8d6c9f778120efe", None),
    ("scan_N1g_C3.txt", "d2e10dd0e774316b", None),
    ("scan_N1g_C4.txt", "c81d057c1ac63189", None),
];

/// SR (reported): the reused F-750 series hashed against ADR 0671 amendment 1, the A/D 0.25 runs measured from the file.
fn reuse_f750() {
    let Some(dir) = std::env::var_os("RCFD_F756_F750_SERIES") else {
        println!("SR: RCFD_F756_F750_SERIES is not set; the F-750 series are not read (reported)");
        return;
    };
    let dir = PathBuf::from(dir);
    for (name, want, f) in F750_SERIES {
        let path = dir.join(name);
        match read_series(&path) {
            Err(e) => println!("SR {name}: not read ({e}); not used"),
            Ok(force) => {
                let h = format!("{:016x}", series_hash(&force));
                let ok = h == want;
                println!(
                    "SR {name}: {} steps, hash {h} against ADR 0671 {want}: {}",
                    force.len(),
                    if ok { "equal" } else { "DIFFERS, not used" }
                );
                if let (true, Some(f)) = (ok, f) {
                    let spec = Spec::forced(Point { amp: AMP, f }, STEPS_F);
                    if force.len() == spec.steps() {
                        let r = Run {
                            label: format!("F-750 BFc-A25-f{f:.2}"),
                            spec,
                            force,
                            max_continuity: 0.0,
                            max_courant: 0.0,
                            stopped: None,
                            seconds: 0.0,
                            cells: 0,
                        };
                        let m = print_forced(&r.label, &r);
                        println!(
                            "    SR f {f:.2}: f / f0_731 {:.4}, |Lhat| / S731 {:.4} (reported, not used by the rule)",
                            f / F0_731,
                            cabs(m.lift) / S731
                        );
                    }
                }
            }
        }
    }
}

/// ADR 0677 D2: stage S, the registered scan (run once). Twelve runs: BFc at each candidate and its two bracket runs, N1 on G20
/// at each candidate; SR; the selection rule.
#[test]
#[ignore = "heavy record (ADR 0677 stage S): twelve runs, about 1.2 to 2 h in release; run once"]
fn f756_scan_record() {
    let t0 = Instant::now();
    reuse_f750();
    let mut jobs: Vec<(String, Job)> = Vec::new();
    for k in 0..CANDIDATES.len() {
        let p = candidate(k);
        let labels = scan_labels(k);
        for (j, n_p) in [STEPS_F, STEPS_MINUS, STEPS_PLUS].into_iter().enumerate() {
            let spec = Spec::forced(p, n_p);
            let l = labels[j].clone();
            jobs.push((l.clone(), Box::new(move || run_bf(&l, 128, 64, spec))));
        }
        let spec = Spec::forced(p, STEPS_F);
        let l = labels[3].clone();
        jobs.push((l.clone(), Box::new(move || run_ib(&l, H_G20, spec, false))));
    }
    let runs = run_all(jobs);
    println!("all scan runs done in {:.0} s", t0.elapsed().as_secs_f64());
    let fails = stop_and_keep("scan", &runs);
    if !fails.is_empty() {
        panic!("SS1: {fails:#?}");
    }
    let get = |l: &str| runs.iter().find(|r| r.label == l).unwrap();
    let to_row = |m: &Measures| Row {
        l1: m.l1,
        amp: cabs(m.lift),
        phase: m.phase,
    };
    let mut table = Vec::new();
    for k in 0..CANDIDATES.len() {
        let p = candidate(k);
        let rows: Vec<Row> = scan_labels(k)
            .iter()
            .map(|l| {
                let m = print_forced(l, get(l));
                let row = to_row(&m);
                println!(
                    "    {l}: |Lhat| / S731 {:.4}, |Lhat| / F_L(S731) {:.4}",
                    row.amp / S731,
                    row.amp / force_scale(S731, p)
                );
                row
            })
            .collect();
        table.push(Cand {
            f: p.f,
            c: rows[0],
            minus: rows[1],
            plus: rows[2],
            n1: rows[3],
        });
    }
    for (k, c) in table.iter().enumerate() {
        let v = verdict(c);
        println!(
            "C{} f {}: S0 {} (f / f0_731 {:.4}); S1 {} (L1 {:.2e}, bracket {:.2e} {:.2e}); S2 {} (|Lhat| {:.5}, limit {:.6}); \
             S3 {} (bracket amplitude change m_c {:.4}, limit 0.20; phase change {:+.2} deg, limit 20; amplitudes {:.5} {:.5}); \
             S4 {} (N1 phase off {:+.2} deg, amplitude off {:.3}); qualifies {}",
            k + 1,
            c.f,
            v.s0,
            c.f / F0_731,
            v.s1,
            c.c.l1,
            c.minus.l1,
            c.plus.l1,
            v.s2,
            c.c.amp,
            0.75 * S731,
            v.s3,
            v.m,
            v.dphase,
            c.minus.amp,
            c.plus.amp,
            v.s4,
            wrap_deg(c.n1.phase - c.c.phase),
            (c.n1.amp - c.c.amp).abs() / c.c.amp,
            v.qualifies()
        );
    }
    match select(&table) {
        Some(k) => println!("SELECTED: C{} {:?}", k + 1, candidate(k)),
        None => println!("SELECTED: none qualifies; no main record (ADR 0677 D2)"),
    }
}

/// ADR 0677 D3: stage M, the registered main record at the selected point (run once).
#[test]
#[ignore = "heavy record (ADR 0677 stage M): nine runs, about 4 to 8 h in release; run once"]
fn f756_lockin_record() {
    let pt = SELECTED.expect("ADR 0677 amendment 1 (the selected point) is not written");
    let t0 = Instant::now();
    let (sf, sm, sp, sx) = (
        Spec::forced(pt, STEPS_F),
        Spec::forced(pt, STEPS_MINUS),
        Spec::forced(pt, STEPS_PLUS),
        Spec::fixed(pt),
    );
    let jobs: Vec<(String, Job)> = vec![
        ("BF0".into(), Box::new(move || run_bf("BF0", 256, 128, sx))),
        ("BFc0".into(), Box::new(move || run_bf("BFc0", 128, 64, sx))),
        ("IB0".into(), Box::new(move || run_ib("IB0", H_G40, sx, true))),
        ("BF".into(), Box::new(move || run_bf("BF", 256, 128, sf))),
        ("BF-".into(), Box::new(move || run_bf("BF-", 256, 128, sm))),
        ("BF+".into(), Box::new(move || run_bf("BF+", 256, 128, sp))),
        ("BFc".into(), Box::new(move || run_bf("BFc", 128, 64, sf))),
        ("IB".into(), Box::new(move || run_ib("IB", H_G40, sf, true))),
        ("N1".into(), Box::new(move || run_ib("N1", H_G40, sf, false))),
    ];
    let runs = run_all(jobs);
    println!("all runs done in {:.0} s, point {pt:?}", t0.elapsed().as_secs_f64());
    let mut fails = stop_and_keep("main", &runs);
    if !fails.is_empty() {
        panic!("{fails:#?}");
    }
    let get = |l: &str| runs.iter().find(|r| r.label == l).unwrap();
    let mut checks: Vec<(String, bool)> = Vec::new();

    // K2 (reported): the main BFc against the scan's run of the same specification
    let k = CANDIDATES.iter().position(|&f| f == pt.f).unwrap();
    let scan_path = series_dir().join(format!("scan_{}.txt", scan_labels(k)[0].replace('-', "_")));
    let main_hash = series_hash(&get("BFc").force);
    match read_series(&scan_path) {
        Ok(s) => println!(
            "K2: main BFc hash {main_hash:016x}, scan {} hash {:016x}: {} (reported)",
            scan_path.display(),
            series_hash(&s),
            if series_hash(&s) == main_hash {
                "equal"
            } else {
                "DIFFERENT"
            }
        ),
        Err(e) => println!("K2: the scan series is not read ({e}) (reported)"),
    }

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
                nat.insert(l, n);
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
    let (f0_bf, f0_c, f0_ib) = (nat["BF0"].f0, nat["BFc0"].f0, nat["IB0"].f0);
    let s_fix = nat["BF0"].amp_halves.1;
    let fl = force_scale(s_fix, pt);
    let d_ib = f0_ib / f0_bf - 1.0;
    let d_c = f0_c / f0_bf - 1.0;
    let f = sf.freq();
    let (r_bf, r_ib, r_c) = (f / f0_bf, f / f0_ib, f / f0_c);
    let (r_m, r_p) = (sm.freq() / f0_bf, sp.freq() / f0_bf);
    println!(
        "F0: f0_IB / f0_BF - 1 = {d_ib:+.4e}, f0_BFc / f0_BF - 1 = {d_c:+.4e} (limit 0.045 each); \
         F2: r_BF {r_bf:.5}, r_IB {r_ib:.5}, r_BFc {r_c:.5}, bracket {r_m:.5} {r_p:.5} (reported); \
         S = {s_fix:.6}, F_L = S + m_a A omega^2 = {fl:.6}"
    );
    println!("G0: r_BF = f / f0_BF = {r_bf:.5} (must lie in [1.0, 1.1])");
    checks.push(("G0".into(), (1.0..=1.1).contains(&r_bf)));
    checks.push(("F0 IB".into(), d_ib.abs() <= 0.045));
    checks.push(("F0 BFc".into(), d_c.abs() <= 0.045));

    // forced runs
    let mut ms = std::collections::HashMap::new();
    for l in ["BF", "BF-", "BF+", "BFc", "IB", "N1"] {
        let m = print_forced(l, get(l));
        if l != "N1" {
            checks.push((format!("L1 {l}"), m.l1 <= 0.03));
        }
        if l.starts_with("BF") {
            checks.push((format!("L2 {l}"), m.l2 <= 0.05));
        }
        ms.insert(l, m);
    }
    let (mb, mm, mp, mc, mi, mn) = (ms["BF"], ms["BF-"], ms["BF+"], ms["BFc"], ms["IB"], ms["N1"]);
    let lb = cabs(mb.lift);
    println!(
        "L2I: IB L2 {:.4e} (limit 0.10), R2 / F_L {:.4e} (limit 0.03); R2 / S {:.4e} (reported)",
        mi.l2,
        mi.r2 / fl,
        mi.r2 / s_fix
    );
    checks.push(("L2I relative".into(), mi.l2 <= 0.10));
    checks.push(("L2I fixed scale".into(), mi.r2 / fl <= 0.03));

    // G1
    println!(
        "G1: |Lhat_BF| / S = {:.4} (must be at least 0.5); |Lhat_BF| / F_L = {:.4}",
        lb / s_fix,
        lb / fl
    );
    checks.push(("G1".into(), lb / s_fix >= 0.5));

    // J1
    let j_phase = wrap_deg(mp.phase - mm.phase).abs();
    let j_amp = (cabs(mp.lift) - cabs(mm.lift)).abs() / lb;
    println!(
        "J1: phase change across the bracket {j_phase:.3} deg (limit 25), amplitude change {j_amp:.4e} (limit 0.25)"
    );
    checks.push(("J1 phase".into(), j_phase <= 25.0));
    checks.push(("J1 amplitude".into(), j_amp <= 0.25));

    // slopes and allowances (ADR 0657 D4, ADR 0671 D3)
    let w_r = r_p - r_m;
    let s_d = slope(mm.mean_x, mp.mean_x, mb.mean_x, r_m, r_p);
    let s_a = slope(cabs(mm.lift), cabs(mp.lift), lb, r_m, r_p);
    let s_al = slope(cabs(mm.lift), cabs(mp.lift), fl, r_m, r_p);
    let s_p = wrap_deg(mp.phase - mm.phase) / w_r;
    let s_c = cabs(csub(mp.lift, mm.lift)) / (lb * w_r);
    let s_cl = cabs(csub(mp.lift, mm.lift)) / (fl * w_r);
    let dr = r_ib - r_bf;
    let dr_c = r_c - r_bf;
    let lim_a1 = 0.04 + allowance(s_d, dr);
    let lim_a2 = 0.08 + allowance(s_a, dr);
    let lim_a2l = 0.06 + allowance(s_al, dr);
    let lim_a3 = 8.0 + allowance(s_p, dr);
    let lim_a4 = 0.12 + allowance(s_c, dr);
    let lim_a4l = 0.08 + allowance(s_cl, dr);
    let lim_r1d = 0.03 + allowance(s_d, dr_c);
    let lim_r1l = 0.08 + allowance(s_c, dr_c);
    let lim_r1ll = 0.05 + allowance(s_cl, dr_c);
    println!(
        "D4: slopes s_D {s_d:+.4}, s_A {s_a:+.4}, s_A^L {s_al:+.4}, s_P {s_p:+.3} deg, s_C {s_c:.4}, s_C^L {s_cl:.4}; \
         Delta r {dr:+.5}, Delta r_c {dr_c:+.5}; limits A1 {lim_a1:.4}, A2 {lim_a2:.4}, A2L {lim_a2l:.4}, A3 {lim_a3:.3} deg, \
         A4 {lim_a4:.4}, A4L {lim_a4l:.4}, R1 {lim_r1d:.4} {lim_r1l:.4} {lim_r1ll:.4}"
    );
    let drag_rel = |m: &Measures| (m.mean_x - mb.mean_x).abs() / mb.mean_x.abs();
    let amp_abs = |m: &Measures| (cabs(m.lift) - lb).abs();
    let phase_diff = |m: &Measures| wrap_deg(m.phase - mb.phase).abs();
    let complex_abs = |m: &Measures| cabs(csub(m.lift, mb.lift));
    let (a1, a3) = (drag_rel(&mi), phase_diff(&mi));
    let (a2, a2l, a4, a4l) = (
        amp_abs(&mi) / lb,
        amp_abs(&mi) / fl,
        complex_abs(&mi) / lb,
        complex_abs(&mi) / fl,
    );
    let a5 = (mi.mean_y - mb.mean_y).abs() / fl;
    let (r1d, r1l, r1ll) = (drag_rel(&mc), complex_abs(&mc) / lb, complex_abs(&mc) / fl);
    let (n1a, n1b, n1bl, n1c, n1d, n1dl) = (
        drag_rel(&mn),
        amp_abs(&mn) / lb,
        amp_abs(&mn) / fl,
        phase_diff(&mn),
        complex_abs(&mn) / lb,
        complex_abs(&mn) / fl,
    );
    // reported: the IB against the reference interpolated linearly to r_IB, and the lift numbers against S alone
    let t = (r_ib - r_m) / w_r;
    let lerp = |a: f64, b: f64| a + t * (b - a);
    let bi_drag = lerp(mm.mean_x, mp.mean_x);
    let bi_lift = (lerp(mm.lift.0, mp.lift.0), lerp(mm.lift.1, mp.lift.1));
    println!(
        "reported: IB against BF interpolated to r_IB: mean drag {:.4e}, lift amplitude {:.4e}, complex {:.4e}; \
         against S: amplitude {:.4e}, complex {:.4e}, R1 complex {:.4e}",
        (mi.mean_x - bi_drag).abs() / bi_drag,
        (cabs(mi.lift) - cabs(bi_lift)).abs() / cabs(bi_lift),
        cabs(csub(mi.lift, bi_lift)) / cabs(bi_lift),
        amp_abs(&mi) / s_fix,
        complex_abs(&mi) / s_fix,
        complex_abs(&mc) / s_fix
    );
    println!("A1 mean drag {a1:.4e} (limit {lim_a1:.4e})");
    println!("A2 lift amplitude relative {a2:.4e} (limit {lim_a2:.4e})");
    println!("A2L lift amplitude against F_L {a2l:.4e} (limit {lim_a2l:.4e})");
    println!("A3 lift phase {a3:.4} deg (limit {lim_a3:.4})");
    println!("A4 lift harmonic complex relative {a4:.4e} (limit {lim_a4:.4e})");
    println!("A4L lift harmonic complex against F_L {a4l:.4e} (limit {lim_a4l:.4e})");
    println!("A5 mean lift against F_L {a5:.4e} (limit 0.02)");
    println!(
        "R1 BFc against BF: mean drag {r1d:.4e} (limit {lim_r1d:.4e}), lift harmonic relative {r1l:.4e} (limit \
         {lim_r1l:.4e}), against F_L {r1ll:.4e} (limit {lim_r1ll:.4e})"
    );
    println!(
        "N1 velocity left out: lift amplitude {n1b:.4e} (must exceed {lim_a2:.4e}) and {n1bl:.4e} of F_L (must exceed \
         {lim_a2l:.4e}), lift phase {n1c:.3} deg (must exceed {lim_a3:.3}), lift harmonic {n1d:.4e} (must exceed \
         {lim_a4:.4e}) and {n1dl:.4e} of F_L (must exceed {lim_a4l:.4e}); mean drag {n1a:.4e} (reported); L1 {:.3e}, L2 \
         {:.3e} (reported)",
        mn.l1, mn.l2
    );
    checks.push(("A1".into(), a1 <= lim_a1));
    checks.push(("A2".into(), a2 <= lim_a2));
    checks.push(("A2L".into(), a2l <= lim_a2l));
    checks.push(("A3".into(), a3 <= lim_a3));
    checks.push(("A4".into(), a4 <= lim_a4));
    checks.push(("A4L".into(), a4l <= lim_a4l));
    checks.push(("A5".into(), a5 <= 0.02));
    checks.push(("R1 drag".into(), r1d <= lim_r1d));
    checks.push(("R1 lift relative".into(), r1l <= lim_r1l));
    checks.push(("R1 lift fixed scale".into(), r1ll <= lim_r1ll));
    checks.push(("N1b".into(), n1b > lim_a2 && n1bl > lim_a2l));
    checks.push(("N1c".into(), n1c > lim_a3));
    checks.push(("N1d".into(), n1d > lim_a4 && n1dl > lim_a4l));
    for (id, ok) in &checks {
        println!("{id}: {}", if *ok { "met" } else { "MISSED" });
        if !ok {
            fails.push(id.clone());
        }
    }
    assert!(fails.is_empty(), "missed: {fails:?}");
}
