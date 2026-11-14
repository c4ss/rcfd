//! F-537 (ADR 0457): the bit record of the default paths that F-537 touches (check B1 of the registration).
//!
//! This file uses only the API of main dab660ed: it was run once in the worktree at that commit (no library line changed)
//! before any library code of F-537, and the digests it printed are the constants of `BASE`. After the change it runs
//! unchanged and compares. The models cover the modal analysis on the elimination path (Hex20, with and without a
//! temperature field; Shell4), the modal analysis and the static solve on the transformation path (a linear constraint and
//! a nodal spring), the pre-stressed modes on both paths (from a static result and uniform), the buckling factors and the
//! public geometric stiffness.

use rcfd_fem::buckling::{factors, BucklingOptions};
use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::geometric::{geometric_stiffness, prestressed_modes, Prestress};
use rcfd_fem::hex20::upgrade_to_hex20;
use rcfd_fem::loads::NodalForceAt;
use rcfd_fem::mesh::ElementKind;
use rcfd_fem::modal::{modal_analysis, modal_analysis_with, ModalOptions, ModalResult};
use rcfd_fem::model::{ElasticSupport, LinearTerm};
use rcfd_fem::solve::{solve_static, solve_static_with, StaticOptions};
use rcfd_fem::{BlockProperty, Constraint, Load, Material, SolidMesh, SolidModel};
use rcfd_linalg::eigen::ResidualTest;

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

fn steel() -> Material {
    Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 1.2e-5).unwrap()
}

fn opts(n: usize) -> ModalOptions {
    ModalOptions::new(n)
        .residual_test(ResidualTest::Backward)
        .tolerance(1e-12)
}

fn modal_digests(tag: &str, r: &ModalResult, out: &mut Vec<(String, u64)>) {
    out.push((format!("{tag} eigenvalues"), digest(r.eigenvalues.iter().copied())));
    out.push((format!("{tag} modes"), digest(r.modes.iter().flatten().copied())));
}

/// A Hex20 cantilever 10 x 1 x 1 (`L = 2`, `h = 0.1`), clamped at `xmin`.
fn hex20_cantilever(temperature: bool) -> SolidModel {
    let base = hex_box([0.0; 3], [2.0, 0.1, 0.1], [10, 1, 1]).unwrap();
    let mesh = upgrade_to_hex20(&base).unwrap().0;
    let t: Vec<f64> = mesh.nodes().iter().map(|x| 293.0 + 60.0 * x[0]).collect();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    if temperature {
        m.add_load(Load::temperature(t, 293.0)).unwrap();
    }
    m
}

/// A Hex8 cantilever 6 x 1 x 1 (`L = 1.2`, `h = 0.2`): `xmin` clamped in x and y, z held by linear constraints `u_z = 0`
/// node by node, a spring in z on the `xmax` nodes.
fn hex8_transformation() -> SolidModel {
    let mut mesh = hex_box([0.0; 3], [1.2, 0.2, 0.2], [6, 1, 1]).unwrap();
    let root: Vec<usize> = mesh.node_set("xmin").unwrap().to_vec();
    let tip: Vec<usize> = mesh.node_set("xmax").unwrap().to_vec();
    mesh.add_node_set("tip", tip).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 1, 0.0)).unwrap();
    for &n in &root {
        m.add_constraint(Constraint::linear(vec![LinearTerm::new(n, 2, 1.0)], 0.0))
            .unwrap();
    }
    m.add_support(ElasticSupport::nodal_spring("tip", 2, 2.0e6)).unwrap();
    m
}

fn shell_plate() -> (SolidModel, Vec<Prescribed>) {
    let n = 4;
    let idx = |i: usize, j: usize| j * (n + 1) + i;
    let mut nodes = Vec::new();
    for j in 0..=n {
        for i in 0..=n {
            nodes.push([i as f64 / n as f64, j as f64 / n as f64, 0.0]);
        }
    }
    let mut conn = Vec::new();
    for j in 0..n {
        for i in 0..n {
            conn.extend([idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1)]);
        }
    }
    let mut pres = Vec::new();
    for (k, x) in nodes.iter().enumerate() {
        if x[0] == 0.0 {
            for comp in 0..6 {
                pres.push(Prescribed::new(k, comp, 0.0));
            }
        }
    }
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Shell4, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::shell(0.01).unwrap()).unwrap();
    mesh.add_element_set("all", (0..n * n).collect()).unwrap();
    let mut s = SolidModel::new(mesh);
    s.add_material("all", steel()).unwrap();
    (s, pres)
}

fn all_digests() -> Vec<(String, u64)> {
    let mut out = Vec::new();
    // Modal on the elimination path, without and with a temperature field.
    let m = hex20_cantilever(false);
    modal_digests("hex20 modal", &modal_analysis(&m, &opts(4)).unwrap(), &mut out);
    let mt = hex20_cantilever(true);
    modal_digests(
        "hex20 modal temperature",
        &modal_analysis(&mt, &opts(4)).unwrap(),
        &mut out,
    );
    // Pre-stressed modes on the elimination path from a static result (axial tension at the tip).
    let mut ms = hex20_cantilever(false);
    ms.add_load(Load::nodal_force("xmax", [2.0e4, 0.0, 0.0])).unwrap();
    let res = solve_static(&ms, &StaticOptions::default()).unwrap();
    let pre = Prestress::from_static(&ms, &res).unwrap();
    out.push(("hex20 prestress".into(), digest(pre.stress().iter().flatten().copied())));
    modal_digests(
        "hex20 prestressed",
        &prestressed_modes(&ms, &pre, &opts(4)).unwrap(),
        &mut out,
    );
    let kg = geometric_stiffness(&ms, &pre, None).unwrap();
    out.push(("hex20 geometric stiffness".into(), digest(kg.val.iter().copied())));
    // Buckling of the cantilever under a uniform compression.
    let comp = Prestress::uniform(&m, [-1.0e6, 0.0, 0.0, 0.0, 0.0, 0.0]).unwrap();
    out.push((
        "hex20 uniform prestress".into(),
        digest(comp.stress().iter().flatten().copied()),
    ));
    let b = factors(&m, &comp, &BucklingOptions::new(2)).unwrap();
    out.push(("hex20 buckling".into(), digest(b.factors.iter().copied())));
    // The transformation path: modal, static, pre-stressed.
    let t = hex8_transformation();
    modal_digests("hex8 mpc modal", &modal_analysis(&t, &opts(3)).unwrap(), &mut out);
    let tip = t.mesh().node_set("tip").unwrap()[0];
    let forces = [NodalForceAt::new(tip, [0.0, 0.0, -500.0])];
    let st = solve_static_with(&t, &[], &forces, &StaticOptions::default()).unwrap();
    out.push((
        "hex8 mpc static".into(),
        digest(st.displacement_values().iter().copied()),
    ));
    let tpre = Prestress::uniform(&t, [5.0e6, 0.0, 0.0, 0.0, 0.0, 0.0]).unwrap();
    modal_digests(
        "hex8 mpc prestressed",
        &prestressed_modes(&t, &tpre, &opts(3)).unwrap(),
        &mut out,
    );
    // Shell4 modal.
    let (s, pres) = shell_plate();
    modal_digests(
        "shell4 modal",
        &modal_analysis_with(&s, &pres, &opts(3)).unwrap(),
        &mut out,
    );
    out
}

/// The digests printed by the run at main dab660ed (`None` before that run).
const BASE: Option<&[(&str, u64)]> = Some(&[
    ("hex20 modal eigenvalues", 0x7ac87a1cda350ba4),
    ("hex20 modal modes", 0x87cc3cf5b6ef2529),
    ("hex20 modal temperature eigenvalues", 0x7ac87a1cda350ba4),
    ("hex20 modal temperature modes", 0x87cc3cf5b6ef2529),
    ("hex20 prestress", 0xbc92e71cfb0c765a),
    ("hex20 prestressed eigenvalues", 0x0c3c6e1d7da0994e),
    ("hex20 prestressed modes", 0x686ae27126a10b91),
    ("hex20 geometric stiffness", 0x9c1d8c815da3795d),
    ("hex20 uniform prestress", 0x9402588f0e3d7373),
    ("hex20 buckling", 0x9313f53e8738c02d),
    ("hex8 mpc modal eigenvalues", 0x9fdb9c6275c46bd7),
    ("hex8 mpc modal modes", 0x3f85ea877ae9269e),
    ("hex8 mpc static", 0x09ca66811dd00543),
    ("hex8 mpc prestressed eigenvalues", 0xae17c3efd47b25a3),
    ("hex8 mpc prestressed modes", 0x754ead8b0745a044),
    ("shell4 modal eigenvalues", 0x1ecace4336a5cab3),
    ("shell4 modal modes", 0xe91e6b993154ce9a),
]);

#[test]
fn b1_default_paths_bits() {
    let got = all_digests();
    for (name, d) in &got {
        println!("(\"{name}\", 0x{d:016x}),");
    }
    let base = BASE.expect("the record of main dab660ed is filled in");
    let mut failed = Vec::new();
    assert_eq!(got.len(), base.len(), "number of digests");
    for ((name, d), (bname, bd)) in got.iter().zip(base) {
        let ok = name == bname && d == bd;
        println!("B1 {name}: {}", if ok { "met" } else { "MISSED" });
        if !ok {
            failed.push(name.clone());
        }
    }
    assert!(failed.is_empty(), "missed: {failed:?}");
}
