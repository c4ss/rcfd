//! F-539 (ADR 0466): check B1, the default Shell3 (constant-strain membrane) and Shell4 paths give the bits they gave before
//! F-539: a Shell3 plate with every shell load (stiffness, mass, loads, solution, stresses, four frequencies), a mixed Shell3 +
//! Shell4 plate (stiffness, solution, stresses), a Shell4 plate (stiffness, solution) and one Shell3 element through the
//! element functions. Uses only API that existed at d0439898; the expected digests were printed by this file run on the
//! unchanged base d0439898 (`f539-scratch/b1-base.log`) and written into `EXPECTED` before any library code of F-539.

use rcfd_core::vec3::Vec3;
use rcfd_fem::assemble::stiffness;
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::mass::consistent;
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::shell::{self, TransverseShear};
use rcfd_fem::shell3::{self, Shell3Options};
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{BlockProperty, Constraint, ElementKind, Load, Material, SolidMesh, SolidModel};
use rcfd_linalg::eigen::ResidualTest;

/// Digests of d0439898 in the order of the run, from the base run (`f539-scratch/b1-base.log`).
const EXPECTED: Option<[(&str, u64); 15]> = Some([
    ("Shell3 stiffness", 0x7297_0a34_d041_6d31),
    ("Shell3 mass", 0x3fa5_bea4_6cb7_3adc),
    ("Shell3 loads", 0x5cde_411f_192f_df78),
    ("Shell3 solution", 0xa84e_0220_1e75_6466),
    ("Shell3 stresses", 0x4475_c030_6a7c_81cf),
    ("Shell3 frequencies", 0x60c4_751a_62d4_71b5),
    ("mixed stiffness", 0x55d5_c034_caf3_e102),
    ("mixed solution", 0xdb41_fb08_b8c3_ec94),
    ("mixed stresses", 0x86cd_e38d_e1d1_2e61),
    ("Shell4 stiffness", 0x02a0_62c7_9741_d495),
    ("Shell4 solution", 0xdbdc_8928_c688_f72d),
    ("element stiffness", 0xb17b_f132_71f1_568c),
    ("element stiffness drilling 0 stabilisation 0", 0x9337_1b6a_9f28_e8d8),
    ("element stiffness displacement shear", 0xf89b_843f_eed0_487c),
    ("element thermal load", 0xf462_f41e_9e83_d038),
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

/// A plate of `nx x ny` cells: cells listed in `quads` are Shell4 (block 0, when any), the others two Shell3 cut alternately.
fn plate(shell4_cells: &[usize], loads: bool) -> SolidModel {
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
        mesh.set_block_property(b, BlockProperty::shell(0.01).unwrap()).unwrap();
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
    if loads {
        m.add_load(Load::shell_pressure_angle_weighted("all", -300.0)).unwrap();
        m.add_load(Load::gravity([0.5, -1.5, -9.81])).unwrap();
        m.add_load(Load::centrifugal([0.0, 0.3, 1.0], [0.4, 0.2, 0.0], 120.0))
            .unwrap();
        let b = nd.iter().map(|x| [x[0] + 2.0 * x[1], -x[2], 3.0 - x[0]]).collect();
        m.add_load(Load::body_force_nodal(b)).unwrap();
        let t = nd.iter().map(|x| 300.0 + 10.0 * x[0] + 5.0 * x[1]).collect();
        m.add_load(Load::temperature(t, 290.0)).unwrap();
    }
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
fn b1_default_shell3_and_shell4_give_their_former_bits() {
    let mut got: Vec<(&str, u64)> = Vec::new();
    let p = plate(&[], true);
    got.push(("Shell3 stiffness", digest(&stiffness(&p).unwrap().val)));
    got.push(("Shell3 mass", digest(&consistent(&p).unwrap().val)));
    got.push(("Shell3 loads", digest(&nodal_load_vector(&p, &[]).unwrap())));
    let r = solve_static(&p, &StaticOptions::default()).unwrap();
    got.push(("Shell3 solution", digest(r.displacement_values())));
    got.push(("Shell3 stresses", digest(&stresses_flat(&p, r.displacement_values()))));
    let opts = ModalOptions::new(4)
        .tolerance(1e-12)
        .residual_test(ResidualTest::Backward);
    got.push((
        "Shell3 frequencies",
        digest(&modal_analysis(&p, &opts).unwrap().frequencies),
    ));
    let mx = plate(&[0, 5, 10, 11], true);
    got.push(("mixed stiffness", digest(&stiffness(&mx).unwrap().val)));
    let r = solve_static(&mx, &StaticOptions::default()).unwrap();
    got.push(("mixed solution", digest(r.displacement_values())));
    got.push(("mixed stresses", digest(&stresses_flat(&mx, r.displacement_values()))));
    let all4: Vec<usize> = (0..12).collect();
    let s4 = plate(&all4, false);
    got.push(("Shell4 stiffness", digest(&stiffness(&s4).unwrap().val)));
    let r = solve_static(&s4, &StaticOptions::default()).unwrap();
    got.push(("Shell4 solution", digest(r.displacement_values())));
    let x: [Vec3; 3] = [[0.3, -0.2, 0.5], [1.1, 0.25, 0.4], [0.45, 0.6, 0.9]];
    let mat = steel();
    got.push((
        "element stiffness",
        digest(shell3::element_stiffness(&x, 0.02, &mat).unwrap().values()),
    ));
    let o = Shell3Options::default().drilling(0.0).stabilisation(0.0);
    got.push((
        "element stiffness drilling 0 stabilisation 0",
        digest(shell3::element_stiffness_with(&x, 0.02, &mat, &o).unwrap().values()),
    ));
    let o = Shell3Options::default().shear(TransverseShear::FullIntegration);
    got.push((
        "element stiffness displacement shear",
        digest(shell3::element_stiffness_with(&x, 0.02, &mat, &o).unwrap().values()),
    ));
    got.push((
        "element thermal load",
        digest(&shell3::thermal_load_vector(&x, 0.02, &mat, &[40.0, -10.0, 25.0]).unwrap()),
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
