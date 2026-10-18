//! Wave 33D (ADR 0071): rotating-frame foundation checks at the fv level. C2b (Taylor-Couette in the rotating frame on a
//! periodic sector, three meshes, observed order), C3 (Ekman layer, 3-D analytic) and C5a (seam mass flows of the rotational
//! pair on an analytic sector). The limits are those registered in the ADR before any of this code ran; a miss is moved
//! unchanged into an ignored `*_known_miss` test.

use rcfd_fv::bc::FixedValue;
use rcfd_fv::simple::{BodyForce, Coupling, SimpleSettings, SimpleSolver};
use rcfd_fv::sources::RotatingFrame;
use rcfd_fv::{FvGeometry, Profile, ScalarField, VectorField};
use rcfd_mesh::gen::{annulus_sector, box_mesh, Grading};
use std::sync::Arc;

const RI: f64 = 0.5;
const RO: f64 = 1.0;
const NU: f64 = 0.1;
const W_I: f64 = 1.0;
const V_I: f64 = 0.5;

/// Absolute-frame Taylor-Couette profile (inner wall W_i, outer wall at rest).
fn tc(r: f64) -> f64 {
    let d = RO * RO - RI * RI;
    -W_I * RI * RI / d * r + W_I * RI * RI * RO * RO / d / r
}

struct SectorRun {
    solver: SimpleSolver,
    converged: bool,
    last: rcfd_fv::simple::IterationInfo,
}

/// 60-degree sector, frame angular velocity `omega_f` (0 = inertial solve without source). The inner wall rotates at W_i in the
/// inertial frame, the outer wall is at rest in the inertial frame; boundary velocities are given in the solved frame.
fn sector_run(n_theta: usize, n_r: usize, omega_f: f64, max_iter: usize) -> SectorRun {
    let mesh = annulus_sector(n_theta, n_r, RI, RO, std::f64::consts::FRAC_PI_3, Grading::Uniform, 0.1).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    let cf = geom.mesh.face_centres();
    // relative wall velocity = absolute wall velocity - omega_f x r
    let wall = |name: &str, w_abs: f64| {
        let w = w_abs - omega_f;
        geom.mesh
            .patch(name)
            .unwrap()
            .faces()
            .map(|f| [-w * cf[f][1], w * cf[f][0], 0.0])
            .collect::<Vec<_>>()
    };
    u.set_bc(
        &geom,
        "inner",
        Box::new(FixedValue(Profile::PerFace(wall("inner", W_I)))),
    )
    .unwrap();
    u.set_bc(
        &geom,
        "outer",
        Box::new(FixedValue(Profile::PerFace(wall("outer", 0.0)))),
    )
    .unwrap();
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut s = SimpleSettings::new(NU, Coupling::Simplec);
    s.alpha_u = 0.8;
    s.alpha_p = 1.0;
    s.convection = rcfd_fv::convection::ConvScheme::Central;
    let mut solver = SimpleSolver::new(geom.clone(), u, p, s);
    let frame = RotatingFrame {
        origin: [0.0; 3],
        axis: [0.0, 0.0, 1.0],
        omega: omega_f,
        centrifugal: true,
    };
    let zone = vec![true; geom.n_cells()];
    let mut last = solver.iterate();
    let mut converged = false;
    for _ in 0..max_iter {
        if omega_f != 0.0 {
            let ub = solver.u.boundary_values(&geom);
            let mut f = BodyForce::uniform(&geom, [0.0; 3]);
            frame.add_force(&geom, &solver.u.values, &ub, &zone, &mut f);
            solver.body_force = Some(f);
            solver.wall_pressure_trapezoid = true;
        }
        last = solver.iterate();
        if last.res_u < 1e-10 && last.continuity < 1e-9 {
            converged = true;
            break;
        }
    }
    SectorRun {
        solver,
        converged,
        last,
    }
}

/// max over cells |u_abs,theta - analytic| / V_i with u_abs = u_rel + omega_f x r.
fn sector_error(run: &SectorRun, omega_f: f64) -> f64 {
    let cc = run.solver.geom.mesh.cell_centres();
    run.solver
        .u
        .values
        .iter()
        .enumerate()
        .map(|(c, v)| {
            let (x, y) = (cc[c][0], cc[c][1]);
            let r = x.hypot(y);
            let ut = (-y * v[0] + x * v[1]) / r + omega_f * r;
            (ut - tc(r)).abs() / V_I
        })
        .fold(0.0, f64::max)
}

fn orders(e: &[f64]) -> Vec<Option<f64>> {
    e.windows(2)
        .map(|w| if w[1] < 1e-7 { None } else { Some((w[0] / w[1]).log2()) })
        .collect()
}

/// Order criterion of the ADR: every refinement >= 1.0 and the mean >= 1.5 (n/a below the round-off floor).
fn order_ok(e: &[f64]) -> bool {
    let o = orders(e);
    let vals: Vec<f64> = o.iter().flatten().copied().collect();
    if vals.len() < o.len() {
        return true;
    }
    vals.iter().all(|&p| p >= 1.0) && vals.iter().sum::<f64>() / vals.len() as f64 >= 1.5
}

#[test]
fn c2b_taylor_couette_sector_in_the_rotating_frame_mesh_sequence() {
    let mut errs = Vec::new();
    for (nt, nr) in [(2, 4), (4, 8), (8, 16)] {
        let run = sector_run(nt, nr, W_I, 4000);
        let e = sector_error(&run, W_I);
        println!(
            "C2b sector {nt}x{nr}: error {e:.3e} V_i, converged {}, last {:?}",
            run.converged, run.last
        );
        assert!(run.converged, "C2b {nt}x{nr} not converged: {:?}", run.last);
        errs.push(e);
    }
    println!("C2b errors {errs:?}, orders {:?}", orders(&errs));
    assert!(order_ok(&errs), "C2b orders {:?}", orders(&errs));
}

/// The registered finest-mesh error limit of C2b (5e-3 V_i): measured 6.76e-3 on the 8 x 16 sector.
#[test]
#[ignore = "known miss: sector Taylor-Couette finest-mesh error 6.76e-3 V_i against the registered 5e-3 (second order, converged)"]
fn c2b_finest_error_known_miss() {
    let run = sector_run(8, 16, W_I, 4000);
    let e = sector_error(&run, W_I);
    assert!(e <= 5e-3, "C2b finest error {e:.3e} (limit 5e-3)");
}

// ---------------------------------------------------------------- C3 Ekman layer

const EK_NU: f64 = 0.01;
const EK_OMEGA: f64 = 1.0;
const EK_UG: f64 = 1.0;

/// Returns (max error / U_g, max |w| / U_g, converged, last).
fn ekman(n: usize, du_criterion: bool) -> (f64, f64, bool, rcfd_fv::simple::IterationInfo) {
    let delta = (EK_NU / EK_OMEGA).sqrt();
    let h = 8.0 * delta;
    let mut mesh = box_mesh([2, n, 2], [0.1, h, 0.1], [Grading::Uniform; 3]).unwrap();
    for (a, b) in [("xmin", "xmax"), ("zmin", "zmax")] {
        mesh.set_periodic(a, b).unwrap();
        mesh.set_periodic(b, a).unwrap();
    }
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::uniform("U", &geom, [EK_UG, 0.0, 0.0]);
    u.set_bc(&geom, "ymin", Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    u.set_bc(&geom, "ymax", Box::new(FixedValue::vector([EK_UG, 0.0, 0.0])))
        .unwrap();
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut s = SimpleSettings::new(EK_NU, Coupling::Simplec);
    s.alpha_u = 0.8;
    s.alpha_p = 1.0;
    s.convection = rcfd_fv::convection::ConvScheme::Central;
    let mut solver = SimpleSolver::new(geom.clone(), u, p, s);
    let frame = RotatingFrame {
        origin: [0.0; 3],
        axis: [0.0, 1.0, 0.0],
        omega: EK_OMEGA,
        centrifugal: false,
    };
    let zone = vec![true; geom.n_cells()];
    let mut last = solver.iterate();
    let mut converged = false;
    let cap = if du_criterion { 30000 } else { 4000 };
    for _ in 0..cap {
        let ub = solver.u.boundary_values(&geom);
        // geostrophic balance: the pressure gradient that balances the Coriolis force of the free stream
        let mut f = BodyForce::uniform(&geom, [0.0, 0.0, -2.0 * EK_OMEGA * EK_UG]);
        frame.add_force(&geom, &solver.u.values, &ub, &zone, &mut f);
        solver.body_force = Some(f);
        solver.wall_pressure_trapezoid = true;
        last = solver.iterate();
        let done = if du_criterion {
            last.max_du < 1e-11 && last.continuity < 1e-9
        } else {
            last.res_u < 1e-10 && last.continuity < 1e-9
        };
        if done {
            converged = true;
            break;
        }
    }
    let cc = geom.mesh.cell_centres();
    let (mut err, mut wmax) = (0.0f64, 0.0f64);
    for (c, v) in solver.u.values.iter().enumerate() {
        let s = cc[c][1] / delta;
        let ue = EK_UG * (1.0 - (-s).exp() * s.cos());
        let we = -EK_UG * (-s).exp() * s.sin();
        err = err.max((v[0] - ue).abs().max((v[2] - we).abs()) / EK_UG);
        wmax = wmax.max(v[2].abs() / EK_UG);
    }
    (err, wmax, converged, last)
}

fn c3_run(du_criterion: bool) -> (Vec<f64>, f64) {
    let mut errs = Vec::new();
    let mut wmax = 0.0;
    for n in [16, 32, 64] {
        let (e, w, conv, last) = ekman(n, du_criterion);
        println!("C3 Ekman N={n}: error {e:.3e} U_g, max|w| {w:.4}, converged {conv}, last {last:?}");
        assert!(conv, "C3 N={n} not converged: {last:?}");
        errs.push(e);
        wmax = w;
    }
    println!("C3 errors {errs:?}, orders {:?}", orders(&errs));
    (errs, wmax)
}

/// Registered set-up (ADR 0071 C3): convergence on the normalised momentum residual 1e-10. DEFECTIVE CRITERION for this flow: the
/// velocity is nearly uniform (free stream U_g), so the residual normalisation stalls at ~6e-3 while the velocity change per
/// iteration is already 7e-7; the corrected test below converges on the velocity change instead.
#[test]
#[ignore = "known miss: residual criterion (normalised res_u 1e-10) not reached in 4000 iterations, normalisation defect; see c3b"]
fn c3_ekman_layer_matches_the_analytic_spiral_known_miss() {
    let (errs, wmax) = c3_run(false);
    assert!(errs[2] <= 2e-3, "C3 finest error {:.3e} (limit 2e-3)", errs[2]);
    assert!(wmax > 0.0);
}

/// The registered finest-mesh error limit of C3 (2e-3 U_g) with the corrected convergence criterion: measured 3.58e-3.
#[test]
#[ignore = "known miss: Ekman finest-mesh error 3.58e-3 U_g against the registered 2e-3 (converged, second-order, spiral amplitude within 0.1 %)"]
fn c3b_ekman_finest_error_known_miss() {
    let (errs, _) = c3_run(true);
    assert!(errs[2] <= 2e-3, "C3 finest error {:.3e} (limit 2e-3)", errs[2]);
}

#[test]
fn c3b_ekman_layer_matches_the_analytic_spiral_velocity_change_criterion() {
    let (errs, wmax) = c3_run(true);
    assert!(order_ok(&errs), "C3 orders {:?}", orders(&errs));
    // analytic maximum of |w|: U_g e^{-pi/4} sin(pi/4) = 0.3224 U_g
    let exact = (-std::f64::consts::FRAC_PI_4).exp() * std::f64::consts::FRAC_PI_4.sin();
    assert!((wmax / exact - 1.0).abs() <= 0.05, "C3 max|w| {wmax} vs {exact}");
}

// ---------------------------------------------------------------- C5a seam

/// (|m_lo + m_hi| / |m_lo|, m_lo, analytic seam flow, congruence residual, converged, last)
fn seam(omega_f: f64) -> (f64, f64, f64, f64, bool, rcfd_fv::simple::IterationInfo) {
    let run = sector_run(8, 16, omega_f, 6000);
    let s = &run.solver;
    let m = |name: &str| -> f64 { s.geom.mesh.patch(name).unwrap().faces().map(|f| s.phi[f]).sum() };
    let (m_lo, m_hi) = (m("theta_min"), m("theta_max"));
    // analytic seam flow of the solved velocity: depth * integral (u_theta - omega r) dr (inflow through theta_min is positive u_theta)
    let (a, b) = (
        -W_I * RI * RI / (RO * RO - RI * RI),
        W_I * RI * RI * RO * RO / (RO * RO - RI * RI),
    );
    let integral = 0.5 * a * (RO * RO - RI * RI) + b * (RO / RI).ln() - 0.5 * omega_f * (RO * RO - RI * RI);
    let exact = 0.1 * integral;
    // congruence of the seam faces: S_hi + R S_lo
    let mesh = &s.geom.mesh;
    let sf = mesh.face_area_vectors();
    let plo = mesh.patch("theta_min").unwrap();
    let rot = plo.rotation.unwrap().matrix();
    let part = s.geom.periodic_partner[mesh.patches().iter().position(|p| p.name == "theta_min").unwrap()]
        .as_ref()
        .unwrap();
    let mut worst: f64 = 0.0;
    for (local, f) in plo.faces().enumerate() {
        let g = part[local];
        let rs = rcfd_fv::field::rotate(&rot, sf[f]);
        let d = [0, 1, 2].map(|k| sf[g][k] + rs[k]);
        let n = (sf[f][0].powi(2) + sf[f][1].powi(2) + sf[f][2].powi(2)).sqrt();
        worst = worst.max((d[0].powi(2) + d[1].powi(2) + d[2].powi(2)).sqrt() / n);
    }
    (
        (m_lo + m_hi).abs() / m_lo.abs(),
        m_lo,
        exact,
        worst,
        run.converged,
        run.last,
    )
}

#[test]
fn c5a_sector_seam_mass_flows_cancel_and_match_the_analytic_flow() {
    let mut bad = Vec::new();
    for omega in [0.0, 1.0, 5.0, 20.0] {
        let (rel, m_lo, exact, cong, conv, last) = seam(omega);
        println!(
            "C5a Omega {omega}: |m_lo+m_hi|/|m_lo| {rel:.3e} (limit 1e-10), m_lo {m_lo:.6e} vs analytic {exact:.6e} ({:+.3} %), congruence {cong:.2e} (limit 1e-12), converged {conv}, continuity {:.2e}",
            100.0 * (m_lo.abs() / exact.abs() - 1.0),
            last.continuity
        );
        if !conv {
            bad.push(format!("Omega {omega}: not converged {last:?}"));
        }
        if rel > 1e-10 {
            bad.push(format!("Omega {omega}: seam cancellation {rel:.3e}"));
        }
        if last.continuity > 1e-9 {
            bad.push(format!("Omega {omega}: continuity {:.3e}", last.continuity));
        }
        if (m_lo.abs() / exact.abs() - 1.0).abs() > 0.02 {
            bad.push(format!("Omega {omega}: seam flow {m_lo} vs {exact}"));
        }
        if cong > 1e-12 {
            bad.push(format!("Omega {omega}: congruence {cong:.3e}"));
        }
    }
    assert!(bad.is_empty(), "{bad:?}");
}
