//! F-236 registered checks P1 to P7 (ADR 0020, part A). Limits are the registered ones.

use rcfd_core::vec3::Vec3;
use rcfd_mesh::gen::{box_mesh, cylinder_ogrid, warp, Grading};
use rcfd_mesh::mesh::Mesh;
use rcfd_mesh::partition::{
    edge_cut, smooth_partition, weighted_imbalance, Bisection, BisectionRule, Decomposition, GraphPartitioner,
    Partitioner, Rcb, SectorKind, Sectors,
};

fn m1() -> Mesh {
    box_mesh([20, 10, 5], [2.0, 1.0, 0.5], [Grading::Uniform; 3]).unwrap()
}
fn m2() -> Mesh {
    box_mesh(
        [24, 12, 6],
        [3.0, 1.5, 0.75],
        [Grading::Expansion(5.0), Grading::Expansion(3.0), Grading::Uniform],
    )
    .unwrap()
}
fn m3() -> Mesh {
    cylinder_ogrid(48, 12, 1.0, 5.0, Grading::Expansion(4.0), 1.0).unwrap()
}

fn bbox_centre(m: &Mesh) -> Vec3 {
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for p in m.points() {
        for d in 0..3 {
            lo[d] = lo[d].min(p[d]);
            hi[d] = hi[d].max(p[d]);
        }
    }
    [0.5 * (lo[0] + hi[0]), 0.5 * (lo[1] + hi[1]), 0.5 * (lo[2] + hi[2])]
}

fn sectors(m: &Mesh, kind: SectorKind) -> Sectors {
    let mut s = Sectors::about_z(kind);
    s.origin = bbox_centre(m);
    s
}

fn strategies(m: &Mesh) -> Vec<(&'static str, Box<dyn Partitioner>)> {
    vec![
        ("rcb", Box::new(Rcb)),
        ("longest", Box::new(Bisection::new(BisectionRule::Longest))),
        ("cycle", Box::new(Bisection::new(BisectionRule::Cycle))),
        ("fixed0", Box::new(Bisection::new(BisectionRule::Fixed(0)))),
        ("inertial", Box::new(Bisection::new(BisectionRule::Inertial))),
        ("radial", Box::new(sectors(m, SectorKind::Radial))),
        ("circumferential", Box::new(sectors(m, SectorKind::Circumferential))),
        ("graph", Box::new(GraphPartitioner::default())),
    ]
}

fn independent_cut(m: &Mesh, part: &[usize]) -> usize {
    let mut cut = 0;
    for c in 0..m.n_cells() {
        for nb in m.cell_neighbours(c) {
            if nb > c && part[nb] != part[c] {
                cut += 1;
            }
        }
    }
    cut
}

#[test]
fn p1_cover_and_cut_consistency() {
    for (mname, m) in [("M1", m1()), ("M2", m2()), ("M3", m3())] {
        for (sname, s) in strategies(&m) {
            for n in 2..=16 {
                let part = s.partition(&m, n).unwrap();
                assert_eq!(part.len(), m.n_cells(), "{mname} {sname} n={n}");
                assert!(part.iter().all(|&p| p < n), "{mname} {sname} n={n}");
                let mut count = vec![0usize; n];
                for &p in &part {
                    count[p] += 1;
                }
                assert!(count.iter().all(|&k| k > 0), "{mname} {sname} n={n}: empty part");
                let d = Decomposition::from_assignment(&m, n, part.clone()).unwrap();
                assert_eq!(d.edge_cut, independent_cut(&m, &part), "{mname} {sname} n={n}");
                assert_eq!(edge_cut(&m, &part), d.edge_cut);
            }
        }
    }
}

#[test]
fn p2_balance_unit_weights() {
    let mut worst: (f64, String) = (0.0, String::new());
    for (mname, m) in [("M1", m1()), ("M2", m2()), ("M3", m3())] {
        for (sname, s) in strategies(&m) {
            for n in 2..=16 {
                let part = s.partition(&m, n).unwrap();
                let imb = weighted_imbalance(&part, n, None);
                if imb > worst.0 {
                    worst = (imb, format!("{mname} {sname} n={n}"));
                }
                assert!(imb <= 0.05, "{mname} {sname} n={n}: imbalance {imb}");
            }
        }
    }
    println!("P2 worst imbalance {} at {}", worst.0, worst.1);
}

fn heavy_weights(m: &Mesh) -> Vec<f64> {
    let mid = bbox_centre(m)[0];
    m.cell_centres()
        .iter()
        .map(|c| if c[0] > mid { 4.0 } else { 1.0 })
        .collect()
}

#[test]
fn p3_node_weighting() {
    let mut worst = 0.0f64;
    let mut control_max = 0.0f64;
    for (mname, m) in [("M1", m1()), ("M2", m2())] {
        let w = heavy_weights(&m);
        let weighted: Vec<(&str, Box<dyn Partitioner>)> = vec![
            (
                "longest",
                Box::new(Bisection::new(BisectionRule::Longest).with_weights(w.clone())),
            ),
            (
                "cycle",
                Box::new(Bisection::new(BisectionRule::Cycle).with_weights(w.clone())),
            ),
            (
                "inertial",
                Box::new(Bisection::new(BisectionRule::Inertial).with_weights(w.clone())),
            ),
            ("circumferential", {
                let mut s = sectors(&m, SectorKind::Circumferential);
                s.weights = Some(w.clone());
                Box::new(s)
            }),
            (
                "graph",
                Box::new(GraphPartitioner {
                    weights: Some(w.clone()),
                    ..GraphPartitioner::default()
                }),
            ),
        ];
        for (sname, s) in &weighted {
            for n in 2..=16 {
                let part = s.partition(&m, n).unwrap();
                let imb = weighted_imbalance(&part, n, Some(&w));
                worst = worst.max(imb);
                assert!(imb <= 0.05, "{mname} {sname} n={n}: weighted imbalance {imb}");
            }
        }
        if mname == "M1" {
            for n in 2..=16 {
                let part = Rcb.partition(&m, n).unwrap();
                control_max = control_max.max(weighted_imbalance(&part, n, Some(&w)));
            }
        }
    }
    println!("P3 worst weighted imbalance {worst}; unweighted Rcb control max {control_max}");
    assert!(
        control_max > 0.05,
        "positive control: unweighted Rcb should be unbalanced, got {control_max}"
    );
}

#[test]
fn p4_graph_cut_not_worse_than_rcb() {
    let mut rows = Vec::new();
    for (mname, m) in [("M1", m1()), ("M2", m2()), ("M3", m3())] {
        for n in 2..=16 {
            let rcb = edge_cut(&m, &Rcb.partition(&m, n).unwrap());
            let graph = edge_cut(&m, &GraphPartitioner::default().partition(&m, n).unwrap());
            let cyc = edge_cut(&m, &Bisection::new(BisectionRule::Cycle).partition(&m, n).unwrap());
            let ine = edge_cut(&m, &Bisection::new(BisectionRule::Inertial).partition(&m, n).unwrap());
            rows.push(format!(
                "{mname} n={n}: rcb {rcb} graph {graph} cycle {cyc} inertial {ine}"
            ));
            assert!(graph <= rcb, "{mname} n={n}: graph {graph} > rcb {rcb}");
        }
    }
    println!("{}", rows.join("\n"));
    let m = m1();
    assert!(edge_cut(&m, &GraphPartitioner::default().partition(&m, 2).unwrap()) <= 50);
}

fn ijk(m: &Mesh, c: usize) -> (usize, usize, usize) {
    let p = m.cell_centres()[c];
    (
        (p[0] * 10.0).floor() as usize,
        (p[1] * 10.0).floor() as usize,
        (p[2] * 10.0).floor() as usize,
    )
}

#[test]
fn p5a_smoothing_restores_slab() {
    let m = m1();
    let slab: Vec<usize> = (0..m.n_cells()).map(|c| usize::from(ijk(&m, c).0 >= 10)).collect();
    assert_eq!(edge_cut(&m, &slab), 50);
    let flips = [
        (2, 2, 2),
        (4, 5, 2),
        (6, 3, 2),
        (3, 7, 2),
        (7, 6, 2),
        (12, 2, 2),
        (14, 5, 2),
        (16, 3, 2),
        (13, 7, 2),
        (17, 6, 2),
    ];
    let mut part = slab.clone();
    for c in 0..m.n_cells() {
        if flips.contains(&ijk(&m, c)) {
            part[c] = 1 - part[c];
        }
    }
    assert_eq!(part.iter().zip(&slab).filter(|(a, b)| a != b).count(), 10);
    assert!(edge_cut(&m, &part) > 50);
    let rep = smooth_partition(&m, &mut part, 2, None, 0.05, 20).unwrap();
    assert_eq!(part, slab);
    assert_eq!(rep.moved, 10);
    assert_eq!(edge_cut(&m, &part), 50);
}

#[test]
fn p5b_smoothing_never_hurts() {
    for (mname, m) in [("M1", m1()), ("M2", m2()), ("M3", m3())] {
        for (sname, s) in strategies(&m) {
            for n in 2..=16 {
                let mut part = s.partition(&m, n).unwrap();
                let (cut0, imb0) = (edge_cut(&m, &part), weighted_imbalance(&part, n, None));
                smooth_partition(&m, &mut part, n, None, 0.05, 10).unwrap();
                let (cut1, imb1) = (edge_cut(&m, &part), weighted_imbalance(&part, n, None));
                assert!(cut1 <= cut0, "{mname} {sname} n={n}: cut {cut0} -> {cut1}");
                assert!(
                    imb1 <= imb0.max(0.05) + 1e-12,
                    "{mname} {sname} n={n}: imbalance {imb0} -> {imb1}"
                );
                assert!(part.iter().all(|&p| p < n));
                let mut count = vec![0usize; n];
                for &p in &part {
                    count[p] += 1;
                }
                assert!(count.iter().all(|&k| k > 0));
            }
        }
    }
}

#[test]
fn p6_sectors_monotone_and_balanced() {
    for (mname, m) in [("M1", m1()), ("M3", m3())] {
        for kind in [SectorKind::Radial, SectorKind::Circumferential] {
            let s = sectors(&m, kind);
            let keys = s.keys(&m).unwrap();
            for n in 2..=16 {
                let part = s.partition(&m, n).unwrap();
                let mut lo = vec![f64::INFINITY; n];
                let mut hi = vec![f64::NEG_INFINITY; n];
                let mut cnt = vec![0usize; n];
                for (c, &p) in part.iter().enumerate() {
                    lo[p] = lo[p].min(keys[c]);
                    hi[p] = hi[p].max(keys[c]);
                    cnt[p] += 1;
                }
                for p in 0..n - 1 {
                    assert!(
                        hi[p] <= lo[p + 1],
                        "{mname} {kind:?} n={n}: bands {p} and {} overlap",
                        p + 1
                    );
                }
                let (mn, mx) = (cnt.iter().min().unwrap(), cnt.iter().max().unwrap());
                assert!(mx - mn <= 1, "{mname} {kind:?} n={n}: sizes {cnt:?}");
            }
        }
    }
}

#[test]
fn p6_circumferential_quadrants_closed_form() {
    let base = box_mesh([8, 8, 1], [8.0, 8.0, 1.0], [Grading::Uniform; 3]).unwrap();
    let m = warp(&base, |p| [p[0] - 4.0, p[1] - 4.0, p[2]]).unwrap();
    let s = Sectors::about_z(SectorKind::Circumferential);
    let part = s.partition(&m, 4).unwrap();
    for (c, p) in m.cell_centres().iter().enumerate() {
        let quadrant = match (p[0] > 0.0, p[1] > 0.0) {
            (true, true) => 0,
            (false, true) => 1,
            (false, false) => 2,
            (true, false) => 3,
        };
        assert_eq!(part[c], quadrant, "cell {c} at {p:?}");
    }
}

#[test]
fn p7_inertial_cuts_a_tilted_slab_across() {
    let base = box_mesh([32, 4, 2], [32.0, 4.0, 2.0], [Grading::Uniform; 3]).unwrap();
    let (s, c) = (30f64.to_radians().sin(), 30f64.to_radians().cos());
    let m = warp(&base, |p| [c * p[0] - s * p[1], s * p[0] + c * p[1], p[2]]).unwrap();
    let part = Bisection::new(BisectionRule::Inertial).partition(&m, 2).unwrap();
    assert_eq!(edge_cut(&m, &part), 8);
    // the parts are the halves of the slices along the long axis (cell centre projection on (cos, sin, 0))
    let proj: Vec<f64> = m.cell_centres().iter().map(|p| c * p[0] + s * p[1]).collect();
    let (mut a, mut b) = (0usize, 0usize);
    let key0 = proj[part.iter().position(|&p| p == 0).unwrap()] < 16.0;
    for (i, &p) in part.iter().enumerate() {
        let first_half = proj[i] < 16.0;
        assert_eq!(first_half, (p == 0) == key0, "cell {i}");
        if p == 0 {
            a += 1
        } else {
            b += 1
        }
    }
    assert_eq!((a, b), (128, 128));
    let rcb = edge_cut(&m, &Rcb.partition(&m, 2).unwrap());
    println!("P7 rcb cut on the tilted slab: {rcb}");
    assert!(rcb > 8, "positive control: rcb cut {rcb}");
}
