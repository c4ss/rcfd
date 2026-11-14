//! F-392 (ADR 0432): the row of check N2 for `rcfd-post`: the solid output refuses the result of a shell model (limit L1),
//! naming the node and its six components.

use rcfd_fem::constraint::Prescribed;
use rcfd_fem::loads::NodalForceAt;
use rcfd_fem::solve::{solve_static_with, StaticOptions};
use rcfd_fem::{BlockProperty, ElementKind, Material, SolidMesh, SolidModel};
use rcfd_post::solid_output::solid_vtu_string;

fn strip() -> SolidModel {
    let mut nodes = Vec::new();
    for j in 0..2 {
        for i in 0..5 {
            nodes.push([0.25 * i as f64, 0.1 * j as f64, 0.0]);
        }
    }
    let mut mesh = SolidMesh::new(nodes).unwrap();
    let mut conn = Vec::new();
    for i in 0..4 {
        conn.extend_from_slice(&[i, i + 1, i + 6, i + 5]);
    }
    mesh.add_block(ElementKind::Shell4, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::shell(0.01).unwrap()).unwrap();
    mesh.add_element_set("all", (0..4).collect()).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(2e11, 0.3, 7850.0, 0.0).unwrap())
        .unwrap();
    m
}

#[test]
fn n2_shell_result_refused_by_the_solid_output() {
    let m = strip();
    let mut pres = Vec::new();
    for k in [0, 5] {
        for c in 0..6 {
            pres.push(Prescribed::new(k, c, 0.0));
        }
    }
    let r = solve_static_with(
        &m,
        &pres,
        &[NodalForceAt::new(4, [0.0, 0.0, 1.0])],
        &StaticOptions::default(),
    )
    .unwrap();
    let e = solid_vtu_string(&m, &r).expect_err("a shell result is refused");
    let msg = e.to_string();
    println!("N2 rcfd-post: {msg}");
    assert!(msg.contains("node 0 has 6 components"), "{msg}");
}
