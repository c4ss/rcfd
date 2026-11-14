//! F-549 (ADR 0470): check B1, the default Shell4 paths and the opt-in ANDES Shell3 of ADR 0466 give the bits they gave
//! before F-549: a Shell4 plate with every shell load (stiffness, mass, loads, solution, stresses, four frequencies), an ANDES
//! Shell3 plate with the same loads (stiffness, loads, solution, stresses), the refusal of an ANDES Shell3 next to a default
//! Shell4 (its message), and one warped Shell4 element through the element functions. Uses only API that existed at
//! fc026814; the expected digests were printed by this file run on the unchanged base fc026814 (`f549-scratch/b1-base.log`)
//! and written into `EXPECTED` before any library code of F-549.

use rcfd_core::vec3::Vec3;
use rcfd_fem::assemble::stiffness;
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::mass::consistent;
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::shell::{self, ShellOptions, TransverseShear};
use rcfd_fem::shell3::Shell3Membrane;
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{BlockProperty, Constraint, ElementKind, Load, Material, SolidMesh, SolidModel};
use rcfd_linalg::eigen::ResidualTest;

/// Digests of fc026814 in the order of the run, from the base run (`f549-scratch/b1-base.log`).
const EXPECTED: Option<[(&str, u64); 17]> = Some([
    ("Shell4 stiffness", 0x02a0_62c7_9741_d495),
    ("Shell4 mass", 0xd250_0130_eda6_b50d),
    ("Shell4 loads", 0x6174_398f_8c05_0049),
    ("Shell4 solution", 0xc469_e45c_d97a_660f),
    ("Shell4 stresses", 0xbc3d_f8a2_e72f_b6f2),
    ("Shell4 frequencies", 0x51f0_74c2_0e57_fdb1),
    ("ANDES Shell3 stiffness", 0x3b04_4eb1_2bbc_b60f),
    ("ANDES Shell3 loads", 0xa5a9_44a5_60c1_5465),
    ("ANDES Shell3 solution", 0x16a1_05a9_b947_d568),
    ("ANDES Shell3 stresses", 0x5feb_6d0f_91b1_4e31),
    ("ANDES Shell3 next to Shell4 refusal", 0x624b_8aa6_0eb5_85c6),
    ("element stiffness", 0xf9e7_4303_74e8_5519),
    ("element stiffness drilling 0", 0xda26_cec6_99d4_4109),
    ("element stiffness displacement shear", 0xdd76_e467_a7d7_f6e5),
    ("element mass", 0xb7b6_854e_6b72_dab0),
    ("element pressure and body loads", 0xd708_1c86_c4fb_02db),
    ("element thermal load", 0x425e_9c41_bb42_faf3),
]);

/// FNV-1a 64 over the little-endian bytes of the bits of each value.
fn digest(v: &[f64]) -> u64 {
    digest_bytes(v.iter().flat_map(|x| x.to_bits().to_le_bytes()))
}

fn digest_bytes<I: IntoIterator<Item = u8>>(bytes: I) -> u64 {
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    for b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}

fn steel() -> Material {
    Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 1.2e-5).unwrap()
}

/// Nodes of a 4 x 3 grid on `[0, 1] x [0, 0.6]`, slightly warped.
fn nodes(nx: usize, ny: usize) -> Vec<Vec3> {
    let mut v = Vec::new();
    for j in 0..=ny {
        for i in 0..=nx {
            let (x, y) = (i as f64 / nx as f64, 0.6 * j as f64 / ny as f64);
            v.push([x, y, 0.003 * (3.0 * x + 1.0).sin() * (2.0 * y).cos()]);
        }
    }
    v
}

/// A plate of 4 x 3 cells: cells listed in `shell4_cells` are Shell4 (block 0, when any, default property), the others two
/// Shell3 cut alternately with the membrane `membrane`; every shell load.
fn plate(shell4_cells: &[usize], membrane: Shell3Membrane) -> SolidModel {
    let (nx, ny) = (4usize, 3usize);
    let idx = |i: usize, j: usize| j * (nx + 1) + i;
    let nd = nodes(nx, ny);
    let mut quads = Vec::new();
    let mut tris = Vec::new();
    for j in 0..ny {
        for i in 0..nx {
            let c = j * nx + i;
            let q = [idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1)];
            if shell4_cells.contains(&c) {
                quads.extend_from_slice(&q);
            } else if (i + j) % 2 == 0 {
                tris.extend_from_slice(&[q[0], q[1], q[2], q[0], q[2], q[3]]);
            } else {
                tris.extend_from_slice(&[q[0], q[1], q[3], q[1], q[2], q[3]]);
            }
        }
    }
    let n = nd.len();
    let mut mesh = SolidMesh::new(nd.clone()).unwrap();
    let mut b = 0;
    let mut ne = 0;
    if !quads.is_empty() {
        mesh.add_block(ElementKind::Shell4, quads.clone()).unwrap();
        mesh.set_block_property(b, BlockProperty::shell(0.01).unwrap()).unwrap();
        b += 1;
        ne += quads.len() / 4;
    }
    if !tris.is_empty() {
        mesh.add_block(ElementKind::Shell3, tris.clone()).unwrap();
        mesh.set_block_property(b, BlockProperty::shell_with_membrane(0.01, membrane).unwrap())
            .unwrap();
        ne += tris.len() / 3;
    }
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    mesh.add_node_set("all", (0..n).collect()).unwrap();
    mesh.add_node_set("root", (0..=ny).map(|j| idx(0, j)).collect())
        .unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    for comp in 0..6 {
        m.add_constraint(Constraint::displacement("root", comp, 0.0)).unwrap();
    }
    m.add_load(Load::shell_pressure("all", 1000.0)).unwrap();
    m.add_load(Load::shell_pressure_angle_weighted("all", -300.0)).unwrap();
    m.add_load(Load::gravity([0.5, -1.5, -9.81])).unwrap();
    m.add_load(Load::centrifugal([0.0, 0.3, 1.0], [0.4, 0.2, 0.0], 120.0))
        .unwrap();
    let bf = nd.iter().map(|x| [x[0] + 2.0 * x[1], -x[2], 3.0 - x[0]]).collect();
    m.add_load(Load::body_force_nodal(bf)).unwrap();
    let t = nd.iter().map(|x| 300.0 + 10.0 * x[0] + 5.0 * x[1]).collect();
    m.add_load(Load::temperature(t, 290.0)).unwrap();
    m
}

fn stresses_flat(m: &SolidModel, u: &[f64]) -> Vec<f64> {
    let mut s = Vec::new();
    for (e, st) in shell::stresses(m, u).unwrap() {
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
    s
}

#[test]
fn b1_default_shell4_and_the_andes_shell3_give_their_former_bits() {
    let mut got: Vec<(&str, u64)> = Vec::new();
    let all4: Vec<usize> = (0..12).collect();
    let p = plate(&all4, Shell3Membrane::ConstantStrain);
    got.push(("Shell4 stiffness", digest(&stiffness(&p).unwrap().val)));
    got.push(("Shell4 mass", digest(&consistent(&p).unwrap().val)));
    got.push(("Shell4 loads", digest(&nodal_load_vector(&p, &[]).unwrap())));
    let r = solve_static(&p, &StaticOptions::default()).unwrap();
    got.push(("Shell4 solution", digest(r.displacement_values())));
    got.push(("Shell4 stresses", digest(&stresses_flat(&p, r.displacement_values()))));
    let opts = ModalOptions::new(4)
        .tolerance(1e-12)
        .residual_test(ResidualTest::Backward);
    got.push((
        "Shell4 frequencies",
        digest(&modal_analysis(&p, &opts).unwrap().frequencies),
    ));
    let a = plate(&[], Shell3Membrane::AndesOpt);
    got.push(("ANDES Shell3 stiffness", digest(&stiffness(&a).unwrap().val)));
    got.push(("ANDES Shell3 loads", digest(&nodal_load_vector(&a, &[]).unwrap())));
    let r = solve_static(&a, &StaticOptions::default()).unwrap();
    got.push(("ANDES Shell3 solution", digest(r.displacement_values())));
    got.push(("ANDES Shell3 stresses", digest(&stresses_flat(&a, r.displacement_values()))));
    let mx = plate(&[0, 5, 10, 11], Shell3Membrane::AndesOpt);
    let msg = format!("{:?}", solve_static(&mx, &StaticOptions::default()).err());
    println!("B1 refusal: {msg}");
    got.push(("ANDES Shell3 next to Shell4 refusal", digest_bytes(msg.bytes())));
    let x: [Vec3; 4] = [[0.3, -0.2, 0.5], [1.4, 0.0, 0.52], [1.2, 0.75, 0.47], [0.4, 0.85, 0.515]];
    let mat = steel();
    got.push((
        "element stiffness",
        digest(shell::element_stiffness(&x, 0.02, &mat).unwrap().values()),
    ));
    let o = ShellOptions::default().drilling(0.0);
    got.push((
        "element stiffness drilling 0",
        digest(shell::element_stiffness_with(&x, 0.02, &mat, &o).unwrap().values()),
    ));
    let o = ShellOptions::default().shear(TransverseShear::FullIntegration);
    got.push((
        "element stiffness displacement shear",
        digest(shell::element_stiffness_with(&x, 0.02, &mat, &o).unwrap().values()),
    ));
    got.push((
        "element mass",
        digest(shell::element_mass(&x, 0.02, &mat).unwrap().values()),
    ));
    let mut loads = shell::pressure_load_vector(&x, 1234.0).unwrap().to_vec();
    loads.extend_from_slice(&shell::body_load_vector(&x, 0.02, |p| Ok([p[0], -2.0 * p[1], 3.0 + p[2]])).unwrap());
    got.push(("element pressure and body loads", digest(&loads)));
    got.push((
        "element thermal load",
        digest(&shell::thermal_load_vector(&x, 0.02, &mat, &[40.0, -10.0, 25.0, 5.0]).unwrap()),
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
