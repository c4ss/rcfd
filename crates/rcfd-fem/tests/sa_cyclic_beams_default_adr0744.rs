//! F-672 (ADR 0744): Beam2 elements in the cyclic constraint by default, that is with the shells option alone. Checks S0,
//! R1, S1 to S3, M-list, M0 to M3, P0 to P3, Q1, Q2, E-S, E-M, E-R, A3, N-0595, N-X, N-W, NC-O, AB-a, H-D and H-T of the
//! registration. The wheel sector, the ring and the measures are those of ADR 0595, the builders of
//! `sa_cyclic_beams_f672.rs` copied; the sector's constraint has `CyclicOptions::new().shells(true)`. The retired records
//! restated here: `nc1_to_nc4_refusals_amended` of that file and `a_b_beam2_cyclic_refused_for_its_own_reason` of
//! `sa_hyper_structures_f666.rs` (ADR 0593).

use rcfd_fem::assemble::stiffness;
use rcfd_fem::beam;
use rcfd_fem::cyclic::{
    full_rotor, modal_analysis_harmonic, rotation_matrix, solve_static_harmonic, HarmonicLoad, Mat3,
};
use rcfd_fem::mass::consistent;
use rcfd_fem::mat_hyper;
use rcfd_fem::material::HyperelasticEnergy;
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::model::CyclicOptions;
use rcfd_fem::mpc::solve_static_mpc;
use rcfd_fem::rotating::{HarmonicSpeedModes, RotatingOptions, RotatingSector};
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

/// The options of ADR 0595: shells and beams.
fn beam_options() -> CyclicOptions {
    CyclicOptions::new().shells(true).beams(true)
}

/// The options under test (ADR 0744 D1): the shells option alone.
fn default_options() -> CyclicOptions {
    CyclicOptions::new().shells(true)
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
    sector_of(options, mat(), false)
}

/// W with the material `material` and the setting `SolidModel::set_hyperelastic_structures(setting)` (H-D, H-T).
fn sector_of(options: CyclicOptions, material: Material, setting: bool) -> SolidModel {
    let mut m = SolidModel::new(sector_mesh());
    m.add_material("all", material).unwrap();
    m.set_hyperelastic_structures(setting);
    clamp(&mut m);
    m.add_constraint(Constraint::cyclic_with("low", "high", axis(), ORIGIN, N, options))
        .unwrap();
    m
}

/// The mesh of W.
fn sector_mesh() -> SolidMesh {
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
    s0_models_of(default_options(), rotated)
}

fn s0_models_of(options: CyclicOptions, rotated: bool) -> (SolidModel, SolidModel) {
    let mut m = sector(options);
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
fn s0_r1_static_k0_shells_option_alone_against_the_full_ring_of_beams() {
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
fn s1_s3_harmonic_static_shells_option_alone_against_the_full_ring_of_beams() {
    let m = sector(default_options());
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
fn m_p_q_modes_shells_option_alone_against_the_full_ring_of_beams() {
    let m = sector(default_options());
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

// ---------------------------------------------------------------------------------------------- E-S, E-M, E-R

/// The S-k harmonic load of ADR 0595 on W (the cosine part `nodal_load(q)`, the sine part `nodal_load(q + 7)`, zero at
/// `2 k = N`).
fn harmonic_load(m: &SolidModel, k: usize) -> HarmonicLoad {
    let dofs = m.dof_map();
    let n = dofs.n_equations();
    let (mut fa, mut fb) = (vec![0.0; n], vec![0.0; n]);
    for q in LOADED {
        for (part, v) in [(0.0, &mut fa), (7.0, &mut fb)] {
            if part != 0.0 && 2 * k >= N {
                continue;
            }
            let (f, mo) = nodal_load(q as f64 + part);
            for c in 0..3 {
                v[dofs.equation(q, c).unwrap()] = f[c];
                v[dofs.equation(q, 3 + c).unwrap()] = mo[c];
            }
        }
    }
    HarmonicLoad::new(k, fa, fb)
}

fn same_bits(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

/// The static values of E-S: the S0 displacements, then the cosine and sine parts of S1 to S3.
fn static_values(options: CyclicOptions) -> Vec<f64> {
    let (m, _) = s0_models_of(options, true);
    let s = solve_static_mpc(&m, &[], &[], &[], &StaticOptions::default()).unwrap();
    let mut out = s.result.displacement_values().to_vec();
    let m = sector(options);
    for k in 1..=N / 2 {
        let hs = solve_static_harmonic(&m, &[], &harmonic_load(&m, k), &StaticOptions::default()).unwrap();
        out.extend(hs.cos);
        out.extend(hs.sin);
    }
    out
}

/// The eigenvalues of E-M: `k = 0 .. 3`, 4 each.
fn modal_values(options: CyclicOptions) -> Vec<f64> {
    let m = sector(options);
    (0..=N / 2)
        .flat_map(|k| {
            modal_analysis_harmonic(&m, &[], k, &modal_options(4))
                .unwrap()
                .eigenvalues
        })
        .collect()
}

fn rotating_options(modes: usize) -> RotatingOptions {
    RotatingOptions::new(modes).modal(modal_options(modes))
}

/// The eigenvalues of E-R: `RotatingSector::modes`, both effects, `k = 0 .. 3` at 30 rad/s, 4 each.
fn rotating_values(options: CyclicOptions) -> Vec<f64> {
    let m = sector(options);
    let sec = RotatingSector::new(&m, &rotating_options(4)).unwrap();
    (0..=N / 2)
        .flat_map(|k| match sec.modes(k, 30.0).unwrap() {
            HarmonicSpeedModes::Modes(h) => h.eigenvalues.clone(),
            HarmonicSpeedModes::LossOfStiffness(l) => panic!("set-up failure: loss of stiffness {l:?}"),
        })
        .collect()
}

/// The values of one bit-equality row for a sector with the given options.
type Values = fn(CyclicOptions) -> Vec<f64>;

#[test]
fn e_s_e_m_e_r_the_shells_option_alone_has_the_bits_of_the_beams_option() {
    let mut fails = Vec::new();
    let rows: [(&str, Values); 3] = [
        ("E-S static", static_values),
        ("E-M modes", modal_values),
        ("E-R rotating modes", rotating_values),
    ];
    for (name, values) in rows {
        let a = values(default_options());
        let b = values(beam_options());
        let same = same_bits(&a, &b);
        println!("{name}: {} values, bits equal: {same} {}", a.len(), met(same));
        if !same {
            fails.push(name.to_string());
        }
    }
    assert!(fails.is_empty(), "{fails:?}");
}

// ---------------------------------------------------------------------------------------------- A3, N-0595, N-X, N-W

fn names(r: &Result<(), FemError>, adr: &str) -> bool {
    matches!(r, Err(e @ FemError::InvalidValue { key: "cyclic", .. }) if e.to_string().contains(adr))
}

/// `nc1_to_nc4_refusals_amended` of `sa_cyclic_beams_f672.rs` (ADR 0595) restated by ADR 0744: NC1, NC2 and NC4 as they
/// were (their bodies copied), NC3 as acceptance (A3), NC4 also with the shells option alone, and the controls N-X and N-W.
#[test]
#[ignore = "ADR 0799: retired record, NC4 and N-W assert the full_rotor and whirl refusals of a beam sector that are lifted by default; restated in sa_beam_opt_ins_default_f672.rs"]
fn a3_nc1_nc2_nc4_restated_and_n_x_n_w() {
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
    hold("NC1 add_constraint", names(&r, "ADR 0595"), format!("{r:?}"));
    // NC2: beams with axis nodes or interpolation.
    let m = sector(beam_options().shell_extensions(true).axis_nodes(true));
    let r = solve_static_mpc(&m, &[], &[], &[], &StaticOptions::default()).map(|_| ());
    hold(
        "NC2 axis nodes: solve_static_mpc",
        names(&r, "ADR 0595"),
        format!("{r:?}"),
    );
    let m = sector(
        beam_options()
            .shell_extensions(true)
            .interpolation(TieOptions::new(TieMethod::NodeToSurface)),
    );
    let r = solve_static_mpc(&m, &[], &[], &[], &StaticOptions::default()).map(|_| ());
    hold(
        "NC2 interpolation: solve_static_mpc",
        names(&r, "ADR 0595"),
        format!("{r:?}"),
    );
    // A3 (NC3 restated): the shells option alone is taken.
    let m = sector(default_options());
    let r = solve_static_mpc(&m, &[], &[], &[], &StaticOptions::default()).map(|_| ());
    hold("A3 shells only: solve_static_mpc", r.is_ok(), format!("{r:?}"));
    let r = modal_analysis_harmonic(&m, &[], 0, &modal_options(2)).map(|h| h.frequencies);
    hold(
        "A3 shells only: modal_analysis_harmonic k = 0",
        r.is_ok(),
        format!("{r:?}"),
    );
    // NC4: full_rotor still refuses Beam2, with the beams option and with the shells option alone.
    for (label, options) in [("beams option", beam_options()), ("shells only", default_options())] {
        let m = sector(options);
        let r = full_rotor(&m).map(|_| ());
        hold(
            &format!("NC4 full_rotor, {label}"),
            matches!(&r, Err(FemError::UnsupportedKind { kind: "Beam2", .. })),
            format!("{r:?}"),
        );
    }
    // N-X: beam ends on extended boundaries stay refused without the beams option (ADR 0744 D2).
    for (label, options) in [
        ("axis nodes", default_options().shell_extensions(true).axis_nodes(true)),
        (
            "interpolation",
            default_options()
                .shell_extensions(true)
                .interpolation(TieOptions::new(TieMethod::NodeToSurface)),
        ),
    ] {
        let m = sector(options);
        let r = solve_static_mpc(&m, &[], &[], &[], &StaticOptions::default()).map(|_| ());
        hold(
            &format!("N-X {label}: solve_static_mpc"),
            names(&r, "ADR 0744"),
            format!("{r:?}"),
        );
        let r = modal_analysis_harmonic(&m, &[], 1, &modal_options(2)).map(|_| ());
        hold(
            &format!("N-X {label}: modal_analysis_harmonic k = 1"),
            names(&r, "ADR 0744"),
            format!("{r:?}"),
        );
    }
    // N-W: the whirl paths keep the opt-in of ADR 0598.
    let m = sector(default_options());
    let sec = RotatingSector::new(&m, &rotating_options(4).coriolis(true)).unwrap();
    let r = sec.whirl_modes(1, 30.0).map(|_| ());
    let ok = match &r {
        Err(FemError::Element { source, .. }) => matches!(source.as_ref(),
            FemError::NotYetSupported { variant, package, .. } if *variant == "Beam2" && *package == beam::SOLID_ROUTINES),
        _ => false,
    };
    hold("N-W whirl_modes k = 1, shells only", ok, format!("{r:?}"));
    assert!(fails.is_empty(), "{fails:?}");
}

// ---------------------------------------------------------------------------------------------- AB-a, H-D, H-T

fn nhc() -> HyperelasticEnergy {
    HyperelasticEnergy::neo_hookean(1e6, 1e7).unwrap()
}

fn hyper() -> Material {
    Material::hyperelastic(nhc(), 1000.0).unwrap()
}

/// The isotropic twin of the energy: `E = 9 K G / s`, `nu = (3K - 2G) / (2 s)`, `s = 3K + G` (ADR 0584 D2).
fn twin() -> Material {
    let (g, k) = mat_hyper::small_strain_moduli(&nhc());
    let s = 3.0 * k + g;
    Material::isotropic_elastic(9.0 * k * g / s, (3.0 * k - 2.0 * g) / (2.0 * s), 1000.0, 0.0).unwrap()
}

/// `a_b_beam2_cyclic_refused_for_its_own_reason` of `sa_hyper_structures_f666.rs` (ADR 0593 A-B) restated by ADR 0744.
#[test]
#[ignore = "ADR 0746: retired record, H-D asserts the refusal of a hyperelastic material on a structural element that is lifted by default; restated in sa_hyper_structures_default_adr0746.rs"]
fn ab_a_h_d_h_t_hyperelastic_beams_on_the_cyclic_path() {
    let mut fails = Vec::new();
    let mut hold = |name: &str, ok: bool, what: String| {
        println!("{name}: {} ({what})", met(ok));
        if !ok {
            fails.push(name.to_string());
        }
    };
    // AB-a: the model of A-B (Beam B of ADR 0584 D3, hyperelastic with the setting; its set-up copied).
    let n = 10;
    let nodes: Vec<Vec3> = (0..=n).map(|i| [i as f64 / n as f64, 0.0, 0.0]).collect();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    let conn: Vec<usize> = (0..n).flat_map(|i| [i, i + 1]).collect();
    mesh.add_block(ElementKind::Beam2, conn).unwrap();
    let section = beam::rectangle_section(0.05, 0.03, 2.6e-7, 5.0 / 6.0).unwrap();
    mesh.set_block_property(0, BlockProperty::beam(section, [0.0, 1.0, 0.0]).unwrap())
        .unwrap();
    mesh.add_element_set("all", (0..n).collect()).unwrap();
    mesh.add_node_set("x0", vec![0]).unwrap();
    mesh.add_node_set("xl", vec![n]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", hyper()).unwrap();
    m.set_hyperelastic_structures(true);
    for c in 0..6 {
        m.add_constraint(Constraint::displacement("x0", c, 0.0)).unwrap();
    }
    m.add_constraint(Constraint::cyclic_with(
        "x0",
        "xl",
        [0.0, 0.0, 1.0],
        [0.0; 3],
        N,
        default_options(),
    ))
    .unwrap();
    let mut o = ModalOptions::new(3);
    o.residual_test = ResidualTest::Backward;
    let r = modal_analysis_harmonic(&m, &[], 0, &o).map(|_| ());
    let beam_refusal = matches!(&r, Err(FemError::NotYetSupported { package, .. }) if *package == beam::FOLLOW_UP);
    hold("AB-a not the beam refusal", !beam_refusal, format!("{r:?}"));
    hold(
        "AB-a refused by the boundary match",
        matches!(&r, Err(FemError::InvalidValue { key: "cyclic", .. })),
        format!("{r:?}"),
    );
    // H-D: W, hyperelastic, without the setting: the material's own refusal.
    let m = sector_of(default_options(), hyper(), false);
    let r = modal_analysis_harmonic(&m, &[], 0, &modal_options(4)).map(|_| ());
    let ok = matches!(&r, Err(FemError::NotYetSupported { variant, package, .. })
        if *variant == "Hyperelastic" && *package == mat_hyper::LINEAR_FOLLOW_UP);
    hold("H-D without the setting", ok, format!("{r:?}"));
    // H-T: W, hyperelastic with the setting, against the twin.
    let h = sector_of(default_options(), hyper(), true);
    let t = sector_of(default_options(), twin(), false);
    let mut worst = 0.0f64;
    for k in 0..=1 {
        let a = modal_analysis_harmonic(&h, &[], k, &modal_options(4)).unwrap();
        let b = modal_analysis_harmonic(&t, &[], k, &modal_options(4)).unwrap();
        assert_eq!(a.eigenvalues.len(), b.eigenvalues.len());
        for (x, y) in a.eigenvalues.iter().zip(&b.eigenvalues) {
            worst = worst.max((x / y - 1.0).abs());
        }
        println!(
            "H-T k = {k}: frequencies {:?} / twin {:?}",
            a.frequencies, b.frequencies
        );
    }
    hold(
        "H-T max |lambda_h / lambda_t - 1| (limit <= 1e-10)",
        worst.is_finite() && worst <= 1e-10,
        format!("{worst:.4e}"),
    );
    assert!(fails.is_empty(), "{fails:?}");
}
