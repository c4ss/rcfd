//! F-574 (ADR 0537): the opt-in warning for the constant-strain Shell3 membrane on warped meshes, checks W1 to W14 as registered.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::hex_box;
use rcfd_fem::shell3::{warped_membrane_report, Shell3Membrane, WARP_ANGLE_DEGREES};
use rcfd_fem::{BlockProperty, ElementKind, FemError, Material, SolidMesh, SolidModel};

const CHORD: f64 = 0.05;
const THICK: f64 = 0.001;
const SPAN: f64 = 0.15;
const TOL: f64 = 1e-8;

fn steel() -> Material {
    Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 0.0).unwrap()
}

/// Nodes and connectivity of the ADR 0473 strip; `shift` moves it along `x` (a second strip on disjoint nodes), `base` is the
/// first node index.
fn strip_parts(nch: usize, nsp: usize, twist_deg: f64, tri: bool, shift: f64, base: usize) -> (Vec<Vec3>, Vec<usize>) {
    let idx = |i: usize, j: usize| base + j * (nch + 1) + i;
    let mut nodes: Vec<Vec3> = Vec::new();
    for j in 0..=nsp {
        for i in 0..=nch {
            let r = SPAN * j as f64 / nsp as f64;
            let s = -0.5 * CHORD + CHORD * i as f64 / nch as f64;
            let th = twist_deg.to_radians() * r / SPAN;
            nodes.push([r + shift, s * th.cos(), s * th.sin()]);
        }
    }
    let mut conn = Vec::new();
    for j in 0..nsp {
        for i in 0..nch {
            let (a, b, c, d) = (idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1));
            if tri {
                conn.extend_from_slice(&[a, b, c, a, c, d]);
            } else {
                conn.extend_from_slice(&[a, b, c, d]);
            }
        }
    }
    (nodes, conn)
}

fn model_of(mesh: SolidMesh) -> SolidModel {
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    m
}

fn strip(nch: usize, nsp: usize, twist_deg: f64, kind: ElementKind, property: BlockProperty) -> SolidModel {
    let (nodes, conn) = strip_parts(nch, nsp, twist_deg, kind == ElementKind::Shell3, 0.0, 0);
    let ne = if kind == ElementKind::Shell3 {
        2 * nch * nsp
    } else {
        nch * nsp
    };
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(kind, conn).unwrap();
    mesh.set_block_property(0, property).unwrap();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    model_of(mesh)
}

fn shell3(twist_deg: f64) -> SolidModel {
    strip(
        12,
        36,
        twist_deg,
        ElementKind::Shell3,
        BlockProperty::shell(THICK).unwrap(),
    )
}

fn membrane(m: Shell3Membrane) -> BlockProperty {
    BlockProperty::shell_with_membrane(THICK, m).unwrap()
}

fn t() -> f64 {
    WARP_ANGLE_DEGREES
}

#[test]
fn w1_w2_the_twisted_strip_fires_at_30_and_15_degrees() {
    for (twist, max) in [(30.0, 1.1784282153278391), (15.0, 0.5892452638404285)] {
        let r = warped_membrane_report(&shell3(twist), t()).unwrap();
        assert_eq!(r.blocks.len(), 1, "twist {twist}");
        assert_eq!(r.blocks[0].block, 0);
        assert_eq!(r.blocks[0].n_edges, 1248, "twist {twist}");
        assert!(
            (r.blocks[0].max_angle - max).abs() <= TOL,
            "twist {twist}: {}",
            r.blocks[0].max_angle
        );
    }
}

#[test]
fn w3_the_flat_strip_does_not_fire() {
    let m = shell3(0.0);
    assert!(warped_membrane_report(&m, t()).unwrap().is_empty());
    assert!(warped_membrane_report(&m, 1e-300).unwrap().is_empty());
}

#[test]
fn w4_the_threshold_on_the_refinement_records() {
    let p = || BlockProperty::shell(THICK).unwrap();
    let r = warped_membrane_report(&strip(24, 72, 30.0, ElementKind::Shell3, p()), t()).unwrap();
    assert_eq!(r.blocks.len(), 1);
    assert_eq!(r.blocks[0].n_edges, 5088);
    assert!((r.blocks[0].max_angle - 0.5892452638410471).abs() <= TOL);
    for (nch, nsp, tw) in [(24, 72, 15.0), (48, 144, 15.0), (48, 144, 30.0)] {
        let r = warped_membrane_report(&strip(nch, nsp, tw, ElementKind::Shell3, p()), t()).unwrap();
        assert!(r.is_empty(), "{nch} x {nsp} at {tw}: {:?}", r.blocks);
    }
}

#[test]
fn w5_the_message_names_the_andes_membrane() {
    let r = warped_membrane_report(&shell3(30.0), t()).unwrap();
    let msgs = r.messages();
    assert_eq!(msgs.len(), 1);
    let expected = "block 0 (Shell3, constant-strain membrane): 1248 edges shared with another Shell3 facet meet at a normal \
angle above 0.4 degrees (largest 1.1784 degrees); the constant-strain membrane follows a strain that varies over a facet only \
with parasitic in-plane shear and may overstiffen a warped shell (ADR 0483: 4.05 percent on the first torsion of the twisted \
strip of ADR 0473); the ANDES membrane of ADR 0466 (BlockProperty::shell_with_membrane with Shell3Membrane::AndesOpt) removes it";
    assert_eq!(msgs[0], expected);
    for s in ["ADR 0466", "AndesOpt", "ADR 0483"] {
        assert!(msgs[0].contains(s));
    }
}

#[test]
fn w6_andes_and_allman_blocks_are_not_reported() {
    for mem in [Shell3Membrane::AndesOpt, Shell3Membrane::Allman] {
        let m = strip(12, 36, 30.0, ElementKind::Shell3, membrane(mem));
        assert!(warped_membrane_report(&m, t()).unwrap().is_empty(), "{mem:?}");
    }
}

#[test]
fn w7_a_shell4_strip_is_not_reported() {
    let m = strip(12, 36, 30.0, ElementKind::Shell4, BlockProperty::shell(THICK).unwrap());
    assert!(warped_membrane_report(&m, t()).unwrap().is_empty());
}

#[test]
fn w8_a_mixed_model_reports_the_constant_strain_block_only() {
    let (n0, c0) = strip_parts(12, 36, 30.0, true, 0.0, 0);
    let base = n0.len();
    let (n1, c1) = strip_parts(12, 36, 30.0, true, 1.0, base);
    let mut nodes = n0;
    nodes.extend(n1);
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Shell3, c0).unwrap();
    mesh.add_block(ElementKind::Shell3, c1).unwrap();
    mesh.set_block_property(0, BlockProperty::shell(THICK).unwrap())
        .unwrap();
    mesh.set_block_property(1, membrane(Shell3Membrane::AndesOpt)).unwrap();
    mesh.add_element_set("all", (0..4 * 12 * 36).collect()).unwrap();
    let r = warped_membrane_report(&model_of(mesh), t()).unwrap();
    assert_eq!(r.blocks.len(), 1);
    assert_eq!(r.blocks[0].block, 0);
    assert_eq!(r.blocks[0].n_edges, 1248);
}

/// Triangles on the shared edge `(0,0,0)-(1,0,0)`, apex `k` at `(0.5, cos a_k, sin a_k)`, one block. The first is `(0, 1, apex)`;
/// the others are `(1, 0, apex)` (the shared edge traversed backwards: a consistent order) or, with `same_direction`, `(0, 1,
/// apex)` (the edge traversed the same way: an inconsistent order).
fn fold(angles: &[f64], same_direction: bool) -> SolidModel {
    let mut nodes: Vec<Vec3> = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]];
    let mut conn = Vec::new();
    for (k, a) in angles.iter().enumerate() {
        let a = a.to_radians();
        nodes.push([0.5, a.cos(), a.sin()]);
        let apex = 2 + k;
        if k == 0 || same_direction {
            conn.extend_from_slice(&[0, 1, apex]);
        } else {
            conn.extend_from_slice(&[1, 0, apex]);
        }
    }
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Shell3, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::shell(THICK).unwrap())
        .unwrap();
    mesh.add_element_set("all", (0..angles.len()).collect()).unwrap();
    model_of(mesh)
}

#[test]
fn w9_a_fold_reads_its_angle_for_either_node_order() {
    // The apexes at 0 and 120 degrees about the shared edge: with the second triangle ordered (1, 0, apex) the normals of the
    // two facets meet at 60 degrees (the normal of (0, 1, apex) at angle a is (0, -sin a, cos a) turned; the second is reversed
    // by the order and turned by 120 degrees, which leaves 180 - 120 = 60).
    let r = warped_membrane_report(&fold(&[0.0, 120.0], false), t()).unwrap();
    assert_eq!(r.blocks.len(), 1);
    assert_eq!(r.blocks[0].n_edges, 1);
    assert!((r.blocks[0].max_angle - 60.0).abs() <= TOL, "{}", r.blocks[0].max_angle);
    // The inconsistent order, both traversing the edge 0 -> 1: the same angle after the flip (60 for the reversed normal, 120
    // unflipped).
    let r = warped_membrane_report(&fold(&[0.0, 120.0], true), t()).unwrap();
    assert_eq!(r.blocks.len(), 1);
    assert!((r.blocks[0].max_angle - 60.0).abs() <= TOL, "{}", r.blocks[0].max_angle);
}

#[test]
fn w10_a_three_facet_edge_does_not_count() {
    let r = warped_membrane_report(&fold(&[0.0, 120.0, 240.0], false), t()).unwrap();
    assert!(r.is_empty());
}

#[test]
fn w11_the_threshold_is_strict() {
    let m = shell3(30.0);
    let max = warped_membrane_report(&m, t()).unwrap().blocks[0].max_angle;
    assert!(warped_membrane_report(&m, max).unwrap().is_empty());
    let below = f64::from_bits(max.to_bits() - 1);
    assert_eq!(warped_membrane_report(&m, below).unwrap().blocks.len(), 1);
}

#[test]
fn w12_the_threshold_is_validated() {
    let m = shell3(30.0);
    for bad in [0.0, -1.0, f64::NAN, f64::INFINITY, 180.0001] {
        match warped_membrane_report(&m, bad) {
            Err(FemError::InvalidValue { key: "threshold", .. }) => {}
            other => panic!("threshold {bad}: {other:?}"),
        }
    }
    assert!(warped_membrane_report(&m, 180.0).unwrap().is_empty());
}

#[test]
fn w13_models_without_shell3_give_an_empty_report() {
    let s4 = strip(12, 36, 30.0, ElementKind::Shell4, BlockProperty::shell(THICK).unwrap());
    assert!(warped_membrane_report(&s4, t()).unwrap().is_empty());
    let hex = SolidModel::new(hex_box([0.0; 3], [1.0; 3], [2, 2, 2]).unwrap());
    assert!(warped_membrane_report(&hex, t()).unwrap().is_empty());
}

#[test]
fn w14_the_constant_and_the_threshold_of_the_report() {
    assert_eq!(WARP_ANGLE_DEGREES, 0.4);
    assert_eq!(
        warped_membrane_report(&shell3(30.0), WARP_ANGLE_DEGREES)
            .unwrap()
            .threshold,
        0.4
    );
}
