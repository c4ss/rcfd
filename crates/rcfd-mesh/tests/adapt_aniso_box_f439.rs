//! F-439 (ADR 0341): coarsening on the directional lattice by a box merge (`AnisoMesh::adapt_with`, `Coarsening::Box`).
//! Limits registered in docs/adr/0341-f439-registration-box-merge-for-directional-coarsening.md (checks B1 to B6) before
//! this file was compiled or run.

use rcfd_core::vec3::{self, Vec3};
use rcfd_mesh::adapt::aniso::{max_axis_level_jump, AnisoMesh, AxisMark, Coarsening};
use rcfd_mesh::adapt::{transfer_field, RefineTransfer, Source};
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

/// (a): the graded 3-D box 3 x 2 x 2 of ADR 0267, volume 0.8.
fn box_a() -> AnisoMesh {
    AnisoMesh::new(
        [vec![0.0, 0.2, 0.5, 1.0], vec![0.0, 0.3, 1.0], vec![0.0, 0.45, 0.8]],
        [true; 3],
        [2, 2, 2],
        patches3(),
    )
    .unwrap()
}

/// (b): the slab 5 x 4 of ADR 0267, volume 0.1.
fn slab_b() -> AnisoMesh {
    AnisoMesh::new(
        [uniform(5), uniform(4), vec![0.0, 0.1]],
        [true, true, false],
        [3, 3, 0],
        patches2(),
    )
    .unwrap()
}

/// (c): the 3-D box 2 x 3 x 2 of ADR 0267 refined along x and z only, volume 1.
fn box_c() -> AnisoMesh {
    AnisoMesh::new(
        [uniform(2), uniform(3), uniform(2)],
        [true, false, true],
        [2, 0, 3],
        patches3(),
    )
    .unwrap()
}

/// (d): a 2 x 2 x 1 unit box with three levels along every axis, volume 1.
fn box_d() -> AnisoMesh {
    AnisoMesh::new(
        [uniform(2), uniform(2), vec![0.0, 1.0]],
        [true; 3],
        [3, 3, 3],
        patches3(),
    )
    .unwrap()
}

fn unit_cell(ml: u8) -> AnisoMesh {
    AnisoMesh::new(
        [vec![0.0, 1.0], vec![0.0, 1.0], vec![0.0, 1.0]],
        [true; 3],
        [ml; 3],
        patches3(),
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

/// G1's marks of ADR 0267.
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

/// The topology and geometry checks of ADR 0054's A1 (a copy of ADR 0267's). `vol` is the exact box volume.
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

fn lin(p: Vec3) -> f64 {
    1.3 + 0.7 * p[0] - 0.4 * p[1] + 0.2 * p[2]
}

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

/// G6c's marks of ADR 0267 on the octants: the three pair merges that leave the five-box tiling.
fn g6c_marks(octants: &AnisoMesh) -> Vec<AxisMark> {
    octants
        .leaves()
        .iter()
        .map(|l| match l.ijk {
            [_, 1, 0] => AxisMark::coarsen([true, false, false]),
            [0, _, 1] => AxisMark::coarsen([false, true, false]),
            [1, 0, _] => AxisMark::coarsen([false, false, true]),
            _ => AxisMark::KEEP,
        })
        .collect()
}

/// The octants of the unit cell and the five-box tiling made from them by pair merges.
fn five_box(ml: u8) -> (AnisoMesh, AnisoMesh) {
    let cell = unit_cell(ml);
    let (octants, _, _) = cell.adapt(&[AxisMark::refine([true; 3])], false).unwrap();
    assert_eq!(octants.n_cells(), 8);
    let (five, _, rep) = octants.adapt(&g6c_marks(&octants), false).unwrap();
    assert_eq!((five.n_cells(), rep.coarsened), (5, 3));
    (octants, five)
}

const ALL: AxisMark = AxisMark {
    refine: [false; 3],
    coarsen: [true; 3],
};

// ------------------------------------------------------------------------------------------------ B1

#[test]
fn b1_the_five_box_tiling_returns_to_the_cell() {
    let (_, five) = five_box(1);
    let mesh = five.to_mesh().unwrap();
    check_topology(&mesh, 1.0, "B1 five-box");
    let mut rng = Lcg(99);
    let q: Vec<f64> = (0..5).map(|_| rng.next() * 3.0 - 1.0).collect();
    let (one, tr, rep) = five.adapt_with(&[ALL; 5], false, Coarsening::Box).unwrap();
    assert_eq!(one.n_cells(), 1);
    assert_eq!(one.leaves()[0].levels, [0, 0, 0]);
    assert_eq!(rep.coarsened, 1);
    assert_eq!(tr.sources, vec![Source::Merged(vec![0, 1, 2, 3, 4])]);
    let m1 = one.to_mesh().unwrap();
    check_topology(&m1, 1.0, "B1 cell");
    let q1 = transfer_field(&mesh, &m1, &tr, &q, RefineTransfer::Linear, [true; 3]).unwrap();
    let (i0, i1) = (integral(&mesh, &q), integral(&m1, &q1));
    println!("B1: 5 cells -> 1 in one call, integral {i0} -> {i1}");
    assert!((i1 - i0).abs() <= 1e-13 * abs_integral(&mesh, &q));
}

// ------------------------------------------------------------------------------------------------ B2

/// All-axes box rounds, balance off, from `state` to `base`, with every registered per-round check. Returns the counts.
fn box_coarsen_to_base(state: &AnisoMesh, base: &AnisoMesh, vol: f64, range: f64, what: &str) -> Vec<usize> {
    let n0 = state.n_cells();
    let nb = base.n_cells();
    let mut cur = state.clone();
    let mut mesh = cur.to_mesh().unwrap();
    check_topology(&mesh, vol, what);
    let mut rng = Lcg(31337 + n0 as u64);
    let mut q: Vec<f64> = (0..n0).map(|_| rng.next() * 3.0 - 1.0).collect();
    let mut ql: Vec<f64> = mesh.cell_centres().iter().map(|&p| lin(p)).collect();
    let scale = abs_integral(&mesh, &q);
    let i0 = integral(&mesh, &q);
    let mut counts = vec![n0];
    let mut rounds = 0;
    while cur.leaves() != base.leaves() {
        assert!(
            rounds < n0 - nb,
            "{what}: not at the base after {rounds} rounds ({} cells)",
            cur.n_cells()
        );
        let (next, tr, rep) = cur
            .adapt_with(&vec![ALL; cur.n_cells()], false, Coarsening::Box)
            .unwrap();
        assert!(next.n_cells() < cur.n_cells(), "{what}: round {rounds}: no merge");
        assert!(rep.coarsened > 0, "{what}");
        let nm = next.to_mesh().unwrap();
        check_topology(&nm, vol, &format!("{what} round {rounds}"));
        q = transfer_field(&mesh, &nm, &tr, &q, RefineTransfer::Linear, [true; 3]).unwrap();
        ql = transfer_field(&mesh, &nm, &tr, &ql, RefineTransfer::Linear, [true; 3]).unwrap();
        let i1 = integral(&nm, &q);
        assert!(
            (i1 - i0).abs() <= 1e-13 * scale,
            "{what}: round {rounds}: integral {i0} -> {i1}"
        );
        let worst = (0..nm.n_cells())
            .map(|c| (ql[c] - lin(nm.cell_centres()[c])).abs())
            .fold(0.0, f64::max);
        assert!(
            worst <= 1e-12 * range,
            "{what}: round {rounds}: linear field error {worst}"
        );
        cur = next;
        mesh = nm;
        rounds += 1;
        counts.push(cur.n_cells());
    }
    assert!(cur.levels().iter().all(|l| *l == [0, 0, 0]), "{what}");
    counts
}

/// Pair all-axes rounds (balance off) until no merge; the final count.
fn pair_coarsen_until_stuck(state: &AnisoMesh) -> AnisoMesh {
    let mut cur = state.clone();
    for _ in 0..500 {
        let (next, _, rep) = cur.adapt(&vec![ALL; cur.n_cells()], false).unwrap();
        if rep.coarsened == 0 {
            return next;
        }
        cur = next;
    }
    panic!("pair coarsening did not stop");
}

fn random_state(base: &AnisoMesh, seed: u64) -> (AnisoMesh, String) {
    let mut rng = Lcg(seed);
    let mut cur = base.clone();
    let mut log = String::new();
    for round in 0..5 {
        let kind = if round == 0 {
            0
        } else {
            ((rng.next() * 3.0) as usize).min(2)
        };
        let balance = rng.next() < 0.5;
        let marks: Vec<AxisMark> = (0..cur.n_cells())
            .map(|_| {
                let r = rng.next();
                if kind == 0 {
                    if r < 0.4 {
                        AxisMark::refine(subset(&mut rng))
                    } else {
                        AxisMark::KEEP
                    }
                } else if r < 0.6 {
                    AxisMark::coarsen(subset(&mut rng))
                } else {
                    AxisMark::KEEP
                }
            })
            .collect();
        let rule = if kind == 2 { Coarsening::Box } else { Coarsening::Pair };
        cur = cur.adapt_with(&marks, balance, rule).unwrap().0;
        log.push_str(&format!(
            "{}{} {} ",
            ["R", "P", "B"][kind],
            if balance { "b" } else { "" },
            cur.n_cells()
        ));
    }
    (cur, log)
}

#[test]
fn b2_every_state_of_splits_and_merges_returns_to_the_base() {
    // (i) G6's state of ADR 0267
    let a = box_a();
    let range_a = 0.7 + 0.4 + 0.2 * 0.8;
    let mut rng = Lcg(2468);
    let mut g6 = a.clone();
    for _ in 0..3 {
        let marks: Vec<AxisMark> = (0..g6.n_cells())
            .map(|_| {
                if rng.next() < 0.5 {
                    AxisMark::refine(subset(&mut rng))
                } else {
                    AxisMark::KEEP
                }
            })
            .collect();
        g6 = g6.adapt(&marks, false).unwrap().0;
    }
    assert_eq!(g6.n_cells(), 146, "G6's state");
    let counts = box_coarsen_to_base(&g6, &a, 0.8, range_a, "B2 (i)");
    println!("B2 (i) G6 state: {counts:?} ({} rounds)", counts.len() - 1);
    // (ii) where pair coarsening stops
    let stuck = pair_coarsen_until_stuck(&g6);
    assert!(stuck.n_cells() > 12, "G6's pair rounds reach the base");
    let counts = box_coarsen_to_base(&stuck, &a, 0.8, range_a, "B2 (ii)");
    println!("B2 (ii) pair-stuck state: {counts:?} ({} rounds)", counts.len() - 1);
    let mut n_stuck = 1;
    // (iii) random sequences of splits, pair merges and box merges
    let d = box_d();
    let cases: [(&str, &AnisoMesh, f64, f64, std::ops::RangeInclusive<u64>); 2] = [
        ("a", &a, 0.8, range_a, 1..=12),
        ("d", &d, 1.0, 0.7 + 0.4 + 0.2, 101..=112),
    ];
    let mut n_random_stuck = 0;
    for (name, base, vol, range, seeds) in cases {
        for seed in seeds {
            let (state, log) = random_state(base, seed);
            let ps = pair_coarsen_until_stuck(&state);
            let is_stuck = ps.n_cells() > base.n_cells();
            if is_stuck {
                n_random_stuck += 1;
            }
            let what = format!("B2 (iii) {name} seed {seed}");
            let counts = box_coarsen_to_base(&state, base, vol, range, &what);
            println!(
                "{what}: build {log}| box rounds {counts:?} ({} rounds); pair rounds stop at {} cells{}",
                counts.len() - 1,
                ps.n_cells(),
                if is_stuck { " (stuck)" } else { "" }
            );
        }
    }
    n_stuck += n_random_stuck;
    println!("B2: {n_random_stuck} of 24 random states are pair-stuck; {n_stuck} of 26 states in all");
    assert!(n_stuck >= 1);
}

// ------------------------------------------------------------------------------------------------ B3

#[test]
fn b3_single_axis_marks_give_the_pair_merges() {
    let (octants, five) = five_box(1);
    let marks = g6c_marks(&octants);
    for balance in [false, true] {
        let (p, tp, rp) = octants.adapt_with(&marks, balance, Coarsening::Pair).unwrap();
        let (b, tb, rb) = octants.adapt_with(&marks, balance, Coarsening::Box).unwrap();
        assert_eq!((b.n_cells(), rb.coarsened), (5, 3), "balance {balance}");
        assert_eq!(b.leaves(), five.leaves());
        assert_eq!(p.leaves(), b.leaves());
        assert_eq!(tp, tb);
        assert_eq!(rp, rb);
    }
}

// ------------------------------------------------------------------------------------------------ B4

fn coarsen(axes: [bool; 3]) -> AxisMark {
    AxisMark::coarsen(axes)
}

#[test]
fn b4_consent_and_marks() {
    let cell = unit_cell(1);
    let (halves, _, _) = cell.adapt(&[AxisMark::refine([true, false, false])], false).unwrap();
    assert_eq!(halves.n_cells(), 2);
    let left = |m: &AnisoMesh| m.leaves().iter().position(|l| l.ijk[0] == 0).unwrap();
    let mut marks = vec![AxisMark::KEEP; 2];
    marks[left(&halves)] = AxisMark::refine([false, true, false]);
    let (three, _, _) = halves.adapt(&marks, false).unwrap();
    assert_eq!(three.n_cells(), 3);
    let is_r = |l: &rcfd_mesh::adapt::aniso::AxisLeaf| l.levels == [1, 0, 0];
    // (a)
    let (two, _, rep) = three.adapt_with(&[ALL; 3], false, Coarsening::Box).unwrap();
    assert_eq!((two.n_cells(), rep.coarsened), (2, 1), "B4 (a) first call");
    assert!(two.leaves().iter().all(|l| l.levels == [1, 0, 0]));
    let (one, _, rep) = two.adapt_with(&[ALL; 2], false, Coarsening::Box).unwrap();
    assert_eq!((one.n_cells(), rep.coarsened), (1, 1), "B4 (a) second call");
    // (b)
    let marks: Vec<AxisMark> = three
        .leaves()
        .iter()
        .map(|l| if is_r(l) { ALL } else { coarsen([true, false, false]) })
        .collect();
    let (b, _, rep) = three.adapt_with(&marks, false, Coarsening::Box).unwrap();
    assert_eq!((b.n_cells(), rep.coarsened), (3, 0), "B4 (b)");
    // (c)
    let marks: Vec<AxisMark> = halves
        .leaves()
        .iter()
        .map(|l| {
            if l.ijk[0] == 0 {
                ALL
            } else {
                coarsen([false, true, false])
            }
        })
        .collect();
    let (c, _, rep) = halves.adapt_with(&marks, false, Coarsening::Box).unwrap();
    assert_eq!((c.n_cells(), rep.coarsened), (2, 0), "B4 (c)");
    // (d), (e)
    let (_, five) = five_box(2);
    let origin = |l: &rcfd_mesh::adapt::aniso::AxisLeaf| l.levels == [1, 1, 1] && l.ijk == [0, 0, 0];
    assert_eq!(five.leaves().iter().filter(|l| origin(l)).count(), 1);
    let marks: Vec<AxisMark> = five
        .leaves()
        .iter()
        .map(|l| if origin(l) { AxisMark::KEEP } else { ALL })
        .collect();
    let (d, _, rep) = five.adapt_with(&marks, false, Coarsening::Box).unwrap();
    assert_eq!((d.n_cells(), rep.coarsened), (5, 0), "B4 (d)");
    let marks: Vec<AxisMark> = five
        .leaves()
        .iter()
        .map(|l| if origin(l) { AxisMark::refine([true; 3]) } else { ALL })
        .collect();
    let (e, _, rep) = five.adapt_with(&marks, false, Coarsening::Box).unwrap();
    assert_eq!((e.n_cells(), rep.coarsened, rep.refined), (12, 0, 1), "B4 (e)");
    check_topology(&e.to_mesh().unwrap(), 1.0, "B4 (e)");
}

// ------------------------------------------------------------------------------------------------ B5, B5b

#[test]
fn b5_balance_refuses_a_merge_beside_a_fine_neighbour() {
    let line = AnisoMesh::new(
        [vec![0.0, 1.0, 2.0], vec![0.0, 1.0], vec![0.0, 1.0]],
        [true, false, false],
        [2, 0, 0],
        patches3(),
    )
    .unwrap();
    let (four, _, _) = line.adapt(&[AxisMark::refine([true; 3]); 2], true).unwrap();
    assert_eq!(four.n_cells(), 4);
    let marks: Vec<AxisMark> = four
        .leaves()
        .iter()
        .map(|l| {
            if l.ijk[0] >= 2 {
                AxisMark::refine([true; 3])
            } else {
                AxisMark::KEEP
            }
        })
        .collect();
    let (six, _, _) = four.adapt(&marks, true).unwrap();
    assert_eq!(six.n_cells(), 6);
    let marks: Vec<AxisMark> = six
        .leaves()
        .iter()
        .map(|l| if l.levels[0] == 1 { ALL } else { AxisMark::KEEP })
        .collect();
    assert_eq!(marks.iter().filter(|m| **m == ALL).count(), 2);
    let (on, _, rep) = six.adapt_with(&marks, true, Coarsening::Box).unwrap();
    assert_eq!((on.n_cells(), rep.coarsened), (6, 0), "balance on");
    check_topology(&on.to_mesh().unwrap(), 2.0, "B5 on");
    let (off, _, rep) = six.adapt_with(&marks, false, Coarsening::Box).unwrap();
    assert_eq!((off.n_cells(), rep.coarsened), (5, 1), "balance off");
    assert_eq!(off.leaves()[0].levels, [0, 0, 0]);
    check_topology(&off.to_mesh().unwrap(), 2.0, "B5 off");
}

#[test]
fn b5b_box_coarsening_with_balance_keeps_the_balance() {
    let mut rng = Lcg(12345);
    let mut cur = box_a();
    for _ in 0..4 {
        let marks = random_marks(cur.n_cells(), &mut rng);
        cur = cur.adapt(&marks, true).unwrap().0;
    }
    let n0 = cur.n_cells();
    let mut mesh = cur.to_mesh().unwrap();
    assert!(max_axis_level_jump(&cur, &mesh).iter().all(|&j| j <= 1));
    let mut q: Vec<f64> = (0..n0).map(|_| rng.next() * 3.0 - 1.0).collect();
    let (i0, scale) = (integral(&mesh, &q), abs_integral(&mesh, &q));
    let mut counts = vec![n0];
    for round in 0..50 {
        let (next, tr, rep) = cur
            .adapt_with(&vec![ALL; cur.n_cells()], true, Coarsening::Box)
            .unwrap();
        let nm = next.to_mesh().unwrap();
        let what = format!("B5b round {round}");
        check_topology(&nm, 0.8, &what);
        let jump = max_axis_level_jump(&next, &nm);
        assert!(jump.iter().all(|&j| j <= 1), "{what}: jump {jump:?}");
        q = transfer_field(&mesh, &nm, &tr, &q, RefineTransfer::Linear, [true; 3]).unwrap();
        let i1 = integral(&nm, &q);
        assert!((i1 - i0).abs() <= 1e-13 * scale, "{what}: integral {i0} -> {i1}");
        cur = next;
        mesh = nm;
        if rep.coarsened == 0 {
            break;
        }
        counts.push(cur.n_cells());
    }
    let at_base = cur.leaves() == box_a().leaves();
    println!(
        "B5b (recorded): {counts:?}, {} rounds with merges, base reached: {at_base}",
        counts.len() - 1
    );
}

// ------------------------------------------------------------------------------------------------ B6

#[test]
fn b6_adapt_is_adapt_with_pair_bit_for_bit() {
    let cases: [(&str, AnisoMesh, usize); 3] = [("a", box_a(), 4), ("b", slab_b(), 5), ("c", box_c(), 4)];
    for (name, base, rounds) in cases {
        for (balance, seed) in [(true, 12345u64), (false, 67890u64)] {
            let mut rng = Lcg(seed);
            let mut cur = base.clone();
            for round in 0..rounds {
                let marks = random_marks(cur.n_cells(), &mut rng);
                let (x, tx, rx) = cur.adapt(&marks, balance).unwrap();
                let (y, ty, ry) = cur.adapt_with(&marks, balance, Coarsening::Pair).unwrap();
                let what = format!("B6 {name} balance {balance} round {round}");
                assert_eq!(x.leaves(), y.leaves(), "{what}");
                assert_eq!(tx, ty, "{what}");
                assert_eq!(rx, ry, "{what}");
                assert_same_mesh(&x.to_mesh().unwrap(), &y.to_mesh().unwrap(), &what);
                cur = x;
            }
        }
    }
    assert_eq!(Coarsening::default(), Coarsening::Pair);
}
