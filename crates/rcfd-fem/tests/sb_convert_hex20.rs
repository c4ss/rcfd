//! F-508 (ADR 0415): curved quadratic hexahedra on the quarter annulus of ADR 0413. The annulus converted by `from_mesh`,
//! upgraded to Hex20, the wall mid-edge nodes projected onto the cylinders. Checks C1 to C8 and the negative controls N1 to N3
//! of the registration.

use rcfd_fem::convert::{from_mesh, onto_cylinder, project_mid_edge_nodes};
use rcfd_fem::hex20::upgrade_to_hex20;
use rcfd_fem::{ElementKind, SolidMesh};
use rcfd_mesh::gen::{annulus_3d_sector, Grading};
use std::collections::BTreeSet;
use std::f64::consts::PI;

const A: f64 = 1.0;
const B: f64 = 2.0;
const H: f64 = 1.0;
const LEVELS: [usize; 4] = [1, 2, 4, 8];

fn rel(x: f64, y: f64) -> f64 {
    (x - y).abs() / y.abs()
}

fn orders(e: &[f64]) -> Vec<f64> {
    e.windows(2).map(|w| (w[0] / w[1]).log2()).collect()
}

/// One level: the measurements of ADR 0415 D1.
struct Level {
    k: usize,
    mesh: SolidMesh,
    volume: f64,
    area_inner: f64,
    area_outer: f64,
    projected: [usize; 2],
    dr_projected: f64,
    dr_corners: f64,
    n_points: usize,
}

fn level(k: usize, project: bool) -> Level {
    let fv = annulus_3d_sector(4 * k, k, k, A, B, H, PI / 2.0, Grading::Uniform).unwrap();
    let h8 = from_mesh(&fv).unwrap();
    let n_points = h8.nodes().len();
    let (q, mids) = upgrade_to_hex20(&h8).unwrap();
    let inner: BTreeSet<usize> = q.node_set("inner").unwrap().iter().copied().collect();
    let outer: BTreeSet<usize> = q.node_set("outer").unwrap().iter().copied().collect();
    // C8: the node sets of the upgraded mesh are the nodes of the wall surface sets.
    for (name, set) in [("inner", &inner), ("outer", &outer)] {
        let faces: BTreeSet<usize> = q.surface_set_nodes(name).unwrap().into_iter().collect();
        assert_eq!(
            set, &faces,
            "C8 k={k}: node set {name} differs from the nodes of its surface set"
        );
    }
    let mut moved = Vec::new();
    let m = project_mid_edge_nodes(&q, &mids, |mid, x| {
        if !project {
            return None;
        }
        let r = if inner.contains(&mid.node) {
            A
        } else if outer.contains(&mid.node) {
            B
        } else {
            return None;
        };
        moved.push((mid.node, r));
        Some(onto_cylinder(x, [0.0; 3], [0.0, 0.0, 1.0], r))
    })
    .unwrap();
    let radius_err = |n: usize, r: f64| {
        let p = m.nodes()[n];
        (p[0].hypot(p[1]) - r).abs()
    };
    let dr_projected = moved.iter().map(|&(n, r)| radius_err(n, r)).fold(0.0f64, f64::max);
    let mut dr_corners = 0.0f64;
    for (set, r) in [(&inner, A), (&outer, B)] {
        for &n in set.iter().filter(|&&n| n < n_points) {
            dr_corners = dr_corners.max(radius_err(n, r));
        }
    }
    let projected = [
        moved.iter().filter(|&&(_, r)| r == A).count(),
        moved.iter().filter(|&&(_, r)| r == B).count(),
    ];
    let area = |name: &str| -> f64 { m.surface_set(name).unwrap().iter().map(|&f| m.face_area(f)).sum() };
    let volume = (0..m.n_elements()).map(|e| m.element_volume(e)).sum();
    let (area_inner, area_outer) = (area("inner"), area("outer"));
    Level {
        k,
        mesh: m,
        volume,
        area_inner,
        area_outer,
        projected,
        dr_projected,
        dr_corners,
        n_points,
    }
}

/// Half angle of one circumferential division at level `k`.
fn phi(k: usize) -> f64 {
    PI / (16.0 * k as f64)
}

/// ADR 0415 arithmetic 3: the volume enclosed by the piecewise-quadratic walls.
fn v_curved(k: usize) -> f64 {
    let (s, c) = phi(k).sin_cos();
    H * (B * B - A * A) * (4 * k) as f64 * s * (4.0 - c) / 3.0
}

/// ADR 0415 arithmetic 4: the 3 x 3 Gauss area of a wall of radius `r`.
fn a_curved(r: f64, k: usize) -> f64 {
    let (s, c) = phi(k).sin_cos();
    let lg = 8.0 / 9.0 * s + 10.0 / 9.0 * (s * s + 12.0 / 5.0 * (1.0 - c) * (1.0 - c)).sqrt();
    H * (4 * k) as f64 * r * lg
}

/// ADR 0415 arithmetic 7: the chord prism and the flat wall faces.
fn v_chord(k: usize) -> f64 {
    0.5 * H * (B * B - A * A) * (4 * k) as f64 * (2.0 * phi(k)).sin()
}

fn a_chord(r: f64, k: usize) -> f64 {
    H * (4 * k) as f64 * 2.0 * r * phi(k).sin()
}

#[test]
fn c1_to_c8_curved_hex20_quarter_annulus() {
    let v_exact = PI * (B * B - A * A) * H / 4.0;
    let (ai_exact, ao_exact) = (PI * A * H / 2.0, PI * B * H / 2.0);
    let (mut ev, mut ei, mut eo) = (Vec::new(), Vec::new(), Vec::new());
    for k in LEVELS {
        let l = level(k, true);
        let m = &l.mesh;
        // C1: counts.
        assert_eq!(m.blocks().len(), 1);
        assert_eq!(m.blocks()[0].kind(), ElementKind::Hex20);
        assert_eq!(m.n_elements(), 4 * k * k * k, "C1 k={k}: elements");
        assert_eq!(l.n_points, (k + 1) * (4 * k + 1) * (k + 1), "C1 k={k}: points");
        let edges = 4 * k * (k + 1) * (k + 1) + 2 * (4 * k + 1) * k * (k + 1);
        assert_eq!(m.nodes().len(), l.n_points + edges, "C1 k={k}: Hex20 nodes");
        let per_wall = 4 * k * (k + 1) + (4 * k + 1) * k;
        assert_eq!(l.projected, [per_wall, per_wall], "C1 k={k}: projected nodes per wall");
        // C2, C7: radius.
        assert!(
            l.dr_projected <= 1e-14,
            "C2 k={k}: projected radius error {:.3e}",
            l.dr_projected
        );
        assert!(
            l.dr_corners <= 1e-14,
            "C7 k={k}: corner radius error {:.3e}",
            l.dr_corners
        );
        // C5, C6: closed forms of the curved geometry.
        let c5 = rel(l.volume, v_curved(k));
        let c6i = rel(l.area_inner, a_curved(A, k));
        let c6o = rel(l.area_outer, a_curved(B, k));
        for name in ["inner", "outer"] {
            assert_eq!(
                m.surface_set(name).unwrap().len(),
                4 * k * k,
                "C6 k={k}: faces of {name}"
            );
        }
        println!(
            "k={}: {} Hex20, {} nodes, projected {:?}, radius err projected {:.3e} corners {:.3e}; \
             C5 {:.3e}, C6 inner {:.3e} outer {:.3e}; errors volume {:.5e} inner {:.5e} outer {:.5e}",
            l.k,
            m.n_elements(),
            m.nodes().len(),
            l.projected,
            l.dr_projected,
            l.dr_corners,
            c5,
            c6i,
            c6o,
            rel(l.volume, v_exact),
            rel(l.area_inner, ai_exact),
            rel(l.area_outer, ao_exact)
        );
        assert!(c5 <= 1e-12, "C5 k={k}: volume against the closed form {c5:.3e}");
        assert!(c6i <= 1e-12 && c6o <= 1e-12, "C6 k={k}: areas {c6i:.3e} {c6o:.3e}");
        ev.push(rel(l.volume, v_exact));
        ei.push(rel(l.area_inner, ai_exact));
        eo.push(rel(l.area_outer, ao_exact));
    }
    let (pv, pi_, po) = (orders(&ev), orders(&ei), orders(&eo));
    println!("C3 volume orders {pv:?}; C4 inner orders {pi_:?}, outer orders {po:?}");
    assert!(pv.iter().all(|&p| p >= 3.0), "C3: volume orders {pv:?}");
    assert!(pi_.iter().all(|&p| p >= 3.0), "C4: inner area orders {pi_:?}");
    assert!(po.iter().all(|&p| p >= 3.0), "C4: outer area orders {po:?}");
}

#[test]
fn n1_n2_n3_unprojected_hex20_is_second_order() {
    let v_exact = PI * (B * B - A * A) * H / 4.0;
    let (ai_exact, ao_exact) = (PI * A * H / 2.0, PI * B * H / 2.0);
    let (mut ev, mut ei, mut eo) = (Vec::new(), Vec::new(), Vec::new());
    for k in LEVELS {
        let l = level(k, false);
        assert_eq!(l.projected, [0, 0]);
        let n3 = [
            rel(l.volume, v_chord(k)),
            rel(l.area_inner, a_chord(A, k)),
            rel(l.area_outer, a_chord(B, k)),
        ];
        println!(
            "k={k} unprojected: N3 {:.3e} {:.3e} {:.3e}; errors volume {:.5e} inner {:.5e} outer {:.5e}",
            n3[0],
            n3[1],
            n3[2],
            rel(l.volume, v_exact),
            rel(l.area_inner, ai_exact),
            rel(l.area_outer, ao_exact)
        );
        assert!(n3.iter().all(|&e| e <= 1e-12), "N3 k={k}: {n3:?}");
        ev.push(rel(l.volume, v_exact));
        ei.push(rel(l.area_inner, ai_exact));
        eo.push(rel(l.area_outer, ao_exact));
    }
    let (pv, pi_, po) = (orders(&ev), orders(&ei), orders(&eo));
    println!("N1 volume orders {pv:?}; N2 inner {pi_:?}, outer {po:?}");
    let band = |p: &[f64]| p.iter().all(|&x| (1.9..=2.1).contains(&x));
    assert!(band(&pv), "N1: {pv:?}");
    assert!(band(&pi_) && band(&po), "N2: {pi_:?} {po:?}");
}
