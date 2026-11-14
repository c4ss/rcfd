//! F-559 (ADR 0511) D4: the bits of the reduction of ADR 0474 (the cutoff and the residual vector of the model's load: the
//! part files of version 2, the assembly's eigenvalues, statics and harmonic response, the recovery), recorded on main
//! 6c4ba2d9 before any library line of F-559 was written. Uses only the API of that commit.

use rcfd_fem::builders::hex_box;
use rcfd_fem::cms::{reduce, Assembly, CmsOptions};
use rcfd_fem::{Constraint, Load, Material, SolidMesh, SolidModel};

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

fn text(s: &str) -> u64 {
    fnv(s.bytes().map(u64::from))
}

fn steel() -> Material {
    Material::isotropic_elastic(2.1e11, 0.3, 7850.0, 0.0).unwrap()
}

fn clamp(m: &mut SolidModel, set: &str) {
    for c in 0..3 {
        m.add_constraint(Constraint::displacement(set, c, 0.0)).unwrap();
    }
}

fn add_point_set(mesh: &mut SolidMesh, name: &str, p: [f64; 3]) {
    let s: Vec<usize> = (0..mesh.nodes().len()).filter(|&i| mesh.nodes()[i] == p).collect();
    assert_eq!(s.len(), 1);
    mesh.add_node_set(name, s).unwrap();
}

/// The small model S of ADR 0469 with the node set `mid`.
fn small() -> SolidModel {
    let mut mesh = hex_box([0.0; 3], [0.4, 0.1, 0.08], [4, 2, 2]).unwrap();
    let mid: Vec<usize> = (0..mesh.nodes().len())
        .filter(|&i| (mesh.nodes()[i][0] - 0.2).abs() < 1e-12)
        .collect();
    mesh.add_node_set("mid", mid).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    clamp(&mut m, "xmin");
    m.add_load(Load::traction("xmax", [1.0e6, 2.0e6, -3.0e6])).unwrap();
    m.add_load(Load::gravity([0.0, -9.81, -9.81])).unwrap();
    m
}

fn part_a_model() -> SolidModel {
    let mesh = hex_box([0.0; 3], [0.5, 0.1, 0.08], [8, 2, 2]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    clamp(&mut m, "xmin");
    m
}

fn part_b_model() -> SolidModel {
    let mut mesh = hex_box([0.5, 0.0, 0.0], [0.5, 0.1, 0.08], [8, 2, 2]).unwrap();
    add_point_set(&mut mesh, "corner", [1.0, 0.1, 0.08]);
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    m.add_load(Load::nodal_force("corner", [1.0, 1.0, 1.0])).unwrap();
    m
}

const N: usize = 10;

fn digests() -> [u64; N] {
    // The ADR 0474 option: the cutoff 32964 rad/s and the residual vector of the model's load.
    let o = CmsOptions::new(0).cutoff(32964.0).residual_vectors(true).threads(1);
    let a = reduce(&part_a_model(), "xmax", &o).unwrap();
    let bm = part_b_model();
    let b = reduce(&bm, "xmin", &o).unwrap();
    let o5 = CmsOptions::new(5).residual_vectors(true).threads(1);
    let b5 = reduce(&bm, "xmin", &o5).unwrap();
    let s = small();
    let sm = reduce(&s, "mid", &o5).unwrap();
    let (tb, tb5, tsm) = (b.to_text(), b5.to_text(), sm.to_text());
    let usm = sm.recover_static(&s, &sm.solve_static().unwrap()).unwrap();
    let asm = Assembly::new(vec![a, b]).unwrap();
    let eig = asm.eigen().unwrap().values;
    let (r0, i0) = asm.harmonic(0.0, 0.0).unwrap();
    let (r1, i1) = asm.harmonic(1000.0, 0.02).unwrap();
    let (r5, i5) = asm.harmonic(5000.0, 0.02).unwrap();
    let xb = asm.parts()[1].expand(&bm, &asm.part_coordinates(1, &r1)).unwrap();
    [
        text(&tb),
        text(&tb5),
        text(&tsm),
        fnv(bits(&eig)),
        fnv(bits(&r0).chain(bits(&i0))),
        fnv(bits(&r1).chain(bits(&i1))),
        fnv(bits(&r5).chain(bits(&i5))),
        fnv(bits(&xb)),
        fnv(bits(&usm)),
        fnv(bits(asm.load())),
    ]
}

/// Prints the digests (run at main 6c4ba2d9 to record [`BASE`]).
#[test]
#[ignore = "prints the digests of D4 (ADR 0511); run at the base to record BASE"]
fn print_base_digests() {
    for d in digests() {
        println!("0x{d:016x},");
    }
}

/// The digests printed at main 6c4ba2d9 (ADR 0511 D4).
const BASE: [u64; N] = [
    0xc9cd_5d78_49d6_2d4f,
    0x88df_7243_2773_82f5,
    0xf21c_3819_f410_d07d,
    0xcf55_7537_5493_61b3,
    0xc5f5_1a73_4c3d_144f,
    0x7acb_06c5_c7b9_94a0,
    0x141c_a469_e9d5_7ee6,
    0xdbdb_d53c_58c4_c196,
    0x2a16_0135_6e74_0c40,
    0x8d7c_2387_e5b5_20b1,
];

#[test]
fn d4_adr_0474_paths_unchanged() {
    let got = digests();
    let names = [
        "text B cutoff + vectors (header 2)",
        "text B m 5 + vectors",
        "text S mid m 5 + vectors",
        "assembly eigenvalues (cutoff + vectors)",
        "assembly statics",
        "assembly harmonic 1000",
        "assembly harmonic 5000",
        "B expanded at 1000",
        "S recovered statics m 5 + vectors",
        "assembly load",
    ];
    let mut bad = Vec::new();
    for i in 0..N {
        if got[i] != BASE[i] {
            bad.push(format!("{}: 0x{:016x} != 0x{:016x}", names[i], got[i], BASE[i]));
        }
    }
    println!("D4: {} of {N} digests equal", N - bad.len());
    assert!(bad.is_empty(), "{bad:?}");
}
