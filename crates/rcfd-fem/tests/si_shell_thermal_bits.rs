//! F-525 (ADR 0445): check B1, the load vectors of models without shells (nodal body-force fields, temperature, gravity and
//! the centrifugal load on Hex8 and Tet10 boxes) are the bits they were before F-525. Uses only API that existed at 5325c69d;
//! the expected digests were printed by this file run on a copy of 5325c69d (`f525-scratch/b1-base.log`) and written into
//! `EXPECTED` before any library code of F-525.

use rcfd_fem::builders::{hex_box, tet_box, upgrade_to_tet10};
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::mesh::{ElementKind, SolidMesh};
use rcfd_fem::{Load, Material, SolidModel};

/// Digests of 5325c69d in the order of the run, from the base run (`f525-scratch/b1-base.log`).
const EXPECTED: Option<[(&str, u64); 4]> = Some([
    ("fields Hex8", 0xce00_7fcb_edec_5bf8),
    ("fields Tet10", 0x1771_1f97_6767_e027),
    ("body Hex8", 0x9f36_e6c0_4533_d2aa),
    ("body Tet10", 0xa428_a1d0_6533_97d4),
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

fn mesh_of(kind: ElementKind) -> SolidMesh {
    match kind {
        ElementKind::Hex8 => hex_box([0.0; 3], [2.0, 1.0, 1.0], [2, 1, 1]).unwrap(),
        _ => upgrade_to_tet10(&tet_box([0.0; 3], [2.0, 1.0, 1.0], [2, 1, 1]).unwrap())
            .unwrap()
            .0,
    }
}

fn model_of(mesh: SolidMesh) -> SolidModel {
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 1.2e-5).unwrap())
        .unwrap();
    m
}

fn fields(kind: ElementKind) -> u64 {
    let mesh = mesh_of(kind);
    let nodes = mesh.nodes().to_vec();
    let mut m = model_of(mesh);
    let b = nodes.iter().map(|x| [x[0] + 2.0 * x[1], -x[2], 3.0 - x[0]]).collect();
    m.add_load(Load::body_force_nodal(b)).unwrap();
    let t = nodes.iter().map(|x| 300.0 + 10.0 * x[0] + 5.0 * x[1] * x[2]).collect();
    m.add_load(Load::temperature(t, 290.0)).unwrap();
    digest(&nodal_load_vector(&m, &[]).unwrap())
}

fn body(kind: ElementKind) -> u64 {
    let mut m = model_of(mesh_of(kind));
    m.add_load(Load::gravity([0.5, -1.5, -9.81])).unwrap();
    m.add_load(Load::centrifugal([0.0, 0.3, 1.0], [0.4, 0.2, 0.0], 120.0)).unwrap();
    digest(&nodal_load_vector(&m, &[]).unwrap())
}

#[test]
fn b1_models_without_shells_give_their_former_load_bits() {
    let got = [
        ("fields Hex8", fields(ElementKind::Hex8)),
        ("fields Tet10", fields(ElementKind::Tet10)),
        ("body Hex8", body(ElementKind::Hex8)),
        ("body Tet10", body(ElementKind::Tet10)),
    ];
    for (name, d) in &got {
        println!("B1 {name}: {d:#018x}");
    }
    let expected = EXPECTED.expect("EXPECTED is written from the base run");
    for ((name, d), (en, e)) in got.iter().zip(expected.iter()) {
        assert_eq!(name, en);
        assert_eq!(d, e, "B1 {name}: {d:#018x} against {e:#018x}");
    }
}
