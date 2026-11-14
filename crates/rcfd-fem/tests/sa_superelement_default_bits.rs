//! F-553 (ADR 0536) D1: the bits of the default paths next to the superelements (the static solve of a solid, a beam and a
//! shell model, the modal analysis, the direct harmonic solve, the reduction of ADR 0469 and its two-part assembly) on the
//! set-ups of ADR 0536, recorded on main 679081a1 before any library line of F-553 was written. Uses only the API of that
//! commit.

use rcfd_core::vec3::Vec3;
use rcfd_fem::beam;
use rcfd_fem::builders::hex_box;
use rcfd_fem::cms::{reduce, Assembly, CmsOptions};
use rcfd_fem::harmonic::HarmonicLoad;
use rcfd_fem::harmonic_direct::{DirectHarmonic, DirectHarmonicOptions, LossFactor};
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{BlockProperty, Constraint, ElementKind, Load, Material, SolidMesh, SolidModel};
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

/// The full solid bar F of ADR 0536 with the static loads (traction on xmax, gravity).
fn solid_static() -> SolidModel {
    let mesh = hex_box([0.0; 3], [1.0, 0.1, 0.08], [16, 2, 2]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m.add_load(Load::traction("xmax", [1.0e6, 2.0e6, -3.0e6])).unwrap();
    m.add_load(Load::gravity([0.0, -9.81, -9.81])).unwrap();
    m
}

/// The full solid bar F with the corner load.
fn solid_corner() -> SolidModel {
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

/// The full beam of ADR 0536 with its static loads.
fn beam_full() -> SolidModel {
    let n = 16;
    let nodes: Vec<Vec3> = (0..=n).map(|i| [i as f64 / n as f64, 0.0, 0.0]).collect();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    let conn: Vec<usize> = (0..n).flat_map(|i| [i, i + 1]).collect();
    mesh.add_block(ElementKind::Beam2, conn).unwrap();
    let s = beam::rectangle_section(0.1, 0.08, 9.2e-6, 5.0 / 6.0).unwrap();
    mesh.set_block_property(0, BlockProperty::beam(s, [0.0, 1.0, 0.0]).unwrap())
        .unwrap();
    mesh.add_element_set("all", (0..n).collect()).unwrap();
    mesh.add_node_set("x0", vec![0]).unwrap();
    mesh.add_node_set("xl", vec![n]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    for c in 0..6 {
        m.add_constraint(Constraint::displacement("x0", c, 0.0)).unwrap();
    }
    m.add_load(Load::nodal_force("xl", [1.0e3, 2.0e3, -3.0e3])).unwrap();
    m.add_load(Load::nodal_moment("xl", [10.0, 20.0, -30.0])).unwrap();
    m.add_load(Load::gravity([0.0, -9.81, -9.81])).unwrap();
    m
}

/// The full shell strip of ADR 0536 with its static loads.
fn shell_full() -> SolidModel {
    let (nx, ny) = (16, 2);
    let idx = |i: usize, j: usize| j * (nx + 1) + i;
    let mut nodes = Vec::new();
    for j in 0..=ny {
        for i in 0..=nx {
            nodes.push([i as f64 / nx as f64, 0.1 * j as f64 / ny as f64, 0.0]);
        }
    }
    let mut conn = Vec::new();
    for j in 0..ny {
        for i in 0..nx {
            conn.extend_from_slice(&[idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1)]);
        }
    }
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Shell4, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::shell(0.01).unwrap()).unwrap();
    mesh.add_element_set("all", (0..nx * ny).collect()).unwrap();
    mesh.add_node_set("x0", (0..=ny).map(|j| idx(0, j)).collect()).unwrap();
    mesh.add_node_set("xl", (0..=ny).map(|j| idx(nx, j)).collect()).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    for c in 0..6 {
        m.add_constraint(Constraint::displacement("x0", c, 0.0)).unwrap();
    }
    m.add_load(Load::nodal_force("xl", [50.0, 20.0, -10.0])).unwrap();
    m.add_load(Load::gravity([0.0, -9.81, -9.81])).unwrap();
    m
}

/// Half A of the solid bar (clamped on xmin, gravity), half B (free, the static loads of x = 1).
fn halves() -> (SolidModel, SolidModel) {
    let mut a = SolidModel::new(hex_box([0.0; 3], [0.5, 0.1, 0.08], [8, 2, 2]).unwrap());
    a.add_material("all", steel()).unwrap();
    for c in 0..3 {
        a.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    a.add_load(Load::gravity([0.0, -9.81, -9.81])).unwrap();
    let mut b = SolidModel::new(hex_box([0.5, 0.0, 0.0], [0.5, 0.1, 0.08], [8, 2, 2]).unwrap());
    b.add_material("all", steel()).unwrap();
    b.add_load(Load::traction("xmax", [1.0e6, 2.0e6, -3.0e6])).unwrap();
    b.add_load(Load::gravity([0.0, -9.81, -9.81])).unwrap();
    (a, b)
}

fn digests() -> Vec<u64> {
    let mut out = Vec::new();
    for m in [solid_static(), beam_full(), shell_full()] {
        let r = solve_static(&m, &StaticOptions::default()).unwrap();
        out.push(fnv(bits(r.displacement_values())));
    }
    let f = solid_corner();
    let modal = modal_analysis(
        &f,
        &ModalOptions::new(8)
            .tolerance(1e-12)
            .residual_test(ResidualTest::Backward)
            .threads(1),
    )
    .unwrap();
    out.push(fnv(bits(&modal.eigenvalues)));
    let dh = DirectHarmonic::new(
        &f,
        &DirectHarmonicOptions::new()
            .loss_factor("all", LossFactor::Constant(0.02))
            .threads(1),
    )
    .unwrap();
    let resp = dh.response(&HarmonicLoad::from_model(&f).unwrap(), 1000.0).unwrap();
    out.push(fnv(bits(&resp.re)));
    out.push(fnv(bits(&resp.im)));
    let (a, b) = halves();
    let pa = reduce(&a, "xmax", &CmsOptions::new(5).threads(1)).unwrap();
    out.push(fnv(bits(pa.stiffness())));
    out.push(fnv(bits(pa.mass())));
    out.push(fnv(bits(pa.load())));
    let pb = reduce(&b, "xmin", &CmsOptions::new(5).threads(1)).unwrap();
    let asm = Assembly::new(vec![pa, pb]).unwrap();
    out.push(fnv(bits(&asm.eigen().unwrap().values)));
    out
}

const NAMES: [&str; 10] = [
    "static solid",
    "static beam",
    "static shell",
    "modal F",
    "harmonic re",
    "harmonic im",
    "reduce A K_r",
    "reduce A M_r",
    "reduce A load",
    "two-part assembly eigenvalues",
];

/// Prints the digests (run at main 679081a1 to record [`BASE`]).
#[test]
#[ignore = "prints the digests of D1 (ADR 0536); run at the base to record BASE"]
fn print_base_digests() {
    for d in digests() {
        println!("0x{d:016x},");
    }
}

/// The digests printed at main 679081a1 (ADR 0536 D1).
const BASE: [u64; 10] = [
    0x5134_3d3a_caa1_aea1,
    0x3b6f_e7ca_9214_f9b7,
    0x122e_370d_8a41_c8a8,
    0x4ee4_06bf_f3f6_6622,
    0xe9d7_b619_2d2b_0e72,
    0x8840_2f9c_1dac_cd75,
    0x4f21_4f69_e311_22ff,
    0x4d89_df91_10a1_8c11,
    0xcac6_159b_7293_50bf,
    0x1f7c_ced2_4974_cee6,
];

#[test]
fn d1_default_paths_unchanged() {
    let got = digests();
    let mut bad = Vec::new();
    for i in 0..NAMES.len() {
        if got[i] != BASE[i] {
            bad.push(format!("{}: 0x{:016x} != 0x{:016x}", NAMES[i], got[i], BASE[i]));
        }
    }
    println!("D1: {} of {} digests equal", NAMES.len() - bad.len(), NAMES.len());
    assert!(bad.is_empty(), "{bad:?}");
}
