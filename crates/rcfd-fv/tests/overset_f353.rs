//! F-353 (ADR 0259): the moving cylinder TR of ADR 0219 with the quadratic donor interpolation of F-336
//! (`DonorInterpolation::Quadratic`, ADR 0251), against the body-fitted run BF and the linear-stencil run of ADR 0219.
//!
//! The set-up helpers are copied unchanged from `overset_f134.rs`. `tr_quadratic_against_linear_and_body_fitted` is the
//! heavy record (A1a, A1b, A2, the fallbacks, the difference to the linear run, L0, X1 to X5, NQ and the numbers of the
//! rule of the recommendation); `tr_quadratic_first_ten_steps` is the fast test of the gate (G1 to G5).
//!
//! Release only: `cargo test --release -p rcfd-fv --test overset_f353`.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::ale::{move_mesh, set_follow_conditions};
use rcfd_fv::bc::FixedValue;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::overset::{MassMode, OversetCoupling, OversetInfo, OversetSolver};
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::sixdof::patch_force_moment;
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{axis_coords, cylinder_ogrid, expansion_for_end_cell, rectilinear_2d, Grading};
use rcfd_mesh::motion::{moved_mesh, Pose};
use rcfd_mesh::overset::{compose, CellKind, DonorInterpolation, Overset, OversetSpec};
use rcfd_mesh::Mesh;
use std::f64::consts::PI;
use std::sync::Arc;
use std::time::Instant;

/// Kinematic viscosity of TR and BF: Re = U D / nu = 40 with U = 1, D = 1.
const NU: f64 = 0.025;
/// Time step of TR and BF.
const DT: f64 = 0.02;
/// Depth of every 2-D mesh.
const DEPTH: f64 = 0.1;
/// `U D L` of TR.
const UDL: f64 = 0.1;
/// Solver controls of every set-up (ADR 0219 PS).
const SOLVE: SolveControl = SolveControl {
    tol_abs: 1e-15,
    tol_rel: 1e-12,
    max_iter: 3000,
};

/// ADR 0219 PS.
fn settings(nu: f64) -> PimpleSettings {
    let mut s = PimpleSettings::piso(nu);
    s.convection = ConvScheme::Central;
    s.momentum_solver = SOLVE;
    s.pressure_solver = SOLVE;
    s
}

/// Records the failed checks and prints every check.
struct Checks(Vec<String>);

impl Checks {
    fn new() -> Self {
        Checks(Vec::new())
    }
    /// `value <= limit`.
    fn at_most(&mut self, id: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{id}: {value:.4e} (limit <= {limit:.3e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{id}: {value:e} > {limit:e}"));
        }
    }
    /// `value >= limit`.
    fn at_least(&mut self, id: &str, value: f64, limit: f64) {
        let ok = value >= limit;
        println!(
            "{id}: {value:.4e} (limit >= {limit:.3e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{id}: {value:e} < {limit:e}"));
        }
    }
    fn holds(&mut self, id: &str, ok: bool, what: &str) {
        println!("{id}: {what} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{id}: {what}"));
        }
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:?}", self.0);
    }
}

// ---- set-ups (copied from overset_f134.rs) ---------------------------------------------------------------------------

/// Displacement of the cylinder along x (ADR 0219 M10).
fn displacement(t: f64) -> f64 {
    if t <= 1.0 {
        -(0.5 * t - (PI * t).sin() / (2.0 * PI))
    } else {
        -(0.5 + (t - 1.0))
    }
}

/// Outer radius and grading of an O-grid of `n` radial cells from `r = 0.5` whose cells grow by 1.08 from 0.01 at the wall.
fn radial(n: usize) -> (f64, Grading) {
    let q: f64 = 1.08;
    let len = 0.01 * (q.powi(n as i32) - 1.0) / (q - 1.0);
    (0.5 + len, Grading::Expansion(q.powi(n as i32 - 1)))
}

fn bf_mesh() -> Mesh {
    let (r_out, g) = radial(66);
    cylinder_ogrid(96, 66, 0.5, r_out, g, DEPTH).unwrap()
}

/// Nodes from `x0` over `len` in `n` cells, the cell at the far end (`toward_end`) or at `x0` of size `h`.
fn graded(x0: f64, len: f64, n: usize, h: f64, toward_end: bool) -> Vec<f64> {
    let r = expansion_for_end_cell(n, len, h).unwrap();
    let r = if toward_end { r } else { 1.0 / r };
    axis_coords(n, len, Grading::Expansion(r))
        .unwrap()
        .into_iter()
        .map(|x| x0 + x)
        .collect()
}

fn join(parts: &[Vec<f64>]) -> Vec<f64> {
    let mut out: Vec<f64> = Vec::new();
    for p in parts {
        let skip = usize::from(!out.is_empty());
        out.extend_from_slice(&p[skip..]);
    }
    out
}

fn uniform(x0: f64, len: f64, n: usize) -> Vec<f64> {
    (0..=n).map(|i| x0 + len * i as f64 / n as f64).collect()
}

/// TR's background (20,350 cells) and component (the inner 30 layers of BF, 2880 cells) as an overset.
fn tr_overset() -> Overset {
    let xs = join(&[
        graded(-25.0, 17.0, 30, 0.1, true),
        uniform(-8.0, 12.5, 125),
        graded(4.5, 17.5, 30, 0.1, false),
    ]);
    let ys = join(&[
        graded(-20.0, 17.5, 30, 0.1, true),
        uniform(-2.5, 5.0, 50),
        graded(2.5, 17.5, 30, 0.1, false),
    ]);
    let bg = rectilinear_2d(&xs, &ys, DEPTH).unwrap();
    let (r_out, g) = radial(30);
    let comp = cylinder_ogrid(96, 30, 0.5, r_out, g, DEPTH).unwrap();
    let composite = compose(&bg, &comp, "c_").unwrap();
    let spec = OversetSpec {
        outer_patches: vec!["c_farfield".into()],
        hole: Arc::new(|x: Vec3| x[0] * x[0] + x[1] * x[1] < 1.0),
    };
    Overset::new(composite, spec).unwrap()
}

fn tr_pose(t: f64) -> Pose {
    Pose::translation([2.0 + displacement(t), 0.0, 0.0])
}

/// The composite at `pose` as a geometry.
fn geometry_at(ov: &Overset, pose: &Pose) -> Arc<FvGeometry> {
    let mesh = moved_mesh(&ov.composite.mesh, ov.composite.points_at(pose)).unwrap();
    Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap())
}

/// TR at `t = 0` (conservative mass mode) on the overset `ov`.
fn tr_new_on(ov: Overset) -> OversetSolver {
    let pose = tr_pose(0.0);
    let geom = geometry_at(&ov, &pose);
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    for name in ["xmin", "ymin", "ymax", "c_cylinder"] {
        u.set_bc(&geom, name, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    p.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
    OversetSolver::new(
        geom,
        u,
        p,
        settings(NU),
        MassMode::Conservative,
        Some(OversetCoupling { overset: ov, pose }),
    )
    .unwrap()
}

/// TR-L: TR of ADR 0219 with the default (linear) stencils.
fn tr_linear() -> OversetSolver {
    tr_new_on(tr_overset())
}

/// TR-Q: TR of ADR 0219 with the quadratic donor interpolation of ADR 0251.
fn tr_quadratic() -> OversetSolver {
    let mut ov = tr_overset();
    ov.set_donor_interpolation(DonorInterpolation::Quadratic);
    tr_new_on(ov)
}

fn walls_tr() -> Vec<String> {
    vec!["c_cylinder".to_string()]
}

/// Net flux through a patch.
fn patch_flux(geom: &FvGeometry, phi: &[f64], patch: &str) -> f64 {
    geom.mesh.patch(patch).unwrap().faces().map(|f| phi[f]).sum()
}

fn force_x(geom: &FvGeometry, u: &VectorField, p: &ScalarField, patch: &str, centre: Vec3) -> f64 {
    patch_force_moment(geom, u, p, 1.0, NU, &[patch.to_string()], centre)
        .unwrap()
        .0[0]
}

/// BF at `t = 0`.
fn bf_new() -> (PimpleSolver, Mesh) {
    let mesh0 = bf_mesh();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh0.clone())).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    for name in ["cylinder", "farfield"] {
        u.set_bc(&geom, name, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    let p = ScalarField::uniform("p", &geom, 0.0);
    (PimpleSolver::new(geom, u, p, settings(NU)), mesh0)
}

/// The points of BF at step `n`.
fn bf_points(mesh0: &Mesh, n: usize) -> Vec<Vec3> {
    let d = displacement(n as f64 * DT);
    mesh0.points().iter().map(|q| [q[0] + d, q[1], q[2]]).collect()
}

fn cd(f: f64) -> f64 {
    f / (0.5 * DEPTH)
}

// ---- the TR runs, stepped alternately --------------------------------------------------------------------------------

/// What one TR run records per step.
struct Track {
    name: &'static str,
    force: Vec<f64>,
    continuity: f64,
    outflow: f64,
    unconverged: usize,
    p_iters: usize,
    changes: usize,
    /// Fallbacks: at the construction, the largest of a step, the sum over the steps, the first step with one.
    fallback_start: usize,
    fallback_max: usize,
    fallback_sum: usize,
    fallback_first: Option<usize>,
    delta: (f64, f64),
    secs: f64,
    /// X1, X2 (TR-Q only are asserted).
    x1: bool,
    x2: bool,
}

impl Track {
    fn new(name: &'static str, s: &OversetSolver, steps: usize) -> Self {
        Track {
            name,
            force: vec![0.0; steps + 1],
            continuity: 0.0,
            outflow: 0.0,
            unconverged: 0,
            p_iters: 0,
            changes: 0,
            fallback_start: s.conn.n_fallback(),
            fallback_max: 0,
            fallback_sum: 0,
            fallback_first: None,
            delta: (f64::INFINITY, f64::NEG_INFINITY),
            secs: 0.0,
            x1: true,
            x2: true,
        }
    }

    /// One step of `s` to step `n`, timed, and its records.
    fn step(&mut self, s: &mut OversetSolver, n: usize) -> OversetInfo {
        let pose = tr_pose(n as f64 * DT);
        let t0 = Instant::now();
        let info = s.step(DT, pose, &walls_tr()).unwrap();
        self.secs += t0.elapsed().as_secs_f64();
        self.continuity = self.continuity.max(info.continuity);
        self.outflow = self.outflow.max(patch_flux(&s.geom, &s.phi, "xmax").abs() / UDL);
        self.unconverged += info.unconverged;
        self.p_iters += info.p_iters;
        self.changes += info.n_changed;
        self.fallback_max = self.fallback_max.max(info.n_fallback);
        self.fallback_sum += info.n_fallback;
        if info.n_fallback > 0 && self.fallback_first.is_none() {
            self.fallback_first = Some(n);
        }
        self.delta = (self.delta.0.min(info.delta), self.delta.1.max(info.delta));
        let centre = pose.translation;
        let nb = s.conn.n_background_cells;
        let cc = s.geom.mesh.cell_centres();
        for c in 0..nb {
            if ((cc[c][0] - centre[0]).powi(2) + (cc[c][1] - centre[1]).powi(2)).sqrt() < 0.5 {
                self.x1 &= s.conn.kind[c] == CellKind::Hole;
            }
        }
        self.x2 &= info.n_fringe_component == 96 && info.n_hole > 0 && info.n_fringe_background > 0;
        self.force[n] = force_x(&s.geom, &s.u, &s.p, "c_cylinder", centre);
        info
    }

    fn print(&self, steps: usize) {
        println!(
            "{}: {:.1} s in its steps; continuity {:.3e}; outflow / (U D L) {:.3e}; unconverged {}; pressure iterations per step {:.1}; kind changes {}; delta / (U^2/2) in [{:.3e}, {:.3e}]; fallbacks: at the start {}, largest of a step {}, sum {}, first step {:?}",
            self.name,
            self.secs,
            self.continuity,
            self.outflow,
            self.unconverged,
            self.p_iters as f64 / steps as f64,
            self.changes,
            self.delta.0 / 0.5,
            self.delta.1 / 0.5,
            self.fallback_start,
            self.fallback_max,
            self.fallback_sum,
            self.fallback_first
        );
    }
}

/// The quadratic stencils of `s`: their number, the smallest size and the number with fewer than nine cells.
fn stencil_sizes(s: &OversetSolver) -> (usize, usize, usize) {
    let st = &s.conn.stencils;
    let smallest = st.iter().map(|(_, x)| x.cells.len()).min().unwrap_or(0);
    let reduced = st.iter().filter(|(_, x)| x.cells.len() < 9).count();
    (st.len(), smallest, reduced)
}

/// TR-L and TR-Q for `steps` steps, stepped alternately (TR-L first on odd steps). Also checks X3 (equal kinds) and X5
/// (the mode) and whether every quadratic stencil has nine cells after every step. Returns the two tracks, the two
/// solvers and whether X3, X5 and the nine cells held.
fn tr_pair(steps: usize, print_every: usize) -> (Track, Track, OversetSolver, OversetSolver, [bool; 3]) {
    let mut sl = tr_linear();
    let mut sq = tr_quadratic();
    let mut tl = Track::new("TR-L", &sl, steps);
    let mut tq = Track::new("TR-Q", &sq, steps);
    let (mut kinds_equal, mut mode_q, mut nine) = (true, true, true);
    for n in 1..=steps {
        if n % 2 == 1 {
            tl.step(&mut sl, n);
            tq.step(&mut sq, n);
        } else {
            tq.step(&mut sq, n);
            tl.step(&mut sl, n);
        }
        kinds_equal &= sl.conn.kind == sq.conn.kind;
        mode_q &= sq.conn.interpolation == DonorInterpolation::Quadratic;
        let (k, smallest, reduced) = stencil_sizes(&sq);
        nine &= smallest == 9 && reduced == 0;
        if n % print_every == 0 {
            println!(
                "step {n}: TR-Q {k} stencils, smallest {smallest}, {reduced} with fewer than nine cells; fallbacks {}",
                sq.conn.n_fallback()
            );
        }
    }
    (tl, tq, sl, sq, [kinds_equal, mode_q, nine])
}

// ---- the heavy record ------------------------------------------------------------------------------------------------

/// The values of the run of ADR 0219's A test (`w49i-work/f134-a.log`): `C_D` of TR-L and BF at `t = 2, 4, 6, 8`, A2.
const L0_CD_TR: [f64; 4] = [2.00050, 1.82257, 1.72278, 1.66752];
const L0_CD_BF: [f64; 4] = [2.00311, 1.82666, 1.72919, 1.67390];
const L0_A2: f64 = 6.2485e-3;

/// ADR 0259: TR with the quadratic donor interpolation against BF and against TR with the linear stencils.
#[test]
#[ignore = "heavy record, ADR 0259: TR-L and TR-Q 400 steps each and BF 400 steps, about 6 minutes; run once"]
fn tr_quadratic_against_linear_and_body_fitted() {
    const STEPS: usize = 400;
    let t0 = Instant::now();
    let (tl, tq, sl, sq, [kinds_equal, mode_q, nine]) = tr_pair(STEPS, 50);
    drop((sl, sq));
    println!("TR-Q: every stencil of nine cells after every step: {nine}");
    let t1 = Instant::now();
    let (mut b, mesh0) = bf_new();
    let walls = vec!["cylinder".to_string()];
    let mut f_bf = vec![0.0; STEPS + 1];
    let (mut cont_bf, mut bf_cap) = (0.0f64, 0usize);
    for n in 1..=STEPS {
        let st = move_mesh(&b.geom, bf_points(&mesh0, n), DT).unwrap();
        let old = b.geom.mesh.clone();
        let dp = set_follow_conditions(&mut b.u, &st, &old, &walls, DT).unwrap();
        let info = b.step_ale_with_walls(DT, st.geom, st.mesh_flux, dp).unwrap();
        cont_bf = cont_bf.max(info.continuity);
        bf_cap = bf_cap.max(info.p_iters);
        f_bf[n] = force_x(&b.geom, &b.u, &b.p, "cylinder", [displacement(n as f64 * DT), 0.0, 0.0]);
    }
    let t_bf = t1.elapsed().as_secs_f64();
    tl.print(STEPS);
    tq.print(STEPS);
    println!(
        "BF: {t_bf:.1} s; continuity {cont_bf:.2e}; largest pressure iterations of a step {bf_cap}; total {:.1} s",
        t0.elapsed().as_secs_f64()
    );

    // the window t = 2 .. 8
    let (fl, fq) = (&tl.force, &tq.force);
    let mut d_all = 0.0f64;
    for n in 1..=STEPS {
        d_all = d_all.max(((fq[n] - fl[n]) / fl[n]).abs());
    }
    let (mut a2q, mut a2q_at, mut a2l, mut a2l_at) = (0.0f64, 0usize, 0.0f64, 0usize);
    let (mut mean_eq, mut mean_el, mut mean_d, mut d_win, mut d_win_at) = (0.0, 0.0, 0.0, 0.0f64, 0usize);
    let (mut mcd_bf, mut mcd_l, mut mcd_q) = (0.0, 0.0, 0.0);
    let (mut worse, mut worse_margin, mut r3_worst, mut r3_worst_at) = (0usize, 0usize, f64::NEG_INFINITY, 0usize);
    let nw = (STEPS - 100 + 1) as f64;
    println!("series: n, t, C_D BF, C_D L, C_D Q, e_L, e_Q, d");
    for n in 100..=STEPS {
        let el = (fl[n] - f_bf[n]).abs() / f_bf[n].abs();
        let eq = (fq[n] - f_bf[n]).abs() / f_bf[n].abs();
        let d = (fq[n] - fl[n]) / fl[n];
        if eq > a2q {
            a2q = eq;
            a2q_at = n;
        }
        if el > a2l {
            a2l = el;
            a2l_at = n;
        }
        if d.abs() > d_win {
            d_win = d.abs();
            d_win_at = n;
        }
        worse += usize::from(eq > el);
        worse_margin += usize::from(eq > el + 1e-4);
        if eq - el > r3_worst {
            r3_worst = eq - el;
            r3_worst_at = n;
        }
        mean_eq += eq / nw;
        mean_el += el / nw;
        mean_d += d / nw;
        mcd_bf += cd(f_bf[n]) / nw;
        mcd_l += cd(fl[n]) / nw;
        mcd_q += cd(fq[n]) / nw;
        println!(
            "series: {n}, {:.2}, {:.6}, {:.6}, {:.6}, {el:.5e}, {eq:.5e}, {d:+.5e}",
            n as f64 * DT,
            cd(f_bf[n]),
            cd(fl[n]),
            cd(fq[n])
        );
    }
    for n in [100, 200, 300, 400] {
        println!(
            "A: t = {:.0}: C_D body-fitted {:.5}, linear {:.5}, quadratic {:.5}; e_L {:.4e}, e_Q {:.4e}, d {:+.4e}",
            n as f64 * DT,
            cd(f_bf[n]),
            cd(fl[n]),
            cd(fq[n]),
            (fl[n] - f_bf[n]).abs() / f_bf[n].abs(),
            (fq[n] - f_bf[n]).abs() / f_bf[n].abs(),
            (fq[n] - fl[n]) / fl[n]
        );
    }
    println!(
        "A: window [2, 8]: mean C_D body-fitted {mcd_bf:.5}, linear {mcd_l:.5}, quadratic {mcd_q:.5}; largest e_L {a2l:.4e} at step {a2l_at}, largest e_Q {a2q:.4e} at step {a2q_at}; mean e_L {mean_el:.4e}, mean e_Q {mean_eq:.4e}; largest abs(d) {d_win:.4e} at step {d_win_at}, mean d {mean_d:+.4e}; steps with e_Q > e_L: {worse}, with e_Q > e_L + 1e-4: {worse_margin}; largest e_Q - e_L {r3_worst:+.4e} at step {r3_worst_at}"
    );

    // the rule of the recommendation (printed; the decision is written in the ADR)
    let time_ratio = tq.secs / tl.secs;
    let r1 = tq.continuity <= 1e-9 && tq.outflow <= 1e-9 && a2q <= 5e-2;
    let r2 = tq.fallback_start == 0 && tq.fallback_max == 0;
    let r3 = worse_margin == 0;
    let r4 = mean_eq <= mean_el;
    let r5 = time_ratio <= 1.5;
    let r6 = tq.unconverged == 0;
    println!(
        "rule: R1 {r1}, R2 {r2}, R3 {r3}, R4 {r4}, R5 {r5} (time ratio {time_ratio:.3}: TR-Q {:.1} s, TR-L {:.1} s), R6 {r6}: recommendation {}",
        tq.secs,
        tl.secs,
        if r1 && r2 && r3 && r4 && r5 && r6 { "FOR" } else { "AGAINST" }
    );

    let mut c = Checks::new();
    c.at_most("A1a TR-Q continuity", tq.continuity, 1e-9);
    c.at_most("A1b TR-Q outflow / (U D L)", tq.outflow, 1e-9);
    c.at_most("A2 TR-Q drag", a2q, 5e-2);
    for (i, n) in [100usize, 200, 300, 400].into_iter().enumerate() {
        c.at_most(
            &format!("L0 C_D TR-L at step {n}"),
            (cd(fl[n]) - L0_CD_TR[i]).abs(),
            5e-6,
        );
        c.at_most(
            &format!("L0 C_D BF at step {n}"),
            (cd(f_bf[n]) - L0_CD_BF[i]).abs(),
            5e-6,
        );
    }
    c.at_most("L0 A2 TR-L", (a2l - L0_A2).abs(), 5e-8);
    c.holds(
        "X1 TR-Q",
        tq.x1,
        "every background cell within 0.5 of the centre is a hole",
    );
    c.holds(
        "X2 TR-Q",
        tq.x2,
        "96 component fringe cells, holes and background fringe cells in every step",
    );
    c.holds("X3 TR-Q", kinds_equal, "the kinds equal those of TR-L after every step");
    c.at_least("X3 TR-Q kind changes", tq.changes as f64, 500.0);
    c.holds(
        "X4 TR-Q",
        tq.unconverged == 0,
        &format!("{} unconverged linear solves", tq.unconverged),
    );
    c.holds(
        "X4 TR-L",
        tl.unconverged == 0,
        &format!("{} unconverged linear solves", tl.unconverged),
    );
    c.holds(
        "X4 BF",
        bf_cap < 3000,
        &format!("largest pressure iterations of a step {bf_cap}"),
    );
    c.holds("X5 TR-Q", mode_q, "the connectivity is quadratic after every step");
    c.at_least("NQ (negative control) largest abs(d) over the 400 steps", d_all, 1e-9);
    c.finish();
}

// ---- the fast test -----------------------------------------------------------------------------------------------------

/// ADR 0259 G1 to G5: the first ten steps of TR-Q (with TR-L beside it): mass, continuity, the fallback count, the
/// stencils, and that the opt-in reaches the solver.
#[test]
fn tr_quadratic_first_ten_steps() {
    const STEPS: usize = 10;
    let (tl, tq, sl, sq, [kinds_equal, mode_q, nine]) = tr_pair(STEPS, 5);
    tl.print(STEPS);
    tq.print(STEPS);
    let du =
        sl.u.values
            .iter()
            .zip(&sq.u.values)
            .map(|(a, b)| vec3::norm(vec3::sub(*a, *b)))
            .fold(0.0, f64::max);
    let mut c = Checks::new();
    c.at_most("G1 TR-Q continuity", tq.continuity, 1e-9);
    c.at_most("G2 TR-Q outflow / (U D L)", tq.outflow, 1e-9);
    c.at_most(
        "G3 TR-Q fallbacks (start, largest of a step)",
        tq.fallback_start.max(tq.fallback_max) as f64,
        0.0,
    );
    c.holds(
        "G4 TR-Q",
        mode_q && nine,
        "quadratic, every stencil of nine cells, after every step",
    );
    c.holds("X3 (printed with G) TR-Q", kinds_equal, "the kinds equal those of TR-L");
    c.at_least(
        "G5 (negative control) largest difference of u, TR-Q against TR-L",
        du,
        1e-12,
    );
    c.finish();
}
