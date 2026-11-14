//! F-428 (ADR 0496): check B0, the default paths that F-428 touches give the bits they gave before F-428: the static solve of a
//! warped Shell4 plate under a shell pressure, a nodal moment and a nodal temperature field, its `shell::stresses`, its
//! consistent mass and first four frequencies, its gravity load, `nonlinear::solve` of the plate, the buckling factors of the
//! plate under an in-plane end load (pre-stress through `geometric_struct`), the Shell3 twin under the temperature, and a Hex8
//! cantilever under a temperature field. Uses only API that existed at 9386de34; the expected digests were printed by this
//! file run on the unchanged base with the registration committed (`f428-scratch/b0-base.log`) and written into `EXPECTED`
//! before any library code of F-428.

use rcfd_fem::buckling::{factors, BucklingOptions};
use rcfd_fem::builders::hex_box;
use rcfd_fem::geometric_struct::prestress_from_static;
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::mass::consistent;
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::nonlinear::{solve, NewtonOptions};
use rcfd_fem::shell;
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{BlockProperty, Constraint, ElementKind, Load, Material, SolidMesh, SolidModel};

/// Digests of 9386de34 in the order of the run, from the base run (`f428-scratch/b0-base.log`).
const EXPECTED: Option<[(&str, u64); 11]> = Some([
    ("Shell4 plate static", 0x1097_bb52_a3fc_b197),
    ("Shell4 plate stresses", 0x0d41_5dd5_cc79_1263),
    ("Shell4 plate load vector", 0x50f7_e96b_30bb_07a0),
    ("Shell4 plate mass", 0xa7f6_efe0_64bf_7001),
    ("Shell4 plate frequencies", 0xa87a_49d3_1c2f_3667),
    ("Shell4 plate gravity", 0xeeb9_d16f_e785_3503),
    ("Shell4 plate nonlinear", 0xab6a_0b52_448d_4fff),
    ("Shell4 plate buckling", 0x7bba_dd4b_7850_c4bf),
    ("Shell3 plate static", 0xb12e_e082_5271_b3a3),
    ("Hex8 cantilever thermal", 0x627c_0dc3_0b17_4dca),
    ("count", 0x0000_0000_0000_000a),
]);

/// FNV-1a 64 over the little-endian bytes of the bits of each value.
fn digest(v: &[f64]) -> u64 {
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    for b in v.iter().flat_map(|x| x.to_bits().to_le_bytes()) {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}

fn steel() -> Material {
    Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 1.2e-5).unwrap()
}

/// The warped 3 x 3 plate of `sa_corot_shell_default_bits.rs`, edge `x = 0` clamped; `loads` adds a shell pressure, a nodal
/// moment on `x = 1` and a nodal temperature field.
fn plate(kind: ElementKind, loads: bool) -> SolidModel {
    let mut nodes = Vec::new();
    for j in 0..=3 {
        for i in 0..=3 {
            nodes.push([i as f64 / 3.0, j as f64 / 3.0, 0.02 * (i * j) as f64]);
        }
    }
    let idx = |i: usize, j: usize| j * 4 + i;
    let mut conn = Vec::new();
    for j in 0..3 {
        for i in 0..3 {
            let (a, b, c, d) = (idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1));
            if kind == ElementKind::Shell3 {
                conn.extend_from_slice(&[a, b, c, a, c, d]);
            } else {
                conn.extend_from_slice(&[a, b, c, d]);
            }
        }
    }
    let ne = if kind == ElementKind::Shell3 { 18 } else { 9 };
    let n = nodes.len();
    let temps: Vec<f64> = nodes.iter().map(|x| 300.0 + 40.0 * x[0] - 25.0 * x[1] * x[1]).collect();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(kind, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::shell(0.01).unwrap()).unwrap();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    mesh.add_node_set("all", (0..n).collect()).unwrap();
    mesh.add_node_set("x0", (0..=3).map(|j| idx(0, j)).collect()).unwrap();
    mesh.add_node_set("x1", (0..=3).map(|j| idx(3, j)).collect()).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    for c in 0..6 {
        m.add_constraint(Constraint::displacement("x0", c, 0.0)).unwrap();
    }
    if loads {
        m.add_load(Load::shell_pressure("all", 1.0e3)).unwrap();
        m.add_load(Load::nodal_moment("x1", [0.3, -2.0, 0.1])).unwrap();
        m.add_load(Load::temperature(temps, 290.0)).unwrap();
    }
    m
}

fn static_values(m: &SolidModel) -> Vec<f64> {
    let s = solve_static(m, &StaticOptions::default()).unwrap();
    let mut v = s.displacement_values().to_vec();
    v.extend_from_slice(s.reactions().unwrap());
    v
}

#[test]
fn b0_default_bits() {
    let mut got: Vec<(&str, u64)> = Vec::new();
    // Shell4: static, stresses, mass, frequencies, gravity.
    let p4 = plate(ElementKind::Shell4, true);
    got.push(("Shell4 plate static", digest(&static_values(&p4))));
    let s = solve_static(&p4, &StaticOptions::default()).unwrap();
    let mut v = Vec::new();
    for (e, st) in shell::stresses(&p4, s.displacement_values()).unwrap() {
        v.push(e as f64);
        v.extend_from_slice(&st.membrane);
        v.extend_from_slice(&st.bending);
        v.extend_from_slice(&st.shear);
        v.extend_from_slice(&st.top);
        v.extend_from_slice(&st.bottom);
    }
    got.push(("Shell4 plate stresses", digest(&v)));
    got.push((
        "Shell4 plate load vector",
        digest(&nodal_load_vector(&p4, &[]).unwrap()),
    ));
    got.push(("Shell4 plate mass", digest(&consistent(&p4).unwrap().val)));
    let free = plate(ElementKind::Shell4, false);
    let r = modal_analysis(&free, &ModalOptions::new(4)).unwrap();
    got.push(("Shell4 plate frequencies", digest(&r.frequencies)));
    let mut g = plate(ElementKind::Shell4, false);
    g.add_load(Load::gravity([0.0, 0.0, -9.81])).unwrap();
    got.push(("Shell4 plate gravity", digest(&static_values(&g))));
    // The Newton driver on the plate (pressure and moment).
    let mut nl = plate(ElementKind::Shell4, false);
    nl.add_load(Load::shell_pressure("all", 1.0e3)).unwrap();
    nl.add_load(Load::nodal_moment("x1", [0.3, -2.0, 0.1])).unwrap();
    let r = solve(&nl, &[], &[], &NewtonOptions::new(2)).unwrap();
    let mut v = r.result.displacement_values().to_vec();
    v.extend_from_slice(r.result.reactions().unwrap());
    for st in &r.steps {
        v.push(st.load_factor);
        v.extend_from_slice(&st.residuals);
    }
    got.push(("Shell4 plate nonlinear", digest(&v)));
    // Buckling of the plate under an in-plane end compression.
    let mut b = plate(ElementKind::Shell4, false);
    b.add_load(Load::nodal_force("x1", [-1.0e4, 0.0, 0.0])).unwrap();
    let s = solve_static(&b, &StaticOptions::default()).unwrap();
    let pre = prestress_from_static(&b, &s).unwrap();
    let f = factors(&b, &pre, &BucklingOptions::new(2)).unwrap();
    got.push(("Shell4 plate buckling", digest(&f.factors)));
    // Shell3 twin under the temperature.
    let p3 = plate(ElementKind::Shell3, true);
    got.push(("Shell3 plate static", digest(&static_values(&p3))));
    // Hex8 cantilever under a temperature field.
    let mesh = hex_box([0.0; 3], [1.0, 0.1, 0.1], [6, 1, 1]).unwrap();
    let temps: Vec<f64> = mesh.nodes().iter().map(|x| 300.0 + 50.0 * x[0] + 80.0 * x[2]).collect();
    let mut h = SolidModel::new(mesh);
    h.add_material("all", steel()).unwrap();
    for c in 0..3 {
        h.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    h.add_load(Load::temperature(temps, 293.0)).unwrap();
    got.push(("Hex8 cantilever thermal", digest(&static_values(&h))));
    got.push(("count", got.len() as u64));
    for (name, d) in &got {
        println!(
            "    (\"{name}\", 0x{:04x}_{:04x}_{:04x}_{:04x}),",
            d >> 48,
            (d >> 32) & 0xffff,
            (d >> 16) & 0xffff,
            d & 0xffff
        );
    }
    match EXPECTED {
        None => panic!("no expected digests yet: print them on the base"),
        Some(expected) => {
            let mut bad = Vec::new();
            for ((name, d), (en, ed)) in got.iter().zip(expected.iter()) {
                if name != en || d != ed {
                    bad.push(format!("{name}: 0x{d:016x} != 0x{ed:016x}"));
                }
            }
            assert_eq!(got.len(), expected.len(), "count");
            assert!(bad.is_empty(), "changed: {bad:?}");
        }
    }
}
