//! F-251 (ADR 0250): strong coupling of a rigid body with the moving-mesh flow step (`rcfd_fv::fsi`), on the spring-mounted
//! piston of ADR 0211 at added-mass to body-mass ratios up to 20, against `omega = sqrt(k/(M + rho A L0))`, with the loose
//! coupling of ADR 0033 D7 as the control.
//!
//! The limits, set-ups, model arithmetic and mutations were registered in ADR 0250 before this file existed. The channel,
//! the crossings and the turning points are those of `sixdof_flow_f244.rs` (copied, that file is not edited).

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::ale::{move_mesh, set_follow_conditions};
use rcfd_fv::bc::{FixedValue, ZeroGradient};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::fsi::{
    relaxation_from_added_mass, strong_step, BodyCoupling, CouplingStep, Relaxation, Relaxer, StrongCoupling,
};
use rcfd_fv::pimple::{CorrectorControl, PimpleSettings, PimpleSolver};
use rcfd_fv::sixdof::{patch_force_moment, Body, BodySpec, BodyState, Integrator};
use rcfd_fv::smoothing::{smooth_displacement, PatchClass, SmoothingMethod};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{box_mesh, Grading};
use rcfd_mesh::motion::{cell_swept_sum, moved_mesh, patch_points};
use rcfd_mesh::{Mesh, PatchKind};
use std::f64::consts::TAU;
use std::sync::Arc;

const RHO: f64 = 1.0;
const SP_A: f64 = 0.04;
const SP_L0: f64 = 1.0;
const SP_X: f64 = 0.01;
const SP_NU: f64 = 0.01;
/// Added mass of the column, `rho A L0`.
const M_A: f64 = RHO * SP_A * SP_L0;

fn tight(max_iter: usize) -> SolveControl {
    SolveControl {
        tol_abs: 1e-16,
        tol_rel: 1e-13,
        max_iter,
    }
}

/// Piston mass at the mass ratio `mu`.
fn mass(mu: f64) -> f64 {
    M_A / mu
}

/// Spring of period 1 at the mass ratio `mu`.
fn stiffness(mu: f64) -> f64 {
    TAU * TAU * (mass(mu) + M_A)
}

fn sp_settings() -> PimpleSettings {
    let mut s = PimpleSettings::piso(SP_NU);
    s.convection = ConvScheme::Central;
    s.momentum_solver = tight(5000);
    s.pressure_solver = tight(5000);
    s
}

/// The C2 channel of ADR 0033 with its smoothing classes (SP of ADR 0211).
struct Sp {
    mesh0: Mesh,
    g0: FvGeometry,
    classes: Vec<PatchClass>,
    piston: Vec<usize>,
}

impl Sp {
    fn new() -> Self {
        let mesh0 = box_mesh([20, 2, 2], [SP_L0, 0.2, 0.2], [Grading::Uniform; 3]).unwrap();
        let g0 = FvGeometry::new(Arc::new(mesh0.clone())).unwrap();
        let classes = mesh0
            .patches()
            .iter()
            .map(|pt| match pt.name.as_str() {
                "xmin" => PatchClass::Prescribed,
                "xmax" => PatchClass::Fixed,
                _ => PatchClass::Slip,
            })
            .collect();
        let piston = patch_points(&mesh0, "xmin").unwrap();
        Sp {
            mesh0,
            g0,
            classes,
            piston,
        }
    }

    /// Points with the piston at `x_p` (Laplacian smoothing from the initial mesh).
    fn points(&self, x_p: f64) -> Vec<Vec3> {
        let mut presc = vec![vec3::ZERO; self.mesh0.n_points()];
        for &q in &self.piston {
            presc[q] = [x_p, 0.0, 0.0];
        }
        let sm = smooth_displacement(
            &self.g0,
            &self.classes,
            &presc,
            SmoothingMethod::Laplacian,
            &tight(5000),
        )
        .unwrap();
        self.mesh0
            .points()
            .iter()
            .zip(&sm.displacement)
            .map(|(p, d)| vec3::add(*p, *d))
            .collect()
    }

    fn geom_at(&self, x_p: f64) -> Arc<FvGeometry> {
        let mesh = moved_mesh(&self.mesh0, self.points(x_p)).unwrap();
        Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap())
    }

    /// Fluid at rest on the mesh with the piston at `X`.
    fn flow(&self) -> PimpleSolver {
        let geom = self.geom_at(SP_X);
        let mut u = VectorField::uniform("U", &geom, vec3::ZERO);
        for name in ["ymin", "ymax", "zmin", "zmax"] {
            u.set_bc(&geom, name, Box::new(ZeroGradient)).unwrap();
        }
        let mut p = ScalarField::uniform("p", &geom, 0.0);
        p.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
        PimpleSolver::new(geom, u, p, sp_settings())
    }

    fn mean_volume(&self) -> f64 {
        self.mesh0.total_volume() / self.mesh0.n_cells() as f64
    }
}

/// The piston at rest at `x = X` on its spring.
fn body(mu: f64) -> Body {
    let mut spec = BodySpec::new(mass(mu), [1.0, 1.0, 1.0]);
    spec.translation_locked = [false, true, true];
    spec.stiffness = [stiffness(mu), 0.0, 0.0];
    Body::new(spec, BodyState::at([SP_X, 0.0, 0.0])).unwrap()
}

fn piston() -> Vec<String> {
    vec!["xmin".to_string()]
}

fn coupling(rho: f64) -> BodyCoupling {
    BodyCoupling {
        rho,
        nu: SP_NU,
        force_patches: piston(),
        follow_patches: piston(),
        integrator: Integrator::Rk4,
    }
}

/// The registered strong settings (ADR 0250): Aitken from the added-mass factor, tolerance `1e-8`, floor `k X`, cap 20.
fn registered(mu: f64) -> StrongCoupling {
    StrongCoupling {
        relaxation: Relaxation::Aitken {
            initial: relaxation_from_added_mass(mass(mu), M_A),
            max: 2.0,
        },
        tolerance: 1e-8,
        force_scale: stiffness(mu) * SP_X,
        moment_scale: 0.0,
        max_iterations: 20,
    }
}

/// Discrete GCL residual of a mesh step: `max_c |sum_f sigma_f W_f - (V^{n+1} - V^n)|` (ADR 0033 G1).
fn gcl_residual(old: &Mesh, new: &Mesh, mesh_flux: &[f64], dt: f64) -> f64 {
    let w: Vec<f64> = mesh_flux.iter().map(|m| m * dt).collect();
    cell_swept_sum(old, &w)
        .iter()
        .zip(old.cell_volumes().iter().zip(new.cell_volumes()))
        .map(|(s, (v0, v1))| (s - (v1 - v0)).abs())
        .fold(0.0, f64::max)
}

/// Upward zero crossings by linear interpolation (ADR 0033 R2).
fn upward_crossings(t: &[f64], x: &[f64]) -> Vec<f64> {
    (1..x.len())
        .filter(|&i| x[i - 1] < 0.0 && x[i] >= 0.0)
        .map(|i| t[i - 1] + (t[i] - t[i - 1]) * (-x[i - 1]) / (x[i] - x[i - 1]))
        .collect()
}

/// `omega_m = 2 pi (N - 1)/(t_N - t_1)` over all upward crossings, and their number.
fn measured_omega(t: &[f64], x: &[f64]) -> (f64, usize) {
    let c = upward_crossings(t, x);
    assert!(c.len() >= 2, "fewer than two upward crossings");
    (TAU * (c.len() - 1) as f64 / (c[c.len() - 1] - c[0]), c.len())
}

/// Turning points refined by the parabola through three samples; the start (at rest) is the first maximum.
fn turning_points(x: &[f64]) -> (Vec<f64>, Vec<f64>) {
    let mut maxima = vec![x[0]];
    let mut minima = Vec::new();
    for i in 1..x.len() - 1 {
        let (a, b, c) = (x[i - 1], x[i], x[i + 1]);
        let curv = a - 2.0 * b + c;
        if b > a && b >= c && curv != 0.0 {
            maxima.push(b - (c - a).powi(2) / (8.0 * curv));
        } else if b < a && b <= c && curv != 0.0 {
            minima.push(b - (c - a).powi(2) / (8.0 * curv));
        }
    }
    (maxima, minima)
}

/// Largest ratio of successive maxima and of the magnitudes of successive minima, and the numbers of each.
fn worst_growth(x: &[f64]) -> (f64, usize, usize) {
    let (mx, mn) = turning_points(x);
    let mut worst: f64 = 0.0;
    for w in mx.windows(2) {
        worst = worst.max(w[1] / w[0]);
    }
    for w in mn.windows(2) {
        worst = worst.max(w[1].abs() / w[0].abs());
    }
    (worst, mx.len(), mn.len())
}

/// Growth per step of a force sequence between `C_8` and `C_20`, `C_n = F_{n+1}^2 - F_n F_{n+2}` (ADR 0250 D3).
fn casoratian_growth(f: &[f64]) -> f64 {
    let c = |i: usize| (f[i + 1] * f[i + 1] - f[i] * f[i + 2]).abs();
    (c(20) / c(8)).powf(1.0 / 24.0)
}

/// Result of a strongly coupled run.
struct StrongRun {
    t: Vec<f64>,
    x: Vec<f64>,
    iterations: Vec<usize>,
    last_residual: f64,
    all_converged: bool,
    cont: f64,
    gcl: f64,
    p_iters: usize,
    first_factors: Vec<Vec<f64>>,
}

impl StrongRun {
    fn max_iterations(&self) -> usize {
        self.iterations.iter().copied().max().unwrap_or(0)
    }
    fn mean_iterations(&self) -> f64 {
        self.iterations.iter().sum::<usize>() as f64 / self.iterations.len() as f64
    }
    fn histogram(&self) -> Vec<usize> {
        let mut h = vec![0; self.max_iterations() + 1];
        for &i in &self.iterations {
            h[i] += 1;
        }
        h
    }
    fn max_abs_x(&self) -> f64 {
        self.x.iter().fold(0.0f64, |a, b| a.max(b.abs()))
    }
}

/// One strong step of the piston on `sp`.
fn strong(
    sp: &Sp,
    flow: &mut PimpleSolver,
    body: &mut Body,
    dt: f64,
    cpl: &BodyCoupling,
    settings: &StrongCoupling,
) -> Result<CouplingStep, String> {
    strong_step(flow, body, dt, cpl, settings, &mut |b: &Body| {
        Ok(sp.points(b.state.x[0]))
    })
}

fn strong_run(
    sp: &Sp,
    mu: f64,
    steps_per_period: usize,
    periods: f64,
    settings: &StrongCoupling,
    rho: f64,
) -> StrongRun {
    let dt = 1.0 / steps_per_period as f64;
    let mut flow = sp.flow();
    let mut body = body(mu);
    let cpl = coupling(rho);
    let mut run = StrongRun {
        t: vec![0.0],
        x: vec![SP_X],
        iterations: Vec::new(),
        last_residual: 0.0,
        all_converged: true,
        cont: 0.0,
        gcl: 0.0,
        p_iters: 0,
        first_factors: Vec::new(),
    };
    let steps = (periods * steps_per_period as f64).round() as usize;
    for n in 1..=steps {
        let old = flow.geom.mesh.clone();
        let info = strong(sp, &mut flow, &mut body, dt, &cpl, settings).unwrap_or_else(|e| panic!("step {n}: {e}"));
        run.gcl = run
            .gcl
            .max(gcl_residual(&old, &flow.geom.mesh, &info.mesh_flux, dt) / sp.mean_volume());
        run.cont = run.cont.max(info.flow.continuity);
        run.p_iters += info.flow.p_iters;
        run.iterations.push(info.iterations);
        run.last_residual = run.last_residual.max(*info.residuals.last().unwrap());
        run.all_converged &= info.converged;
        if n <= 3 {
            run.first_factors.push(info.factors.clone());
        }
        run.t.push(n as f64 * dt);
        run.x.push(body.state.x[0]);
    }
    run
}

/// Result of a loosely coupled run (ADR 0033 D7) with every failure caught.
struct LooseRun {
    t: Vec<f64>,
    x: Vec<f64>,
    /// x component of the force used in steps 1, 2, ...
    forces: Vec<f64>,
    /// Step of the first error or non-finite value, and what it was.
    ended: Option<(usize, String)>,
}

impl LooseRun {
    /// First step with `|x| >= 10 X`, or the step of the first failure if that came first.
    fn diverged_at(&self) -> Option<usize> {
        let big = self.x.iter().position(|x| x.abs() >= 10.0 * SP_X);
        match (big, &self.ended) {
            (Some(b), _) => Some(b),
            (None, Some((n, _))) => Some(*n),
            (None, None) => None,
        }
    }
}

fn loose_run(sp: &Sp, mu: f64, steps_per_period: usize, steps: usize) -> LooseRun {
    let dt = 1.0 / steps_per_period as f64;
    let mut flow = sp.flow();
    let mut body = body(mu);
    let patches = piston();
    let mut run = LooseRun {
        t: vec![0.0],
        x: vec![SP_X],
        forces: Vec::new(),
        ended: None,
    };
    for n in 1..=steps {
        let step = (|| -> Result<(), String> {
            let (f, m) = patch_force_moment(&flow.geom, &flow.u, &flow.p, RHO, SP_NU, &patches, body.state.x)?;
            if !f[0].is_finite() {
                return Err("non-finite force".into());
            }
            run.forces.push(f[0]);
            body.advance(dt, Integrator::Rk4, (f, m))?;
            let x_p = body.state.x[0];
            if !x_p.is_finite() || x_p.abs() > 1e3 * SP_L0 {
                return Err(format!("piston at {x_p:e}"));
            }
            if x_p >= SP_L0 {
                return Err(format!("piston through the outlet at {x_p:.3}"));
            }
            let st = move_mesh(&flow.geom, sp.points(x_p), dt)?;
            let old = flow.geom.mesh.clone();
            let walls = set_follow_conditions(&mut flow.u, &st, &old, &patches, dt)?;
            flow.step_ale_with_walls(dt, st.geom, st.mesh_flux, walls)?;
            if !flow.u.values.iter().flatten().all(|x| x.is_finite()) {
                return Err("non-finite velocity".into());
            }
            Ok(())
        })();
        if let Err(e) = step {
            run.ended = Some((n, e));
            break;
        }
        run.t.push(n as f64 * dt);
        run.x.push(body.state.x[0]);
    }
    run
}

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

fn vbits(v: &[Vec3]) -> Vec<u64> {
    v.iter().flatten().map(|x| x.to_bits()).collect()
}

/// Bit pattern of the flow and the body.
fn state_bits(flow: &PimpleSolver, body: &Body) -> Vec<Vec<u64>> {
    vec![
        vbits(&flow.u.values),
        bits(&flow.p.values),
        bits(&flow.phi),
        bits(&body.state.x),
        bits(&body.state.v),
        vec![flow.time.to_bits()],
    ]
}

// =================================================================================================================
// No flow: the relaxation (H1) and the model problem (P0)
// =================================================================================================================

/// ADR 0250 H1: the added-mass factor, and the two relaxations on the affine 3-vector map `G(x) = -2 x + c`.
#[test]
fn h1_added_mass_factor_and_relaxation_on_an_affine_map() {
    let w = relaxation_from_added_mass(1.0, 4.0);
    let w0 = relaxation_from_added_mass(0.5, 0.0);
    let c = [0.3, -1.1, 0.7];
    let g = |x: Vec3| vec3::add(vec3::scale(x, -2.0), c);
    let run = |relaxation: Relaxation, n: usize| -> Vec<f64> {
        let mut rel = Relaxer::new(relaxation);
        let mut x = [0.0, 0.0, 0.0];
        let mut norms = Vec::new();
        for _ in 0..n {
            let r = vec3::sub(g(x), x);
            norms.push(vec3::norm(r));
            x = vec3::add(x, vec3::scale(r, rel.factor(r)));
        }
        norms
    };
    let aitken = run(Relaxation::Aitken { initial: 1.0, max: 2.0 }, 3);
    let fixed = run(Relaxation::Fixed(0.25), 2);
    let (a, f) = (aitken[2] / aitken[0], fixed[1] / fixed[0]);
    println!(
        "H1: factor(1, 4) {w:.17} (1/3), factor(0.5, 0) {w0}; Aitken |r_3|/|r_1| {a:.2e} (limit 1e-12); Fixed(0.25) r_2/r_1 \
         {f:.15} (0.25 to 1e-12)"
    );
    assert!((w - 1.0 / 3.0).abs() <= 1e-15, "H1 factor {w}");
    assert!((w0 - 1.0).abs() <= 1e-15, "H1 factor {w0}");
    assert!(a <= 1e-12, "H1 Aitken {a}");
    assert!((f - 0.25).abs() <= 1e-12, "H1 fixed {f}");
}

/// The model of ADR 0250 (arithmetic section): RK4 body with a constant force, fluid force
/// `-rho A (L0 - x_{n+1}) (w_{n+1/2} - w_{n-1/2})/dt`. Returns `(x history, forces used, step of the end)`; `strong` solves
/// the step's force by the relaxed iteration to `1e-12`.
fn model(mu: f64, steps_per_period: usize, steps: usize, strong: bool) -> (Vec<f64>, Vec<f64>, Option<usize>) {
    let (m, k, dt) = (mass(mu), stiffness(mu), 1.0 / steps_per_period as f64);
    let rk4 = |x: f64, v: f64, f: f64| -> (f64, f64) {
        let acc = |x: f64| (f - k * x) / m;
        let (k1x, k1v) = (v, acc(x));
        let (k2x, k2v) = (v + 0.5 * dt * k1v, acc(x + 0.5 * dt * k1x));
        let (k3x, k3v) = (v + 0.5 * dt * k2v, acc(x + 0.5 * dt * k2x));
        let (k4x, k4v) = (v + dt * k3v, acc(x + dt * k3x));
        (
            x + dt / 6.0 * (k1x + 2.0 * k2x + 2.0 * k3x + k4x),
            v + dt / 6.0 * (k1v + 2.0 * k2v + 2.0 * k3v + k4v),
        )
    };
    let fluid = |x1: f64, x0: f64, w_prev: f64| -> (f64, f64) {
        let w = (x1 - x0) / dt;
        (-RHO * SP_A * (SP_L0 - x1) * (w - w_prev) / dt, w)
    };
    let (mut x, mut v, mut w_prev, mut f) = (SP_X, 0.0, 0.0, 0.0);
    let (mut xs, mut fs) = (vec![x], Vec::new());
    for n in 1..=steps {
        let (x1, v1, w, f_next);
        if strong {
            let omega = relaxation_from_added_mass(m, M_A);
            let mut fk = f;
            loop {
                let (a, b) = rk4(x, v, fk);
                let (ft, ww) = fluid(a, x, w_prev);
                if (ft - fk).abs() <= 1e-12 * k * SP_X {
                    (x1, v1, w, f_next) = (a, b, ww, ft);
                    break;
                }
                fk += omega * (ft - fk);
            }
        } else {
            fs.push(f);
            let (a, b) = rk4(x, v, f);
            let (ft, ww) = fluid(a, x, w_prev);
            (x1, v1, w, f_next) = (a, b, ww, ft);
        }
        if x1 >= SP_L0 || x1.abs() > 50.0 {
            return (xs, fs, Some(n));
        }
        (x, v, w_prev, f) = (x1, v1, w, f_next);
        xs.push(x);
    }
    (xs, fs, None)
}

/// ADR 0250 P0: the model problem: the threshold of the loose loop at a mass ratio of 2, its growth per step at 4 for two
/// steps, and the decay of the converged strong scheme. The values were known before the registration.
#[test]
fn p0_model_threshold_growth_and_converged_scheme() {
    let (x19, _, end19) = model(1.9, 100, 1150, false);
    let (g19, _, _) = worst_growth(&x19);
    let (x21, _, end21) = model(2.1, 100, 1150, false);
    let big21 = x21.iter().position(|x| x.abs() >= 10.0 * SP_X);
    let (_, f100, _) = model(4.0, 100, 1150, false);
    let (_, f200, _) = model(4.0, 200, 2300, false);
    let (g100, g200) = (casoratian_growth(&f100), casoratian_growth(&f200));
    let (xs, _, ends) = model(4.0, 100, 1150, true);
    let (gs, _, _) = worst_growth(&xs);
    println!(
        "P0: loose mu 1.9: ended {end19:?}, largest turning ratio {g19:.5} (limit <= 1); loose mu 2.1: |x| >= 10 X at step \
         {big21:?} (limit < 1150), ended {end21:?}; loose mu 4 growth per step T/100 {g100:.4}, T/200 {g200:.4} (limit \
         [1.38, 1.42]); converged scheme mu 4 T/100 largest turning ratio {gs:.5} (limit [0.92, 0.93]), ended {ends:?}"
    );
    assert!(end19.is_none() && g19 <= 1.0, "P0 mu 1.9: {end19:?} {g19}");
    assert!(big21.is_some_and(|n| n < 1150), "P0 mu 2.1: {big21:?}");
    for g in [g100, g200] {
        assert!((1.38..=1.42).contains(&g), "P0 growth {g}");
    }
    assert!(ends.is_none() && (0.92..=0.93).contains(&gs), "P0 strong {gs}");
}

// =================================================================================================================
// Acceptance: mass ratio 4
// =================================================================================================================

/// ADR 0250 S0 to S5: the strongly coupled piston at a mass ratio of 4, `T/100` and `T/200`, 11.5 periods.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn s1_strong_coupling_at_mass_ratio_4_oscillates_at_the_added_mass_frequency() {
    let mu = 4.0;
    let sp = Sp::new();
    let ratio = M_A / mass(mu);
    let shift = ((mass(mu) + M_A) / mass(mu)).sqrt() - 1.0;
    let r1 = strong_run(&sp, mu, 100, 11.5, &registered(mu), RHO);
    let r2 = strong_run(&sp, mu, 200, 11.5, &registered(mu), RHO);
    let (om1, n1) = measured_omega(&r1.t, &r1.x);
    let (om2, n2) = measured_omega(&r2.t, &r2.x);
    let s1 = om1 / TAU;
    let s2 = (om2 - om1).abs() / om1;
    let (g1, mx1, mn1) = worst_growth(&r1.x);
    let (g2, mx2, mn2) = worst_growth(&r2.x);
    println!(
        "S0: rho A L0/M {ratio} (limit >= 1), fluid-free shift {shift:.4} (limit >= 0.08), k {:.6}",
        stiffness(mu)
    );
    println!(
        "S1 T/100: omega_m/omega {s1:.6} (limit |. - 1| <= 0.02) over {n1} crossings; T/200: {:.6} over {n2}; S2 dt halving \
         {s2:.2e} (limit 5e-3)",
        om2 / TAU
    );
    println!(
        "S3 largest turning-point ratio T/100 {g1:.5} ({mx1} maxima, {mn1} minima), T/200 {g2:.5} ({mx2}, {mn2}) (limit 1 + \
         1e-3, crossings >= 11)"
    );
    for (name, r) in [("T/100", &r1), ("T/200", &r2)] {
        println!(
            "S4 {name}: converged every step {}, iterations largest {} mean {:.3} (limits 8, 5), histogram {:?}, largest last \
             residual {:.2e} (limit 1e-8), Aitken factors of steps 1 to 3 {:?}",
            r.all_converged,
            r.max_iterations(),
            r.mean_iterations(),
            r.histogram(),
            r.last_residual,
            r.first_factors
        );
        println!(
            "S5 {name}: continuity {:.2e} (limit 1e-10), GCL {:.2e} mean(V) (limit 1e-12), max |x| {:.6e} (limit X (1 + \
             1e-9)), pressure iterations {}",
            r.cont,
            r.gcl,
            r.max_abs_x(),
            r.p_iters
        );
    }
    assert!(ratio >= 1.0 && shift >= 0.08, "S0 {ratio} {shift}");
    assert!((s1 - 1.0).abs() <= 0.02, "S1 {s1}");
    assert!(s2 <= 5e-3, "S2 {s2}");
    assert!(g1 <= 1.0 + 1e-3 && g2 <= 1.0 + 1e-3, "S3 {g1} {g2}");
    assert!(n1 >= 11 && n2 >= 11, "S3 crossings {n1} {n2}");
    for r in [&r1, &r2] {
        assert!(r.all_converged, "S4 not converged");
        assert!(r.max_iterations() <= 8 && r.mean_iterations() <= 5.0, "S4 iterations");
        assert!(r.last_residual <= 1e-8, "S4 residual {}", r.last_residual);
        assert!(r.cont <= 1e-10, "S5 continuity {}", r.cont);
        assert!(r.gcl <= 1e-12, "S5 GCL {}", r.gcl);
        assert!(r.max_abs_x() <= SP_X * (1.0 + 1e-9), "S5 amplitude {}", r.max_abs_x());
    }
    assert!(
        sp.mesh0.patches().iter().all(|p| p.kind != PatchKind::Cyclic),
        "S5 cyclic"
    );
}

/// ADR 0250 SN1 (negative control of S1): with the fluid force off the frequency is far from `omega`.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn sn1_strong_coupling_without_the_fluid_force_misses_the_frequency() {
    let mu = 4.0;
    let sp = Sp::new();
    let r = strong_run(&sp, mu, 100, 11.5, &registered(mu), 0.0);
    let (om, n) = measured_omega(&r.t, &r.x);
    let miss = (om / TAU - 1.0).abs();
    println!(
        "SN1: fluid force off: omega_m/omega {:.6} over {n} crossings, |. - 1| {miss:.4} (limit >= 0.08; sqrt(5) = {:.6})",
        om / TAU,
        5.0f64.sqrt()
    );
    assert!(miss >= 0.08, "SN1 {miss}");
}

/// ADR 0250 L1 to L3: the loose coupling of ADR 0033 D7 on the same case diverges, at the same rate per step for both steps.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn l1_loose_coupling_diverges_at_mass_ratio_4_at_a_rate_independent_of_the_step() {
    let sp = Sp::new();
    let mut growth = Vec::new();
    let mut at = Vec::new();
    for spp in [100usize, 200] {
        let r = loose_run(&sp, 4.0, spp, 1150 * spp / 100);
        let d = r.diverged_at();
        let g = if r.forces.len() >= 23 {
            casoratian_growth(&r.forces)
        } else {
            f64::NAN
        };
        println!(
            "L T/{spp}: diverged at step {d:?} (limit <= 60), ended {:?} after {} steps, max |x| {:.3e}, growth per step C_8 to \
             C_20 {g:.4} (limit [1.30, 1.50]; sqrt(2) = 1.4142)",
            r.ended,
            r.x.len() - 1,
            r.x.iter().fold(0.0f64, |a, b| a.max(b.abs()))
        );
        growth.push(g);
        at.push(d);
    }
    assert!(at[0].is_some_and(|n| n <= 60), "L1 {:?}", at[0]);
    assert!(at[1].is_some_and(|n| n <= 60), "L2 {:?}", at[1]);
    for g in growth {
        assert!((1.30..=1.50).contains(&g), "L3 {g}");
    }
}

// =================================================================================================================
// The coupling iteration
// =================================================================================================================

/// The first step from rest at a mass ratio of 4, `T/100`, with the given relaxation and cap (tolerance `1e-8`, floor `k X`).
fn first_step(sp: &Sp, relaxation: Relaxation, cap: usize, tolerance: f64) -> Result<CouplingStep, String> {
    let mu = 4.0;
    let mut flow = sp.flow();
    let mut body = body(mu);
    let settings = StrongCoupling {
        relaxation,
        tolerance,
        force_scale: stiffness(mu) * SP_X,
        moment_scale: 0.0,
        max_iterations: cap,
    };
    strong(sp, &mut flow, &mut body, 0.01, &coupling(RHO), &settings)
}

/// ADR 0250 C1: the gain `1 + mu/2` of the sub-iteration from the residual ratios of three fixed factors.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn c1_fixed_relaxation_contracts_at_the_derived_rate() {
    let sp = Sp::new();
    let mut all = Vec::new();
    for w in [0.25, 0.5, 1.0] {
        let info = first_step(&sp, Relaxation::Fixed(w), 8, 1e-300).unwrap();
        let r: Vec<f64> = info.force_residuals.iter().map(|v| v[0]).collect();
        let gains: Vec<f64> = (1..=5).map(|k| (1.0 - r[k] / r[k - 1]) / w).collect();
        println!(
            "C1 w = {w}: residual ratios {:?}, gains {gains:.5?} (limit [2.90, 3.00]; 1 + mu/2 = 3), iterations {}",
            (1..=5).map(|k| r[k] / r[k - 1]).collect::<Vec<_>>(),
            info.iterations
        );
        all.extend(gains);
    }
    for g in all {
        assert!((2.90..=3.00).contains(&g), "C1 gain {g}");
    }
}

fn unrelaxed(cap: usize) -> Result<CouplingStep, String> {
    let sp = Sp::new();
    first_step(&sp, Relaxation::Fixed(1.0), cap, 1e-8)
}

/// ADR 0250 C2 as registered (cap 20). By amendment 1 it cannot finish: the unrelaxed iteration drives the piston through the
/// outlet near iteration 15. Run once for the record (the outcome is in the addendum of ADR 0250).
#[test]
#[ignore = "defective set-up (ADR 0250 amendment 1): the diverging iteration leaves the mesh before the cap of 20; replaced by c2b"]
fn c2_unrelaxed_iteration_diverges_registered_record() {
    let info = unrelaxed(20).unwrap();
    let r: Vec<f64> = info.force_residuals.iter().map(|v| vec3::norm(*v)).collect();
    assert!(!info.converged, "C2 converged");
    assert!(r[19] / r[0] >= 1e4, "C2 {}", r[19] / r[0]);
}

/// ADR 0250 C2b (amendment 1; negative control of the convergence condition `w < 4/(2 + mu)`): without relaxation the
/// sub-iteration diverges.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn c2b_unrelaxed_iteration_diverges() {
    let info = unrelaxed(10).unwrap();
    let r: Vec<f64> = info.force_residuals.iter().map(|v| vec3::norm(*v)).collect();
    let growth = r[9] / r[0];
    println!(
        "C2b Fixed(1), cap 10: converged {}, iterations {}, |r_10|/|r_1| {growth:.1} (limit >= 200; 1.977^9 = 461)",
        info.converged, info.iterations
    );
    assert!(!info.converged, "C2b converged");
    assert!(growth >= 200.0, "C2b {growth}");
}

/// ADR 0250 C3: the fixed factor from the added mass converges in a few iterations.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn c3_added_mass_factor_converges() {
    let sp = Sp::new();
    let w = relaxation_from_added_mass(mass(4.0), M_A);
    let info = first_step(&sp, Relaxation::Fixed(w), 20, 1e-8).unwrap();
    println!(
        "C3 Fixed({w:.6}): converged {}, iterations {} (limit <= 8), residuals {:?}",
        info.converged, info.iterations, info.residuals
    );
    assert!(info.converged && info.iterations <= 8, "C3 {}", info.iterations);
}

/// ADR 0250 C4: Aitken started outside the range of a fixed factor converges to the same motion.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn c4_aitken_from_outside_the_fixed_range_gives_the_same_motion() {
    let mu = 4.0;
    let sp = Sp::new();
    let mut settings = registered(mu);
    settings.relaxation = Relaxation::Aitken { initial: 1.0, max: 2.0 };
    let r = strong_run(&sp, mu, 100, 11.5, &settings, RHO);
    let reference = strong_run(&sp, mu, 100, 11.5, &registered(mu), RHO);
    let diff =
        r.x.iter()
            .zip(&reference.x)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f64::max)
            / SP_X;
    println!(
        "C4 Aitken from 1: converged every step {}, iterations largest {} mean {:.3} (limits 8, 6), histogram {:?}, max |x - \
         x(S1)| {diff:.2e} X (limit 1e-5), factors of steps 1 to 3 {:?}",
        r.all_converged,
        r.max_iterations(),
        r.mean_iterations(),
        r.histogram(),
        r.first_factors
    );
    assert!(r.all_converged, "C4 not converged");
    assert!(r.max_iterations() <= 8 && r.mean_iterations() <= 6.0, "C4 iterations");
    assert!(diff <= 1e-5, "C4 {diff}");
}

/// ADR 0250 R1: a repeated iterate starts from exactly the state at `t_n`: step 37 of the strong run equals one loose-style
/// pass with the force of its last iterate, bit for bit.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn r1_the_restart_of_an_iterate_is_exact() {
    let mu = 4.0;
    let sp = Sp::new();
    let (dt, cpl, settings) = (0.01, coupling(RHO), registered(mu));
    let mut flow_a = sp.flow();
    let mut body_a = body(mu);
    let mut last = None;
    for _ in 0..37 {
        last = Some(strong(&sp, &mut flow_a, &mut body_a, dt, &cpl, &settings).unwrap());
    }
    let last = last.unwrap();
    let mut flow_b = sp.flow();
    let mut body_b = body(mu);
    for _ in 0..36 {
        strong(&sp, &mut flow_b, &mut body_b, dt, &cpl, &settings).unwrap();
    }
    body_b.advance(dt, Integrator::Rk4, (last.force, last.moment)).unwrap();
    let st = move_mesh(&flow_b.geom, sp.points(body_b.state.x[0]), dt).unwrap();
    let old = flow_b.geom.mesh.clone();
    let walls = set_follow_conditions(&mut flow_b.u, &st, &old, &piston(), dt).unwrap();
    flow_b.step_ale_with_walls(dt, st.geom, st.mesh_flux, walls).unwrap();
    let (a, b) = (state_bits(&flow_a, &body_a), state_bits(&flow_b, &body_b));
    let differing: usize = a
        .iter()
        .zip(&b)
        .map(|(p, q)| p.iter().zip(q).filter(|(x, y)| x != y).count())
        .sum();
    println!(
        "R1: step 37 took {} iterations; values differing in u, p, phi, x, v, time: {differing} (limit 0)",
        last.iterations
    );
    assert!(last.iterations >= 2, "R1 vacuous: one iteration");
    assert_eq!(differing, 0, "R1");
}

// =================================================================================================================
// Threshold, equivalence, small ratio, extension, refusals
// =================================================================================================================

/// ADR 0250 L4: below the threshold of D3 the loose coupling is stable at a mass ratio above 1.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn l4_loose_coupling_is_stable_at_mass_ratio_1_5() {
    let sp = Sp::new();
    let r = loose_run(&sp, 1.5, 100, 1150);
    println!("L4: ended {:?} after {} steps", r.ended, r.x.len() - 1);
    assert!(r.ended.is_none(), "L4 ended {:?}", r.ended);
    let (g, mx, mn) = worst_growth(&r.x);
    let (om, n) = measured_omega(&r.t, &r.x);
    println!(
        "L4 loose mu 1.5: largest turning ratio {g:.5} ({mx} maxima, {mn} minima; limit 1 + 1e-3), omega_m/omega {:.6} over {n} \
         crossings (limit |. - 1| <= 0.02)",
        om / TAU
    );
    assert!(g <= 1.0 + 1e-3, "L4 {g}");
    assert!((om / TAU - 1.0).abs() <= 0.02, "L4 {}", om / TAU);
}

/// ADR 0250 L5: above the threshold of D3 it diverges.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn l5_loose_coupling_diverges_at_mass_ratio_3() {
    let sp = Sp::new();
    let r = loose_run(&sp, 3.0, 100, 1150);
    let d = r.diverged_at();
    println!(
        "L5 loose mu 3: diverged at step {d:?} (limit <= 120), ended {:?} after {} steps",
        r.ended,
        r.x.len() - 1
    );
    assert!(d.is_some_and(|n| n <= 120), "L5 {d:?}");
}

/// ADR 0250 Q1: with one iteration the strong step is the loose step of ADR 0033 D7, bit for bit.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn q1_one_iteration_is_the_loose_coupling_bit_for_bit() {
    let mu = 0.2;
    let sp = Sp::new();
    let dt = 0.01;
    let mut settings = registered(mu);
    settings.max_iterations = 1;
    let cpl = coupling(RHO);
    let (mut flow_a, mut body_a) = (sp.flow(), body(mu));
    let (mut flow_b, mut body_b) = (sp.flow(), body(mu));
    let patches = piston();
    let mut differing = 0usize;
    for _ in 0..100 {
        strong(&sp, &mut flow_a, &mut body_a, dt, &cpl, &settings).unwrap();
        let (f, m) =
            patch_force_moment(&flow_b.geom, &flow_b.u, &flow_b.p, RHO, SP_NU, &patches, body_b.state.x).unwrap();
        body_b.advance(dt, Integrator::Rk4, (f, m)).unwrap();
        let st = move_mesh(&flow_b.geom, sp.points(body_b.state.x[0]), dt).unwrap();
        let old = flow_b.geom.mesh.clone();
        let walls = set_follow_conditions(&mut flow_b.u, &st, &old, &patches, dt).unwrap();
        flow_b.step_ale_with_walls(dt, st.geom, st.mesh_flux, walls).unwrap();
        differing += state_bits(&flow_a, &body_a)
            .iter()
            .zip(&state_bits(&flow_b, &body_b))
            .map(|(p, q)| p.iter().zip(q).filter(|(x, y)| x != y).count())
            .sum::<usize>();
    }
    println!(
        "Q1: 100 steps, values differing in u, p, phi, x, v, time after any step: {differing} (limit 0); history entries {} \
         and {}; x {:.6e}",
        flow_a.history.len(),
        flow_b.history.len(),
        body_a.state.x[0]
    );
    assert_eq!(differing, 0, "Q1");
    assert_eq!(flow_a.history.len(), flow_b.history.len(), "Q1 history");
}

/// ADR 0250 A1, A2: at a mass ratio of 0.2 (SP of ADR 0211) the strong and the loose coupling agree in frequency, and their
/// difference is the coupling lag, which falls with the step.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn a1_strong_and_loose_coupling_agree_at_mass_ratio_0_2() {
    let mu = 0.2;
    let sp = Sp::new();
    let mut d = Vec::new();
    let mut first = None;
    for spp in [100usize, 200] {
        let steps = (6.5 * spp as f64).round() as usize;
        let l = loose_run(&sp, mu, spp, steps);
        assert!(l.ended.is_none(), "A loose ended {:?}", l.ended);
        let s = strong_run(&sp, mu, spp, 6.5, &registered(mu), RHO);
        let diff = s.x.iter().zip(&l.x).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max) / SP_X;
        d.push(diff);
        let (oml, _) = measured_omega(&l.t, &l.x);
        let (oms, _) = measured_omega(&s.t, &s.x);
        let (gl, _, _) = worst_growth(&l.x);
        let (gs, _, _) = worst_growth(&s.x);
        println!(
            "A T/{spp}: omega_m/omega loose {:.6}, strong {:.6}, relative difference {:.2e}; largest turning ratio loose \
             {gl:.5}, strong {gs:.5}; max |x_s - x_l| {diff:.4} X; strong converged {}, iterations largest {} mean {:.3}",
            oml / TAU,
            oms / TAU,
            (oms - oml).abs() / oml,
            s.all_converged,
            s.max_iterations(),
            s.mean_iterations()
        );
        if spp == 100 {
            first = Some((oml, oms, gl, gs, s.all_converged, s.max_iterations()));
        }
    }
    let (oml, oms, gl, gs, converged, its) = first.unwrap();
    let ratio = d[1] / d[0];
    println!("A2: D(T/200)/D(T/100) {ratio:.4} (limit [0.45, 0.65])");
    assert!((oms - oml).abs() / oml <= 1e-3, "A1 difference");
    assert!(
        (oml / TAU - 1.0).abs() <= 0.02 && (oms / TAU - 1.0).abs() <= 0.02,
        "A1 frequency"
    );
    assert!(converged && its <= 8, "A1 iterations {its}");
    assert!((0.945..=0.960).contains(&gl), "A2 loose {gl}");
    assert!((0.975..=0.992).contains(&gs), "A2 strong {gs}");
    assert!(d[0] <= 0.25, "A2 D {}", d[0]);
    assert!((0.45..=0.65).contains(&ratio), "A2 ratio {ratio}");
}

/// ADR 0250 X1 (extension): a mass ratio of 20.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn x1_strong_coupling_at_mass_ratio_20() {
    let mu = 20.0;
    let sp = Sp::new();
    let r = strong_run(&sp, mu, 100, 11.5, &registered(mu), RHO);
    let (om, n) = measured_omega(&r.t, &r.x);
    let (g, mx, mn) = worst_growth(&r.x);
    println!(
        "X1 mu 20: omega_m/omega {:.6} over {n} crossings (limit |. - 1| <= 0.02), largest turning ratio {g:.5} ({mx} maxima, \
         {mn} minima; limit 1 + 1e-3), converged every step {}, iterations largest {} mean {:.3} (limits 8, 5), histogram {:?}",
        om / TAU,
        r.all_converged,
        r.max_iterations(),
        r.mean_iterations(),
        r.histogram()
    );
    assert!((om / TAU - 1.0).abs() <= 0.02, "X1 {}", om / TAU);
    assert!(g <= 1.0 + 1e-3, "X1 {g}");
    assert!(r.all_converged, "X1 not converged");
    assert!(r.max_iterations() <= 8 && r.mean_iterations() <= 5.0, "X1 iterations");
}

/// ADR 0250 E1: invalid settings and a failure inside an iterate are errors that leave the flow and the body untouched.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn e1_refusals_leave_the_flow_and_the_body_untouched() {
    let mu = 4.0;
    let sp = Sp::new();
    let (dt, cpl, good) = (0.01, coupling(RHO), registered(mu));
    let mut flow = sp.flow();
    let mut body = body(mu);
    for _ in 0..3 {
        strong(&sp, &mut flow, &mut body, dt, &cpl, &good).unwrap();
    }
    let before = state_bits(&flow, &body);
    let geom = flow.geom.clone();
    let history = flow.history.len();
    let with = |f: &dyn Fn(&mut StrongCoupling)| {
        let mut s = good;
        f(&mut s);
        s
    };
    let bad: Vec<(&str, StrongCoupling)> = vec![
        ("Fixed(0)", with(&|s| s.relaxation = Relaxation::Fixed(0.0))),
        ("Fixed(2)", with(&|s| s.relaxation = Relaxation::Fixed(2.0))),
        (
            "Aitken cap below the initial factor",
            with(&|s| s.relaxation = Relaxation::Aitken { initial: 0.5, max: 0.1 }),
        ),
        ("tolerance 0", with(&|s| s.tolerance = 0.0)),
        ("no iterations", with(&|s| s.max_iterations = 0)),
        ("NaN force scale", with(&|s| s.force_scale = f64::NAN)),
    ];
    let mut errors = 0;
    let mut check = |name: &str, r: Result<CouplingStep, String>, flow: &PimpleSolver, body: &Body| {
        let e = r.err().unwrap_or_else(|| panic!("E1 {name}: no error"));
        println!("E1 {name}: {e}");
        assert_eq!(state_bits(flow, body), before, "E1 {name}: state changed");
        assert!(Arc::ptr_eq(&flow.geom, &geom), "E1 {name}: geometry changed");
        assert_eq!(flow.history.len(), history, "E1 {name}: history changed");
        errors += 1;
    };
    for (name, s) in &bad {
        let r = strong(&sp, &mut flow, &mut body, dt, &cpl, s);
        check(name, r, &flow, &body);
    }
    flow.settings.corrector_control = Some(CorrectorControl::registered());
    let r = strong(&sp, &mut flow, &mut body, dt, &cpl, &good);
    flow.settings.corrector_control = None;
    check("corrector_control", r, &flow, &body);
    let mut calls = 0;
    let r = strong_step(&mut flow, &mut body, dt, &cpl, &good, &mut |b: &Body| {
        calls += 1;
        if calls == 2 {
            Err("points refused".to_string())
        } else {
            Ok(sp.points(b.state.x[0]))
        }
    });
    check("points fails at its second call", r, &flow, &body);
    assert_eq!(errors, 8, "E1 count");
    // the untouched state still steps
    let info = strong(&sp, &mut flow, &mut body, dt, &cpl, &good).unwrap();
    assert!(info.converged, "E1 the step after the refusals");
}
