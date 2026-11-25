//! F-672 (ADR 0799) B-M and B-D: the record of main. Three texts, each hashed by 64-bit FNV-1a over its UTF-8 bytes: `H_S`
//! the ADR 0556 Shell4 frustum sector (no beams) under `shells(true)`, the whirl paths P1 to P8 then the full rotor and its
//! expansions; `H_W` the wheel sector W of ADR 0595 under `shells(true).beams(true)`, P1 to P8; `H_F` W under
//! `shells(true).beams(true).full_rotor_beams(true)`, the full rotor and its expansions. The records were made by running
//! this file on the library of main (ADR 0799 commit 2: this file alone on top of the registration), twice on each
//! platform; a platform without a record prints and does not compare (ADR 0792 amendment 1). The builders are those of
//! `sa_rotating_cyclic_beams_f675.rs` and `sa_cyclic_beams_full_rotor_f674.rs`, copied.

use rcfd_fem::cyclic::{full_rotor, modal_analysis_harmonic};
use rcfd_fem::modal::ModalOptions;
use rcfd_fem::model::CyclicOptions;
use rcfd_fem::rotating::{RotatingOptions, RotatingSector, SparseStateOptions};
use rcfd_fem::{BeamSection, BlockProperty, Constraint, ElementKind, Material, SolidMesh, SolidModel};
use rcfd_linalg::eigen::ResidualTest;
use std::f64::consts::PI;

type Vec3 = [f64; 3];

const E: f64 = 2.0e11;
const NU: f64 = 0.3;
const RHO: f64 = 7850.0;
const N: usize = 6;
const ORIGIN: Vec3 = [0.3, -0.2, 0.5];
/// Ring elements per sector.
const NR: usize = 4;
/// The loaded sector nodes of ADR 0595 (their node sets are kept, the builders being copies).
const LOADED: [usize; 5] = [0, 1, 2, 3, 5];
/// The frustum of ADR 0556: columns and rows of the sector.
const NC: usize = 2;
const NA: usize = 3;

/// The record of Windows x86_64: `[H_S, H_W, H_F]`.
const RECORD_WINDOWS: Option<[u64; 3]> = Some([0xd135e8503d05e054, 0xe314b82213528f82, 0x21c219cfc1e34528]);
/// The record of Linux x86_64: `[H_S, H_W, H_F]`.
const RECORD_LINUX: Option<[u64; 3]> = Some([0xd853128f3fd29fe5, 0xc2c826f49eec0288, 0x027865e08223bcf8]);

fn axis() -> Vec3 {
    [1.0 / 3.0, 2.0 / 3.0, 2.0 / 3.0]
}

fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn ea() -> Vec3 {
    let l = 5.0f64.sqrt();
    [2.0 / l, -1.0 / l, 0.0]
}

fn eb() -> Vec3 {
    cross(axis(), ea())
}

fn comb(a: f64, x: Vec3, b: f64, y: Vec3) -> Vec3 {
    [0, 1, 2].map(|k| a * x[k] + b * y[k])
}

/// `P(phi, r, z) = O + r (cos phi e_a + sin phi e_b) + z n`.
fn p(phi: f64, r: f64, z: f64) -> Vec3 {
    let (n, a, b) = (axis(), ea(), eb());
    let (s, c) = phi.sin_cos();
    [0, 1, 2].map(|k| ORIGIN[k] + r * (c * a[k] + s * b[k]) + z * n[k])
}

fn alpha() -> f64 {
    2.0 * PI / N as f64
}

fn mat() -> Material {
    Material::isotropic_elastic(E, NU, RHO, 0.0).unwrap()
}

fn ring_section() -> BeamSection {
    BeamSection::new(1.8e-3, 1.35e-7, 5.4e-7, 3.7e-7, 5.0 / 6.0, 5.0 / 6.0).unwrap()
}

fn spoke_section() -> BeamSection {
    BeamSection::new(1.2e-3, 1.0e-7, 2.5e-7, 2.4e-7, 0.85, 0.75).unwrap()
}

fn v_ring() -> Vec3 {
    comb(0.8, axis(), 0.6, ea())
}

fn v_spoke() -> Vec3 {
    comb(0.6, eb(), 0.8, axis())
}

fn modal_options(modes: usize) -> ModalOptions {
    let mut o = ModalOptions::new(modes);
    o.tolerance = 1e-12;
    o.residual_test = ResidualTest::Backward;
    o
}

/// Option set (a) of ADR 0799: the shells option alone.
fn options_a() -> CyclicOptions {
    CyclicOptions::new().shells(true)
}

/// Option set (b): with the beams option.
fn options_b() -> CyclicOptions {
    options_a().beams(true)
}

/// Option set (c): with the full-rotor option as well.
fn options_c() -> CyclicOptions {
    options_b().full_rotor_beams(true)
}

fn clamp(m: &mut SolidModel) {
    for comp in 0..6 {
        m.add_constraint(Constraint::displacement("hub", comp, 0.0)).unwrap();
    }
}

fn wheel_mesh() -> SolidMesh {
    let a = alpha();
    let mut nodes: Vec<Vec3> = (0..=NR).map(|i| p(i as f64 * a / NR as f64, 1.0, 0.0)).collect();
    nodes.push(p(a / 2.0, 0.65, 0.125));
    nodes.push(p(a / 2.0, 0.3, 0.25));
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Beam2, vec![0, 1, 1, 2, 2, 3, 3, 4])
        .unwrap();
    mesh.add_block(ElementKind::Beam2, vec![6, 5, 5, 2]).unwrap();
    mesh.set_block_property(0, BlockProperty::beam(ring_section(), v_ring()).unwrap())
        .unwrap();
    mesh.set_block_property(1, BlockProperty::beam(spoke_section(), v_spoke()).unwrap())
        .unwrap();
    mesh.add_element_set("all", (0..6).collect()).unwrap();
    mesh.add_node_set("low", vec![0]).unwrap();
    mesh.add_node_set("high", vec![NR]).unwrap();
    mesh.add_node_set("hub", vec![6]).unwrap();
    for q in LOADED {
        mesh.add_node_set(&format!("p{q}"), vec![q]).unwrap();
    }
    mesh
}

/// The wheel sector W of ADR 0595 with the cyclic constraint of `options`.
fn sector(options: CyclicOptions) -> SolidModel {
    let mut m = SolidModel::new(wheel_mesh());
    m.add_material("all", mat()).unwrap();
    clamp(&mut m);
    m.add_constraint(Constraint::cyclic_with("low", "high", axis(), ORIGIN, N, options))
        .unwrap();
    m
}

/// The ADR 0556 D3 frustum point at column `i` and generator parameter `s`.
fn frustum_point(i: usize, s: f64) -> Vec3 {
    let phi = 2.0 * PI * i as f64 / (N * NC) as f64;
    p(phi, 1.0 - 0.4 * s, 0.8 * s)
}

/// The Shell4 frustum sector of ADR 0556 (no beams) with the cyclic constraint of `options`.
fn shell_sector(options: CyclicOptions) -> SolidModel {
    let id = |i: usize, a: usize| a * (NC + 1) + i;
    let mut nodes = Vec::new();
    for a in 0..=NA {
        for i in 0..=NC {
            nodes.push(frustum_point(i, a as f64 / NA as f64));
        }
    }
    let mut conn = Vec::new();
    for a in 0..NA {
        for i in 0..NC {
            conn.extend([id(i, a), id(i + 1, a), id(i + 1, a + 1), id(i, a + 1)]);
        }
    }
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Shell4, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::shell(0.05).unwrap()).unwrap();
    mesh.add_element_set("all", (0..NA * NC).collect()).unwrap();
    mesh.add_node_set("hub", (0..=NC).map(|i| id(i, 0)).collect()).unwrap();
    mesh.add_node_set("low", (0..=NA).map(|a| id(0, a)).collect()).unwrap();
    mesh.add_node_set("high", (0..=NA).map(|a| id(NC, a)).collect())
        .unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", mat()).unwrap();
    clamp(&mut m);
    m.add_constraint(Constraint::cyclic_with("low", "high", axis(), ORIGIN, N, options))
        .unwrap();
    m
}

/// The full-rotor model of ADR 0601: the mesh of a rotor with a node set `r<p>` on every node, the material and the clamp.
fn rotor_model(mesh: &SolidMesh) -> SolidModel {
    let mut mesh = mesh.clone();
    for p in 0..mesh.nodes().len() {
        mesh.add_node_set(&format!("r{p}"), vec![p]).unwrap();
    }
    let mut m = SolidModel::new(mesh);
    m.add_material("all", mat()).unwrap();
    clamp(&mut m);
    m
}

/// The whirl paths P1 to P8 of ADR 0799 on a sector model: (name, `Debug` text of the result).
fn whirl_texts(m: &SolidModel) -> Vec<(String, String)> {
    let o = RotatingOptions::new(4).modal(modal_options(4)).coriolis(true);
    let sec = match RotatingSector::new(m, &o) {
        Ok(s) => s,
        Err(e) => return vec![("RotatingSector::new".to_string(), format!("Err({e:?})"))],
    };
    let ks = [0usize, 1, 2, 3];
    let speeds = [0.0, 30.0, 60.0];
    let mut out = vec![("P1 coriolis".to_string(), format!("{:?}", sec.part().coriolis()))];
    for k in ks {
        out.push((
            format!("P2 whirl_modes k = {k}"),
            format!("{:?}", sec.whirl_modes(k, 30.0)),
        ));
    }
    out.push((
        "P3 whirl_campbell".to_string(),
        format!("{:?}", sec.whirl_campbell(&ks, &speeds)),
    ));
    for k in ks {
        out.push((
            format!("P4 split_modes k = {k}"),
            format!("{:?}", sec.split_modes(k, 30.0)),
        ));
    }
    out.push((
        "P5 split_campbell".to_string(),
        format!("{:?}", sec.split_campbell(&ks, &speeds)),
    ));
    for k in ks {
        out.push((
            format!("P6 whirl_state k = {k}"),
            format!("{:?}", sec.whirl_state(k, 30.0)),
        ));
    }
    out.push((
        "P7 whirl_stability".to_string(),
        format!("{:?}", sec.whirl_stability(&ks, &speeds)),
    ));
    let sparse = sec.whirl_state_sparse(1, 30.0, &SparseStateOptions::new(2.0e4, 4));
    out.push(("P8 whirl_state_sparse k = 1".to_string(), format!("{sparse:?}")));
    out
}

/// The full rotor of a sector model and its expansions at `k = 1`: (name, `Debug` text).
fn rotor_texts(m: &SolidModel) -> Vec<(String, String)> {
    let rotor = match full_rotor(m) {
        Ok(r) => r,
        Err(e) => return vec![("full_rotor".to_string(), format!("Err({e:?})"))],
    };
    let mut out = vec![("full_rotor".to_string(), format!("Ok({rotor:?})"))];
    let fmodel = rotor_model(rotor.mesh());
    let (sd, fd) = (m.dof_map(), fmodel.dof_map());
    let text = match modal_analysis_harmonic(m, &[], 1, &modal_options(4)) {
        Ok(h) => format!("Ok({:?})", rotor.expand(&sd, &fd, 1, &h.modes[0])),
        Err(e) => format!("Err({e:?})"),
    };
    out.push(("expand k = 1, first mode".to_string(), text));
    let n = sd.n_equations();
    let fa: Vec<f64> = (0..n).map(|e| (0.37 * e as f64 + 0.1).sin()).collect();
    let fb: Vec<f64> = (0..n).map(|e| (0.53 * e as f64 + 0.4).cos()).collect();
    out.push((
        "expand_load k = 1".to_string(),
        format!("{:?}", rotor.expand_load(&sd, &fd, 1, &fa, &fb)),
    ));
    out.push((
        "expand_field k = 1".to_string(),
        format!("{:?}", rotor.expand_field(&sd, &fd, 1, &fa, &fb)),
    ));
    out
}

fn fnv(texts: &[(String, String)]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for (name, text) in texts {
        for b in name.bytes().chain([b'\n']).chain(text.bytes()).chain([b'\n']) {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x100000001b3);
        }
    }
    h
}

fn record() -> Option<[u64; 3]> {
    if cfg!(all(windows, target_arch = "x86_64")) {
        RECORD_WINDOWS
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        RECORD_LINUX
    } else {
        None
    }
}

fn show(name: &str, texts: &[(String, String)]) {
    for (path, text) in texts {
        let head: String = text.chars().take(24).collect();
        println!("  {name} {path}: {} characters, {head}...", text.len());
    }
}

#[test]
fn b_m_the_record_of_main() {
    let mut s = whirl_texts(&shell_sector(options_a()));
    s.extend(rotor_texts(&shell_sector(options_a())));
    let w = whirl_texts(&sector(options_b()));
    let f = rotor_texts(&sector(options_c()));
    show("H_S", &s);
    show("H_W", &w);
    show("H_F", &f);
    let got = [fnv(&s), fnv(&w), fnv(&f)];
    println!(
        "B-M hashes [H_S, H_W, H_F] = [{:#018x}, {:#018x}, {:#018x}] on {} {}",
        got[0],
        got[1],
        got[2],
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    match record() {
        Some(r) => {
            println!(
                "B-M record [{:#018x}, {:#018x}, {:#018x}]: equal {}",
                r[0],
                r[1],
                r[2],
                got == r
            );
            assert_eq!(got, r, "B-M");
        }
        None => println!("B-M: no record for this platform; printed, not compared"),
    }
}
