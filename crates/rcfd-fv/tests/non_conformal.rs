//! Non-conformal interface (F-114): two blocks with different cell counts along the interface, joined by
//! `rcfd_mesh::non_conformal::join_non_conformal`, give the Poiseuille channel solution of a conformal block.
//!
//! Checks fixed before running: laminar channel, nu = 0.1, height 1, length 2 (interface at x = 1, perpendicular to the
//! flow), parabolic inlet of mean velocity 1, exact solution `u = 6 y (1 - y)`, `v = 0`, `dp/dx = -12 nu`; second-order
//! linear upwind convection, two non-orthogonal correctors; interface blocks of 20 and 27 cells across the channel
//! (a non-integer cell ratio, 46 interface faces). Reference: conformal single blocks of 20 and of 27 cells across.
//! Error: the volume-weighted L2 norm of `u - u_exact` (and the maximum) at cell centres, relative to the
//! maximum velocity 1.5.
//! * "Within discretisation error": the non-conformal error is at most 1.5 times the larger of the two conformal errors
//!   (L2 and maximum). If the larger conformal L2 error were below 1e-4 (round-off level, a degenerate comparison),
//!   the criterion would be an absolute error below 2e-4.
//! * Patch test: heat conduction `T = 1 + 2 x + 3 y` (Dirichlet on every boundary, no flow, linear-upwind scheme for
//!   its gradient correction) is reproduced to 1e-8 on the joined mesh after sweeping to a normalised
//!   residual of 1e-13 (exact for a scheme with correct non-orthogonal and skewness handling).
//! * Conservation: the sum of the interface face fluxes equals the sum of the inlet fluxes and the outlet fluxes to
//!   1e-12 relative, and the mean pressure gradient within 2 percent of -12 nu.

use rcfd_fv::convection::ConvScheme;
use rcfd_fv::simple::{Coupling, SimpleSettings, SimpleSolver};
use rcfd_fv::{FixedValue, FvGeometry, Profile, ScalarField, VectorField, ZeroGradient};
use rcfd_mesh::gen::{channel, warp, Grading};
use rcfd_mesh::non_conformal::join_non_conformal;
use rcfd_mesh::Mesh;
use std::sync::Arc;

const NU: f64 = 0.1;

fn block(nx: usize, ny: usize, x0: f64, len: f64) -> Mesh {
    let m = channel([nx, ny, 1], [len, 1.0, 0.1], [Grading::Uniform; 3], true).unwrap();
    warp(&m, |p| [p[0] + x0, p[1], p[2]]).unwrap()
}

struct Result_ {
    l2: f64,
    max: f64,
    flux_in: f64,
    flux_out: f64,
    flux_interface: Option<f64>,
    dpdx: f64,
    iterations: usize,
    res: f64,
}

fn solve(mesh: Mesh, interface: Option<Vec<usize>>) -> Result_ {
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    let cf = geom.mesh.face_centres();
    let prof: Vec<[f64; 3]> = geom
        .mesh
        .patch("inlet")
        .unwrap()
        .faces()
        .map(|f| [6.0 * cf[f][1] * (1.0 - cf[f][1]), 0.0, 0.0])
        .collect();
    u.set_bc(&geom, "inlet", Box::new(FixedValue(Profile::PerFace(prof))))
        .unwrap();
    u.set_bc(&geom, "walls", Box::new(FixedValue::vector([0.0; 3])))
        .unwrap();
    u.set_bc(&geom, "outlet", Box::new(ZeroGradient)).unwrap();
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    p.set_bc(&geom, "outlet", Box::new(FixedValue::scalar(0.0))).unwrap();
    let mut st = SimpleSettings::new(NU, Coupling::Simplec);
    st.convection = ConvScheme::LinearUpwind;
    st.non_orth_correctors = 2;
    st.pressure_solver.tol_rel = 1e-13;
    st.pressure_solver.tol_abs = 1e-22;
    let mut s = SimpleSolver::new(geom.clone(), u, p, st);
    let mut res = f64::INFINITY;
    let mut iterations = 0;
    for i in 1..=4000 {
        let info = s.iterate();
        res = info.res_u.max(info.res_p);
        iterations = i;
        if res < 1e-10 {
            break;
        }
    }
    let cc = geom.mesh.cell_centres();
    let vol = geom.mesh.cell_volumes();
    let (mut e2, mut vsum, mut emax) = (0.0, 0.0, 0.0f64);
    for c in 0..cc.len() {
        let ue = 6.0 * cc[c][1] * (1.0 - cc[c][1]);
        let e = (s.u.values[c][0] - ue).abs().max(s.u.values[c][1].abs());
        e2 += vol[c] * e * e;
        vsum += vol[c];
        emax = emax.max(e);
    }
    let patch_flux = |name: &str| -> f64 {
        let pt = geom.mesh.patch(name).unwrap();
        pt.faces().map(|f| s.phi[f]).sum()
    };
    // mean pressure gradient between the inlet-side and outlet-side faces: use the pressure at cell centres in the
    // first and last 10 percent of the channel, fitted by the two means
    let (mut p_in, mut n_in, mut p_out, mut n_out, mut x_in, mut x_out) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    for c in 0..cc.len() {
        if cc[c][0] < 0.5 {
            p_in += s.p.values[c];
            x_in += cc[c][0];
            n_in += 1.0;
        } else if cc[c][0] > 1.5 {
            p_out += s.p.values[c];
            x_out += cc[c][0];
            n_out += 1.0;
        }
    }
    let dpdx = (p_out / n_out - p_in / n_in) / (x_out / n_out - x_in / n_in);
    Result_ {
        l2: (e2 / vsum).sqrt() / 1.5,
        max: emax / 1.5,
        flux_in: -patch_flux("inlet"),
        flux_out: patch_flux("outlet"),
        flux_interface: interface.map(|fs| fs.iter().map(|&f| s.phi[f]).sum()),
        dpdx,
        iterations,
        res,
    }
}

#[test]
fn non_conformal_poiseuille_matches_conformal_blocks() {
    let c20 = solve(block(20, 20, 0.0, 2.0), None);
    let c27 = solve(block(20, 27, 0.0, 2.0), None);
    let (a, b) = (block(10, 20, 0.0, 1.0), block(10, 27, 1.0, 1.0));
    let (m, rep) = join_non_conformal(&a, "outlet", &b, "inlet", 1e-8).unwrap();
    assert_eq!(rep.n_overlap_faces, 46);
    let nc = solve(m, Some(rep.interface_faces.clone()));
    for (name, r) in [
        ("conformal 20", &c20),
        ("conformal 27", &c27),
        ("non-conformal 20|27", &nc),
    ] {
        println!(
            "{name}: L2 {:.3e}, max {:.3e}, flux in {:.12} out {:.12} interface {:?}, dp/dx {:.5} (exact {}), {} its res {:.1e}",
            r.l2,
            r.max,
            r.flux_in,
            r.flux_out,
            r.flux_interface,
            r.dpdx,
            -12.0 * NU,
            r.iterations,
            r.res
        );
        assert!(r.res < 1e-10, "{name} not converged");
    }
    let band_l2 = c20.l2.max(c27.l2);
    let band_max = c20.max.max(c27.max);
    if band_l2 >= 1e-4 {
        assert!(nc.l2 <= 1.5 * band_l2, "L2 {} vs band {}", nc.l2, band_l2);
        assert!(nc.max <= 1.5 * band_max, "max {} vs band {}", nc.max, band_max);
    } else {
        assert!(
            nc.l2 < 2e-4 && nc.max < 2e-4,
            "degenerate comparison: {} {}",
            nc.l2,
            nc.max
        );
    }
    let qi = nc.flux_interface.unwrap();
    assert!(
        (qi - nc.flux_in).abs() < 1e-12 * nc.flux_in.abs(),
        "interface {qi} vs inlet {}",
        nc.flux_in
    );
    assert!(
        (qi - nc.flux_out).abs() < 1e-12 * nc.flux_in.abs(),
        "interface {qi} vs outlet {}",
        nc.flux_out
    );
    assert!((nc.dpdx / (-12.0 * NU) - 1.0).abs() < 0.02, "dp/dx {}", nc.dpdx);
}

#[test]
fn linear_temperature_field_is_reproduced_across_the_interface() {
    use rcfd_fv::energy::EnergySolver;
    let (a, b) = (block(10, 20, 0.0, 1.0), block(10, 27, 1.0, 1.0));
    let (m, rep) = join_non_conformal(&a, "outlet", &b, "inlet", 1e-8).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(m)).unwrap());
    let exact = |p: [f64; 3]| 1.0 + 2.0 * p[0] + 3.0 * p[1];
    let mut t = ScalarField::uniform("T", &geom, 0.0);
    let cf = geom.mesh.face_centres();
    for name in ["inlet", "outlet", "walls"] {
        let vals: Vec<[f64; 3]> = geom
            .mesh
            .patch(name)
            .unwrap()
            .faces()
            .map(|f| [exact(cf[f]), 0.0, 0.0])
            .collect();
        t.set_bc(&geom, name, Box::new(FixedValue(Profile::PerFace(vals))))
            .unwrap();
    }
    let mut e = EnergySolver::new(geom.clone(), t, 1.0);
    e.convection = ConvScheme::LinearUpwind;
    e.solver.tol_rel = 1e-13;
    e.solver.tol_abs = 1e-300;
    e.solver.max_iter = 5000;
    let phi = vec![0.0; geom.mesh.n_faces()];
    // sweeps until the normalised residual is at round-off (the explicit correction converges linearly)
    let mut res = 1.0;
    for _ in 0..3000 {
        res = e.iterate(&phi);
        if res < 1e-13 {
            break;
        }
    }
    assert!(res < 1e-13, "not converged: {res}");
    let cc = geom.mesh.cell_centres();
    let err = (0..cc.len())
        .map(|c| (e.t.values[c] - exact(cc[c])).abs())
        .fold(0.0, f64::max);
    println!(
        "patch test: max |T - T_exact| = {err:.3e}, {} overlap faces",
        rep.n_overlap_faces
    );
    assert!(err < 1e-8, "{err}");
}
