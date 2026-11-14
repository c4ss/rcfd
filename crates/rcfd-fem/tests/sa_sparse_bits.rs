//! F-431 (ADR 0447): the bit record of the default solver paths. Checks B1 to B6 of the registration.
//!
//! This file uses only the API of main 46d78b64: it was run once in the worktree at that commit (plus the registration and
//! this file, no library line changed) before any library code of F-431, and the digests it printed are the constants of
//! `MAIN`. After the change it runs unchanged and compares.
//!
//! Model S: the clamped Hex8 cube `cube(11)` of `si_amg.rs` (ADR 0305; `hex_box([0; 3], [1; 3], [11; 3])`, E = 2e11,
//! nu = 0.3, `xmin` clamped, traction (0, 0, -1e6) on `xmax`) with the density 7850 that a modal analysis needs.

use rcfd_fem::buckling::{factors, BucklingOptions};
use rcfd_fem::builders::hex_box;
use rcfd_fem::geometric::Prestress;
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{Constraint, Load, Material, SolidModel};
use rcfd_linalg::eigen::{
    count_below, lowest_buckling_factors, lowest_eigenpairs, BucklingEigenOptions, SubspaceOptions,
};
use rcfd_linalg::skyline::SkylineOrdering;
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

/// Model S.
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

/// The 5-point Dirichlet Laplacian on `m x m`.
fn lap5(m: usize) -> CsrMatrix {
    let n = m * m;
    let mut t = Vec::new();
    for j in 0..m {
        for i in 0..m {
            let r = j * m + i;
            t.push((r, r, 4.0));
            if i > 0 {
                t.push((r, r - 1, -1.0));
            }
            if i + 1 < m {
                t.push((r, r + 1, -1.0));
            }
            if j > 0 {
                t.push((r, r - m, -1.0));
            }
            if j + 1 < m {
                t.push((r, r + m, -1.0));
            }
        }
    }
    CsrMatrix::from_triplets(n, t)
}

fn identity(n: usize, s: f64) -> CsrMatrix {
    CsrMatrix::from_triplets(n, (0..n).map(|i| (i, i, s)).collect())
}

/// The digests of the cases, in the order of `MAIN`.
fn digests() -> Vec<(&'static str, u64)> {
    let s = model_s();
    let mut out = Vec::new();
    // B1
    let r = solve_static(&s, &StaticOptions::default()).unwrap();
    out.push(("B1 static u", digest(r.displacement_values().iter().copied())));
    out.push(("B1 static reactions", digest(r.reactions().unwrap().iter().copied())));
    // B2
    let md = modal_analysis(&s, &ModalOptions::new(6)).unwrap();
    out.push(("B2 modal eigenvalues", digest(md.eigenvalues.iter().copied())));
    out.push(("B2 modal frequencies", digest(md.frequencies.iter().copied())));
    out.push(("B2 modal modes", digest(md.modes.iter().flatten().copied())));
    out.push(("B2 modal iterations, shift", digest([md.iterations as f64, md.shift])));
    // B3
    let p = Prestress::uniform(&s, [-1.0e8, 0.0, 0.0, 0.0, 0.0, 0.0]).unwrap();
    let bk = factors(&s, &p, &BucklingOptions::new(2)).unwrap();
    out.push(("B3 buckling factors", digest(bk.factors.iter().copied())));
    out.push(("B3 buckling modes", digest(bk.modes.iter().flatten().copied())));
    out.push(("B3 buckling iterations", digest([bk.iterations as f64])));
    // B4
    let k = lap5(30);
    let m = identity(900, 1.0);
    let e = lowest_eigenpairs(&k, &m, 6, &SubspaceOptions::default()).unwrap();
    out.push(("B4 eigen values", digest(e.values.iter().copied())));
    out.push(("B4 eigen vectors", digest(e.vectors.iter().flatten().copied())));
    // B5
    let c = count_below(&k, &m, 0.05, &SkylineOrdering::Rcm).unwrap();
    out.push(("B5 count_below", digest([c as f64])));
    // B6
    let k20 = lap5(20);
    let g = identity(400, -1.0);
    let b = lowest_buckling_factors(&k20, &g, 3, &BucklingEigenOptions::default()).unwrap();
    out.push(("B6 buckling eigen values", digest(b.values.iter().copied())));
    out.push(("B6 buckling eigen vectors", digest(b.vectors.iter().flatten().copied())));
    out
}

/// The digests printed by the run at main 46d78b64 (before any library code of F-431).
const MAIN: [(&str, u64); 14] = [
    ("B1 static u", 0x51127b4fe7a4dd13),
    ("B1 static reactions", 0x94fe8f3bfa7ed28f),
    ("B2 modal eigenvalues", 0x74d22b1ae5dec7ce),
    ("B2 modal frequencies", 0x2af9030dc1b8f716),
    ("B2 modal modes", 0x83ec88e119ee44be),
    ("B2 modal iterations, shift", 0x9c293343a9a2fa38),
    ("B3 buckling factors", 0x6a305588191d92a5),
    ("B3 buckling modes", 0xd92296f0c9f9d117),
    ("B3 buckling iterations", 0x39be1df14e6eb0c2),
    ("B4 eigen values", 0x79f2fd646418a721),
    ("B4 eigen vectors", 0x6b32f12c65652769),
    ("B5 count_below", 0x38ebb0f14dbc2579),
    ("B6 buckling eigen values", 0xacb6927b7dff6149),
    ("B6 buckling eigen vectors", 0xbc17aa543fad79bf),
];

#[test]
fn b1_to_b6_default_paths_keep_their_bits() {
    let got = digests();
    for (name, d) in &got {
        println!("        (\"{name}\", 0x{d:016x}),");
    }
    assert_eq!(MAIN.len(), got.len(), "the constants of MAIN are recorded");
    let mut bad = Vec::new();
    for ((name, want), (gname, d)) in MAIN.iter().zip(&got) {
        assert_eq!(name, gname, "case order");
        if want != d {
            bad.push(*name);
        }
    }
    assert!(bad.is_empty(), "digests differ from main: {bad:?}");
}
