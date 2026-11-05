//! ADR 0183 amendment P-AB, library checks of the EXP-ROT prerequisites: P-B, a wall at rest on a rigidly moving mesh (V5,
//! V5-a, the negative control V5-N and the report-only V5-U), and P-A, the Barth-Jespersen limiter of `linear_upwind` on the
//! PISO momentum convection (PA-2 overshoot of a convected top hat, PA-3 Galilean invariance on the moving-mesh step). The
//! limits are those registered in the amendment before this code ran.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::ale::{move_mesh, set_follow_conditions, set_rest_conditions, MeshStep};
use rcfd_fv::bc::{FixedValue, Profile, ZeroGradient};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::gradient::{least_squares, limiter, Limiter};
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{annulus_3d_sector, box_2d, cavity_2d_rect, Grading};
use rcfd_mesh::motion::{cell_swept_sum, Pose};
use rcfd_mesh::Mesh;
use std::sync::Arc;

/// FAN-01 rotation rate (rad/s), axial speed (m/s) and viscosity (m^2/s) of ADR 0183.
const OMEGA: f64 = 155.614;
const U: f64 = 9.478;
const NU: f64 = 1.5e-5;
const R_IN: f64 = 0.124;
const R_OUT: f64 = 0.25;

const TIGHT: SolveControl = SolveControl {
    tol_abs: 1e-16,
    tol_rel: 1e-13,
    max_iter: 5000,
};

/// `Omega dt = 1` degree: 360 steps a revolution.
fn dt_deg() -> f64 {
    std::f64::consts::PI / 180.0 / OMEGA
}

/// PISO, Euler, `linear_upwind` with or without the P-A limiter, strict linear solvers.
fn settings(nu: f64, lim: Option<Limiter>) -> PimpleSettings {
    let mut s = PimpleSettings::piso(nu);
    s.convection = ConvScheme::LinearUpwind;
    s.linear_upwind_limiter = lim;
    s.momentum_solver = TIGHT;
    s.pressure_solver = TIGHT;
    s
}

// ---------------------------------------------------------------------------------------------------------------
// V5: the annulus sector
// ---------------------------------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum Walls {
    /// P-B: walls at rest.
    Rest,
    /// ADR 0033 D3: walls that follow the mesh.
    Follow,
}

/// One run of the sector; maxima over all steps.
#[derive(Debug, Default)]
struct SectorRun {
    steps: usize,
    max_ct: f64,
    max_cr: f64,
    worst_cont: f64,
    worst_gcl: f64,
    seam_pairs: f64,
    wall_sweep: f64,
    stopped: Option<String>,
}

/// The V5 sector `annulus_3d_sector(8, 8, nz, R_IN, R_OUT, height, 40 deg)`; `periodic`: `zmin` / `zmax` a translational
/// cyclic pair (V5), otherwise a velocity inlet and a pressure outlet (V5-U).
fn sector(nz: usize, height: f64, periodic: bool) -> Mesh {
    let mut m = annulus_3d_sector(8, 8, nz, R_IN, R_OUT, height, 40f64.to_radians(), Grading::Uniform).unwrap();
    if periodic {
        m.set_periodic("zmin", "zmax").unwrap();
        m.set_periodic("zmax", "zmin").unwrap();
    }
    m
}

fn walls() -> Vec<String> {
    vec!["inner".to_string(), "outer".to_string()]
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

/// Radial distance of a point from the z axis.
fn radius(x: Vec3) -> f64 {
    (x[0] * x[0] + x[1] * x[1]).sqrt()
}

/// `max_f |m_f + m_g| / (Omega r_f |S_f|)` over the seam pairs (the i-th face of `theta_min` pairs with the i-th face of
/// `theta_max`: both patches are filled in the same (z, r) order by the generator).
fn seam_pair_flux(mesh: &Mesh, flux: &[f64]) -> f64 {
    let (a, b) = (mesh.patch("theta_min").unwrap(), mesh.patch("theta_max").unwrap());
    let sf = mesh.face_area_vectors();
    let fc = mesh.face_centres();
    a.faces()
        .zip(b.faces())
        .map(|(f, g)| (flux[f] + flux[g]).abs() / (OMEGA * radius(fc[f]) * vec3::norm(sf[f])))
        .fold(0.0, f64::max)
}

/// `max_f |m_f| / (Omega r_f |S_f|)` over the faces of `inner` and `outer`.
fn wall_sweep(mesh: &Mesh, flux: &[f64]) -> f64 {
    let sf = mesh.face_area_vectors();
    let fc = mesh.face_centres();
    walls()
        .iter()
        .flat_map(|n| mesh.patch(n).unwrap().faces().collect::<Vec<_>>())
        .map(|f| flux[f].abs() / (OMEGA * radius(fc[f]) * vec3::norm(sf[f])))
        .fold(0.0, f64::max)
}

/// One revolution (360 steps) of the sector, rotating rigidly about z through the origin or at rest.
fn sector_run(periodic: bool, nz: usize, height: f64, rotating: bool, mode: Walls) -> SectorRun {
    let mesh = sector(nz, height, periodic);
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh.clone())).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0, 0.0, U]);
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    for w in walls() {
        u.set_bc(&geom, &w, Box::new(FixedValue::vector(vec3::ZERO))).unwrap();
    }
    if !periodic {
        u.set_bc(&geom, "zmin", Box::new(FixedValue::vector([0.0, 0.0, U])))
            .unwrap();
        u.set_bc(&geom, "zmax", Box::new(ZeroGradient)).unwrap();
        p.set_bc(&geom, "zmax", Box::new(FixedValue::scalar(0.0))).unwrap();
    }
    let mut s = PimpleSolver::new(geom, u, p, settings(NU, Some(Limiter::BarthJespersen)));
    let dt = dt_deg();
    let mean_v = mesh.total_volume() / mesh.n_cells() as f64;
    let mut r = SectorRun::default();
    for n in 1..=360 {
        let info = if rotating {
            let pose = Pose {
                origin: vec3::ZERO,
                axis: [0.0, 0.0, 1.0],
                angle: OMEGA * n as f64 * dt,
                translation: vec3::ZERO,
            };
            let pts: Vec<Vec3> = mesh.points().iter().map(|&q| pose.apply(q)).collect();
            let st = move_mesh(&s.geom, pts, dt).unwrap();
            let old = s.geom.mesh.clone();
            r.worst_gcl = r.worst_gcl.max(gcl_residual(&old, &st, dt) / mean_v);
            if n == 1 {
                r.seam_pairs = seam_pair_flux(&st.geom.mesh, &st.mesh_flux);
            }
            r.wall_sweep = r.wall_sweep.max(wall_sweep(&st.geom.mesh, &st.mesh_flux));
            let dpdn = match mode {
                Walls::Rest => set_rest_conditions(&mut s.u, &st, &walls(), dt).unwrap(),
                Walls::Follow => set_follow_conditions(&mut s.u, &st, &old, &walls(), dt).unwrap(),
            };
            s.step_ale_with_walls(dt, st.geom, st.mesh_flux, dpdn).unwrap()
        } else {
            s.step(dt)
        };
        r.steps = n;
        if let Some(d) = &s.diverged {
            r.stopped = Some(d.message(Some(s.geom.mesh.cell_centres())));
            break;
        }
        r.worst_cont = r.worst_cont.max(info.continuity);
        for (c, v) in s.geom.mesh.cell_centres().iter().zip(&s.u.values) {
            let rad = radius(*c);
            let ct = (c[0] * v[1] - c[1] * v[0]) / rad;
            let cr = (c[0] * v[0] + c[1] * v[1]) / rad;
            r.max_ct = r.max_ct.max(if ct.is_nan() { f64::INFINITY } else { ct.abs() });
            r.max_cr = r.max_cr.max(if cr.is_nan() { f64::INFINITY } else { cr.abs() });
        }
    }
    r
}

/// V5 runs: (C0 on the sector at rest, the rotating sector with walls at rest).
fn v5_runs() -> (SectorRun, SectorRun) {
    (
        sector_run(true, 2, 0.05, false, Walls::Rest),
        sector_run(true, 2, 0.05, true, Walls::Rest),
    )
}

fn v5_limit(c0: f64) -> f64 {
    (10.0 * c0).max(1e-9 * U)
}

/// V5 (ADR 0183, set-up of amendment P-AB): walls at rest on an annulus sector that turns one revolution keep
/// `max |c_theta| <= max(10 C0, 1e-9 U)`.
#[test]
fn v5_walls_at_rest_on_a_rotating_sector_make_no_swirl() {
    let (stat, rot) = v5_runs();
    let limit = v5_limit(stat.max_ct);
    println!(
        "V5: C0 {:.3e} m/s, rotating max |c_theta| {:.3e} m/s, limit {limit:.3e} m/s ({:.3e} U); max |c_r| at rest {:.3e}, rotating {:.3e}; steps {} / {}",
        stat.max_ct,
        rot.max_ct,
        rot.max_ct / U,
        stat.max_cr,
        rot.max_cr,
        stat.steps,
        rot.steps
    );
    assert!(stat.stopped.is_none(), "V5 static run stopped: {:?}", stat.stopped);
    assert!(rot.stopped.is_none(), "V5 rotating run stopped: {:?}", rot.stopped);
    assert!(
        rot.max_ct <= limit,
        "V5: max |c_theta| {:.3e} > {limit:.3e}",
        rot.max_ct
    );
}

/// V5-a: construction of the rotating V5 run (V2 forms): continuity, GCL, seam pairs at step 1; the wall sweep is reported.
#[test]
fn v5a_construction_of_the_rotating_sector() {
    let rot = sector_run(true, 2, 0.05, true, Walls::Rest);
    println!(
        "V5-a: continuity max {:.3e} (1e-9), GCL max {:.3e} mean(V) (1e-12), seam pairs step 1 {:.3e} Omega r |S| (1e-12), wall sweep max |m_f| {:.3e} Omega r |S| (reported)",
        rot.worst_cont, rot.worst_gcl, rot.seam_pairs, rot.wall_sweep
    );
    assert!(rot.stopped.is_none(), "V5-a: stopped: {:?}", rot.stopped);
    assert!(rot.worst_cont <= 1e-9, "V5-a continuity {:.3e}", rot.worst_cont);
    assert!(rot.worst_gcl <= 1e-12, "V5-a GCL {:.3e}", rot.worst_gcl);
    assert!(rot.seam_pairs <= 1e-12, "V5-a seam pairs {:.3e}", rot.seam_pairs);
}

/// V5-N: negative control. The same rotating run with walls that follow the mesh exceeds the V5 limit or stops at the guard.
#[test]
fn v5n_following_walls_exceed_the_v5_limit() {
    let stat = sector_run(true, 2, 0.05, false, Walls::Rest);
    let fol = sector_run(true, 2, 0.05, true, Walls::Follow);
    let limit = v5_limit(stat.max_ct);
    println!(
        "V5-N: following walls max |c_theta| {:.3e} m/s against the V5 limit {limit:.3e} m/s; stopped {:?} after {} steps",
        fol.max_ct, fol.stopped, fol.steps
    );
    assert!(fol.stopped.is_some() || fol.max_ct > limit, "V5-N: {:.3e}", fol.max_ct);
}

/// V5-U (report only, no limit): the literal "axial inflow" variant, a velocity inlet and a pressure outlet; the developing
/// layers carry a radial velocity.
#[test]
#[ignore = "report only (ADR 0183 amendment P-AB, V5-U); seconds in release"]
fn v5u_inflow_variant_report_only() {
    let stat = sector_run(false, 8, 0.2, false, Walls::Rest);
    let rot = sector_run(false, 8, 0.2, true, Walls::Rest);
    println!(
        "V5-U: at rest max |c_theta| {:.3e} m/s, max |c_r| {:.3e} m/s (stopped {:?}); rotating max |c_theta| {:.3e} m/s ({:.3e} U), max |c_r| {:.3e} m/s (stopped {:?}); continuity max {:.3e}",
        stat.max_ct,
        stat.max_cr,
        stat.stopped,
        rot.max_ct,
        rot.max_ct / U,
        rot.max_cr,
        rot.stopped,
        rot.worst_cont
    );
}

// ---------------------------------------------------------------------------------------------------------------
// PA-2: a top hat of u_y convected by a uniform u_x
// ---------------------------------------------------------------------------------------------------------------

/// Largest overshoot `max(u_y - 1, -u_y, 0)` over 80 steps of the convected top hat (`box_2d(100, 2, 1, 0.02)`, periodic
/// in y, `nu = 1e-6`, Courant 0.5).
fn top_hat_overshoot(lim: Option<Limiter>) -> (f64, Vec<Vec3>) {
    let mesh = box_2d(100, 2, 1.0, 0.02, false, true).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let h = |x: f64| if (0.1..0.3).contains(&x) { 1.0 } else { 0.0 };
    let mut u = VectorField::from_fn("U", &geom, |c| [1.0, h(c[0]), 0.0]);
    u.set_bc(&geom, "xmin", Box::new(FixedValue::vector([1.0, 0.0, 0.0])))
        .unwrap();
    u.set_bc(&geom, "xmax", Box::new(ZeroGradient)).unwrap();
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    p.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
    let mut s = PimpleSolver::new(geom, u, p, settings(1e-6, lim));
    let mut o: f64 = 0.0;
    for _ in 0..80 {
        s.step(0.005);
        assert!(
            s.diverged.is_none(),
            "PA-2: guard stop {:?}",
            s.diverged.as_ref().map(|d| d.message(None))
        );
        for v in &s.u.values {
            o = o.max((v[1] - 1.0).max(-v[1]).max(0.0));
        }
    }
    (o, s.u.values.clone())
}

/// PA-2: with P-A the overshoot is at most half the unlimited one, which is at least 1e-3.
#[test]
fn pa2_the_limiter_bounds_a_convected_top_hat() {
    let (o_lu, u_lu) = top_hat_overshoot(None);
    let (o_bj, u_bj) = top_hat_overshoot(Some(Limiter::BarthJespersen));
    let diff = u_lu
        .iter()
        .zip(&u_bj)
        .map(|(a, b)| vec3::norm(vec3::sub(*a, *b)))
        .fold(0.0, f64::max);
    println!(
        "PA-2: overshoot unlimited {o_lu:.4e} (>= 1e-3), limited {o_bj:.4e} (<= 0.5 x unlimited, ratio {:.4}); max |u_BJ - u_LU| at the end {diff:.3e}",
        o_bj / o_lu
    );
    assert!(o_lu >= 1e-3, "PA-2 non-vacuity: unlimited overshoot {o_lu:.3e}");
    assert!(
        o_bj <= 0.5 * o_lu,
        "PA-2: limited {o_bj:.3e} against unlimited {o_lu:.3e}"
    );
}

// ---------------------------------------------------------------------------------------------------------------
// PA-3: Galilean invariance with the limiter on the moving-mesh step (the ADR 0033 G3 cavity)
// ---------------------------------------------------------------------------------------------------------------

/// The G3 lid-driven cavity started from rest in a fixed frame or in a frame translating with `v`, `linear_upwind` with
/// P-A.
fn cavity_run(v: Vec3) -> (PimpleSolver, Vec<Vec3>) {
    let (steps, dt) = (20, 0.02);
    let mesh = cavity_2d_rect(8, 8, 1.0, 1.0).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh.clone())).unwrap());
    let mut u = VectorField::uniform("U", &geom, v);
    u.set_bc(&geom, "walls", Box::new(FixedValue(Profile::Uniform(v))))
        .unwrap();
    u.set_bc(
        &geom,
        "lid",
        Box::new(FixedValue(Profile::Uniform(vec3::add(v, [1.0, 0.0, 0.0])))),
    )
    .unwrap();
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut s = PimpleSolver::new(geom, u, p, settings(0.05, Some(Limiter::BarthJespersen)));
    for n in 1..=steps {
        if v == vec3::ZERO {
            s.step(dt);
        } else {
            let t = n as f64 * dt;
            let pts: Vec<Vec3> = mesh.points().iter().map(|&q| vec3::add(q, vec3::scale(v, t))).collect();
            let st = move_mesh(&s.geom, pts, dt).unwrap();
            s.step_ale(dt, st.geom, st.mesh_flux).unwrap();
        }
    }
    let u_rel = s.u.values.iter().map(|x| vec3::sub(*x, v)).collect();
    (s, u_rel)
}

/// PA-3: `max |u_moving - V - u_fixed| <= 1e-9` with the limiter; at least one limiter factor below 1 in the final fixed
/// state.
#[test]
fn pa3_galilean_invariance_with_the_limiter_on_the_moving_step() {
    let (fixed, u_fixed) = cavity_run(vec3::ZERO);
    let (_, u_mov) = cavity_run([0.6, 0.2, 0.0]);
    let diff = u_fixed
        .iter()
        .zip(&u_mov)
        .map(|(a, b)| vec3::norm(vec3::sub(*a, *b)))
        .fold(0.0, f64::max);
    let g = &fixed.geom;
    let ub = fixed.u.boundary_values(g);
    let mut limited = 0;
    for k in 0..2 {
        let cells: Vec<f64> = fixed.u.values.iter().map(|w| w[k]).collect();
        let b: Vec<f64> = ub.iter().map(|w| w[k]).collect();
        let gr = least_squares(g, &cells, &b);
        limited += limiter(g, &cells, &b, &gr, Limiter::BarthJespersen)
            .iter()
            .filter(|&&x| x < 1.0)
            .count();
    }
    println!("PA-3: max |u_moving - V - u_fixed| {diff:.3e} (1e-9); limiter factors below 1 in the final fixed state: {limited} (cell, component) pairs of {}", 2 * g.n_cells());
    assert!(limited >= 1, "PA-3 non-vacuity: no limited cell");
    assert!(diff <= 1e-9, "PA-3: {diff:.3e}");
}
