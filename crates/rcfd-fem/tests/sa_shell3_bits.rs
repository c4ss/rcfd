//! F-419 (ADR 0454): check B1, models without Shell3 give the bits they gave before F-419: a Shell4 plate (stiffness, mass, the
//! load vector of pressure, gravity, centrifugal, nodal body force and temperature, the static solution, the stresses, four
//! frequencies), a beam cantilever (stiffness, solution) and a Hex8 box (stiffness, mass, gravity load, solution). Uses only API
//! that existed at 8c1a4db2; the expected digests were printed by this file run on the unchanged base 8c1a4db2
//! (`f419-scratch/b1-base.log`) and written into `EXPECTED` before any library code of F-419.

use rcfd_fem::assemble::stiffness;
use rcfd_fem::builders::hex_box;
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::mass::consistent;
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::shell;
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{BeamSection, BlockProperty, Constraint, ElementKind, Load, Material, SolidMesh, SolidModel};
use rcfd_linalg::eigen::ResidualTest;

/// Digests of 8c1a4db2 in the order of the run, from the base run (`f419-scratch/b1-base.log`).
const EXPECTED: Option<[(&str, u64); 13]> = Some([
    ("Shell4 stiffness", 0x02a0_62c7_9741_d495),
    ("Shell4 mass", 0xd250_0130_eda6_b50d),
    ("Shell4 loads", 0x84b2_7615_25eb_ddbb),
    ("Shell4 solution", 0x9e11_69ea_0080_f8ef),
    ("Shell4 stresses", 0x711d_6a81_c460_e692),
    ("Shell4 frequencies", 0x51f0_74c2_0e57_fdb1),
    ("Beam2 stiffness", 0xd516_55cf_ecfd_9e80),
    ("Beam2 solution", 0x818f_c7d7_0eaf_9026),
    ("Hex8 stiffness", 0x6b88_5b7d_a0b2_1378),
    ("Hex8 mass", 0x10cf_436e_9a26_29e8),
    ("Hex8 loads", 0xdac7_f474_1340_357b),
    ("Hex8 solution", 0xcf42_ebd0_a9d3_6353),
    ("Hex8 reactions", 0x346f_43e4_c730_cac8),
]);

/// FNV-1a 64 over the little-endian bytes of the bits of each value.
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

fn steel() -> Material {
    Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 1.2e-5).unwrap()
}

/// A Shell4 plate of 4 x 3 elements on `[0, 1] x [0, 0.6]`, slightly warped, clamped at `x = 0`, with every shell load.
fn shell4_plate() -> SolidModel {
    let (nx, ny) = (4usize, 3usize);
    let idx = |i: usize, j: usize| j * (nx + 1) + i;
    let mut nodes = Vec::new();
    for j in 0..=ny {
        for i in 0..=nx {
            let (x, y) = (i as f64 / nx as f64, 0.6 * j as f64 / ny as f64);
            nodes.push([x, y, 0.003 * (3.0 * x + 1.0).sin() * (2.0 * y).cos()]);
        }
    }
    let mut conn = Vec::new();
    for j in 0..ny {
        for i in 0..nx {
            conn.extend_from_slice(&[idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1)]);
        }
    }
    let n = nodes.len();
    let mut mesh = SolidMesh::new(nodes.clone()).unwrap();
    mesh.add_block(ElementKind::Shell4, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::shell(0.01).unwrap()).unwrap();
    mesh.add_element_set("all", (0..nx * ny).collect()).unwrap();
    mesh.add_node_set("all", (0..n).collect()).unwrap();
    mesh.add_node_set("root", (0..=ny).map(|j| idx(0, j)).collect())
        .unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    for comp in 0..6 {
        m.add_constraint(Constraint::displacement("root", comp, 0.0)).unwrap();
    }
    m.add_load(Load::shell_pressure("all", 1000.0)).unwrap();
    m.add_load(Load::gravity([0.5, -1.5, -9.81])).unwrap();
    m.add_load(Load::centrifugal([0.0, 0.3, 1.0], [0.4, 0.2, 0.0], 120.0))
        .unwrap();
    let b = nodes.iter().map(|x| [x[0] + 2.0 * x[1], -x[2], 3.0 - x[0]]).collect();
    m.add_load(Load::body_force_nodal(b)).unwrap();
    let t = nodes.iter().map(|x| 300.0 + 10.0 * x[0] + 5.0 * x[1]).collect();
    m.add_load(Load::temperature(t, 290.0)).unwrap();
    m
}

fn beam_cantilever() -> SolidModel {
    let nodes: Vec<[f64; 3]> = (0..5).map(|i| [0.25 * i as f64, 0.0, 0.0]).collect();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Beam2, vec![0, 1, 1, 2, 2, 3, 3, 4])
        .unwrap();
    let section = BeamSection::new(0.012, 3.0e-6, 5.0e-6, 4.0e-6, 0.85, 0.75).unwrap();
    mesh.set_block_property(0, BlockProperty::beam(section, [0.0, 0.0, 1.0]).unwrap())
        .unwrap();
    mesh.add_element_set("all", (0..4).collect()).unwrap();
    mesh.add_node_set("root", vec![0]).unwrap();
    mesh.add_node_set("tip", vec![4]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    for comp in 0..6 {
        m.add_constraint(Constraint::displacement("root", comp, 0.0)).unwrap();
    }
    m.add_load(Load::nodal_force("tip", [0.0, 100.0, -50.0])).unwrap();
    m
}

fn hex8_box() -> SolidModel {
    let mesh = hex_box([0.0; 3], [2.0, 1.0, 1.0], [2, 1, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    for comp in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", comp, 0.0)).unwrap();
    }
    m.add_load(Load::gravity([0.5, -1.5, -9.81])).unwrap();
    m
}

#[test]
fn b1_models_without_shell3_give_their_former_bits() {
    let mut got: Vec<(&str, u64)> = Vec::new();
    let p = shell4_plate();
    got.push(("Shell4 stiffness", digest(&stiffness(&p).unwrap().val)));
    got.push(("Shell4 mass", digest(&consistent(&p).unwrap().val)));
    got.push(("Shell4 loads", digest(&nodal_load_vector(&p, &[]).unwrap())));
    let r = solve_static(&p, &StaticOptions::default()).unwrap();
    got.push(("Shell4 solution", digest(r.displacement_values())));
    let mut s = Vec::new();
    for (e, st) in shell::stresses(&p, r.displacement_values()).unwrap() {
        s.push(e as f64);
        for a in st.axes {
            s.extend_from_slice(&a);
        }
        s.extend_from_slice(&st.membrane);
        s.extend_from_slice(&st.bending);
        s.extend_from_slice(&st.shear);
        s.extend_from_slice(&st.top);
        s.extend_from_slice(&st.bottom);
    }
    got.push(("Shell4 stresses", digest(&s)));
    let opts = ModalOptions::new(4)
        .tolerance(1e-12)
        .residual_test(ResidualTest::Backward);
    got.push((
        "Shell4 frequencies",
        digest(&modal_analysis(&p, &opts).unwrap().frequencies),
    ));
    let b = beam_cantilever();
    got.push(("Beam2 stiffness", digest(&stiffness(&b).unwrap().val)));
    let r = solve_static(&b, &StaticOptions::default()).unwrap();
    got.push(("Beam2 solution", digest(r.displacement_values())));
    let h = hex8_box();
    got.push(("Hex8 stiffness", digest(&stiffness(&h).unwrap().val)));
    got.push(("Hex8 mass", digest(&consistent(&h).unwrap().val)));
    got.push(("Hex8 loads", digest(&nodal_load_vector(&h, &[]).unwrap())));
    let r = solve_static(&h, &StaticOptions::default()).unwrap();
    got.push(("Hex8 solution", digest(r.displacement_values())));
    got.push((
        "Hex8 reactions",
        digest(r.reactions().expect("a static result has reactions")),
    ));
    for (name, d) in &got {
        println!("B1 {name}: {d:#018x}");
    }
    let expected = EXPECTED.expect("EXPECTED is written from the base run");
    assert_eq!(got.len(), expected.len());
    for ((name, d), (en, e)) in got.iter().zip(expected.iter()) {
        assert_eq!(name, en);
        assert_eq!(d, e, "B1 {name}: {d:#018x} against {e:#018x}");
    }
}
