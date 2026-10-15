//! F-232 (ADR 0030 part A): surface diagnostics and repair. Limits are those registered in
//! docs/adr/0030-wave19b-meshing.md before this file was compiled or run.

use rcfd_mesh::surface::{cube, diagnose, icosphere, repair, torus, DiagOptions, RepairOptions, TriSurface};

fn clean_counts(s: &TriSurface) -> (usize, usize, usize, i64) {
    let d = diagnose(s, &DiagOptions::default());
    (d.n_points_used, d.n_edges, d.n_tris, d.euler)
}

fn disjoint(s: &TriSurface, ids: &[usize]) -> bool {
    for (i, &a) in ids.iter().enumerate() {
        for &b in &ids[i + 1..] {
            if s.tris[a].iter().any(|v| s.tris[b].contains(v)) {
                return false;
            }
        }
    }
    true
}

fn without(s: &TriSurface, ids: &[usize]) -> TriSurface {
    let mut o = s.clone();
    let mut k = 0;
    o.tris.retain(|_| {
        let keep = !ids.contains(&k);
        k += 1;
        keep
    });
    o.tags.truncate(o.tris.len());
    o
}

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs()
}

#[test]
fn s1_generators() {
    let s = icosphere(1.0, 2);
    assert_eq!(clean_counts(&s), (162, 480, 320, 2));
    let c = cube(2.0, 3);
    assert_eq!(clean_counts(&c), (56, 162, 108, 2));
    assert!(rel(c.area(), 24.0) < 1e-12 && rel(c.enclosed_volume(), 8.0) < 1e-12);
    let t = torus(2.0, 0.5, 24, 12);
    assert_eq!(clean_counts(&t), (288, 864, 576, 0));
    for x in [&s, &c, &t] {
        let d = diagnose(x, &DiagOptions::default());
        assert!(d.is_closed_manifold() && d.enclosed_volume > 0.0 && d.components == 1);
        assert_eq!((d.free_edges, d.nonmanifold_edges, d.misoriented_edges), (0, 0, 0));
    }
}

#[test]
fn s2_detector_counts() {
    let s = icosphere(1.0, 2);
    let ids = [0usize, 100, 200];
    assert!(disjoint(&s, &ids));
    let d = diagnose(&without(&s, &ids), &DiagOptions::default());
    assert_eq!(d.free_edges, 9);
    assert_eq!(d.open_loops.len(), 3);
    assert!(d.open_loops.iter().all(|l| l.len() == 3));
    // (b) reversed
    let ids5 = [0usize, 60, 120, 200, 300];
    assert!(disjoint(&s, &ids5));
    let mut r = s.clone();
    for &i in &ids5 {
        r.tris[i].swap(1, 2);
    }
    assert_eq!(diagnose(&r, &DiagOptions::default()).misoriented_edges, 15);
    // (c) duplicates
    let mut dd = s.clone();
    for &i in &[3usize, 50, 90, 150] {
        let t = dd.tris[i];
        dd.tris.push(t);
        dd.tags.push(0);
    }
    let d = diagnose(&dd, &DiagOptions::default());
    assert_eq!((d.duplicate_faces, d.nonmanifold_edges), (4, 12));
    // (d) degenerate
    let mut g = s.clone();
    g.tris.push([5, 5, 6]);
    g.tags.push(0);
    let n0 = g.points.len();
    g.points.push([10.0, 0.0, 0.0]);
    g.points.push([11.0, 0.0, 0.0]);
    g.points.push([12.0, 0.0, 0.0]);
    g.tris.push([n0, n0 + 1, n0 + 2]);
    g.tags.push(0);
    assert_eq!(diagnose(&g, &DiagOptions::default()).degenerate_tris, 2);
    // (e) fin
    let mut f = s.clone();
    let e = f.tris[0];
    f.points.push([2.0, 2.0, 2.0]);
    let apex = f.points.len() - 1;
    f.tris.push([e[0], e[1], apex]);
    f.tags.push(0);
    assert_eq!(diagnose(&f, &DiagOptions::default()).nonmanifold_edges, 1);
    // cube
    let c = cube(2.0, 3);
    let d = diagnose(&without(&c, &[7]), &DiagOptions::default());
    assert_eq!((d.free_edges, d.open_loops.len()), (3, 1));
}

#[test]
#[ignore = "set-up defect: icosahedron vertex 0 has valence 5, not 6 (ADR 0030 addendum); kept as a record, see s3c"]
fn s3_repair_outcomes() {
    let o = RepairOptions::default();
    for base in [icosphere(1.0, 2), cube(2.0, 3)] {
        let v0 = base.enclosed_volume();
        let n0 = base.n_tris();
        let (r, rep) = repair(&without(&base, &[0]), &o);
        assert_eq!(rep.holes_filled, 1);
        let d = diagnose(&r, &DiagOptions::default());
        assert!(d.is_closed_manifold());
        assert_eq!((r.n_tris(), d.euler), (n0, 2));
        assert!(rel(r.enclosed_volume(), v0) <= 1e-12);
    }
    let s = icosphere(1.0, 2);
    let v0 = s.enclosed_volume();
    let around: Vec<usize> = (0..s.n_tris()).filter(|&i| s.tris[i].contains(&0)).collect();
    assert_eq!(around.len(), 6);
    let holed = without(&s, &around);
    let d = diagnose(&holed, &DiagOptions::default());
    assert_eq!((d.open_loops.len(), d.open_loops[0].len()), (1, 6));
    let (r, rep) = repair(&holed, &o);
    assert_eq!(rep.tris_added, 4);
    let d = diagnose(&r, &DiagOptions::default());
    assert!(d.is_closed_manifold());
    assert_eq!((r.n_tris(), d.euler), (318, 2));
    assert!(rel(r.enclosed_volume(), v0) <= 0.005);
    // (b) orientation
    let mut m = s.clone();
    for &i in &[0usize, 60, 120, 200, 300] {
        m.tris[i].swap(1, 2);
    }
    let (r, _) = repair(&m, &o);
    let d = diagnose(&r, &DiagOptions::default());
    assert_eq!(d.misoriented_edges, 0);
    assert!(rel(r.enclosed_volume(), v0) <= 1e-12);
    // (c) duplicates
    let mut dd = s.clone();
    for &i in &[3usize, 50, 90, 150] {
        let t = dd.tris[i];
        dd.tris.push(t);
        dd.tags.push(0);
    }
    let (r, _) = repair(&dd, &o);
    assert_eq!(r.n_tris(), 320);
    assert_eq!(diagnose(&r, &DiagOptions::default()).nonmanifold_edges, 0);
    // (d) degenerate
    let mut g = s.clone();
    g.tris.push([5, 5, 6]);
    g.tags.push(0);
    let (r, _) = repair(&g, &o);
    assert_eq!(r.n_tris(), 320);
}

#[test]
fn s3c_repair_outcomes() {
    let o = RepairOptions::default();
    for base in [icosphere(1.0, 2), cube(2.0, 3)] {
        let v0 = base.enclosed_volume();
        let n0 = base.n_tris();
        let (r, rep) = repair(&without(&base, &[0]), &o);
        assert_eq!(rep.holes_filled, 1);
        let d = diagnose(&r, &DiagOptions::default());
        assert!(d.is_closed_manifold());
        assert_eq!((r.n_tris(), d.euler), (n0, 2));
        assert!(rel(r.enclosed_volume(), v0) <= 1e-12);
    }
    let s = icosphere(1.0, 2);
    let v0 = s.enclosed_volume();
    let around: Vec<usize> = (0..s.n_tris()).filter(|&i| s.tris[i].contains(&12)).collect();
    assert_eq!(around.len(), 6);
    let holed = without(&s, &around);
    let d = diagnose(&holed, &DiagOptions::default());
    assert_eq!((d.open_loops.len(), d.open_loops[0].len()), (1, 6));
    let (r, rep) = repair(&holed, &o);
    assert_eq!(rep.tris_added, 4);
    let d = diagnose(&r, &DiagOptions::default());
    assert!(d.is_closed_manifold());
    assert_eq!((r.n_tris(), d.euler), (318, 2));
    assert!(rel(r.enclosed_volume(), v0) <= 0.005);
    // (b) orientation
    let mut m = s.clone();
    for &i in &[0usize, 60, 120, 200, 300] {
        m.tris[i].swap(1, 2);
    }
    let (r, _) = repair(&m, &o);
    let d = diagnose(&r, &DiagOptions::default());
    assert_eq!(d.misoriented_edges, 0);
    assert!(rel(r.enclosed_volume(), v0) <= 1e-12);
    // (c) duplicates
    let mut dd = s.clone();
    for &i in &[3usize, 50, 90, 150] {
        let t = dd.tris[i];
        dd.tris.push(t);
        dd.tags.push(0);
    }
    let (r, _) = repair(&dd, &o);
    assert_eq!(r.n_tris(), 320);
    assert_eq!(diagnose(&r, &DiagOptions::default()).nonmanifold_edges, 0);
    // (d) degenerate
    let mut g = s.clone();
    g.tris.push([5, 5, 6]);
    g.tags.push(0);
    let (r, _) = repair(&g, &o);
    assert_eq!(r.n_tris(), 320);
}

#[test]
fn s4_idempotence_and_determinism() {
    let o = RepairOptions::default();
    let s = icosphere(1.0, 2);
    let mut cases = vec![without(&s, &[0, 100, 200])];
    let mut m = s.clone();
    for &i in &[0usize, 60, 120] {
        m.tris[i].swap(1, 2);
    }
    cases.push(m);
    let mut dd = s.clone();
    dd.tris.push(s.tris[3]);
    dd.tags.push(0);
    cases.push(dd);
    for c in cases {
        let (r1, _) = repair(&c, &o);
        let (r2, rep2) = repair(&r1, &o);
        assert_eq!(r1, r2);
        assert_eq!(rep2, Default::default());
        let (r3, _) = repair(&c, &o);
        assert_eq!(r1, r3);
    }
    let (r, rep) = repair(&s, &o);
    assert_eq!(r, s);
    assert_eq!(rep, Default::default());
}

#[test]
fn s5_inside_out() {
    let s = icosphere(1.0, 2);
    let mut f = s.clone();
    f.flip_all();
    let (r, rep) = repair(&f, &RepairOptions::default());
    assert_eq!(rep.components_inverted, 1);
    assert!(rel(r.enclosed_volume(), s.enclosed_volume()) <= 1e-12);
}

#[test]
fn s6_small_features() {
    let mut s = icosphere(1.0, 2);
    let n0 = s.points.len();
    s.points.push([5.0, 0.0, 0.0]);
    s.points.push([6.0, 0.0, 0.0]);
    s.points.push([5.5, 0.0017453, 0.0]);
    s.tris.push([n0, n0 + 1, n0 + 2]);
    s.tags.push(0);
    let d = diagnose(&s, &DiagOptions::default());
    // smallest angle: atan(0.0017453/0.5) = 0.2 degrees at... the apex angles are about 0.2/0.2/179.6
    assert_eq!((d.sliver_tris, d.degenerate_tris), (1, 0));
    let mut t = icosphere(1.0, 2);
    let m = t.points.len();
    t.points.push([7.0, 0.0, 0.0]);
    t.points.push([7.0 + 2e-5, 0.0, 0.0]);
    t.points.push([7.0, 1.0, 0.0]);
    t.tris.push([m, m + 1, m + 2]);
    t.tags.push(0);
    let d = diagnose(&t, &DiagOptions::default());
    assert_eq!(d.small_edges, 1);
}
