//! F-552 (ADR 0474) D2: the bits of the reduction of ADR 0469 without the option (the part files, the assembly's
//! eigenvalues and harmonic response, the recovery) and of the residual path of ADR 0464 (whose orthogonalisation F-552
//! shares), recorded on main 4abff5fe before any library line of F-552 was written. Uses only the API of that commit.

use rcfd_fem::builders::hex_box;
use rcfd_fem::cms::{condense, reduce, Assembly, CmsOptions};
use rcfd_fem::damped::{damped_modes, DampedModes, DampedOptions};
use rcfd_fem::harmonic_direct::Dashpot;
use rcfd_fem::modal::ModalOptions;
use rcfd_fem::{Constraint, Load, Material, SolidMesh, SolidModel};
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

/// The tip-dashpot bar of ADR 0464 (`sa_damped_residual.rs`): `c = 0.9 Z`, 20 modes, with the residual vectors.
fn damped_tip() -> DampedModes {
    let mut model = SolidModel::new(hex_box([0.0; 3], [1.0, 0.01, 0.01], [100, 1, 1]).unwrap());
    model
        .add_material("all", Material::isotropic_elastic(2.0e11, 0.0, 7850.0, 0.0).unwrap())
        .unwrap();
    model.add_constraint(Constraint::displacement("all", 1, 0.0)).unwrap();
    model.add_constraint(Constraint::displacement("all", 2, 0.0)).unwrap();
    model.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    let c = 0.9 * 1e-4 * (2.0e11f64 * 7850.0).sqrt();
    let modal = ModalOptions::new(20)
        .tolerance(1e-12)
        .residual_test(ResidualTest::Backward);
    let mut o = DampedOptions::with_modal(modal).residual_vectors(true).threads(1);
    for a in model.mesh().node_set("xmax").unwrap().to_vec() {
        o = o.dashpot(Dashpot::ground(a, 0, c / 4.0));
    }
    damped_modes(&model, &o).unwrap()
}

/// Every field of a damped result as bits (the list of `sa_damped_residual.rs`).
fn damped_bits(d: &DampedModes) -> Vec<u64> {
    let mut v: Vec<f64> = Vec::new();
    v.push(d.modes.len() as f64);
    for m in &d.modes {
        v.extend([
            m.eigenvalue.re,
            m.eigenvalue.im,
            m.decay_rate,
            m.damped_omega,
            m.frequency,
            m.damping_ratio,
            f64::from(u8::from(m.oscillating)),
            m.residual,
        ]);
        for s in &m.shape {
            v.extend([s.re, s.im]);
        }
    }
    v.push(d.roots.len() as f64);
    for r in &d.roots {
        v.extend([r.re, r.im]);
    }
    v.extend(d.omegas.iter().copied());
    v.extend([d.basis as f64, d.oscillating as f64, d.non_oscillating as f64]);
    v.push(d.defects.len() as f64);
    for g in &d.defects {
        v.extend(g.indices.iter().map(|&i| i as f64));
        v.extend([g.mean.re, g.mean.im, g.spread]);
    }
    for n in &d.notes {
        v.push(n.len() as f64);
        v.extend(n.bytes().map(f64::from));
    }
    v.push(d.residual_vectors.len() as f64);
    v.extend(d.residual_vectors.iter().flatten().copied());
    v.into_iter().map(f64::to_bits).collect()
}

const N: usize = 11;

fn digests() -> [u64; N] {
    let o = CmsOptions::new(5).threads(1);
    let a = reduce(&part_a_model(), "xmax", &o).unwrap();
    let bm = part_b_model();
    let b = reduce(&bm, "xmin", &o).unwrap();
    let s = small();
    let sx = condense(&s, "xmax", Some(1)).unwrap();
    let sm = reduce(&s, "mid", &o).unwrap();
    let (ta, tb, tsx, tsm) = (a.to_text(), b.to_text(), sx.to_text(), sm.to_text());
    let asm = Assembly::new(vec![a, b]).unwrap();
    let eig = asm.eigen().unwrap().values;
    let (r1, i1) = asm.harmonic(1000.0, 0.02).unwrap();
    let (r5, i5) = asm.harmonic(5000.0, 0.02).unwrap();
    let xb = asm.parts()[1].expand(&bm, &asm.part_coordinates(1, &r1)).unwrap();
    let ub = sx.solve_static().unwrap();
    let us = sx.recover_static(&s, &ub).unwrap();
    [
        text(&ta),
        text(&tb),
        text(&tsx),
        text(&tsm),
        fnv(bits(&eig)),
        fnv(bits(&r1).chain(bits(&i1))),
        fnv(bits(&r5).chain(bits(&i5))),
        fnv(bits(&xb)),
        fnv(bits(&ub)),
        fnv(bits(&us)),
        fnv(damped_bits(&damped_tip())),
    ]
}

/// Prints the digests (run at main 4abff5fe to record [`BASE`]).
#[test]
#[ignore = "prints the digests of D2 (ADR 0474); run at the base to record BASE"]
fn print_base_digests() {
    for d in digests() {
        println!("0x{d:016x},");
    }
}

/// The digests printed at main 4abff5fe (ADR 0474 D2).
const BASE: [u64; N] = [
    0x9216_8094_badc_cd27,
    0xe95e_74e1_876e_c237,
    0x6af2_c2a6_44bf_c513,
    0x2f16_c564_13d5_4c75,
    0x1f7c_ced2_4974_cee6,
    0x70fb_8c42_9d76_ef66,
    0xc629_b892_c14f_2900,
    0x0a04_219d_2f00_a892,
    0x6f67_cb31_e6ea_eba5,
    0xf158_5cf5_aad3_37fe,
    0x4711_2205_f12b_d257,
];

#[test]
fn d2_default_paths_unchanged() {
    let got = digests();
    let names = [
        "text A m 5",
        "text B m 5",
        "text S xmax condensed",
        "text S mid m 5",
        "assembly eigenvalues",
        "assembly harmonic 1000",
        "assembly harmonic 5000",
        "B expanded at 1000",
        "S condensed static u_b",
        "S recovered statics",
        "damped residual path (ADR 0464)",
    ];
    let mut bad = Vec::new();
    for i in 0..N {
        if got[i] != BASE[i] {
            bad.push(format!("{}: 0x{:016x} != 0x{:016x}", names[i], got[i], BASE[i]));
        }
    }
    println!("D2: {} of {N} digests equal", N - bad.len());
    assert!(bad.is_empty(), "{bad:?}");
}
