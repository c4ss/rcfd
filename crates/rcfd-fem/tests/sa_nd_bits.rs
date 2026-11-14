//! F-529 (ADR 0503): the bit record of the default and existing sparse paths. Check B (cases D1 to D5) of the registration.
//!
//! This file uses only the API of base 98e767f5: it was run once in the worktree at that commit (plus the registration and
//! this file, no library line changed) before any library code of F-529, and the digests it printed are the constants of
//! `BASE`. After the change it runs unchanged and compares.

use rcfd_fem::builders::hex_box;
use rcfd_fem::solve::{solve_static_report, StaticOptions, StaticSolver};
use rcfd_fem::{Constraint, Load, Material, SolidModel};
use rcfd_linalg::sparse_chol::{minimum_degree_order, SparseLdlt, SparseOptions, SparseOrdering};
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

/// Model S: `cube(11, 7850)` of `sa_sparse_solve.rs`.
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

fn lap5(m: usize) -> CsrMatrix {
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
    CsrMatrix::from_triplets(m * m, t)
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
    let mut s: u64 = 0xF529_0503;
    (0..n)
        .map(|_| {
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            2.0 * ((s >> 11) as f64 / (1u64 << 53) as f64) - 1.0
        })
        .collect()
}

fn static_case(out: &mut Vec<(&'static str, u64)>, names: [&'static str; 3], solver: StaticSolver) {
    let (r, rep) = solve_static_report(&model_s(), &[], &[], &StaticOptions::with_solver(solver)).unwrap();
    out.push((names[0], digest(r.displacement_values().iter().copied())));
    out.push((names[1], digest(r.reactions().unwrap().iter().copied())));
    let mut meta: Vec<f64> = rep.solver.bytes().map(f64::from).collect();
    meta.push(rep.iterations as f64);
    meta.push(rep.restarts as f64);
    meta.push(rep.relative_residual);
    out.push((names[2], digest(meta)));
}

/// The digests of the cases, in the order of `BASE`.
fn digests() -> Vec<(&'static str, u64)> {
    let mut out = Vec::new();
    // D1, D2
    static_case(
        &mut out,
        ["D1 direct u", "D1 direct reactions", "D1 direct report"],
        StaticSolver::Direct,
    );
    static_case(
        &mut out,
        ["D2 sparse u", "D2 sparse reactions", "D2 sparse report"],
        StaticSolver::SparseDirect,
    );
    // D3
    let a = lap7(20);
    let f = SparseLdlt::factor(&a).unwrap();
    out.push(("D3 order", digest_usize(f.order().iter().copied())));
    out.push((
        "D3 parent",
        digest_usize(f.parent().iter().map(|p| p.unwrap_or(usize::MAX))),
    ));
    let mut vals = Vec::new();
    let mut rows = Vec::new();
    for j in 0..f.n() {
        let (r, v) = f.column(j);
        rows.extend_from_slice(r);
        vals.extend_from_slice(v);
    }
    out.push(("D3 L rows", digest_usize(rows)));
    out.push(("D3 L values", digest(vals)));
    out.push(("D3 D", digest(f.pivots().iter().copied())));
    out.push(("D3 solve", digest(f.solve(&rhs(a.n)))));
    // D4
    out.push(("D4 md order 100^2", digest_usize(minimum_degree_order(&lap5(100)))));
    // D5
    let a = lap5(30);
    let b = rhs(a.n);
    let rev: Vec<usize> = (0..a.n).rev().collect();
    for (name_d, name_x, ordering) in [
        ("D5 natural D", "D5 natural solve", SparseOrdering::Natural),
        ("D5 reversed D", "D5 reversed solve", SparseOrdering::Given(rev)),
    ] {
        let f = SparseLdlt::factor_with(
            &a,
            &SparseOptions {
                ordering,
                ..SparseOptions::default()
            },
        )
        .unwrap();
        out.push((name_d, digest(f.pivots().iter().copied())));
        out.push((name_x, digest(f.solve(&b))));
    }
    out
}

/// The digests printed by the run at base 98e767f5 (before any library code of F-529).
const BASE: [(&str, u64); 17] = [
    ("D1 direct u", 0x51127b4fe7a4dd13),
    ("D1 direct reactions", 0x94fe8f3bfa7ed28f),
    ("D1 direct report", 0x4c7311d7e94360b2),
    ("D2 sparse u", 0xd134659ab26cf909),
    ("D2 sparse reactions", 0xb8b394665fd30412),
    ("D2 sparse report", 0x354d77e3fe5418ae),
    ("D3 order", 0x7bd5a92d04a54100),
    ("D3 parent", 0x3f031814dede70eb),
    ("D3 L rows", 0x06c298d86fa8b08d),
    ("D3 L values", 0x30b3711d038561ea),
    ("D3 D", 0xbc5ad408ec316795),
    ("D3 solve", 0x630d92a4457a31e2),
    ("D4 md order 100^2", 0x547b6b09e4b837f0),
    ("D5 natural D", 0xac5e84c3ea7306d2),
    ("D5 natural solve", 0x7421612f2decd14c),
    ("D5 reversed D", 0xac5e84c3ea7306d2),
    ("D5 reversed solve", 0x01a3ee389fb2a5f6),
];

#[test]
fn b_default_and_existing_sparse_paths_keep_their_bits() {
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
