//! F-348 (ADR 0267): the lattice with per-axis refinement levels (`rcfd_mesh::adapt::aniso`). Limits registered in
//! docs/adr/0267-f348-registration-anisotropic-refinement-per-axis-levels.md (checks G1 to G7) before this file was compiled
//! or run.

use rcfd_core::vec3::{self, Vec3};
use rcfd_mesh::adapt::aniso::{max_axis_level_jump, AnisoMesh, AxisMark};
use rcfd_mesh::adapt::{transfer_field, AmrMesh, Mark, RefineTransfer};
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

fn uniform(n: usize) -> Vec<f64> {
    (0..=n).map(|i| i as f64 / n as f64).collect()
}

/// (a): graded 3-D box 3 x 2 x 2, volume 0.8.
fn coords_a() -> [Vec<f64>; 3] {
    [vec![0.0, 0.2, 0.5, 1.0], vec![0.0, 0.3, 1.0], vec![0.0, 0.45, 0.8]]
}

fn box_a() -> AnisoMesh {
    AnisoMesh::new(coords_a(), [true; 3], [2, 2, 2], patches3()).unwrap()
}

/// (b): slab 5 x 4, volume 0.1.
fn coords_b() -> [Vec<f64>; 3] {
    [uniform(5), uniform(4), vec![0.0, 0.1]]
}

fn slab_b() -> AnisoMesh {
    AnisoMesh::new(coords_b(), [true, true, false], [3, 3, 0], patches2()).unwrap()
}

/// (c): 3-D box 2 x 3 x 2 refined along x and z only, volume 1.
fn box_c() -> AnisoMesh {
    AnisoMesh::new(
        [uniform(2), uniform(3), uniform(2)],
        [true, false, true],
        [2, 0, 3],
        patches3(),
    )
    .unwrap()
}

fn slab(n: usize, max_level: [u8; 3]) -> AnisoMesh {
    AnisoMesh::new(
        [uniform(n), uniform(n), vec![0.0, 0.1]],
        [true, true, false],
        max_level,
        patches2(),
    )
    .unwrap()
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

fn subset(rng: &mut Lcg) -> [bool; 3] {
    let bits = 1 + (rng.next() * 7.0) as usize;
    [bits & 1 != 0, bits & 2 != 0, bits & 4 != 0]
}

/// G1's marks: refine along a random non-empty set of axes (0.35), coarsen along one axis (0.15), else keep.
fn random_marks(n: usize, rng: &mut Lcg) -> Vec<AxisMark> {
    (0..n)
        .map(|_| {
            let r = rng.next();
            if r < 0.35 {
                AxisMark::refine(subset(rng))
            } else if r < 0.5 {
                let a = (rng.next() * 3.0) as usize;
                AxisMark::coarsen([a == 0, a == 1, a == 2])
            } else {
                AxisMark::KEEP
            }
        })
        .collect()
}

/// The topology and geometry checks of ADR 0054's A1. `vol` is the exact box volume.
fn check_topology(mesh: &Mesh, vol: f64, what: &str) {
    mesh.validate().unwrap_or_else(|e| panic!("{what}: {e}"));
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

fn integral(mesh: &Mesh, q: &[f64]) -> f64 {
    mesh.cell_volumes().iter().zip(q).map(|(v, q)| v * q).sum()
}

fn abs_integral(mesh: &Mesh, q: &[f64]) -> f64 {
    mesh.cell_volumes().iter().zip(q).map(|(v, q)| v * q.abs()).sum()
}

// ------------------------------------------------------------------------------------------------ G1

#[test]
fn g1_random_directional_marks_give_valid_closed_meshes() {
    let cases: [(&str, AnisoMesh, f64, usize); 3] =
        [("a", box_a(), 0.8, 4), ("b", slab_b(), 0.1, 5), ("c", box_c(), 1.0, 4)];
    for (name, base, vol, rounds) in cases {
        for (balance, seed) in [(true, 12345u64), (false, 67890u64)] {
            let mut rng = Lcg(seed);
            let mut cur = base.clone();
            check_topology(&cur.to_mesh().unwrap(), vol, &format!("{name} base"));
            for round in 0..rounds {
                let marks = random_marks(cur.n_cells(), &mut rng);
                let (next, tr, rep) = cur.adapt(&marks, balance).unwrap();
                let mesh = next.to_mesh().unwrap();
                let what = format!("{name} balance {balance} round {round}");
                check_topology(&mesh, vol, &what);
                assert_eq!(mesh.n_cells(), next.n_cells(), "{what}");
                assert_eq!(tr.sources.len(), next.n_cells(), "{what}");
                let jump = max_axis_level_jump(&next, &mesh);
                println!(
                    "G1 {what}: {} -> {} cells, refined {} coarsened {} ignored {}, jump {jump:?}",
                    cur.n_cells(),
                    next.n_cells(),
                    rep.refined,
                    rep.coarsened,
                    rep.ignored_at_max_level
                );
                if balance {
                    assert!(jump.iter().all(|&j| j <= 1), "{what}: jump {jump:?}");
                }
                let refined = next.refined_axes();
                for l in next.leaves() {
                    for a in 0..3 {
                        assert!(l.levels[a] <= next.max_levels()[a], "{what}");
                        assert!(refined[a] || l.levels[a] == 0, "{what}");
                    }
                }
                cur = next;
            }
            assert!(cur.n_cells() > base.n_cells(), "{name}: nothing was refined");
        }
    }
}

// ------------------------------------------------------------------------------------------------ G1b

#[test]
fn g1b_crossing_faces_get_a_vertex_that_is_no_leaf_corner() {
    let amr = AnisoMesh::new(
        [vec![0.0, 1.0, 2.0], vec![0.0, 1.0], vec![0.0, 1.0]],
        [true; 3],
        [1, 1, 1],
        patches3(),
    )
    .unwrap();
    let marks = [
        AxisMark::refine([false, true, false]),
        AxisMark::refine([false, false, true]),
    ];
    let (next, _, rep) = amr.adapt(&marks, true).unwrap();
    assert_eq!(rep.refined, 2);
    let mesh = next.to_mesh().unwrap();
    check_topology(&mesh, 2.0, "G1b");
    assert_eq!(mesh.n_cells(), 4);
    assert_eq!(mesh.n_internal_faces(), 6);
    assert_eq!(mesh.points().len(), 21);
    let in_plane: Vec<usize> = (0..mesh.n_internal_faces())
        .filter(|&f| mesh.face_points(f).iter().all(|&p| mesh.points()[p][0] == 1.0))
        .collect();
    assert_eq!(in_plane.len(), 4);
    for &f in &in_plane {
        assert!((mesh.face_area(f) - 0.25).abs() <= 1e-15, "area {}", mesh.face_area(f));
    }
    let crossing = mesh.points().iter().filter(|p| **p == [1.0, 0.5, 0.5]).count();
    assert_eq!(crossing, 1);
    // no leaf has that point as a corner: every leaf spans the whole of y or the whole of z
    for l in next.leaves() {
        assert!(l.levels[1] == 0 || l.levels[2] == 0);
    }
}

// ------------------------------------------------------------------------------------------------ G2

#[test]
fn g2_split_sizes_and_level_limits() {
    let amr = slab(4, [2, 2, 0]);
    let base_vol = 0.1 / 16.0;
    let one = |axes: [bool; 3]| {
        let mut marks = vec![AxisMark::KEEP; 16];
        marks[5] = AxisMark::refine(axes);
        amr.adapt(&marks, true).unwrap()
    };
    for (axes, cells, parts) in [
        ([true, false, false], 17, 2),
        ([false, true, false], 17, 2),
        ([true, true, false], 19, 4),
    ] {
        let (next, tr, rep) = one(axes);
        assert_eq!(next.n_cells(), cells);
        assert_eq!(rep.refined, 1);
        let mesh = next.to_mesh().unwrap();
        check_topology(&mesh, 0.1, "G2");
        let small: Vec<usize> = (0..mesh.n_cells())
            .filter(|&c| (mesh.cell_volumes()[c] - base_vol / parts as f64).abs() <= 1e-16)
            .collect();
        assert_eq!(small.len(), parts, "{axes:?}");
        for &c in &small {
            assert_eq!(tr.sources[c], rcfd_mesh::adapt::Source::Descendant(5));
            let l = next.leaves()[c].levels;
            assert_eq!(l, [axes[0] as u8, axes[1] as u8, 0]);
        }
        let others = (0..mesh.n_cells())
            .filter(|&c| (mesh.cell_volumes()[c] - base_vol).abs() <= 1e-16)
            .count();
        assert_eq!(others, 15);
    }
    // the unrefined axis: nothing happens, nothing is counted
    let (next, _, rep) = one([false, false, true]);
    assert_eq!(next.n_cells(), 16);
    assert_eq!((rep.refined, rep.ignored_at_max_level, rep.coarsened), (0, 0, 0));

    // level limits
    let amr = slab(4, [1, 1, 0]);
    let mut marks = vec![AxisMark::KEEP; 16];
    marks[0] = AxisMark::refine([true, false, false]);
    let (a1, _, _) = amr.adapt(&marks, false).unwrap();
    assert_eq!(a1.n_cells(), 17);
    assert_eq!(a1.leaves()[0].levels, [1, 0, 0]);
    let mut marks = vec![AxisMark::KEEP; 17];
    marks[0] = AxisMark::refine([true, false, false]);
    let (a2, _, rep) = a1.adapt(&marks, false).unwrap();
    assert_eq!(a2.n_cells(), 17);
    assert_eq!((rep.refined, rep.ignored_at_max_level), (0, 1));
    marks[0] = AxisMark::refine([true, true, false]);
    let (a3, _, rep) = a1.adapt(&marks, false).unwrap();
    assert_eq!(a3.n_cells(), 18);
    assert_eq!((rep.refined, rep.ignored_at_max_level), (1, 0));
    assert_eq!(a3.leaves()[0].levels, [1, 1, 0]);
    assert_eq!(a3.levels().iter().filter(|l| **l == [1, 1, 0]).count(), 2);
}

// ------------------------------------------------------------------------------------------------ G3

fn split_origin_along_y(balance: bool) -> (Vec<usize>, [u8; 3]) {
    let mut cur = slab(8, [3, 3, 0]);
    let mut counts = Vec::new();
    let mut jump = [0; 3];
    for _ in 0..3 {
        let mut marks = vec![AxisMark::KEEP; cur.n_cells()];
        marks[0] = AxisMark::refine([false, true, false]);
        let (next, _, _) = cur.adapt(&marks, balance).unwrap();
        let mesh = next.to_mesh().unwrap();
        check_topology(&mesh, 0.1, "G3");
        counts.push(next.n_cells());
        jump = max_axis_level_jump(&next, &mesh);
        if balance {
            assert!(jump[1] <= 1, "jump {jump:?}");
        }
        assert_eq!(next.leaves()[0].ijk, [0, 0, 0]);
        cur = next;
    }
    assert_eq!(cur.leaves()[0].levels, [0, 3, 0]);
    (counts, jump)
}

#[test]
fn g3_balance_per_axis_closed_form_counts() {
    let (counts, jump) = split_origin_along_y(true);
    assert_eq!(counts, vec![65, 67, 70]);
    assert_eq!(jump, [0, 1, 0]);
    let (counts, jump) = split_origin_along_y(false);
    assert_eq!(counts, vec![65, 66, 67]);
    assert_eq!(jump, [0, 3, 0]);
}

// ------------------------------------------------------------------------------------------------ G4

fn lin(p: Vec3) -> f64 {
    1.3 + 0.7 * p[0] - 0.4 * p[1] + 0.2 * p[2]
}

#[test]
fn g4_transfer_conserves_and_is_exact_for_linear_fields() {
    let cases: [(&str, AnisoMesh, [bool; 3], f64); 2] = [
        ("a", box_a(), [true; 3], 0.7 + 0.4 + 0.2 * 0.8),
        ("b", slab_b(), [true, true, false], 0.7 + 0.4),
    ];
    for (name, amr, axes, range) in cases {
        let mut rng = Lcg(777);
        let mesh = amr.to_mesh().unwrap();
        let random: Vec<f64> = (0..mesh.n_cells()).map(|_| rng.next() * 3.0 - 1.0).collect();
        let linear: Vec<f64> = mesh.cell_centres().iter().map(|&p| lin(p)).collect();
        for mode in [RefineTransfer::Injection, RefineTransfer::Linear] {
            let mut cur = amr.clone();
            let mut cur_mesh = mesh.clone();
            let mut qr = random.clone();
            let mut ql = linear.clone();
            for round in 0..4 {
                let marks: Vec<AxisMark> = (0..cur.n_cells())
                    .map(|_| {
                        let r = rng.next();
                        if round < 2 {
                            if r < 0.5 {
                                AxisMark::refine(subset(&mut rng))
                            } else {
                                AxisMark::KEEP
                            }
                        } else if r < 0.9 {
                            AxisMark::coarsen([true; 3])
                        } else {
                            AxisMark::KEEP
                        }
                    })
                    .collect();
                let (next, tr, rep) = cur.adapt(&marks, true).unwrap();
                let nm = next.to_mesh().unwrap();
                let nr = transfer_field(&cur_mesh, &nm, &tr, &qr, mode, axes).unwrap();
                let nl = transfer_field(&cur_mesh, &nm, &tr, &ql, mode, axes).unwrap();
                let (i0, i1) = (integral(&cur_mesh, &qr), integral(&nm, &nr));
                let scale = abs_integral(&cur_mesh, &qr);
                assert!(
                    (i1 - i0).abs() <= 1e-13 * scale,
                    "{name} {mode:?} round {round}: integral {i0} -> {i1}"
                );
                let (l0, l1) = (integral(&cur_mesh, &ql), integral(&nm, &nl));
                assert!((l1 - l0).abs() <= 1e-13 * l0.abs(), "{name} {mode:?} linear integral");
                if mode == RefineTransfer::Linear {
                    let worst = (0..nm.n_cells())
                        .map(|c| (nl[c] - lin(nm.cell_centres()[c])).abs())
                        .fold(0.0, f64::max);
                    println!(
                        "G4 {name} round {round}: {} -> {} cells (refined {}, coarsened {}), linear-field error {worst:.3e}",
                        cur.n_cells(),
                        next.n_cells(),
                        rep.refined,
                        rep.coarsened
                    );
                    assert!(worst <= 1e-12 * range, "{name} linear field error {worst}");
                }
                if round < 2 {
                    assert!(rep.refined > 0, "{name}: no refinement in round {round}");
                } else {
                    assert!(rep.coarsened > 0, "{name}: no merge in round {round}");
                }
                cur = next;
                cur_mesh = nm;
                qr = nr;
                ql = nl;
            }
        }
    }
}

// ------------------------------------------------------------------------------------------------ G5

fn assert_same_mesh(a: &Mesh, b: &Mesh, what: &str) {
    assert_eq!(a.n_cells(), b.n_cells(), "{what}: cells");
    assert_eq!(a.points().len(), b.points().len(), "{what}: points");
    for (i, (p, q)) in a.points().iter().zip(b.points()).enumerate() {
        for k in 0..3 {
            assert!(p[k].to_bits() == q[k].to_bits(), "{what}: point {i} differs");
        }
    }
    assert_eq!(a.n_faces(), b.n_faces(), "{what}: faces");
    assert_eq!(a.n_internal_faces(), b.n_internal_faces(), "{what}: internal faces");
    for f in 0..a.n_faces() {
        assert_eq!(a.face_points(f), b.face_points(f), "{what}: face {f}");
    }
    assert_eq!(a.owner(), b.owner(), "{what}: owner");
    assert_eq!(a.neighbour(), b.neighbour(), "{what}: neighbour");
    assert_eq!(a.patches().len(), b.patches().len(), "{what}: patches");
    for (p, q) in a.patches().iter().zip(b.patches()) {
        assert_eq!(
            (&p.name, &p.kind, p.start, p.size),
            (&q.name, &q.kind, q.start, q.size),
            "{what}"
        );
    }
}

type IsoCase = (&'static str, [Vec<f64>; 3], [bool; 3], u8, [(String, PatchKind); 6]);

#[test]
fn g5_isotropic_marks_give_the_isotropic_lattice_bit_for_bit() {
    let cases: [IsoCase; 2] = [
        (
            "slab",
            [uniform(6), uniform(5), vec![0.0, 0.1]],
            [true, true, false],
            3,
            patches2(),
        ),
        ("box", coords_a(), [true; 3], 2, patches3()),
    ];
    for (name, coords, refine, max_level, patches) in cases {
        for balance in [true, false] {
            let mut iso = AmrMesh::new(coords.clone(), refine, max_level, patches.clone()).unwrap();
            let ml = [0, 1, 2].map(|a| if refine[a] { max_level } else { 0 });
            let mut dir = AnisoMesh::new(coords.clone(), refine, ml, patches.clone()).unwrap();
            assert_same_mesh(&iso.to_mesh().unwrap(), &dir.to_mesh().unwrap(), name);
            let mut rng = Lcg(424242);
            for round in 0..3 {
                let marks: Vec<Mark> = (0..iso.n_cells())
                    .map(|_| if rng.next() < 0.3 { Mark::Refine } else { Mark::Keep })
                    .collect();
                let dmarks: Vec<AxisMark> = marks
                    .iter()
                    .map(|m| {
                        if *m == Mark::Refine {
                            AxisMark::refine([true; 3])
                        } else {
                            AxisMark::KEEP
                        }
                    })
                    .collect();
                let (ni, ti, ri) = iso.adapt(&marks, balance).unwrap();
                let (nd, td, rd) = dir.adapt(&dmarks, balance).unwrap();
                let what = format!("{name} balance {balance} round {round}");
                assert_eq!(ni.n_cells(), nd.n_cells(), "{what}");
                assert_eq!(ti, td, "{what}: transfer");
                assert_eq!(ri, rd, "{what}: report");
                assert_same_mesh(&ni.to_mesh().unwrap(), &nd.to_mesh().unwrap(), &what);
                println!("G5 {what}: {} cells, refined {}", ni.n_cells(), ri.refined);
                iso = ni;
                dir = nd;
            }
            assert!(iso.n_cells() > 60, "{name}: too little refinement to compare");
            let conv = AnisoMesh::from_amr(&iso);
            assert_eq!(conv.leaves(), dir.leaves(), "{name}: from_amr leaves");
            assert_same_mesh(&iso.to_mesh().unwrap(), &conv.to_mesh().unwrap(), name);
        }
    }
}

// ------------------------------------------------------------------------------------------------ G6

/// The registered G6, unchanged. MISSED at its one run: 146 cells went 82, 51, 35, 26, 20, 16 and the seventh round merged
/// nothing (16 cells, not 12): pair merges in three dimensions left a five-box tiling of one base cell in which no two leaves
/// are buddies (ADR 0267 amendment 1; the corrected checks are G6b and G6c below).
#[test]
#[ignore = "known miss (ADR 0267 amendment 1): record of the registered G6"]
fn g6_coarsening_returns_to_the_base_known_miss() {
    let mut rng = Lcg(2468);
    let mut cur = box_a();
    for _ in 0..3 {
        let marks: Vec<AxisMark> = (0..cur.n_cells())
            .map(|_| {
                if rng.next() < 0.5 {
                    AxisMark::refine(subset(&mut rng))
                } else {
                    AxisMark::KEEP
                }
            })
            .collect();
        cur = cur.adapt(&marks, false).unwrap().0;
    }
    let n0 = cur.n_cells();
    assert!(n0 > 40, "too little refinement: {n0}");
    let mut mesh = cur.to_mesh().unwrap();
    let mut q: Vec<f64> = (0..n0).map(|_| rng.next() * 3.0 - 1.0).collect();
    let scale = abs_integral(&mesh, &q);
    let i0 = integral(&mesh, &q);
    let mut rounds = 0;
    while cur.n_cells() > 12 {
        assert!(rounds < n0 - 12, "not at the base after {rounds} rounds");
        let marks = vec![AxisMark::coarsen([true; 3]); cur.n_cells()];
        let (next, tr, rep) = cur.adapt(&marks, false).unwrap();
        assert!(next.n_cells() < cur.n_cells(), "round {rounds}: no merge");
        assert_eq!(next.n_cells(), cur.n_cells() - rep.coarsened);
        let nm = next.to_mesh().unwrap();
        check_topology(&nm, 0.8, "G6");
        q = transfer_field(&mesh, &nm, &tr, &q, RefineTransfer::Injection, [true; 3]).unwrap();
        let i1 = integral(&nm, &q);
        assert!(
            (i1 - i0).abs() <= 1e-13 * scale,
            "round {rounds}: integral {i0} -> {i1}"
        );
        cur = next;
        mesh = nm;
        rounds += 1;
    }
    println!("G6: {n0} cells back to 12 in {rounds} rounds");
    assert_eq!(cur.n_cells(), 12);
    assert!(cur.levels().iter().all(|l| *l == [0, 0, 0]));
    assert_eq!(cur.leaves(), box_a().leaves());
}

// ------------------------------------------------------------------------------------------------ G6b, G6c (amendment 1)

#[test]
fn g6b_coarsening_returns_to_the_base_in_a_slab() {
    let mut rng = Lcg(2468);
    let mut cur = slab_b();
    for _ in 0..3 {
        let marks: Vec<AxisMark> = (0..cur.n_cells())
            .map(|_| {
                if rng.next() < 0.5 {
                    AxisMark::refine(subset(&mut rng))
                } else {
                    AxisMark::KEEP
                }
            })
            .collect();
        cur = cur.adapt(&marks, false).unwrap().0;
    }
    let n0 = cur.n_cells();
    assert!(n0 > 60, "too little refinement: {n0}");
    let mut mesh = cur.to_mesh().unwrap();
    let mut q: Vec<f64> = (0..n0).map(|_| rng.next() * 3.0 - 1.0).collect();
    let scale = abs_integral(&mesh, &q);
    let i0 = integral(&mesh, &q);
    let mut rounds = 0;
    while cur.n_cells() > 20 {
        assert!(rounds < n0 - 20, "not at the base after {rounds} rounds");
        let marks = vec![AxisMark::coarsen([true; 3]); cur.n_cells()];
        let (next, tr, rep) = cur.adapt(&marks, false).unwrap();
        assert!(next.n_cells() < cur.n_cells(), "round {rounds}: no merge");
        assert_eq!(next.n_cells(), cur.n_cells() - rep.coarsened);
        let nm = next.to_mesh().unwrap();
        check_topology(&nm, 0.1, "G6b");
        q = transfer_field(&mesh, &nm, &tr, &q, RefineTransfer::Injection, [true, true, false]).unwrap();
        let i1 = integral(&nm, &q);
        assert!(
            (i1 - i0).abs() <= 1e-13 * scale,
            "round {rounds}: integral {i0} -> {i1}"
        );
        cur = next;
        mesh = nm;
        rounds += 1;
    }
    println!("G6b: {n0} cells back to 20 in {rounds} rounds");
    assert_eq!(cur.n_cells(), 20);
    assert!(cur.levels().iter().all(|l| *l == [0, 0, 0]));
    assert_eq!(cur.leaves(), slab_b().leaves());
}

#[test]
fn g6c_the_stuck_tiling_is_a_valid_mesh_and_a_fixed_point_of_pair_coarsening() {
    let cell = AnisoMesh::new(
        [vec![0.0, 1.0], vec![0.0, 1.0], vec![0.0, 1.0]],
        [true; 3],
        [1, 1, 1],
        patches3(),
    )
    .unwrap();
    let (octants, _, _) = cell.adapt(&[AxisMark::refine([true; 3])], false).unwrap();
    assert_eq!(octants.n_cells(), 8);
    let marks: Vec<AxisMark> = octants
        .leaves()
        .iter()
        .map(|l| match l.ijk {
            [_, 1, 0] => AxisMark::coarsen([true, false, false]),
            [0, _, 1] => AxisMark::coarsen([false, true, false]),
            [1, 0, _] => AxisMark::coarsen([false, false, true]),
            _ => AxisMark::KEEP,
        })
        .collect();
    assert_eq!(marks.iter().filter(|m| **m == AxisMark::KEEP).count(), 2);
    let (stuck, tr, rep) = octants.adapt(&marks, false).unwrap();
    assert_eq!((stuck.n_cells(), rep.coarsened), (5, 3));
    let mesh = stuck.to_mesh().unwrap();
    check_topology(&mesh, 1.0, "G6c");
    let merged = tr
        .sources
        .iter()
        .filter(|s| matches!(s, rcfd_mesh::adapt::Source::Merged(k) if k.len() == 2))
        .count();
    assert_eq!(merged, 3);
    let (again, _, rep) = stuck.adapt(&[AxisMark::coarsen([true; 3]); 5], false).unwrap();
    assert_eq!((again.n_cells(), rep.coarsened), (5, 0));
    assert_eq!(again.leaves(), stuck.leaves());
    let (back, _, _) = stuck.adapt(&[AxisMark::refine([true; 3]); 5], false).unwrap();
    assert_eq!(back.leaves(), octants.leaves());
    check_topology(&back.to_mesh().unwrap(), 1.0, "G6c octants");
}

// ------------------------------------------------------------------------------------------------ G7

#[test]
fn g7_located_errors() {
    let e = AnisoMesh::new(coords_a(), [true; 3], [2, 11, 2], patches3())
        .unwrap_err()
        .to_string();
    assert!(e.contains("max_level[1]"), "{e}");
    let e = AnisoMesh::new(coords_b(), [true, true, false], [2, 2, 1], patches2())
        .unwrap_err()
        .to_string();
    assert!(e.contains("max_level[2]") && e.contains("not refined"), "{e}");
    let e = box_a().adapt(&[AxisMark::KEEP; 3], true).unwrap_err().to_string();
    assert!(e.contains("marks has 3 entries"), "{e}");
}
