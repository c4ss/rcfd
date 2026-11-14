//! F-603 (ADR 0532) D6: the bits of the reduction of ADR 0511 (the cutoff, the residual vector of the model's load and the
//! vectors of load patterns: the part files, the assemblies' eigenvalues, harmonic response and component statics, the
//! recovery), recorded on main 291a210c before any library line of F-603 was written. Uses only the API of that commit.

use rcfd_fem::builders::hex_box;
use rcfd_fem::cms::{reduce, Assembly, CmsOptions};
use rcfd_fem::loads::nodal_load_vector;
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

/// The small model S of ADR 0469 with the node set `mid` and the traction and gravity as selected.
fn small_with(traction: bool, gravity: bool) -> SolidModel {
    let mut mesh = hex_box([0.0; 3], [0.4, 0.1, 0.08], [4, 2, 2]).unwrap();
    let mid: Vec<usize> = (0..mesh.nodes().len())
        .filter(|&i| (mesh.nodes()[i][0] - 0.2).abs() < 1e-12)
        .collect();
    mesh.add_node_set("mid", mid).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    clamp(&mut m, "xmin");
    if traction {
        m.add_load(Load::traction("xmax", [1.0e6, 2.0e6, -3.0e6])).unwrap();
    }
    if gravity {
        m.add_load(Load::gravity([0.0, -9.81, -9.81])).unwrap();
    }
    m
}

fn part_a_model() -> SolidModel {
    let mesh = hex_box([0.0; 3], [0.5, 0.1, 0.08], [8, 2, 2]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    clamp(&mut m, "xmin");
    m
}

fn part_b_with(force: [f64; 3]) -> SolidModel {
    let mut mesh = hex_box([0.5, 0.0, 0.0], [0.5, 0.1, 0.08], [8, 2, 2]).unwrap();
    add_point_set(&mut mesh, "corner", [1.0, 0.1, 0.08]);
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    m.add_load(Load::nodal_force("corner", force)).unwrap();
    m
}

fn part_b_model() -> SolidModel {
    part_b_with([1.0, 1.0, 1.0])
}

const N: usize = 12;

fn digests() -> [u64; N] {
    let c0 = CmsOptions::new(0).cutoff(32964.0).threads(1);
    let c1 = c0.clone().residual_vectors(true);
    let pats: Vec<Vec<f64>> = (0..3)
        .map(|c| {
            let mut f = [0.0; 3];
            f[c] = 1.0;
            nodal_load_vector(&part_b_with(f), &[]).unwrap()
        })
        .collect();
    let p = c1.clone().load_patterns(pats.clone());
    let am = part_a_model();
    let bm = part_b_model();
    let a1 = reduce(&am, "xmax", &c1).unwrap();
    let bp = reduce(&bm, "xmin", &p).unwrap();
    let b1 = reduce(&bm, "xmin", &c1).unwrap();
    let a0 = reduce(&am, "xmax", &c0).unwrap();
    let b0 = reduce(&bm, "xmin", &c0).unwrap();
    let s = small_with(true, true);
    let pg = nodal_load_vector(&small_with(false, true), &[]).unwrap();
    let pt = nodal_load_vector(&small_with(true, false), &[]).unwrap();
    let o5 = CmsOptions::new(5)
        .residual_vectors(true)
        .load_pattern(pg)
        .load_pattern(pt)
        .threads(1);
    let sm = reduce(&s, "mid", &o5).unwrap();
    let (tbp, tb1, tsm, ta0) = (bp.to_text(), b1.to_text(), sm.to_text(), a0.to_text());
    let usm = sm.recover_static(&s, &sm.solve_static().unwrap()).unwrap();
    let asm = Assembly::new(vec![a1, bp]).unwrap();
    let cut = Assembly::new(vec![a0, b0]).unwrap();
    let eig = asm.eigen().unwrap().values;
    let eig0 = cut.eigen().unwrap().values;
    let (r1, i1) = asm.harmonic(1000.0, 0.02).unwrap();
    let (r5, i5) = asm.harmonic(5000.0, 0.02).unwrap();
    let ly = asm.parts()[1].reduce_load(&bm, &pats[1]).unwrap();
    let load = asm.load_from(&[Vec::new(), ly]).unwrap();
    let (sy, _) = asm.harmonic_with_load(0.0, 0.0, &load).unwrap();
    let xb = asm.parts()[1].expand(&bm, &asm.part_coordinates(1, &r1)).unwrap();
    [
        text(&tbp),
        text(&tb1),
        text(&tsm),
        text(&ta0),
        fnv(bits(&eig)),
        fnv(bits(&eig0)),
        fnv(bits(&r1).chain(bits(&i1))),
        fnv(bits(&r5).chain(bits(&i5))),
        fnv(bits(&sy)),
        fnv(bits(&xb)),
        fnv(bits(&usm)),
        fnv(bits(asm.load())),
    ]
}

/// Prints the digests (run at main 291a210c to record [`BASE`]).
#[test]
#[ignore = "prints the digests of D6 (ADR 0532); run at the base to record BASE"]
fn print_base_digests() {
    for d in digests() {
        println!("0x{d:016x},");
    }
}

/// The digests printed at main 291a210c (ADR 0532 D6).
const BASE: [u64; N] = [
    0xb34f_564d_474c_f795,
    0xc9cd_5d78_49d6_2d4f,
    0x5d04_5948_1f38_fe2a,
    0x9216_8094_badc_cd27,
    0x20ad_d38c_2e03_fecf,
    0xd499_9d38_1ba4_49ab,
    0xc0c3_4f81_4882_c7a9,
    0x9bc8_dda5_2a44_a66e,
    0xa146_7349_ca54_b1da,
    0xe2dd_ecb4_0467_256d,
    0x2a16_0135_6e74_0c40,
    0xde7a_8691_14fd_78b6,
];

#[test]
fn d6_adr_0511_paths_unchanged() {
    let got = digests();
    let names = [
        "text B the ADR 0511 option (patterns)",
        "text B the ADR 0474 option",
        "text S mid m 5 + vector + patterns",
        "text A cutoff alone",
        "assembly eigenvalues (ADR 0511 option)",
        "assembly eigenvalues (cutoff alone)",
        "assembly harmonic 1000",
        "assembly harmonic 5000",
        "component y statics through reduce_load",
        "B expanded at 1000",
        "S recovered statics",
        "assembly load",
    ];
    let mut bad = Vec::new();
    for i in 0..N {
        if got[i] != BASE[i] {
            bad.push(format!("{}: 0x{:016x} != 0x{:016x}", names[i], got[i], BASE[i]));
        }
    }
    println!("D6: {} of {N} digests equal", N - bad.len());
    assert!(bad.is_empty(), "{bad:?}");
}
