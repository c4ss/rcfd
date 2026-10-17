//! F-102 (ADR 0054): refinement, coarsening, field transfer and adaptation criteria. Limits are those registered in
//! docs/adr/0054-wave28c-adaptation-turbo-post.md (checks A1, A2, A3, A6) before this file was compiled or run.

use rcfd_core::vec3::{self, Vec3};
use rcfd_mesh::adapt::*;
use rcfd_mesh::gen::{axis_coords, Grading};
use rcfd_mesh::surface::box_surface;
use rcfd_mesh::{Mesh, PatchKind};
use std::collections::HashMap;

fn patches3() -> [(String, PatchKind); 6] {
    ["xmin", "xmax", "ymin", "ymax", "zmin", "zmax"].map(|n| (n.to_string(), PatchKind::Wall))
}

fn patches2() -> [(String, PatchKind); 6] {
    let w = |n: &str| (n.to_string(), PatchKind::Wall);
    [
        w("walls"),
        w("walls"),
        w("walls"),
        w("lid"),
        ("frontAndBack".into(), PatchKind::Empty),
        ("frontAndBack".into(), PatchKind::Empty),
    ]
}

fn box3(n: [usize; 3], l: [f64; 3], g: [Grading; 3], max_level: u8) -> AmrMesh {
    let c = [0, 1, 2].map(|a| axis_coords(n[a], l[a], g[a]).unwrap());
    AmrMesh::new(c, [true; 3], max_level, patches3()).unwrap()
}

fn slab(n: usize, max_level: u8) -> AmrMesh {
    let c = [
        axis_coords(n, 1.0, Grading::Uniform).unwrap(),
        axis_coords(n, 1.0, Grading::Uniform).unwrap(),
        vec![0.0, 0.1],
    ];
    AmrMesh::new(c, [true, true, false], max_level, patches2()).unwrap()
}

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

/// Independent topology / geometry checks of A1. `vol` is the exact box volume.
fn check_topology(mesh: &Mesh, vol: f64, what: &str) {
    mesh.validate().unwrap_or_else(|e| panic!("{what}: {e}"));
    // each internal face in exactly two cells' lists, each boundary face in exactly one
    let mut count = vec![0usize; mesh.n_faces()];
    for c in 0..mesh.n_cells() {
        for &f in mesh.cell_faces(c) {
            count[f] += 1;
        }
    }
    for f in 0..mesh.n_faces() {
        let want = if f < mesh.n_internal_faces() { 2 } else { 1 };
        assert_eq!(count[f], want, "{what}: face {f}");
    }
    // no two faces share the same point set
    let mut keys: HashMap<Vec<usize>, usize> = HashMap::new();
    for f in 0..mesh.n_faces() {
        let mut k = mesh.face_points(f).to_vec();
        k.sort_unstable();
        assert!(keys.insert(k, f).is_none(), "{what}: duplicate face {f}");
    }
    let total: f64 = mesh.cell_volumes().iter().sum();
    assert!((total - vol).abs() <= 1e-13 * vol, "{what}: volume {total} vs {vol}");
    for c in 0..mesh.n_cells() {
        assert!(mesh.cell_volumes()[c] > 0.0, "{what}: cell {c} volume");
        let (mut s, mut m) = (vec3::ZERO, 0.0);
        for &f in mesh.cell_faces(c) {
            let sg = if mesh.owner()[f] == c { 1.0 } else { -1.0 };
            s = vec3::add(s, vec3::scale(mesh.face_area_vectors()[f], sg));
            m += mesh.face_area(f);
        }
        assert!(
            vec3::norm(s) <= 1e-13 * m,
            "{what}: cell {c} closure {}",
            vec3::norm(s) / m
        );
    }
}

fn random_marks(n: usize, rng: &mut Lcg) -> Vec<Mark> {
    (0..n)
        .map(|_| {
            let r = rng.next();
            if r < 0.30 {
                Mark::Refine
            } else if r < 0.45 {
                Mark::Coarsen
            } else {
                Mark::Keep
            }
        })
        .collect()
}

#[test]
fn a1_topology_after_random_adaptation() {
    let mut rng = Lcg(12345);
    for (what, mut amr, vol) in [
        (
            "graded box",
            box3(
                [6, 5, 4],
                [1.3, 0.9, 0.7],
                [Grading::Expansion(3.0), Grading::Uniform, Grading::Symmetric(2.0)],
                2,
            ),
            1.3 * 0.9 * 0.7,
        ),
        ("slab", slab(10, 2), 0.1),
    ] {
        for round in 0..4 {
            let mesh = amr.to_mesh().unwrap();
            check_topology(&mesh, vol, &format!("{what} round {round} before"));
            let marks = random_marks(amr.n_cells(), &mut rng);
            let balance = round != 3;
            let (next, tr, rep) = amr.adapt(&marks, balance).unwrap();
            assert_eq!(tr.sources.len(), next.n_cells());
            let m2 = next.to_mesh().unwrap();
            check_topology(&m2, vol, &format!("{what} round {round} (balance {balance})"));
            if balance {
                let jump = max_face_level_jump(&next, &m2);
                println!(
                    "A1 {what} round {round}: {} cells, refined {} coarsened {}, jump {jump}",
                    next.n_cells(),
                    rep.refined,
                    rep.coarsened
                );
                assert!(jump <= 1, "{what} round {round}: level jump {jump}");
            }
            amr = next;
        }
    }
}

#[test]
fn a1_single_refinement_cell_counts() {
    let amr = box3([4, 4, 4], [1.0; 3], [Grading::Uniform; 3], 2);
    let mesh = amr.to_mesh().unwrap();
    // interior cell (1,1,1) of the 4x4x4 mesh
    let id = amr.leaves().iter().position(|l| l.ijk == [1, 1, 1]).unwrap();
    let mut marks = vec![Mark::Keep; 64];
    marks[id] = Mark::Refine;
    let (next, _, rep) = amr.adapt(&marks, true).unwrap();
    assert_eq!(rep.refined, 1);
    assert_eq!(next.n_cells(), 71);
    let m2 = next.to_mesh().unwrap();
    check_topology(&m2, 1.0, "single refinement");
    let nine = (0..m2.n_cells()).filter(|&c| m2.cell_faces(c).len() == 9).count();
    assert_eq!(nine, 6, "six face neighbours become polyhedra with 9 faces");
    assert_eq!(mesh.n_cells(), 64);
    let s = slab(4, 2);
    let id = s.leaves().iter().position(|l| l.ijk == [1, 1, 0]).unwrap();
    let mut marks = vec![Mark::Keep; 16];
    marks[id] = Mark::Refine;
    let (next, _, _) = s.adapt(&marks, true).unwrap();
    assert_eq!(next.n_cells(), 19);
    check_topology(&next.to_mesh().unwrap(), 0.1, "slab single refinement");
}

fn lin(p: Vec3) -> f64 {
    1.3 + 0.7 * p[0] - 0.4 * p[1] + 0.2 * p[2]
}

fn integral(mesh: &Mesh, q: &[f64]) -> f64 {
    mesh.cell_volumes().iter().zip(q).map(|(v, q)| v * q).sum()
}

#[test]
fn a2_transfer_conserves_and_is_exact_for_linear_fields() {
    let mut rng = Lcg(777);
    let amr = box3([5, 4, 3], [1.0, 1.2, 0.8], [Grading::Uniform; 3], 2);
    let mesh = amr.to_mesh().unwrap();
    let random: Vec<f64> = (0..mesh.n_cells()).map(|_| rng.next() * 3.0 - 1.0).collect();
    let linear: Vec<f64> = mesh.cell_centres().iter().map(|&p| lin(p)).collect();
    // two adaptation cycles, refine heavy then coarsen heavy
    for mode in [RefineTransfer::Injection, RefineTransfer::Linear] {
        let mut cur = amr.clone();
        let mut cur_mesh = mesh.clone();
        let mut qr = random.clone();
        let mut ql = linear.clone();
        for cycle in 0..4 {
            let marks: Vec<Mark> = (0..cur.n_cells())
                .map(|_| {
                    let r = rng.next();
                    if cycle < 2 {
                        if r < 0.5 {
                            Mark::Refine
                        } else {
                            Mark::Keep
                        }
                    } else if r < 0.9 {
                        Mark::Coarsen
                    } else {
                        Mark::Keep
                    }
                })
                .collect();
            let (next, tr, _) = cur.adapt(&marks, true).unwrap();
            let nm = next.to_mesh().unwrap();
            let nr = transfer_field(&cur_mesh, &nm, &tr, &qr, mode, [true; 3]).unwrap();
            let nl = transfer_field(&cur_mesh, &nm, &tr, &ql, mode, [true; 3]).unwrap();
            let (i0, i1) = (integral(&cur_mesh, &qr), integral(&nm, &nr));
            let scale: f64 = cur_mesh.cell_volumes().iter().zip(&qr).map(|(v, q)| v * q.abs()).sum();
            assert!(
                (i1 - i0).abs() <= 1e-13 * scale,
                "{mode:?} cycle {cycle}: integral {i0} -> {i1}"
            );
            let (l0, l1) = (integral(&cur_mesh, &ql), integral(&nm, &nl));
            assert!((l1 - l0).abs() <= 1e-13 * l0.abs(), "{mode:?} linear integral");
            if mode == RefineTransfer::Linear {
                let range = 0.7 + 0.4 + 0.2;
                let worst = (0..nm.n_cells())
                    .map(|c| (nl[c] - lin(nm.cell_centres()[c])).abs())
                    .fold(0.0, f64::max);
                println!("A2 cycle {cycle}: linear-field error {worst:.3e}");
                assert!(worst <= 1e-12 * range, "linear field error {worst}");
            }
            cur = next;
            cur_mesh = nm;
            qr = nr;
            ql = nl;
        }
    }
}

#[test]
fn a2_refine_then_coarsen_back_and_sibling_rule() {
    let amr = box3([3, 3, 3], [1.0; 3], [Grading::Uniform; 3], 2);
    let mesh = amr.to_mesh().unwrap();
    let q: Vec<f64> = mesh.cell_centres().iter().map(|&p| lin(p)).collect();
    let (fine, tr, _) = amr.adapt(&[Mark::Refine; 27], true).unwrap();
    assert_eq!(fine.n_cells(), 27 * 8);
    let fm = fine.to_mesh().unwrap();
    let qf = transfer_field(&mesh, &fm, &tr, &q, RefineTransfer::Linear, [true; 3]).unwrap();
    // coarsening only one child of a family does nothing
    let mut marks = vec![Mark::Keep; fine.n_cells()];
    marks[0] = Mark::Coarsen;
    let (same, _, rep) = fine.adapt(&marks, true).unwrap();
    assert_eq!(same.n_cells(), fine.n_cells());
    assert_eq!(rep.coarsened, 0);
    // all coarsen: back to the original
    let (back, tr2, rep) = fine.adapt(&vec![Mark::Coarsen; fine.n_cells()], true).unwrap();
    assert_eq!(back.n_cells(), 27);
    assert_eq!(rep.coarsened, 27);
    let bm = back.to_mesh().unwrap();
    let qb = transfer_field(&fm, &bm, &tr2, &qf, RefineTransfer::Linear, [true; 3]).unwrap();
    let worst = qb.iter().zip(&q).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max);
    assert!(worst <= 1e-13 * 1.3, "round trip {worst}");
    assert_eq!(bm.content_hash(), mesh.content_hash());
}

// ---------------------------------------------------------------------------------------------------------------------
// A3 criteria
// ---------------------------------------------------------------------------------------------------------------------

const AXES2: [bool; 3] = [true, true, false];

fn field(mesh: &Mesh, f: impl Fn(Vec3) -> f64) -> Vec<f64> {
    mesh.cell_centres().iter().map(|&p| f(p)).collect()
}

fn spec(refine_above: f64) -> MarkSpec {
    MarkSpec {
        refine_above,
        coarsen_below: 0.5 * refine_above,
        min_level: 0,
        max_level: 3,
    }
}

#[test]
fn a3a_gradient_tanh_front() {
    let amr = slab(40, 2);
    let mesh = amr.to_mesh().unwrap();
    let w = 0.06;
    let phi = field(&mesh, |p| ((p[0] - 0.5) / w).tanh());
    let ind = gradient_indicator(&mesh, &phi, AXES2, false).unwrap();
    let t = 2.0;
    let marks = spec(t).marks(&ind, &amr.levels()).unwrap();
    let (mut must, mut must_not) = (0, 0);
    for c in 0..mesh.n_cells() {
        let x = mesh.cell_centres()[c][0];
        let g = (1.0 - ((x - 0.5) / w).tanh().powi(2)) / w;
        if g >= 1.3 * t {
            must += 1;
            assert_eq!(marks[c], Mark::Refine, "cell {c} x={x} analytic grad {g}");
        }
        if g <= 0.7 * t {
            must_not += 1;
            assert_ne!(marks[c], Mark::Refine, "cell {c} x={x} analytic grad {g}");
        }
    }
    println!(
        "A3a: {must} cells must be marked, {must_not} must not; refined {}",
        marks.iter().filter(|m| **m == Mark::Refine).count()
    );
    assert!(must > 0 && must_not > 0);
}

#[test]
fn a3b_gradient_ramp_exact_zero() {
    let amr = slab(40, 2);
    let mesh = amr.to_mesh().unwrap();
    let h = 0.025;
    let ramp = |x: f64| ((x - 0.4) * 5.0).clamp(0.0, 1.0);
    let phi = field(&mesh, |p| ramp(p[0]));
    let ind = gradient_indicator(&mesh, &phi, AXES2, false).unwrap();
    let marks = spec(2.5).marks(&ind, &amr.levels()).unwrap();
    for c in 0..mesh.n_cells() {
        let x = mesh.cell_centres()[c][0];
        let dist = if x < 0.4 {
            0.4 - x
        } else if x > 0.6 {
            x - 0.6
        } else {
            0.0
        };
        if dist > 1.01 * h {
            assert_ne!(marks[c], Mark::Refine, "x={x}");
            assert_eq!(ind[c], 0.0, "x={x}: gradient must be exactly zero");
        }
        if x >= 0.4 + h && x <= 0.6 - h {
            assert_eq!(marks[c], Mark::Refine, "x={x}");
            assert!((ind[c] - 5.0).abs() <= 1e-10, "x={x}: {}", ind[c]);
        }
    }
}

#[test]
fn a3c_curvature_quadratic_and_gaussian() {
    let amr = slab(12, 2);
    let mesh = amr.to_mesh().unwrap();
    let (a, b, cxy) = (3.0, 2.0, 1.0);
    let phi = field(&mesh, |p| 0.5 * a * p[0] * p[0] + b * p[1] * p[1] + cxy * p[0] * p[1]);
    let ind = hessian_indicator(&mesh, &phi, AXES2, false).unwrap();
    let exact = (a * a + (2.0 * b) * (2.0 * b) + 2.0 * cxy * cxy).sqrt();
    let h = 1.0 / 12.0;
    let mut n = 0;
    for c in 0..mesh.n_cells() {
        let p = mesh.cell_centres()[c];
        let d = p[0].min(1.0 - p[0]).min(p[1]).min(1.0 - p[1]);
        if d >= 2.0 * h - 1e-12 {
            n += 1;
            assert!(
                (ind[c] - exact).abs() <= 1e-9 * exact,
                "cell {c}: {} vs {exact}",
                ind[c]
            );
        }
    }
    assert!(n > 0);
    // Gaussian
    let amr = slab(32, 2);
    let mesh = amr.to_mesh().unwrap();
    let sigma = 0.1;
    let g = |p: Vec3| {
        let r2 = (p[0] - 0.5).powi(2) + (p[1] - 0.5).powi(2);
        (-r2 / (2.0 * sigma * sigma)).exp()
    };
    let hf = |p: Vec3| {
        let (dx, dy) = (p[0] - 0.5, p[1] - 0.5);
        let s2 = sigma * sigma;
        let e = g(p);
        let hxx = e * (dx * dx / (s2 * s2) - 1.0 / s2);
        let hyy = e * (dy * dy / (s2 * s2) - 1.0 / s2);
        let hxy = e * dx * dy / (s2 * s2);
        (hxx * hxx + hyy * hyy + 2.0 * hxy * hxy).sqrt()
    };
    let phi = field(&mesh, g);
    let ind = hessian_indicator(&mesh, &phi, AXES2, false).unwrap();
    let peak = hf([0.5, 0.5, 0.0]);
    let t = 0.5 * peak;
    let marks = spec(t).marks(&ind, &amr.levels()).unwrap();
    let (mut must, mut must_not) = (0, 0);
    for c in 0..mesh.n_cells() {
        let e = hf(mesh.cell_centres()[c]);
        if e >= 1.3 * t {
            must += 1;
            assert_eq!(marks[c], Mark::Refine, "cell {c}: analytic {e}, discrete {}", ind[c]);
        }
        if e <= 0.7 * t {
            must_not += 1;
            assert_ne!(marks[c], Mark::Refine, "cell {c}: analytic {e}, discrete {}", ind[c]);
        }
    }
    println!("A3c: Gaussian {must} must / {must_not} must-not");
    assert!(must > 0 && must_not > 0);
}

#[test]
fn a3d_zz_indicator() {
    let amr = slab(40, 2);
    let mesh = amr.to_mesh().unwrap();
    let phi = field(&mesh, lin);
    let eta = zz_indicator(&mesh, &phi, AXES2).unwrap();
    assert!(
        eta.iter().all(|&e| e <= 1e-12),
        "linear field eta max {}",
        eta.iter().cloned().fold(0.0, f64::max)
    );
    let w = 0.06;
    let phi = field(&mesh, |p| ((p[0] - 0.5) / w).tanh());
    let eta = zz_indicator(&mesh, &phi, AXES2).unwrap();
    let h = 0.025;
    let (imax, emax) = eta
        .iter()
        .cloned()
        .enumerate()
        .fold((0, 0.0), |a, (i, e)| if e > a.1 { (i, e) } else { a });
    let xmax = mesh.cell_centres()[imax][0];
    let far = (0..mesh.n_cells())
        .filter(|&c| (mesh.cell_centres()[c][0] - 0.5).abs() > 6.0 * w)
        .map(|c| eta[c])
        .fold(0.0, f64::max);
    println!("A3d: eta max {emax:.3e} at x = {xmax}, far-field max {far:.3e}");
    assert!((xmax - 0.5).abs() <= 2.0 * h, "max at {xmax}");
    assert!(emax >= 100.0 * far, "ratio {}", emax / far);
    for c in 0..mesh.n_cells() {
        if eta[c] >= 0.5 * emax {
            assert!((mesh.cell_centres()[c][0] - 0.5).abs() <= 3.0 * w + 1e-12, "cell {c}");
        }
    }
}

#[test]
fn a3e_region_indicators() {
    let amr = box3([10, 10, 10], [1.0; 3], [Grading::Uniform; 3], 2);
    let mesh = amr.to_mesh().unwrap();
    let cc = mesh.cell_centres();
    let lo = [0.2, 0.3, 0.1];
    let hi = [0.6, 0.8, 0.5];
    let ind = region_indicator(&mesh, &RegionShape::Box { lo, hi }).unwrap();
    for c in 0..mesh.n_cells() {
        let inside = (0..3).all(|a| cc[c][a] >= lo[a] && cc[c][a] <= hi[a]);
        assert_eq!(ind[c] == 1.0, inside);
    }
    assert!(ind.contains(&1.0));
    let ind = region_indicator(
        &mesh,
        &RegionShape::Sphere {
            centre: [0.5; 3],
            radius: 0.3,
        },
    )
    .unwrap();
    for c in 0..mesh.n_cells() {
        assert_eq!(ind[c] == 1.0, vec3::norm(vec3::sub(cc[c], [0.5; 3])) <= 0.3);
    }
    let ind = region_indicator(
        &mesh,
        &RegionShape::Slab {
            point: [0.5, 0.0, 0.0],
            normal: [2.0, 0.0, 0.0],
            half_thickness: 0.15,
        },
    )
    .unwrap();
    for c in 0..mesh.n_cells() {
        assert_eq!(ind[c] == 1.0, (cc[c][0] - 0.5).abs() <= 0.15);
    }
    let s = box_surface([0.0; 3], [1.0; 3], [2, 2, 2]);
    let idx = rcfd_mesh::octree::SurfaceIndex::new(&s);
    let d = 0.2;
    let ind = region_indicator(&mesh, &RegionShape::SurfaceDistance { index: &idx, cutoff: d }).unwrap();
    let mut n = 0;
    for c in 0..mesh.n_cells() {
        let p = cc[c];
        let dist = p[0].min(1.0 - p[0]).min(p[1]).min(1.0 - p[1]).min(p[2]).min(1.0 - p[2]);
        // cell centres are at odd multiples of 0.05, never within round-off of 0.2
        assert_eq!(ind[c] == 1.0, dist <= d, "cell {c} dist {dist}");
        n += (ind[c] == 1.0) as usize;
    }
    assert!(n > 0 && n < mesh.n_cells());
}

#[test]
fn a3f_a3g_a3h_limiter_hysteresis_combine() {
    let s = MarkSpec {
        refine_above: 1.0,
        coarsen_below: 0.5,
        min_level: 1,
        max_level: 2,
    };
    // level limiter
    let m = s.marks(&[2.0, 2.0, 0.0, 0.0], &[2, 1, 1, 2]).unwrap();
    assert_eq!(m, vec![Mark::Keep, Mark::Refine, Mark::Keep, Mark::Coarsen]);
    // hysteresis: 1.05 T refine, then 0.95 T and 0.6 T keep, 0.4 T coarsen
    let m = s.marks(&[1.05, 0.95, 0.6, 0.4], &[1, 1, 1, 2]).unwrap();
    assert_eq!(m, vec![Mark::Refine, Mark::Keep, Mark::Keep, Mark::Coarsen]);
    // validation with located errors
    for (bad, name) in [
        (
            MarkSpec {
                coarsen_below: 1.0,
                ..s
            },
            "coarsen_below",
        ),
        (
            MarkSpec {
                coarsen_below: 2.0,
                ..s
            },
            "coarsen_below",
        ),
        (
            MarkSpec {
                refine_above: f64::NAN,
                ..s
            },
            "refine_above",
        ),
        (
            MarkSpec {
                refine_above: -1.0,
                ..s
            },
            "refine_above",
        ),
        (
            MarkSpec {
                coarsen_below: -0.1,
                ..s
            },
            "coarsen_below",
        ),
        (MarkSpec { min_level: 3, ..s }, "min_level"),
    ] {
        let e = bad.marks(&[1.0], &[1]).unwrap_err().to_string();
        assert!(e.contains(name), "{e}");
    }
    assert!(s.marks(&[1.0], &[1, 1]).is_err());
    assert!(s.marks(&[f64::NAN], &[1]).is_err());
    // combine
    let c = combine_marks(&[
        &[Mark::Refine, Mark::Coarsen, Mark::Coarsen, Mark::Keep],
        &[Mark::Keep, Mark::Coarsen, Mark::Keep, Mark::Keep],
    ])
    .unwrap();
    assert_eq!(c, vec![Mark::Refine, Mark::Coarsen, Mark::Keep, Mark::Keep]);
    assert!(combine_marks(&[]).is_err());
    assert!(combine_marks(&[&[Mark::Keep], &[Mark::Keep, Mark::Keep]]).is_err());
}

#[test]
fn a6_located_errors() {
    let ok = |c: Vec<f64>| c;
    let new = |c: [Vec<f64>; 3], ml: u8| AmrMesh::new(c, [true; 3], ml, patches3()).map(|_| ());
    let u = vec![0.0, 1.0];
    assert!(new([u.clone(), u.clone(), u.clone()], 2).is_ok());
    let e = new([ok(vec![0.0]), u.clone(), u.clone()], 2).unwrap_err().to_string();
    assert!(e.contains("coords[0]"), "{e}");
    let e = new([u.clone(), vec![0.0, 0.5, 0.5], u.clone()], 2)
        .unwrap_err()
        .to_string();
    assert!(e.contains("coords[1]"), "{e}");
    let e = new([u.clone(), u.clone(), vec![0.0, f64::NAN]], 2)
        .unwrap_err()
        .to_string();
    assert!(e.contains("coords[2]"), "{e}");
    let e = new([u.clone(), u.clone(), u.clone()], 11).unwrap_err().to_string();
    assert!(e.contains("max_level"), "{e}");
    let mut p = patches3();
    p[1].0 = "xmin".into();
    p[1].1 = PatchKind::Inlet;
    assert!(AmrMesh::new([u.clone(), u.clone(), u.clone()], [true; 3], 1, p).is_err());
    let amr = slab(4, 2);
    let e = amr.adapt(&[Mark::Keep], true).unwrap_err().to_string();
    assert!(e.contains("marks"), "{e}");
    let mesh = amr.to_mesh().unwrap();
    assert!(lsq_gradient(&mesh, &[0.0], AXES2).is_err());
    let mut f = vec![0.0; mesh.n_cells()];
    f[3] = f64::INFINITY;
    assert!(gradient_indicator(&mesh, &f, AXES2, false).is_err());
    assert!(region_indicator(
        &mesh,
        &RegionShape::Sphere {
            centre: [0.0; 3],
            radius: -1.0
        }
    )
    .is_err());
    assert!(region_indicator(
        &mesh,
        &RegionShape::Box {
            lo: [1.0; 3],
            hi: [0.0; 3]
        }
    )
    .is_err());
    let (n2, tr, _) = amr.adapt(&[Mark::Refine; 16], true).unwrap();
    let m2 = n2.to_mesh().unwrap();
    let e = transfer_field(&mesh, &m2, &tr, &[0.0], RefineTransfer::Injection, AXES2)
        .unwrap_err()
        .to_string();
    assert!(e.contains("transfer_field"), "{e}");
    // marks at the finest level are ignored and counted
    let (_, _, rep) = n2.adapt(&vec![Mark::Refine; n2.n_cells()], true).unwrap();
    assert_eq!(rep.ignored_at_max_level, 0);
    let (n3, _, _) = n2.adapt(&vec![Mark::Refine; n2.n_cells()], true).unwrap();
    let (_, _, rep) = n3.adapt(&vec![Mark::Refine; n3.n_cells()], true).unwrap();
    assert_eq!(rep.ignored_at_max_level, n3.n_cells());
}

#[test]
fn a1_family_coarsening_with_balance() {
    let mut rng = Lcg(2024);
    let mut amr = box3([4, 4, 3], [1.0, 1.0, 0.75], [Grading::Uniform; 3], 3);
    let mut total_coarsened = 0;
    for round in 0..8 {
        // refine a random 40 percent in odd rounds, coarsen random whole families in even rounds
        let marks: Vec<Mark> = if round % 2 == 0 {
            random_marks(amr.n_cells(), &mut rng)
                .into_iter()
                .map(|m| if m == Mark::Coarsen { Mark::Keep } else { m })
                .collect()
        } else {
            let mut pick: HashMap<(u8, [i64; 3]), bool> = HashMap::new();
            amr.leaves()
                .iter()
                .map(|l| {
                    let key = (l.level, [l.ijk[0] >> 1, l.ijk[1] >> 1, l.ijk[2] >> 1]);
                    let take = *pick.entry(key).or_insert_with(|| rng.next() < 0.7);
                    if take && l.level > 0 {
                        Mark::Coarsen
                    } else {
                        Mark::Keep
                    }
                })
                .collect()
        };
        let (next, tr, rep) = amr.adapt(&marks, true).unwrap();
        total_coarsened += rep.coarsened;
        let m = next.to_mesh().unwrap();
        check_topology(&m, 0.75, &format!("family round {round}"));
        assert!(max_face_level_jump(&next, &m) <= 1, "round {round}");
        let merged = tr.sources.iter().filter(|s| matches!(s, Source::Merged(_))).count();
        assert_eq!(merged, rep.coarsened);
        println!(
            "A1 family round {round}: {} cells, refined {}, coarsened {}",
            next.n_cells(),
            rep.refined,
            rep.coarsened
        );
        amr = next;
    }
    assert!(total_coarsened > 0, "no merge was exercised");
}
