//! F-628 (ADR 0556): shells in the cyclic constraint. Checks S1a, S1b, S2a, S2b, E1, E2, E3, R1, B3, B4 and the negative
//! controls NC1 to NC6 of the registration (B1 and B2 are the existing tests and `baseline_hashes`).

use rcfd_fem::builders::{hex_box, BOX_FACES};
use rcfd_fem::cyclic::{modal_analysis_harmonic, rotation_matrix, solve_static_harmonic, HarmonicLoad, Mat3};
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::model::CyclicOptions;
use rcfd_fem::mpc::{modal_analysis_mpc, solve_static_mpc};
use rcfd_fem::shell;
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::tie::{TieMethod, TieOptions};
use rcfd_fem::{
    BeamSection, BlockProperty, Constraint, ElementKind, FaceRef, FemError, Load, Material, SetKind, SolidMesh,
    SolidModel,
};
use rcfd_linalg::eigen::ResidualTest;
use std::f64::consts::PI;

type Vec3 = [f64; 3];

const E: f64 = 2.0e11;
const NU: f64 = 0.3;
const RHO: f64 = 7850.0;
const N: usize = 6;
const NC: usize = 2;
const NA: usize = 3;
const T: f64 = 0.05;
const OMEGA: f64 = 100.0;
const ORIGIN: Vec3 = [0.3, -0.2, 0.5];

fn axis() -> Vec3 {
    [1.0 / 3.0, 2.0 / 3.0, 2.0 / 3.0]
}

fn mat() -> Material {
    Material::isotropic_elastic(E, NU, RHO, 0.0).unwrap()
}

fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// The point of the D3 frustum at column `i` (of `N NC` around) and generator parameter `s`, radial offset `dr`.
fn point(i: usize, s: f64, dr: f64) -> Vec3 {
    let n = axis();
    let l = 5.0f64.sqrt();
    let ea = [2.0 / l, -1.0 / l, 0.0];
    let eb = cross(n, ea);
    let phi = 2.0 * PI * i as f64 / (N * NC) as f64;
    let r = 1.0 - 0.4 * s + dr;
    let z = 0.8 * s;
    let (sp, cp) = phi.sin_cos();
    [0, 1, 2].map(|k| ORIGIN[k] + r * (cp * ea[k] + sp * eb[k]) + z * n[k])
}

fn modal_options(modes: usize) -> ModalOptions {
    let mut o = ModalOptions::new(modes);
    o.tolerance = 1e-12;
    o.residual_test = ResidualTest::Backward;
    o
}

/// FNV-1a 64 over the bits of the values.
fn fnv(values: impl IntoIterator<Item = f64>) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for v in values {
        for b in v.to_bits().to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x100000001b3);
        }
    }
    h
}

// ---------------------------------------------------------------------------------------------- B3, B4: the solid twin

/// The Hex8 sector of the D3 frustum (2 x 3 x 1, radial thickness `2 T`), node `(i, a, w)` at `w (NA + 1) (NC + 1) + a (NC +
/// 1) + i`; surface sets `low` / `high` (the cut faces), node sets `low_nodes` / `high_nodes` of the same nodes, `clamp` (a =
/// 0), element set `all`. Centrifugal load about the axis.
fn solid_sector() -> SolidMesh {
    let id = |i: usize, a: usize, w: usize| w * (NA + 1) * (NC + 1) + a * (NC + 1) + i;
    let mut nodes = Vec::new();
    for w in 0..2 {
        for a in 0..=NA {
            for i in 0..=NC {
                nodes.push(point(i, a as f64 / NA as f64, (w as f64 - 0.5) * 2.0 * T));
            }
        }
    }
    let mut conn = Vec::new();
    for a in 0..NA {
        for i in 0..NC {
            for w in 0..2 {
                conn.extend([id(i, a, w), id(i + 1, a, w), id(i + 1, a + 1, w), id(i, a + 1, w)]);
            }
        }
    }
    // Reorder: each element's eight nodes are the w = 0 face then the w = 1 face.
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Hex8, conn).unwrap();
    let ne = NA * NC;
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    let col = |i: usize| -> Vec<usize> {
        let mut v: Vec<usize> = (0..2).flat_map(|w| (0..=NA).map(move |a| id(i, a, w))).collect();
        v.sort_unstable();
        v
    };
    for (name, i) in [("low", 0), ("high", NC)] {
        let nodes = col(i);
        let mut faces = Vec::new();
        for e in 0..ne {
            for f in 0..6 {
                let fr = FaceRef { element: e, face: f };
                if mesh.face_nodes(fr).iter().all(|p| nodes.binary_search(p).is_ok()) {
                    faces.push(fr);
                }
            }
        }
        mesh.add_surface_set(name, faces).unwrap();
        mesh.add_node_set(&format!("{name}_nodes"), nodes).unwrap();
    }
    let mut clamp: Vec<usize> = (0..2).flat_map(|w| (0..=NC).map(move |i| id(i, 0, w))).collect();
    clamp.sort_unstable();
    mesh.add_node_set("clamp", clamp).unwrap();
    mesh
}

fn solid_model(constraint: Constraint) -> SolidModel {
    let mut m = SolidModel::new(solid_sector());
    m.add_material("all", mat()).unwrap();
    for comp in 0..3 {
        m.add_constraint(Constraint::displacement("clamp", comp, 0.0)).unwrap();
    }
    m.add_constraint(constraint).unwrap();
    m.add_load(Load::centrifugal(axis(), ORIGIN, OMEGA)).unwrap();
    m
}

/// The displacements of `solve_static_mpc` and the frequencies of `modal_analysis_harmonic` k = 0 .. 3 (the loads removed
/// for the modes: a modal analysis ignores them).
fn solid_results(m: &SolidModel) -> (Vec<f64>, Vec<Vec<f64>>) {
    let s = solve_static_mpc(m, &[], &[], &[], &StaticOptions::default()).unwrap();
    let u = s.result.displacement_values().to_vec();
    let f = (0..=N / 2)
        .map(|k| {
            modal_analysis_harmonic(m, &[], k, &modal_options(3))
                .unwrap()
                .frequencies
        })
        .collect();
    (u, f)
}

fn solid_default() -> SolidModel {
    solid_model(Constraint::cyclic("low", "high", axis(), ORIGIN, N))
}

/// B4: the hash of the default-options results, recorded on the unchanged library at d4ae697d.
const B4_HASH: u64 = 0xb49f8bf2c903ab48;

#[test]
fn b4_solid_twin_default_bits_recorded() {
    let (u, f) = solid_results(&solid_default());
    let h = fnv(u.iter().copied().chain(f.iter().flatten().copied()));
    println!("B4 hash {h:#018x}, |u| = {}, f = {f:?}", u.len());
    assert_eq!(h, B4_HASH, "B4: {h:#018x}");
}

#[test]
fn b3_solid_twin_default_against_the_shells_option() {
    let (u0, f0) = solid_results(&solid_default());
    let opt = CyclicOptions::new().shells(true);
    let (u1, f1) = solid_results(&solid_model(Constraint::cyclic_with(
        "low_nodes",
        "high_nodes",
        axis(),
        ORIGIN,
        N,
        opt,
    )));
    let same_u = u0.len() == u1.len() && u0.iter().zip(&u1).all(|(a, b)| a.to_bits() == b.to_bits());
    let same_f = f0
        .iter()
        .flatten()
        .zip(f1.iter().flatten())
        .all(|(a, b)| a.to_bits() == b.to_bits());
    println!("B3 displacements equal bits: {same_u}; frequencies equal bits: {same_f}");
    assert!(same_u && same_f, "B3");
}

// ---------------------------------------------------------------------------------------------- the shell sector and ring

/// Nodes of a quad, cut into two triangles along the diagonal from its first node for Shell3.
fn push_element(conn: &mut Vec<usize>, kind: ElementKind, q: [usize; 4]) {
    match kind {
        ElementKind::Shell4 => conn.extend(q),
        _ => conn.extend([q[0], q[1], q[2], q[0], q[2], q[3]]),
    }
}

/// A shell mesh of `columns` columns of nodes (`periodic`: the last column's elements close on column 0) and `NA + 1` rows,
/// node `(i, a)` at `a columns + i`; `shift(i, a)` moves a node. Element set `all`, node set `clamp` (a = 0).
fn shell_mesh(kind: ElementKind, columns: usize, periodic: bool, shift: &dyn Fn(usize, usize) -> Vec3) -> SolidMesh {
    let id = |i: usize, a: usize| a * columns + i % columns;
    let mut nodes = Vec::new();
    for a in 0..=NA {
        for i in 0..columns {
            let p = point(i, a as f64 / NA as f64, 0.0);
            let d = shift(i, a);
            nodes.push([p[0] + d[0], p[1] + d[1], p[2] + d[2]]);
        }
    }
    let quads = if periodic { columns } else { columns - 1 };
    let mut conn = Vec::new();
    for a in 0..NA {
        for i in 0..quads {
            push_element(
                &mut conn,
                kind,
                [id(i, a), id(i + 1, a), id(i + 1, a + 1), id(i, a + 1)],
            );
        }
    }
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(kind, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::shell(T).unwrap()).unwrap();
    let ne = mesh.blocks()[0].len();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    mesh.add_node_set("clamp", (0..columns).map(|i| id(i, 0)).collect())
        .unwrap();
    mesh
}

fn no_shift(_: usize, _: usize) -> Vec3 {
    [0.0; 3]
}

/// The sector node of column `i <= NC`, row `a`.
fn sector_node(i: usize, a: usize) -> usize {
    a * (NC + 1) + i
}

/// The ring node of column `i` of sector `j`, row `a`.
fn ring_node(j: usize, i: usize, a: usize) -> usize {
    a * (N * NC) + (j * NC + i) % (N * NC)
}

/// The loaded sector nodes (not high, not clamped): `(i, a)`.
fn loaded() -> Vec<(usize, usize)> {
    (1..=NA).flat_map(|a| (0..NC).map(move |i| (i, a))).collect()
}

/// The force and moment of sector node `q` (D4).
fn nodal_load(q: f64) -> (Vec3, Vec3) {
    (
        [
            1e3 * (1.3 * q + 0.2).sin(),
            1e3 * (0.7 * q + 0.5).cos(),
            1e3 * (2.1 * q + 1.1).sin(),
        ],
        [
            1e2 * (1.9 * q + 0.3).cos(),
            1e2 * (0.4 * q + 0.9).sin(),
            1e2 * (1.1 * q + 1.7).cos(),
        ],
    )
}

fn clamp(m: &mut SolidModel) {
    for comp in 0..6 {
        m.add_constraint(Constraint::displacement("clamp", comp, 0.0)).unwrap();
    }
}

fn shell_options() -> CyclicOptions {
    CyclicOptions::new().shells(true)
}

/// The sector model: the D3 sector with node sets `low` and `high`, the clamp and the cyclic constraint with `options`.
fn sector(kind: ElementKind, options: CyclicOptions, shift: &dyn Fn(usize, usize) -> Vec3) -> SolidModel {
    let mut mesh = shell_mesh(kind, NC + 1, false, shift);
    mesh.add_node_set("low", (0..=NA).map(|a| sector_node(0, a)).collect())
        .unwrap();
    mesh.add_node_set("high", (0..=NA).map(|a| sector_node(NC, a)).collect())
        .unwrap();
    for (i, a) in loaded() {
        mesh.add_node_set(&format!("p{}", sector_node(i, a)), vec![sector_node(i, a)])
            .unwrap();
    }
    let mut m = SolidModel::new(mesh);
    m.add_material("all", mat()).unwrap();
    clamp(&mut m);
    m.add_constraint(Constraint::cyclic_with("low", "high", axis(), ORIGIN, N, options))
        .unwrap();
    m
}

/// The full ring, the clamp, with a node set `p<node>` on every node of rows a >= 1.
fn ring(kind: ElementKind) -> SolidModel {
    let mut mesh = shell_mesh(kind, N * NC, true, &no_shift);
    for p in N * NC..mesh.nodes().len() {
        mesh.add_node_set(&format!("p{p}"), vec![p]).unwrap();
    }
    let mut m = SolidModel::new(mesh);
    m.add_material("all", mat()).unwrap();
    clamp(&mut m);
    m
}

fn rot(j: usize) -> Mat3 {
    rotation_matrix(axis(), 2.0 * PI * j as f64 / N as f64)
}

fn apply(r: &Mat3, v: Vec3) -> Vec3 {
    [0, 1, 2].map(|i| r[i][0] * v[0] + r[i][1] * v[1] + r[i][2] * v[2])
}

/// `(cos(j theta), sin(j theta))`, `theta = 2 pi k / N`.
fn phase(k: usize, j: usize) -> (f64, f64) {
    let t = 2.0 * PI * ((j * k) % N) as f64 / N as f64;
    (t.cos(), t.sin())
}

/// `(e_u, e_theta)` of the sector field `(a, b)` of harmonic `k` mapped to every sector against the ring field (D4).
fn field_error(sector: &SolidModel, a: &[f64], b: &[f64], k: usize, ring: &SolidModel, ur: &[f64]) -> (f64, f64) {
    let (sd, rd) = (sector.dof_map(), ring.dof_map());
    let (mut du, mut dt, mut mu, mut mt) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for j in 0..N {
        let r = rot(j);
        let (c, s) = phase(k, j);
        for row in 0..=NA {
            for i in 0..=NC {
                let p = sector_node(i, row);
                let rn = ring_node(j, i, row);
                for base in [0, 3] {
                    let v: Vec3 = [0, 1, 2].map(|m| {
                        let e = sd.equation(p, base + m).unwrap();
                        c * a[e] - s * b[e]
                    });
                    let w = apply(&r, v);
                    for m in 0..3 {
                        let x = ur[rd.equation(rn, base + m).unwrap()];
                        let d = (w[m] - x).abs();
                        if base == 0 {
                            du = du.max(d);
                            mu = mu.max(x.abs());
                        } else {
                            dt = dt.max(d);
                            mt = mt.max(x.abs());
                        }
                    }
                }
            }
        }
    }
    (du / mu, dt / mt)
}

fn met(ok: bool) -> &'static str {
    if ok {
        "met"
    } else {
        "MISSED"
    }
}

// ---------------------------------------------------------------------------------------------- S1, R1

fn s1(kind: ElementKind, name: &str) {
    let mut m = sector(kind, shell_options(), &no_shift);
    let mut full = ring(kind);
    for model in [&mut m, &mut full] {
        model.add_load(Load::centrifugal(axis(), ORIGIN, OMEGA)).unwrap();
        model.add_load(Load::shell_pressure("all", 2e4)).unwrap();
    }
    for (i, a) in loaded() {
        let q = sector_node(i, a);
        let (f, mo) = nodal_load(q as f64);
        m.add_load(Load::nodal_force(&format!("p{q}"), f)).unwrap();
        m.add_load(Load::nodal_moment(&format!("p{q}"), mo)).unwrap();
        for j in 0..N {
            let r = rot(j);
            let set = format!("p{}", ring_node(j, i, a));
            full.add_load(Load::nodal_force(&set, apply(&r, f))).unwrap();
            full.add_load(Load::nodal_moment(&set, apply(&r, mo))).unwrap();
        }
    }
    let s = solve_static_mpc(&m, &[], &[], &[], &StaticOptions::default()).unwrap();
    let us = s.result.displacement_values().to_vec();
    let ur = solve_static(&full, &StaticOptions::default()).unwrap();
    let zero = vec![0.0; us.len()];
    let (eu, et) = field_error(&m, &us, &zero, 0, &full, ur.displacement_values());
    let e = eu.max(et);
    println!(
        "{name}: e_u {eu:.4e}, e_theta {et:.4e}, max {e:.4e} (limit <= 1e-10) {}",
        met(e <= 1e-10)
    );
    // R1: the rotation rows hold in the solution.
    let ci = rcfd_fem::cyclic::the_cyclic_constraint(&m).unwrap();
    let bd = rcfd_fem::cyclic::boundary(&m, ci).unwrap();
    let dofs = m.dof_map();
    let theta = |p: usize| -> Vec3 { [0, 1, 2].map(|c| us[dofs.equation(p, 3 + c).unwrap()]) };
    let tmax = (0..m.mesh().nodes().len())
        .flat_map(theta)
        .fold(0.0f64, |acc, x| acc.max(x.abs()));
    let mut r1 = 0.0f64;
    for &(l, h) in bd.pairs() {
        let w = apply(bd.rotation(), theta(l));
        let th = theta(h);
        for c in 0..3 {
            r1 = r1.max((th[c] - w[c]).abs());
        }
    }
    let r1 = r1 / tmax;
    println!("R1 ({name}): {r1:.4e} (limit <= 1e-14) {}", met(r1 <= 1e-14));
    assert!(e <= 1e-10, "{name}: {e:e}");
    assert!(r1 <= 1e-14, "R1 {name}: {r1:e}");
}

#[test]
fn s1a_r1_shell4_static_k0_against_the_full_ring() {
    s1(ElementKind::Shell4, "S1a Shell4 k = 0");
}

#[test]
fn s1b_r1_shell3_static_k0_against_the_full_ring() {
    s1(ElementKind::Shell3, "S1b Shell3 k = 0");
}

// ---------------------------------------------------------------------------------------------- S2

fn s2(kind: ElementKind, name: &str) {
    let m = sector(kind, shell_options(), &no_shift);
    let dofs = m.dof_map();
    let n = dofs.n_equations();
    let mut worst = 0.0f64;
    for k in 1..=N / 2 {
        let doubled = 2 * k < N;
        let (mut fa, mut fb) = (vec![0.0; n], vec![0.0; n]);
        for (i, a) in loaded() {
            let q = sector_node(i, a);
            for (part, v) in [(0.0, &mut fa), (7.0, &mut fb)] {
                if part != 0.0 && !doubled {
                    continue;
                }
                let (f, mo) = nodal_load(q as f64 + part);
                for c in 0..3 {
                    v[dofs.equation(q, c).unwrap()] = f[c];
                    v[dofs.equation(q, 3 + c).unwrap()] = mo[c];
                }
            }
        }
        let load = HarmonicLoad::new(k, fa.clone(), fb.clone());
        let hs = solve_static_harmonic(&m, &[], &load, &StaticOptions::default()).unwrap();
        let mut full = ring(kind);
        for (i, a) in loaded() {
            let q = sector_node(i, a);
            let get = |v: &[f64], base: usize| -> Vec3 { [0, 1, 2].map(|c| v[dofs.equation(q, base + c).unwrap()]) };
            for j in 0..N {
                let (c, s) = phase(k, j);
                let r = rot(j);
                let comb = |base: usize| -> Vec3 {
                    let (x, y) = (get(&fa, base), get(&fb, base));
                    apply(&r, [0, 1, 2].map(|t| c * x[t] - s * y[t]))
                };
                let set = format!("p{}", ring_node(j, i, a));
                full.add_load(Load::nodal_force(&set, comb(0))).unwrap();
                full.add_load(Load::nodal_moment(&set, comb(3))).unwrap();
            }
        }
        let ur = solve_static(&full, &StaticOptions::default()).unwrap();
        let (eu, et) = field_error(&m, &hs.cos, &hs.sin, k, &full, ur.displacement_values());
        let e = eu.max(et);
        println!(
            "{name} k = {k}: e_u {eu:.4e}, e_theta {et:.4e}, max {e:.4e} (limit <= 1e-10) {}",
            met(e <= 1e-10)
        );
        worst = worst.max(e);
    }
    println!(
        "{name}: max over k {worst:.4e} (limit <= 1e-10) {}",
        met(worst <= 1e-10)
    );
    assert!(worst <= 1e-10, "{name}: {worst:e}");
}

#[test]
fn s2a_shell4_harmonic_static_against_the_full_ring() {
    s2(ElementKind::Shell4, "S2a Shell4");
}

#[test]
fn s2b_shell3_harmonic_static_against_the_full_ring() {
    s2(ElementKind::Shell3, "S2b Shell3");
}

// ---------------------------------------------------------------------------------------------- E1, E2, E3

fn e(kind: ElementKind, name: &str) {
    let m = sector(kind, shell_options(), &no_shift);
    let full = ring(kind);
    let harmonics: Vec<_> = (0..=N / 2)
        .map(|k| modal_analysis_harmonic(&m, &[], k, &modal_options(4)).unwrap())
        .collect();
    let rr = modal_analysis(&full, &modal_options(24)).unwrap();
    let lc = (1.0 - 1e-6) * harmonics.iter().map(|h| h.values[3]).fold(f64::INFINITY, f64::min);
    let mut fails = Vec::new();
    let last = *rr.eigenvalues.last().unwrap();
    let complete = last >= lc;
    println!(
        "{name}: cut-off {lc:.6e}, largest ring eigenvalue {last:.6e} (set-up: >= cut-off) {}",
        met(complete)
    );
    if !complete {
        fails.push("ring list incomplete".to_string());
    }
    let mut sec: Vec<(f64, usize)> = harmonics
        .iter()
        .flat_map(|h| h.eigenvalues.iter().filter(|&&v| v < lc).map(move |&v| (v, h.harmonic)))
        .collect();
    sec.sort_by(|a, b| a.0.total_cmp(&b.0));
    let ringv: Vec<f64> = rr.eigenvalues.iter().copied().filter(|&v| v < lc).collect();
    println!(
        "{name}: count sector {} ring {} (equal) {}",
        sec.len(),
        ringv.len(),
        met(sec.len() == ringv.len())
    );
    if sec.len() != ringv.len() {
        fails.push(format!("counts {} {}", sec.len(), ringv.len()));
    }
    let mut per = [0.0f64; N / 2 + 1];
    for (&(vs, k), &vr) in sec.iter().zip(&ringv) {
        per[k] = per[k].max(((vs / vr).sqrt() - 1.0).abs());
    }
    for (k, &d) in per.iter().enumerate() {
        println!(
            "{name} k = {k}: max |f_s/f_r - 1| {d:.4e} (limit <= 1e-8) {}",
            met(d <= 1e-8)
        );
        if d > 1e-8 {
            fails.push(format!("k = {k}: {d:e}"));
        }
    }
    for h in &harmonics {
        let split = h.pair_splits.iter().fold(0.0f64, |a, &x| a.max(x));
        if h.multiplicity == 2 {
            println!(
                "{name} k = {}: pair split {split:.4e} (limit <= 1e-8) {}",
                h.harmonic,
                met(split <= 1e-8)
            );
            if split > 1e-8 {
                fails.push(format!("split k = {}: {split:e}", h.harmonic));
            }
        }
    }
    // E3: the k = 0 branch is modal_analysis_mpc.
    let mpc = modal_analysis_mpc(&m, &[], &modal_options(4)).unwrap();
    let same = mpc.result.frequencies.len() == harmonics[0].frequencies.len()
        && mpc
            .result
            .frequencies
            .iter()
            .zip(&harmonics[0].frequencies)
            .all(|(a, b)| a.to_bits() == b.to_bits());
    println!(
        "E3 ({name}): k = 0 frequencies equal bits to modal_analysis_mpc: {same} {}",
        met(same)
    );
    if !same {
        fails.push("E3".into());
    }
    println!("{name}: sector k = 0 frequencies {:?}", harmonics[0].frequencies);
    assert!(fails.is_empty(), "{name}: {fails:?}");
}

#[test]
fn e1_e3_shell4_frequencies_of_each_harmonic_against_the_full_ring() {
    e(ElementKind::Shell4, "E1 Shell4");
}

#[test]
fn e2_e3_shell3_frequencies_of_each_harmonic_against_the_full_ring() {
    e(ElementKind::Shell3, "E2 Shell3");
}

// ---------------------------------------------------------------------------------------------- NC1 to NC6

fn dist(a: Vec3, b: Vec3) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

#[test]
#[ignore = "ADR 0744: retired record, NC3 asserts the Beam2 refusal that is lifted by default; restated in sa_cyclic_shells_refusals_adr0744.rs"]
fn nc1_to_nc6_refusals() {
    let mut fails = Vec::new();
    let mut hold = |name: &str, ok: bool, detail: String| {
        println!("{name}: {detail} {}", met(ok));
        if !ok {
            fails.push(format!("{name}: {detail}"));
        }
    };
    // NC1: the shell sector with the default options (the boundaries must be surface sets; a shell has none).
    let mut m = SolidModel::new(shell_mesh(ElementKind::Shell4, NC + 1, false, &no_shift));
    let r = m.add_constraint(Constraint::cyclic("low", "high", axis(), ORIGIN, N));
    hold(
        "NC1 default options on a shell sector",
        matches!(
            &r,
            Err(FemError::UnknownSet {
                kind: SetKind::Surface,
                ..
            })
        ),
        format!("{r:?}"),
    );
    // NC2: the existing N2 set-up (Hex8 box, disjoint Shell4 element, default options) on the harmonic paths.
    let hb = hex_box([0.0, 1.0, 0.0], [1.0, 1.0, 1.0], [1, 1, 1]).unwrap();
    let nh = hb.nodes().len();
    let mut nodes = hb.nodes().to_vec();
    nodes.extend_from_slice(&[[3.0, 0.0, 0.0], [4.0, 0.0, 0.0], [4.0, 1.0, 0.0], [3.0, 1.0, 0.0]]);
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Hex8, hb.blocks()[0].connectivity().to_vec())
        .unwrap();
    mesh.add_block(ElementKind::Shell4, vec![nh, nh + 1, nh + 2, nh + 3])
        .unwrap();
    mesh.set_block_property(1, BlockProperty::shell(0.01).unwrap()).unwrap();
    mesh.add_element_set("all", vec![0, 1]).unwrap();
    for name in BOX_FACES {
        mesh.add_surface_set(name, hb.surface_set(name).unwrap().to_vec())
            .unwrap();
    }
    let mut m2 = SolidModel::new(mesh);
    m2.add_material("all", mat()).unwrap();
    m2.add_constraint(Constraint::cyclic("ymin", "ymax", [0.0, 0.0, 1.0], [0.0; 3], 4))
        .unwrap();
    let shell4 = |r: &Result<(), FemError>| matches!(r, Err(FemError::NotYetSupported { variant: "Shell4", package, .. }) if *package == shell::FOLLOW_UP);
    let r = modal_analysis_harmonic(&m2, &[], 1, &ModalOptions::new(2)).map(|_| ());
    hold(
        "NC2 default options, Shell4: modal_analysis_harmonic",
        shell4(&r),
        format!("{r:?}"),
    );
    let n2 = m2.dof_map().n_equations();
    let r = solve_static_harmonic(
        &m2,
        &[],
        &HarmonicLoad::new(1, vec![0.0; n2], vec![0.0; n2]),
        &StaticOptions::default(),
    )
    .map(|_| ());
    hold(
        "NC2 default options, Shell4: solve_static_harmonic",
        shell4(&r),
        format!("{r:?}"),
    );
    // NC3: a Beam2 model with the shells option.
    let mut mesh = SolidMesh::new(vec![point(0, 0.5, 0.0), point(1, 0.5, 0.0), point(2, 0.5, 0.0)]).unwrap();
    mesh.add_block(ElementKind::Beam2, vec![0, 1, 1, 2]).unwrap();
    let section = BeamSection::new(0.012, 3e-6, 5e-6, 4e-6, 0.85, 0.75).unwrap();
    mesh.set_block_property(0, BlockProperty::beam(section, axis()).unwrap())
        .unwrap();
    mesh.add_element_set("all", vec![0, 1]).unwrap();
    mesh.add_node_set("low", vec![0]).unwrap();
    mesh.add_node_set("high", vec![2]).unwrap();
    let mut mb = SolidModel::new(mesh);
    mb.add_material("all", mat()).unwrap();
    mb.add_constraint(Constraint::cyclic_with(
        "low",
        "high",
        axis(),
        ORIGIN,
        N,
        shell_options(),
    ))
    .unwrap();
    let beam = |r: &Result<(), FemError>| matches!(r, Err(FemError::NotYetSupported { variant: "Cyclic", package, .. }) if *package == rcfd_fem::beam::FOLLOW_UP);
    let r = solve_static_mpc(&mb, &[], &[], &[], &StaticOptions::default()).map(|_| ());
    hold(
        "NC3 Beam2 with the shells option: solve_static_mpc",
        beam(&r),
        format!("{r:?}"),
    );
    let r = modal_analysis_harmonic(&mb, &[], 1, &ModalOptions::new(1)).map(|_| ());
    hold(
        "NC3 Beam2 with the shells option: modal_analysis_harmonic",
        beam(&r),
        format!("{r:?}"),
    );
    // NC4: a six-unknown shell node (low) paired with a three-unknown Hex8 node (high).
    let p = point(0, 0.5, 0.0);
    let qh = point(NC, 0.5, 0.0);
    let hb = hex_box(qh, [0.05, 0.05, 0.05], [1, 1, 1]).unwrap();
    let corner = (0..hb.nodes().len())
        .min_by(|&a, &b| dist(hb.nodes()[a], qh).total_cmp(&dist(hb.nodes()[b], qh)))
        .unwrap();
    let nh = hb.nodes().len();
    let mut nodes = hb.nodes().to_vec();
    nodes.extend_from_slice(&[
        p,
        [p[0] - 0.05, p[1], p[2]],
        [p[0] - 0.05, p[1] - 0.05, p[2]],
        [p[0], p[1] - 0.05, p[2]],
    ]);
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Hex8, hb.blocks()[0].connectivity().to_vec())
        .unwrap();
    mesh.add_block(ElementKind::Shell4, vec![nh, nh + 1, nh + 2, nh + 3])
        .unwrap();
    mesh.set_block_property(1, BlockProperty::shell(0.01).unwrap()).unwrap();
    mesh.add_element_set("all", vec![0, 1]).unwrap();
    mesh.add_node_set("low", vec![nh]).unwrap();
    mesh.add_node_set("high", vec![corner]).unwrap();
    let mut m4 = SolidModel::new(mesh);
    m4.add_material("all", mat()).unwrap();
    m4.add_constraint(Constraint::cyclic_with(
        "low",
        "high",
        axis(),
        ORIGIN,
        N,
        shell_options(),
    ))
    .unwrap();
    let r = solve_static_mpc(&m4, &[], &[], &[], &StaticOptions::default()).map(|_| ());
    let ok = matches!(&r, Err(e @ FemError::InvalidValue { key: "cyclic", .. })
        if e.to_string().contains(&format!("low node {nh} has 6 unknowns")) && e.to_string().contains(&format!("{corner} has 3")));
    hold("NC4 a shell node paired with a solid node", ok, format!("{r:?}"));
    // NC5: the shells option with nodes on the axis and with interpolation.
    for (label, opts) in [
        ("axis nodes", shell_options().axis_nodes(true)),
        (
            "interpolation",
            shell_options().interpolation(TieOptions::new(TieMethod::NodeToSurface)),
        ),
    ] {
        let m5 = sector(ElementKind::Shell4, opts, &no_shift);
        let r = solve_static_mpc(&m5, &[], &[], &[], &StaticOptions::default()).map(|_| ());
        let ok =
            matches!(&r, Err(e @ FemError::InvalidValue { key: "cyclic", .. }) if e.to_string().contains("ADR 0556"));
        hold(&format!("NC5 the shells option with {label}"), ok, format!("{r:?}"));
    }
    // NC6: the high column moved by 1e-6 L (L = 1 m, the largest radius) along the axis.
    let moved = |i: usize, _a: usize| -> Vec3 {
        if i == NC {
            axis().map(|c| 1e-6 * c)
        } else {
            [0.0; 3]
        }
    };
    let m6 = sector(ElementKind::Shell4, shell_options(), &moved);
    let r = solve_static_mpc(&m6, &[], &[], &[], &StaticOptions::default()).map(|_| ());
    let ok = matches!(&r, Err(e @ FemError::InvalidValue { key: "cyclic", .. }) if e.to_string().contains("has no node of the high boundary"));
    hold("NC6 the high column moved by 1e-6 L", ok, format!("{r:?}"));
    assert!(fails.is_empty(), "missed: {fails:#?}");
}
