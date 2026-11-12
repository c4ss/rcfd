//! F-309 (ADR 0234): the second-order immersed forcing, the fluid-solid face flux at the surface crossing.
//!
//! Acceptance: circular Couette flow between an immersed rotating cylinder and an immersed ring (the set-up CC and the check V1
//! of ADR 0218, limits unchanged) reaches an observed order of 1.5 with the forcing of `ImmersedForcing::build_second_order`.
//! The other checks are those registered in ADR 0234: the first-order forcing as the control, the face-flux measure, the PISO
//! path, DFG 2D-1 on M2, the reconstruction on the exact field, the net flux of a body, the defaults (state hashes measured on
//! main before the code) and the refusals. The set-up helpers are copies of those of `immersed_solid_f219.rs` (that file is not
//! edited). The heavy tests run in release only.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::convection::{face_flux, ConvScheme};
use rcfd_fv::diffusion::{uniform_gamma, GradientOptions};
use rcfd_fv::immersed::{ImmersedBody, ImmersedForcing, ImmersedSettings, SolidShape};
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::simple::{Coupling, SimpleSettings, SimpleSolver};
use rcfd_fv::{FixedValue, FvGeometry, Profile, ScalarField, VectorField, ZeroGradient};
use rcfd_mesh::gen::{axis_coords, box_2d, box_2d_coords, expansion_for_end_cell, Grading};
use std::sync::Arc;
use std::time::Instant;

fn fnv(words: impl Iterator<Item = u64>) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for w in words {
        for b in w.to_le_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}

fn state_hash(u: &[Vec3], p: &[f64]) -> u64 {
    fnv(u
        .iter()
        .flat_map(|v| v.iter().map(|x| x.to_bits()))
        .chain(p.iter().map(|x| x.to_bits())))
}

// ---------------------------------------------------------------------------------------------------------------------------
// Circular Couette (CC), the set-up of ADR 0218
// ---------------------------------------------------------------------------------------------------------------------------

const CC_C: Vec3 = [1.2, 1.2, 0.0];
const R1: f64 = 0.5;
const R2: f64 = 1.0;
const OMEGA: f64 = 1.0;

/// Which forcing a run uses.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Forcing {
    /// `ImmersedForcing::build` with `modified = true` (ADR 0218).
    First,
    /// `ImmersedForcing::build_second_order` (ADR 0234).
    Second,
}

fn cc_bodies() -> Vec<ImmersedBody> {
    let mut inner = ImmersedBody::fixed(
        "inner",
        SolidShape::Cylinder {
            centre: CC_C,
            axis: [0.0, 0.0, 1.0],
            radius: R1,
        },
    );
    inner.omega = [0.0, 0.0, OMEGA];
    inner.origin = CC_C;
    let mut outer = ImmersedBody::fixed(
        "outer",
        SolidShape::Cylinder {
            centre: CC_C,
            axis: [0.0, 0.0, 1.0],
            radius: R2,
        },
    );
    outer.inverted = true;
    vec![inner, outer]
}

/// The exact Couette profile `A r + B/r`, at any radius (the smooth extension on either side of a surface).
fn cc_exact(x: Vec3) -> Vec3 {
    let (dx, dy) = (x[0] - CC_C[0], x[1] - CC_C[1]);
    let r = (dx * dx + dy * dy).sqrt();
    let a = -OMEGA * R1 * R1 / (R2 * R2 - R1 * R1);
    let b = OMEGA * R1 * R1 * R2 * R2 / (R2 * R2 - R1 * R1);
    let ut = a * r + b / r;
    [-ut * dy / r, ut * dx / r, 0.0]
}

fn cc_fields_with(
    n: usize,
    forcing: Forcing,
    theta_min: f64,
) -> (Arc<FvGeometry>, VectorField, ScalarField, ImmersedForcing) {
    let geom = Arc::new(FvGeometry::new(Arc::new(box_2d(n, n, 2.4, 2.4, false, false).unwrap())).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    for w in ["xmin", "xmax", "ymin", "ymax"] {
        u.set_bc(&geom, w, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut st = ImmersedSettings::new(1e8);
    st.theta_min = theta_min;
    let gamma = uniform_gamma(&geom, 1.0);
    let ib = match forcing {
        Forcing::First => ImmersedForcing::build(&geom, &cc_bodies(), st, &gamma).unwrap(),
        Forcing::Second => ImmersedForcing::build_second_order(&geom, &cc_bodies(), st, &gamma).unwrap(),
    };
    (geom, u, p, ib)
}

fn cc_fields(n: usize, forcing: Forcing) -> (Arc<FvGeometry>, VectorField, ScalarField, ImmersedForcing) {
    cc_fields_with(n, forcing, 1e-2)
}

/// `e = sqrt(sum_F V |u - u_exact|^2 / sum_F V) / (omega R1)` over the fluid cells; also its radial and azimuthal parts.
fn cc_error(geom: &FvGeometry, ib: &ImmersedForcing, u: &[Vec3]) -> (f64, f64, f64) {
    let (cc, vol) = (geom.mesh.cell_centres(), geom.mesh.cell_volumes());
    let (mut num, mut nr, mut nt, mut den) = (0.0, 0.0, 0.0, 0.0);
    for c in 0..geom.n_cells() {
        if ib.body_of_cell[c].is_some() {
            continue;
        }
        let (dx, dy) = (cc[c][0] - CC_C[0], cc[c][1] - CC_C[1]);
        let r = (dx * dx + dy * dy).sqrt();
        assert!(r > R1 && r < R2, "fluid cell {c} outside the annulus (r = {r})");
        let d = vec3::sub(u[c], cc_exact(cc[c]));
        let dr = (d[0] * dx + d[1] * dy) / r;
        let dt = (-d[0] * dy + d[1] * dx) / r;
        num += vol[c] * vec3::dot(d, d);
        nr += vol[c] * dr * dr;
        nt += vol[c] * dt * dt;
        den += vol[c];
    }
    let w = OMEGA * R1;
    ((num / den).sqrt() / w, (nr / den).sqrt() / w, (nt / den).sqrt() / w)
}

/// The fluid-solid faces of a forcing: the faces of its modification terms (the flux faces of the second-order forcing).
fn fs_faces(ib: &ImmersedForcing) -> Vec<usize> {
    ib.bodies.iter().flat_map(|b| &b.terms).filter_map(|t| t.face).collect()
}

/// The flux measure of ADR 0234: `sqrt(sum_f (phi_f - u_ext(x_f) . S_f)^2 / sum_f |S_f|^2) / (omega R1)` over the fluid-solid
/// faces.
fn cc_flux_error(geom: &FvGeometry, ib: &ImmersedForcing, phi: &[f64]) -> f64 {
    let (sf, cf) = (geom.mesh.face_area_vectors(), geom.mesh.face_centres());
    let (mut num, mut den) = (0.0, 0.0);
    for f in fs_faces(ib) {
        let d = phi[f] - vec3::dot(cc_exact(cf[f]), sf[f]);
        num += d * d;
        den += vec3::dot(sf[f], sf[f]);
    }
    (num / den).sqrt() / (OMEGA * R1)
}

/// Printed only: the largest `|p|` over the solid cells next to the fluid and over the fluid cells.
fn cc_pressure_sizes(ib: &ImmersedForcing, p: &[f64]) -> (f64, f64) {
    let mut ps: f64 = 0.0;
    let solid = ib.solid_mask();
    let pf = (0..p.len())
        .filter(|&c| !solid[c])
        .map(|c| p[c].abs())
        .fold(0.0, f64::max);
    for ff in &ib.flux_faces {
        ps = ps.max(p[ff.solid].abs());
    }
    (ps, pf)
}

struct CcRun {
    e: f64,
    er: f64,
    et: f64,
    eps: f64,
    converged: bool,
    iterations: usize,
}

fn cc_simplec(n: usize, forcing: Forcing) -> CcRun {
    let (geom, u, p, ib) = cc_fields(n, forcing);
    let mut st = SimpleSettings::new(1.0, Coupling::Simplec);
    st.convection = ConvScheme::Central;
    st.momentum_solver.tol_abs = 1e-20;
    st.pressure_solver.tol_abs = 1e-20;
    let mut s = SimpleSolver::new(geom.clone(), u, p, st);
    s.immersed = Some(ib.clone());
    let t0 = Instant::now();
    let (mut iterations, mut converged) = (0, false);
    for it in 1..=20000 {
        let info = s.iterate();
        iterations = it;
        if !info.max_du.is_finite() {
            break;
        }
        if info.max_du <= 1e-10 {
            converged = true;
            break;
        }
    }
    let (e, er, et) = cc_error(&geom, &ib, &s.u.values);
    let eps = cc_flux_error(&geom, &ib, &s.phi);
    let dz = 2.4 / n as f64;
    let (_, m_in) = ib.force_moment(0, &s.u.values, 1.0, CC_C);
    let exact = 4.0 * std::f64::consts::PI * OMEGA * R1 * R1 * R2 * R2 / (R2 * R2 - R1 * R1);
    let (ps, pf) = cc_pressure_sizes(&ib, &s.p.values);
    println!(
        "CC N {n} {forcing:?}: e {e:.5e} (radial {er:.4e}, azimuthal {et:.4e}), eps {eps:.5e}, converged {converged} after \
         {iterations} ({:.1} s); torque inner {:.5} (exact -{exact:.5}); fluid-solid faces {}; max |p| solid-next-to-fluid \
         {ps:.3e}, fluid {pf:.3e}",
        t0.elapsed().as_secs_f64(),
        m_in[2] / dz,
        fs_faces(&ib).len()
    );
    CcRun {
        e,
        er,
        et,
        eps,
        converged,
        iterations,
    }
}

/// ADR 0234 A1 (the acceptance: the check V1 of ADR 0218 with the second-order forcing), S1, C1, V1 and V2. The six runs are
/// made once; every verdict is evaluated and printed before the first assertion.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn a1_s1_c1_v1_v2_circular_couette() {
    let second: Vec<CcRun> = [24, 48, 96].iter().map(|&n| cc_simplec(n, Forcing::Second)).collect();
    let first: Vec<CcRun> = [24, 48, 96].iter().map(|&n| cc_simplec(n, Forcing::First)).collect();
    let order = |r: &[CcRun]| (r[1].e / r[2].e).log2();
    let (o2, o1) = (order(&second), order(&first));
    println!(
        "A1: second e {:.4e} {:.4e} {:.4e}, orders {:.3} {:.3}; C1: first e {:.4e} {:.4e} {:.4e}, orders {:.3} {:.3}",
        second[0].e,
        second[1].e,
        second[2].e,
        (second[0].e / second[1].e).log2(),
        o2,
        first[0].e,
        first[1].e,
        first[2].e,
        (first[0].e / first[1].e).log2(),
        o1
    );
    println!(
        "V1: e_second(96)/e_first(96) {:.4}; V2: eps second {:.4e} {:.4e} {:.4e} (order {:.3}), first {:.4e} {:.4e} {:.4e} \
         (order {:.3}), ratio at 96 {:.3}",
        second[2].e / first[2].e,
        second[0].eps,
        second[1].eps,
        second[2].eps,
        (second[1].eps / second[2].eps).log2(),
        first[0].eps,
        first[1].eps,
        first[2].eps,
        (first[1].eps / first[2].eps).log2(),
        first[2].eps / second[2].eps
    );
    println!(
        "parts at 96: second radial {:.4e} azimuthal {:.4e}; first radial {:.4e} azimuthal {:.4e}",
        second[2].er, second[2].et, first[2].er, first[2].et
    );
    // `a <= b`, false for a NaN
    let le = |a: f64, b: f64| a <= b;
    let mut missed: Vec<String> = Vec::new();
    for (r, n) in second.iter().chain(&first).zip([24, 48, 96, 24, 48, 96]) {
        if !r.converged {
            missed.push(format!("S1: CC run N {n} not converged ({} iterations)", r.iterations));
        }
    }
    if !le(o1, 1.5) || o1 == 1.5 {
        missed.push(format!(
            "C1 (negative control) did not hold: first-order forcing order {o1:.3}"
        ));
    }
    if !le(second[2].e, 0.75 * first[2].e) {
        missed.push(format!(
            "V1: e_second(96) {:.4e} against e_first(96) {:.4e}",
            second[2].e, first[2].e
        ));
    }
    if !le(3.0 * second[2].eps, first[2].eps) {
        missed.push(format!(
            "V2: eps_first(96) {:.4e} against eps_second(96) {:.4e}",
            first[2].eps, second[2].eps
        ));
    }
    if !le(second[2].e, 1e-2) {
        missed.push(format!("A1: e(96) {:.4e}", second[2].e));
    }
    if !le(1.5, o2) {
        missed.push(format!("A1: order {o2:.3}"));
    }
    assert!(missed.is_empty(), "{missed:#?}");
}

/// Printed, no limit (ADR 0234): CC with the second-order forcing on a fourth mesh, N = 192, and the order from 96.
#[test]
#[ignore = "printed only (ADR 0234), no limit; run in release with --ignored"]
fn printed_circular_couette_on_192() {
    let a = cc_simplec(96, Forcing::Second);
    let b = cc_simplec(192, Forcing::Second);
    println!(
        "printed: second e(96) {:.4e}, e(192) {:.4e}, order {:.3}; eps order {:.3}; converged {} {}",
        a.e,
        b.e,
        (a.e / b.e).log2(),
        (a.eps / b.eps).log2(),
        a.converged,
        b.converged
    );
}

/// ADR 0234 V3: the PISO path on CC at N = 48 against SIMPLEC with the same forcing.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn v3_circular_couette_with_piso() {
    let simplec = cc_simplec(48, Forcing::Second);
    assert!(simplec.converged, "S1: CC SIMPLEC 48 not converged");
    let (geom, u, p, ib) = cc_fields(48, Forcing::Second);
    let mut st = PimpleSettings::piso(1.0);
    st.convection = ConvScheme::Central;
    let mut s = PimpleSolver::new(geom.clone(), u, p, st);
    s.immersed = Some(ib.clone());
    let mut change = f64::NAN;
    for _ in 0..400 {
        let before = s.u.values.clone();
        s.step(0.0025);
        assert!(s.diverged.is_none(), "{:?}", s.diverged);
        change =
            s.u.values
                .iter()
                .zip(&before)
                .map(|(a, b)| vec3::norm(vec3::sub(*a, *b)))
                .fold(0.0, f64::max);
    }
    let (e, _, _) = cc_error(&geom, &ib, &s.u.values);
    let eps = cc_flux_error(&geom, &ib, &s.phi);
    println!(
        "V3: PISO e {e:.5e} (SIMPLEC {:.5e}), eps {eps:.5e} (SIMPLEC {:.5e}), last step change {change:.3e}",
        simplec.e, simplec.eps
    );
    assert!(change <= 1e-9, "V3 set-up: not steady ({change:.3e})");
    assert!(e <= 1.2 * simplec.e, "V3: e {e:.4e} against SIMPLEC {:.4e}", simplec.e);
    assert!(
        eps <= 1.5 * simplec.eps,
        "V3: eps {eps:.4e} against SIMPLEC {:.4e}",
        simplec.eps
    );
}

// ---------------------------------------------------------------------------------------------------------------------------
// U1 to U3, B1, N1, R1
// ---------------------------------------------------------------------------------------------------------------------------

fn lcg(seed: &mut u64) -> f64 {
    *seed = seed
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    ((*seed >> 11) as f64) / ((1u64 << 53) as f64)
}

/// `(max, rms)` over the flux faces of `|phi_f/|S_f| - u_ext(x_f) . n_f|` after the reconstruction of the exact field.
fn u1_errors(n: usize) -> (f64, f64) {
    let (geom, _, _, ib) = cc_fields_with(n, Forcing::Second, 1e-9);
    let cc = geom.mesh.cell_centres();
    let hby_a: Vec<Vec3> = (0..geom.n_cells())
        .map(|c| match ib.body_of_cell[c] {
            Some(b) => cc_bodies()[b].velocity_at(cc[c]),
            None => cc_exact(cc[c]),
        })
        .collect();
    let zero_b = vec![[0.0; 3]; geom.mesh.n_faces() - geom.n_internal];
    let mut phi = face_flux(&geom, &hby_a, &zero_b);
    let src = ib.reconstruct_face_flux(&hby_a, &mut phi, true).expect("flux faces");
    assert_eq!(src.len(), geom.n_cells());
    let cf = geom.mesh.face_centres();
    let (mut mx, mut sq) = (0.0f64, 0.0);
    for ff in &ib.flux_faces {
        let d = (phi[ff.face] - vec3::dot(cc_exact(cf[ff.face]), ff.sf)).abs() / ff.area;
        mx = mx.max(d);
        sq += d * d;
    }
    (mx, (sq / ib.flux_faces.len() as f64).sqrt())
}

/// ADR 0234 U1: the reconstruction of the exact Couette field is second order.
#[test]
fn u1_reconstruction_of_the_exact_field() {
    let (m48, r48) = u1_errors(48);
    let (m96, r96) = u1_errors(96);
    println!(
        "U1: max {m48:.4e} {m96:.4e}, rms {r48:.4e} {r96:.4e}, rms ratio {:.3}",
        r48 / r96
    );
    assert!(m96 <= 2e-3, "U1: max at N 96 {m96:.4e}");
    assert!(r48 / r96 >= 3.0, "U1: rms ratio {:.3}", r48 / r96);
}

/// ADR 0234 U2: the net flux of each body, the sum of the sources, the sources of the fluid cells and the other faces.
#[test]
fn u2_net_flux_and_sources() {
    let (geom, _, _, ib) = cc_fields(24, Forcing::Second);
    let mut seed = 0x0309_u64;
    let hby_a: Vec<Vec3> = (0..geom.n_cells())
        .map(|_| [2.0 * lcg(&mut seed) - 1.0, 2.0 * lcg(&mut seed) - 1.0, 0.0])
        .collect();
    let zero_b = vec![[0.0; 3]; geom.mesh.n_faces() - geom.n_internal];
    let before = face_flux(&geom, &hby_a, &zero_b);
    let mut phi = before.clone();
    let src = ib.reconstruct_face_flux(&hby_a, &mut phi, true).expect("flux faces");
    let nb = ib.bodies.len();
    let (mut net, mut area) = (vec![0.0; nb], vec![0.0; nb]);
    let mut is_flux = vec![false; geom.mesh.n_faces()];
    for ff in &ib.flux_faces {
        net[ff.body] += ff.sign * (phi[ff.face] - ff.rigid);
        area[ff.body] += ff.area;
        is_flux[ff.face] = true;
    }
    let total: f64 = src.iter().sum();
    println!(
        "U2: net {net:?}, areas {area:?}, sum of the sources {total:.3e}, flux faces {}",
        ib.flux_faces.len()
    );
    assert!(ib.flux_faces.len() > 100);
    for b in 0..nb {
        assert!(net[b].abs() <= 1e-12 * area[b], "U2: body {b} net {:.3e}", net[b]);
    }
    assert!(total.abs() <= 1e-12 * area.iter().sum::<f64>(), "U2: sum {total:.3e}");
    let solid = ib.solid_mask();
    for c in 0..geom.n_cells() {
        if !solid[c] {
            assert_eq!(src[c].to_bits(), 0.0f64.to_bits(), "U2: source in the fluid cell {c}");
        }
    }
    let mut changed = 0;
    for f in 0..geom.mesh.n_faces() {
        if is_flux[f] {
            changed += usize::from(phi[f].to_bits() != before[f].to_bits());
        } else {
            assert_eq!(phi[f].to_bits(), before[f].to_bits(), "U2: face {f} changed");
        }
    }
    assert!(changed > 100, "U2: only {changed} flux faces changed");
}

/// ADR 0234 U3 and B1: the forcing of `build` has no flux faces and is left alone; `build_second_order` is `build` plus the
/// faces; it refuses the plain (unmodified) forcing.
#[test]
fn u3_b1_default_forcing_and_build_error() {
    let (geom, _, _, first) = cc_fields(24, Forcing::First);
    let (_, _, _, second) = cc_fields(24, Forcing::Second);
    assert!(first.flux_faces.is_empty());
    assert_eq!(second.flux_faces.len(), fs_faces(&second).len());
    assert_eq!(first.coeff, second.coeff);
    assert_eq!(first.source, second.source);
    assert_eq!(first.bodies, second.bodies);
    assert_eq!(first.body_of_cell, second.body_of_cell);
    let mut seed = 7u64;
    let hby_a: Vec<Vec3> = (0..geom.n_cells())
        .map(|_| [lcg(&mut seed), lcg(&mut seed), 0.0])
        .collect();
    let zero_b = vec![[0.0; 3]; geom.mesh.n_faces() - geom.n_internal];
    let before = face_flux(&geom, &hby_a, &zero_b);
    let mut phi = before.clone();
    assert!(first.reconstruct_face_flux(&hby_a, &mut phi, true).is_none());
    assert!(first.reconstruct_face_flux(&hby_a, &mut phi, false).is_none());
    assert!(phi.iter().zip(&before).all(|(a, b)| a.to_bits() == b.to_bits()));
    // B1
    let mut st = ImmersedSettings::new(1e8);
    st.modified = false;
    let err = ImmersedForcing::build_second_order(&geom, &cc_bodies(), st, &uniform_gamma(&geom, 1.0)).unwrap_err();
    println!("B1: {err}");
    assert!(err.contains("modified"), "{err}");
}

/// ADR 0234 N1: the forcing of ADR 0218 gives the states it gave on main bf3376b1 (hashes measured there before the code).
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn n1_default_forcing_is_bit_identical() {
    let (geom, u, p, ib) = cc_fields(24, Forcing::First);
    let mut st = SimpleSettings::new(1.0, Coupling::Simplec);
    st.convection = ConvScheme::Central;
    st.momentum_solver.tol_abs = 1e-20;
    st.pressure_solver.tol_abs = 1e-20;
    let mut s = SimpleSolver::new(geom.clone(), u, p, st);
    s.immersed = Some(ib);
    for _ in 0..40 {
        s.iterate();
    }
    let h1 = state_hash(&s.u.values, &s.p.values);
    let (geom, u, p, ib) = cc_fields(24, Forcing::First);
    let mut st = PimpleSettings::piso(1.0);
    st.convection = ConvScheme::Central;
    let mut s = PimpleSolver::new(geom, u, p, st);
    s.immersed = Some(ib);
    for _ in 0..20 {
        s.step(0.0025);
    }
    let h2 = state_hash(&s.u.values, &s.p.values);
    println!("N1: SIMPLEC {h1:016x}, PISO {h2:016x}");
    assert_eq!(h1, 0x3b49_c4ed_4863_d1b6, "N1: SIMPLEC state hash");
    assert_eq!(h2, 0x9fff_5079_2adb_6d4a, "N1: PISO state hash");
}

/// ADR 0234 R1 (SIMPLE): the gradient options of the pressure equation are refused with the flux faces.
#[test]
#[should_panic(expected = "ADR 0234")]
fn r1_simple_refuses_the_gradient_options() {
    let (geom, u, p, ib) = cc_fields(24, Forcing::Second);
    let mut s = SimpleSolver::new(geom, u, p, SimpleSettings::new(1.0, Coupling::Simplec));
    s.immersed = Some(ib);
    s.gradient_pressure = Some(GradientOptions::default());
    s.iterate();
}

/// ADR 0234 R1 (PIMPLE): the gradient options of the pressure equation are refused with the flux faces.
#[test]
#[should_panic(expected = "ADR 0234")]
fn r1_pimple_refuses_the_gradient_options() {
    let (geom, u, p, ib) = cc_fields(24, Forcing::Second);
    let mut st = PimpleSettings::piso(1.0);
    st.gradient_pressure = Some(GradientOptions::default());
    let mut s = PimpleSolver::new(geom, u, p, st);
    s.immersed = Some(ib);
    s.step(0.0025);
}

// ---------------------------------------------------------------------------------------------------------------------------
// V4: DFG 2D-1 on M1 and M2 (the set-up of ADR 0218) with the second-order forcing
// ---------------------------------------------------------------------------------------------------------------------------

const H: f64 = 0.41;
const NU: f64 = 1e-3;
const DIAM: f64 = 0.1;
const CENTRE: Vec3 = [0.2, 0.2, 0.0];
const DZ: f64 = 0.01;
const K_DFG: f64 = 1e7;
const CD_REF: f64 = 5.579_535_233_84;
const CL_REF: f64 = 0.010_618_948_146;

fn geo_end(n: usize, len: f64, h: f64) -> Vec<f64> {
    let r = expansion_for_end_cell(n, len, h).unwrap();
    axis_coords(n, len, Grading::Expansion(r)).unwrap()
}

fn geo_start(n: usize, len: f64, h: f64) -> Vec<f64> {
    let r = expansion_for_end_cell(n, len, h).unwrap();
    axis_coords(n, len, Grading::Expansion(1.0 / r)).unwrap()
}

fn uniform(a: f64, b: f64, n: usize, centre: f64) -> Vec<f64> {
    let h = (b - a) / n as f64;
    let i0 = ((centre - a) / h).round() as i64;
    (0..=n).map(|i| centre + (i as i64 - i0) as f64 * h).collect()
}

fn join(parts: &[Vec<f64>], offsets: &[f64]) -> Vec<f64> {
    let mut out: Vec<f64> = Vec::new();
    for (p, &o) in parts.iter().zip(offsets) {
        let start = usize::from(!out.is_empty());
        out.extend(p[start..].iter().map(|x| x + o));
    }
    out
}

fn bisect(xs: &[f64]) -> Vec<f64> {
    let mut out = vec![xs[0]];
    for w in xs.windows(2) {
        out.push(0.5 * (w[0] + w[1]));
        out.push(w[1]);
    }
    out
}

/// ADR 0218 set-up: M1 coordinates, bisected `level - 1` times.
fn dfg_family(level: usize) -> (Vec<f64>, Vec<f64>) {
    let h = 0.005;
    let mut xs = join(
        &[
            geo_end(10, 0.12, h),
            uniform(0.12, 0.32, 40, 0.2),
            geo_start(38, 1.88, h),
        ],
        &[0.0, 0.0, 0.32],
    );
    let mut ys = join(
        &[
            geo_end(10, 0.12, h),
            uniform(0.12, 0.28, 32, 0.2),
            geo_start(10, 0.13, h),
        ],
        &[0.0, 0.0, 0.28],
    );
    for _ in 1..level {
        xs = bisect(&xs);
        ys = bisect(&ys);
    }
    (xs, ys)
}

fn inflow_mean(um: f64, y1: f64, y2: f64) -> f64 {
    let prim = |y: f64| H * y * y / 2.0 - y * y * y / 3.0;
    4.0 * um / (H * H) * (prim(y2) - prim(y1)) / (y2 - y1)
}

fn inflow_point(um: f64, y: f64) -> f64 {
    4.0 * um * y * (H - y) / (H * H)
}

fn coefficients(ib: &ImmersedForcing, u: &[Vec3], ubar: f64) -> (f64, f64) {
    let (f, _) = ib.force_moment(0, u, 1.0, CENTRE);
    let q = 0.5 * ubar * ubar * DIAM * DZ;
    (f[0] / q, f[1] / q)
}

struct Steady {
    solver: SimpleSolver,
    xs: Vec<f64>,
    cd: f64,
    cl: f64,
    iterations: usize,
    converged: bool,
}

/// One 2D-1 run (the set-up and the convergence rule of ADR 0218) with the second-order forcing.
fn run_2d1(level: usize, init: Option<&Steady>, cap: usize) -> Steady {
    let (xs, ys) = dfg_family(level);
    let um = 0.3;
    let geom = Arc::new(FvGeometry::new(Arc::new(box_2d_coords(&xs, &ys, DZ).unwrap())).unwrap());
    let mut u = VectorField::from_fn("U", &geom, |c| [inflow_point(um, c[1]), 0.0, 0.0]);
    let cf = geom.mesh.face_centres();
    let prof: Vec<Vec3> = geom
        .mesh
        .patch("xmin")
        .unwrap()
        .faces()
        .map(|f| {
            let y = cf[f][1];
            let j = ys.partition_point(|&v| v < y) - 1;
            [inflow_mean(um, ys[j], ys[j + 1]), 0.0, 0.0]
        })
        .collect();
    u.set_bc(&geom, "xmin", Box::new(FixedValue(Profile::PerFace(prof))))
        .unwrap();
    for w in ["ymin", "ymax"] {
        u.set_bc(&geom, w, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    u.set_bc(&geom, "xmax", Box::new(ZeroGradient)).unwrap();
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    p.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
    let body = ImmersedBody::fixed(
        "cylinder",
        SolidShape::Cylinder {
            centre: CENTRE,
            axis: [0.0, 0.0, 1.0],
            radius: 0.5 * DIAM,
        },
    );
    let ib =
        ImmersedForcing::build_second_order(&geom, &[body], ImmersedSettings::new(K_DFG), &uniform_gamma(&geom, NU))
            .unwrap();
    let (nx, ny) = (xs.len() - 1, ys.len() - 1);
    assert_eq!(nx * ny, [4576, 18304][level - 1], "cell count of M{level}");
    let ubar = 0.2;
    if let Some(c) = init {
        let nxc = c.xs.len() - 1;
        for j in 0..ny {
            for i in 0..nx {
                let pc = i / 2 + nxc * (j / 2);
                u.values[i + nx * j] = c.solver.u.values[pc];
                p.values[i + nx * j] = c.solver.p.values[pc];
            }
        }
    }
    let mut st = SimpleSettings::new(NU, Coupling::Simplec);
    st.convection = ConvScheme::LinearUpwind;
    st.momentum_solver.tol_abs = 1e-20;
    st.pressure_solver.tol_abs = 1e-20;
    let mut s = SimpleSolver::new(geom, u, p, st);
    s.immersed = Some(ib.clone());
    let t0 = Instant::now();
    let mut hist: Vec<(f64, f64)> = vec![coefficients(&ib, &s.u.values, ubar)];
    let (mut iterations, mut converged) = (0, false);
    let (mut cd, mut cl) = (f64::NAN, f64::NAN);
    for it in 1..=cap {
        s.iterate();
        iterations = it;
        if it % 10 == 0 {
            (cd, cl) = coefficients(&ib, &s.u.values, ubar);
            hist.push((cd, cl));
            if !cd.is_finite() {
                break;
            }
            if it >= 200 {
                let (cd0, cl0) = hist[hist.len() - 21];
                if (cd - cd0).abs() <= 1e-7 * cd.abs() && (cl - cl0).abs() <= 1e-8 {
                    converged = true;
                    break;
                }
            }
        }
    }
    println!(
        "V4 M{level} second: converged {converged} after {iterations} iterations ({:.0} s); C_D {cd:.8} ({:+.4} %), C_L \
         {cl:.8e} ({:+.4} %)",
        t0.elapsed().as_secs_f64(),
        100.0 * (cd - CD_REF) / CD_REF,
        100.0 * (cl - CL_REF) / CL_REF
    );
    Steady {
        solver: s,
        xs,
        cd,
        cl,
        iterations,
        converged,
    }
}

/// ADR 0234 V4: DFG 2D-1 on M2 with the second-order forcing.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn v4_two_d_one_on_m2() {
    let m1 = run_2d1(1, None, 4000);
    assert!(
        m1.converged,
        "V4 set-up: M1 not converged ({} iterations)",
        m1.iterations
    );
    let m2 = run_2d1(2, Some(&m1), 4000);
    assert!(
        m2.converged,
        "V4 set-up: M2 not converged ({} iterations)",
        m2.iterations
    );
    let ecd = (m2.cd - CD_REF).abs() / CD_REF;
    let ecl = (m2.cl - CL_REF).abs() / CL_REF;
    assert!(ecd <= 0.05, "V4: C_D {} error {ecd:.4}", m2.cd);
    assert!(ecl <= 0.05, "V4: C_L {} error {ecl:.4}", m2.cl);
}
