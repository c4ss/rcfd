//! F-420 (ADR 0473): the bit record of the default paths that F-420 touches (check B1b of the registration).
//!
//! This file uses only the API of main 4abff5fe: it was run once in the worktree at that commit with the registration
//! committed and no library line changed, and the digests it printed are the constants of `BASE`. After the change it runs
//! unchanged and compares. The models are of solid elements only (buckling and pre-stressed modes on both paths, a static and
//! a uniform pre-stress, the assembled geometric stiffness, the rotating part of F-396 with its Campbell table), and the load
//! vectors of the shared `loads.rs` hook on shells and a beam (gravity and the centrifugal load on Shell4 and Shell3, gravity
//! on a beam, a static solve of a beam with gravity).

use rcfd_core::vec3::Vec3;
use rcfd_fem::buckling::{factors, BucklingOptions};
use rcfd_fem::builders::hex_box;
use rcfd_fem::geometric::{geometric_stiffness, prestressed_modes, Prestress};
use rcfd_fem::hex20::upgrade_to_hex20;
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::modal::{ModalOptions, ModalResult};
use rcfd_fem::model::ElasticSupport;
use rcfd_fem::rotating::{spin_softening_mass, RotatingOptions, RotatingPart, SpeedModes};
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{BeamSection, BlockProperty, Constraint, ElementKind, Load, Material, SolidMesh, SolidModel};
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

const AXIS: Vec3 = [0.0, 0.0, 1.0];
const ORIGIN: Vec3 = [0.0; 3];

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

/// A Hex20 cantilever 10 x 1 x 1 (`L = 1`, `h = 0.1`), clamped at `xmin`.
fn hex20_cantilever() -> SolidModel {
    let base = hex_box([0.0; 3], [1.0, 0.1, 0.1], [10, 1, 1]).unwrap();
    let mesh = upgrade_to_hex20(&base).unwrap().0;
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m
}

/// A Hex8 cantilever 6 x 1 x 1 clamped at `xmin`, with a spring in `y` at `xmax` (the transformation path).
fn hex8_spring() -> SolidModel {
    let mesh = hex_box([0.0; 3], [1.2, 0.2, 0.2], [6, 1, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m.add_support(ElasticSupport::nodal_spring("xmax", 1, 3.0e6)).unwrap();
    m
}

/// The radial Hex20 beam of F-396 (`si_rotating.rs`): `L = 1`, `h = 0.05`, 20 x 2 x 2, clamped at `x = 0`, `nu = 0`.
fn radial_beam() -> SolidModel {
    let base = hex_box([0.0, -0.025, -0.025], [1.0, 0.05, 0.05], [20, 2, 2]).unwrap();
    let mesh = upgrade_to_hex20(&base).unwrap().0;
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(2.0e11, 0.0, 7850.0, 0.0).unwrap())
        .unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m
}

/// A 3 x 2 plate of Shell4 (or Shell3, each cell cut in two) in general position, root clamped, gravity and the centrifugal
/// load.
fn shell_plate(tri: bool) -> SolidModel {
    let (nx, ny) = (3usize, 2usize);
    let idx = |i: usize, j: usize| j * (nx + 1) + i;
    let mut nodes = Vec::new();
    for j in 0..=ny {
        for i in 0..=nx {
            let (x, y) = (0.2 * i as f64, 0.15 * j as f64);
            nodes.push([
                0.1 + 0.8 * x + 0.1 * y,
                0.3 + 0.2 * x + 0.9 * y,
                0.05 + 0.3 * x - 0.2 * y,
            ]);
        }
    }
    let mut conn = Vec::new();
    let mut ne = 0;
    for j in 0..ny {
        for i in 0..nx {
            let (a, b, c, d) = (idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1));
            if tri {
                conn.extend_from_slice(&[a, b, c, a, c, d]);
                ne += 2;
            } else {
                conn.extend_from_slice(&[a, b, c, d]);
                ne += 1;
            }
        }
    }
    let n = nodes.len();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    let kind = if tri { ElementKind::Shell3 } else { ElementKind::Shell4 };
    mesh.add_block(kind, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::shell(0.01).unwrap()).unwrap();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    mesh.add_node_set("all", (0..n).collect()).unwrap();
    mesh.add_node_set("root", (0..=ny).map(|j| idx(0, j)).collect())
        .unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    for comp in 0..6 {
        m.add_constraint(Constraint::displacement("root", comp, 0.0)).unwrap();
    }
    m.add_load(Load::gravity([0.5, -1.5, -9.81])).unwrap();
    m.add_load(Load::centrifugal([0.0, 0.3, 1.0], [0.4, 0.2, 0.0], 120.0))
        .unwrap();
    m
}

/// A cantilever of four Beam2 elements with gravity.
fn beam_cantilever() -> SolidModel {
    let nodes: Vec<[f64; 3]> = (0..5).map(|i| [0.25 * i as f64, 0.1 * i as f64, 0.0]).collect();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Beam2, vec![0, 1, 1, 2, 2, 3, 3, 4])
        .unwrap();
    let section = BeamSection::new(0.012, 3.0e-6, 5.0e-6, 4.0e-6, 0.85, 0.75).unwrap();
    mesh.set_block_property(0, BlockProperty::beam(section, [0.0, 0.0, 1.0]).unwrap())
        .unwrap();
    mesh.add_element_set("all", (0..4).collect()).unwrap();
    mesh.add_node_set("root", vec![0]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    for comp in 0..6 {
        m.add_constraint(Constraint::displacement("root", comp, 0.0)).unwrap();
    }
    m.add_load(Load::gravity([0.5, -1.5, -9.81])).unwrap();
    m
}

fn all_digests() -> Vec<(String, u64)> {
    let mut out = Vec::new();
    // Buckling and pre-stressed modes of the Hex20 cantilever, a static pre-stress (elimination path).
    let c = hex20_cantilever();
    let mut cs = c.clone();
    cs.add_load(Load::nodal_force("xmax", [-2.0e4, 0.0, 0.0])).unwrap();
    let res = solve_static(&cs, &StaticOptions::default()).unwrap();
    let pre = Prestress::from_static(&cs, &res).unwrap();
    out.push((
        "hex20 static prestress".into(),
        digest(pre.stress().iter().flatten().copied()),
    ));
    let b = factors(&cs, &pre, &BucklingOptions::new(2)).unwrap();
    out.push(("hex20 buckling factors".into(), digest(b.factors.iter().copied())));
    out.push(("hex20 buckling modes".into(), digest(b.modes.iter().flatten().copied())));
    let kg = geometric_stiffness(&cs, &pre, Some(1)).unwrap();
    out.push(("hex20 geometric stiffness".into(), digest(kg.val.iter().copied())));
    let pm = prestressed_modes(&cs, &pre.scaled(-0.5), &opts(3)).unwrap();
    modal_digests("hex20 prestressed modes", &pm, &mut out);
    // A uniform pre-stress.
    let uni = Prestress::uniform(&c, [-1.0e6, 2.0e5, 0.0, 3.0e4, 0.0, -1.0e4]).unwrap();
    out.push((
        "hex20 uniform prestress".into(),
        digest(uni.stress().iter().flatten().copied()),
    ));
    let pu = prestressed_modes(&c, &uni, &opts(3)).unwrap();
    modal_digests("hex20 uniform prestressed modes", &pu, &mut out);
    // The transformation path.
    let t = hex8_spring();
    let ut = Prestress::uniform(&t, [-2.0e6, 0.0, 0.0, 0.0, 0.0, 0.0]).unwrap();
    let bt = factors(&t, &ut, &BucklingOptions::new(2)).unwrap();
    out.push((
        "hex8 spring buckling factors".into(),
        digest(bt.factors.iter().copied()),
    ));
    let pt = prestressed_modes(&t, &ut.scaled(0.1), &opts(3)).unwrap();
    modal_digests("hex8 spring prestressed modes", &pt, &mut out);
    // The rotating part of F-396.
    let r = radial_beam();
    let part = RotatingPart::new(&r, AXIS, ORIGIN, &RotatingOptions::new(4).modal(opts(4))).unwrap();
    out.push((
        "radial unit prestress".into(),
        digest(part.unit_prestress().stress().iter().flatten().copied()),
    ));
    out.push((
        "radial unit geometric stiffness".into(),
        digest(part.unit_geometric_stiffness().val.iter().copied()),
    ));
    out.push((
        "radial spin softening".into(),
        digest(part.spin_softening().val.iter().copied()),
    ));
    let mp = spin_softening_mass(&r, [0.3, -0.2, 1.0], Some(1)).unwrap();
    out.push(("radial spin softening tilted".into(), digest(mp.val.iter().copied())));
    for w in [0.0, 150.0] {
        match part.modes(w).unwrap() {
            SpeedModes::Modes(m) => modal_digests(&format!("radial modes {w}"), &m, &mut out),
            SpeedModes::LossOfStiffness(l) => panic!("unexpected loss {l:?}"),
        }
    }
    let table = part.campbell(&[0.0, 100.0, 200.0]).unwrap();
    out.push((
        "radial campbell".into(),
        digest(
            table
                .tracks
                .iter()
                .flat_map(|t| t.eigenvalues.iter().map(|v| v.unwrap_or(-1.0))),
        ),
    ));
    // The loads hook on shells and a beam.
    for tri in [false, true] {
        let p = shell_plate(tri);
        let tag = if tri { "shell3" } else { "shell4" };
        out.push((format!("{tag} loads"), digest(nodal_load_vector(&p, &[]).unwrap())));
        let s = solve_static(&p, &StaticOptions::default()).unwrap();
        out.push((
            format!("{tag} solution"),
            digest(s.displacement_values().iter().copied()),
        ));
    }
    let bm = beam_cantilever();
    out.push(("beam loads".into(), digest(nodal_load_vector(&bm, &[]).unwrap())));
    let s = solve_static(&bm, &StaticOptions::default()).unwrap();
    out.push(("beam solution".into(), digest(s.displacement_values().iter().copied())));
    out
}

/// The digests printed on the base 4abff5fe (ADR 0473 D9).
const BASE: &[(&str, u64)] = &[
    ("hex20 static prestress", 0x7fbe709513a8d1ca),
    ("hex20 buckling factors", 0xa8ce1fce4f9ceab6),
    ("hex20 buckling modes", 0x7563fd204c82bc66),
    ("hex20 geometric stiffness", 0xb947632bd030cb70),
    ("hex20 prestressed modes eigenvalues", 0x4d8da00df0ec419c),
    ("hex20 prestressed modes modes", 0x9f1876b10a1ca762),
    ("hex20 uniform prestress", 0x1e60e82ef70ccef7),
    ("hex20 uniform prestressed modes eigenvalues", 0x2707f82dbbc8d8f2),
    ("hex20 uniform prestressed modes modes", 0x2af0f6d4674ba77a),
    ("hex8 spring buckling factors", 0xd89defd033cdcab2),
    ("hex8 spring prestressed modes eigenvalues", 0xc9cadff66d376fe0),
    ("hex8 spring prestressed modes modes", 0xebd5635aa0089174),
    ("radial unit prestress", 0x33a5be0186b63e89),
    ("radial unit geometric stiffness", 0x8a2d58b4c9616338),
    ("radial spin softening", 0xd5f790ceb9a2310a),
    ("radial spin softening tilted", 0x945c8c0c2eeb3efc),
    ("radial modes 0 eigenvalues", 0xb1552d9fb2b59c32),
    ("radial modes 0 modes", 0xc9c3bd313259f294),
    ("radial modes 150 eigenvalues", 0x61cb3013708b5920),
    ("radial modes 150 modes", 0xd7e2ad3a94ceaaa2),
    ("radial campbell", 0x381f3481353892a4),
    ("shell4 loads", 0xaacc352c940672df),
    ("shell4 solution", 0x0f5cad0cd4320883),
    ("shell3 loads", 0x53c34e5f0624fdc8),
    ("shell3 solution", 0xd497818c661a95aa),
    ("beam loads", 0x1a9c4249079a6b61),
    ("beam solution", 0xa919a4484dab315e),
];

#[test]
fn b1b_default_paths_keep_their_bits() {
    let got = all_digests();
    for (name, d) in &got {
        println!("(\"{name}\", 0x{d:016x}),");
    }
    assert_eq!(got.len(), BASE.len(), "digest count");
    let mut missed = Vec::new();
    for ((name, d), (bname, bd)) in got.iter().zip(BASE) {
        assert_eq!(name, bname, "digest order");
        if d != bd {
            missed.push(name.clone());
        }
    }
    assert!(missed.is_empty(), "changed bits: {missed:?}");
}
