//! F-139 (ADR 0256): the co-simulation participant interface `rcfd_fv::cosim`. A mock structural participant (a wall of six
//! lumped, independently sprung nodes) exchanges displacement and force with the moving-mesh flow across a wall whose 16
//! faces do not match the nodes, against the closed form `omega_j^2 = k/(m + rho tanh(kappa_j L)/kappa_j)` of the cosine
//! modes of the wall.
//!
//! The limits, set-ups, model arithmetic and mutations were registered in ADR 0256 before this file existed. The crossings
//! and turning points are those of `strong_fsi_f251.rs` (copied, that file is not edited).

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::bc::{FixedValue, Symmetry, ZeroGradient};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::cosim::{
    coupled_step, piecewise_linear_rows, AtCap, CoupledStep, CouplingSettings, FlowWall, Fluid, InterfaceMap,
    MeshMotion, Participant, SprungWall, Structure,
};
use rcfd_fv::fsi::{
    relaxation_from_added_mass, strong_step_with, BodyCoupling, Relaxation, StrongCoupling, WallPressure,
};
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::sixdof::{
    patch_face_forces, patch_force_moment, patch_force_moment_wall_gradient, Body, BodySpec, BodyState, Integrator,
};
use rcfd_fv::smoothing::{smooth_displacement, PatchClass, SmoothingMethod};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{box_mesh, Grading};
use rcfd_mesh::motion::{moved_mesh, patch_points};
use rcfd_mesh::Mesh;
use std::cell::Cell;
use std::f64::consts::{PI, TAU};
use std::rc::Rc;
use std::sync::{Arc, OnceLock};

const RHO: f64 = 1.0;
const WALL: &str = "xmin";

fn tight(max_iter: usize) -> SolveControl {
    SolveControl {
        tol_abs: 1e-16,
        tol_rel: 1e-13,
        max_iter,
    }
}

/// A channel along x whose wall `xmin` moves, with the smoothing of the interior (the channel of ADR 0250, any section).
struct Channel {
    mesh0: Mesh,
    g0: FvGeometry,
    classes: Vec<PatchClass>,
    wall: Vec<usize>,
    nu: f64,
    /// `ymin` and `ymax` impermeable slip walls (`Symmetry`); otherwise zero-gradient, which leaves their flux free.
    slip_y: bool,
}

impl Channel {
    fn new(cells: [usize; 3], size: [f64; 3], nu: f64, slip_y: bool) -> Arc<Self> {
        let mesh0 = box_mesh(cells, size, [Grading::Uniform; 3]).unwrap();
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
        let wall = patch_points(&mesh0, WALL).unwrap();
        Arc::new(Channel {
            mesh0,
            g0,
            classes,
            wall,
            nu,
            slip_y,
        })
    }

    /// Mesh points for the wall points displaced by `d` (Laplacian smoothing from the reference mesh).
    fn points(&self, d: &[Vec3]) -> Result<Vec<Vec3>, String> {
        let mut presc = vec![vec3::ZERO; self.mesh0.n_points()];
        for (&q, v) in self.wall.iter().zip(d) {
            presc[q] = *v;
        }
        let sm = smooth_displacement(
            &self.g0,
            &self.classes,
            &presc,
            SmoothingMethod::Laplacian,
            &tight(5000),
        )
        .map_err(|e| e.to_string())?;
        Ok(self
            .mesh0
            .points()
            .iter()
            .zip(&sm.displacement)
            .map(|(p, d)| vec3::add(*p, *d))
            .collect())
    }

    fn settings(&self) -> PimpleSettings {
        let mut s = PimpleSettings::piso(self.nu);
        s.convection = ConvScheme::Central;
        s.momentum_solver = tight(5000);
        s.pressure_solver = tight(5000);
        s
    }

    /// Fluid at rest on the mesh with the wall points displaced by `d`.
    fn flow(&self, d: &[Vec3]) -> PimpleSolver {
        let mesh = moved_mesh(&self.mesh0, self.points(d).unwrap()).unwrap();
        let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
        let mut u = VectorField::uniform("U", &geom, vec3::ZERO);
        for name in ["ymin", "ymax"] {
            if self.slip_y {
                u.set_bc(&geom, name, Box::new(Symmetry)).unwrap();
            } else {
                u.set_bc(&geom, name, Box::new(ZeroGradient)).unwrap();
            }
        }
        for name in ["zmin", "zmax"] {
            u.set_bc(&geom, name, Box::new(ZeroGradient)).unwrap();
        }
        let mut p = ScalarField::uniform("p", &geom, 0.0);
        p.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
        PimpleSolver::new(geom, u, p, self.settings())
    }

    fn wall_y(&self) -> Vec<f64> {
        self.wall.iter().map(|&q| self.mesh0.points()[q][1]).collect()
    }
}

fn motion(ch: &Arc<Channel>) -> MeshMotion {
    let ch = ch.clone();
    Box::new(move |d| ch.points(d))
}

// ---- set-up CWs (ADR 0256 P3, G1, amendment 1) -----------------------------------------------------------------------------------

const CW_L: f64 = 1.0;
const CW_W: f64 = 0.4;
const CW_LZ: f64 = 0.1;
const CW_MU: f64 = 4.0;
const CW_X: f64 = 0.002;
const CW_NU: f64 = 1e-4;
const CW_NODES: usize = 6;
const CW_DT: f64 = 0.01;
const CW_STEPS: usize = 650;

/// Surface mass of the wall.
fn cw_m() -> f64 {
    RHO * CW_L / CW_MU
}

/// Foundation spring per unit area: `T_0 = 1`.
fn cw_k() -> f64 {
    TAU * TAU * (cw_m() + RHO * CW_L)
}

fn cw_omega(j: usize) -> f64 {
    let l = if j == 0 {
        CW_L
    } else {
        let kappa = j as f64 * PI / CW_W;
        (kappa * CW_L).tanh() / kappa
    };
    (cw_k() / (cw_m() + RHO * l)).sqrt()
}

/// CWs of amendment 1: `ymin` and `ymax` are slip walls.
fn cw_channel() -> Arc<Channel> {
    Channel::new([20, 8, 2], [CW_L, CW_W, CW_LZ], CW_NU, true)
}

fn cw_node_y() -> Vec<f64> {
    (0..CW_NODES).map(|i| CW_W * i as f64 / (CW_NODES - 1) as f64).collect()
}

/// Tributary length of every node.
fn cw_tributary() -> Vec<f64> {
    let h = CW_W / (CW_NODES - 1) as f64;
    (0..CW_NODES)
        .map(|i| if i == 0 || i == CW_NODES - 1 { 0.5 * h } else { h })
        .collect()
}

fn cw_q0() -> Vec<f64> {
    cw_node_y()
        .iter()
        .map(|y| CW_X * (1.0 + 0.5 * (PI * y / CW_W).cos()))
        .collect()
}

fn cw_structure() -> SprungWall {
    let t = cw_tributary();
    SprungWall::new(
        cw_node_y().iter().map(|y| [0.0, *y, 0.5 * CW_LZ]).collect(),
        [1.0, 0.0, 0.0],
        t.iter().map(|t| cw_m() * CW_LZ * t).collect(),
        t.iter().map(|t| cw_k() * CW_LZ * t).collect(),
        cw_q0(),
    )
    .unwrap()
}

fn cw_rows(ch: &Channel) -> Vec<Vec<(usize, f64)>> {
    piecewise_linear_rows(&cw_node_y(), &ch.wall_y()).unwrap()
}

fn along_x(q: &[f64]) -> Vec<Vec3> {
    q.iter().map(|x| [*x, 0.0, 0.0]).collect()
}

/// Fluid participant at rest on the mesh of the initial wall, the map, the structure.
fn cw_parts(ch: &Arc<Channel>, rho: f64, mesh_motion: MeshMotion) -> (FlowWall, SprungWall, InterfaceMap) {
    let structure = cw_structure();
    let rows = cw_rows(ch);
    let flow = ch.flow(&InterfaceMapProbe(&rows).points(&along_x(&cw_q0())));
    let fluid = FlowWall::new(flow, WALL, &ch.mesh0, rho, CW_NU, mesh_motion).unwrap();
    let map = InterfaceMap::new(rows, CW_NODES, fluid.face_points()).unwrap();
    (fluid, structure, map)
}

/// The consistent map written out in the test (for the initial mesh, before a map object exists).
struct InterfaceMapProbe<'a>(&'a [Vec<(usize, f64)>]);

impl InterfaceMapProbe<'_> {
    fn points(&self, q: &[Vec3]) -> Vec<Vec3> {
        self.0
            .iter()
            .map(|row| {
                row.iter()
                    .fold(vec3::ZERO, |a, &(i, w)| vec3::add(a, vec3::scale(q[i], w)))
            })
            .collect()
    }
}

/// The registered settings (ADR 0256 C3).
fn cw_settings() -> CouplingSettings {
    CouplingSettings {
        relaxation: Relaxation::Aitken {
            initial: relaxation_from_added_mass(cw_m(), RHO * CW_L),
            max: 2.0,
        },
        tolerance: 1e-8,
        force_scale: cw_k() * CW_X * CW_W * CW_LZ,
        max_iterations: 20,
        at_cap: AtCap::Stop,
    }
}

/// Modal coordinates `Q_0`, `Q_1` of nodal displacements.
fn modal(q: &[f64]) -> (f64, f64) {
    let (t, y) = (cw_tributary(), cw_node_y());
    let c: Vec<f64> = y.iter().map(|y| (PI * y / CW_W).cos()).collect();
    let q0 = q.iter().zip(&t).map(|(q, t)| q * t).sum::<f64>() / t.iter().sum::<f64>();
    let q1 = q.iter().zip(&t).zip(&c).map(|((q, t), c)| q * t * c).sum::<f64>()
        / t.iter().zip(&c).map(|(t, c)| t * c * c).sum::<f64>();
    (q0, q1)
}

fn upward_crossings(t: &[f64], x: &[f64]) -> Vec<f64> {
    (1..x.len())
        .filter(|&i| x[i - 1] < 0.0 && x[i] >= 0.0)
        .map(|i| t[i - 1] + (t[i] - t[i - 1]) * (-x[i - 1]) / (x[i] - x[i - 1]))
        .collect()
}

fn measured_omega(t: &[f64], x: &[f64]) -> (f64, usize) {
    let c = upward_crossings(t, x);
    assert!(c.len() >= 2, "fewer than two upward crossings");
    (TAU * (c.len() - 1) as f64 / (c[c.len() - 1] - c[0]), c.len())
}

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

fn worst_growth(x: &[f64]) -> f64 {
    let (mx, mn) = turning_points(x);
    let mut worst: f64 = 0.0;
    for w in mx.windows(2) {
        worst = worst.max(w[1] / w[0]);
    }
    for w in mn.windows(2) {
        worst = worst.max(w[1].abs() / w[0].abs());
    }
    worst
}

/// Record of a CW run.
#[derive(Default)]
struct CwRun {
    t: Vec<f64>,
    q0: Vec<f64>,
    q1: Vec<f64>,
    iterations: Vec<usize>,
    last_residual: f64,
    all_converged: bool,
    continuity: f64,
    min_volume_ratio: f64,
    work_mismatch: f64,
    swept_mismatch: f64,
    force_spread: f64,
    end_times: (f64, f64),
    first_factors: Vec<Vec<f64>>,
    ended: String,
}

impl CwRun {
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
}

fn dot_sum(a: &[Vec3], b: &[Vec3]) -> (f64, f64) {
    let terms: Vec<f64> = a.iter().zip(b).map(|(x, y)| vec3::dot(*x, *y)).collect();
    (terms.iter().sum(), terms.iter().map(|x| x.abs()).sum())
}

/// CW for at most `steps` steps; stops at the first error or when `stop(Q_0)` holds.
fn cw_run(settings: &CouplingSettings, rho: f64, steps: usize, stop: &dyn Fn(f64) -> bool) -> CwRun {
    cw_run_on(&cw_channel(), settings, rho, steps, stop)
}

fn cw_run_on(
    ch: &Arc<Channel>,
    settings: &CouplingSettings,
    rho: f64,
    steps: usize,
    stop: &dyn Fn(f64) -> bool,
) -> CwRun {
    let (mut fluid, mut structure, map) = cw_parts(ch, rho, motion(ch));
    let trib = cw_tributary();
    let wall_faces = ch.mesh0.patch(WALL).unwrap().faces();
    let mut run = CwRun {
        all_converged: true,
        min_volume_ratio: 1.0,
        ended: "completed".into(),
        ..Default::default()
    };
    let (a, b) = modal(&structure.q);
    run.t.push(0.0);
    run.q0.push(a);
    run.q1.push(b);
    for n in 1..=steps {
        let before = structure.q.clone();
        let step = match coupled_step(&mut fluid, &mut structure, &map, CW_DT, settings) {
            Ok(s) => s,
            Err(e) => {
                run.ended = format!("step {n}: {e}");
                break;
            }
        };
        run.iterations.push(step.iterations);
        run.all_converged &= step.converged;
        run.last_residual = run.last_residual.max(*step.residuals.last().unwrap());
        if n <= 3 {
            run.first_factors.push(step.factors.clone());
        }
        let info = fluid.last_step.unwrap();
        run.continuity = run.continuity.max(info.continuity);
        run.min_volume_ratio = run.min_volume_ratio.min(fluid.min_volume_ratio);
        // M3: the work of the flow's forces over the step on the two meshes, and the swept volume of the wall faces
        let dq: Vec<Vec3> = structure
            .q
            .iter()
            .zip(&before)
            .map(|(a, b)| [a - b, 0.0, 0.0])
            .collect();
        let nodal = map.forces_to_nodes(&step.face_forces).unwrap();
        let d_faces = map
            .face_displacements(&map.displacement_to_points(&dq).unwrap())
            .unwrap();
        let (ws, _) = dot_sum(&nodal, &dq);
        let (wf, scale) = dot_sum(&step.face_forces, &d_faces);
        if scale > 0.0 {
            run.work_mismatch = run.work_mismatch.max((ws - wf).abs() / scale);
        }
        let p_new = map.displacement_to_points(&along_x(&structure.q)).unwrap();
        let p_old = map.displacement_to_points(&along_x(&before)).unwrap();
        let moved: Vec<Vec3> = p_new.iter().zip(&p_old).map(|(a, b)| vec3::sub(*a, *b)).collect();
        let mean = map.face_displacements(&moved).unwrap();
        let (mut diff, mut total) = (0.0, 0.0);
        for (k, f) in wall_faces.clone().enumerate() {
            let sx = fluid.flow.geom.mesh.face_area_vectors()[f][0];
            let reference = sx * mean[k][0];
            diff += (fluid.mesh_flux[f] * CW_DT - reference).abs();
            total += reference.abs();
        }
        if total > 0.0 {
            run.swept_mismatch = run.swept_mismatch.max(diff / total);
        }
        let per_length: Vec<f64> = step.node_forces.iter().zip(&trib).map(|(f, t)| f[0] / t).collect();
        let spread =
            per_length.iter().fold(f64::MIN, |a, b| a.max(*b)) - per_length.iter().fold(f64::MAX, |a, b| a.min(*b));
        run.force_spread = run.force_spread.max(spread / (cw_k() * CW_X * CW_LZ));
        let (a, b) = modal(&structure.q);
        run.t.push(structure.t);
        run.q0.push(a);
        run.q1.push(b);
        if !a.is_finite() || stop(a) {
            run.ended = format!("step {n}: stop condition");
            break;
        }
    }
    run.end_times = (fluid.time(), structure.time());
    run
}

/// The registered run of CW (A1 to A5, S5, M3 read the same run).
fn cw_registered() -> &'static CwRun {
    static RUN: OnceLock<CwRun> = OnceLock::new();
    RUN.get_or_init(|| {
        let run = cw_run(&cw_settings(), RHO, CW_STEPS, &|_| false);
        println!(
            "CW: ended '{}', {} steps, iterations max {} mean {:.3}, histogram {:?}",
            run.ended,
            run.iterations.len(),
            run.max_iterations(),
            run.mean_iterations(),
            run.histogram()
        );
        println!("CW: Aitken factors of the first three steps {:?}", run.first_factors);
        let (mx0, mn0) = turning_points(&run.q0);
        let (mx1, mn1) = turning_points(&run.q1);
        println!("CW: Q0 maxima {mx0:?}\nCW: Q0 minima {mn0:?}\nCW: Q1 maxima {mx1:?}\nCW: Q1 minima {mn1:?}");
        run
    })
}

// ---- acceptance ----------------------------------------------------------------------------------------------------

#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn a1_the_coupled_iteration_converges_every_step() {
    let run = cw_registered();
    println!(
        "A1: converged {}, iterations max {} mean {:.3}, largest last test quantity {:.3e}",
        run.all_converged,
        run.max_iterations(),
        run.mean_iterations(),
        run.last_residual
    );
    assert_eq!(run.ended, "completed");
    assert_eq!(run.iterations.len(), CW_STEPS);
    assert!(run.all_converged);
    assert!(run.max_iterations() <= 16);
    assert!(run.mean_iterations() <= 12.0);
    assert!(run.last_residual <= 1e-8);
}

fn cw_frequencies() -> ((f64, usize), (f64, usize)) {
    let run = cw_registered();
    let (w0, n0) = measured_omega(&run.t, &run.q0);
    let (w1, n1) = measured_omega(&run.t, &run.q1);
    ((w0 / cw_omega(0), n0), (w1 / cw_omega(1), n1))
}

#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn a2_the_two_modes_oscillate_at_the_closed_form() {
    let ((r0, n0), (r1, n1)) = cw_frequencies();
    println!(
        "A2: omega_0 closed {:.6}, omega_1 closed {:.6}; omega_m/omega: Q0 {r0:.6} ({n0} crossings), Q1 {r1:.6} ({n1} crossings)",
        cw_omega(0),
        cw_omega(1)
    );
    assert!((r0 - 1.0).abs() <= 0.005);
    assert!((r1 - 1.0).abs() <= 0.03);
    assert!(n0 >= 6 && n1 >= 11);
}

/// The set-up CW as registered, before amendment 1: `ymin` and `ymax` zero-gradient, which leaves them open to the
/// transverse flow of mode 1. Kept with A2's limits as the record.
#[test]
#[ignore = "record of the registered set-up (ADR 0256 amendment 1): side walls open to the transverse flow"]
fn a2_registered_set_up_zero_gradient_sides_record() {
    let ch = Channel::new([20, 8, 2], [CW_L, CW_W, CW_LZ], CW_NU, false);
    let run = cw_run_on(&ch, &cw_settings(), RHO, CW_STEPS, &|_| false);
    println!(
        "A2 record: ended '{}', {} steps, iterations max {} mean {:.3}",
        run.ended,
        run.iterations.len(),
        run.max_iterations(),
        run.mean_iterations()
    );
    let (w0, n0) = measured_omega(&run.t, &run.q0);
    let (w1, n1) = measured_omega(&run.t, &run.q1);
    let (r0, r1) = (w0 / cw_omega(0), w1 / cw_omega(1));
    println!("A2 record: omega_m/omega: Q0 {r0:.6} ({n0} crossings), Q1 {r1:.6} ({n1} crossings)");
    assert!((r0 - 1.0).abs() <= 0.005);
    assert!((r1 - 1.0).abs() <= 0.03);
    assert!(n0 >= 6 && n1 >= 11);
}

#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn a3_the_frequencies_are_those_of_the_discrete_model() {
    let ((r0, _), (r1, _)) = cw_frequencies();
    println!(
        "A3: Q0 {r0:.6} against the model 0.998625 ({:+.2e}); Q1 {r1:.6} against 1.011470 ({:+.2e})",
        r0 - 0.998625,
        r1 - 1.011470
    );
    assert!((r0 - 0.998625).abs() <= 3e-3);
    assert!((r1 - 1.011470).abs() <= 5e-3);
}

#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn a4_the_turning_points_of_both_modes_never_grow() {
    let run = cw_registered();
    let (g0, g1) = (worst_growth(&run.q0), worst_growth(&run.q1));
    println!("A4: largest ratio of successive same-side turning points: Q0 {g0:.5}, Q1 {g1:.5}");
    assert!(g0 <= 1.0 + 1e-3);
    assert!(g1 <= 1.0 + 1e-3);
}

#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn a5_displacement_and_force_are_fields() {
    let q = cw_q0();
    let spread = q.iter().fold(f64::MIN, |a, b| a.max(*b)) - q.iter().fold(f64::MAX, |a, b| a.min(*b));
    let run = cw_registered();
    println!(
        "A5: spread of the initial displacement {spread:e} (X = {CW_X}); largest spread of the force per length / (k X Lz) {:.4}",
        run.force_spread
    );
    assert!((spread - CW_X).abs() <= 1e-12 * CW_X);
    assert!(run.force_spread >= 0.1);
}

#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn s5_set_up_continuity_volume_and_times() {
    let run = cw_registered();
    println!(
        "S5: continuity {:.3e}, min volume ratio {:.6}, end times {:?}",
        run.continuity, run.min_volume_ratio, run.end_times
    );
    assert!(run.continuity <= 1e-10);
    assert!(run.min_volume_ratio >= 0.9);
    assert!((run.end_times.0 - 6.5).abs() <= 1e-9 && (run.end_times.1 - 6.5).abs() <= 1e-9);
}

#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn sn1_without_the_fluid_force_the_frequency_is_far_off() {
    let run = cw_run(&cw_settings(), 0.0, CW_STEPS, &|_| false);
    assert_eq!(run.ended, "completed");
    let (w0, n0) = measured_omega(&run.t, &run.q0);
    println!(
        "SN1: fluid force off: omega_m/omega_0 {:.6} ({n0} crossings), iterations max {}",
        w0 / cw_omega(0),
        run.max_iterations()
    );
    assert!((w0 / cw_omega(0) - 1.0).abs() >= 0.08);
}

// ---- mapping -------------------------------------------------------------------------------------------------------

/// A fixed pseudo-random sequence in `(-1, 1)`.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0
    }
    fn vectors(&mut self, n: usize) -> Vec<Vec3> {
        (0..n).map(|_| [self.next(), self.next(), self.next()]).collect()
    }
}

#[test]
fn m1_the_displacement_map_is_consistent() {
    let ch = cw_channel();
    let (_, _, map) = cw_parts(&ch, RHO, motion(&ch));
    let rows = cw_rows(&ch);
    let row_error = rows
        .iter()
        .map(|r| (r.iter().map(|x| x.1).sum::<f64>() - 1.0).abs())
        .fold(0.0, f64::max);
    let uniform = map.displacement_to_points(&[[0.37, -1.2, 2.5e-3]; CW_NODES]).unwrap();
    let uniform_error = uniform
        .iter()
        .flat_map(|d| [(d[0] - 0.37) / 0.37, (d[1] + 1.2) / 1.2, (d[2] - 2.5e-3) / 2.5e-3])
        .fold(0.0f64, |a, b| a.max(b.abs()));
    let line = |y: f64| 0.3 - 1.7 * y;
    let linear = map
        .displacement_to_points(&cw_node_y().iter().map(|y| [line(*y), 0.0, 0.0]).collect::<Vec<_>>())
        .unwrap();
    let linear_error = linear
        .iter()
        .zip(ch.wall_y())
        .map(|(d, y)| (d[0] - line(y)).abs() / 0.38)
        .fold(0.0, f64::max);
    let entries: usize = rows.iter().map(|r| r.len()).sum();
    println!(
        "M1: row sums {row_error:e}, uniform {uniform_error:e}, linear {linear_error:e}; {} points, {entries} weights",
        rows.len()
    );
    assert_eq!(rows.len(), 27);
    assert!(row_error <= 1e-15);
    assert!(uniform_error <= 1e-15);
    assert!(linear_error <= 1e-15);
}

#[test]
fn m2_the_force_map_conserves_the_resultant_and_the_work() {
    let ch = cw_channel();
    let (_, _, map) = cw_parts(&ch, RHO, motion(&ch));
    let mut rng = Lcg(139);
    let forces = rng.vectors(map.n_faces());
    let q = rng.vectors(map.n_nodes());
    let nodal = map.forces_to_nodes(&forces).unwrap();
    let mut resultant: f64 = 0.0;
    for k in 0..3 {
        let a: f64 = nodal.iter().map(|f| f[k]).sum();
        let b: f64 = forces.iter().map(|f| f[k]).sum();
        let scale: f64 = forces.iter().map(|f| f[k].abs()).sum();
        resultant = resultant.max((a - b).abs() / scale);
    }
    let d_faces = map
        .face_displacements(&map.displacement_to_points(&q).unwrap())
        .unwrap();
    let (ws, _) = dot_sum(&nodal, &q);
    let (wf, scale) = dot_sum(&forces, &d_faces);
    println!(
        "M2: resultant {resultant:e}, work structure {ws:.15e} flow {wf:.15e} relative {:e}",
        (ws - wf).abs() / scale
    );
    assert_eq!(map.n_faces(), 16);
    assert!(resultant <= 1e-13);
    assert!((ws - wf).abs() / scale <= 1e-13);
}

#[test]
fn mn1_an_averaging_force_map_loses_the_resultant() {
    let ch = cw_channel();
    let (fluid, _, _) = cw_parts(&ch, RHO, motion(&ch));
    let rows = cw_rows(&ch);
    let forces = vec![[1.0, 0.0, 0.0]; fluid.face_points().len()];
    let (mut sum, mut count) = (vec![0.0; CW_NODES], vec![0usize; CW_NODES]);
    for (pts, force) in fluid.face_points().iter().zip(&forces) {
        let mut seen = vec![false; CW_NODES];
        for &p in pts {
            for &(i, w) in &rows[p] {
                sum[i] += w * force[0] / pts.len() as f64;
                seen[i] = true;
            }
        }
        for (c, s) in count.iter_mut().zip(seen) {
            *c += s as usize;
        }
    }
    let averaged: f64 = sum.iter().zip(&count).map(|(s, c)| s / *c as f64).sum();
    let total: f64 = forces.iter().map(|f| f[0]).sum();
    println!("MN1: averaged map {averaged:.4} against the resultant {total:.4}");
    assert!((averaged - total).abs() / total >= 0.1);
}

#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn m3_work_and_swept_volume_agree_on_the_two_meshes_in_the_run() {
    let run = cw_registered();
    println!(
        "M3: work mismatch {:.3e}, swept-volume mismatch {:.3e}",
        run.work_mismatch, run.swept_mismatch
    );
    assert_eq!(run.iterations.len(), CW_STEPS);
    assert!(run.work_mismatch <= 1e-10);
    assert!(run.swept_mismatch <= 1e-10);
}

// ---- checkpoint and failure ----------------------------------------------------------------------------------------

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

/// Every value of the fluid participant's state, as bits.
fn fluid_bits(f: &FlowWall) -> Vec<u64> {
    let mut out = Vec::new();
    out.extend(f.flow.u.values.iter().flatten().map(|x| x.to_bits()));
    out.extend(bits(&f.flow.p.values));
    out.extend(bits(&f.flow.phi));
    out.push(f.flow.time.to_bits());
    out.extend(f.flow.geom.mesh.points().iter().flatten().map(|x| x.to_bits()));
    out.extend(
        f.flow
            .u
            .boundary_values(&f.flow.geom)
            .iter()
            .flatten()
            .map(|x| x.to_bits()),
    );
    out.extend(bits(&f.wall_dpdn));
    out.push(f.flow.history.len() as u64);
    out
}

fn structure_bits(s: &SprungWall) -> Vec<u64> {
    let mut out = bits(&s.q);
    out.extend(bits(&s.v));
    out.push(s.t.to_bits());
    out
}

fn differing(a: &[u64], b: &[u64]) -> usize {
    assert_eq!(a.len(), b.len());
    a.iter().zip(b).filter(|(x, y)| x != y).count()
}

fn step_bits(s: &CoupledStep) -> Vec<u64> {
    let mut out = vec![s.iterations as u64, s.converged as u64];
    out.extend(bits(&s.residuals));
    out.extend(bits(&s.residual_norms));
    out.extend(bits(&s.factors));
    for v in [&s.node_forces, &s.node_displacements, &s.face_forces] {
        out.extend(v.iter().flatten().map(|x| x.to_bits()));
    }
    out
}

#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn r1_a_repeated_step_restores_both_participants_bit_for_bit() {
    let ch = cw_channel();
    let (mut fluid, mut structure, map) = cw_parts(&ch, RHO, motion(&ch));
    for _ in 0..37 {
        coupled_step(&mut fluid, &mut structure, &map, CW_DT, &cw_settings()).unwrap();
    }
    let (f0, s0) = (fluid_bits(&fluid), structure_bits(&structure));
    let first = coupled_step(&mut fluid, &mut structure, &map, CW_DT, &cw_settings()).unwrap();
    let (f1, s1) = (fluid_bits(&fluid), structure_bits(&structure));
    assert!(differing(&f0, &f1) > 100, "step 38 changed the flow");
    fluid.restore().unwrap();
    structure.restore().unwrap();
    let back = (
        differing(&f0, &fluid_bits(&fluid)),
        differing(&s0, &structure_bits(&structure)),
    );
    let second = coupled_step(&mut fluid, &mut structure, &map, CW_DT, &cw_settings()).unwrap();
    let again = (
        differing(&f1, &fluid_bits(&fluid)),
        differing(&s1, &structure_bits(&structure)),
        differing(&step_bits(&first), &step_bits(&second)),
    );
    println!(
        "R1: step 38 took {} iterations; values differing after restore {back:?} (of {} and {}); after the repeated step {again:?}",
        first.iterations,
        f0.len(),
        s0.len()
    );
    assert_eq!(back, (0, 0));
    assert_eq!(again, (0, 0, 0));
}

/// What a faulty structure does at its `fail_at`-th call of `advance`.
#[derive(Clone, Copy, PartialEq)]
enum Fault {
    Error,
    TimeOff,
    NotANumber,
}

struct FaultyStructure {
    inner: SprungWall,
    calls: usize,
    fail_at: usize,
    fault: Fault,
}

impl Participant for FaultyStructure {
    fn name(&self) -> &str {
        self.inner.name()
    }
    fn time(&self) -> f64 {
        self.inner.time()
    }
    fn checkpoint(&mut self) -> Result<(), String> {
        self.inner.checkpoint()
    }
    fn restore(&mut self) -> Result<(), String> {
        self.inner.restore()
    }
}

impl Structure for FaultyStructure {
    fn node_coordinates(&self) -> Vec<Vec3> {
        self.inner.node_coordinates()
    }
    fn advance(&mut self, dt: f64, forces: &[Vec3]) -> Result<Vec<Vec3>, String> {
        self.calls += 1;
        let hit = self.calls == self.fail_at;
        if hit && self.fault == Fault::Error {
            return Err("injected failure of the structural solver".into());
        }
        let mut q = self.inner.advance(dt, forces)?;
        if hit && self.fault == Fault::TimeOff {
            self.inner.t -= 0.5 * dt;
        }
        if hit && self.fault == Fault::NotANumber {
            q[2][0] = f64::NAN;
        }
        Ok(q)
    }
}

#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn e1_a_failing_participant_stops_the_step_with_a_located_error() {
    let ch = cw_channel();
    // (a), (c), (d): the structure fails
    for (fault, fail_at, iteration, label) in [
        (Fault::Error, 2, 2, "a"),
        (Fault::TimeOff, 1, 1, "c"),
        (Fault::NotANumber, 1, 1, "d"),
    ] {
        let (mut fluid, structure, map) = cw_parts(&ch, RHO, motion(&ch));
        let mut faulty = FaultyStructure {
            inner: structure,
            calls: 0,
            fail_at,
            fault,
        };
        let (f0, s0) = (fluid_bits(&fluid), structure_bits(&faulty.inner));
        let e = coupled_step(&mut fluid, &mut faulty, &map, CW_DT, &cw_settings()).unwrap_err();
        println!("E1 ({label}): {e}");
        assert_eq!(e.participant, "structure");
        assert_eq!(e.time, 0.0);
        assert_eq!(e.iteration, iteration);
        assert_eq!(differing(&f0, &fluid_bits(&fluid)), 0);
        assert_eq!(differing(&s0, &structure_bits(&faulty.inner)), 0);
        if fault == Fault::Error {
            assert!(e.message.contains("injected failure"));
            let step = coupled_step(&mut fluid, &mut faulty, &map, CW_DT, &cw_settings()).unwrap();
            println!("E1 (a): the next step converged in {} iterations", step.iterations);
            assert!(step.converged);
        }
    }
    // (b): the fluid's mesh motion fails at its second call
    {
        let calls = Rc::new(Cell::new(0usize));
        let (counter, channel) = (calls.clone(), ch.clone());
        let failing: MeshMotion = Box::new(move |d| {
            counter.set(counter.get() + 1);
            if counter.get() == 2 {
                return Err("injected failure of the mesh motion".into());
            }
            channel.points(d)
        });
        let (mut fluid, mut structure, map) = cw_parts(&ch, RHO, failing);
        let (f0, s0) = (fluid_bits(&fluid), structure_bits(&structure));
        let e = coupled_step(&mut fluid, &mut structure, &map, CW_DT, &cw_settings()).unwrap_err();
        println!("E1 (b): {e}");
        assert_eq!((e.participant.as_str(), e.time, e.iteration), ("fluid", 0.0, 2));
        assert!(e.message.contains("injected failure"));
        assert_eq!(calls.get(), 2);
        assert_eq!(differing(&f0, &fluid_bits(&fluid)), 0);
        assert_eq!(differing(&s0, &structure_bits(&structure)), 0);
    }
    // (e), (f): the cap
    let mut plain = cw_settings();
    plain.relaxation = Relaxation::Fixed(1.0);
    plain.max_iterations = 5;
    {
        let (mut fluid, mut structure, map) = cw_parts(&ch, RHO, motion(&ch));
        let (f0, s0) = (fluid_bits(&fluid), structure_bits(&structure));
        let e = coupled_step(&mut fluid, &mut structure, &map, CW_DT, &plain).unwrap_err();
        println!("E1 (e): {e}");
        assert_eq!((e.participant.as_str(), e.time, e.iteration), ("coupler", 0.0, 5));
        assert!(e.message.contains("did not converge"));
        assert_eq!(differing(&f0, &fluid_bits(&fluid)), 0);
        assert_eq!(differing(&s0, &structure_bits(&structure)), 0);
        plain.at_cap = AtCap::Keep;
        let step = coupled_step(&mut fluid, &mut structure, &map, CW_DT, &plain).unwrap();
        println!(
            "E1 (f): kept, converged {}, {} iterations",
            step.converged, step.iterations
        );
        assert!(!step.converged);
        assert_eq!(step.iterations, 5);
        assert!((structure.time() - CW_DT).abs() <= 1e-15);
    }
    // (g): invalid settings and a map that does not fit
    {
        let (mut fluid, mut structure, map) = cw_parts(&ch, RHO, motion(&ch));
        let (f0, s0) = (fluid_bits(&fluid), structure_bits(&structure));
        let mut bad = Vec::new();
        let mut s = cw_settings();
        s.tolerance = 0.0;
        bad.push(s);
        let mut s = cw_settings();
        s.max_iterations = 0;
        bad.push(s);
        let mut s = cw_settings();
        s.relaxation = Relaxation::Fixed(0.0);
        bad.push(s);
        let mut s = cw_settings();
        s.force_scale = f64::NAN;
        bad.push(s);
        for s in &bad {
            let e = coupled_step(&mut fluid, &mut structure, &map, CW_DT, s).unwrap_err();
            println!("E1 (g): {e}");
            assert_eq!((e.participant.as_str(), e.iteration), ("coupler", 0));
        }
        let five: Vec<f64> = (0..5).map(|i| CW_W * i as f64 / 4.0).collect();
        let wrong = InterfaceMap::new(
            piecewise_linear_rows(&five, &ch.wall_y()).unwrap(),
            5,
            fluid.face_points(),
        )
        .unwrap();
        let e = coupled_step(&mut fluid, &mut structure, &wrong, CW_DT, &cw_settings()).unwrap_err();
        println!("E1 (g): {e}");
        assert_eq!((e.participant.as_str(), e.iteration), ("coupler", 0));
        assert!(e.message.contains("5 nodes"));
        assert_eq!(differing(&f0, &fluid_bits(&fluid)), 0);
        assert_eq!(differing(&s0, &structure_bits(&structure)), 0);
    }
}

// ---- controls ------------------------------------------------------------------------------------------------------

#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn n1_the_plain_fixed_point_iteration_diverges() {
    let ch = cw_channel();
    let (mut fluid, mut structure, map) = cw_parts(&ch, RHO, motion(&ch));
    let mut s = cw_settings();
    s.relaxation = Relaxation::Fixed(1.0);
    s.max_iterations = 10;
    s.at_cap = AtCap::Keep;
    let step = coupled_step(&mut fluid, &mut structure, &map, CW_DT, &s).unwrap();
    let r = &step.residual_norms;
    let ratios: Vec<f64> = r.windows(2).map(|w| w[1] / w[0]).collect();
    println!(
        "N1: converged {}, |r_10|/|r_1| {:.2}, ratios {ratios:.5?}",
        step.converged,
        r[9] / r[0]
    );
    assert!(!step.converged);
    assert_eq!(r.len(), 10);
    assert!(r[9] / r[0] >= 200.0);
}

#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn n2_the_loose_coupling_grows() {
    let mut s = cw_settings();
    s.max_iterations = 1;
    s.at_cap = AtCap::Keep;
    let run = cw_run(&s, RHO, CW_STEPS, &|q0| q0.abs() >= 10.0 * CW_X);
    let ended_at = run.t.len() - 1 + usize::from(run.ended.contains("co-simulation"));
    println!("N2: loose coupling ended at step {ended_at}: {}", run.ended);
    assert_ne!(run.ended, "completed");
    assert!(ended_at <= 60);
}

#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn c3_the_added_mass_factor_converges_as_a_fixed_factor() {
    let ch = cw_channel();
    let (mut fluid, mut structure, map) = cw_parts(&ch, RHO, motion(&ch));
    let mut s = cw_settings();
    s.relaxation = Relaxation::Fixed(relaxation_from_added_mass(cw_m(), RHO * CW_L));
    s.max_iterations = 60;
    let step = coupled_step(&mut fluid, &mut structure, &map, CW_DT, &s).unwrap();
    println!("C3: converged {} in {} iterations", step.converged, step.iterations);
    assert!(step.converged);
    assert!(step.iterations <= 45);
}

// ---- one rigid segment: ADR 0250's strong step ---------------------------------------------------------------------

const SP_A: f64 = 0.04;
const SP_L0: f64 = 1.0;
const SP_X: f64 = 0.01;
const SP_NU: f64 = 0.01;
const SP_MU: f64 = 4.0;

fn sp_mass() -> f64 {
    RHO * SP_A * SP_L0 / SP_MU
}

fn sp_stiffness() -> f64 {
    TAU * TAU * (sp_mass() + RHO * SP_A * SP_L0)
}

fn sp_channel() -> Arc<Channel> {
    Channel::new([20, 2, 2], [SP_L0, 0.2, 0.2], SP_NU, false)
}

/// The one-node participants of SPm1.
fn sp_parts(ch: &Arc<Channel>) -> (FlowWall, SprungWall, InterfaceMap) {
    let n = ch.wall.len();
    let flow = ch.flow(&vec![[SP_X, 0.0, 0.0]; n]);
    let fluid = FlowWall::new(flow, WALL, &ch.mesh0, RHO, SP_NU, motion(ch)).unwrap();
    let structure = SprungWall::new(
        vec![[0.0, 0.1, 0.1]],
        [1.0, 0.0, 0.0],
        vec![sp_mass()],
        vec![sp_stiffness()],
        vec![SP_X],
    )
    .unwrap();
    let rows = piecewise_linear_rows(&[0.1], &ch.wall_y()).unwrap();
    let map = InterfaceMap::new(rows, 1, fluid.face_points()).unwrap();
    (fluid, structure, map)
}

fn sp_settings() -> CouplingSettings {
    CouplingSettings {
        relaxation: Relaxation::Aitken {
            initial: relaxation_from_added_mass(sp_mass(), RHO * SP_A * SP_L0),
            max: 2.0,
        },
        tolerance: 1e-8,
        force_scale: sp_stiffness() * SP_X,
        max_iterations: 20,
        at_cap: AtCap::Stop,
    }
}

#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn q1_one_rigid_segment_is_the_strong_step_of_adr_0250() {
    let ch = sp_channel();
    let dt = 0.01;
    // the strong step of ADR 0250 with the wall pressure of ADR 0252
    let n = ch.wall.len();
    let mut flow = ch.flow(&vec![[SP_X, 0.0, 0.0]; n]);
    let mut spec = BodySpec::new(sp_mass(), [1.0, 1.0, 1.0]);
    spec.translation_locked = [false, true, true];
    spec.stiffness = [sp_stiffness(), 0.0, 0.0];
    let mut body = Body::new(spec, BodyState::at([SP_X, 0.0, 0.0])).unwrap();
    let coupling = BodyCoupling {
        rho: RHO,
        nu: SP_NU,
        force_patches: vec![WALL.to_string()],
        follow_patches: vec![WALL.to_string()],
        integrator: Integrator::Rk4,
    };
    let s = sp_settings();
    let strong = StrongCoupling {
        relaxation: s.relaxation,
        tolerance: s.tolerance,
        force_scale: s.force_scale,
        moment_scale: 0.0,
        max_iterations: s.max_iterations,
    };
    let mut wall_dpdn = Vec::new();
    let (mut xs, mut its_strong) = (Vec::new(), Vec::new());
    for _ in 0..100 {
        let step = strong_step_with(
            &mut flow,
            &mut body,
            dt,
            &coupling,
            &strong,
            WallPressure::PrescribedGradient,
            &mut wall_dpdn,
            &mut |b: &Body| ch.points(&vec![[b.state.x[0], 0.0, 0.0]; n]),
        )
        .unwrap();
        assert!(step.converged);
        xs.push(body.state.x[0]);
        its_strong.push(step.iterations);
    }
    // the same through the participant interface
    let (mut fluid, mut structure, map) = sp_parts(&ch);
    let (mut worst, mut differing_counts) = (0.0f64, 0usize);
    for k in 0..100 {
        let step = coupled_step(&mut fluid, &mut structure, &map, dt, &s).unwrap();
        assert!(step.converged);
        worst = worst.max((structure.q[0] - xs[k]).abs());
        differing_counts += usize::from(step.iterations != its_strong[k]);
    }
    let dv = (structure.v[0] - body.state.v[0]).abs();
    let p_scale = flow.p.values.iter().fold(0.0f64, |a, b| a.max(b.abs()));
    let dp = fluid
        .flow
        .p
        .values
        .iter()
        .zip(&flow.p.values)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f64::max);
    println!(
        "Q1: max |x - x_strong| {worst:e} (X = {SP_X}), |v - v_strong| {dv:e}, steps with another iteration count {differing_counts}, max |p - p_strong|/max|p| {:e}; strong iterations max {}",
        dp / p_scale,
        its_strong.iter().max().unwrap()
    );
    assert!(worst <= 1e-8 * SP_X);
    assert!(dv <= 1e-8 * TAU * SP_X);
    assert!(differing_counts <= 2);
    assert!(dp / p_scale <= 1e-6);
}

// ---- defaults ------------------------------------------------------------------------------------------------------

/// Sum of the face loads in face order, and their moment about `x_ref`, as `force_moment` accumulates them.
fn summed(fluid: &FlowWall, gradient: Option<&[f64]>, x_ref: Vec3) -> (Vec3, Vec3) {
    let geom = &fluid.flow.geom;
    let loads = patch_face_forces(
        geom,
        &fluid.flow.u,
        &fluid.flow.p,
        fluid.rho,
        fluid.nu,
        &[WALL.to_string()],
        gradient,
    )
    .unwrap();
    let (mut force, mut moment) = (vec3::ZERO, vec3::ZERO);
    for (f, df) in geom.mesh.patch(WALL).unwrap().faces().zip(&loads) {
        force = vec3::add(force, *df);
        moment = vec3::add(moment, vec3::cross(vec3::sub(geom.mesh.face_centres()[f], x_ref), *df));
    }
    (force, moment)
}

fn vec_bits(v: (Vec3, Vec3)) -> Vec<u64> {
    v.0.iter().chain(&v.1).map(|x| x.to_bits()).collect()
}

#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn b2_the_face_loads_sum_to_the_existing_resultants_bit_for_bit() {
    let x_ref = [0.013, 0.07, -0.02];
    let patches = [WALL.to_string()];
    let cw = cw_channel();
    let sp = sp_channel();
    let (mut fa, mut sa, ma) = cw_parts(&cw, RHO, motion(&cw));
    let (mut fb, mut sb, mb) = sp_parts(&sp);
    for _ in 0..5 {
        coupled_step(&mut fa, &mut sa, &ma, CW_DT, &cw_settings()).unwrap();
        coupled_step(&mut fb, &mut sb, &mb, 0.01, &sp_settings()).unwrap();
    }
    for (label, fluid) in [("CW", &fa), ("SPm1", &fb)] {
        let (g, u, p) = (&fluid.flow.geom, &fluid.flow.u, &fluid.flow.p);
        let default = patch_force_moment(g, u, p, fluid.rho, fluid.nu, &patches, x_ref).unwrap();
        let gradient =
            patch_force_moment_wall_gradient(g, u, p, fluid.rho, fluid.nu, &patches, x_ref, &fluid.wall_dpdn).unwrap();
        let a = differing(&vec_bits(default), &vec_bits(summed(fluid, None, x_ref)));
        let b = differing(
            &vec_bits(gradient),
            &vec_bits(summed(fluid, Some(&fluid.wall_dpdn), x_ref)),
        );
        println!(
            "B2 {label}: default force {:?}, with the wall gradient {:?}; values differing {a} and {b}",
            default.0, gradient.0
        );
        assert_ne!(default.0[0].to_bits(), gradient.0[0].to_bits());
        assert!(default.0[0] != 0.0);
        assert_eq!((a, b), (0, 0));
    }
}
