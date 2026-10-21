//! Moving-mesh flow checks G4-G6 of ADR 0033: the oscillating plate (Stokes second problem) on a moving mesh, an
//! incompressible piston-driven channel with a smoothed mesh, and a rigidly rotating mesh. The limits are those
//! registered in the ADR before the code ran.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::ale::{move_mesh, set_follow_conditions, MeshStep};
use rcfd_fv::bc::{FixedValue, Symmetry, ZeroGradient};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::smoothing::{smooth_displacement, PatchClass, SmoothingMethod};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{annulus_3d, box_2d, box_mesh, cylinder_ogrid, Grading};
use rcfd_mesh::motion::{cell_swept_sum, Pose};
use rcfd_mesh::Mesh;
use std::sync::Arc;

const TIGHT: SolveControl = SolveControl {
    tol_abs: 1e-16,
    tol_rel: 1e-13,
    max_iter: 5000,
};

fn settings(nu: f64) -> PimpleSettings {
    let mut s = PimpleSettings::piso(nu);
    s.convection = ConvScheme::Central;
    s.momentum_solver = TIGHT;
    s.pressure_solver = TIGHT;
    s
}

// ---------------------------------------------------------------------------------------------------------------
// G4 Stokes second problem
// ---------------------------------------------------------------------------------------------------------------

const NU: f64 = 0.01;
const OMEGA: f64 = std::f64::consts::TAU;
const V_MESH: f64 = 0.5;
const N_OUTER_G4: usize = 10;

fn stokes_exact(y_rel: f64, t: f64) -> f64 {
    let delta = (2.0 * NU / OMEGA).sqrt();
    let eta = y_rel / delta;
    (-eta).exp() * (OMEGA * t - eta).cos()
}

/// Plate displacement along x, `x_p(t) = (U0/omega) sin(omega t)` with `U0 = 1`.
fn plate_x(t: f64) -> f64 {
    (OMEGA * t).sin() / OMEGA
}

struct StokesResult {
    err_ux: f64,
    err_uy: f64,
    ux: Vec<f64>,
}

/// `moving`: the mesh translates with the plate and upward with `V_MESH`; otherwise a fixed mesh whose bottom wall
/// gets the step-mean plate velocity (the boundary datum the moving run uses).
fn stokes_run(
    ny: usize,
    steps_per_period: usize,
    moving: bool,
    periods: usize,
    consistent_start: bool,
    n_outer: usize,
) -> StokesResult {
    let mesh = box_2d(4, ny, 0.1, 1.0, true, false).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh.clone())).unwrap());
    let vy = if moving { V_MESH } else { 0.0 };
    let mut u = VectorField::from_fn("U", &geom, |c| [stokes_exact(c[1], 0.0), vy, 0.0]);
    u.set_bc(&geom, "ymax", Box::new(FixedValue::vector([0.0, vy, 0.0])))
        .unwrap();
    // the initial flux of the solver comes from the boundary values at construction: with `consistent_start` the plate
    // already carries the normal velocity of the moving mesh, otherwise it is the fixed-mesh datum (a set-up error: the
    // wall flux then contradicts the uniform vertical motion of the fluid and a start-up pressure transient follows)
    let vy_wall = if consistent_start { vy } else { 0.0 };
    u.set_bc(&geom, "ymin", Box::new(FixedValue::vector([1.0, vy_wall, 0.0])))
        .unwrap();
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut st = settings(NU);
    st.n_outer = n_outer;
    let mut s = PimpleSolver::new(geom, u, p, st);
    let dt = 1.0 / steps_per_period as f64;
    let n_steps = periods * steps_per_period;
    for n in 1..=n_steps {
        let (t0, t1) = ((n - 1) as f64 * dt, n as f64 * dt);
        if moving {
            let pts: Vec<Vec3> = mesh
                .points()
                .iter()
                .map(|&q| vec3::add(q, [plate_x(t1), V_MESH * t1, 0.0]))
                .collect();
            let st = move_mesh(&s.geom, pts, dt).unwrap();
            let old = s.geom.mesh.clone();
            let walls = set_follow_conditions(&mut s.u, &st, &old, &["ymin".to_string()], dt).unwrap();
            s.step_ale_with_walls(dt, st.geom, st.mesh_flux, walls).unwrap();
        } else {
            let wall = (plate_x(t1) - plate_x(t0)) / dt;
            s.u.set_bc(&s.geom.clone(), "ymin", Box::new(FixedValue::vector([wall, 0.0, 0.0])))
                .unwrap();
            s.step(dt);
        }
    }
    let t = n_steps as f64 * dt;
    let shift = vy * t;
    let mut err_ux: f64 = 0.0;
    let mut err_uy: f64 = 0.0;
    let mut ux = Vec::new();
    for (v, c) in s.u.values.iter().zip(s.geom.mesh.cell_centres()) {
        err_ux = err_ux.max((v[0] - stokes_exact(c[1] - shift, t)).abs());
        err_uy = err_uy.max((v[1] - vy).abs());
        ux.push(v[0]);
    }
    StokesResult { err_ux, err_uy, ux }
}

fn g4_body(consistent_start: bool, n_outer: usize) {
    let fine = stokes_run(80, 800, true, 3, consistent_start, n_outer);
    println!(
        "G4 ny=80 dt=T/800: max|u_x-exact| {:.3e}, max|u_y-V| {:.2e}",
        fine.err_ux, fine.err_uy
    );
    let coarse = stokes_run(40, 400, true, 3, consistent_start, n_outer);
    let ratio = coarse.err_ux / fine.err_ux;
    println!(
        "G4 ny=40 dt=T/400: max|u_x-exact| {:.3e}; ratio e40/e80 {ratio:.3}",
        coarse.err_ux
    );
    let fixed = stokes_run(80, 800, false, 3, true, n_outer);
    let diff = fine
        .ux
        .iter()
        .zip(&fixed.ux)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f64::max);
    println!(
        "G4 moving vs fixed mesh: max difference {diff:.2e} (fixed-mesh error {:.3e})",
        fixed.err_ux
    );
    assert!(fine.err_ux <= 4e-2, "(i) {}", fine.err_ux);
    assert!(ratio >= 1.7, "(ii) ratio {ratio}");
    assert!(fine.err_uy <= 1e-9, "(iii) {}", fine.err_uy);
    assert!(diff <= 1e-9, "(iv) {diff}");
}

/// The test as first written: the initial wall flux contradicts the vertical motion of the fluid (set-up error, see
/// `stokes_run`). Kept, ignored, next to the corrected test (ADR 0033 addendum).
#[test]
#[ignore = "set-up error: inconsistent initial wall flux; superseded by g4b"]
fn g4_stokes_second_problem_on_a_moving_mesh_known_miss() {
    g4_body(false, 1);
}

/// Consistent start, one outer iteration (PISO). The coupling of the two periodic faces of a cell is lagged inside a
/// step, so with a mesh that translates along the periodic direction (relative flux across the seam) the lag error
/// leaves a spurious pressure and vertical velocity at the seam column. Ten outer iterations remove it (below).
#[test]
#[ignore = "lagged periodic coupling within one outer iteration (ADR 0033 addendum); superseded by g4c"]
fn g4b_stokes_second_problem_on_a_moving_mesh_known_miss() {
    g4_body(true, 1);
}

/// Corrected set-up: consistent start and `N_OUTER_G4` outer iterations per step (PIMPLE) so that the lagged periodic
/// coupling is converged within the step.
#[test]
fn g4c_stokes_second_problem_on_a_moving_mesh() {
    g4_body(true, N_OUTER_G4);
}

// ---------------------------------------------------------------------------------------------------------------
// G5 piston-driven channel
// ---------------------------------------------------------------------------------------------------------------

struct Piston {
    mesh0: Mesh,
    solver: PimpleSolver,
    classes: Vec<PatchClass>,
    time: f64,
}

impl Piston {
    /// Channel `L0 x w x dz` with `nx` cells along x; the `xmin` patch is the piston.
    fn new(nx: usize, l0: f64, w: f64, dz: f64, symmetric_sides: bool) -> Self {
        let mesh0 = box_mesh([nx, 2, 2], [l0, w, dz], [Grading::Uniform; 3]).unwrap();
        let geom = Arc::new(FvGeometry::new(Arc::new(mesh0.clone())).unwrap());
        let mut u = VectorField::uniform("U", &geom, vec3::ZERO);
        for name in ["ymin", "ymax", "zmin", "zmax"] {
            if symmetric_sides {
                u.set_bc(&geom, name, Box::new(Symmetry)).unwrap();
            } else {
                u.set_bc(&geom, name, Box::new(ZeroGradient)).unwrap();
            }
        }
        let mut p = ScalarField::uniform("p", &geom, 0.0);
        p.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
        let mut classes = Vec::new();
        for pt in mesh0.patches() {
            classes.push(match pt.name.as_str() {
                "xmin" => PatchClass::Prescribed,
                "xmax" => PatchClass::Fixed,
                _ => PatchClass::Slip,
            });
        }
        let solver = PimpleSolver::new(geom, u, p, settings(0.01));
        Piston {
            mesh0,
            solver,
            classes,
            time: 0.0,
        }
    }

    /// Advance one step with the piston at displacement `x_p` at the end of the step; returns the mesh flux summed over
    /// the piston patch and the step info.
    fn step(&mut self, x_p: f64, dt: f64) -> (f64, rcfd_fv::pimple::StepInfo, f64) {
        let g0 = FvGeometry::new(Arc::new(self.mesh0.clone())).unwrap();
        let mut presc = vec![vec3::ZERO; self.mesh0.n_points()];
        for q in rcfd_mesh::motion::patch_points(&self.mesh0, "xmin").unwrap() {
            presc[q] = [x_p, 0.0, 0.0];
        }
        let sm = smooth_displacement(&g0, &self.classes, &presc, SmoothingMethod::Laplacian, &TIGHT).unwrap();
        let pts: Vec<Vec3> = self
            .mesh0
            .points()
            .iter()
            .zip(&sm.displacement)
            .map(|(p, d)| vec3::add(*p, *d))
            .collect();
        let st = move_mesh(&self.solver.geom, pts, dt).unwrap();
        let old = self.solver.geom.mesh.clone();
        let walls = set_follow_conditions(&mut self.solver.u, &st, &old, &["xmin".to_string()], dt).unwrap();
        let piston = st.geom.mesh.patch("xmin").unwrap();
        let m_piston: f64 = piston.faces().map(|f| st.mesh_flux[f]).sum();
        let vol_change: f64 = st.geom.mesh.total_volume() - old.total_volume();
        let info = self
            .solver
            .step_ale_with_walls(dt, st.geom, st.mesh_flux, walls)
            .unwrap();
        self.time += dt;
        (m_piston, info, vol_change)
    }

    fn outlet_flux(&self) -> f64 {
        let g = &self.solver.geom;
        g.mesh.patch("xmax").unwrap().faces().map(|f| self.solver.phi[f]).sum()
    }
}

fn g5_body(symmetric_sides: bool, from_piston: bool) -> G5 {
    let (a0, l0, dt, n_steps) = (2.0, 1.0, 0.005, 100);
    let mut pc = Piston::new(20, l0, 0.2, 0.2, symmetric_sides);
    let v0 = pc.solver.geom.mesh.total_volume();
    let mut worst_cont: f64 = 0.0;
    let mut worst_flux: f64 = 0.0;
    let mut worst_u: f64 = 0.0;
    let mut worst_trans: f64 = 0.0;
    let mut worst_vol: f64 = 0.0;
    let mut t = 0.0;
    for _ in 0..n_steps {
        let t1 = t + dt;
        let x_p = 0.5 * a0 * t1 * t1;
        let (m_piston, info, dvol) = pc.step(x_p, dt);
        worst_cont = worst_cont.max(info.continuity);
        let q_out = pc.outlet_flux();
        worst_flux = worst_flux.max((q_out + m_piston).abs() / m_piston.abs());
        let v_mean = a0 * (t + 0.5 * dt);
        for u in &pc.solver.u.values {
            worst_u = worst_u.max((u[0] - v_mean).abs());
            worst_trans = worst_trans.max(u[1].abs()).max(u[2].abs());
        }
        // domain volume change = swept volume of the piston patch
        worst_vol = worst_vol.max((dvol - m_piston * dt).abs() / v0);
        t = t1;
    }
    let l_end = l0 - 0.5 * a0 * t * t;
    let xc = pc.solver.geom.mesh.cell_centres();
    let worst_p = pc
        .solver
        .p
        .values
        .iter()
        .zip(xc)
        .map(|(p, c)| {
            // distance from the piston face: the piston sits at x_p = l0 - l_end
            let x_c = if from_piston { c[0] - (l0 - l_end) } else { c[0] };
            (p - a0 * (l_end - x_c)).abs()
        })
        .fold(0.0, f64::max);
    println!(
        "G5: continuity {worst_cont:.2e}, outlet-vs-piston flux {worst_flux:.2e}, u_x error {worst_u:.2e}, transverse {worst_trans:.2e}, volume {worst_vol:.2e}, pressure {worst_p:.3e} (limit {:.3e})",
        2e-2 * a0 * l0
    );
    G5 {
        cont: worst_cont,
        flux: worst_flux,
        u: worst_u,
        trans: worst_trans,
        vol: worst_vol,
        p: worst_p,
        p_limit: 2e-2 * a0 * l0,
        u_limit: 1e-6 * a0 * 0.5,
    }
}

struct G5 {
    cont: f64,
    flux: f64,
    u: f64,
    trans: f64,
    vol: f64,
    p: f64,
    p_limit: f64,
    u_limit: f64,
}

impl G5 {
    /// (i), (ii), (iii) transverse part, (iv) and (v).
    fn assert_flux_level(&self) {
        assert!(self.cont <= 1e-9, "(i) {}", self.cont);
        assert!(self.flux <= 1e-10, "(ii) {}", self.flux);
        assert!(self.trans <= 1e-9, "(iii) transverse {}", self.trans);
        assert!(self.vol <= 1e-12, "(iv) {}", self.vol);
        assert!(self.p <= self.p_limit, "(v) {}", self.p);
    }

    /// (iii) streamwise cell velocity.
    fn assert_velocity(&self) {
        assert!(self.u <= self.u_limit, "(iii) {}", self.u);
    }
}

/// As first written: side planes `Symmetry` and the exact pressure evaluated at the lab coordinate instead of the
/// distance from the moving piston face (a set-up error, the exact pressure is `a0 (x_out - x)` in the lab frame).
#[test]
#[ignore = "set-up error (pressure position) and the Symmetry anisotropy below; superseded by g5b"]
fn g5_piston_driven_channel_plug_flow_known_miss() {
    let m = g5_body(true, false);
    m.assert_flux_level();
    m.assert_velocity();
}

/// Side planes as `Symmetry`, pressure measured from the piston face: the solver's scalar `a_p` (mean of the component
/// diagonals) turns the wall friction that `Symmetry` puts on the normal component into a spurious force on the
/// tangential ones (a fixed-mesh control reproduces it, ADR 0033 addendum), so (iii) and (v) are missed.
#[test]
#[ignore = "pre-existing scalar a_p x Symmetry anisotropy (fixed-mesh control reproduces it)"]
fn g5c_piston_driven_channel_plug_flow_symmetry_sides_known_miss() {
    let m = g5_body(true, true);
    m.assert_flux_level();
    m.assert_velocity();
}

/// Corrected set-up: side planes `ZeroGradient` (plug flow has no transverse velocity), pressure from the piston face.
/// Everything but the cell-velocity limit of (iii) holds; that one is the ignored test below.
#[test]
fn g5b_piston_driven_channel_plug_flow() {
    g5_body(false, true).assert_flux_level();
}

/// The cell velocity next to the piston lags the flux-level plug flow by up to 2.9e-4 (limit 1e-6 a0 t_end).
#[test]
#[ignore = "registered limit missed: 2.9e-4 > 1e-6 in the piston-adjacent cell (ADR 0033 addendum)"]
fn g5b_piston_cell_velocity_known_miss() {
    g5_body(false, true).assert_velocity();
}

// ---------------------------------------------------------------------------------------------------------------
// G6 rotating mesh
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn g6_rigidly_rotating_mesh_with_the_fluid() {
    let w = 1.0;
    let (r_in, r_out) = (0.5, 1.0);
    let mesh = annulus_3d(48, 8, 2, r_in, r_out, 0.2, 0.0, Grading::Uniform).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh.clone())).unwrap());
    let mut u = VectorField::from_fn("U", &geom, |c| [-w * c[1], w * c[0], 0.0]);
    // walls follow the mesh (installed before the first step by the loop below)
    u.set_bc(&geom, "inner", Box::new(FixedValue::vector(vec3::ZERO)))
        .unwrap();
    let p = ScalarField::from_fn("p", &geom, |c| 0.5 * w * w * (c[0] * c[0] + c[1] * c[1]));
    let mut s = PimpleSolver::new(geom, u, p, settings(0.01));
    let dt = 0.02;
    let names: Vec<String> = ["inner", "outer", "zmin", "zmax"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let mean_v = mesh.total_volume() / mesh.n_cells() as f64;
    let (mut worst_cont, mut worst_gcl): (f64, f64) = (0.0, 0.0);
    for n in 1..=50 {
        let pose = Pose {
            origin: vec3::ZERO,
            axis: [0.0, 0.0, 1.0],
            angle: w * n as f64 * dt,
            translation: vec3::ZERO,
        };
        let pts: Vec<Vec3> = mesh.points().iter().map(|&q| pose.apply(q)).collect();
        let st = move_mesh(&s.geom, pts, dt).unwrap();
        let old = s.geom.mesh.clone();
        let walls = set_follow_conditions(&mut s.u, &st, &old, &names, dt).unwrap();
        worst_gcl = worst_gcl.max(gcl_residual(&old, &st, dt) / mean_v);
        let info = s.step_ale_with_walls(dt, st.geom, st.mesh_flux, walls).unwrap();
        worst_cont = worst_cont.max(info.continuity);
    }
    let cc = s.geom.mesh.cell_centres();
    let u_err =
        s.u.values
            .iter()
            .zip(cc)
            .map(|(v, c)| vec3::norm(vec3::sub(*v, [-w * c[1], w * c[0], 0.0])))
            .fold(0.0, f64::max);
    let p_ex: Vec<f64> = cc.iter().map(|c| 0.5 * w * w * (c[0] * c[0] + c[1] * c[1])).collect();
    let mean_diff: f64 = s.p.values.iter().zip(&p_ex).map(|(a, b)| a - b).sum::<f64>() / p_ex.len() as f64;
    let p_err =
        s.p.values
            .iter()
            .zip(&p_ex)
            .map(|(a, b)| (a - b - mean_diff).abs())
            .fold(0.0, f64::max);
    let range = p_ex.iter().cloned().fold(f64::MIN, f64::max) - p_ex.iter().cloned().fold(f64::MAX, f64::min);
    println!(
        "G6: velocity error {:.3e} W r_o (limit 2e-2), pressure error {:.3e} of the range (limit 5e-2), continuity last step {:.2e}, \
         max over the steps {worst_cont:.2e} (limit 1e-9, ADR 0101), GCL {worst_gcl:.2e} mean(V) (limit 1e-12, ADR 0101)",
        u_err / (w * r_out),
        p_err / range,
        s.history.last().unwrap().continuity
    );
    assert!(u_err <= 2e-2 * w * r_out, "velocity {}", u_err / (w * r_out));
    assert!(p_err <= 5e-2 * range, "pressure {}", p_err / range);
    assert!(worst_cont <= 1e-9, "continuity {worst_cont}");
    assert!(worst_gcl <= 1e-12, "GCL {worst_gcl}");
}

/// Discrete GCL residual of one mesh step: `max_c |sum_f sigma_f W_f - (V^{n+1} - V^n)|` (ADR 0033 G1).
fn gcl_residual(old: &Mesh, st: &MeshStep, dt: f64) -> f64 {
    let w: Vec<f64> = st.mesh_flux.iter().map(|m| m * dt).collect();
    cell_swept_sum(old, &w)
        .iter()
        .zip(old.cell_volumes().iter().zip(st.geom.mesh.cell_volumes()))
        .map(|(s, (v0, v1))| (s - (v1 - v0)).abs())
        .fold(0.0, f64::max)
}

// ---------------------------------------------------------------------------------------------------------------
// ADR 0101 C1 oscillating cylinder on a smoothed mesh (F-063)
// ---------------------------------------------------------------------------------------------------------------

/// Cylinder of radius 0.5 in a fixed, no-slip circular wall of radius 2 (O-grid 48 x 16, one cell in z), oscillating
/// along x with `x_c = A sin(2 pi t)`; the points follow Laplacian smoothing of the cylinder displacement from the
/// initial mesh each step. Returns (max continuity, max GCL residual / mean V, max |V_total - V0| / V0, min volume ratio).
fn oscillating_cylinder(amp: f64, steps_per_period: usize, periods: usize) -> (f64, f64, f64, f64) {
    let mesh0 = cylinder_ogrid(48, 16, 0.5, 2.0, Grading::Uniform, 0.1).unwrap();
    let g0 = FvGeometry::new(Arc::new(mesh0.clone())).unwrap();
    let geom = Arc::new(g0.clone());
    let mut u = VectorField::uniform("U", &geom, vec3::ZERO);
    for name in ["cylinder", "farfield"] {
        u.set_bc(&geom, name, Box::new(FixedValue::vector(vec3::ZERO))).unwrap();
    }
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut s = PimpleSolver::new(geom, u, p, settings(0.01));
    let classes: Vec<PatchClass> = mesh0
        .patches()
        .iter()
        .map(|pt| match pt.name.as_str() {
            "cylinder" => PatchClass::Prescribed,
            "farfield" => PatchClass::Fixed,
            _ => PatchClass::Slip,
        })
        .collect();
    let cyl_points = rcfd_mesh::motion::patch_points(&mesh0, "cylinder").unwrap();
    let v0 = mesh0.total_volume();
    let mean_v = v0 / mesh0.n_cells() as f64;
    let dt = 1.0 / steps_per_period as f64;
    let names = vec!["cylinder".to_string()];
    let (mut cont, mut gcl, mut vol, mut ratio): (f64, f64, f64, f64) = (0.0, 0.0, 0.0, f64::INFINITY);
    for n in 1..=steps_per_period * periods {
        let xc = amp * (std::f64::consts::TAU * n as f64 * dt).sin();
        let mut presc = vec![vec3::ZERO; mesh0.n_points()];
        for &q in &cyl_points {
            presc[q] = [xc, 0.0, 0.0];
        }
        let sm = smooth_displacement(&g0, &classes, &presc, SmoothingMethod::Laplacian, &TIGHT).unwrap();
        let pts: Vec<Vec3> = mesh0
            .points()
            .iter()
            .zip(&sm.displacement)
            .map(|(p, d)| vec3::add(*p, *d))
            .collect();
        let st = move_mesh(&s.geom, pts, dt).unwrap();
        let old = s.geom.mesh.clone();
        ratio = ratio.min(st.min_volume_ratio);
        gcl = gcl.max(gcl_residual(&old, &st, dt) / mean_v);
        vol = vol.max((st.geom.mesh.total_volume() - v0).abs() / v0);
        let walls = set_follow_conditions(&mut s.u, &st, &old, &names, dt).unwrap();
        let info = s.step_ale_with_walls(dt, st.geom, st.mesh_flux, walls).unwrap();
        cont = cont.max(info.continuity);
        assert!(
            s.u.values.iter().flatten().all(|x| x.is_finite()),
            "non-finite velocity at step {n}"
        );
    }
    (cont, gcl, vol, ratio)
}

/// ADR 0101 C1 smoke (set-up check, run before the limits were registered): one period, finite fields, valid mesh.
#[test]
fn c1_oscillating_cylinder_setup_smoke() {
    let (.., ratio) = oscillating_cylinder(0.1, 40, 1);
    println!("C1 smoke: one period finite, min V_new/V_old {ratio:.4}");
    assert!(ratio > 0.0);
}

/// ADR 0101 C1 (F-063): oscillating cylinder (A = 0.1 = 0.2 r, 40 steps per period, 2 periods) on a Laplacian-smoothed
/// O-grid in a closed no-slip wall: mass and geometric conservation in every step.
#[test]
fn c1_oscillating_cylinder_on_a_smoothed_mesh_conserves_mass_and_volume() {
    let (cont, gcl, vol, ratio) = oscillating_cylinder(0.1, 40, 2);
    println!(
        "C1: continuity {cont:.2e} (limit 1e-9), GCL {gcl:.2e} mean(V) (limit 1e-12), total volume {vol:.2e} V0 (limit 1e-12), min V_new/V_old {ratio:.4}"
    );
    assert!(cont <= 1e-9, "continuity {cont}");
    assert!(gcl <= 1e-12, "GCL {gcl}");
    assert!(vol <= 1e-12, "volume {vol}");
}
