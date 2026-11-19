//! F-672 (ADR 0595): Beam2 elements in the cyclic constraint. Checks S0, R1, S1 to S3, M-list, M0 to M3, P0 to P3, Q1, Q2,
//! B3 and the negative controls NC1 to NC4 and NC-O of the registration (B1 and B2 are the existing tests and
//! `baseline_hashes`).

use rcfd_fem::assemble::stiffness;
use rcfd_fem::cyclic::{
    full_rotor, modal_analysis_harmonic, rotation_matrix, solve_static_harmonic, HarmonicLoad, Mat3,
};
use rcfd_fem::mass::consistent;
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::model::CyclicOptions;
use rcfd_fem::mpc::solve_static_mpc;
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::tie::{TieMethod, TieOptions};
use rcfd_fem::{BeamSection, BlockProperty, Constraint, ElementKind, FemError, Load, Material, SolidMesh, SolidModel};
use rcfd_linalg::eigen::ResidualTest;
use rcfd_linalg::CsrMatrix;
use std::f64::consts::PI;

type Vec3 = [f64; 3];

const E: f64 = 2.0e11;
const NU: f64 = 0.3;
const RHO: f64 = 7850.0;
const N: usize = 6;
const OMEGA: f64 = 100.0;
const ORIGIN: Vec3 = [0.3, -0.2, 0.5];
/// Ring elements per sector.
const NR: usize = 4;
/// The loaded sector nodes (not the high node 4, not the hub 6).
const LOADED: [usize; 5] = [0, 1, 2, 3, 5];

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

fn rot(j: usize) -> Mat3 {
    rotation_matrix(axis(), 2.0 * PI * j as f64 / N as f64)
}

fn apply(r: &Mat3, v: Vec3) -> Vec3 {
    [0, 1, 2].map(|i| r[i][0] * v[0] + r[i][1] * v[1] + r[i][2] * v[2])
}

fn modal_options(modes: usize) -> ModalOptions {
    let mut o = ModalOptions::new(modes);
    o.tolerance = 1e-12;
    o.residual_test = ResidualTest::Backward;
    o
}

fn beam_options() -> CyclicOptions {
    CyclicOptions::new().shells(true).beams(true)
}

/// The force and moment of sector node `q` (as `sa_cyclic_shells.rs`).
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
        m.add_constraint(Constraint::displacement("hub", comp, 0.0)).unwrap();
    }
}

/// The wheel sector W of ADR 0595 with the cyclic constraint of `options`.
fn sector(options: CyclicOptions) -> SolidModel {
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
    let mut m = SolidModel::new(mesh);
    m.add_material("all", mat()).unwrap();
    clamp(&mut m);
    m.add_constraint(Constraint::cyclic_with("low", "high", axis(), ORIGIN, N, options))
        .unwrap();
    m
}

/// The ring node of sector node `q` in sector `j`.
fn ring_node(j: usize, q: usize) -> usize {
    match q {
        0..=NR => (NR * j + q) % (NR * N),
        5 => NR * N + j,
        6 => NR * N + N + j,
        _ => unreachable!("sector node"),
    }
}

/// The full ring of beams; `rotated`: orientations `R^j v` in sector `j` (else `v` everywhere: control NC-O).
fn ring(rotated: bool) -> SolidModel {
    let a = alpha();
    let mut nodes: Vec<Vec3> = (0..NR * N).map(|i| p(i as f64 * a / NR as f64, 1.0, 0.0)).collect();
    for j in 0..N {
        nodes.push(p((j as f64 + 0.5) * a, 0.65, 0.125));
    }
    for j in 0..N {
        nodes.push(p((j as f64 + 0.5) * a, 0.3, 0.25));
    }
    let mut mesh = SolidMesh::new(nodes).unwrap();
    for j in 0..N {
        let r = if rotated { rot(j) } else { rot(0) };
        let conn: Vec<usize> = (0..NR).flat_map(|i| [ring_node(j, i), ring_node(j, i + 1)]).collect();
        let b = mesh.add_block(ElementKind::Beam2, conn).unwrap();
        mesh.set_block_property(b, BlockProperty::beam(ring_section(), apply(&r, v_ring())).unwrap())
            .unwrap();
        let conn = vec![ring_node(j, 6), ring_node(j, 5), ring_node(j, 5), ring_node(j, 2)];
        let b = mesh.add_block(ElementKind::Beam2, conn).unwrap();
        mesh.set_block_property(b, BlockProperty::beam(spoke_section(), apply(&r, v_spoke())).unwrap())
            .unwrap();
    }
    mesh.add_element_set("all", (0..6 * N).collect()).unwrap();
    mesh.add_node_set("hub", (0..N).map(|j| ring_node(j, 6)).collect())
        .unwrap();
    for j in 0..N {
        for q in LOADED {
            let rn = ring_node(j, q);
            mesh.add_node_set(&format!("p{rn}"), vec![rn]).unwrap();
        }
    }
    let mut m = SolidModel::new(mesh);
    m.add_material("all", mat()).unwrap();
    clamp(&mut m);
    m
}

/// `(cos(j theta), sin(j theta))`, `theta = 2 pi k / N`.
fn phase(k: usize, j: usize) -> (f64, f64) {
    let t = 2.0 * PI * ((j * k) % N) as f64 / N as f64;
    (t.cos(), t.sin())
}

/// The sector field `(a, b)` of harmonic `k` expanded to the ring (ADR 0595 D2): `R^j (c a - s b)`, translations and
/// rotations.
fn expand(sector: &SolidModel, a: &[f64], b: &[f64], k: usize, ring: &SolidModel) -> Vec<f64> {
    let (sd, rd) = (sector.dof_map(), ring.dof_map());
    let mut out = vec![0.0; rd.n_equations()];
    for j in 0..N {
        let r = rot(j);
        let (c, s) = phase(k, j);
        for q in (0..7).filter(|&q| q != NR) {
            for base in [0, 3] {
                let v: Vec3 = [0, 1, 2].map(|m| {
                    let e = sd.equation(q, base + m).unwrap();
                    c * a[e] - s * b[e]
                });
                let w = apply(&r, v);
                for m in 0..3 {
                    out[rd.equation(ring_node(j, q), base + m).unwrap()] = w[m];
                }
            }
        }
    }
    out
}

/// `(e_u, e_theta)` of the sector field `(a, b)` of harmonic `k` mapped to every sector against the ring field `ur`.
fn field_error(sector: &SolidModel, a: &[f64], b: &[f64], k: usize, ring: &SolidModel, ur: &[f64]) -> (f64, f64) {
    let (sd, rd) = (sector.dof_map(), ring.dof_map());
    let (mut du, mut dt, mut mu, mut mt) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for j in 0..N {
        let r = rot(j);
        let (c, s) = phase(k, j);
        for q in 0..7 {
            let rn = ring_node(j, q);
            for base in [0, 3] {
                let v: Vec3 = [0, 1, 2].map(|m| {
                    let e = sd.equation(q, base + m).unwrap();
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
    (du / mu, dt / mt)
}

fn met(ok: bool) -> &'static str {
    if ok {
        "met"
    } else {
        "MISSED"
    }
}

/// The S0 sector and ring (`rotated` as [`ring`]) with their loads.
fn s0_models(rotated: bool) -> (SolidModel, SolidModel) {
    let mut m = sector(beam_options());
    let mut full = ring(rotated);
    for model in [&mut m, &mut full] {
        model.add_load(Load::centrifugal(axis(), ORIGIN, OMEGA)).unwrap();
    }
    for q in LOADED {
        let (f, mo) = nodal_load(q as f64);
        m.add_load(Load::nodal_force(&format!("p{q}"), f)).unwrap();
        m.add_load(Load::nodal_moment(&format!("p{q}"), mo)).unwrap();
        for j in 0..N {
            let r = rot(j);
            let set = format!("p{}", ring_node(j, q));
            full.add_load(Load::nodal_force(&set, apply(&r, f))).unwrap();
            full.add_load(Load::nodal_moment(&set, apply(&r, mo))).unwrap();
        }
    }
    (m, full)
}

// ---------------------------------------------------------------------------------------------- S0, R1, NC-O

#[test]
fn s0_r1_static_k0_against_the_full_ring_of_beams() {
    let (m, full) = s0_models(true);
    let s = solve_static_mpc(&m, &[], &[], &[], &StaticOptions::default()).unwrap();
    let us = s.result.displacement_values().to_vec();
    let ur = solve_static(&full, &StaticOptions::default()).unwrap();
    let zero = vec![0.0; us.len()];
    let (eu, et) = field_error(&m, &us, &zero, 0, &full, ur.displacement_values());
    let e = eu.max(et);
    println!(
        "S0: e_u {eu:.4e}, e_theta {et:.4e}, max {e:.4e} (limit <= 1e-10) {}",
        met(e <= 1e-10)
    );
    let dofs = m.dof_map();
    let theta = |q: usize| -> Vec3 { [0, 1, 2].map(|c| us[dofs.equation(q, 3 + c).unwrap()]) };
    let tmax = (0..7).flat_map(theta).fold(0.0f64, |acc, x| acc.max(x.abs()));
    let w = apply(&rot(1), theta(0));
    let th = theta(NR);
    let r1 = (0..3).map(|c| (th[c] - w[c]).abs()).fold(0.0f64, f64::max) / tmax;
    println!("R1: {r1:.4e} (limit <= 1e-14) {}", met(r1 <= 1e-14));
    assert!(e <= 1e-10, "S0: {e:e}");
    assert!(r1 <= 1e-14, "R1: {r1:e}");
}

#[test]
fn nc_o_ring_without_rotated_orientation_is_seen() {
    let (m, full) = s0_models(false);
    let s = solve_static_mpc(&m, &[], &[], &[], &StaticOptions::default()).unwrap();
    let us = s.result.displacement_values().to_vec();
    let ur = solve_static(&full, &StaticOptions::default()).unwrap();
    let zero = vec![0.0; us.len()];
    let (eu, et) = field_error(&m, &us, &zero, 0, &full, ur.displacement_values());
    let e = eu.max(et);
    println!(
        "NC-O: e_u {eu:.4e}, e_theta {et:.4e}, max {e:.4e} (limit >= 1e-3) {}",
        met(e >= 1e-3)
    );
    assert!(e >= 1e-3, "NC-O: {e:e}");
}

// ---------------------------------------------------------------------------------------------- S1 to S3

#[test]
fn s1_s3_harmonic_static_against_the_full_ring_of_beams() {
    let m = sector(beam_options());
    let dofs = m.dof_map();
    let n = dofs.n_equations();
    let mut fails = Vec::new();
    for k in 1..=N / 2 {
        let doubled = 2 * k < N;
        let (mut fa, mut fb) = (vec![0.0; n], vec![0.0; n]);
        for q in LOADED {
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
        let mut full = ring(true);
        for q in LOADED {
            let get = |v: &[f64], base: usize| -> Vec3 { [0, 1, 2].map(|c| v[dofs.equation(q, base + c).unwrap()]) };
            for j in 0..N {
                let (c, s) = phase(k, j);
                let r = rot(j);
                let part = |base: usize| -> Vec3 {
                    let (x, y) = (get(&fa, base), get(&fb, base));
                    apply(&r, [0, 1, 2].map(|t| c * x[t] - s * y[t]))
                };
                let set = format!("p{}", ring_node(j, q));
                full.add_load(Load::nodal_force(&set, part(0))).unwrap();
                full.add_load(Load::nodal_moment(&set, part(3))).unwrap();
            }
        }
        let ur = solve_static(&full, &StaticOptions::default()).unwrap();
        let (eu, et) = field_error(&m, &hs.cos, &hs.sin, k, &full, ur.displacement_values());
        let e = eu.max(et);
        println!(
            "S{k}: e_u {eu:.4e}, e_theta {et:.4e}, max {e:.4e} (limit <= 1e-10) {}",
            met(e <= 1e-10)
        );
        if e > 1e-10 {
            fails.push(format!("S{k}: {e:e}"));
        }
    }
    assert!(fails.is_empty(), "{fails:?}");
}

// ---------------------------------------------------------------------------------------------- M, P, Q

fn inf_norm(a: &CsrMatrix) -> f64 {
    (0..a.n)
        .map(|r| {
            a.val[a.row_ptr[r]..a.row_ptr[r + 1]]
                .iter()
                .map(|v| v.abs())
                .sum::<f64>()
        })
        .fold(0.0, f64::max)
}

fn matvec(a: &CsrMatrix, x: &[f64]) -> Vec<f64> {
    let mut y = vec![0.0; a.n];
    a.matvec(x, &mut y);
    y
}

#[test]
fn m_p_q_modes_of_each_harmonic_against_the_full_ring_of_beams() {
    let m = sector(beam_options());
    let full = ring(true);
    let harmonics: Vec<_> = (0..=N / 2)
        .map(|k| modal_analysis_harmonic(&m, &[], k, &modal_options(4)).unwrap())
        .collect();
    let rr = modal_analysis(&full, &modal_options(30)).unwrap();
    let lc = (1.0 - 1e-6) * harmonics.iter().map(|h| h.values[3]).fold(f64::INFINITY, f64::min);
    let mut fails = Vec::new();
    let last = *rr.eigenvalues.last().unwrap();
    let complete = last >= lc;
    println!(
        "M-list: cut-off {lc:.6e}, largest ring eigenvalue {last:.6e} (>= cut-off) {}; ring iterations {}",
        met(complete),
        rr.iterations
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
        "M-list: count sector {} ring {} (equal) {}",
        sec.len(),
        ringv.len(),
        met(sec.len() == ringv.len())
    );
    if sec.len() != ringv.len() {
        fails.push(format!("counts {} {}", sec.len(), ringv.len()));
    }
    let mut per = [0.0f64; N / 2 + 1];
    for (&(vs, k), &vr) in sec.iter().zip(&ringv) {
        per[k] = per[k].max((vs / vr - 1.0).abs());
    }
    for (k, &d) in per.iter().enumerate() {
        println!(
            "M{k}: max |lambda_s/lambda_r - 1| {d:.4e} (limit <= 1e-10) {}",
            met(d <= 1e-10)
        );
        if d > 1e-10 {
            fails.push(format!("M{k}: {d:e}"));
        }
    }
    // P: the expanded sector modes in the ring, backward error at the free unknowns.
    let kf = stiffness(&full).unwrap();
    let mf = consistent(&full).unwrap();
    let (nk, nm) = (inf_norm(&kf), inf_norm(&mf));
    let rd = full.dof_map();
    let fixed: Vec<bool> = (0..rd.n_equations())
        .map(|e| {
            let (node, _) = rd.node_component(e).unwrap();
            node >= NR * N + N
        })
        .collect();
    for h in &harmonics {
        let mut worst = 0.0f64;
        let mut count = 0;
        for (lam, mode) in h.eigenvalues.iter().zip(&h.modes) {
            if *lam >= lc {
                continue;
            }
            count += 1;
            let phi = expand(&m, &mode.cos, &mode.sin, h.harmonic, &full);
            let (kp, mp) = (matvec(&kf, &phi), matvec(&mf, &phi));
            let r = (0..phi.len())
                .filter(|&e| !fixed[e])
                .map(|e| (kp[e] - lam * mp[e]).abs())
                .fold(0.0f64, f64::max);
            let pn = phi.iter().fold(0.0f64, |a, x| a.max(x.abs()));
            worst = worst.max(r / ((nk + lam.abs() * nm) * pn));
        }
        println!(
            "P{}: e_phi {worst:.4e} over {count} modes (limit <= 1e-10) {}; sector residuals max {:.3e}, iterations {}",
            h.harmonic,
            met(worst <= 1e-10),
            h.residuals.iter().fold(0.0f64, |a, &x| a.max(x)),
            h.iterations
        );
        if worst > 1e-10 || count == 0 {
            fails.push(format!("P{}: {worst:e} ({count})", h.harmonic));
        }
        if h.multiplicity == 2 {
            let split = h.pair_splits.iter().fold(0.0f64, |a, &x| a.max(x));
            println!(
                "Q{}: pair split {split:.4e} (limit <= 1e-10) {}",
                h.harmonic,
                met(split <= 1e-10)
            );
            if split > 1e-10 {
                fails.push(format!("Q{}: {split:e}", h.harmonic));
            }
        }
        println!("k = {}: sector frequencies {:?}", h.harmonic, h.frequencies);
    }
    assert!(fails.is_empty(), "{fails:?}");
}

// ---------------------------------------------------------------------------------------------- B3: the shell twin

const NC: usize = 2;
const NA: usize = 3;

/// The ADR 0556 D3 frustum point at column `i` and generator parameter `s`.
fn frustum_point(i: usize, s: f64) -> Vec3 {
    let phi = 2.0 * PI * i as f64 / (N * NC) as f64;
    p(phi, 1.0 - 0.4 * s, 0.8 * s)
}

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
    m.add_load(Load::centrifugal(axis(), ORIGIN, OMEGA)).unwrap();
    m
}

fn shell_results(m: &SolidModel) -> Vec<f64> {
    let s = solve_static_mpc(m, &[], &[], &[], &StaticOptions::default()).unwrap();
    let mut out = s.result.displacement_values().to_vec();
    for k in 0..=N / 2 {
        out.extend(
            modal_analysis_harmonic(m, &[], k, &modal_options(4))
                .unwrap()
                .frequencies,
        );
    }
    out
}

#[test]
fn b3_shell_sector_bits_with_and_without_the_beams_option() {
    let a = shell_results(&shell_sector(CyclicOptions::new().shells(true)));
    let b = shell_results(&shell_sector(beam_options()));
    let same = a.len() == b.len() && a.iter().zip(&b).all(|(x, y)| x.to_bits() == y.to_bits());
    println!("B3: {} values, bits equal: {same} {}", a.len(), met(same));
    assert!(same, "B3");
}

// ---------------------------------------------------------------------------------------------- NC1 to NC4

fn names_adr(r: &Result<(), FemError>) -> bool {
    matches!(r, Err(e @ FemError::InvalidValue { key: "cyclic", .. }) if e.to_string().contains("ADR 0595"))
}

/// ADR 0595 amendment 1: the run-1 record (its NC1 set-up refused by `add_constraint`); body unchanged. The corrected run is
/// `nc1_to_nc4_refusals_amended`.
#[test]
#[ignore = "ADR 0595 amendment 1: run-1 record, the NC1 set-up refused by add_constraint (UnknownSet Surface low)"]
fn nc1_to_nc4_refusals_run1_record() {
    let mut fails = Vec::new();
    let mut hold = |name: &str, ok: bool, what: String| {
        println!("{name}: {} ({what})", met(ok));
        if !ok {
            fails.push(name.to_string());
        }
    };
    // NC1: beams without shells.
    let m = sector(CyclicOptions::new().beams(true));
    let r = solve_static_mpc(&m, &[], &[], &[], &StaticOptions::default()).map(|_| ());
    hold("NC1 solve_static_mpc", names_adr(&r), format!("{r:?}"));
    let r = modal_analysis_harmonic(&m, &[], 1, &ModalOptions::new(2)).map(|_| ());
    hold("NC1 modal_analysis_harmonic k = 1", names_adr(&r), format!("{r:?}"));
    // NC2: beams with axis nodes or interpolation.
    let m = sector(beam_options().shell_extensions(true).axis_nodes(true));
    let r = solve_static_mpc(&m, &[], &[], &[], &StaticOptions::default()).map(|_| ());
    hold("NC2 axis nodes: solve_static_mpc", names_adr(&r), format!("{r:?}"));
    let m = sector(
        beam_options()
            .shell_extensions(true)
            .interpolation(TieOptions::new(TieMethod::NodeToSurface)),
    );
    let r = solve_static_mpc(&m, &[], &[], &[], &StaticOptions::default()).map(|_| ());
    hold("NC2 interpolation: solve_static_mpc", names_adr(&r), format!("{r:?}"));
    // NC3: shells only, unchanged.
    let m = sector(CyclicOptions::new().shells(true));
    let beam = |r: &Result<(), FemError>| matches!(r, Err(FemError::NotYetSupported { variant: "Cyclic", package, .. }) if *package == rcfd_fem::beam::FOLLOW_UP);
    let r = solve_static_mpc(&m, &[], &[], &[], &StaticOptions::default()).map(|_| ());
    hold("NC3 shells only: solve_static_mpc", beam(&r), format!("{r:?}"));
    let r = modal_analysis_harmonic(&m, &[], 0, &ModalOptions::new(2)).map(|_| ());
    hold(
        "NC3 shells only: modal_analysis_harmonic k = 0",
        beam(&r),
        format!("{r:?}"),
    );
    // NC4: full_rotor still refuses Beam2.
    let m = sector(beam_options());
    let r = full_rotor(&m).map(|_| ());
    hold(
        "NC4 full_rotor",
        matches!(&r, Err(FemError::UnsupportedKind { kind: "Beam2", .. })),
        format!("{r:?}"),
    );
    assert!(fails.is_empty(), "{fails:?}");
}

/// NC1 to NC4 as amended by ADR 0595 amendment 1: NC1 is the refusal of `add_constraint` (D1a).
#[test]
#[ignore = "ADR 0744: retired record, NC3 asserts the Beam2 refusal that is lifted by default; restated in sa_cyclic_beams_default_adr0744.rs"]
fn nc1_to_nc4_refusals_amended() {
    let mut fails = Vec::new();
    let mut hold = |name: &str, ok: bool, what: String| {
        println!("{name}: {} ({what})", met(ok));
        if !ok {
            fails.push(name.to_string());
        }
    };
    // NC1 (amended): beams without shells, refused by add_constraint.
    let mut m = SolidModel::new(sector(beam_options()).mesh().clone());
    m.add_material("all", mat()).unwrap();
    let r = m.add_constraint(Constraint::cyclic_with(
        "low",
        "high",
        axis(),
        ORIGIN,
        N,
        CyclicOptions::new().beams(true),
    ));
    hold("NC1 add_constraint", names_adr(&r), format!("{r:?}"));
    // NC2: beams with axis nodes or interpolation.
    let m = sector(beam_options().shell_extensions(true).axis_nodes(true));
    let r = solve_static_mpc(&m, &[], &[], &[], &StaticOptions::default()).map(|_| ());
    hold("NC2 axis nodes: solve_static_mpc", names_adr(&r), format!("{r:?}"));
    let m = sector(
        beam_options()
            .shell_extensions(true)
            .interpolation(TieOptions::new(TieMethod::NodeToSurface)),
    );
    let r = solve_static_mpc(&m, &[], &[], &[], &StaticOptions::default()).map(|_| ());
    hold("NC2 interpolation: solve_static_mpc", names_adr(&r), format!("{r:?}"));
    // NC3: shells only, unchanged.
    let m = sector(CyclicOptions::new().shells(true));
    let beam = |r: &Result<(), FemError>| matches!(r, Err(FemError::NotYetSupported { variant: "Cyclic", package, .. }) if *package == rcfd_fem::beam::FOLLOW_UP);
    let r = solve_static_mpc(&m, &[], &[], &[], &StaticOptions::default()).map(|_| ());
    hold("NC3 shells only: solve_static_mpc", beam(&r), format!("{r:?}"));
    let r = modal_analysis_harmonic(&m, &[], 0, &ModalOptions::new(2)).map(|_| ());
    hold(
        "NC3 shells only: modal_analysis_harmonic k = 0",
        beam(&r),
        format!("{r:?}"),
    );
    // NC4: full_rotor still refuses Beam2.
    let m = sector(beam_options());
    let r = full_rotor(&m).map(|_| ());
    hold(
        "NC4 full_rotor",
        matches!(&r, Err(FemError::UnsupportedKind { kind: "Beam2", .. })),
        format!("{r:?}"),
    );
    assert!(fails.is_empty(), "{fails:?}");
}
