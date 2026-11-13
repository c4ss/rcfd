//! F-468 (ADR 0396): the pose-continuous donor interpolation of the overset coupling, `DonorInterpolation::Continuous`.
//! The kinematic checks (no flow solve): exact for linear fields on TG_2 and on TR (A2a, A2b), continuous across every face
//! of a uniform, a graded and a curved donor mesh where the library's linear stencil jumps (A3, A3b), continuous in the pose
//! at the moments the linear stencil's donor switches (A4), continuous where a cell enters its support (A5), and its
//! candidates equal to a search over every cell (A6). The flow checks are in `crates/rcfd-fv/tests/overset_f468.rs`.
//!
//! Release: `cargo test --release -p rcfd-mesh --test overset_f468 -- --nocapture`.

use rcfd_core::vec3::{self, Vec3};
use rcfd_mesh::gen::{axis_coords, cylinder_ogrid, expansion_for_end_cell, rectilinear_2d, Grading};
use rcfd_mesh::motion::{moved_mesh, Pose};
use rcfd_mesh::overset::{
    compose, linear_weights, pose_inverse_apply, CellKind, DonorInterpolation, Overset, OversetSpec, Stencil,
    CONTINUOUS_SUPPORT,
};
use std::f64::consts::PI;
use std::sync::Arc;

const DEPTH: f64 = 0.1;

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
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:?}", self.0);
    }
}

/// A fixed linear congruential sequence in [0, 1).
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

fn uniform(x0: f64, len: f64, n: usize) -> Vec<f64> {
    (0..=n).map(|i| x0 + len * i as f64 / n as f64).collect()
}

// ---- the set-ups --------------------------------------------------------------------------------------------------

fn tg_pose(t: f64) -> Pose {
    Pose {
        origin: [0.0; 3],
        axis: [0.0, 0.0, 1.0],
        angle: t,
        translation: [PI / 2.0 - 0.3 + 0.5 * t, PI / 2.0 - 0.2 + 0.3 * t, 0.0],
    }
}

/// TG_2 of ADR 0359: the 80 by 80 background of `[0, pi]^2`, the 40 by 40 component square of half side 0.6, the hole.
fn tg2() -> Overset {
    let bs = uniform(0.0, PI, 80);
    let bg = rectilinear_2d(&bs, &bs, DEPTH).unwrap();
    let cs = uniform(-0.6, 1.2, 40);
    let comp = rectilinear_2d(&cs, &cs, DEPTH).unwrap();
    let composite = compose(&bg, &comp, "c_").unwrap();
    let spec = OversetSpec {
        outer_patches: ["c_xmin", "c_xmax", "c_ymin", "c_ymax"].map(String::from).to_vec(),
        hole: Arc::new(|x: Vec3| x[0].abs().max(x[1].abs()) < 0.25),
    };
    Overset::new(composite, spec).unwrap()
}

/// Nodes `x0 + axis_coords(n, len)` with the cell at the far end (`toward_end`) or at `x0` of size `h`.
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

/// ADR 0219 TR (the set-up of the mesh crate's unit tests): the graded background and the O-grid component.
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
    let composite = compose(&bg, &comp, "c_").unwrap();
    let spec = OversetSpec {
        outer_patches: vec!["c_farfield".into()],
        hole: Arc::new(|x: Vec3| x[0] * x[0] + x[1] * x[1] < 1.0),
    };
    Overset::new(composite, spec).unwrap()
}

fn continuous(mut ov: Overset) -> Overset {
    ov.set_donor_interpolation(DonorInterpolation::Continuous);
    ov
}

/// The library's linear stencil (ADR 0219 M4) at `x` of the part `from_background`: the located donor and its calculated
/// face neighbours with `linear_weights`. `None` where there is no calculated donor.
fn linear_stencil(ov: &Overset, kind: &[CellKind], x: Vec3, from_background: bool) -> Option<Stencil> {
    let mesh = &ov.composite.mesh;
    let loc = if from_background {
        ov.background_locator()
    } else {
        ov.component_locator()
    };
    let d = loc.locate(mesh, x)?;
    if kind[d] != CellKind::Calculated {
        return None;
    }
    let mut cells = vec![d];
    cells.extend(
        ov.neighbours(d)
            .iter()
            .copied()
            .filter(|&q| kind[q] == CellKind::Calculated),
    );
    let pts: Vec<Vec3> = cells.iter().map(|&q| mesh.cell_centres()[q]).collect();
    let weights = linear_weights(&pts, x, ov.dims).ok()?;
    Some(Stencil { cells, weights })
}

/// `sum w_k f(x_k)` over the reference centres.
fn value(ov: &Overset, st: &Stencil, f: impl Fn(Vec3) -> f64) -> f64 {
    let cc = ov.composite.mesh.cell_centres();
    st.cells.iter().zip(&st.weights).map(|(&k, &w)| w * f(cc[k])).sum()
}

/// The quadratic of A3 (constant Hessian with a mixed term).
fn quad(x: Vec3) -> f64 {
    x[0] * x[0] + 3.0 * x[0] * x[1] - x[1] * x[1]
}

/// ADR 0396 jump estimator: `2 D(delta) - D(2 delta)` with `D(s) = I(x + s n) - I(x - s n)`; `None` where `I` is not
/// defined at one of the four points.
fn jump(i: impl Fn(Vec3) -> Option<f64>, x: Vec3, n: Vec3, delta: f64) -> Option<f64> {
    let at = |s: f64| i(vec3::add(x, vec3::scale(n, s)));
    let d1 = at(delta)? - at(-delta)?;
    let d2 = at(2.0 * delta)? - at(-2.0 * delta)?;
    Some((2.0 * d1 - d2).abs())
}

// ---- A2: linear fields ---------------------------------------------------------------------------------------------

/// The largest error of a linear field over the fringe cells (absolute or relative to `max |f|`), the largest
/// `|sum w - 1|`, the number of fallbacks, the weight range and the stencil sizes.
fn linear_errors(
    ov: &Overset,
    pose: &Pose,
    f: impl Fn(Vec3) -> f64,
    relative: bool,
) -> (f64, f64, usize, [f64; 2], [usize; 2]) {
    let moved = moved_mesh(&ov.composite.mesh, ov.composite.points_at(pose)).unwrap();
    let conn = ov.connectivity(&moved, pose).unwrap();
    assert_eq!(conn.interpolation, DonorInterpolation::Continuous);
    let cc = moved.cell_centres();
    let scale = if relative {
        cc.iter().map(|&x| f(x).abs()).fold(0.0, f64::max)
    } else {
        1.0
    };
    let (mut e, mut s, mut wr, mut nr) = (0.0f64, 0.0f64, [f64::INFINITY, f64::NEG_INFINITY], [usize::MAX, 0]);
    for (c, st) in &conn.stencils {
        let v: f64 = st.cells.iter().zip(&st.weights).map(|(&k, &w)| w * f(cc[k])).sum();
        e = e.max((v - f(cc[*c])).abs() / scale);
        s = s.max((st.weights.iter().sum::<f64>() - 1.0).abs());
        for &w in &st.weights {
            wr = [wr[0].min(w), wr[1].max(w)];
        }
        nr = [nr[0].min(st.cells.len()), nr[1].max(st.cells.len())];
        for &k in &st.cells {
            assert_eq!(conn.kind[k], CellKind::Calculated, "stencil cell {k} of {c}");
            assert_ne!(
                k < conn.n_background_cells,
                *c < conn.n_background_cells,
                "stencil cell {k} of {c}"
            );
        }
    }
    (e, s, conn.n_fallback(), wr, nr)
}

#[test]
fn a2a_tg2_linear_fields_are_exact() {
    let ov = continuous(tg2());
    let f = |x: Vec3| 1.0 + 2.0 * x[0] - 3.0 * x[1];
    let mut c = Checks::new();
    for t in [0.0, 0.3, 0.5] {
        let (e, s, nf, wr, nr) = linear_errors(&ov, &tg_pose(t), f, false);
        println!(
            "A2a tg_pose({t}): weights in [{:.3e}, {:.4}], {} to {} cells",
            wr[0], wr[1], nr[0], nr[1]
        );
        c.at_most(&format!("A2a linear field at tg_pose({t})"), e, 1e-12);
        c.at_most(&format!("A2a |sum w - 1| at tg_pose({t})"), s, 1e-13);
        c.at_most(&format!("A2a fallbacks at tg_pose({t})"), nf as f64, 0.0);
    }
    c.finish();
}

#[test]
fn a2b_tr_linear_fields_are_exact() {
    let ov = continuous(tr());
    let f = |x: Vec3| 3.0 + 2.0 * x[0] - 5.0 * x[1];
    let mut c = Checks::new();
    for s in [0.0, -0.37, -1.234, -3.9, -7.5] {
        let pose = Pose::translation([2.0 + s, 0.0, 0.0]);
        let (e, sw, nf, wr, nr) = linear_errors(&ov, &pose, f, true);
        println!(
            "A2b x = {}: weights in [{:.3e}, {:.4}], {} to {} cells",
            2.0 + s,
            wr[0],
            wr[1],
            nr[0],
            nr[1]
        );
        c.at_most(&format!("A2b linear field at x = {}", 2.0 + s), e, 1e-12);
        c.at_most(&format!("A2b |sum w - 1| at x = {}", 2.0 + s), sw, 1e-13);
        c.at_most(&format!("A2b fallbacks at x = {}", 2.0 + s), nf as f64, 0.0);
    }
    c.finish();
}

// ---- A3: across the faces ------------------------------------------------------------------------------------------

/// Over the internal faces `faces` of the part `from_background` (every cell calculated): the largest jump of the
/// continuous interpolant, the jumps of the linear stencil, the number of points without a continuous stencil, at the
/// point `x_f + 0.25 h t` (`h` the owner's largest extent), `delta = 1e-6 h`.
fn face_jumps(ov: &Overset, faces: std::ops::Range<usize>, from_background: bool) -> (f64, Vec<f64>, usize) {
    let mesh = &ov.composite.mesh;
    let kind = vec![CellKind::Calculated; mesh.n_cells()];
    let (mut jc, mut jl, mut missing) = (0.0f64, Vec::new(), 0);
    for f in faces {
        let h = ov.support_radius(mesh.owner()[f]).unwrap() / CONTINUOUS_SUPPORT;
        let n = vec3::unit(mesh.face_area_vectors()[f]);
        let t = [-n[1], n[0], 0.0];
        let x = vec3::add(mesh.face_centres()[f], vec3::scale(t, 0.25 * h));
        let delta = 1e-6 * h;
        let ic = |p: Vec3| {
            ov.continuous_stencil(&kind, p, from_background)
                .ok()
                .map(|s| value(ov, &s, quad))
        };
        match jump(ic, x, n, delta) {
            Some(j) => jc = jc.max(j),
            None => missing += 1,
        }
        let il = |p: Vec3| linear_stencil(ov, &kind, p, from_background).map(|s| value(ov, &s, quad));
        jl.push(jump(il, x, n, delta).unwrap_or(f64::NAN));
    }
    (jc, jl, missing)
}

#[test]
fn a3_uniform_mesh_faces() {
    let ov = continuous(tg2());
    let nf = 2 * 80 * 79;
    let (jc, jl, missing) = face_jumps(&ov, 0..nf, true);
    let big = jl.iter().filter(|&&j| j >= 1e-4).count();
    let jmin = jl.iter().copied().fold(f64::INFINITY, f64::min);
    let jmax = jl.iter().copied().fold(0.0, f64::max);
    println!("A3: {nf} faces; linear jumps in [{jmin:.4e}, {jmax:.4e}], {big} at least 1e-4");
    let mut c = Checks::new();
    c.at_most("A3 largest jump of the continuous interpolant", jc, 1e-9);
    c.at_least(
        "A3 fraction of the faces where the linear stencil jumps by 1e-4",
        big as f64 / nf as f64,
        0.9,
    );
    c.at_most("A3 points without a continuous stencil", missing as f64, 0.0);
    c.finish();
}

#[test]
fn a3b_graded_and_curved_mesh_faces() {
    let ov = continuous(tr());
    let bg_faces = 185 * 109 + 184 * 110;
    let comp_faces = 96 * 29 + 96 * 30;
    assert_eq!(ov.composite.mesh.n_internal_faces(), bg_faces + comp_faces);
    let mut c = Checks::new();
    for (tag, faces, from_bg) in [
        ("background (graded)", 0..bg_faces, true),
        ("component (O-grid)", bg_faces..bg_faces + comp_faces, false),
    ] {
        let n = faces.len();
        let (jc, jl, missing) = face_jumps(&ov, faces, from_bg);
        let big = jl.iter().filter(|&&j| j >= 1e-4).count();
        let jmax = jl.iter().copied().fold(0.0, f64::max);
        println!("A3b {tag}: {n} faces; linear jumps up to {jmax:.4e}, {big} at least 1e-4");
        c.at_most(
            &format!("A3b {tag} largest jump of the continuous interpolant"),
            jc,
            1e-9,
        );
        c.at_most(
            &format!("A3b {tag} points without a continuous stencil"),
            missing as f64,
            0.0,
        );
    }
    c.finish();
}

// ---- A4: in the pose -----------------------------------------------------------------------------------------------

#[test]
fn a4_tg2_pose_continuity_at_the_donor_switches() {
    let lin = tg2();
    let ov = continuous(tg2());
    let mesh = &ov.composite.mesh;
    let nb = ov.composite.n_background_cells;
    let rc = mesh.cell_centres();
    let field = |p: Vec3| (2.0 * p[0] + 0.3).sin() * (3.0 * p[1] - 0.2).cos();
    // the fringe point in the frame of the part it is interpolated from, and whether that part is the background
    let point = |c: usize, t: f64| -> (Vec3, bool) {
        let pose = tg_pose(t);
        if c >= nb {
            (pose.apply(rc[c]), true)
        } else {
            (pose_inverse_apply(&pose, rc[c]), false)
        }
    };
    let donor = |c: usize, t: f64| {
        let (x, bg) = point(c, t);
        let loc = if bg {
            ov.background_locator()
        } else {
            ov.component_locator()
        };
        loc.locate(mesh, x)
    };
    let kinds = |t: f64| {
        let pose = tg_pose(t);
        let moved = moved_mesh(mesh, ov.composite.points_at(&pose)).unwrap();
        lin.connectivity(&moved, &pose).unwrap().kind
    };
    let mut events = Vec::new();
    let mut k0 = kinds(0.0);
    'outer: for j in 0..100 {
        let (ta, tb) = (0.005 * j as f64, 0.005 * (j + 1) as f64);
        let k1 = kinds(tb);
        if k0 == k1 {
            for c in 0..k0.len() {
                if k0[c] != CellKind::Fringe || donor(c, ta) == donor(c, tb) {
                    continue;
                }
                let (mut a, mut b) = (ta, tb);
                let da = donor(c, ta);
                for _ in 0..40 {
                    let m = 0.5 * (a + b);
                    if donor(c, m) == da {
                        a = m;
                    } else {
                        b = m;
                    }
                }
                if kinds(a) != k0 || kinds(b) != k0 {
                    println!("A4: event at cell {c} in ({ta}, {tb}) skipped: the kinds change in the bracket");
                    continue;
                }
                events.push((c, a, b));
                if events.len() == 12 {
                    break 'outer;
                }
            }
        }
        k0 = k1;
    }
    assert_eq!(events.len(), 12, "A4: fewer than 12 events (defective set-up)");
    let mut c = Checks::new();
    let mut big = 0;
    for (cell, a, b) in events {
        let (xa, bg) = point(cell, a);
        let (xb, _) = point(cell, b);
        let kind_ab = kinds(a);
        let ic = |x: Vec3| value(&ov, &ov.continuous_stencil(&kind_ab, x, bg).unwrap(), field);
        let il = |x: Vec3| value(&ov, &linear_stencil(&ov, &kind_ab, x, bg).unwrap(), field);
        let (jc, jl) = ((ic(xb) - ic(xa)).abs(), (il(xb) - il(xa)).abs());
        if jl >= 1e-5 {
            big += 1;
        }
        println!(
            "A4 cell {cell} ({}) at t = {a:.15} (bracket {:.1e}): continuous jump {jc:.3e}, linear jump {jl:.3e}",
            if cell >= nb { "component" } else { "background" },
            b - a
        );
        c.at_most(&format!("A4 continuous jump at cell {cell}"), jc, 1e-9);
    }
    c.at_least("A4 events where the linear stencil jumps by 1e-5", big as f64, 6.0);
    c.finish();
}

// ---- A5: where a cell enters its support ---------------------------------------------------------------------------

#[test]
fn a5_support_boundary() {
    let ov = continuous(tg2());
    let mesh = &ov.composite.mesh;
    let kind = vec![CellKind::Calculated; mesh.n_cells()];
    let h = PI / 80.0;
    let inner: Vec<usize> = (0..ov.composite.n_background_cells)
        .filter(|&k| {
            let x = mesh.cell_centres()[k];
            x[0] > 3.0 * h && x[0] < PI - 3.0 * h && x[1] > 3.0 * h && x[1] < PI - 3.0 * h
        })
        .collect();
    let mut g = Lcg(0x0396_f468);
    let mut jc = 0.0f64;
    for _ in 0..400 {
        let k = inner[((g.next() * inner.len() as f64) as usize).min(inner.len() - 1)];
        let a = 2.0 * PI * g.next();
        let e = [a.cos(), a.sin(), 0.0];
        let r = ov.support_radius(k).unwrap();
        let x = vec3::add(mesh.cell_centres()[k], vec3::scale(e, r));
        let ic = |p: Vec3| ov.continuous_stencil(&kind, p, true).ok().map(|s| value(&ov, &s, quad));
        jc = jc.max(jump(ic, x, e, 1e-6 * r / CONTINUOUS_SUPPORT).expect("A5: no stencil (defective set-up)"));
    }
    let mut c = Checks::new();
    c.at_most("A5 largest jump at the edge of a support", jc, 1e-9);
    c.finish();
}

// ---- A6: the candidate search --------------------------------------------------------------------------------------

/// 2000 points in the box of the centres of the part `from_background`: the candidates against a search over every cell.
fn search_differences(ov: &Overset, kind: &[CellKind], from_background: bool, seed: u64) -> (usize, usize) {
    let mesh = &ov.composite.mesh;
    let cells = if from_background {
        ov.composite.background_cells()
    } else {
        ov.composite.component_cells()
    };
    let cc = mesh.cell_centres();
    let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
    for c in cells.clone() {
        for d in 0..3 {
            lo[d] = lo[d].min(cc[c][d]);
            hi[d] = hi[d].max(cc[c][d]);
        }
    }
    let mut g = Lcg(seed);
    let (mut diff, mut refused) = (0, 0);
    for _ in 0..2000 {
        let x = [
            lo[0] + (hi[0] - lo[0]) * g.next(),
            lo[1] + (hi[1] - lo[1]) * g.next(),
            0.5 * (lo[2] + hi[2]),
        ];
        let brute: Vec<usize> = cells
            .clone()
            .filter(|&k| {
                let r = ov.support_radius(k).unwrap();
                let d2: f64 = (0..3).filter(|&d| ov.dims[d]).map(|d| (cc[k][d] - x[d]).powi(2)).sum();
                kind[k] == CellKind::Calculated && d2.sqrt() / (r / CONTINUOUS_SUPPORT) < CONTINUOUS_SUPPORT
            })
            .collect();
        match ov.continuous_stencil(kind, x, from_background) {
            Ok(st) => {
                if st.cells != brute {
                    diff += 1;
                }
            }
            Err(_) => refused += 1,
        }
    }
    (diff, refused)
}

#[test]
fn a6_candidates_equal_a_full_search() {
    let mut c = Checks::new();
    let tg = continuous(tg2());
    let pose = tg_pose(0.3);
    let moved = moved_mesh(&tg.composite.mesh, tg.composite.points_at(&pose)).unwrap();
    let mut lin = tg2();
    lin.set_donor_interpolation(DonorInterpolation::Linear);
    let tg_kind = lin.connectivity(&moved, &pose).unwrap().kind;
    let trc = continuous(tr());
    let pose = Pose::translation([2.0, 0.0, 0.0]);
    let moved = moved_mesh(&trc.composite.mesh, trc.composite.points_at(&pose)).unwrap();
    let tr_kind = trc.connectivity(&moved, &pose).unwrap().kind;
    for (tag, ov, kind, bg, seed) in [
        ("TG_2 background", &tg, &tg_kind, true, 1u64),
        ("TG_2 component", &tg, &tg_kind, false, 2),
        ("TR background", &trc, &tr_kind, true, 3),
        ("TR component", &trc, &tr_kind, false, 4),
    ] {
        let (diff, refused) = search_differences(ov, kind, bg, seed);
        println!("A6 {tag}: 2000 points, {refused} without a plane (printed)");
        c.at_most(&format!("A6 {tag} differences from the full search"), diff as f64, 0.0);
    }
    c.finish();
}
