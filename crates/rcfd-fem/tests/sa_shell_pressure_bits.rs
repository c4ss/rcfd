//! F-540 (ADR 0461): check B1, the default shell pressure (`Load::shell_pressure`) gives the bits it gave before F-540: the
//! load vector and the static solution of the Shell3 cylinder quarter of ADR 0454 on the one-diagonal and the checkerboard
//! 32 x 16 meshes, of a Shell4 cylinder quarter and of a mixed Shell3 + Shell4 quarter. Uses only API that existed at
//! 32fbefad; the expected digests were printed by this file run on the unchanged base 32fbefad (`f540-scratch/b1-base.log`)
//! and written into `EXPECTED` before any library code of F-540.

use rcfd_core::vec3::Vec3;
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::solve::{solve_static_with, StaticOptions};
use rcfd_fem::{BlockProperty, ElementKind, Load, Material, SolidMesh, SolidModel};
use std::f64::consts::PI;

/// Digests of 32fbefad in the order of the run, from the base run (`f540-scratch/b1-base.log`).
const EXPECTED: Option<[(&str, u64); 8]> = Some([
    ("Shell3 one-diagonal loads", 0xd4a2_33ac_a9f0_8078),
    ("Shell3 one-diagonal solution", 0xd076_914e_7c62_4752),
    ("Shell3 checkerboard loads", 0x0717_d429_f864_339a),
    ("Shell3 checkerboard solution", 0xb8b6_5fa6_0fec_66c1),
    ("Shell4 loads", 0xc7f8_85ac_b9e2_1d7e),
    ("Shell4 solution", 0x323f_f72b_7adb_4ea8),
    ("mixed loads", 0xe039_e1c8_8ed2_75df),
    ("mixed solution", 0xe19b_9dad_2339_f062),
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

/// The cylinder quarter of ADR 0454 arithmetic 13 (R = 1, H = 0.5, t = 0.01, nu = 0.3, p = 1e5, the symmetry conditions, the
/// end z = H free) on `nq x nz` cells. `kind(i, j)`: `None` a Shell4 cell, `Some(d)` two Shell3 triangles cut along `q0-q2`
/// (`d = 0`) or `q1-q3` (`d = 1`). Returns the load vector and the displacements of every node.
fn cylinder(nq: usize, nz: usize, kind: &dyn Fn(usize, usize) -> Option<usize>) -> (Vec<f64>, Vec<f64>) {
    let (r0, lh, t, p) = (1.0, 0.5, 0.01, 1e5);
    let idx = |i: usize, j: usize| (nz + 1) * i + j;
    let mut nodes: Vec<Vec3> = Vec::new();
    for i in 0..=nq {
        let ph = PI / 2.0 * i as f64 / nq as f64;
        for j in 0..=nz {
            nodes.push([r0 * ph.cos(), r0 * ph.sin(), lh * j as f64 / nz as f64]);
        }
    }
    let (mut tris, mut quads) = (Vec::new(), Vec::new());
    for i in 0..nq {
        for j in 0..nz {
            let q = [idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1)];
            match kind(i, j) {
                None => quads.extend_from_slice(&q),
                Some(0) => tris.extend_from_slice(&[q[0], q[1], q[2], q[0], q[2], q[3]]),
                Some(_) => tris.extend_from_slice(&[q[0], q[1], q[3], q[1], q[2], q[3]]),
            }
        }
    }
    let n = nodes.len();
    let mut mesh = SolidMesh::new(nodes.clone()).unwrap();
    let mut count = 0;
    let mut block = 0;
    if !tris.is_empty() {
        count += tris.len() / 3;
        mesh.add_block(ElementKind::Shell3, tris).unwrap();
        mesh.set_block_property(block, BlockProperty::shell(t).unwrap())
            .unwrap();
        block += 1;
    }
    if !quads.is_empty() {
        count += quads.len() / 4;
        mesh.add_block(ElementKind::Shell4, quads).unwrap();
        mesh.set_block_property(block, BlockProperty::shell(t).unwrap())
            .unwrap();
    }
    mesh.add_element_set("all", (0..count).collect()).unwrap();
    mesh.add_node_set("all", (0..n).collect()).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 0.0).unwrap())
        .unwrap();
    m.add_load(Load::shell_pressure("all", p)).unwrap();
    let mut pres = Vec::new();
    for j in 0..=nz {
        for comp in [1, 3, 5] {
            pres.push(Prescribed::new(idx(0, j), comp, 0.0));
        }
        for comp in [0, 4, 5] {
            pres.push(Prescribed::new(idx(nq, j), comp, 0.0));
        }
    }
    for i in 0..=nq {
        for comp in [2, 3, 4] {
            pres.push(Prescribed::new(idx(i, 0), comp, 0.0));
        }
    }
    let f = nodal_load_vector(&m, &[]).unwrap();
    let r = solve_static_with(&m, &pres, &[], &StaticOptions::default()).unwrap();
    let u: Vec<f64> = (0..n).flat_map(|k| r.displacement(k).iter().copied()).collect();
    (f, u)
}

#[test]
fn b1_default_shell_pressure_bits() {
    let mut got: Vec<(&str, u64)> = Vec::new();
    let (f, u) = cylinder(32, 16, &|_, _| Some(0));
    got.push(("Shell3 one-diagonal loads", digest(&f)));
    got.push(("Shell3 one-diagonal solution", digest(&u)));
    let (f, u) = cylinder(32, 16, &|i, j| Some((i + j) % 2));
    got.push(("Shell3 checkerboard loads", digest(&f)));
    got.push(("Shell3 checkerboard solution", digest(&u)));
    let (f, u) = cylinder(16, 8, &|_, _| None);
    got.push(("Shell4 loads", digest(&f)));
    got.push(("Shell4 solution", digest(&u)));
    let (f, u) = cylinder(16, 8, &|i, _| if i % 4 == 0 { None } else { Some(0) });
    got.push(("mixed loads", digest(&f)));
    got.push(("mixed solution", digest(&u)));
    for (name, d) in &got {
        println!("B1 {name}: 0x{d:016x}");
    }
    match EXPECTED {
        None => panic!("EXPECTED not yet recorded: run on the base and write the digests above into EXPECTED"),
        Some(expected) => {
            assert_eq!(got.len(), expected.len());
            for ((name, d), (en, ed)) in got.iter().zip(expected.iter()) {
                assert_eq!(name, en);
                assert_eq!(*d, *ed, "B1 {name}: 0x{d:016x} != 0x{ed:016x}");
            }
        }
    }
}
