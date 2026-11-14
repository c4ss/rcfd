//! F-516 (ADR 0430): the beam output refuses a result with the model's number of unknowns in another layout, naming the
//! first node and both counts. Checks C1 to C5 of the registration (C6, C7 are the existing test files, unedited).

use rcfd_core::vec3::Vec3;
use rcfd_fem::{
    BeamSection, BlockProperty, Constraint, DofMap, ElementKind, Load, Material, SolidMesh, SolidModel, SolidResult,
};
use rcfd_post::solid_output::{beam_fields, solid_vtu_string};

fn section1() -> BeamSection {
    BeamSection::new(0.012, 3.0e-6, 5.0e-6, 4.0e-6, 0.85, 0.75).unwrap()
}

fn frame_model() -> SolidModel {
    let nodes: Vec<Vec3> = vec![
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [1.0, 1.0, 1.0],
        [2.0, 2.0, 2.0],
    ];
    let v = [1.0, 2.0, 3.0];
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Beam2, vec![0, 1, 1, 2]).unwrap();
    mesh.add_block(ElementKind::Beam2, vec![2, 3]).unwrap();
    for b in 0..2 {
        mesh.set_block_property(b, BlockProperty::beam(section1(), v).unwrap())
            .unwrap();
    }
    mesh.add_element_set("all", vec![0, 1, 2]).unwrap();
    mesh.add_node_set("root", vec![0]).unwrap();
    mesh.add_node_set("tip", vec![3]).unwrap();
    mesh.add_node_set("lone", vec![4]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(2.1e11, 0.3, 7850.0, 0.0).unwrap())
        .unwrap();
    for c in 0..6 {
        m.add_constraint(Constraint::displacement("root", c, 0.0)).unwrap();
    }
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("lone", c, 0.0)).unwrap();
    }
    m.add_load(Load::nodal_force("tip", [1200.0, -2500.0, 3700.0])).unwrap();
    m.add_load(Load::nodal_moment("tip", [450.0, -300.0, 200.0])).unwrap();
    m
}

fn result_with(counts: &[usize]) -> SolidResult {
    let dofs = DofMap::from_counts(counts);
    let n = dofs.n_equations();
    SolidResult::new(dofs, vec![0.0; n]).unwrap()
}

/// The refusal text of both entry points, required equal.
fn refusal(model: &SolidModel, r: &SolidResult) -> String {
    let a = solid_vtu_string(model, r).unwrap_err().to_string();
    let b = beam_fields(model, r).unwrap_err().to_string();
    assert_eq!(a, b);
    a
}

#[test]
fn c1_first_differing_node_named() {
    let m = frame_model();
    let e = refusal(&m, &result_with(&[6, 6, 6, 3, 6]));
    println!("C1: {e}");
    assert!(
        e.contains("node 3 has 3 unknowns in the result and 6 in the model"),
        "{e}"
    );
    assert!(e.contains("displacement"), "{e}");
    assert!(!e.contains("expected 27"), "{e}");
}

#[test]
fn c2_node_zero_named() {
    let m = frame_model();
    let e = refusal(&m, &result_with(&[3, 6, 6, 6, 6]));
    println!("C2: {e}");
    assert!(
        e.contains("node 0 has 3 unknowns in the result and 6 in the model"),
        "{e}"
    );
}

#[test]
fn c3_own_layout_accepted() {
    let m = frame_model();
    let r = result_with(&[6, 6, 6, 6, 3]);
    assert!(solid_vtu_string(&m, &r).is_ok());
    assert!(beam_fields(&m, &r).is_ok());
}

#[test]
fn c4_node_count_named() {
    let m = frame_model();
    let r = SolidResult::new(DofMap::uniform(9, 3), vec![0.0; 27]).unwrap();
    let e = refusal(&m, &r);
    println!("C4: {e}");
    assert!(e.contains("the result numbers 9 nodes and the model 5"), "{e}");
    assert!(e.contains("displacement"), "{e}");
}

#[test]
fn c5_wrong_length_keeps_its_text() {
    let m = frame_model();
    let r = SolidResult::new(DofMap::uniform(5, 3), vec![0.0; 15]).unwrap();
    let e = refusal(&m, &r);
    println!("C5: {e}");
    assert!(e.contains("length 15") && e.contains("expected 27"), "{e}");
    assert!(!e.contains("unknowns in the result and"), "{e}");
}
