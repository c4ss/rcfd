//! F-336 (ADR 0251): the opt-in donor interpolation of the overset coupling that reproduces quadratic fields
//! (`rcfd_mesh::overset::DonorInterpolation::Quadratic`), the flow checks: the Taylor-Green order across a moving
//! component (A1, TG_L of ADR 0219 with `dt` proportional to `h^2`), the order of the calculated component cells with the
//! component at rest (A2, RL of ADR 0232), the uniform flow U of ADR 0219 (A3), continuity and the fallback count of
//! every run (C1, C2), the coincident-mesh control CM of ADR 0246, the negative control NL (RL with the default stencils)
//! and the refusal of a non-positive interpolated `rAU` (E1). The kinematic checks are in
//! `crates/rcfd-mesh/tests/overset_f336.rs`.
//!
//! Release only (the flow runs take minutes): `cargo test --release -p rcfd-fv --test overset_f336`.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::bc::FixedValue;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::overset::{MassMode, OversetCoupling, OversetSolver};
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::{FvGeometry, Profile, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::rectilinear_2d;
use rcfd_mesh::motion::{moved_mesh, Pose};
use rcfd_mesh::overset::{compose, CellKind, DonorInterpolation, Overset, OversetSpec, Stencil};
use rcfd_mesh::Mesh;
use std::f64::consts::PI;
use std::sync::Arc;
use std::time::Instant;

const DEPTH: f64 = 0.1;
const NU_TG: f64 = 0.1;
const T_END: f64 = 0.5;
/// Background cell size of level 1.
const H1: f64 = PI / 40.0;
/// ADR 0219 PS.
const SOLVE: SolveControl = SolveControl {
    tol_abs: 1e-15,
    tol_rel: 1e-12,
    max_iter: 3000,
};

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

// ---- set-ups ---------------------------------------------------------------------------------------------------------------

fn uniform(x0: f64, len: f64, n: usize) -> Vec<f64> {
    (0..=n).map(|i| x0 + len * i as f64 / n as f64).collect()
}

/// A square component of half width `half` with `n` cells per side over `bg`, the hole `max(|x_ref|, |y_ref|) < hole`,
/// with the donor interpolation `mode`.
fn box_overset(bg: Mesh, half: f64, n: usize, hole: f64, mode: DonorInterpolation) -> Overset {
    let cs = uniform(-half, 2.0 * half, n);
    let comp = rectilinear_2d(&cs, &cs, DEPTH).unwrap();
    let composite = compose(&bg, &comp, "c_").unwrap();
    let spec = OversetSpec {
        outer_patches: ["c_xmin", "c_xmax", "c_ymin", "c_ymax"].map(String::from).to_vec(),
        hole: Arc::new(move |x: Vec3| x[0].abs().max(x[1].abs()) < hole),
    };
    let mut ov = Overset::new(composite, spec).unwrap();
    if mode != DonorInterpolation::Linear {
        ov.set_donor_interpolation(mode);
    }
    ov
}

fn tg_background(level: usize) -> Mesh {
    let bs = uniform(0.0, PI, 40 * level);
    rectilinear_2d(&bs, &bs, DEPTH).unwrap()
}

/// TG_L of ADR 0219.
fn tg_overset(level: usize, mode: DonorInterpolation) -> Overset {
    box_overset(tg_background(level), 0.6, 20 * level, 0.25, mode)
}

fn tg_pose(t: f64) -> Pose {
    Pose {
        origin: [0.0; 3],
        axis: [0.0, 0.0, 1.0],
        angle: t,
        translation: [PI / 2.0 - 0.3 + 0.5 * t, PI / 2.0 - 0.2 + 0.3 * t, 0.0],
    }
}

/// RL of ADR 0232 amendment 3.
fn rl_overset(level: usize, mode: DonorInterpolation) -> Overset {
    box_overset(tg_background(level), 7.5 * H1, 20 * level, 3.1 * H1, mode)
}

fn diagonal(d: f64) -> Pose {
    Pose::translation([d, d, 0.0])
}

fn rl_pose() -> Pose {
    diagonal(16.5 * H1)
}

/// CM of ADR 0246: a component whose cells coincide with background cells.
fn cm_overset(level: usize, mode: DonorInterpolation) -> Overset {
    box_overset(tg_background(level), 8.0 * H1, 16 * level, 3.1 * H1, mode)
}

fn cm_pose() -> Pose {
    diagonal(16.0 * H1)
}

fn tg_u(x: Vec3, t: f64) -> Vec3 {
    let e = (-2.0 * NU_TG * t).exp();
    [-x[0].cos() * x[1].sin() * e, x[0].sin() * x[1].cos() * e, 0.0]
}

fn tg_p(x: Vec3, t: f64) -> f64 {
    -((2.0 * x[0]).cos() + (2.0 * x[1]).cos()) * (-4.0 * NU_TG * t).exp() / 4.0
}

fn geometry_at(ov: &Overset, pose: &Pose) -> Arc<FvGeometry> {
    let mesh = moved_mesh(&ov.composite.mesh, ov.composite.points_at(pose)).unwrap();
    Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap())
}

/// Set the exact velocity at time `t` on the four background sides.
fn tg_conditions(geom: &FvGeometry, u: &mut VectorField, t: f64) {
    for name in ["xmin", "xmax", "ymin", "ymax"] {
        let v: Vec<Vec3> = geom
            .mesh
            .patch(name)
            .unwrap()
            .faces()
            .map(|f| tg_u(geom.mesh.face_centres()[f], t))
            .collect();
        u.set_bc(geom, name, Box::new(FixedValue(Profile::PerFace(v)))).unwrap();
    }
}

/// Observed order between two errors on meshes whose spacings have the ratio `ratio` (coarse over fine).
fn order(coarse: f64, fine: f64, ratio: f64) -> f64 {
    (coarse / fine).ln() / ratio.ln()
}

// ---- Taylor-Green runs -----------------------------------------------------------------------------------------------------

struct Run {
    s: OversetSolver,
    /// Largest continuity over the steps (C1).
    cont: f64,
    delta_max: f64,
    /// Unconverged linear solves over the steps.
    unconverged: usize,
    /// Largest `n_fallback` over the steps and at the construction (C2).
    fallbacks: usize,
    secs: f64,
}

/// A Taylor-Green run on `ov` with the pose `pose_at(t)`, `steps` steps to `t = 0.5`, started from the exact fields.
fn tg_run(ov: Overset, steps: usize, pose_at: &dyn Fn(f64) -> Pose) -> Run {
    let t0 = Instant::now();
    let dt = T_END / steps as f64;
    let pose = pose_at(0.0);
    let geom = geometry_at(&ov, &pose);
    let mut u = VectorField::from_fn("U", &geom, |x| tg_u(x, 0.0));
    tg_conditions(&geom, &mut u, 0.0);
    let p = ScalarField::from_fn("p", &geom, |x| tg_p(x, 0.0));
    let mut s = OversetSolver::new(
        geom,
        u,
        p,
        settings(NU_TG),
        MassMode::Conservative,
        Some(OversetCoupling { overset: ov, pose }),
    )
    .unwrap();
    let (mut cont, mut delta_max, mut unconverged) = (0.0f64, 0.0f64, 0usize);
    let mut fallbacks = s.conn.n_fallback();
    for k in 1..=steps {
        let t = k as f64 * dt;
        let g = s.geom.clone();
        tg_conditions(&g, &mut s.u, t);
        let info = s.step(dt, pose_at(t), &[]).unwrap();
        cont = cont.max(info.continuity);
        delta_max = delta_max.max(info.delta.abs());
        unconverged += info.unconverged;
        fallbacks = fallbacks.max(info.n_fallback);
    }
    Run {
        s,
        cont,
        delta_max,
        unconverged,
        fallbacks,
        secs: t0.elapsed().as_secs_f64(),
    }
}

/// Cell classes: component fringe, component calculated, background fringe, background calculated.
const CLASSES: [&str; 4] = ["CF", "CC", "BF", "BC"];

fn class_of(kind: CellKind, background: bool) -> Option<usize> {
    match (kind, background) {
        (CellKind::Hole, _) => None,
        (CellKind::Fringe, false) => Some(0),
        (CellKind::Calculated, false) => Some(1),
        (CellKind::Fringe, true) => Some(2),
        (CellKind::Calculated, true) => Some(3),
    }
}

/// The errors of a run at `t = 0.5`: `eps_c = |u_c - u_ex| / e^{-2 nu t}`.
struct Errors {
    class_max: [f64; 4],
    class_rms: [f64; 4],
    /// Largest error over the non-hole cells (A1), its cell, class and place.
    max: f64,
    max_at: (usize, usize, Vec3),
    /// Largest error over the calculated cells (F3 of ADR 0232).
    calc_max: f64,
    /// Root mean square over the non-hole cells, weighted with the volumes (F2 of ADR 0232).
    rms: f64,
}

fn errors(r: &Run) -> Errors {
    let s = &r.s;
    let nb = s.conn.n_background_cells;
    let vol = s.geom.mesh.cell_volumes();
    let centres = s.geom.mesh.cell_centres();
    let scale = (-2.0 * NU_TG * T_END).exp();
    let (mut sum, mut vsum) = ([0.0; 4], [0.0; 4]);
    let mut e = Errors {
        class_max: [0.0; 4],
        class_rms: [0.0; 4],
        max: 0.0,
        max_at: (0, 0, [0.0; 3]),
        calc_max: 0.0,
        rms: 0.0,
    };
    for c in 0..s.geom.n_cells() {
        let Some(k) = class_of(s.conn.kind[c], c < nb) else {
            continue;
        };
        let eps = vec3::norm(vec3::sub(s.u.values[c], tg_u(centres[c], T_END))) / scale;
        e.class_max[k] = e.class_max[k].max(eps);
        sum[k] += vol[c] * eps * eps;
        vsum[k] += vol[c];
        if eps > e.max {
            e.max = eps;
            e.max_at = (c, k, centres[c]);
        }
        if k == 1 || k == 3 {
            e.calc_max = e.calc_max.max(eps);
        }
    }
    for k in 0..4 {
        e.class_rms[k] = (sum[k] / vsum[k]).sqrt();
    }
    e.rms = (sum.iter().sum::<f64>() / vsum.iter().sum::<f64>()).sqrt();
    e
}

fn print_run(name: &str, r: &Run, e: &Errors) {
    println!(
        "{name}: {:.1} s; continuity {:.2e}; largest abs(delta) {:.3e}; unconverged solves {}; fallbacks {}",
        r.secs, r.cont, r.delta_max, r.unconverged, r.fallbacks
    );
    let per_class: Vec<String> = (0..4)
        .map(|k| format!("{} {:.4e} / {:.4e}", CLASSES[k], e.class_max[k], e.class_rms[k]))
        .collect();
    println!(
        "{name}: largest / root-mean-square error by class: {}; largest {:.4e} in cell {} ({}) at ({:.4}, {:.4}); \
         calculated cells {:.4e}; root mean square of the non-hole cells {:.4e}",
        per_class.join(", "),
        e.max,
        e.max_at.0,
        CLASSES[e.max_at.1],
        e.max_at.2[0],
        e.max_at.2[1],
        e.calc_max,
        e.rms
    );
}

fn check_run(c: &mut Checks, name: &str, r: &Run, quadratic: bool) {
    c.at_most(&format!("C1 {name} continuity"), r.cont, 1e-9);
    if quadratic {
        c.at_most(&format!("C2 {name} fallbacks"), r.fallbacks as f64, 0.0);
    }
}

const Q: DonorInterpolation = DonorInterpolation::Quadratic;
const L: DonorInterpolation = DonorInterpolation::Linear;

/// RL at level `level` (`200 level` steps) with the donor interpolation `mode`.
fn rl_run(level: usize, mode: DonorInterpolation) -> Run {
    tg_run(rl_overset(level, mode), 200 * level, &|_| rl_pose())
}

/// TG_L at level `level` (`200 level^2` steps: `dt` proportional to `h^2`) with the donor interpolation `mode`.
fn tg_level_run(level: usize, mode: DonorInterpolation) -> Run {
    tg_run(tg_overset(level, mode), 200 * level * level, &tg_pose)
}

/// The numbers of ADR 0246 for RL at levels 1 to 4: the root mean square over CC with the linear stencils (T) and with
/// the nine-cell product stencils of the test (TQ).
const RL_LINEAR_0246: [f64; 4] = [1.5486e-4, 6.3902e-5, 3.9830e-5, 2.4949e-5];
const RL_PRODUCT_0246: [f64; 4] = [3.2411e-4, 8.5532e-5, 3.9022e-5, 2.2269e-5];

// ---- A2 and NL -------------------------------------------------------------------------------------------------------------

/// ADR 0251 A2 on the levels 1 and 2 with C1 and C2, and the negative control NL (the default stencils on the same two
/// levels). The levels 3 and 4 are in `a2_rl_levels_1_to_4`.
#[test]
fn a2_rl_levels_1_and_2_and_the_linear_control() {
    let mut c = Checks::new();
    let (mut eq, mut el) = (Vec::new(), Vec::new());
    for l in [1usize, 2] {
        let r = rl_run(l, Q);
        let e = errors(&r);
        print_run(&format!("RL level {l}, quadratic"), &r, &e);
        check_run(&mut c, &format!("RL level {l}, quadratic"), &r, true);
        eq.push(e.class_rms[1]);
        let r = rl_run(l, L);
        let e = errors(&r);
        print_run(&format!("RL level {l}, linear"), &r, &e);
        check_run(&mut c, &format!("RL level {l}, linear"), &r, false);
        el.push(e.class_rms[1]);
        println!(
            "RL level {l}: root mean square over CC {:.4e} quadratic, {:.4e} linear (ADR 0246: {:.4e}), ratio {:.3}; \
             ADR 0246 with the product stencils {:.4e}",
            eq[l - 1],
            el[l - 1],
            RL_LINEAR_0246[l - 1],
            eq[l - 1] / el[l - 1],
            RL_PRODUCT_0246[l - 1]
        );
    }
    c.at_least("A2 order, levels 1 to 2", order(eq[0], eq[1], 2.0), 1.7);
    c.at_most(
        "NL (negative control) order with the default stencils, levels 1 to 2",
        order(el[0], el[1], 2.0),
        1.5,
    );
    c.finish();
}

/// ADR 0251 A2 (the acceptance): RL with the quadratic donor interpolation on the levels 1 to 4.
#[test]
#[ignore = "slow: RL on four levels, about 15 minutes; run once for ADR 0251 addendum 1"]
fn a2_rl_levels_1_to_4() {
    let mut c = Checks::new();
    let mut eq = Vec::new();
    for l in 1usize..=4 {
        let r = rl_run(l, Q);
        let e = errors(&r);
        print_run(&format!("RL level {l}, quadratic"), &r, &e);
        check_run(&mut c, &format!("RL level {l}, quadratic"), &r, true);
        println!(
            "RL level {l}: root mean square over CC {:.4e} (largest {:.4e}); ADR 0246: linear {:.4e} (ratio {:.3}), \
             product stencils {:.4e} (ratio {:.3})",
            e.class_rms[1],
            e.class_max[1],
            RL_LINEAR_0246[l - 1],
            e.class_rms[1] / RL_LINEAR_0246[l - 1],
            RL_PRODUCT_0246[l - 1],
            e.class_rms[1] / RL_PRODUCT_0246[l - 1]
        );
        eq.push(e.class_rms[1]);
    }
    for l in 1..4 {
        c.at_least(
            &format!("A2 order, levels {l} to {}", l + 1),
            order(eq[l - 1], eq[l], (l + 1) as f64 / l as f64),
            1.7,
        );
    }
    c.finish();
}

// ---- A1 --------------------------------------------------------------------------------------------------------------------

/// The numbers of ADR 0232 for TG_L at levels 1 to 3 with the linear stencils: the largest error, the largest error of
/// the calculated cells and the root mean square of the non-hole cells.
const TG_LINEAR_0232: [[f64; 3]; 3] = [
    [1.0785e-3, 8.734e-4, 5.534e-4],
    [3.514e-4, 2.096e-4, 1.395e-4],
    [1.712e-4, 1.104e-4, 6.316e-5],
];

/// The TG_L runs of A1 on `levels` and their printed numbers; returns the largest errors.
fn a1_runs(c: &mut Checks, levels: &[usize]) -> Vec<f64> {
    let mut out = Vec::new();
    let mut all: Vec<Errors> = Vec::new();
    for &l in levels {
        let r = tg_level_run(l, Q);
        let e = errors(&r);
        print_run(&format!("TG_{l}, quadratic"), &r, &e);
        check_run(c, &format!("TG_{l}, quadratic"), &r, true);
        let was = TG_LINEAR_0232[l - 1];
        println!(
            "TG_{l}: largest {:.4e} (linear, ADR 0232: {:.4e}), calculated cells {:.4e} ({:.4e}), root mean square \
             {:.4e} ({:.4e})",
            e.max, was[0], e.calc_max, was[1], e.rms, was[2]
        );
        out.push(e.max);
        all.push(e);
    }
    for (i, w) in levels.windows(2).enumerate() {
        let ratio = w[1] as f64 / w[0] as f64;
        println!(
            "TG levels {} to {} (printed): order of the calculated maximum {:.3}, of the root mean square {:.3}",
            w[0],
            w[1],
            order(all[i].calc_max, all[i + 1].calc_max, ratio),
            order(all[i].rms, all[i + 1].rms, ratio)
        );
    }
    out
}

/// ADR 0251 A1 on the levels 1 and 2, with C1 and C2. The level 3 is in `a1_tg_levels_1_to_3`.
#[test]
fn a1_tg_levels_1_and_2() {
    let mut c = Checks::new();
    let e = a1_runs(&mut c, &[1, 2]);
    c.at_least("A1 order, levels 1 to 2", order(e[0], e[1], 2.0), 1.7);
    c.finish();
}

/// ADR 0251 A1 (the acceptance): TG_L with the quadratic donor interpolation on the levels 1 to 3, `dt` proportional to
/// `h^2`.
#[test]
#[ignore = "slow: TG_L on three levels with 200, 800 and 1800 steps, about 10 minutes; run once for ADR 0251 addendum 1"]
fn a1_tg_levels_1_to_3() {
    let mut c = Checks::new();
    let e = a1_runs(&mut c, &[1, 2, 3]);
    c.at_least("A1 order, levels 1 to 2", order(e[0], e[1], 2.0), 1.7);
    c.at_least("A1 order, levels 2 to 3", order(e[1], e[2], 1.5), 1.7);
    c.finish();
}

// ---- CM --------------------------------------------------------------------------------------------------------------------

/// The background alone with the library step (TGB of ADR 0219), `steps` steps to `t = 0.5`.
fn background_alone(level: usize, steps: usize) -> Vec<Vec3> {
    let dt = T_END / steps as f64;
    let geom = Arc::new(FvGeometry::new(Arc::new(tg_background(level))).unwrap());
    let mut u = VectorField::from_fn("U", &geom, |x| tg_u(x, 0.0));
    tg_conditions(&geom, &mut u, 0.0);
    let p = ScalarField::from_fn("p", &geom, |x| tg_p(x, 0.0));
    let mut s = PimpleSolver::new(geom.clone(), u, p, settings(NU_TG));
    for k in 1..=steps {
        tg_conditions(&geom, &mut s.u, k as f64 * dt);
        s.step(dt);
    }
    s.u.values.clone()
}

/// ADR 0251 CM (control): with coincident meshes and the quadratic donor interpolation the overset solution is the
/// solution of the background alone.
#[test]
fn cm_coincident_meshes_give_the_background_alone() {
    let mut c = Checks::new();
    for l in [1usize, 2] {
        let ov = cm_overset(l, Q);
        let r = tg_run(ov.clone(), 200 * l, &|_| cm_pose());
        let alone = background_alone(l, 200 * l);
        let s = &r.s;
        let centres = s.geom.mesh.cell_centres();
        let (mut worst, mut worst_weight) = (0.0f64, 0.0f64);
        for cell in 0..s.geom.n_cells() {
            if s.conn.kind[cell] != CellKind::Calculated {
                continue;
            }
            let twin = ov
                .background_locator()
                .locate(&ov.composite.mesh, centres[cell])
                .unwrap();
            assert!(
                vec3::norm(vec3::sub(centres[twin], centres[cell])) < 1e-9,
                "the cells do not coincide"
            );
            worst = worst.max(vec3::norm(vec3::sub(s.u.values[cell], alone[twin])));
        }
        for (_, st) in &s.conn.stencils {
            worst_weight = worst_weight.max((st.weights[0] - 1.0).abs());
        }
        println!(
            "CM level {l}: {:.1} s; largest abs(1 - donor weight) {worst_weight:.3e}; largest abs(delta) {:.3e}; \
             unconverged solves {}",
            r.secs, r.delta_max, r.unconverged
        );
        check_run(&mut c, &format!("CM level {l}"), &r, true);
        c.at_most(
            &format!("CM level {l}: difference to the background alone"),
            worst,
            1e-6,
        );
    }
    c.finish();
}

// ---- A3 --------------------------------------------------------------------------------------------------------------------

const U_FLOW: Vec3 = [1.0, 0.5, 0.0];
const DT_U: f64 = 0.01;

fn u_pose(t: f64) -> Pose {
    Pose {
        origin: [0.0; 3],
        axis: [0.0, 0.0, 1.0],
        angle: t,
        translation: [t, 0.5 * t, 0.0],
    }
}

fn patch_flux(geom: &FvGeometry, phi: &[f64], patch: &str) -> f64 {
    geom.mesh.patch(patch).unwrap().faces().map(|f| phi[f]).sum()
}

/// ADR 0251 A3: the uniform flow U of ADR 0219 (U1 and U3) through a component that moves and turns, with the quadratic
/// donor interpolation.
#[test]
fn a3_uniform_flow_through_a_moving_rotating_component() {
    let bs = uniform(-2.5, 5.0, 50);
    let ov = box_overset(rectilinear_2d(&bs, &bs, DEPTH).unwrap(), 0.6, 16, 0.25, Q);
    let pose = u_pose(0.0);
    let geom = geometry_at(&ov, &pose);
    let mut u = VectorField::uniform("U", &geom, U_FLOW);
    for name in ["xmin", "ymin", "ymax"] {
        u.set_bc(&geom, name, Box::new(FixedValue::vector(U_FLOW))).unwrap();
    }
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    p.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
    let mut s = OversetSolver::new(
        geom,
        u,
        p,
        settings(0.01),
        MassMode::Conservative,
        Some(OversetCoupling { overset: ov, pose }),
    )
    .unwrap();
    let n = s.geom.n_cells();
    let un = vec3::norm(U_FLOW);
    let (mut eu, mut ep, mut cont, mut q) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    let (mut changes, mut fallbacks, mut unconverged) = (0, s.conn.n_fallback(), 0);
    let (mut reduced, mut smallest, mut stencils) = (0usize, usize::MAX, 0usize);
    for k in 1..=80 {
        let info = s.step(DT_U, u_pose(k as f64 * DT_U), &[]).unwrap();
        cont = cont.max(info.continuity);
        changes += info.n_changed;
        fallbacks = fallbacks.max(info.n_fallback);
        unconverged += info.unconverged;
        assert_eq!(s.conn.interpolation, Q);
        for (_, st) in &s.conn.stencils {
            stencils += 1;
            reduced += usize::from(st.cells.len() < 9);
            smallest = smallest.min(st.cells.len());
        }
        for c in 0..n {
            if s.conn.kind[c] != CellKind::Hole {
                eu = eu.max(vec3::norm(vec3::sub(s.u.values[c], U_FLOW)) / un);
                ep = ep.max(s.p.values[c].abs() / (un * un));
            }
        }
        let qx = patch_flux(&s.geom, &s.phi, "xmax");
        q = q.max((qx - 0.5).abs() / 0.5);
    }
    println!(
        "U with the quadratic donor interpolation: {stencils} stencils over 80 steps, {reduced} with fewer than nine \
         cells (smallest {smallest}); {changes} kind changes; unconverged solves {unconverged}"
    );
    let mut c = Checks::new();
    c.at_most("A3 U1 velocity", eu, 1e-10);
    c.at_most("A3 U1 pressure", ep, 1e-10);
    c.at_most("A3 U3 continuity", cont, 1e-9);
    c.at_most("A3 U3 outflow", q, 1e-10);
    c.at_most("C2 U fallbacks", fallbacks as f64, 0.0);
    c.at_least("U4 (set-up) kind changes", changes as f64, 20.0);
    c.finish();
}

// ---- E1 --------------------------------------------------------------------------------------------------------------------

/// ADR 0251 E1: a step whose interpolated `rAU` is not positive in a fringe cell is refused by name.
#[test]
fn e1_non_positive_interpolated_rau_is_refused() {
    let ov = rl_overset(1, Q);
    let pose = rl_pose();
    let geom = geometry_at(&ov, &pose);
    let mut u = VectorField::from_fn("U", &geom, |x| tg_u(x, 0.0));
    tg_conditions(&geom, &mut u, 0.0);
    let p = ScalarField::from_fn("p", &geom, |x| tg_p(x, 0.0));
    let mut s = OversetSolver::new(
        geom,
        u,
        p,
        settings(NU_TG),
        MassMode::Conservative,
        Some(OversetCoupling { overset: ov, pose }),
    )
    .unwrap();
    let dt = T_END / 200.0;
    let g = s.geom.clone();
    tg_conditions(&g, &mut s.u, dt);
    let mut st = s.prepare(dt, pose, &[]).unwrap();
    assert_eq!(st.conn.interpolation, Q);
    let nb = st.conn.n_background_cells;
    let (cell, stn) = st
        .conn
        .stencils
        .iter_mut()
        .find(|(c, _)| *c < nb)
        .expect("a background fringe cell");
    let cell = *cell;
    *stn = Stencil {
        cells: vec![stn.cells[0]],
        weights: vec![-1.0],
    };
    let e = s.advance(dt, st);
    println!("E1: fringe cell {cell}: {e:?}");
    let mut c = Checks::new();
    c.holds(
        "E1",
        e.as_ref()
            .is_err_and(|m| m.contains("rAU") && m.contains(&format!("fringe cell {cell} "))),
        "the step is refused and names rAU and the cell",
    );
    c.finish();
}
