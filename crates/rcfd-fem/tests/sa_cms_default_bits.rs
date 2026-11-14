//! F-430 (ADR 0469) D1: the bits of the default paths next to the substructuring (the static solve and its stresses, the
//! modal analysis, the dense modal basis, the direct harmonic solve) on the set-ups of ADR 0469, recorded on main 13b387a2
//! before any library line of F-430 was written. Uses only the API of that commit.

use rcfd_fem::builders::hex_box;
use rcfd_fem::harmonic::{HarmonicLoad, ModalBasis};
use rcfd_fem::harmonic_direct::{DirectHarmonic, DirectHarmonicOptions, LossFactor};
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::recover::IntegrationPointField;
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{Constraint, Load, Material, SolidModel};
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

fn bits(v: &[f64]) -> impl Iterator<Item = u64> + '_ {
    v.iter().map(|x| x.to_bits())
}

fn steel() -> Material {
    Material::isotropic_elastic(2.1e11, 0.3, 7850.0, 0.0).unwrap()
}

/// The small model S of ADR 0469.
fn small() -> SolidModel {
    let mesh = hex_box([0.0; 3], [0.4, 0.1, 0.08], [4, 2, 2]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m.add_load(Load::traction("xmax", [1.0e6, 2.0e6, -3.0e6])).unwrap();
    m.add_load(Load::gravity([0.0, -9.81, -9.81])).unwrap();
    m
}

/// The full bar F of ADR 0469 with the corner load.
fn bar() -> SolidModel {
    let mut mesh = hex_box([0.0; 3], [1.0, 0.1, 0.08], [16, 2, 2]).unwrap();
    let corner: Vec<usize> = (0..mesh.nodes().len())
        .filter(|&p| mesh.nodes()[p] == [1.0, 0.1, 0.08])
        .collect();
    assert_eq!(corner.len(), 1);
    mesh.add_node_set("corner", corner).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m.add_load(Load::nodal_force("corner", [1.0, 1.0, 1.0])).unwrap();
    m
}

fn digests() -> [u64; 6] {
    let s = small();
    let r = solve_static(&s, &StaticOptions::default()).unwrap();
    let u = r.displacement_values().to_vec();
    let field = IntegrationPointField::compute(&s, &u).unwrap();
    let stress: Vec<f64> = field.stress().iter().flat_map(|x| x.iter().copied()).collect();
    let f = bar();
    let modal = modal_analysis(
        &f,
        &ModalOptions::new(8)
            .tolerance(1e-12)
            .residual_test(ResidualTest::Backward)
            .threads(1),
    )
    .unwrap();
    let basis = ModalBasis::all_modes(&s, Some(1)).unwrap();
    let dh = DirectHarmonic::new(
        &f,
        &DirectHarmonicOptions::new()
            .loss_factor("all", LossFactor::Constant(0.02))
            .threads(1),
    )
    .unwrap();
    let resp = dh.response(&HarmonicLoad::from_model(&f).unwrap(), 1000.0).unwrap();
    [
        fnv(bits(&u)),
        fnv(bits(&stress)),
        fnv(bits(&modal.eigenvalues)),
        fnv(bits(basis.eigenvalues())),
        fnv(bits(&resp.re)),
        fnv(bits(&resp.im)),
    ]
}

/// Prints the digests (run at main 13b387a2 to record [`BASE`]).
#[test]
#[ignore = "prints the digests of D1 (ADR 0469); run at the base to record BASE"]
fn print_base_digests() {
    for d in digests() {
        println!("0x{d:016x},");
    }
}

/// The digests printed at main 13b387a2 (ADR 0469 D1).
const BASE: [u64; 6] = [
    0xa86d_036f_fc56_057d,
    0x934c_b1c2_0806_acb0,
    0x4ee4_06bf_f3f6_6622,
    0xd9d8_097e_e42f_8ff0,
    0xe9d7_b619_2d2b_0e72,
    0x8840_2f9c_1dac_cd75,
];

#[test]
fn d1_default_paths_unchanged() {
    let got = digests();
    let names = ["static u", "static stress", "modal F", "all_modes S", "harmonic re", "harmonic im"];
    let mut bad = Vec::new();
    for i in 0..6 {
        if got[i] != BASE[i] {
            bad.push(format!("{}: 0x{:016x} != 0x{:016x}", names[i], got[i], BASE[i]));
        }
    }
    println!("D1: {} of 6 digests equal", 6 - bad.len());
    assert!(bad.is_empty(), "{bad:?}");
}
