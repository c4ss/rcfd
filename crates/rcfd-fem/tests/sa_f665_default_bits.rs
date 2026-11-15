//! F-665 (ADR 0589): check N1 (record of D5), the existing beam paths give the bits they gave before F-665: on the planar and
//! the oblique Beck columns of Beam2 elements of D3, `solve_static` with the dead tip force, `modal_analysis`,
//! `geometric::prestressed_modes` and `buckling::factors`. Uses only API that existed at 06b66e35; the expected digests were
//! printed by this file run on the unchanged base 06b66e35 (`f665-scratch/n1-base.log`) and written into `EXPECTED` before
//! any library code of F-665.

use rcfd_core::vec3::Vec3;
use rcfd_fem::beam;
use rcfd_fem::buckling::{factors, BucklingOptions};
use rcfd_fem::geometric::{prestressed_modes, Prestress};
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{BeamSection, BlockProperty, Constraint, ElementKind, Load, Material, SolidMesh, SolidModel};
use rcfd_linalg::eigen::ResidualTest;

/// Digests of 06b66e35 in the order of the run (`f665-scratch/n1-base.log`); `None` prints them.
const EXPECTED: Option<[(&str, u64); 8]> = Some([
    ("planar static", 0xab39_1f66_6ee4_82f6),
    ("planar modal", 0xfd7c_7a7f_8a20_4c49),
    ("planar prestressed", 0xb0db_0434_3eb4_14f4),
    ("planar buckling", 0x31a7_acb4_7d14_9383),
    ("oblique static", 0x855f_82f5_00a1_15aa),
    ("oblique modal", 0xf979_9672_ce18_acce),
    ("oblique prestressed", 0xf70d_10ee_9f83_b93f),
    ("oblique buckling", 0x59aa_8e9f_c934_1635),
]);

fn digest(v: &[f64]) -> u64 {
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    for x in v {
        for b in x.to_bits().to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01B3);
        }
    }
    h
}

const E: f64 = 2.0e11;
const N: usize = 40;

fn column(axis: Vec3, section: BeamSection, orientation: Vec3) -> SolidModel {
    let nodes: Vec<Vec3> = (0..=N)
        .map(|i| {
            let s = i as f64 / N as f64;
            [s * axis[0], s * axis[1], s * axis[2]]
        })
        .collect();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    let conn: Vec<usize> = (0..N).flat_map(|i| [i, i + 1]).collect();
    mesh.add_block(ElementKind::Beam2, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::beam(section, orientation).unwrap())
        .unwrap();
    mesh.add_element_set("all", (0..N).collect()).unwrap();
    mesh.add_node_set("all", (0..=N).collect()).unwrap();
    mesh.add_node_set("x0", vec![0]).unwrap();
    mesh.add_node_set("xl", vec![N]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(E, 0.3, 7850.0, 0.0).unwrap())
        .unwrap();
    for c in 0..6 {
        m.add_constraint(Constraint::displacement("x0", c, 0.0)).unwrap();
    }
    m
}

/// The planar column of D3 and its tip force.
fn planar() -> (SolidModel, Vec3) {
    let b = 0.01;
    let s = beam::rectangle_section(b, b, 0.1406 * b.powi(4), 5.0 / 6.0).unwrap();
    let p = 20.0 * E * s.i_z();
    let mut m = column([1.0, 0.0, 0.0], s, [0.0, 1.0, 0.0]);
    for c in [2, 3, 4] {
        m.add_constraint(Constraint::displacement("all", c, 0.0)).unwrap();
    }
    (m, [-p, 0.0, 0.0])
}

/// The oblique 3-D column of D3 and its tip force.
fn oblique() -> (SolidModel, Vec3) {
    let (b, h) = (0.02, 0.01);
    let s = beam::rectangle_section(b, h, 0.229 * b * h * h * h, 5.0 / 6.0).unwrap();
    let p = 20.0 * E * s.i_y();
    let a = [1.0 / 3.0, 2.0 / 3.0, 2.0 / 3.0];
    let m = column(a, s, [0.0, 0.0, 1.0]);
    (m, [-p * a[0], -p * a[1], -p * a[2]])
}

fn modal_digest(r: &rcfd_fem::modal::ModalResult) -> u64 {
    let mut v = r.eigenvalues.clone();
    for x in &r.modes {
        v.extend_from_slice(x);
    }
    v.extend_from_slice(&r.residuals);
    v.push(r.iterations as f64);
    digest(&v)
}

#[test]
fn n1_default_bits() {
    let mut got: Vec<(&str, u64)> = Vec::new();
    let opts = ModalOptions::new(4).residual_test(ResidualTest::Backward);
    for (tag, (m, f)) in [("planar", planar()), ("oblique", oblique())] {
        let mut md = m.clone();
        md.add_load(Load::nodal_force("xl", f)).unwrap();
        let s = solve_static(&md, &StaticOptions::default()).unwrap();
        let pre = Prestress::from_static(&md, &s).unwrap();
        let names: [&str; 4] = if tag == "planar" {
            ["planar static", "planar modal", "planar prestressed", "planar buckling"]
        } else {
            [
                "oblique static",
                "oblique modal",
                "oblique prestressed",
                "oblique buckling",
            ]
        };
        got.push((names[0], digest(s.displacement_values())));
        let r = modal_analysis(&m, &opts).unwrap();
        println!("{tag} modal {:?}", r.eigenvalues);
        got.push((names[1], modal_digest(&r)));
        let r = prestressed_modes(&m, &pre.scaled(0.05), &opts).unwrap();
        println!("{tag} prestressed {:?}", r.eigenvalues);
        got.push((names[2], modal_digest(&r)));
        let r = factors(&m, &pre, &BucklingOptions::new(2)).unwrap();
        println!("{tag} buckling {:?}", r.factors);
        let mut v = r.factors.clone();
        for x in &r.modes {
            v.extend_from_slice(x);
        }
        v.extend_from_slice(&r.residuals);
        v.push(r.iterations as f64);
        v.push(r.sturm.counted as f64);
        got.push((names[3], digest(&v)));
    }
    for (name, d) in &got {
        println!("(\"{name}\", 0x{d:016x}),");
    }
    match EXPECTED {
        Some(e) => assert_eq!(got, e.to_vec()),
        None => panic!("no expected digests yet (the base run prints them)"),
    }
}
