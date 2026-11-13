//! F-388 (ADR 0291): the bit record of the basic models. Checks B1 to B4 and the negative control N1 of the registration.
//!
//! This file uses only the API of main ee449e26: it was run on a `git archive` copy of that commit before any library code
//! of F-388, and the digests it printed are the constants of `MAIN`. After the change it runs in the tree and compares.

use rcfd_core::vec3::Vec3;
use rcfd_fem::assemble::stiffness;
use rcfd_fem::builders::{hex_box, tet_box, upgrade_to_tet10, BOX_FACES};
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::recover::{nodal_average, IntegrationPointField};
use rcfd_fem::solve::{solve_static_with, CgPreconditioner, StaticOptions, StaticSolver};
use rcfd_fem::{Constraint, Load, Material, SolidMesh, SolidModel};

/// FNV-1a 64 over the count and the `to_bits()` of each value, little-endian bytes.
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

fn digest_usize(values: &[usize]) -> u64 {
    digest(values.iter().map(|&v| f64::from_bits(v as u64)))
}

fn steel() -> Material {
    Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 0.0).unwrap()
}

fn model_of(mesh: SolidMesh) -> SolidModel {
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    m
}

fn tet10_box(origin: Vec3, lengths: Vec3, div: [usize; 3]) -> SolidMesh {
    upgrade_to_tet10(&tet_box(origin, lengths, div).unwrap()).unwrap().0
}

fn patch_field(x: Vec3) -> Vec3 {
    [
        1e-3 * (x[0] + 2.0 * x[1] - x[2]),
        1e-3 * (0.5 * x[0] - x[1] + 3.0 * x[2]),
        1e-3 * (-x[0] + x[1] + 0.7 * x[2]),
    ]
}

const P_ORIGIN: Vec3 = [0.1, -0.2, 0.3];
const P_LENGTHS: Vec3 = [1.2, 1.0, 0.8];

/// The patch models P (Hex8 and Tet10) with the per-node prescribed values on every boundary node.
fn patch_models() -> Vec<(&'static str, SolidModel, Vec<Prescribed>)> {
    let meshes = [
        ("P-Hex8", hex_box(P_ORIGIN, P_LENGTHS, [3, 3, 3]).unwrap()),
        ("P-Tet10", tet10_box(P_ORIGIN, P_LENGTHS, [2, 2, 2])),
    ];
    meshes
        .into_iter()
        .map(|(name, mesh)| {
            let mut boundary: Vec<usize> = BOX_FACES
                .iter()
                .flat_map(|f| mesh.node_set(f).unwrap().to_vec())
                .collect();
            boundary.sort_unstable();
            boundary.dedup();
            let mut pres = Vec::new();
            for &n in &boundary {
                let u = patch_field(mesh.nodes()[n]);
                for (c, &v) in u.iter().enumerate() {
                    pres.push(Prescribed::new(n, c, v));
                }
            }
            (name, model_of(mesh), pres)
        })
        .collect()
}

/// The cantilever C.
fn cantilever() -> SolidModel {
    let mesh = hex_box([0.0; 3], [1.0, 0.1, 0.1], [20, 2, 2]).unwrap();
    let mut m = model_of(mesh);
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m.add_load(Load::nodal_force("xmax", [0.0, 0.0, -100.0])).unwrap();
    m
}

/// The bar models B (Hex8 and Tet10), `xmin` clamped.
fn bars() -> Vec<(&'static str, SolidModel)> {
    let meshes = [
        ("B-Hex8", hex_box([0.0; 3], [1.0, 0.1, 0.1], [10, 1, 1]).unwrap()),
        ("B-Tet10", tet10_box([0.0; 3], [1.0, 0.1, 0.1], [6, 1, 1])),
    ];
    meshes
        .into_iter()
        .map(|(name, mesh)| {
            let mut m = model_of(mesh);
            for c in 0..3 {
                m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
            }
            (name, m)
        })
        .collect()
}

fn flat6(v: &[[f64; 6]]) -> Vec<f64> {
    v.iter().flat_map(|s| s.iter().copied()).collect()
}

/// Every digest of the registered models, in a fixed order, with its name.
fn all_digests() -> Vec<(String, u64)> {
    let mut out = Vec::new();
    let mut push = |name: String, d: u64| {
        println!("    (\"{name}\", 0x{d:016x}),");
        out.push((name, d));
    };
    for (name, model, pres) in patch_models() {
        let r = solve_static_with(&model, &pres, &[], &StaticOptions::default().threads(1)).unwrap();
        push(format!("{name} displacement"), digest(r.displacement_values().iter().copied()));
        push(format!("{name} reactions"), digest(r.reactions().unwrap().iter().copied()));
        let field = IntegrationPointField::from_result(&model, &r).unwrap();
        push(format!("{name} stress"), digest(flat6(field.stress())));
        let k = stiffness(&model).unwrap();
        push(format!("K {name} pattern"), digest_usize(&k.row_ptr) ^ digest_usize(&k.col).rotate_left(1));
        push(format!("K {name} values"), digest(k.val.iter().copied()));
    }
    let c = cantilever();
    let solvers = [
        ("C-direct", StaticOptions::default().threads(1)),
        (
            "C-pcg",
            StaticOptions::with_solver(StaticSolver::PreconditionedCg {
                preconditioner: CgPreconditioner::BlockJacobi,
                tolerance: 1e-8,
                max_iterations: 20000,
            }),
        ),
    ];
    for (name, opts) in solvers {
        let r = solve_static_with(&c, &[], &[], &opts).unwrap();
        push(format!("{name} displacement"), digest(r.displacement_values().iter().copied()));
        push(format!("{name} reactions"), digest(r.reactions().unwrap().iter().copied()));
        let field = IntegrationPointField::from_result(&c, &r).unwrap();
        push(format!("{name} stress"), digest(flat6(field.stress())));
        let nodal = nodal_average(&c, &field, field.stress(), None).unwrap();
        push(
            format!("{name} nodal stress"),
            digest(nodal.iter().flat_map(|s| s.unwrap_or([f64::NAN; 6]))),
        );
    }
    let k = stiffness(&c).unwrap();
    push("K C pattern".into(), digest_usize(&k.row_ptr) ^ digest_usize(&k.col).rotate_left(1));
    push("K C values".into(), digest(k.val.iter().copied()));
    for (name, model) in bars() {
        let r = modal_analysis(&model, &ModalOptions::new(4)).unwrap();
        push(format!("{name} eigenvalues"), digest(r.eigenvalues.iter().copied()));
        push(format!("{name} frequencies"), digest(r.frequencies.iter().copied()));
        push(format!("{name} modes"), digest(r.modes.iter().flatten().copied()));
    }
    out
}

/// The digests printed by this file on main ee449e26 (record run of the registration).
const MAIN: &[(&str, u64)] = &[
    ("P-Hex8 displacement", 0xb63760e800d5386b),
    ("P-Hex8 reactions", 0xb1f9e97743715c44),
    ("P-Hex8 stress", 0x977d10acac84df90),
    ("K P-Hex8 pattern", 0xd4ad9c039a8591a9),
    ("K P-Hex8 values", 0x0feb5c655461b48b),
    ("P-Tet10 displacement", 0x9db5fd3873108200),
    ("P-Tet10 reactions", 0xbda9b12b25ef84e3),
    ("P-Tet10 stress", 0x5d6c91c973442d5c),
    ("K P-Tet10 pattern", 0xfc495cf74e9afccf),
    ("K P-Tet10 values", 0x558f616371f10b4f),
    ("C-direct displacement", 0x36ba978186575b91),
    ("C-direct reactions", 0x013a7fa67b8d609e),
    ("C-direct stress", 0xf8195fc6f210799e),
    ("C-direct nodal stress", 0xff16adba1f5ef2e5),
    ("C-pcg displacement", 0x92a7cebf5760028a),
    ("C-pcg reactions", 0x69d30f77b48fec73),
    ("C-pcg stress", 0x9e606f6eaa329775),
    ("C-pcg nodal stress", 0xf20167aa66172022),
    ("K C pattern", 0x87caf720b3a8d1b7),
    ("K C values", 0x08aeab234e3188f7),
    ("B-Hex8 eigenvalues", 0x52252f590ed9ba7d),
    ("B-Hex8 frequencies", 0xfa31dd28316c8b14),
    ("B-Hex8 modes", 0x3890924ef2010520),
    ("B-Tet10 eigenvalues", 0xa41ea8b12eed3386),
    ("B-Tet10 frequencies", 0x33db964b9d3e6d7c),
    ("B-Tet10 modes", 0x109c955ae7319ba4),
];

/// B1 to B4: every digest equals main's.
#[test]
fn b1_to_b4_basic_models_have_the_bits_of_main() {
    println!("digests of this tree:");
    let now = all_digests();
    assert!(!MAIN.is_empty(), "record run: no digests of main recorded yet");
    assert_eq!(now.len(), MAIN.len(), "number of digests");
    let mut missed = Vec::new();
    for ((name, d), (mname, md)) in now.iter().zip(MAIN) {
        assert_eq!(name, mname, "digest order");
        let ok = d == md;
        println!("{name}: {} (main 0x{md:016x}, now 0x{d:016x})", if ok { "equal" } else { "DIFFERS" });
        if !ok {
            missed.push(name.clone());
        }
    }
    assert!(missed.is_empty(), "digests differ from main: {missed:?}");
}

/// N1: one ulp in one displacement entry changes the digest.
#[test]
fn n1_digest_sees_one_ulp() {
    let (_, model, pres) = patch_models().remove(0);
    let r = solve_static_with(&model, &pres, &[], &StaticOptions::default().threads(1)).unwrap();
    let mut u = r.displacement_values().to_vec();
    let a = digest(u.iter().copied());
    let k = u.len() / 2;
    u[k] = f64::from_bits(u[k].to_bits() + 1);
    let b = digest(u.iter().copied());
    println!("N1: 0x{a:016x} -> 0x{b:016x}");
    assert_ne!(a, b);
}
