//! F-547 (ADR 0465): the bit record of the paths that F-547 touches (check B1 of the registration).
//!
//! This file uses only the API of main a52a5291: it was run once in the worktree at that commit (no library line changed)
//! before any library code of F-547, and the digests it printed are the constants of `BASE`. After the change it runs
//! unchanged and compares. The models cover the pre-stressed modes on the transformation: an isotropic sector with a cyclic
//! constraint (a centrifugal pre-stress from the sector's static solve, and a uniform pre-stress), the orthotropic and the
//! tabulated material with cylindrical axes about the cyclic axis (accepted before and after F-547), and an isotropic block
//! with a linear constraint and a spring (no cyclic constraint); and the elimination path of an isotropic block.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::hex_box;
use rcfd_fem::geometric::{prestressed_modes, Prestress};
use rcfd_fem::hex20::upgrade_to_hex20_mapped;
use rcfd_fem::material::MaterialAxes;
use rcfd_fem::modal::{ModalOptions, ModalResult};
use rcfd_fem::model::{ElasticSupport, LinearTerm};
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{Constraint, Load, Material, SolidMesh, SolidModel};
use rcfd_linalg::eigen::ResidualTest;
use std::f64::consts::PI;

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

fn ortho_c() -> Material {
    let axes = MaterialAxes::cylindrical(ORIGIN, AXIS).unwrap();
    Material::orthotropic(
        [1.4e11, 1.0e10, 1.0e10],
        [0.3, 0.3, 0.45],
        [5.0e9, 5.0e9, 3.5e9],
        1600.0,
        [0.0; 3],
        axes,
    )
    .unwrap()
}

fn tab_o() -> Material {
    Material::tabulated(
        ortho_c(),
        vec![200.0, 600.0],
        vec![
            ("e_1".to_string(), vec![1.4e11, 1.0e11]),
            ("g_12".to_string(), vec![5.0e9, 4.0e9]),
        ],
    )
    .unwrap()
}

fn opts(n: usize) -> ModalOptions {
    ModalOptions::new(n)
        .residual_test(ResidualTest::Backward)
        .tolerance(1e-12)
}

/// A Hex20 sector of the annulus `r in [0.25, 1]`, `z in [-0.1, 0.1]` of 12 sectors, 3 x 2 x 1 in `(r, phi, z)`, `xmin`
/// clamped, one cyclic constraint about z through the origin; with `field` the nodal temperature `293 + 250 (r - 0.25)/0.75
/// + 200 z` (reference 293 K).
fn sector(mat: Material, field: bool) -> SolidModel {
    let alpha = 2.0 * PI / 12.0;
    let b = hex_box([0.25, 0.0, -0.1], [0.75, alpha, 0.2], [3, 2, 1]).unwrap();
    let mesh: SolidMesh = upgrade_to_hex20_mapped(&b, |p| [p[0] * p[1].cos(), p[0] * p[1].sin(), p[2]])
        .unwrap()
        .0;
    let t: Vec<f64> = mesh
        .nodes()
        .iter()
        .map(|x| 293.0 + 250.0 * ((x[0] * x[0] + x[1] * x[1]).sqrt() - 0.25) / 0.75 + 200.0 * x[2])
        .collect();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", mat).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m.add_constraint(Constraint::cyclic("ymin", "ymax", AXIS, ORIGIN, 12))
        .unwrap();
    if field {
        m.add_load(Load::temperature(t, 293.0)).unwrap();
    }
    m
}

/// The pre-stress of the static solve of `m` with the centrifugal load at `omega` about the cyclic axis added.
fn centrifugal(m: &SolidModel, omega: f64) -> Prestress {
    let mut c = m.clone();
    c.add_load(Load::centrifugal(AXIS, ORIGIN, omega)).unwrap();
    let r = solve_static(&c, &StaticOptions::default()).unwrap();
    Prestress::from_static(&c, &r).unwrap()
}

fn modal_digests(tag: &str, r: &ModalResult, out: &mut Vec<(String, u64)>) {
    out.push((format!("{tag} eigenvalues"), digest(r.eigenvalues.iter().copied())));
    out.push((format!("{tag} modes"), digest(r.modes.iter().flatten().copied())));
}

fn all_digests() -> Vec<(String, u64)> {
    let mut out = Vec::new();
    // The isotropic sector: the centrifugal pre-stress and its modes; a uniform pre-stress.
    let s = sector(steel(), false);
    let p = centrifugal(&s, 300.0);
    out.push((
        "iso sector centrifugal prestress".into(),
        digest(p.stress().iter().flatten().copied()),
    ));
    modal_digests(
        "iso sector prestressed centrifugal",
        &prestressed_modes(&s, &p, &opts(3)).unwrap(),
        &mut out,
    );
    let u = Prestress::uniform(&s, [1.0e8, 1.0e8, 0.0, 0.0, 0.0, 0.0]).unwrap();
    modal_digests(
        "iso sector prestressed uniform",
        &prestressed_modes(&s, &u, &opts(3)).unwrap(),
        &mut out,
    );
    // The isotropic sector with a temperature field (its thermal stress in the pre-stress).
    let st = sector(steel(), true);
    let pt = centrifugal(&st, 300.0);
    modal_digests(
        "iso sector with field prestressed centrifugal",
        &prestressed_modes(&st, &pt, &opts(3)).unwrap(),
        &mut out,
    );
    // The orthotropic and the tabulated material with cylindrical axes about the cyclic axis.
    let so = sector(ortho_c(), false);
    let po = centrifugal(&so, 300.0);
    modal_digests(
        "ortho_c sector prestressed centrifugal",
        &prestressed_modes(&so, &po, &opts(3)).unwrap(),
        &mut out,
    );
    let stab = sector(tab_o(), true);
    let ptab = centrifugal(&stab, 300.0);
    modal_digests(
        "tab_o sector prestressed centrifugal",
        &prestressed_modes(&stab, &ptab, &opts(3)).unwrap(),
        &mut out,
    );
    // The transformation without a cyclic constraint: a block with a linear constraint and a spring.
    let mut mesh = hex_box([0.0; 3], [1.0, 0.4, 0.25], [5, 2, 1]).unwrap();
    let root: Vec<usize> = mesh.node_set("xmin").unwrap().to_vec();
    let tip: Vec<usize> = mesh.node_set("xmax").unwrap().to_vec();
    mesh.add_node_set("tip", tip).unwrap();
    let mut b = SolidModel::new(mesh.clone());
    b.add_material("all", steel()).unwrap();
    b.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    b.add_constraint(Constraint::displacement("xmin", 1, 0.0)).unwrap();
    for &n in &root {
        b.add_constraint(Constraint::linear(vec![LinearTerm::new(n, 2, 1.0)], 0.0))
            .unwrap();
    }
    b.add_support(ElasticSupport::nodal_spring("tip", 2, 5.0e7)).unwrap();
    let pb = Prestress::uniform(&b, [2.0e8, 0.0, 0.0, 0.0, 0.0, 0.0]).unwrap();
    modal_digests(
        "iso block transformation prestressed",
        &prestressed_modes(&b, &pb, &opts(3)).unwrap(),
        &mut out,
    );
    // The elimination path: the same block clamped at xmin.
    let mut e = SolidModel::new(mesh);
    e.add_material("all", steel()).unwrap();
    for c in 0..3 {
        e.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    let pe = Prestress::uniform(&e, [2.0e8, 0.0, 0.0, 0.0, 0.0, 0.0]).unwrap();
    modal_digests(
        "iso block elimination prestressed",
        &prestressed_modes(&e, &pe, &opts(3)).unwrap(),
        &mut out,
    );
    out
}

/// The digests printed by the run at main a52a5291 (filled in from that run's output).
const BASE: Option<&[(&str, u64)]> = Some(&[
    ("iso sector centrifugal prestress", 0x27082fdb8cf07b53),
    ("iso sector prestressed centrifugal eigenvalues", 0x07a7f308496241f3),
    ("iso sector prestressed centrifugal modes", 0xfb0aac294902af18),
    ("iso sector prestressed uniform eigenvalues", 0xee520e967abc7c47),
    ("iso sector prestressed uniform modes", 0x29c52e08d865a099),
    (
        "iso sector with field prestressed centrifugal eigenvalues",
        0x8ee9a388e67c1271,
    ),
    (
        "iso sector with field prestressed centrifugal modes",
        0xd836afe091a83642,
    ),
    ("ortho_c sector prestressed centrifugal eigenvalues", 0x3a00fa6e5abcdedc),
    ("ortho_c sector prestressed centrifugal modes", 0xa2b384490a671787),
    ("tab_o sector prestressed centrifugal eigenvalues", 0xbdbfd78a80e0b3a3),
    ("tab_o sector prestressed centrifugal modes", 0xad72a0a220044333),
    ("iso block transformation prestressed eigenvalues", 0x6865a8992c6f4fa4),
    ("iso block transformation prestressed modes", 0x9c5eeb287def8f53),
    ("iso block elimination prestressed eigenvalues", 0xd41ab6be725d476c),
    ("iso block elimination prestressed modes", 0x51af532429fd3b72),
]);

#[test]
fn b1_default_paths_bits() {
    let got = all_digests();
    for (name, d) in &got {
        println!("(\"{name}\", 0x{d:016x}),");
    }
    let base = BASE.expect("the record of main a52a5291 is filled in");
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
