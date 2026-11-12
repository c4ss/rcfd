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
use rcfd_fv::{FixedValue, FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::box_2d;
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
