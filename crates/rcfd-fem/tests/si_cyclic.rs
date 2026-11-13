//! F-394 (ADR 0386): cyclic symmetry, one sector of a rotor for statics and for modal analysis by harmonic index. Checks E1
//! to E6, C1 to C3, S1, S2, X1 and the negative controls R1 to R6 of the registration.

use rcfd_core::vec3::Vec3;
use rcfd_fem::assemble::stiffness;
use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::{prescribed_equations, Prescribed};
use rcfd_fem::cyclic::{self, boundary, full_rotor, modal_analysis_harmonic, HarmonicModes};
use rcfd_fem::hex20::upgrade_to_hex20_mapped;
use rcfd_fem::mass::consistent;
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::mpc;
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{Constraint, FaceRef, FemError, Load, Material, SolidMesh, SolidModel};
use rcfd_linalg::eigen::ResidualTest;
use rcfd_linalg::CsrMatrix;
use std::f64::consts::PI;

const E: f64 = 2.0e11;
const RHO: f64 = 7850.0;
const N: usize = 12;
const AXIS: Vec3 = [0.0, 0.0, 1.0];
// Ring: centre-line radius, radial thickness, axial width.
const R: f64 = 1.0;
const T: f64 = 0.05;
const W: f64 = 0.1;

/// Records every measured value and its limit; asserts at the end so that one run records all numbers.
struct Checks {
    failed: Vec<String>,
}

impl Checks {
    fn new() -> Checks {
        Checks { failed: Vec::new() }
    }
    fn le(&mut self, id: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{id}: {value:.3e} (limit {limit:.0e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.failed.push(format!("{id}: {value:e} > {limit:e}"));
        }
    }
    fn holds(&mut self, id: &str, ok: bool, what: &str) {
        println!("{id}: {what}: {}", if ok { "holds" } else { "FAILS" });
        if !ok {
            self.failed.push(format!("{id}: {what}"));
        }
    }
    fn finish(self) {
        assert!(self.failed.is_empty(), "failed: {:?}", self.failed);
    }
}

fn material(nu: f64) -> Material {
    Material::isotropic_elastic(E, nu, RHO, 0.0).unwrap()
}

fn alpha_of(sectors: usize) -> f64 {
    2.0 * PI / sectors as f64
}

/// A Hex20 sector of an annulus: `hex_box` in `(r, phi, z)` mapped to `(r cos phi, r sin phi, z)`; `ymin` is the low face
/// `phi = 0`, `ymax` the high face `phi = alpha`, `xmin` the inner and `xmax` the outer face.
fn polar_sector(r0: f64, dr: f64, z0: f64, dz: f64, alpha: f64, div: [usize; 3]) -> SolidMesh {
    let b = hex_box([r0, 0.0, z0], [dr, alpha, dz], div).unwrap();
    upgrade_to_hex20_mapped(&b, |p| [p[0] * p[1].cos(), p[0] * p[1].sin(), p[2]])
        .unwrap()
        .0
}

fn nodes_where<F: Fn(Vec3) -> bool>(mesh: &SolidMesh, f: F) -> Vec<usize> {
    (0..mesh.nodes().len()).filter(|&p| f(mesh.nodes()[p])).collect()
}

/// The same mesh with node `node` moved by `delta` (blocks and sets copied).
fn with_node_moved(mesh: &SolidMesh, node: usize, delta: Vec3) -> SolidMesh {
    let mut x = mesh.nodes().to_vec();
    for i in 0..3 {
        x[node][i] += delta[i];
    }
    let mut out = SolidMesh::new(x).unwrap();
    for b in mesh.blocks() {
        out.add_block(b.kind(), b.connectivity().to_vec()).unwrap();
    }
    for n in mesh.node_set_names() {
        out.add_node_set(n, mesh.node_set(n).unwrap().to_vec()).unwrap();
    }
    for n in mesh.element_set_names() {
        out.add_element_set(n, mesh.element_set(n).unwrap().to_vec()).unwrap();
    }
    for n in mesh.surface_set_names() {
        out.add_surface_set(n, mesh.surface_set(n).unwrap().to_vec()).unwrap();
    }
    out
}

fn model_on(mesh: SolidMesh, nu: f64) -> SolidModel {
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material(nu)).unwrap();
    m
}

fn clamp(m: &mut SolidModel, set: &str) {
    for c in 0..3 {
        m.add_constraint(Constraint::displacement(set, c, 0.0)).unwrap();
    }
}

fn cyclic_of(sectors: usize) -> Constraint {
    Constraint::cyclic("ymin", "ymax", AXIS, [0.0; 3], sectors)
}

fn mv(m: &CsrMatrix, x: &[f64]) -> Vec<f64> {
    let mut y = vec![0.0; m.n];
    m.matvec(x, &mut y);
    y
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn inf(v: &[f64]) -> f64 {
    v.iter().fold(0.0f64, |m, x| m.max(x.abs()))
}

fn invalid(r: Result<impl std::fmt::Debug, FemError>, what: &str) -> (&'static str, String) {
    match r {
        Err(FemError::InvalidValue { key, reason }) => {
            println!("{what}: refused, key {key}: {reason}");
            (key, reason)
        }
        other => panic!("{what}: expected InvalidValue, got {other:?}"),
    }
}

// ------------------------------------------------------------------------------------------------------------ models

/// RS: the supported ring, 4 elements per sector, clamped at the inner-face nodes at phi = alpha/2.
fn rs_mesh() -> SolidMesh {
    let a = alpha_of(N);
    let mut mesh = polar_sector(R - T / 2.0, T, -W / 2.0, W, a, [1, 4, 1]);
    let spoke = nodes_where(&mesh, |x| {
        ((x[0] * x[0] + x[1] * x[1]).sqrt() - (R - T / 2.0)).abs() < 1e-12 && (x[1].atan2(x[0]) - a / 2.0).abs() < 1e-12
    });
    assert_eq!(spoke.len(), 3, "spoke nodes");
    mesh.add_node_set("spoke", spoke).unwrap();
    mesh
}

fn rs_sector_on(mesh: SolidMesh) -> SolidModel {
    let mut m = model_on(mesh, 0.0);
    clamp(&mut m, "spoke");
    m.add_constraint(cyclic_of(N)).unwrap();
    m
}

fn rs_sector() -> SolidModel {
    rs_sector_on(rs_mesh())
}

/// DS: the thick disc a = 0.25, b = 1, h = 0.2, clamped on the inner face.
fn ds_sector(sectors: usize) -> SolidModel {
    let mut m = model_on(polar_sector(0.25, 0.75, -0.1, 0.2, alpha_of(sectors), [3, 1, 1]), 0.3);
    clamp(&mut m, "xmin");
    m.add_constraint(cyclic_of(sectors)).unwrap();
    m
}

fn equivalence_options(modes: usize) -> ModalOptions {
    let mut o = ModalOptions::new(modes).tolerance(1e-9);
    o.max_iterations = 400;
    o
}

// ------------------------------------------------------------------------------------------------------- equivalence

/// E1 (or E2) and E3 to E6 for one sector model and its full model (the clamp of `clamp_set` on the full rotor).
fn equivalence(name: &str, e_id: &str, model: &SolidModel, nu: f64, clamp_set: &str, ch: &mut Checks) {
    let harmonics: Vec<HarmonicModes> = (0..=N / 2)
        .map(|k| modal_analysis_harmonic(model, &[], k, &equivalence_options(4)).unwrap())
        .collect();
    for h in &harmonics {
        println!(
            "{name} k = {}: multiplicity {}, f = {:?} Hz, pair splits {:?}, shift {:e}, iterations {}, unknowns {}",
            h.harmonic, h.multiplicity, h.frequencies, h.pair_splits, h.shift, h.iterations, h.unknowns
        );
    }
    // E6: single for k = 0 and 6, doubled otherwise.
    let mult_ok = harmonics.iter().all(|h| {
        let want = if h.harmonic == 0 || 2 * h.harmonic == N { 1 } else { 2 };
        h.multiplicity == want && h.eigenvalues.len() == want * h.values.len() && h.values.len() == 4
    });
    ch.holds(&format!("E6 {name}"), mult_ok, "k = 0, 6 single, k = 1..5 doubled");
    let lambda_c = 0.999
        * harmonics
            .iter()
            .map(|h| h.values.iter().fold(0.0f64, |m, &v| m.max(v)))
            .fold(f64::INFINITY, f64::min);
    // The sector eigenvalues below the cut-off, with their harmonic and mode index.
    let mut below: Vec<(f64, usize, usize)> = Vec::new();
    for h in &harmonics {
        for (i, &l) in h.eigenvalues.iter().enumerate() {
            if l < lambda_c {
                below.push((l, h.harmonic, i));
            }
        }
    }
    below.sort_by(|a, b| a.0.total_cmp(&b.0));
    let n_c = below.len();
    // The full model.
    let rotor = full_rotor(model).unwrap();
    let mut full = model_on(rotor.mesh().clone(), nu);
    clamp(&mut full, clamp_set);
    let fr = modal_analysis(&full, &equivalence_options(n_c + 8)).unwrap();
    println!(
        "{name} full: {} unknowns, {} modes, shift {:e}, iterations {}, largest {:e}, cut-off {lambda_c:e}, n_c {n_c}",
        fr.unknowns,
        fr.eigenvalues.len(),
        fr.shift,
        fr.iterations,
        fr.eigenvalues.last().unwrap()
    );
    assert!(
        *fr.eigenvalues.last().unwrap() > lambda_c,
        "defective set-up: the full model's computed modes do not pass the cut-off"
    );
    // The ambiguity guard of the registration (a defective set-up, not a miss).
    let near = below
        .iter()
        .map(|b| b.0)
        .chain(fr.eigenvalues.iter().copied())
        .map(|l| (l / lambda_c - 1.0).abs())
        .fold(f64::INFINITY, f64::min);
    println!("{name}: nearest eigenvalue to the cut-off at {near:.3e} relative");
    assert!(
        near >= 1e-7,
        "defective set-up: an eigenvalue within 1e-7 of the cut-off"
    );
    let full_below: Vec<f64> = fr.eigenvalues.iter().copied().filter(|&l| l < lambda_c).collect();
    ch.holds(
        &format!("{e_id} count"),
        full_below.len() == n_c,
        &format!("full count {} = sector count {n_c}", full_below.len()),
    );
    let worst = below
        .iter()
        .zip(&full_below)
        .map(|(s, &f)| ((s.0 / f).sqrt() - 1.0).abs())
        .fold(0.0f64, f64::max);
    ch.le(&format!("{e_id} frequencies"), worst, 1e-8);
    // E3: per-harmonic counts of the pairing and the pair splits.
    let mut counts_ok = full_below.len() == n_c;
    for h in &harmonics {
        let s_count = h.eigenvalues.iter().filter(|&&l| l < lambda_c).count();
        let paired = below.iter().filter(|b| b.1 == h.harmonic).count();
        counts_ok &= s_count == paired && s_count % h.multiplicity == 0;
    }
    ch.holds(&format!("E3 {name} counts"), counts_ok, "per-harmonic counts paired");
    let split = harmonics
        .iter()
        .filter(|h| h.multiplicity == 2)
        .flat_map(|h| h.pair_splits.iter().copied())
        .fold(0.0f64, f64::max);
    ch.le(&format!("E3 {name} pair split"), split, 1e-8);
    // E4, E5: the expanded modes.
    let sdofs = model.dof_map();
    let fdofs = full.dof_map();
    let k = stiffness(&full).unwrap();
    let m = consistent(&full).unwrap();
    let fixed = prescribed_equations(&full, &[]).unwrap();
    let mut free = vec![true; fdofs.n_equations()];
    for &(e, _) in &fixed {
        free[e] = false;
    }
    let mut phis: Vec<Vec<f64>> = Vec::new();
    let mut worst_res: f64 = 0.0;
    let mut nonzero_at_clamp: f64 = 0.0;
    for &(l, kh, i) in &below {
        let phi = rotor.expand(&sdofs, &fdofs, kh, &harmonics[kh].modes[i]);
        let kp = mv(&k, &phi);
        let mp = mv(&m, &phi);
        let (mut rn, mut kn) = (0.0, 0.0);
        for e in 0..phi.len() {
            if free[e] {
                rn += (kp[e] - l * mp[e]).powi(2);
                kn += kp[e].powi(2);
            } else {
                nonzero_at_clamp = nonzero_at_clamp.max(phi[e].abs());
            }
        }
        worst_res = worst_res.max((rn / kn).sqrt());
        phis.push(phi);
    }
    ch.le(&format!("E4 {name} residual"), worst_res, 1e-7);
    ch.holds(
        &format!("E4 {name} clamp"),
        nonzero_at_clamp == 0.0,
        "expanded modes zero at the clamp",
    );
    let mphis: Vec<Vec<f64>> = phis.iter().map(|p| mv(&m, p)).collect();
    let mut orth: f64 = 0.0;
    for i in 0..phis.len() {
        for j in 0..phis.len() {
            let d = dot(&phis[i], &mphis[j]) - if i == j { 1.0 } else { 0.0 };
            orth = orth.max(d.abs());
        }
    }
    ch.le(&format!("E5 {name} M-orthonormality"), orth, 1e-8);
}

#[test]
fn e1_e3_to_e6_supported_ring() {
    let mut ch = Checks::new();
    equivalence("RS", "E1", &rs_sector(), 0.0, "spoke", &mut ch);
    ch.finish();
}

#[test]
fn e2_e3_to_e6_disc() {
    let mut ch = Checks::new();
    equivalence("DS", "E2", &ds_sector(N), 0.3, "xmin", &mut ch);
    ch.finish();
}

// ------------------------------------------------------------------------------------------------------- closed forms

#[test]
fn c1_to_c3_thin_ring_in_plane_bending() {
    let mut ch = Checks::new();
    let mut rf = model_on(polar_sector(R - T / 2.0, T, -W / 2.0, W, alpha_of(N), [1, 6, 1]), 0.0);
    rf.add_constraint(cyclic_of(N)).unwrap();
    let m = consistent(&rf).unwrap();
    let dofs = rf.dof_map();
    let opts = ModalOptions::new(2)
        .residual_test(ResidualTest::Backward)
        .tolerance(1e-12);
    for (id, n) in [("C1", 2usize), ("C2", 3usize)] {
        let h = modal_analysis_harmonic(&rf, &[], n, &opts).unwrap();
        let nf = n as f64;
        let w2 = E * T * T * nf * nf * (nf * nf - 1.0).powi(2) / (12.0 * RHO * R.powi(4) * (nf * nf + 1.0));
        println!(
            "RF k = {n}: values {:?}, closed form {w2:e}, splits {:?}, shift {:e}, residuals {:?}",
            h.values, h.pair_splits, h.shift, h.residuals
        );
        ch.le(&format!("{id} n = {n}"), (h.values[0] / w2 - 1.0).abs(), 0.02);
        println!("{id} signed: {:+.4e}", h.values[0] / w2 - 1.0);
        // C3: the axial share of the selected mode (both eigenvectors of the pair).
        let mut share: f64 = 0.0;
        for mode in &h.modes[..2] {
            let (ma, mb) = (mv(&m, &mode.cos), mv(&m, &mode.sin));
            let total = dot(&mode.cos, &ma) + dot(&mode.sin, &mb);
            let mut zpart = 0.0;
            for p in 0..dofs.n_nodes() {
                let e = dofs.equation(p, 2).unwrap();
                zpart += mode.cos[e] * ma[e] + mode.sin[e] * mb[e];
            }
            share = share.max((zpart / total).abs());
        }
        ch.le(&format!("C3 n = {n} axial share"), share, 1e-2);
    }
    ch.finish();
}

// ------------------------------------------------------------------------------------------------------------ statics

const A_IN: f64 = 0.25;
const B_OUT: f64 = 1.0;
const H_DISC: f64 = 0.02;
const NU_DISC: f64 = 0.3;
const OMEGA: f64 = 100.0;

/// DP: the spinning disc sector with the node sets `mid` (z = 0) and `pin` (the node (a, 0, 0)).
fn dp_mesh() -> SolidMesh {
    let mut mesh = polar_sector(A_IN, B_OUT - A_IN, -H_DISC / 2.0, H_DISC, alpha_of(N), [8, 2, 1]);
    let mid = nodes_where(&mesh, |x| x[2] == 0.0);
    let pin = nodes_where(&mesh, |x| {
        (x[0] - A_IN).abs() < 1e-12 && x[1].abs() < 1e-12 && x[2] == 0.0
    });
    assert_eq!(pin.len(), 1, "pin node");
    mesh.add_node_set("mid", mid).unwrap();
    mesh.add_node_set("pin", pin).unwrap();
    mesh
}

fn dp_constraints(m: &mut SolidModel, pin_set: &str) {
    m.add_constraint(Constraint::displacement("mid", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement(pin_set, 1, 0.0)).unwrap();
    m.add_load(Load::centrifugal(AXIS, [0.0; 3], OMEGA)).unwrap();
}

#[test]
fn s1_s2_spinning_disc() {
    let mut ch = Checks::new();
    let mut dp = model_on(dp_mesh(), NU_DISC);
    dp_constraints(&mut dp, "pin");
    dp.add_constraint(cyclic_of(N)).unwrap();
    let sol = mpc::solve_static_mpc(&dp, &[], &[], &[], &StaticOptions::default()).unwrap();
    let u = sol.result.displacement_values().to_vec();
    let dofs = dp.dof_map();
    let x = dp.mesh().nodes();
    let rim = dp.mesh().node_set("xmax").unwrap();
    let mut sum = 0.0;
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for &p in rim {
        let r = (x[p][0] * x[p][0] + x[p][1] * x[p][1]).sqrt();
        let ur = (u[dofs.equation(p, 0).unwrap()] * x[p][0] + u[dofs.equation(p, 1).unwrap()] * x[p][1]) / r;
        sum += ur;
        lo = lo.min(ur);
        hi = hi.max(ur);
    }
    let mean = sum / rim.len() as f64;
    let ub =
        RHO * OMEGA * OMEGA * B_OUT * ((3.0 + NU_DISC) * A_IN * A_IN + (1.0 - NU_DISC) * B_OUT * B_OUT) / (4.0 * E);
    println!(
        "DP: rim mean {mean:e}, min {lo:e}, max {hi:e}, closed form {ub:e}, dropped rows {}, residual {:e}",
        sol.dropped.len(),
        sol.report.relative_residual
    );
    ch.le("S1 rim displacement", (mean / ub - 1.0).abs(), 0.01);
    println!("S1 signed: {:+.4e}", mean / ub - 1.0);
    // The same through the dispatch of solve_static: identical bits.
    let via = solve_static(&dp, &StaticOptions::default()).unwrap();
    ch.holds(
        "S1 dispatch",
        via.displacement_values()
            .iter()
            .zip(&u)
            .all(|(a, b)| a.to_bits() == b.to_bits()),
        "solve_static gives the bits of solve_static_mpc",
    );
    // S2: the full disc.
    let rotor = full_rotor(&dp).unwrap();
    let mut mesh = rotor.mesh().clone();
    let pin = dp.mesh().node_set("pin").unwrap()[0];
    // Amendment 1: the full disc also needs its in-plane translations held, at points where the exact axisymmetric
    // solution has the held component zero (u_y at phi = 0 and 180 degrees, u_x at 90 degrees).
    mesh.add_node_set("pin0", vec![rotor.node(0, pin), rotor.node(6, pin)])
        .unwrap();
    mesh.add_node_set("pin3", vec![rotor.node(3, pin)]).unwrap();
    let mut full = model_on(mesh, NU_DISC);
    dp_constraints(&mut full, "pin0");
    full.add_constraint(Constraint::displacement("pin3", 0, 0.0)).unwrap();
    let uf = solve_static(&full, &StaticOptions::default()).unwrap();
    let fdofs = full.dof_map();
    let ufv = uf.displacement_values();
    let mut diff: f64 = 0.0;
    for p in 0..x.len() {
        let q = rotor.node(0, p);
        for c in 0..3 {
            diff = diff.max((u[dofs.equation(p, c).unwrap()] - ufv[fdofs.equation(q, c).unwrap()]).abs());
        }
    }
    ch.le("S2 sector against full", diff / inf(ufv), 1e-8);
    ch.finish();
}

// ------------------------------------------------------------------------------------------------------------ X1

#[test]
fn x1_landed_and_dispatched() {
    let mut ch = Checks::new();
    ch.holds(
        "X1 PENDING",
        cyclic::PENDING.is_none(),
        &format!("{:?}", cyclic::PENDING),
    );
    ch.holds(
        "X1 dispatch",
        mpc::uses_transformation(&rs_sector()),
        "a model with a Cyclic constraint takes the transformation",
    );
    let mut plain = model_on(rs_mesh(), 0.0);
    clamp(&mut plain, "spoke");
    ch.holds(
        "X1 plain",
        !mpc::uses_transformation(&plain),
        "RS without the Cyclic constraint does not",
    );
    ch.finish();
}

// ------------------------------------------------------------------------------------------------- negative controls

#[test]
fn r1_unmatched_boundaries_refused() {
    let base = rs_mesh();
    let model = rs_sector_on(base.clone());
    let ci = 3; // three clamps, then the cyclic constraint
    let bd = boundary(&model, ci).unwrap();
    let length = bd.length();
    println!(
        "R1: L = {length:e}, tolerance {:e}, {} pairs",
        bd.tolerance(),
        bd.pairs().len()
    );
    let highs = base.surface_set_nodes("ymax").unwrap();
    let moved = highs[2];
    let partner = bd.pairs().iter().find(|p| p.1 == moved).unwrap().0;
    let x = base.nodes()[moved];
    let r = (x[0] * x[0] + x[1] * x[1]).sqrt();
    let radial = |s: f64| [s * x[0] / r, s * x[1] / r, 0.0];
    // R1: 100 times the tolerance.
    let m1 = rs_sector_on(with_node_moved(&base, moved, radial(1e-6 * length)));
    let (key, reason) = invalid(boundary(&m1, ci), "R1");
    assert_eq!(key, "cyclic");
    assert!(
        reason.contains(&format!("low node {partner} ")),
        "R1 names low node {partner}: {reason}"
    );
    // The static solve meets the same refusal.
    let (key, _) = invalid(
        mpc::solve_static_mpc(&m1, &[], &[], &[], &StaticOptions::default()),
        "R1 solve",
    );
    assert_eq!(key, "cyclic");
    // R1b: below the tolerance.
    let m2 = rs_sector_on(with_node_moved(&base, moved, radial(1e-10 * length)));
    let bd2 = boundary(&m2, ci).unwrap();
    assert_eq!(bd2.pairs(), bd.pairs(), "R1b pairs");
    println!("R1b: accepted, same {} pairs", bd2.pairs().len());
    // R1c: an extra face in the high set.
    let mut mesh = base.clone();
    let mut faces = mesh.surface_set("ymax").unwrap().to_vec();
    faces.push(FaceRef { element: 3, face: 3 });
    mesh.add_surface_set("high2", faces).unwrap();
    let mut m3 = model_on(mesh, 0.0);
    m3.add_constraint(Constraint::cyclic("ymin", "high2", AXIS, [0.0; 3], N))
        .unwrap();
    let (key, reason) = invalid(boundary(&m3, 0), "R1c");
    assert_eq!(key, "cyclic");
    assert!(
        reason.contains("high node") && reason.contains("image of no node"),
        "R1c: {reason}"
    );
    // R1d: R7's model of si_scaffold.rs.
    let mut mesh = hex_box([0.0; 3], [1.0, 0.1, 0.1], [20, 2, 2]).unwrap();
    let (lo, hi) = (
        mesh.surface_set("ymin").unwrap().to_vec(),
        mesh.surface_set("ymax").unwrap().to_vec(),
    );
    mesh.add_surface_set("low", lo).unwrap();
    mesh.add_surface_set("high", hi).unwrap();
    let mut m4 = model_on(mesh, 0.3);
    clamp(&mut m4, "xmin");
    m4.add_load(Load::nodal_force("xmax", [0.0, 0.0, -100.0])).unwrap();
    m4.add_constraint(Constraint::cyclic("low", "high", [1.0, 0.0, 0.0], [0.0; 3], 12))
        .unwrap();
    let (key, reason) = invalid(solve_static(&m4, &StaticOptions::default()), "R1d");
    assert_eq!(key, "cyclic");
    assert!(reason.contains("low node 0 "), "R1d: {reason}");
}

#[test]
fn r2_fewer_than_two_sectors_refused() {
    for s in [1usize, 0] {
        let mut m = model_on(rs_mesh(), 0.0);
        let (key, _) = invalid(
            m.add_constraint(Constraint::cyclic("ymin", "ymax", AXIS, [0.0; 3], s)),
            &format!("R2 N = {s}"),
        );
        assert_eq!(key, "sectors");
    }
}

#[test]
fn r3_harmonic_above_half_refused() {
    let (key, reason) = invalid(
        modal_analysis_harmonic(&rs_sector(), &[], 7, &ModalOptions::new(2)),
        "R3 k = 7",
    );
    assert_eq!(key, "harmonic");
    assert!(reason.contains('7') && reason.contains("12"), "R3: {reason}");
    let five = ds_sector(5);
    let (key, _) = invalid(
        modal_analysis_harmonic(&five, &[], 3, &ModalOptions::new(2)),
        "R3 N = 5, k = 3",
    );
    assert_eq!(key, "harmonic");
    let h = modal_analysis_harmonic(&five, &[], 2, &equivalence_options(2)).unwrap();
    println!(
        "R3 N = 5, k = 2: accepted, multiplicity {}, f = {:?}",
        h.multiplicity, h.frequencies
    );
    assert_eq!(h.multiplicity, 2);
}

#[test]
fn r4_node_on_both_boundaries_refused() {
    let mesh = rs_mesh();
    let first = mesh.surface_set_nodes("ymin").unwrap()[0];
    let mut m = model_on(mesh, 0.0);
    m.add_constraint(Constraint::cyclic("ymin", "ymin", AXIS, [0.0; 3], N))
        .unwrap();
    let (key, reason) = invalid(boundary(&m, 0), "R4");
    assert_eq!(key, "cyclic");
    assert!(reason.contains(&format!("node {first} is on both")), "R4: {reason}");
}

#[test]
fn r5_cyclic_constraint_count_refused() {
    let mut plain = model_on(rs_mesh(), 0.0);
    clamp(&mut plain, "spoke");
    let (key, _) = invalid(
        modal_analysis_harmonic(&plain, &[], 1, &ModalOptions::new(2)),
        "R5 none",
    );
    assert_eq!(key, "cyclic");
    let mut two = rs_sector();
    two.add_constraint(cyclic_of(N)).unwrap();
    let (key, _) = invalid(modal_analysis_harmonic(&two, &[], 1, &ModalOptions::new(2)), "R5 two");
    assert_eq!(key, "cyclic");
}

#[test]
#[ignore = "record of ADR 0386 before ADR 0395 (F-489) accepted rigid sets at k > 0; the valid row is kept in the _after_landed copy below (maintainer, 2026-11-13)"]
fn r6_rigid_set_and_non_zero_value_refused_at_k_2() {
    let mut m = rs_sector();
    m.add_constraint(Constraint::rigid_set("spoke", [R - T / 2.0, 0.0, 0.0]))
        .unwrap();
    let e = modal_analysis_harmonic(&m, &[], 2, &ModalOptions::new(2)).unwrap_err();
    println!("R6 rigid set: {e}");
    assert!(
        matches!(
            e,
            FemError::NotYetSupported {
                variant: "RigidSet",
                ..
            }
        ),
        "R6 {e:?}"
    );
    let node = rs_mesh().node_set("ymin").unwrap()[0];
    let (key, reason) = invalid(
        modal_analysis_harmonic(
            &rs_sector(),
            &[Prescribed::new(node, 0, 1e-3)],
            2,
            &ModalOptions::new(2),
        ),
        "R6 value",
    );
    assert_eq!(key, "constraint");
    assert!(reason.contains("zero values only"), "R6: {reason}");
}

/// The copy of R6 after ADR 0395 (F-489): a rigid set at `k = 2` is accepted; the refusal of a non-zero value stays.
#[test]
fn r6_non_zero_value_refused_at_k_2_after_landed() {
    let node = rs_mesh().node_set("ymin").unwrap()[0];
    let (key, reason) = invalid(
        modal_analysis_harmonic(
            &rs_sector(),
            &[Prescribed::new(node, 0, 1e-3)],
            2,
            &ModalOptions::new(2),
        ),
        "R6 value",
    );
    assert_eq!(key, "constraint");
    assert!(reason.contains("zero values only"), "R6: {reason}");
}
