//! F-521 (ADR 0437): the bytes of the solid output for a beam model (no shell). Procedure B0 records the FNV-1a 64 hash and
//! the length of `solid_vtu_string` for the beam frame fixture F of ADR 0423 at the base commit; check B1b compares against them.

use rcfd_core::vec3::Vec3;
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{
    BeamSection, BlockProperty, Constraint, ElementKind, Load, Material, SolidMesh, SolidModel, SolidResult,
};
use rcfd_post::solid_output::solid_vtu_string;

fn frame_model() -> SolidModel {
    let nodes: Vec<Vec3> = vec![
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [1.0, 1.0, 1.0],
        [2.0, 2.0, 2.0],
    ];
    let v = [1.0, 2.0, 3.0];
    let section = BeamSection::new(0.012, 3.0e-6, 5.0e-6, 4.0e-6, 0.85, 0.75).unwrap();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Beam2, vec![0, 1, 1, 2]).unwrap();
    mesh.add_block(ElementKind::Beam2, vec![2, 3]).unwrap();
    for b in 0..2 {
        mesh.set_block_property(b, BlockProperty::beam(section, v).unwrap())
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

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
}

/// (fixture name, hash, length): F with reactions, then without.
fn fixtures() -> Vec<(String, u64, usize)> {
    let model = frame_model();
    let res = solve_static(&model, &StaticOptions::default()).unwrap();
    let plain = SolidResult::new(res.dofs().clone(), res.displacement_values().to_vec()).unwrap();
    let mut out = Vec::new();
    for (tag, r) in [("frame reactions", &res), ("frame no reactions", &plain)] {
        let s = solid_vtu_string(&model, r).unwrap();
        out.push((tag.to_string(), fnv1a64(s.as_bytes()), s.len()));
    }
    out
}

#[test]
fn b0_no_shell_hashes_at_the_base() {
    let f = fixtures();
    assert_eq!(f.len(), 2);
    for (name, h, len) in &f {
        assert!(*len > 0);
        println!("B0: {name}: hash 0x{h:016x}, length {len}");
    }
}

/// B1b: the values recorded by B0 at the base commit af0fd223 (ADR 0437 addendum 0).
#[test]
fn b1b_no_shell_bytes_unchanged() {
    let recorded: [(&str, u64, usize); 2] = [
        ("frame reactions", 0x34ff_07d0_75a2_933a, 2637),
        ("frame no reactions", 0xde67_d058_7843_b447, 2231),
    ];
    let now = fixtures();
    let mut missed = Vec::new();
    for ((name, h, len), (rn, rh, rl)) in now.iter().zip(recorded) {
        assert_eq!(name, rn, "fixture order");
        let ok = *h == rh && *len == rl;
        println!(
            "B1b: {name}: hash 0x{h:016x} length {len} against 0x{rh:016x} {rl}: {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            missed.push(name.clone());
        }
    }
    assert!(missed.is_empty(), "B1b missed: {missed:?}");
}
