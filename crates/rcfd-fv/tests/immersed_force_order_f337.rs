//! F-337 (ADR 0239): the force and moment of the fluid on an immersed solid, second order.
//!
//! The diagnostic of ADR 0239 (the measurements that decide its hypotheses H1 to H5 for the first-order torque of
//! `ImmersedForcing::force_moment`) and the registered checks. The set-up helpers are copies of those of
//! `immersed_second_order_f309.rs` and `immersed_solid_f219.rs` (those files are not edited). The heavy tests run in release only.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::diffusion::uniform_gamma;
use rcfd_fv::immersed::{ImmersedBody, ImmersedForcing, ImmersedSettings, SolidShape};
use rcfd_fv::simple::{Coupling, SimpleSettings, SimpleSolver};
use rcfd_fv::{FixedValue, FvGeometry, Profile, ScalarField, VectorField, ZeroGradient};
use rcfd_mesh::gen::{axis_coords, box_2d, box_2d_coords, expansion_for_end_cell, Grading};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

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

/// The closed-form torque per unit depth on the inner cylinder, `4 pi rho nu omega R1^2 R2^2/(R2^2 - R1^2)`; the moment of the
/// fluid on the cylinder is its negative.
fn cc_torque() -> f64 {
    4.0 * std::f64::consts::PI * OMEGA * R1 * R1 * R2 * R2 / (R2 * R2 - R1 * R1)
}

/// A converged CC run (the SIMPLEC set-up of ADR 0218 and ADR 0234).
struct CcState {
    n: usize,
    geom: Arc<FvGeometry>,
    ib: ImmersedForcing,
    u: Vec<Vec3>,
    p: Vec<f64>,
    phi: Vec<f64>,
    converged: bool,
    iterations: usize,
}

fn cc_solve(n: usize, forcing: Forcing) -> CcState {
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
    println!(
        "CC N {n} {forcing:?}: converged {converged} after {iterations} ({:.1} s)",
        t0.elapsed().as_secs_f64()
    );
    CcState {
        n,
        geom,
        ib,
        u: s.u.values.clone(),
        p: s.p.values.clone(),
        phi: s.phi.clone(),
        converged,
        iterations,
    }
}

impl CcState {
    fn dz(&self) -> f64 {
        2.4 / self.n as f64
    }

    /// The z moment per unit depth of `force_moment` on the inner body about the axis.
    fn moment_old(&self) -> f64 {
        self.ib.force_moment(0, &self.u, 1.0, CC_C).1[2] / self.dz()
    }

    /// The z moment per unit depth of the terms of the inner body, each applied at the centre of its cell (computed here from
    /// the public terms; not a library call).
    fn moment_cell_arms(&self) -> f64 {
        let (cc, vol) = (self.geom.mesh.cell_centres(), self.geom.mesh.cell_volumes());
        let mut m = vec3::ZERO;
        for t in &self.ib.bodies[0].terms {
            let df: Vec3 = [0, 1, 2].map(|k| vol[t.cell] * (t.coeff * self.u[t.cell][k] - t.source[k]));
            m = vec3::add(m, vec3::cross(vec3::sub(cc[t.cell], CC_C), df));
        }
        m[2] / self.dz()
    }
}

/// ADR 0239, the diagnostic: the measurements of the hypotheses H1 to H5. Printed; the verdicts are computed and printed with the
/// registered deciding numbers, nothing is asserted except the convergence of the runs.
#[test]
#[ignore = "diagnostic of ADR 0239 (H1 to H5), printed; run in release with --ignored"]
fn diag_first_order_torque() {
    let t = cc_torque();
    for n in [48, 96] {
        let s = cc_solve(n, Forcing::Second);
        assert!(s.converged, "CC N {n} not converged ({} iterations)", s.iterations);
        let (cc, cf) = (s.geom.mesh.cell_centres(), s.geom.mesh.face_centres());
        let dz = s.dz();
        let body = &cc_bodies()[0];
        let m_fm = s.moment_old();
        let de = (m_fm + t).abs();
        // H1
        let m_cc = s.moment_cell_arms();
        // H2 to H5 over the fluid-solid faces of the inner body
        let theta: HashMap<usize, f64> = s.ib.bodies[0]
            .terms
            .iter()
            .filter_map(|tm| tm.face.map(|f| (f, tm.theta)))
            .collect();
        let (mut tw, mut tw_exact, mut tp, mut q, mut tq) = (0.0, 0.0, 0.0, 0.0, 0.0);
        let mut faces = 0;
        for ff in s.ib.flux_faces.iter().filter(|ff| ff.body == 0) {
            faces += 1;
            let (c, sc) = (ff.fluid, ff.solid);
            let d = vec3::sub(cc[sc], cc[c]);
            let dist = vec3::norm(d);
            let th = theta[&ff.face];
            let te = th.max(1e-2);
            let xg = vec3::add(cc[c], vec3::scale(d, th));
            let kf = ff.area / dist; // nu = 1
            let arm = vec3::sub(xg, CC_C);
            // H2: the one-sided wall difference at the crossing, solved and exact field
            let v: Vec3 = vec3::sub(s.u[c], body.velocity_at(cc[c]));
            let ve: Vec3 = vec3::sub(cc_exact(cc[c]), body.velocity_at(cc[c]));
            tw += vec3::cross(arm, vec3::scale(v, kf / te))[2];
            tw_exact += vec3::cross(arm, vec3::scale(ve, kf / te))[2];
            // H3: the face pressure of the solver (uniform mesh: the mean of the two cells), acting on the solid side
            let pf = 0.5 * (s.p[c] + s.p[sc]);
            tp += vec3::cross(vec3::sub(cf[ff.face], CC_C), vec3::scale(ff.sf, ff.sign * pf))[2];
            // H4: the exact traction at the crossing, -mu A dv/dn along the segment from the fluid to the solid centre
            let e = vec3::scale(d, 1.0 / dist);
            let eps = 1e-5;
            let vex = |x: Vec3| vec3::sub(cc_exact(x), body.velocity_at(x));
            let dv = vec3::scale(
                vec3::sub(
                    vex(vec3::add(xg, vec3::scale(e, eps))),
                    vex(vec3::sub(xg, vec3::scale(e, eps))),
                ),
                0.5 / eps,
            );
            q += vec3::cross(arm, vec3::scale(dv, -ff.area))[2];
            // H5: the momentum removed with the mass source of the solid cell
            let qs = -ff.sign * (s.phi[ff.face] - ff.rigid);
            tq += vec3::cross(vec3::sub(cc[sc], CC_C), vec3::scale(body.velocity_at(cc[sc]), qs))[2];
        }
        let (tw, tw_exact, tp, q, tq) = (tw / dz, tw_exact / dz, tp / dz, q / dz, tq / dz);
        let (f_old, _) = s.ib.force_moment(0, &s.u, 1.0, CC_C);
        println!(
            "DIAG N {n}: exact {:.5}; force_moment {m_fm:.5} (dE {de:.4e}, {:.3} %); {faces} fluid-solid faces; force \
             ({:.3e}, {:.3e})",
            -t,
            100.0 * de / t,
            f_old[0] / dz,
            f_old[1] / dz
        );
        println!(
            "DIAG N {n} H1: M_cc {m_cc:.5}, error {:.4e} ({:.3} %), dE/3 {:.4e}: holds {}",
            (m_cc + t).abs(),
            100.0 * (m_cc + t).abs() / t,
            de / 3.0,
            (m_cc + t).abs() <= de / 3.0
        );
        println!(
            "DIAG N {n} H2: T_w solved {tw:.5}, error {:.4e} ({:.3} %), dE/2 {:.4e}: holds {}; exact field {tw_exact:.5}, \
             error {:.4e} ({:.3} %)",
            (tw + t).abs(),
            100.0 * (tw + t).abs() / t,
            de / 2.0,
            (tw + t).abs() >= de / 2.0,
            (tw_exact + t).abs(),
            100.0 * (tw_exact + t).abs() / t
        );
        println!(
            "DIAG N {n} H3: T_p {tp:.4e} ({:.3} % of T): holds {}",
            100.0 * tp.abs() / t,
            tp.abs() >= de / 2.0
        );
        println!(
            "DIAG N {n} H4: Q {q:.5}, error {:.4e} ({:.3} %): holds {}",
            (q + t).abs(),
            100.0 * (q + t).abs() / t,
            (q + t).abs() >= de / 2.0
        );
        println!(
            "DIAG N {n} H5: T_q {tq:.4e} ({:.3} % of T): holds {}",
            100.0 * tq.abs() / t,
            tq.abs() >= de / 2.0
        );
        // the comparison of H5: the forcing of ADR 0218, which has no flux of ADR 0234 D1
        let s1 = cc_solve(n, Forcing::First);
        assert!(s1.converged, "CC N {n} first not converged");
        let (a, b) = (s1.moment_old(), s1.moment_cell_arms());
        println!(
            "DIAG N {n} forcing of ADR 0218: force_moment {a:.5} ({:.3} %), cell arms {b:.5} ({:.3} %)",
            100.0 * (a + t).abs() / t,
            100.0 * (b + t).abs() / t
        );
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// The registered checks of ADR 0239
// ---------------------------------------------------------------------------------------------------------------------------

impl CcState {
    /// The z moment per unit depth of `force_moment_second_order` on the inner body about the axis.
    fn moment_new(&self) -> f64 {
        self.ib.force_moment_second_order(0, &self.u, 1.0, CC_C).1[2] / self.dz()
    }

    /// ADR 0239 V1: the right-hand side of D1 per unit depth, the moment about the axis of the discrete pressure, viscous and
    /// momentum fluxes and of the transpose part of the stress on the outer faces of the cells that carry the terms of the inner
    /// body, with the means of the two cells as face values (uniform mesh, `nu = rho = 1`).
    fn moment_outer_faces(&self) -> f64 {
        let mesh = &self.geom.mesh;
        let (cc, cf, sf) = (mesh.cell_centres(), mesh.face_centres(), mesh.face_area_vectors());
        let mut in_r = vec![false; self.geom.n_cells()];
        for t in &self.ib.bodies[0].terms {
            in_r[t.cell] = true;
        }
        let solid = self.ib.solid_mask();
        let mut m = vec3::ZERO;
        let mut faces = 0;
        for f in 0..self.geom.n_internal {
            let (o, nb) = (mesh.owner()[f], mesh.neighbour()[f]);
            if in_r[o] == in_r[nb] {
                continue;
            }
            let (c_in, c_out, sg) = if in_r[o] { (o, nb, 1.0) } else { (nb, o, -1.0) };
            assert!(!solid[c_out], "outer face {f}: the cell outside is solid");
            faces += 1;
            let s_out = vec3::scale(sf[f], sg);
            let kf = vec3::norm(s_out) / vec3::norm(vec3::sub(cc[c_out], cc[c_in]));
            let uf = vec3::scale(vec3::add(self.u[c_in], self.u[c_out]), 0.5);
            let pf = 0.5 * (self.p[c_in] + self.p[c_out]);
            let mut g = vec3::scale(vec3::sub(self.u[c_out], self.u[c_in]), kf);
            g = vec3::sub(g, vec3::scale(s_out, pf));
            g = vec3::sub(g, vec3::scale(uf, sg * self.phi[f]));
            m = vec3::add(m, vec3::cross(vec3::sub(cf[f], CC_C), g));
            m = vec3::sub(m, vec3::cross(s_out, uf));
        }
        assert!(faces > 40, "only {faces} outer faces");
        m[2] / self.dz()
    }
}

/// ADR 0239 A1 (the acceptance: the order of the torque), A1b, S1, C1 and V1. The three runs are made once; every verdict is
/// evaluated and printed before the first assertion.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn a1_a1b_s1_c1_v1_couette_torque() {
    let t = cc_torque();
    let runs: Vec<CcState> = [24, 48, 96].iter().map(|&n| cc_solve(n, Forcing::Second)).collect();
    let new: Vec<f64> = runs.iter().map(|s| (s.moment_new() + t).abs() / t).collect();
    let old: Vec<f64> = runs.iter().map(|s| (s.moment_old() + t).abs() / t).collect();
    for (s, (en, eo)) in runs.iter().zip(new.iter().zip(&old)) {
        let (f, _) = s.ib.force_moment_second_order(0, &s.u, 1.0, CC_C);
        println!(
            "CC N {} second: moment new {:.6} (E {en:.4e}), old {:.6} (E {eo:.4e}), exact {:.6}; force ({:.3e}, {:.3e})",
            s.n,
            s.moment_new(),
            s.moment_old(),
            -t,
            f[0] / s.dz(),
            f[1] / s.dz()
        );
    }
    let (o_new, o_old) = ((new[1] / new[2]).log2(), (old[1] / old[2]).log2());
    println!(
        "A1: order new 48 to 96 {o_new:.3} (24 to 48 {:.3}); C1: order old {o_old:.3} (24 to 48 {:.3}); A1b: E_new(96)/E_old(96) \
         {:.4}",
        (new[0] / new[1]).log2(),
        (old[0] / old[1]).log2(),
        new[2] / old[2]
    );
    let (m_new, m_out) = (runs[1].moment_new(), runs[1].moment_outer_faces());
    println!(
        "V1 N 48: M_new {m_new:.6}, M_out {m_out:.6}, difference {:.3e} of T (M_out against exact {:.3e} of T)",
        (m_new - m_out).abs() / t,
        (m_out + t).abs() / t
    );
    let m_out96 = runs[2].moment_outer_faces();
    println!(
        "printed N 96: M_out {m_out96:.6}, difference to M_new {:.3e} of T (against exact {:.3e} of T)",
        (runs[2].moment_new() - m_out96).abs() / t,
        (m_out96 + t).abs() / t
    );
    // printed, no limit: the new evaluation with the forcing of ADR 0218
    let first: Vec<CcState> = [24, 48, 96].iter().map(|&n| cc_solve(n, Forcing::First)).collect();
    for s in &first {
        println!(
            "printed CC N {} forcing of ADR 0218: new {:.6} (E {:.4e}), old {:.6} (E {:.4e})",
            s.n,
            s.moment_new(),
            (s.moment_new() + t).abs() / t,
            s.moment_old(),
            (s.moment_old() + t).abs() / t
        );
    }
    // `a <= b`, false for a NaN
    let le = |a: f64, b: f64| a <= b;
    let mut missed: Vec<String> = Vec::new();
    for s in &runs {
        if !s.converged {
            missed.push(format!(
                "S1: CC run N {} not converged ({} iterations)",
                s.n, s.iterations
            ));
        }
    }
    if !le(o_old, 1.5) || o_old == 1.5 {
        missed.push(format!("C1 (negative control) did not hold: old order {o_old:.3}"));
    }
    if !le(new[2], 0.5 * old[2]) {
        missed.push(format!(
            "A1b: E_new(96) {:.4e} against E_old(96) {:.4e}",
            new[2], old[2]
        ));
    }
    if !le((m_new - m_out).abs(), 2e-3 * t) {
        missed.push(format!("V1: M_new {m_new:.6} against M_out {m_out:.6}"));
    }
    if !le(1.5, o_new) {
        missed.push(format!("A1: order {o_new:.3}"));
    }
    assert!(missed.is_empty(), "{missed:#?}");
}

/// Printed, no limit (ADR 0239): the torque on a fourth mesh, N = 192, and the orders from 96.
#[test]
#[ignore = "printed only (ADR 0239), no limit; run in release with --ignored"]
fn printed_couette_torque_on_192() {
    let t = cc_torque();
    let a = cc_solve(96, Forcing::Second);
    let b = cc_solve(192, Forcing::Second);
    let e = |m: f64| (m + t).abs() / t;
    println!(
        "printed: new E(96) {:.4e}, E(192) {:.4e}, order {:.3}; old E(96) {:.4e}, E(192) {:.4e}, order {:.3}; converged {} {}",
        e(a.moment_new()),
        e(b.moment_new()),
        (e(a.moment_new()) / e(b.moment_new())).log2(),
        e(a.moment_old()),
        e(b.moment_old()),
        (e(a.moment_old()) / e(b.moment_old())).log2(),
        a.converged,
        b.converged
    );
}

fn lcg(seed: &mut u64) -> f64 {
    *seed = seed
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    ((*seed >> 11) as f64) / ((1u64 << 53) as f64)
}

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

/// The pseudo-random field of the probe of ADR 0239.
fn probe_field(n_cells: usize) -> Vec<Vec3> {
    let mut seed = 0x0337_u64;
    (0..n_cells)
        .map(|_| [2.0 * lcg(&mut seed) - 1.0, 2.0 * lcg(&mut seed) - 1.0, 0.0])
        .collect()
}

const X1: Vec3 = [0.3, -0.2, 0.1];
const X2: Vec3 = [-1.1, 0.7, 0.4];

/// The forcings of the unit checks with their meshes: CC N 24 with `build` and `build_second_order`, DFG M1.
fn unit_forcings() -> Vec<(&'static str, Arc<FvGeometry>, ImmersedForcing)> {
    let (g1, _, _, first) = cc_fields(24, Forcing::First);
    let (g2, _, _, second) = cc_fields(24, Forcing::Second);
    let (g3, dfg) = dfg_forcing(1);
    vec![
        ("CC 24 build", g1, first),
        ("CC 24 build_second_order", g2, second),
        ("DFG M1", g3, dfg),
    ]
}

/// ADR 0239 N1: `force_moment` returns the bits it returned on main 1dd5a0e0 (hashes measured there before the code).
#[test]
fn n1_force_moment_is_bit_identical() {
    let mut got = Vec::new();
    for (name, geom, ib) in unit_forcings() {
        let u = probe_field(geom.n_cells());
        for b in 0..ib.bodies.len() {
            let (f, m) = ib.force_moment(b, &u, 1.3, X1);
            let h = fnv(f.iter().chain(m.iter()).map(|x| x.to_bits()));
            println!("N1 {name} body {b}: {h:016x}");
            got.push(h);
        }
    }
    assert_eq!(
        got,
        [
            0x5a21_ad65_f079_8915,
            0x1a36_4daf_defe_88ba,
            0x5a21_ad65_f079_8915,
            0x1a36_4daf_defe_88ba,
            0xead7_c19f_e86c_3d8b
        ]
    );
}

/// ADR 0239 U1 to U3: the force of the new method is that of `force_moment`; its moment is the sum of the terms with the arms
/// of the cell centres; the reference point transfers.
#[test]
fn u1_u2_u3_force_and_cell_arms() {
    for (name, geom, ib) in unit_forcings() {
        let u = probe_field(geom.n_cells());
        let (cc, vol) = (geom.mesh.cell_centres(), geom.mesh.cell_volumes());
        for b in 0..ib.bodies.len() {
            let (f_old, m_old) = ib.force_moment(b, &u, 1.3, X1);
            let (f, m) = ib.force_moment_second_order(b, &u, 1.3, X1);
            // U1
            assert!(
                f.iter().zip(&f_old).all(|(a, b)| a.to_bits() == b.to_bits()),
                "U1 {name} body {b}: force {f:?} against {f_old:?}"
            );
            // U2
            let mut want = vec3::ZERO;
            let mut mods = 0;
            for t in &ib.bodies[b].terms {
                let df: Vec3 = [0, 1, 2].map(|k| 1.3 * vol[t.cell] * (t.coeff * u[t.cell][k] - t.source[k]));
                want = vec3::add(want, vec3::cross(vec3::sub(cc[t.cell], X1), df));
                mods += usize::from(t.face.is_some());
            }
            let d2 = vec3::norm(vec3::sub(m, want)) / vec3::norm(want);
            // U3
            let (f2, m2) = ib.force_moment_second_order(b, &u, 1.3, X2);
            let shift = vec3::cross(vec3::sub(X2, X1), f2);
            let d3 = vec3::norm(vec3::sub(vec3::sub(m, m2), shift)) / vec3::norm(m);
            println!(
                "U1 to U3 {name} body {b}: {} terms ({mods} modification terms); U2 {d2:.3e}; U3 {d3:.3e}; new against old \
                 moment {:.3e}",
                ib.bodies[b].terms.len(),
                vec3::norm(vec3::sub(m, m_old)) / vec3::norm(m)
            );
            assert!(mods > 20, "{name} body {b}: only {mods} modification terms");
            assert!(d2 <= 1e-12, "U2 {name} body {b}: {d2:.3e}");
            assert!(d3 <= 1e-10, "U3 {name} body {b}: {d3:.3e}");
        }
    }
}

/// ADR 0239 U4: without modification terms the two evaluations are the same to the bit.
#[test]
fn u4_plain_forcing_is_unchanged() {
    let geom = Arc::new(FvGeometry::new(Arc::new(box_2d(24, 24, 2.4, 2.4, false, false).unwrap())).unwrap());
    let mut st = ImmersedSettings::new(1e8);
    st.modified = false;
    let ib = ImmersedForcing::build(&geom, &cc_bodies(), st, &uniform_gamma(&geom, 1.0)).unwrap();
    let u = probe_field(geom.n_cells());
    for b in 0..ib.bodies.len() {
        assert!(ib.bodies[b].terms.len() > 20 && ib.bodies[b].terms.iter().all(|t| t.face.is_none()));
        let (f_old, m_old) = ib.force_moment(b, &u, 1.3, X1);
        let (f, m) = ib.force_moment_second_order(b, &u, 1.3, X1);
        let same = f.iter().chain(m.iter()).zip(f_old.iter().chain(m_old.iter()));
        assert!(
            same.clone().all(|(a, b)| a.to_bits() == b.to_bits()),
            "U4 body {b}: {f:?} {m:?} against {f_old:?} {m_old:?}"
        );
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// DFG 2D-1 on M1 and M2 (the set-up of ADR 0218) with the second-order forcing
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

/// The mesh of level `level` of the family of ADR 0218 and the second-order forcing of the cylinder on it.
fn dfg_forcing(level: usize) -> (Arc<FvGeometry>, ImmersedForcing) {
    let (xs, ys) = dfg_family(level);
    let geom = Arc::new(FvGeometry::new(Arc::new(box_2d_coords(&xs, &ys, DZ).unwrap())).unwrap());
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
    (geom, ib)
}

/// `C_D`, `C_L` of ADR 0218 from the new evaluation.
fn coefficients(ib: &ImmersedForcing, u: &[Vec3], ubar: f64) -> (f64, f64) {
    let (f, _) = ib.force_moment_second_order(0, u, 1.0, CENTRE);
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

/// One 2D-1 run (the set-up and the convergence rule of ADR 0218, the second-order forcing as in ADR 0234 V4) with the
/// coefficients from the new evaluation.
fn run_2d1(level: usize, init: Option<&Steady>, cap: usize) -> Steady {
    let (xs, ys) = dfg_family(level);
    let um = 0.3;
    let (geom, ib) = dfg_forcing(level);
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
    let q = 0.5 * ubar * ubar * DIAM * DZ;
    let (f_old, m_old) = ib.force_moment(0, &s.u.values, 1.0, CENTRE);
    let (_, m_new) = ib.force_moment_second_order(0, &s.u.values, 1.0, CENTRE);
    println!(
        "A2 M{level} second: converged {converged} after {iterations} iterations ({:.0} s); new C_D {cd:.8} ({:+.4} %), C_L \
         {cl:.8e} ({:+.4} %); old C_D {:.8}, C_L {:.8e}; moment coefficient M_z/(q D) new {:.6e}, old {:.6e}",
        t0.elapsed().as_secs_f64(),
        100.0 * (cd - CD_REF) / CD_REF,
        100.0 * (cl - CL_REF) / CL_REF,
        f_old[0] / q,
        f_old[1] / q,
        m_new[2] / (q * DIAM),
        m_old[2] / (q * DIAM)
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

/// ADR 0239 A2 (the acceptance: DFG 2D-1 on M2, drag within 1 percent and lift within 3 percent of the FeatFlow values).
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn a2_two_d_one_on_m2() {
    let m1 = run_2d1(1, None, 4000);
    assert!(
        m1.converged,
        "A2 set-up: M1 not converged ({} iterations)",
        m1.iterations
    );
    let m2 = run_2d1(2, Some(&m1), 4000);
    assert!(
        m2.converged,
        "A2 set-up: M2 not converged ({} iterations)",
        m2.iterations
    );
    let ecd = (m2.cd - CD_REF).abs() / CD_REF;
    let ecl = (m2.cl - CL_REF).abs() / CL_REF;
    println!("A2: C_D error {ecd:.5}, C_L error {ecl:.5}");
    assert!(ecd <= 0.01, "A2: C_D {} error {ecd:.4}", m2.cd);
    assert!(ecl <= 0.03, "A2: C_L {} error {ecl:.4}", m2.cl);
}
