//! F-594 (ADR 0509): the bit record of the default paths that F-594 touches. Check B (cases G1 to G8) of the registration.
//!
//! This file uses only the API of base 624909b9: it was run once in the worktree at that commit (plus the registration and
//! this file, no library line changed) before any library code of F-594, and the digests it printed are the constants of
//! `BASE`. After the change it runs unchanged and compares.

use rcfd_fem::buckling::{factors, BucklingOptions};
use rcfd_fem::builders::hex_box;
use rcfd_fem::geometric::Prestress;
use rcfd_fem::harmonic::HarmonicLoad;
use rcfd_fem::harmonic_direct::{DirectHarmonic, DirectHarmonicOptions, LossFactor};
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::solve::{solve_static_report, StaticOptions, StaticSolver};
use rcfd_fem::{Constraint, Load, Material, SolidModel};
use rcfd_linalg::eigen::Factorisation;
use rcfd_linalg::sparse_chol::SupernodalLdlt;
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

fn digest_usize<I: IntoIterator<Item = usize>>(values: I) -> u64 {
    digest(values.into_iter().map(|u| f64::from_bits(u as u64)))
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

fn rhs(n: usize) -> Vec<f64> {
    let mut s: u64 = 0xF594_0509;
    (0..n)
        .map(|_| {
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            2.0 * ((s >> 11) as f64 / (1u64 << 53) as f64) - 1.0
        })
        .collect()
}

fn static_case(out: &mut Vec<(&'static str, u64)>, names: [&'static str; 3], model: &SolidModel, solver: StaticSolver) {
    let (r, rep) = solve_static_report(model, &[], &[], &StaticOptions::with_solver(solver)).unwrap();
    out.push((names[0], digest(r.displacement_values().iter().copied())));
    out.push((names[1], digest(r.reactions().unwrap().iter().copied())));
    let mut meta: Vec<f64> = rep.solver.bytes().map(f64::from).collect();
    meta.push(rep.iterations as f64);
    meta.push(rep.relative_residual);
    out.push((names[2], digest(meta)));
}

/// The digests of the cases, in the order of `BASE`.
fn digests() -> Vec<(&'static str, u64)> {
    let mut out = Vec::new();
    let s = model_s();
    // G1
    let m = modal_analysis(&s, &ModalOptions::new(6)).unwrap();
    out.push(("G1 modal frequencies", digest(m.frequencies.iter().copied())));
    out.push(("G1 modal modes", digest(m.modes.iter().flatten().copied())));
    let mut meta = m.residuals.clone();
    meta.push(m.iterations as f64);
    meta.push(m.sturm.shift);
    meta.push(m.sturm.counted as f64);
    meta.push(m.sturm.expected as f64);
    out.push(("G1 modal record", digest(meta)));
    let w1 = m.eigenvalues[0].sqrt();
    // G2
    let ms = modal_analysis(&s, &ModalOptions::new(6).factorisation(Factorisation::Sparse)).unwrap();
    out.push(("G2 sparse modal frequencies", digest(ms.frequencies.iter().copied())));
    out.push(("G2 sparse modal modes", digest(ms.modes.iter().flatten().copied())));
    // G3
    let p = Prestress::uniform(&s, [-1.0e8, 0.0, 0.0, 0.0, 0.0, 0.0]).unwrap();
    let b = factors(&s, &p, &BucklingOptions::new(2)).unwrap();
    out.push(("G3 buckling factors", digest(b.factors.iter().copied())));
    out.push(("G3 buckling modes", digest(b.modes.iter().flatten().copied())));
    // G4
    let h = DirectHarmonic::new(
        &s,
        &DirectHarmonicOptions::new().loss_factor("all", LossFactor::Constant(0.02)),
    )
    .unwrap();
    let r = h.response(&HarmonicLoad::from_model(&s).unwrap(), 1.5 * w1).unwrap();
    out.push(("G4 harmonic re", digest(r.re.iter().copied())));
    out.push(("G4 harmonic im", digest(r.im.iter().copied())));
    // G5
    let rr = model_r();
    static_case(
        &mut out,
        ["G5 transformation u", "G5 transformation reactions", "G5 transformation report"],
        &rr,
        StaticSolver::Direct,
    );
    // G6
    let mr = modal_analysis(&rr, &ModalOptions::new(6)).unwrap();
    out.push(("G6 transformation modal frequencies", digest(mr.frequencies.iter().copied())));
    out.push(("G6 transformation modal modes", digest(mr.modes.iter().flatten().copied())));
    // G7
    let a = lap7(20);
    let f = SupernodalLdlt::factor(&a).unwrap();
    out.push(("G7 order", digest_usize(f.order().iter().copied())));
    let mut vals = Vec::new();
    let mut rows = Vec::new();
    for j in 0..f.n() {
        let (r, v) = f.column(j);
        rows.extend_from_slice(r);
        vals.extend_from_slice(v);
    }
    out.push(("G7 L rows", digest_usize(rows)));
    out.push(("G7 L values", digest(vals)));
    out.push(("G7 D", digest(f.pivots().iter().copied())));
    out.push(("G7 solve", digest(f.solve(&rhs(a.n)))));
    // G8
    static_case(
        &mut out,
        ["G8 supernodal u", "G8 supernodal reactions", "G8 supernodal report"],
        &s,
        StaticSolver::SupernodalDirect,
    );
    out
}

/// The digests printed by the run at base 624909b9 (before any library code of F-594).
const BASE: [(&str, u64); 22] = [
    ("G1 modal frequencies", 0x2af9030dc1b8f716),
    ("G1 modal modes", 0x83ec88e119ee44be),
    ("G1 modal record", 0x0dd6a298721f3e20),
    ("G2 sparse modal frequencies", 0x9c6dc2b0118000dd),
    ("G2 sparse modal modes", 0x8e1db5fb12d631ca),
    ("G3 buckling factors", 0x6a305588191d92a5),
    ("G3 buckling modes", 0xd92296f0c9f9d117),
    ("G4 harmonic re", 0x948146afbce92eed),
    ("G4 harmonic im", 0xa1fba8febbaa1c1b),
    ("G5 transformation u", 0x8aca1447fe73114c),
    ("G5 transformation reactions", 0xcf25f3c1bf574988),
    ("G5 transformation report", 0x247bba82b1209768),
    ("G6 transformation modal frequencies", 0x3222abe6e8db816f),
    ("G6 transformation modal modes", 0x56e04ee31abeef69),
    ("G7 order", 0xb228cfc978091c38),
    ("G7 L rows", 0x9753d84b94221b96),
    ("G7 L values", 0xf411b0e853f5d915),
    ("G7 D", 0x43a9f1cd0461fd62),
    ("G7 solve", 0x19a672ad9e57016c),
    ("G8 supernodal u", 0x19ec6031bcfbfd3f),
    ("G8 supernodal reactions", 0x386b505edcd93796),
    ("G8 supernodal report", 0x0df3c68603eb471c),
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
