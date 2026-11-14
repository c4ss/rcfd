//! F-416 (ADR 0476) B1: the bits of the F-391 beam paths next to the flexible rotor (the modal analysis of the shafts Z-a and
//! Z-b of ADR 0476, the local stiffness and mass of one Beam2 element), recorded on main 57808e24 before any library line of
//! F-416 was written. Uses only the API of that commit.

use rcfd_fem::beam;
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::{BlockProperty, Constraint, ElasticSupport, ElementKind, Material, SolidMesh, SolidModel};
use rcfd_linalg::eigen::ResidualTest;

/// The FNV-1a hash of a sequence of 64-bit words.
fn fnv(words: impl IntoIterator<Item = u64>) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for w in words {
        for b in w.to_le_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    h
}

fn steel() -> Material {
    Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 0.0).unwrap()
}

/// Z-a (`springs = None`, pinned) and Z-b (bearings `k` at the ends) of ADR 0476.
fn shaft(springs: Option<f64>) -> SolidModel {
    let (l, r, ne) = (1.0, 0.05, 20usize);
    let nodes: Vec<[f64; 3]> = (0..=ne).map(|i| [0.0, 0.0, l * i as f64 / ne as f64]).collect();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Beam2, (0..ne).flat_map(|i| [i, i + 1]).collect())
        .unwrap();
    let s = beam::circle_section(r, 0.9).unwrap();
    mesh.set_block_property(0, BlockProperty::beam(s, [1.0, 0.0, 0.0]).unwrap())
        .unwrap();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    mesh.add_node_set("all", (0..=ne).collect()).unwrap();
    mesh.add_node_set("ends", vec![0, ne]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    m.add_constraint(Constraint::displacement("all", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("all", 5, 0.0)).unwrap();
    match springs {
        None => {
            m.add_constraint(Constraint::displacement("ends", 0, 0.0)).unwrap();
            m.add_constraint(Constraint::displacement("ends", 1, 0.0)).unwrap();
        }
        Some(k) => {
            m.add_support(ElasticSupport::nodal_spring("ends", 0, k)).unwrap();
            m.add_support(ElasticSupport::nodal_spring("ends", 1, k)).unwrap();
        }
    }
    m
}

fn digests() -> [u64; 5] {
    let opts = ModalOptions::new(8)
        .tolerance(1e-12)
        .residual_test(ResidualTest::Backward);
    let a = modal_analysis(&shaft(None), &opts).unwrap();
    let b = modal_analysis(&shaft(Some(1.0e7)), &opts).unwrap();
    let s = beam::circle_section(0.05, 0.9).unwrap();
    let k = beam::local_stiffness(0.05, &s, &steel());
    let m = beam::local_mass(0.05, &s, &steel());
    [
        fnv(a.eigenvalues.iter().map(|x| x.to_bits())),
        fnv(a.modes[0].iter().map(|x| x.to_bits())),
        fnv(b.eigenvalues.iter().map(|x| x.to_bits())),
        fnv(k.values().iter().map(|x| x.to_bits())),
        fnv(m.values().iter().map(|x| x.to_bits())),
    ]
}

/// Prints the digests (run at main 57808e24 to record [`BASE`]).
#[test]
#[ignore = "prints the digests of B1 (ADR 0476); run at the base to record BASE"]
fn print_base_digests() {
    for d in digests() {
        println!("0x{d:016x},");
    }
}

/// The digests printed at main 57808e24 (ADR 0476 B1).
const BASE: [u64; 5] = [
    0x5545_8fc6_5f27_5b84,
    0xa300_10ef_cb9a_bb22,
    0x8c43_1937_a62f_36a1,
    0x0259_67c2_1356_1a6d,
    0xfe5a_5218_70df_b5b9,
];

#[test]
fn b1_default_beam_paths_unchanged() {
    let got = digests();
    let names = [
        "Z-a modal eigenvalues",
        "Z-a modal first mode",
        "Z-b modal eigenvalues",
        "Beam2 local stiffness",
        "Beam2 local mass",
    ];
    let mut bad = Vec::new();
    for ((n, g), b) in names.iter().zip(got).zip(BASE) {
        println!(
            "B1 {n}: 0x{g:016x} (base 0x{b:016x}) {}",
            if g == b { "equal" } else { "DIFFERS" }
        );
        if g != b {
            bad.push(*n);
        }
    }
    assert!(bad.is_empty(), "B1 differs: {bad:?}");
}
