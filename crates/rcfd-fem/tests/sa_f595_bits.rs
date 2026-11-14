//! F-595 (ADR 0531): the bit record of the default paths and of the F-594 level-structure path that F-595 touches. Check B
//! (cases G1 to G7) of the registration.
//!
//! This file uses only the API of base 291a210c: it was run once in the worktree at that commit (plus the registration and
//! this file, no library line changed) before any library code of F-595, and the digests it printed are the constants of
//! `BASE`. After the change it runs unchanged and compares.

use rcfd_fem::buckling::{factors, BucklingOptions, BucklingResult};
use rcfd_fem::builders::hex_box;
use rcfd_fem::geometric::Prestress;
use rcfd_fem::modal::{modal_analysis, ModalOptions, ModalResult};
use rcfd_fem::{Constraint, Load, Material, SolidModel};
use rcfd_linalg::eigen::{
    lowest_buckling_factors, lowest_eigenpairs, BucklingEigenOptions, Factorisation, SubspaceOptions,
};
use rcfd_linalg::CsrMatrix;

/// FNV-1a 64 over the count and the `to_bits()` of each value, little-endian bytes (the digest of ADR 0291).
fn digest<I: IntoIterator<Item = f64>>(values: I) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |x: u64| {
        for b in x.to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    let mut n = 0u64;
    let mut buf = Vec::new();
    for v in values {
        buf.push(v.to_bits());
        n += 1;
    }
    eat(n);
    for b in buf {
        eat(b);
    }
    h
}

/// Model S: `cube(11, 7850)` of `sa_supernodal_solve.rs`.
fn model_s() -> SolidModel {
    let mut m = SolidModel::new(hex_box([0.0; 3], [1.0; 3], [11; 3]).unwrap());
    m.add_material("all", Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 0.0).unwrap())
        .unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m.add_load(Load::traction("xmax", [0.0, 0.0, -1.0e6])).unwrap();
    m
}

/// Model R: S with a rigid set on `xmax` (the transformation path).
fn model_r() -> SolidModel {
    let mut m = model_s();
    m.add_constraint(Constraint::rigid_set("xmax", [1.0, 0.5, 0.5]))
        .unwrap();
    m
}

fn lap7(m: usize) -> CsrMatrix {
    let id = |i: usize, j: usize, k: usize| i + m * j + m * m * k;
    let mut t = Vec::new();
    for k in 0..m {
        for j in 0..m {
            for i in 0..m {
                let p = id(i, j, k);
                t.push((p, p, 6.0));
                let mut nb = |q: usize| t.push((p, q, -1.0));
                if i > 0 {
                    nb(id(i - 1, j, k));
                }
                if i + 1 < m {
                    nb(id(i + 1, j, k));
                }
                if j > 0 {
                    nb(id(i, j - 1, k));
                }
                if j + 1 < m {
                    nb(id(i, j + 1, k));
                }
                if k > 0 {
                    nb(id(i, j, k - 1));
                }
                if k + 1 < m {
                    nb(id(i, j, k + 1));
                }
            }
        }
    }
    CsrMatrix::from_triplets(m * m * m, t)
}

fn identity(n: usize, s: f64) -> CsrMatrix {
    CsrMatrix::from_triplets(n, (0..n).map(|i| (i, i, s)).collect())
}

fn modal_case(out: &mut Vec<(String, u64)>, tag: &str, m: &ModalResult, record: bool) {
    out.push((format!("{tag} frequencies"), digest(m.frequencies.iter().copied())));
    out.push((format!("{tag} modes"), digest(m.modes.iter().flatten().copied())));
    if record {
        let mut meta = m.residuals.clone();
        meta.push(m.iterations as f64);
        meta.push(m.sturm.shift);
        meta.push(m.sturm.counted as f64);
        meta.push(m.sturm.expected as f64);
        out.push((format!("{tag} record"), digest(meta)));
    }
}

fn buckling_case(out: &mut Vec<(String, u64)>, tag: &str, b: &BucklingResult) {
    out.push((format!("{tag} factors"), digest(b.factors.iter().copied())));
    out.push((format!("{tag} modes"), digest(b.modes.iter().flatten().copied())));
    out.push((format!("{tag} residuals"), digest(b.residuals.iter().copied())));
}

/// The digests of the cases, in the order of `BASE`.
fn digests() -> Vec<(String, u64)> {
    let mut out = Vec::new();
    let s = model_s();
    let sn = Factorisation::Supernodal;
    // G1, G2
    modal_case(
        &mut out,
        "G1 modal",
        &modal_analysis(&s, &ModalOptions::new(6)).unwrap(),
        true,
    );
    modal_case(
        &mut out,
        "G2 levels modal",
        &modal_analysis(&s, &ModalOptions::new(6).factorisation(sn)).unwrap(),
        true,
    );
    // G3, G4
    let p = Prestress::uniform(&s, [-1.0e8, 0.0, 0.0, 0.0, 0.0, 0.0]).unwrap();
    buckling_case(
        &mut out,
        "G3 buckling",
        &factors(&s, &p, &BucklingOptions::new(2)).unwrap(),
    );
    buckling_case(
        &mut out,
        "G4 levels buckling",
        &factors(&s, &p, &BucklingOptions::new(2).factorisation(sn)).unwrap(),
    );
    // G5
    let r = model_r();
    modal_case(
        &mut out,
        "G5 transformation modal",
        &modal_analysis(&r, &ModalOptions::new(6)).unwrap(),
        false,
    );
    modal_case(
        &mut out,
        "G5 transformation levels modal",
        &modal_analysis(&r, &ModalOptions::new(6).factorisation(sn)).unwrap(),
        false,
    );
    // G6, G7
    let k = lap7(12);
    let n = k.n;
    for (tag, f) in [("default", Factorisation::Envelope), ("levels", sn)] {
        let opts = SubspaceOptions {
            factorisation: f,
            ..SubspaceOptions::default()
        };
        let e = lowest_eigenpairs(&k, &identity(n, 1.0), 5, &opts).unwrap();
        out.push((format!("G6 {tag} values"), digest(e.values.iter().copied())));
        out.push((format!("G6 {tag} vectors"), digest(e.vectors.iter().flatten().copied())));
        let bopts = BucklingEigenOptions {
            factorisation: f,
            ..BucklingEigenOptions::default()
        };
        let b = lowest_buckling_factors(&k, &identity(n, -1.0), 3, &bopts).unwrap();
        out.push((format!("G7 {tag} values"), digest(b.values.iter().copied())));
        out.push((format!("G7 {tag} vectors"), digest(b.vectors.iter().flatten().copied())));
    }
    out
}

/// The digests printed by the run at base 291a210c (before any library code of F-595).
const BASE: [(&str, u64); 24] = [
    ("G1 modal frequencies", 0x2af9030dc1b8f716),
    ("G1 modal modes", 0x83ec88e119ee44be),
    ("G1 modal record", 0x0dd6a298721f3e20),
    ("G2 levels modal frequencies", 0x9ae89b652c1188ae),
    ("G2 levels modal modes", 0xddc8701f8ac5618a),
    ("G2 levels modal record", 0x52b2d1e3caf6dac0),
    ("G3 buckling factors", 0x6a305588191d92a5),
    ("G3 buckling modes", 0xd92296f0c9f9d117),
    ("G3 buckling residuals", 0x79d086a577aba762),
    ("G4 levels buckling factors", 0xc605fffc96136bf6),
    ("G4 levels buckling modes", 0x29e39a32a5c3a2df),
    ("G4 levels buckling residuals", 0x89755dd14ee488d7),
    ("G5 transformation modal frequencies", 0x3222abe6e8db816f),
    ("G5 transformation modal modes", 0x56e04ee31abeef69),
    ("G5 transformation levels modal frequencies", 0xfee1bd986acd5589),
    ("G5 transformation levels modal modes", 0x1a27fe1e9dc2be94),
    ("G6 default values", 0x6b87e2f21095beb9),
    ("G6 default vectors", 0x5a3d9c2843ccfc41),
    ("G7 default values", 0x144a2d0de0a8f16e),
    ("G7 default vectors", 0x43e478b0ce11c709),
    ("G6 levels values", 0xf08dc33e84c19543),
    ("G6 levels vectors", 0xca672e246216c991),
    ("G7 levels values", 0x9aede637d0cbc7d2),
    ("G7 levels vectors", 0xc76815a2a520690b),
];

#[test]
fn b_default_paths_keep_their_bits() {
    let got = digests();
    for (name, d) in &got {
        println!("    (\"{name}\", 0x{d:016x}),");
    }
    assert_eq!(BASE.len(), got.len(), "the constants of BASE are recorded");
    let mut bad = Vec::new();
    for ((name, want), (gname, d)) in BASE.iter().zip(&got) {
        assert_eq!(name, gname, "case order");
        if want != d {
            bad.push(*name);
        }
    }
    assert!(bad.is_empty(), "digests differ from base: {bad:?}");
}
