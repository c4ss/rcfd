//! F-361 (ADR 0261): the least-squares (interface quasi-Newton) update of the nodal force in the coupled iteration of
//! `rcfd_fv::cosim`, on the model problem of ADR 0256 (a sprung wall of six nodes against 16 non-matching wall faces), on
//! its one-node piston, and on mock affine fluids.
//!
//! The limits, set-ups, model arithmetic and mutations were registered in ADR 0261 before this file existed. The set-ups
//! CWs and SPm1, the modal coordinates and the crossings are copied from `cosim_f139.rs` (that file is not edited).

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::bc::{FixedValue, Symmetry, ZeroGradient};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::cosim::{
    coupled_step, piecewise_linear_rows, AtCap, CoupledStep, CouplingSettings, FlowWall, Fluid, InterfaceMap,
    MeshMotion, Participant, SprungWall,
};
use rcfd_fv::fsi::{relaxation_from_added_mass, Relaxation, StrongCoupling};
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::smoothing::{smooth_displacement, PatchClass, SmoothingMethod};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{box_mesh, Grading};
use rcfd_mesh::motion::{moved_mesh, patch_points};
use rcfd_mesh::Mesh;
use std::f64::consts::{PI, TAU};
use std::sync::{Arc, OnceLock};

const RHO: f64 = 1.0;
const WALL: &str = "xmin";
/// The registered column filter (ADR 0261 D5).
const FILTER: f64 = 1e-2;

fn tight(max_iter: usize) -> SolveControl {
    SolveControl {
        tol_abs: 1e-16,
        tol_rel: 1e-13,
        max_iter,
    }
}

// ---- the channel of ADR 0256 (copied) ------------------------------------------------------------------------------

struct Channel {
    mesh0: Mesh,
    g0: FvGeometry,
    classes: Vec<PatchClass>,
    wall: Vec<usize>,
    nu: f64,
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

// ---- set-up CWs (ADR 0256 P3, G1, amendment 1; copied) ------------------------------------------------------------

const CW_L: f64 = 1.0;
const CW_W: f64 = 0.4;
const CW_LZ: f64 = 0.1;
const CW_MU: f64 = 4.0;
const CW_X: f64 = 0.002;
const CW_NU: f64 = 1e-4;
const CW_NODES: usize = 6;
const CW_DT: f64 = 0.01;
const CW_STEPS: usize = 650;

fn cw_m() -> f64 {
    RHO * CW_L / CW_MU
}

fn cw_k() -> f64 {
    TAU * TAU * (cw_m() + RHO * CW_L)
}

fn cw_channel() -> Arc<Channel> {
    Channel::new([20, 8, 2], [CW_L, CW_W, CW_LZ], CW_NU, true)
}

fn cw_node_y() -> Vec<f64> {
    (0..CW_NODES).map(|i| CW_W * i as f64 / (CW_NODES - 1) as f64).collect()
}

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

fn along_x(q: &[f64]) -> Vec<Vec3> {
    q.iter().map(|x| [*x, 0.0, 0.0]).collect()
}

fn mapped(rows: &[Vec<(usize, f64)>], q: &[Vec3]) -> Vec<Vec3> {
    rows.iter()
        .map(|row| {
            row.iter()
                .fold(vec3::ZERO, |a, &(i, w)| vec3::add(a, vec3::scale(q[i], w)))
        })
        .collect()
}

fn cw_parts(ch: &Arc<Channel>) -> (FlowWall, SprungWall, InterfaceMap) {
    let structure = cw_structure();
    let rows = piecewise_linear_rows(&cw_node_y(), &ch.wall_y()).unwrap();
    let flow = ch.flow(&mapped(&rows, &along_x(&cw_q0())));
    let fluid = FlowWall::new(flow, WALL, &ch.mesh0, RHO, CW_NU, motion(ch)).unwrap();
    let map = InterfaceMap::new(rows, CW_NODES, fluid.face_points()).unwrap();
    (fluid, structure, map)
}

/// ADR 0256 C3 (the Aitken reference).
fn cw_aitken() -> CouplingSettings {
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

/// ADR 0261 S3: C3 with the least-squares update.
fn cw_least_squares() -> CouplingSettings {
    CouplingSettings {
        relaxation: Relaxation::LeastSquares {
            initial: relaxation_from_added_mass(cw_m(), RHO * CW_L),
            filter: FILTER,
        },
        ..cw_aitken()
    }
}

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

#[derive(Default)]
struct CwRun {
    t: Vec<f64>,
    q0: Vec<f64>,
    q1: Vec<f64>,
    iterations: Vec<usize>,
    last_residual: f64,
    all_converged: bool,
    columns: Vec<Vec<usize>>,
    factors: Vec<Vec<f64>>,
    first_residuals: Vec<Vec<f64>>,
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
    /// Columns dropped by the filter over the run: an update after iteration `k` could use `k - 1` columns at most, and
    /// a column once dropped stays out, so the shortfall of every step's last update counts the dropped ones.
    fn dropped(&self) -> usize {
        self.columns
            .iter()
            .map(|c| match c.last() {
                Some(&last) => c.len() - 1 - last,
                None => 0,
            })
            .sum()
    }
}

fn cw_run(settings: &CouplingSettings) -> CwRun {
    let ch = cw_channel();
    let (mut fluid, mut structure, map) = cw_parts(&ch);
    let mut run = CwRun {
        all_converged: true,
        ended: "completed".into(),
        ..Default::default()
    };
    let (a, b) = modal(&structure.q);
    run.t.push(0.0);
    run.q0.push(a);
    run.q1.push(b);
    for n in 1..=CW_STEPS {
        let step: CoupledStep = match coupled_step(&mut fluid, &mut structure, &map, CW_DT, settings) {
            Ok(s) => s,
            Err(e) => {
                run.ended = format!("step {n}: {e}");
                break;
            }
        };
        run.iterations.push(step.iterations);
        run.all_converged &= step.converged;
        run.last_residual = run.last_residual.max(*step.residuals.last().unwrap());
        run.columns.push(step.columns.clone());
        run.factors.push(step.factors.clone());
        if n <= 3 {
            let r0 = step.residual_norms[0];
            run.first_residuals
                .push(step.residual_norms.iter().map(|r| r / r0).collect());
        }
        let (a, b) = modal(&structure.q);
        run.t.push(structure.t);
        run.q0.push(a);
        run.q1.push(b);
    }
    run
}

fn sci(v: &[f64]) -> String {
    v.iter().map(|x| format!("{x:.1e}")).collect::<Vec<_>>().join(" ")
}

fn report(label: &str, run: &CwRun) {
    println!(
        "{label}: ended '{}', {} steps, converged {}, iterations max {} mean {:.4}, histogram {:?}, largest last test quantity {:.3e}, columns dropped {}",
        run.ended,
        run.iterations.len(),
        run.all_converged,
        run.max_iterations(),
        run.mean_iterations(),
        run.histogram(),
        run.last_residual,
        run.dropped()
    );
    for (n, r) in run.first_residuals.iter().enumerate() {
        println!(
            "{label}: step {}: |r_k|/|r_1| {}; columns {:?}; factors {:.4?}",
            n + 1,
            sci(r),
            run.columns[n],
            run.factors[n]
        );
    }
}

fn cw_least_squares_run() -> &'static CwRun {
    static RUN: OnceLock<CwRun> = OnceLock::new();
    RUN.get_or_init(|| {
        let run = cw_run(&cw_least_squares());
        report("CW least squares", &run);
        run
    })
}

fn cw_aitken_run() -> &'static CwRun {
    static RUN: OnceLock<CwRun> = OnceLock::new();
    RUN.get_or_init(|| {
        let run = cw_run(&cw_aitken());
        report("CW Aitken", &run);
        run
    })
}

// ---- acceptance ----------------------------------------------------------------------------------------------------

#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn qa1_the_least_squares_update_converges_the_model_problem_in_at_most_seven() {
    let run = cw_least_squares_run();
    println!(
        "QA1: converged {}, iterations max {} mean {:.4}, largest last test quantity {:.3e}",
        run.all_converged,
        run.max_iterations(),
        run.mean_iterations(),
        run.last_residual
    );
    assert_eq!(run.ended, "completed");
    assert_eq!(run.iterations.len(), CW_STEPS);
    assert!(run.all_converged);
    assert!(run.last_residual <= 1e-8);
    assert!(run.max_iterations() <= 7);
    assert!(run.mean_iterations() <= 6.0);
}

#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn qa2_the_frequencies_are_those_of_the_aitken_run() {
    let (ls, ai) = (cw_least_squares_run(), cw_aitken_run());
    assert_eq!(ls.ended, "completed");
    assert_eq!(ai.ended, "completed");
    let (l0, ln0) = measured_omega(&ls.t, &ls.q0);
    let (l1, ln1) = measured_omega(&ls.t, &ls.q1);
    let (a0, an0) = measured_omega(&ai.t, &ai.q0);
    let (a1, an1) = measured_omega(&ai.t, &ai.q1);
    let (d0, d1) = ((l0 / a0 - 1.0).abs(), (l1 / a1 - 1.0).abs());
    let dq = ls.q0.iter().zip(&ai.q0).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max);
    println!(
        "QA2: omega Q0 {l0:.9} against {a0:.9} ({ln0}, {an0} crossings), relative {d0:.2e}; Q1 {l1:.9} against {a1:.9} ({ln1}, {an1}), relative {d1:.2e}; largest |dQ0|/X {:.2e}",
        dq / CW_X
    );
    assert!(d0 <= 1e-6);
    assert!(d1 <= 1e-6);
    assert_eq!(ln0, an0);
    assert_eq!(ln1, an1);
}

#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn qa4_the_plain_fixed_point_diverges() {
    let ch = cw_channel();
    let (mut fluid, mut structure, map) = cw_parts(&ch);
    let mut s = cw_aitken();
    s.relaxation = Relaxation::Fixed(1.0);
    s.max_iterations = 10;
    s.at_cap = AtCap::Keep;
    let step = coupled_step(&mut fluid, &mut structure, &map, CW_DT, &s).unwrap();
    let r = &step.residual_norms;
    println!("QA4: converged {}, |r_10|/|r_1| {:.2}", step.converged, r[9] / r[0]);
    assert!(!step.converged);
    assert_eq!(r.len(), 10);
    assert!(r[9] / r[0] >= 200.0);
}

// ---- set-up checks -------------------------------------------------------------------------------------------------

#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn s1_the_aitken_reference_is_the_run_of_adr_0256() {
    let run = cw_aitken_run();
    println!(
        "S1: Aitken histogram {:?}, mean {:.4}",
        run.histogram(),
        run.mean_iterations()
    );
    assert!(run.all_converged);
    assert_eq!(run.iterations.len(), CW_STEPS);
    assert_eq!(run.histogram(), vec![0, 0, 0, 0, 0, 0, 0, 13, 210, 343, 84]);
}

#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn s2_every_step_starts_relaxed_and_uses_no_more_columns_than_differences() {
    let run = cw_least_squares_run();
    let initial = relaxation_from_added_mass(cw_m(), RHO * CW_L);
    let mut bad = 0;
    for (c, w) in run.columns.iter().zip(&run.factors) {
        let first_relaxed = c.first().is_none_or(|&c0| c0 == 0) && w.first().is_none_or(|&w0| w0 == initial);
        let bounded = c.iter().enumerate().all(|(i, &n)| n <= i);
        bad += usize::from(!(first_relaxed && bounded));
    }
    println!("S2: steps violating the first relaxed update or the column bound: {bad}");
    assert_eq!(run.columns.len(), CW_STEPS);
    assert_eq!(bad, 0);
}

// ---- one rigid node (SPm1 of ADR 0256; copied) ---------------------------------------------------------------------

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
fn qa3_one_rigid_node_needs_no_more_iterations_than_aitken() {
    let run = |settings: &CouplingSettings| -> (Vec<f64>, Vec<usize>) {
        let ch = Channel::new([20, 2, 2], [SP_L0, 0.2, 0.2], SP_NU, false);
        let (mut fluid, mut structure, map) = sp_parts(&ch);
        let (mut xs, mut its) = (Vec::new(), Vec::new());
        for _ in 0..100 {
            let step = coupled_step(&mut fluid, &mut structure, &map, 0.01, settings).unwrap();
            assert!(step.converged);
            xs.push(structure.q[0]);
            its.push(step.iterations);
        }
        (xs, its)
    };
    let aitken = sp_settings();
    let least = CouplingSettings {
        relaxation: Relaxation::LeastSquares {
            initial: relaxation_from_added_mass(sp_mass(), RHO * SP_A * SP_L0),
            filter: FILTER,
        },
        ..aitken
    };
    let (xa, ia) = run(&aitken);
    let (xl, il) = run(&least);
    let more = ia.iter().zip(&il).filter(|(a, l)| l > a).count();
    let other = ia.iter().zip(&il).filter(|(a, l)| l != a).count();
    let (ta, tl) = (ia.iter().sum::<usize>(), il.iter().sum::<usize>());
    let dx = xa.iter().zip(&xl).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max);
    println!(
        "QA3: steps with more least-squares iterations {more}, with another count {other}; totals {tl} (least squares) and {ta} (Aitken); largest |dx| {dx:e} ({:.2e} X); Aitken max {}",
        dx / SP_X,
        ia.iter().max().unwrap()
    );
    assert_eq!(more, 0);
    assert!(tl <= ta);
    assert!(dx <= 1e-8 * SP_X);
}

// ---- mock affine fluids (fast) -------------------------------------------------------------------------------------

type Law = Box<dyn Fn(&[f64]) -> Vec<Vec3>>;

/// A fluid with one wall point and one single-point face per node, whose face forces are a given function of the x
/// displacements of its points.
struct MockFluid {
    d: Vec<f64>,
    t: f64,
    law: Law,
    saved: Option<(Vec<f64>, f64)>,
}

impl Participant for MockFluid {
    fn name(&self) -> &str {
        "fluid"
    }
    fn time(&self) -> f64 {
        self.t
    }
    fn checkpoint(&mut self) -> Result<(), String> {
        self.saved = Some((self.d.clone(), self.t));
        Ok(())
    }
    fn restore(&mut self) -> Result<(), String> {
        let (d, t) = self.saved.clone().ok_or("no checkpoint")?;
        self.d = d;
        self.t = t;
        Ok(())
    }
}

impl Fluid for MockFluid {
    fn point_coordinates(&self) -> Vec<Vec3> {
        (0..self.d.len()).map(|i| [0.0, i as f64, 0.0]).collect()
    }
    fn face_points(&self) -> Vec<Vec<usize>> {
        (0..self.d.len()).map(|i| vec![i]).collect()
    }
    fn forces(&self) -> Result<Vec<Vec3>, String> {
        Ok((self.law)(&self.d))
    }
    fn advance(&mut self, dt: f64, displacements: &[Vec3]) -> Result<Vec<Vec3>, String> {
        self.d = displacements.iter().map(|v| v[0]).collect();
        self.t += dt;
        Ok((self.law)(&self.d))
    }
}

const MOCK_DT: f64 = 0.01;
const MOCK_M: f64 = 1.0;

fn mock_parts(d0: Vec<f64>, law: Law) -> (MockFluid, SprungWall, InterfaceMap) {
    let n = d0.len();
    let structure = SprungWall::new(
        (0..n).map(|i| [0.0, i as f64, 0.0]).collect(),
        [1.0, 0.0, 0.0],
        vec![MOCK_M; n],
        vec![10.0; n],
        d0.clone(),
    )
    .unwrap();
    let fluid = MockFluid {
        d: d0,
        t: 0.0,
        law,
        saved: None,
    };
    let map = InterfaceMap::new((0..n).map(|i| vec![(i, 1.0)]).collect(), n, fluid.face_points()).unwrap();
    (fluid, structure, map)
}

/// Three nodes, `F_x = c - K d` with `K = (2 M/dt^2) Q diag(4, 1, 0.25) Q`, `Q = I - (2/3) 1 1^T` (slopes of the map
/// about `-4, -1, -0.25`), and `F_y` of a face one tenth of `F_x` of the next (the transverse block of ADR 0261 D1).
fn u1_law() -> Law {
    let q = |i: usize, j: usize| f64::from(u8::from(i == j)) - 2.0 / 3.0;
    let lam = [4.0, 1.0, 0.25];
    let scale = 2.0 * MOCK_M / (MOCK_DT * MOCK_DT);
    let mut k = [[0.0; 3]; 3];
    for (i, row) in k.iter_mut().enumerate() {
        for (j, kij) in row.iter_mut().enumerate() {
            *kij = scale * (0..3).map(|l| q(i, l) * lam[l] * q(l, j)).sum::<f64>();
        }
    }
    let c = [0.3, -0.1, 0.2];
    Box::new(move |d: &[f64]| {
        let fx: Vec<f64> = (0..3)
            .map(|i| c[i] - (0..3).map(|j| k[i][j] * d[j]).sum::<f64>())
            .collect();
        (0..3).map(|i| [fx[i], 0.1 * fx[(i + 1) % 3], 0.0]).collect()
    })
}

#[test]
fn u1_a_linear_map_of_four_eigenvalues_terminates_by_iteration_six() {
    let settings = |relaxation| CouplingSettings {
        relaxation,
        tolerance: 1e-12,
        force_scale: 0.0,
        max_iterations: 40,
        at_cap: AtCap::Keep,
    };
    let run = |relaxation| {
        let (mut fluid, mut structure, map) = mock_parts(vec![0.001, 0.002, -0.001], u1_law());
        coupled_step(&mut fluid, &mut structure, &map, MOCK_DT, &settings(relaxation)).unwrap()
    };
    let ls = run(Relaxation::LeastSquares {
        initial: 0.3,
        filter: FILTER,
    });
    let ai = run(Relaxation::Aitken { initial: 0.3, max: 2.0 });
    let r0 = ls.residual_norms[0];
    println!(
        "U1: least squares converged {} in {} (|r_k|/|r_1| {}, columns {:?}); Aitken converged {} in {}",
        ls.converged,
        ls.iterations,
        sci(&ls.residual_norms.iter().map(|r| r / r0).collect::<Vec<_>>()),
        ls.columns,
        ai.converged,
        ai.iterations
    );
    assert!(ls.converged);
    assert!(ls.iterations <= 6);
}

/// One node, `F_x = -K d - beta d^3` with the slope `-2` of the map at `d = 0` and a cubic of a tenth of the linear part at
/// `d = 0.01`; `F_y = F_z = 0`.
fn u2_law() -> Law {
    let k = 4.0 * MOCK_M / (MOCK_DT * MOCK_DT);
    let beta = 0.1 * k / 1e-4;
    Box::new(move |d: &[f64]| vec![[-k * d[0] - beta * d[0].powi(3), 0.0, 0.0]])
}

#[test]
fn u2_with_one_node_the_least_squares_update_is_aitkens_secant() {
    let settings = |relaxation| CouplingSettings {
        relaxation,
        tolerance: 1e-10,
        force_scale: 0.0,
        max_iterations: 40,
        at_cap: AtCap::Stop,
    };
    let steps = |relaxation| -> Vec<CoupledStep> {
        let (mut fluid, mut structure, map) = mock_parts(vec![0.01], u2_law());
        (0..5)
            .map(|_| coupled_step(&mut fluid, &mut structure, &map, MOCK_DT, &settings(relaxation)).unwrap())
            .collect()
    };
    let ls = steps(Relaxation::LeastSquares {
        initial: 0.25,
        filter: FILTER,
    });
    let ai = steps(Relaxation::Aitken {
        initial: 0.25,
        max: 2.0,
    });
    let mut worst: f64 = 0.0;
    for (n, (l, a)) in ls.iter().zip(&ai).enumerate() {
        println!(
            "U2: step {}: iterations {} and {}; columns {:?}; Aitken factors {:.4?}",
            n + 1,
            l.iterations,
            a.iterations,
            l.columns,
            a.factors
        );
        assert!(l.converged && a.converged);
        assert_eq!(l.iterations, a.iterations);
        let expected: Vec<usize> = (0..l.columns.len()).map(|i| usize::from(i > 0)).collect();
        assert_eq!(l.columns, expected);
        assert!(a.factors.iter().all(|w| w.abs() < 2.0));
        let rel = |x: f64, y: f64| (x - y).abs() / y.abs().max(1e-300);
        worst = worst
            .max(rel(l.node_forces[0][0], a.node_forces[0][0]))
            .max(rel(l.node_displacements[0][0], a.node_displacements[0][0]));
    }
    println!("U2: largest relative difference of the final force and displacement {worst:e}");
    assert!(worst <= 1e-12);
}

#[test]
fn u3_the_least_squares_settings_are_validated() {
    let bad = [
        (0.0, FILTER),
        (2.0, FILTER),
        (f64::NAN, FILTER),
        (1.0 / 3.0, 0.0),
        (1.0 / 3.0, 1.0),
        (1.0 / 3.0, f64::NAN),
    ];
    for (initial, filter) in bad {
        let mut s = CouplingSettings::least_squares(1.0 / 3.0);
        s.relaxation = Relaxation::LeastSquares { initial, filter };
        let e = s.validate();
        println!("U3: ({initial}, {filter}): {e:?}");
        assert!(e.is_err());
    }
    let good = CouplingSettings::least_squares(1.0 / 3.0);
    assert_eq!(
        good.relaxation,
        Relaxation::LeastSquares {
            initial: 1.0 / 3.0,
            filter: 1e-2
        }
    );
    assert!(good.validate().is_ok());
    let mut strong = StrongCoupling::aitken(1.0 / 3.0);
    strong.relaxation = good.relaxation;
    let e = strong.validate();
    println!("U3: strong coupling: {e:?}");
    assert!(e.is_err());
}
