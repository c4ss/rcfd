//! F-336 (ADR 0251): the opt-in donor interpolation of the overset coupling that reproduces quadratic fields. The
//! kinematic checks (no flow solve): the default stencils are those of ADR 0219 M4 bit for bit (D1), and with
//! `DonorInterpolation::Quadratic` the weights sum to one (P1), reproduce quadratics on Cartesian, distorted, curved and
//! three-dimensional donor meshes (P2 to P4), use calculated cells of the other part only (P5), have nine cells and no
//! fallback on the registered set-ups (P6), a bounded sum of absolute values (P7), an interpolation of third order (P8),
//! a reported fallback to the linear stencil where the neighbourhood cannot carry a quadratic (P9) and named refusals
//! (P10). The flow checks are in `crates/rcfd-fv/tests/overset_f336.rs`.
//!
//! Release: `cargo test --release -p rcfd-mesh --test overset_f336`.

use rcfd_core::vec3::{self, Vec3};
use rcfd_mesh::gen::{axis_coords, box_mesh, cylinder_ogrid, expansion_for_end_cell, rectilinear_2d, warp, Grading};
use rcfd_mesh::motion::{moved_mesh, Pose};
use rcfd_mesh::overset::{
    compose, containment_margin, linear_weights, pose_inverse_apply, quadratic_weights, CellKind, Connectivity,
    DonorInterpolation, Overset, OversetSpec, Stencil,
};
use rcfd_mesh::Mesh;
use std::f64::consts::PI;
use std::sync::Arc;
use std::time::Instant;

const DEPTH: f64 = 0.1;
/// Background cell size of level 1 of RL and TG_L.
const H1: f64 = PI / 40.0;

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

fn square_spec(hole: f64) -> OversetSpec {
    OversetSpec {
        outer_patches: ["c_xmin", "c_xmax", "c_ymin", "c_ymax"].map(String::from).to_vec(),
        hole: Arc::new(move |x: Vec3| x[0].abs().max(x[1].abs()) < hole),
    }
}

/// A square component of half width `half` with `n` cells per side over `bg`, the hole `max(|x_ref|, |y_ref|) < hole`.
fn box_overset(bg: &Mesh, half: f64, n: usize, hole: f64) -> Overset {
    let cs = uniform(-half, 2.0 * half, n);
    let comp = rectilinear_2d(&cs, &cs, DEPTH).unwrap();
    Overset::new(compose(bg, &comp, "c_").unwrap(), square_spec(hole)).unwrap()
}

fn tg_background(level: usize) -> Mesh {
    let bs = uniform(0.0, PI, 40 * level);
    rectilinear_2d(&bs, &bs, DEPTH).unwrap()
}

/// RL of ADR 0232 amendment 3 at level `level`.
fn rl(level: usize) -> Overset {
    box_overset(&tg_background(level), 7.5 * H1, 20 * level, 3.1 * H1)
}

fn diagonal(d: f64) -> Pose {
    Pose::translation([d, d, 0.0])
}

fn rl_pose() -> Pose {
    diagonal(16.5 * H1)
}

/// TG_L of ADR 0219.
fn tg(level: usize) -> Overset {
    box_overset(&tg_background(level), 0.6, 20 * level, 0.25)
}

fn tg_pose(t: f64) -> Pose {
    Pose {
        origin: [0.0; 3],
        axis: [0.0, 0.0, 1.0],
        angle: t,
        translation: [PI / 2.0 - 0.3 + 0.5 * t, PI / 2.0 - 0.2 + 0.3 * t, 0.0],
    }
}

/// The 200 poses of the registered motion of TG_L.
fn tg_poses() -> Vec<Pose> {
    (1..=200).map(|k| tg_pose(k as f64 * 0.0025)).collect()
}

/// U of ADR 0219 and its 80 poses.
fn u_overset() -> Overset {
    let bs = uniform(-2.5, 5.0, 50);
    box_overset(&rectilinear_2d(&bs, &bs, DEPTH).unwrap(), 0.6, 16, 0.25)
}

fn u_poses() -> Vec<Pose> {
    (1..=80)
        .map(|k| {
            let t = k as f64 * 0.01;
            Pose {
                origin: [0.0; 3],
                axis: [0.0, 0.0, 1.0],
                angle: t,
                translation: [t, 0.5 * t, 0.0],
            }
        })
        .collect()
}

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

/// TR of ADR 0219 (the cylinder O-grid over a graded background) and the five poses of its unit tests.
fn tr() -> Overset {
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
    let bg = rectilinear_2d(&xs, &ys, 0.1).unwrap();
    let q: f64 = 1.08;
    let r_out = 0.5 + 0.01 * (q.powi(30) - 1.0) / (q - 1.0);
    let comp = cylinder_ogrid(96, 30, 0.5, r_out, Grading::Expansion(q.powi(29)), 0.1).unwrap();
    let spec = OversetSpec {
        outer_patches: vec!["c_farfield".into()],
        hole: Arc::new(|x: Vec3| x[0] * x[0] + x[1] * x[1] < 1.0),
    };
    Overset::new(compose(&bg, &comp, "c_").unwrap(), spec).unwrap()
}

fn tr_poses() -> Vec<Pose> {
    [0.0, -0.37, -1.234, -3.9, -7.5]
        .iter()
        .map(|s| Pose::translation([2.0 + s, 0.0, 0.0]))
        .collect()
}

/// A fixed pseudo-random number in [-1, 1] of a position.
fn jitter(x: Vec3, seed: f64) -> f64 {
    let v = (x[0] * 127.1 + x[1] * 311.7 + x[2] * 74.7 + seed).sin() * 43758.5453;
    2.0 * (v - v.floor()) - 1.0
}

/// `mesh` (a box from `lo` to `hi` in the directions `dirs`) with every interior point displaced by up to `amp` in each
/// of those directions; the points on the sides of the box stay.
fn distorted(mesh: &Mesh, lo: f64, hi: f64, dirs: &[usize], amp: f64, planar: bool) -> Mesh {
    let dirs = dirs.to_vec();
    warp(mesh, move |p| {
        let tol = 1e-9 * (hi - lo);
        if dirs.iter().any(|&d| p[d] - lo < tol || hi - p[d] < tol) {
            return p;
        }
        // in a mesh that is one cell thick both layers of points get the same displacement
        let key = if planar { [p[0], p[1], 0.0] } else { p };
        let mut q = p;
        for &d in &dirs {
            q[d] += amp * jitter(key, 1.0 + d as f64);
        }
        q
    })
    .unwrap()
}

/// DC of ADR 0251: RL at level 2 with the interior points of both parts displaced by up to 0.15 of the cell size.
fn dc() -> Overset {
    let level = 2;
    let bg = distorted(&tg_background(level), 0.0, PI, &[0, 1], 0.15 * H1 / level as f64, true);
    let half = 7.5 * H1;
    let n = 20 * level;
    let cs = uniform(-half, 2.0 * half, n);
    let comp = distorted(
        &rectilinear_2d(&cs, &cs, DEPTH).unwrap(),
        -half,
        half,
        &[0, 1],
        0.15 * 2.0 * half / n as f64,
        true,
    );
    Overset::new(compose(&bg, &comp, "c_").unwrap(), square_spec(3.1 * H1)).unwrap()
}

/// B3 of ADR 0251 (`distort`: B3d).
fn b3(distort: bool) -> (Overset, Pose) {
    let mut bg = box_mesh([24; 3], [2.4; 3], [Grading::Uniform; 3]).unwrap();
    let mut comp = box_mesh([14; 3], [1.4; 3], [Grading::Uniform; 3]).unwrap();
    if distort {
        bg = distorted(&bg, 0.0, 2.4, &[0, 1, 2], 0.015, false);
        comp = distorted(&comp, 0.0, 1.4, &[0, 1, 2], 0.015, false);
    }
    let spec = OversetSpec {
        outer_patches: ["c_xmin", "c_xmax", "c_ymin", "c_ymax", "c_zmin", "c_zmax"]
            .map(String::from)
            .to_vec(),
        hole: Arc::new(|x: Vec3| (0..3).map(|d| (x[d] - 0.7).abs()).fold(0.0, f64::max) < 0.15),
    };
    let ov = Overset::new(compose(&bg, &comp, "c_").unwrap(), spec).unwrap();
    let pose = Pose {
        origin: [0.7; 3],
        axis: vec3::unit([1.0, 2.0, 3.0]),
        angle: 0.3,
        translation: [0.53, 0.52, 0.49],
    };
    (ov, pose)
}

/// FB of ADR 0251: RL at level 1 shifted by 0.4 H with the hole `< 6.1 H`.
fn fb() -> (Overset, Pose) {
    (
        box_overset(&tg_background(1), 7.5 * H1, 20, 6.1 * H1),
        diagonal(16.9 * H1),
    )
}

fn quadratic(mut ov: Overset) -> Overset {
    ov.set_donor_interpolation(DonorInterpolation::Quadratic);
    ov
}

fn mesh_at(ov: &Overset, pose: &Pose) -> Mesh {
    moved_mesh(&ov.composite.mesh, ov.composite.points_at(pose)).unwrap()
}

/// `1 + 0.7 x - 0.4 y + 0.3 x^2 - 0.6 x y + 0.5 y^2`: all six monomials of degree at most two in two directions.
fn q2(x: Vec3) -> f64 {
    1.0 + 0.7 * x[0] - 0.4 * x[1] + 0.3 * x[0] * x[0] - 0.6 * x[0] * x[1] + 0.5 * x[1] * x[1]
}

/// `q2 + 0.2 z - 0.35 z^2 + 0.45 x z - 0.25 y z`: all ten monomials in three directions.
fn q3(x: Vec3) -> f64 {
    q2(x) + 0.2 * x[2] - 0.35 * x[2] * x[2] + 0.45 * x[0] * x[2] - 0.25 * x[1] * x[2]
}

fn linear_field(x: Vec3) -> f64 {
    3.0 + 2.0 * x[0] - 5.0 * x[1] + 1.5 * x[2]
}

fn tg_u(x: Vec3) -> Vec3 {
    [-x[0].cos() * x[1].sin(), x[0].sin() * x[1].cos(), 0.0]
}

// ---- what is measured on the stencils of one or more poses -------------------------------------------------------------

/// The measures of ADR 0251 P1 to P8 over the stencils of the poses seen so far.
#[derive(Debug)]
struct Stats {
    stencils: usize,
    /// P1: largest `|sum w - 1|`.
    sum: f64,
    /// P2 to P4: largest error of the field over the stencils that are not fallbacks, over `max |f|` of the cells.
    field: f64,
    /// The same over the fallback stencils.
    field_fallback: f64,
    /// P5: stencil cells that are not calculated, of the same part, or twice in a stencil; donors that are not the
    /// locator's.
    inadmissible: usize,
    wrong_donor: usize,
    /// P6.
    smallest: usize,
    largest: usize,
    fallbacks: usize,
    /// Stencils that are not fallbacks and have fewer than `whole` cells.
    reduced: usize,
    /// P7: largest `sum |w|` and smallest weight over the stencils that are not fallbacks.
    lambda: f64,
    w_min: f64,
    /// P8: largest truncation of the Taylor-Green velocity.
    trunc: f64,
}

impl Stats {
    fn new() -> Self {
        Stats {
            stencils: 0,
            sum: 0.0,
            field: 0.0,
            field_fallback: 0.0,
            inadmissible: 0,
            wrong_donor: 0,
            smallest: usize::MAX,
            largest: 0,
            fallbacks: 0,
            reduced: 0,
            lambda: 0.0,
            w_min: f64::INFINITY,
            trunc: 0.0,
        }
    }

    /// Add the stencils of `conn` (the connectivity of `ov` at `pose` on the mesh `moved`).
    fn add(
        &mut self,
        ov: &Overset,
        moved: &Mesh,
        pose: &Pose,
        conn: &Connectivity,
        f: &dyn Fn(Vec3) -> f64,
        whole: usize,
    ) {
        let nb = ov.composite.n_background_cells;
        let reference = &ov.composite.mesh;
        let cc = moved.cell_centres();
        let fmax = cc.iter().map(|&x| f(x).abs()).fold(0.0, f64::max);
        self.fallbacks += conn.n_fallback();
        for (c, st) in &conn.stencils {
            let fallback = conn.fallbacks.binary_search(c).is_ok();
            self.stencils += 1;
            let sw: f64 = st.weights.iter().sum();
            self.sum = self.sum.max((sw - 1.0).abs());
            let fi: f64 = st.cells.iter().zip(&st.weights).map(|(&k, w)| w * f(cc[k])).sum();
            let e = (fi - f(cc[*c])).abs() / fmax;
            let mut ui = [0.0; 3];
            for (&k, w) in st.cells.iter().zip(&st.weights) {
                ui = vec3::add(ui, vec3::scale(tg_u(cc[k]), *w));
            }
            self.trunc = self.trunc.max(vec3::norm(vec3::sub(ui, tg_u(cc[*c]))));
            let donor = if *c < nb {
                ov.component_locator()
                    .locate(reference, pose_inverse_apply(pose, cc[*c]))
            } else {
                ov.background_locator().locate(reference, cc[*c])
            };
            if donor != Some(st.cells[0]) {
                self.wrong_donor += 1;
            }
            let mut seen = st.cells.clone();
            seen.sort_unstable();
            seen.dedup();
            self.inadmissible += st.cells.len() - seen.len();
            self.inadmissible += st
                .cells
                .iter()
                .filter(|&&k| conn.kind[k] != CellKind::Calculated || (k < nb) == (*c < nb))
                .count();
            if fallback {
                self.field_fallback = self.field_fallback.max(e);
            } else {
                self.field = self.field.max(e);
                self.smallest = self.smallest.min(st.cells.len());
                self.largest = self.largest.max(st.cells.len());
                self.reduced += usize::from(st.cells.len() < whole);
                self.lambda = self.lambda.max(st.weights.iter().map(|w| w.abs()).sum());
                self.w_min = st.weights.iter().fold(self.w_min, |a, &w| a.min(w));
            }
        }
    }

    fn print(&self, name: &str) {
        println!(
            "{name}: {} stencils of {} to {} cells, {} reduced, {} fallbacks; |sum w - 1| {:.2e}; field {:.2e}; \
             inadmissible cells {}, wrong donors {}; Lambda {:.4}, smallest weight {:.4}; truncation {:.4e}",
            self.stencils,
            self.smallest,
            self.largest,
            self.reduced,
            self.fallbacks,
            self.sum,
            self.field,
            self.inadmissible,
            self.wrong_donor,
            self.lambda,
            self.w_min,
            self.trunc
        );
    }
}

/// The measures over the connectivities of `ov` at `poses`.
fn examine(ov: &Overset, poses: &[Pose], f: &dyn Fn(Vec3) -> f64, whole: usize) -> Stats {
    let mut s = Stats::new();
    for pose in poses {
        let moved = mesh_at(ov, pose);
        let conn = ov.connectivity(&moved, pose).unwrap();
        s.add(ov, &moved, pose, &conn, f, whole);
    }
    s
}

/// The stencils of ADR 0219 M4 built from the public parts: the donor of the locator, its calculated face neighbours,
/// `linear_weights`.
fn m4_stencils(ov: &Overset, moved: &Mesh, pose: &Pose, kind: &[CellKind]) -> Vec<(usize, Stencil)> {
    let nb = ov.composite.n_background_cells;
    let reference = &ov.composite.mesh;
    let cc = moved.cell_centres();
    let rc = reference.cell_centres();
    let mut out = Vec::new();
    for c in 0..kind.len() {
        if kind[c] != CellKind::Fringe {
            continue;
        }
        let (x, locator) = if c < nb {
            (pose_inverse_apply(pose, cc[c]), ov.component_locator())
        } else {
            (cc[c], ov.background_locator())
        };
        let d = locator.locate(reference, x).unwrap();
        let mut cells = vec![d];
        cells.extend(
            ov.neighbours(d)
                .iter()
                .copied()
                .filter(|&q| kind[q] == CellKind::Calculated),
        );
        let pts: Vec<Vec3> = cells.iter().map(|&q| rc[q]).collect();
        let weights = linear_weights(&pts, x, ov.dims).unwrap();
        out.push((c, Stencil { cells, weights }));
    }
    out
}

fn same_bits(a: &[(usize, Stencil)], b: &[(usize, Stencil)]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|((ca, sa), (cb, sb))| {
            ca == cb
                && sa.cells == sb.cells
                && sa.weights.len() == sb.weights.len()
                && sa
                    .weights
                    .iter()
                    .zip(&sb.weights)
                    .all(|(x, y)| x.to_bits() == y.to_bits())
        })
}

// ---- D1 --------------------------------------------------------------------------------------------------------------------

/// ADR 0251 D1: without the opt-in, and after the opt-in was switched on and off again, the stencils are those of M4.
#[test]
fn d1_default_stencils_are_those_of_m4() {
    let mut c = Checks::new();
    let cases: Vec<(&str, Overset, Vec<Pose>)> = vec![
        ("RL level 1", rl(1), vec![rl_pose()]),
        ("RL level 2", rl(2), vec![rl_pose()]),
        ("TG_1, 200 poses", tg(1), tg_poses()),
        ("U, 80 poses", u_overset(), u_poses()),
        ("TR, 5 poses", tr(), tr_poses()),
    ];
    for (name, ov, poses) in cases {
        assert_eq!(ov.donor_interpolation(), DonorInterpolation::Linear);
        let mut back = ov.clone();
        back.set_donor_interpolation(DonorInterpolation::Quadratic);
        back.set_donor_interpolation(DonorInterpolation::Linear);
        let (mut equal, mut clean, mut n) = (true, true, 0);
        for pose in &poses {
            let moved = mesh_at(&ov, pose);
            let a = ov.connectivity(&moved, pose).unwrap();
            let b = back.connectivity(&moved, pose).unwrap();
            let m4 = m4_stencils(&ov, &moved, pose, &a.kind);
            equal &= a.kind == b.kind && same_bits(&a.stencils, &m4) && same_bits(&b.stencils, &m4);
            clean &= a.fallbacks.is_empty()
                && b.fallbacks.is_empty()
                && a.interpolation == DonorInterpolation::Linear
                && b.interpolation == DonorInterpolation::Linear;
            n += a.stencils.len();
        }
        c.holds(
            &format!("D1 {name}"),
            equal && clean,
            &format!("{n} stencils equal to M4 bit for bit, no fallback listed"),
        );
    }
    c.finish();
}

// ---- P1, P2, P5 to P8, NK on the Cartesian set-ups ---------------------------------------------------------------------

/// ADR 0251 P1, P2, P5, P6, P7, P8 and NK on RL, TG_L and U.
#[test]
fn p_cartesian_set_ups() {
    let mut c = Checks::new();
    let all = |c: &mut Checks, name: &str, s: &Stats, whole: bool| {
        s.print(name);
        c.at_most(&format!("P1 {name} |sum w - 1|"), s.sum, 1e-12);
        c.at_most(&format!("P2 {name} q2"), s.field, 1e-11);
        c.holds(
            &format!("P5 {name}"),
            s.inadmissible == 0 && s.wrong_donor == 0,
            "calculated cells of the other part, the donor first",
        );
        c.at_most(&format!("P6 {name} fallbacks"), s.fallbacks as f64, 0.0);
        if whole {
            c.holds(
                &format!("P6 {name} size"),
                s.smallest == 9 && s.largest == 9,
                "nine cells in every stencil",
            );
            c.at_most(&format!("P7 {name} Lambda"), s.lambda, 2.5);
            c.at_least(&format!("P7 {name} smallest weight"), s.w_min, -0.5);
        }
    };
    let levels = [1usize, 2, 4];
    let (mut rl_q, mut rl_l, mut tg_q) = (Vec::new(), Vec::new(), Vec::new());
    for &l in &levels {
        let lin = rl(l);
        let s_lin = examine(&lin, &[rl_pose()], &q2, 5);
        let ov = quadratic(lin);
        let s = examine(&ov, &[rl_pose()], &q2, 9);
        all(&mut c, &format!("RL level {l}"), &s, true);
        rl_q.push(s.trunc);
        rl_l.push(s_lin.trunc);
        println!(
            "RL level {l}, default stencils: q2 {:.3e}, truncation {:.4e}, Lambda {:.4}, smallest weight {:.4}",
            s_lin.field, s_lin.trunc, s_lin.lambda, s_lin.w_min
        );
        let s = examine(&quadratic(tg(l)), &tg_poses(), &q2, 9);
        all(&mut c, &format!("TG_{l}, 200 poses"), &s, true);
        tg_q.push(s.trunc);
    }
    let s = examine(&quadratic(u_overset()), &u_poses(), &q2, 9);
    all(&mut c, "U, 80 poses", &s, false);
    for (name, v, limit) in [("P8 RL", &rl_q, 2.7), ("P8 TG_L", &tg_q, 2.7)] {
        println!("{name}: I_sup {:.4e}, {:.4e}, {:.4e}", v[0], v[1], v[2]);
        c.at_least(&format!("{name} order, levels 1 to 2"), (v[0] / v[1]).log2(), limit);
        c.at_least(&format!("{name} order, levels 2 to 4"), (v[1] / v[2]).log2(), limit);
    }
    println!(
        "NK RL, default stencils: I_sup {:.4e}, {:.4e}, {:.4e}",
        rl_l[0], rl_l[1], rl_l[2]
    );
    c.at_most(
        "NK (negative control) order, levels 1 to 2",
        (rl_l[0] / rl_l[1]).log2(),
        2.3,
    );
    c.at_most(
        "NK (negative control) order, levels 2 to 4",
        (rl_l[1] / rl_l[2]).log2(),
        2.3,
    );
    // cost (printed, no limit): the connectivity of RL at level 4 in both modes
    let lin = rl(4);
    let moved = mesh_at(&lin, &rl_pose());
    let time = |ov: &Overset| {
        let t0 = Instant::now();
        for _ in 0..5 {
            ov.connectivity(&moved, &rl_pose()).unwrap();
        }
        t0.elapsed().as_secs_f64() / 5.0
    };
    let t_lin = time(&lin);
    let t0 = Instant::now();
    let quad = quadratic(lin);
    let t_table = t0.elapsed().as_secs_f64();
    let t_quad = time(&quad);
    println!(
        "cost, RL level 4 ({} cells): connectivity {:.2} ms linear, {:.2} ms quadratic; the point table once {:.2} ms",
        quad.composite.mesh.n_cells(),
        1e3 * t_lin,
        1e3 * t_quad,
        1e3 * t_table
    );
    c.finish();
}

// ---- P3 --------------------------------------------------------------------------------------------------------------------

/// ADR 0251 P1, P3, P5, P6 on a distorted quadrilateral donor mesh (DC) and on the cylinder O-grid over a graded
/// background (TR).
#[test]
fn p_distorted_and_curved_donor_meshes() {
    let mut c = Checks::new();
    for (name, ov, poses, limit) in [
        ("DC", quadratic(dc()), vec![rl_pose()], 1e-11),
        ("TR, 5 poses", quadratic(tr()), tr_poses(), 1e-10),
    ] {
        let s = examine(&ov, &poses, &q2, 9);
        s.print(name);
        c.at_most(&format!("P1 {name} |sum w - 1|"), s.sum, 1e-12);
        c.at_most(&format!("P3 {name} q2"), s.field, limit);
        c.holds(
            &format!("P5 {name}"),
            s.inadmissible == 0 && s.wrong_donor == 0,
            "calculated cells of the other part, the donor first",
        );
        c.at_most(&format!("P6 {name} fallbacks"), s.fallbacks as f64, 0.0);
    }
    // the distortion is real: the default stencils of DC against those of RL at level 2
    let (a, b) = (dc(), rl(2));
    let d = a
        .composite
        .mesh
        .cell_centres()
        .iter()
        .zip(b.composite.mesh.cell_centres())
        .map(|(x, y)| vec3::norm(vec3::sub(*x, *y)))
        .fold(0.0, f64::max);
    c.at_least(
        "DC (set-up) largest displacement of a cell centre over the cell size",
        d / (0.75 * H1 / 2.0),
        0.03,
    );
    c.finish();
}

// ---- P4 --------------------------------------------------------------------------------------------------------------------

/// ADR 0251 P1, P4, P5, P6 in three dimensions (B3, and B3d with displaced points).
#[test]
fn p_three_dimensions() {
    let mut c = Checks::new();
    for (name, distort) in [("B3", false), ("B3d", true)] {
        let (ov, pose) = b3(distort);
        assert_eq!(ov.dims, [true; 3]);
        let ov = quadratic(ov);
        let moved = mesh_at(&ov, &pose);
        let conn = ov.connectivity(&moved, &pose).unwrap();
        let mut s = Stats::new();
        s.add(&ov, &moved, &pose, &conn, &q3, 27);
        s.print(name);
        c.at_most(&format!("P1 {name} |sum w - 1|"), s.sum, 1e-12);
        c.at_most(&format!("P4 {name} q3"), s.field, 1e-11);
        c.holds(
            &format!("P5 {name}"),
            s.inadmissible == 0 && s.wrong_donor == 0,
            "calculated cells of the other part, the donor first",
        );
        if !distort {
            c.at_most(&format!("P6 {name} fallbacks"), s.fallbacks as f64, 0.0);
        }
        // the donors against a full search, and how far outside its donor a fringe centre lies (printed)
        let nb = ov.composite.n_background_cells;
        let reference = &ov.composite.mesh;
        let cc = moved.cell_centres();
        let (mut differ, mut margin) = (0, f64::INFINITY);
        for (cell, st) in &conn.stencils {
            let (x, loc) = if *cell < nb {
                (pose_inverse_apply(&pose, cc[*cell]), ov.component_locator())
            } else {
                (cc[*cell], ov.background_locator())
            };
            differ += usize::from(loc.locate_brute_force(reference, x) != Some(st.cells[0]));
            margin = margin.min(containment_margin(reference, st.cells[0], x));
        }
        println!(
            "{name}: {} background and {} component fringe cells; donors that differ from the full search {differ}; \
             smallest containment margin {margin:.3e}",
            conn.n_fringe_background(),
            conn.n_fringe_component()
        );
    }
    c.finish();
}

// ---- P9 --------------------------------------------------------------------------------------------------------------------

/// ADR 0251 P9: where the donor's neighbourhood cannot carry a quadratic the stencil is the linear one, and the cell is
/// listed.
#[test]
fn p9_fallback_is_the_linear_stencil_and_is_listed() {
    let mut c = Checks::new();
    let (lin, pose) = fb();
    let moved = mesh_at(&lin, &pose);
    let base = lin.connectivity(&moved, &pose).unwrap();
    let ov = quadratic(lin);
    let conn = ov.connectivity(&moved, &pose).unwrap();
    let nb = ov.composite.n_background_cells;
    let n_bg = conn.fallbacks.iter().filter(|&&k| k < nb).count();
    let n_comp = conn.fallbacks.len() - n_bg;
    c.at_least("P9 (i) background fallbacks", n_bg as f64, 8.0);
    c.at_least("P9 (i) component fallbacks", n_comp as f64, 8.0);
    let pick = |v: &[(usize, Stencil)], fallback: bool| -> Vec<(usize, Stencil)> {
        v.iter()
            .filter(|(k, _)| conn.fallbacks.binary_search(k).is_ok() == fallback)
            .cloned()
            .collect()
    };
    c.holds(
        "P9 (ii)",
        conn.kind == base.kind && same_bits(&pick(&conn.stencils, true), &pick(&base.stencils, true)),
        "every fallback stencil equals the M4 stencil bit for bit",
    );
    let mut s = Stats::new();
    s.add(&ov, &moved, &pose, &conn, &q2, 9);
    s.print("FB");
    let others = conn.stencils.len() - conn.fallbacks.len();
    c.at_least("P9 (iii) stencils that are not fallbacks", others as f64, 1.0);
    c.at_most("P9 (iii) q2 over the stencils that are not fallbacks", s.field, 1e-11);
    c.holds(
        "P9 (iv)",
        conn.fallbacks.windows(2).all(|w| w[0] < w[1])
            && conn.fallbacks.iter().all(|&k| conn.kind[k] == CellKind::Fringe)
            && conn.n_fallback() == conn.fallbacks.len()
            && conn.interpolation == DonorInterpolation::Quadratic,
        "the list is sorted, of fringe cells, and counted",
    );
    c.at_least("P9 (v) q2 over the fallback stencils", s.field_fallback, 1e-5);
    let mut l = Stats::new();
    l.add(&ov, &moved, &pose, &conn, &linear_field, 9);
    c.at_most(
        "P9 a linear field over every stencil",
        l.field.max(l.field_fallback),
        1e-12,
    );
    c.finish();
}

// ---- P10 -------------------------------------------------------------------------------------------------------------------

/// ADR 0251 P10: the refusals R1 to R3 of `quadratic_weights`, and a stencil of exactly six points.
#[test]
fn p10_quadratic_weights_refusals() {
    let dims = [true, true, false];
    let p = |x: f64, y: f64| -> Vec3 { [x, y, 0.05] };
    let mut c = Checks::new();
    let five = [p(0.0, 0.0), p(1.0, 0.0), p(0.0, 1.0), p(1.0, 1.0), p(2.0, 0.0)];
    let e = quadratic_weights(&five, p(0.4, 0.3), dims).map_err(|e| e.to_string());
    println!("P10 R1: {e:?}");
    c.holds(
        "P10 R1",
        e.as_ref().is_err_and(|m| m.contains("monomials")),
        "five points refused",
    );
    let lines = [
        p(0.0, 0.0),
        p(1.0, 0.0),
        p(2.0, 0.0),
        p(0.0, 1.0),
        p(1.0, 1.0),
        p(2.0, 1.0),
    ];
    let e = quadratic_weights(&lines, p(0.6, 0.4), dims).map_err(|e| e.to_string());
    println!("P10 R2: {e:?}");
    c.holds(
        "P10 R2",
        e.as_ref().is_err_and(|m| m.contains("rank")),
        "two parallel lines refused",
    );
    let block: Vec<Vec3> = (-1..=1)
        .flat_map(|j| (-1..=1).map(move |i| [i as f64, j as f64, 0.05]))
        .collect();
    let e = quadratic_weights(&block, p(3.5, 0.0), dims).map_err(|e| e.to_string());
    println!("P10 R3: {e:?}");
    c.holds(
        "P10 R3",
        e.as_ref().is_err_and(|m| m.contains("sum |w|")),
        "a point 2.5 cells outside the block refused",
    );
    let six = [
        p(0.0, 0.0),
        p(1.0, 0.0),
        p(2.0, 0.0),
        p(0.0, 1.0),
        p(1.0, 1.0),
        p(0.0, 2.0),
    ];
    let x = p(0.6, 0.5);
    let w = quadratic_weights(&six, x, dims).unwrap();
    let fi: f64 = six.iter().zip(&w).map(|(k, w)| w * q2(*k)).sum();
    c.at_most("P10 six points, q2", (fi - q2(x)).abs(), 1e-11);
    // the block itself, at the corner of its centre cell (printed: the weights of the least-squares quadratic)
    let w = quadratic_weights(&block, p(0.5, 0.5), dims).unwrap();
    println!(
        "nine cells, the point at the corner (1/2, 1/2): weights {:?}, sum |w| {:.4}",
        w.iter().map(|v| (v * 1e4).round() / 1e4).collect::<Vec<_>>(),
        w.iter().map(|v| v.abs()).sum::<f64>()
    );
    c.finish();
}
