//! F-591 (ADR 0502): check B0, the default paths that F-591 touches give the bits they gave before F-591: the gravity,
//! centrifugal and nodal body-force load vectors of a warped Shell4 plate, of its Shell3 twin and of a Hex8 box, the gravity
//! and centrifugal loads of a Beam2 cantilever, the static solve of the Shell4 plate under the three body loads, the pre-stress
//! and buckling factors of the Shell4 and Shell3 plates, a rotating part of each plate, and the spin-softening mass of the Hex8
//! box. Uses only API that existed at 70348e9f; the expected digests were printed by this file run on the unchanged base with
//! the registration committed (`f591-scratch/b0-base.log`) and written into `EXPECTED` before any library code of F-591.

use rcfd_core::vec3::Vec3;
use rcfd_fem::beam;
use rcfd_fem::buckling::{factors, BucklingOptions};
use rcfd_fem::builders::hex_box;
use rcfd_fem::geometric_struct::prestress_from_static;
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::modal::ModalOptions;
use rcfd_fem::rotating::{spin_softening_mass, RotatingOptions, RotatingPart, SpeedModes};
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{BlockProperty, Constraint, ElementKind, Load, Material, SolidMesh, SolidModel};

/// Digests of 70348e9f in the order of the run, from the base run (`f591-scratch/b0-base.log`).
const EXPECTED: Option<[(&str, u64); 20]> = Some([
    ("Shell4 gravity vector", 0x0f90_35fe_cd95_da78),
    ("Shell4 centrifugal vector", 0x37b8_81f3_46d5_5e16),
    ("Shell4 body-force vector", 0xdd63_378c_f5fb_8a89),
    ("Shell3 gravity vector", 0x9cff_6661_26fb_f9c2),
    ("Shell3 centrifugal vector", 0x2f9c_e00f_6842_1eeb),
    ("Shell3 body-force vector", 0xa64f_ec8a_d00c_a2a1),
    ("Hex8 gravity vector", 0xdd9b_aa19_cbe1_b724),
    ("Hex8 centrifugal vector", 0x61e4_ddc2_0d02_095f),
    ("Hex8 body-force vector", 0x6e5c_f0f2_c354_7ca9),
    ("Hex8 spin softening", 0xfb15_5dd8_b838_313c),
    ("Beam2 gravity vector", 0x5dbe_dd2c_dd7e_d62c),
    ("Beam2 centrifugal vector", 0xc4a4_3c82_0e9e_0913),
    ("Shell4 body-load static", 0x3973_ef7c_abcb_11f9),
    ("Shell4 pre-stress", 0x4e11_a786_0584_ae92),
    ("Shell4 buckling", 0x7bba_dd4b_7850_c4bf),
    ("Shell3 pre-stress", 0x24a3_8713_102f_fcae),
    ("Shell3 buckling", 0x8241_89b0_9265_cebe),
    ("Shell4 rotating part", 0x6406_aaa4_5c3d_b6ba),
    ("Shell3 rotating part", 0x9968_2a6f_1527_4618),
    ("count", 0x0000_0000_0000_0013),
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

const AXIS: Vec3 = [0.2, 0.3, 1.0];
const ORIGIN: Vec3 = [0.1, 0.05, -0.1];

/// The warped 3 x 3 plate of `sa_laminate_default_bits.rs`, edge `x = 0` clamped.
fn plate(kind: ElementKind) -> SolidModel {
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
    m
}

fn body_field(m: &SolidModel) -> Vec<Vec3> {
    m.mesh()
        .nodes()
        .iter()
        .map(|x| [100.0 + 30.0 * x[0], 200.0 - 50.0 * x[1], -300.0 + 70.0 * x[2]])
        .collect()
}

/// The three body-load vectors of a model, each alone: gravity, centrifugal, nodal body force.
fn body_vectors(m: &SolidModel) -> [Vec<f64>; 3] {
    let mut g = m.clone();
    g.add_load(Load::gravity([1.0, -2.0, -9.81])).unwrap();
    let mut c = m.clone();
    c.add_load(Load::centrifugal(AXIS, ORIGIN, 50.0)).unwrap();
    let mut b = m.clone();
    b.add_load(Load::body_force_nodal(body_field(m))).unwrap();
    [
        nodal_load_vector(&g, &[]).unwrap(),
        nodal_load_vector(&c, &[]).unwrap(),
        nodal_load_vector(&b, &[]).unwrap(),
    ]
}

fn static_values(m: &SolidModel) -> Vec<f64> {
    let s = solve_static(m, &StaticOptions::default()).unwrap();
    let mut v = s.displacement_values().to_vec();
    v.extend_from_slice(s.reactions().unwrap());
    v
}

fn buckling_values(m: &SolidModel) -> (Vec<f64>, Vec<f64>) {
    let mut b = m.clone();
    b.add_load(Load::nodal_force("x1", [-1.0e4, 0.0, 0.0])).unwrap();
    let s = solve_static(&b, &StaticOptions::default()).unwrap();
    let pre = prestress_from_static(&b, &s).unwrap();
    let f = factors(&b, &pre, &BucklingOptions::new(2)).unwrap();
    (pre.stress().iter().flatten().copied().collect(), f.factors)
}

fn rotating_values(m: &SolidModel) -> Vec<f64> {
    let part = RotatingPart::new(m, AXIS, ORIGIN, &RotatingOptions::new(4).modal(ModalOptions::new(4))).unwrap();
    let mut v: Vec<f64> = part.unit_prestress().stress().iter().flatten().copied().collect();
    v.extend_from_slice(&part.unit_geometric_stiffness().val);
    v.extend_from_slice(&part.spin_softening().val);
    match part.modes(100.0).unwrap() {
        SpeedModes::Modes(r) => v.extend_from_slice(&r.frequencies),
        SpeedModes::LossOfStiffness(_) => v.push(f64::NAN),
    }
    v
}

#[test]
fn b0_default_bits() {
    let mut got: Vec<(&str, u64)> = Vec::new();
    let p4 = plate(ElementKind::Shell4);
    let [g, c, b] = body_vectors(&p4);
    got.push(("Shell4 gravity vector", digest(&g)));
    got.push(("Shell4 centrifugal vector", digest(&c)));
    got.push(("Shell4 body-force vector", digest(&b)));
    let p3 = plate(ElementKind::Shell3);
    let [g, c, b] = body_vectors(&p3);
    got.push(("Shell3 gravity vector", digest(&g)));
    got.push(("Shell3 centrifugal vector", digest(&c)));
    got.push(("Shell3 body-force vector", digest(&b)));
    // Hex8 box.
    let mesh = hex_box([0.0; 3], [1.0, 0.1, 0.1], [6, 1, 1]).unwrap();
    let mut h = SolidModel::new(mesh);
    h.add_material("all", steel()).unwrap();
    for c in 0..3 {
        h.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    let [g, c, b] = body_vectors(&h);
    got.push(("Hex8 gravity vector", digest(&g)));
    got.push(("Hex8 centrifugal vector", digest(&c)));
    got.push(("Hex8 body-force vector", digest(&b)));
    got.push((
        "Hex8 spin softening",
        digest(&spin_softening_mass(&h, AXIS, None).unwrap().val),
    ));
    // Beam2 cantilever: gravity and centrifugal.
    let n = 6;
    let nodes: Vec<Vec3> = (0..=n).map(|i| [i as f64 / n as f64, 0.1, 0.0]).collect();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    let conn: Vec<usize> = (0..n).flat_map(|i| [i, i + 1]).collect();
    mesh.add_block(ElementKind::Beam2, conn).unwrap();
    let section = beam::rectangle_section(0.05, 0.03, 1.0e-7, 5.0 / 6.0).unwrap();
    mesh.set_block_property(0, BlockProperty::beam(section, [0.0, 1.0, 0.0]).unwrap())
        .unwrap();
    mesh.add_element_set("all", (0..n).collect()).unwrap();
    mesh.add_node_set("x0", vec![0]).unwrap();
    let mut bm = SolidModel::new(mesh);
    bm.add_material("all", steel()).unwrap();
    for c in 0..6 {
        bm.add_constraint(Constraint::displacement("x0", c, 0.0)).unwrap();
    }
    let mut g = bm.clone();
    g.add_load(Load::gravity([1.0, -2.0, -9.81])).unwrap();
    got.push(("Beam2 gravity vector", digest(&nodal_load_vector(&g, &[]).unwrap())));
    let mut c = bm.clone();
    c.add_load(Load::centrifugal(AXIS, ORIGIN, 50.0)).unwrap();
    got.push(("Beam2 centrifugal vector", digest(&nodal_load_vector(&c, &[]).unwrap())));
    // Static solve of the Shell4 plate under the three body loads.
    let mut s = p4.clone();
    s.add_load(Load::gravity([1.0, -2.0, -9.81])).unwrap();
    s.add_load(Load::centrifugal(AXIS, ORIGIN, 50.0)).unwrap();
    s.add_load(Load::body_force_nodal(body_field(&p4))).unwrap();
    got.push(("Shell4 body-load static", digest(&static_values(&s))));
    // Pre-stress and buckling.
    let (pre, f) = buckling_values(&p4);
    got.push(("Shell4 pre-stress", digest(&pre)));
    got.push(("Shell4 buckling", digest(&f)));
    let (pre, f) = buckling_values(&p3);
    got.push(("Shell3 pre-stress", digest(&pre)));
    got.push(("Shell3 buckling", digest(&f)));
    // Rotating parts.
    got.push(("Shell4 rotating part", digest(&rotating_values(&p4))));
    got.push(("Shell3 rotating part", digest(&rotating_values(&p3))));
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
