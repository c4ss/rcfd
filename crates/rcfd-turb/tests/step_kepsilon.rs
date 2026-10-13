//! F-042: turbulent backward-facing step, Driver and Seegmiller (1985) geometry (expansion ratio
//! 1.125, Re_h = U0 h / nu = 37500), k-epsilon with standard wall functions.
//!
//! Reference (D. M. Driver, H. L. Seegmiller, AIAA J. 23 (1985) 163-171): reattachment length
//! x_r / h = 6.26 +- 0.10 (value as recorded in cases/backward_step/case.json). The inlet profile
//! (power-law boundary layer, thickness 1.5 h on both walls, 1 % free-stream turbulence) is an
//! approximation, not the measured profile. Heavy: run in release with `--ignored --nocapture`.

use rcfd_fv::bc::FixedValue;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::simple::{Coupling, SimpleSettings, SimpleSolver};
use rcfd_fv::{FvGeometry, Profile, ScalarField, VectorField, ZeroGradient};
use rcfd_mesh::gen::{backward_step_graded, Grading, StepGrading, StepSpec};
use rcfd_turb::{KEpsilon, KEpsilonVariant};
use std::sync::Arc;

fn env<T: std::str::FromStr>(k: &str, d: T) -> T {
    std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d)
}

/// Last negative-to-positive sign change of the streamwise velocity in the first cell row above
/// the floor downstream of the step, in step heights.
fn reattachment(s: &SimpleSolver, l_in: f64) -> Option<f64> {
    let cc = s.geom.mesh.cell_centres();
    let ymin = cc.iter().map(|c| c[1]).fold(f64::INFINITY, f64::min);
    let mut row: Vec<(f64, f64)> = (0..cc.len())
        .filter(|&c| (cc[c][1] - ymin).abs() < 1e-9 && cc[c][0] > l_in)
        .map(|c| (cc[c][0], s.u.values[c][0]))
        .collect();
    row.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let mut out = None;
    for w in row.windows(2) {
        if w[0].1 < 0.0 && w[1].1 >= 0.0 {
            let t = -w[0].1 / (w[1].1 - w[0].1);
            out = Some(w[0].0 + t * (w[1].0 - w[0].0) - l_in);
        }
    }
    out
}

#[test]
#[ignore = "heavy: k-epsilon backward-facing step, run in release"]
fn driver_seegmiller_step_k_epsilon() {
    let re_h = 37500.0;
    let nu = 1.0 / re_h;
    let (l_in, l_out) = (env("LIN", 5.0), env("LOUT", 40.0));
    let n = [env("NXU", 24), env("NXD", 160), env("NYI", 40), env("NYS", 20)];
    let spec = StepSpec::from_expansion_ratio(1.125, 1.0, l_in, l_out, 0.1, n).unwrap();
    let gr = StepGrading {
        x_up: Grading::Expansion(1.0 / env("RXU", 3.0)),
        x_down: Grading::Expansion(env("RXD", 8.0)),
        y_step: Grading::Symmetric(env("RYS", 2.0)),
        y_in: Grading::Symmetric(env("RYI", 8.0)),
    };
    let geom = Arc::new(FvGeometry::new(Arc::new(backward_step_graded(spec, gr).unwrap())).unwrap());
    let delta = 1.5;
    let (cmu, kappa) = (0.09f64, 0.41f64);
    let u_tau = 0.038f64;
    let pf = |f: usize| -> (f64, f64, f64) {
        // distance to the nearer inlet-channel wall
        let y = geom.mesh.face_centres()[f][1] - spec.step;
        let d = y.min(spec.h_in - y).max(1e-6);
        let u = if d < delta { (d / delta).powf(1.0 / 7.0) } else { 1.0 };
        let k_free = 1.5 * 0.01f64.powi(2);
        let k = if d < delta {
            (u_tau * u_tau / cmu.sqrt() * (1.0 - d / delta).powi(2)).max(k_free)
        } else {
            k_free
        };
        let l = (kappa * d).min(0.09 * delta);
        (u, k, cmu.powf(0.75) * k.powf(1.5) / l)
    };
    let inlet: Vec<usize> = geom.mesh.patch("inlet").unwrap().faces().collect();
    let prof_u: Vec<[f64; 3]> = inlet.iter().map(|&f| [pf(f).0, 0.0, 0.0]).collect();
    let prof_k: Vec<[f64; 3]> = inlet.iter().map(|&f| [pf(f).1, 0.0, 0.0]).collect();
    let prof_e: Vec<[f64; 3]> = inlet.iter().map(|&f| [pf(f).2, 0.0, 0.0]).collect();
    let mut u = VectorField::uniform("U", &geom, [1.0, 0.0, 0.0]);
    u.set_bc(&geom, "inlet", Box::new(FixedValue(Profile::PerFace(prof_u))))
        .unwrap();
    u.set_bc(&geom, "walls", Box::new(FixedValue::vector([0.0; 3])))
        .unwrap();
    u.set_bc(&geom, "outlet", Box::new(ZeroGradient)).unwrap();
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    p.set_bc(&geom, "outlet", Box::new(FixedValue::scalar(0.0))).unwrap();
    let mut st = SimpleSettings::new(nu, Coupling::Simplec);
    st.alpha_u = env("AU", 0.8);
    st.convection = if env::<String>("SCH", "linup".into()) == "up" {
        ConvScheme::Upwind
    } else {
        ConvScheme::LinearUpwind
    };
    let mut flow = SimpleSolver::new(geom.clone(), u, p, st);
    let variant = match env::<String>("VAR", "std".into()).as_str() {
        "rng" => KEpsilonVariant::Rng,
        "real" => KEpsilonVariant::Realizable,
        _ => KEpsilonVariant::Standard,
    };
    let mut model = KEpsilon::new(variant, geom.clone(), nu, &["walls"], 1.5e-4, 1e-5).unwrap();
    model.relax_turb = env("AT", 0.6);
    model
        .k
        .set_bc(&geom, "inlet", Box::new(FixedValue(Profile::PerFace(prof_k))))
        .unwrap();
    model
        .eps
        .set_bc(&geom, "inlet", Box::new(FixedValue(Profile::PerFace(prof_e))))
        .unwrap();
    let nit: usize = env("NIT", 6000);
    let mut change = f64::INFINITY;
    let mut last = None;
    for it in 1..=nit {
        let prev = model.nut.clone();
        change = model.update(&flow.u, &flow.phi);
        if it == nit {
            let cc = geom.mesh.cell_centres();
            let (mut im, mut dm) = (0, 0.0);
            for c in 0..prev.len() {
                let d = (model.nut[c] - prev[c]).abs() / (prev[c] + nu);
                if d > dm {
                    dm = d;
                    im = c;
                }
            }
            println!(
                "max nut change {dm:.2e} at x {:.3} y {:.3} (nut {:.3e}, k {:.3e})",
                cc[im][0], cc[im][1], model.nut[im], model.k.values[im]
            );
        }
        flow.nu_face = Some(model.nu_face());
        let info = flow.iterate();
        let xr = reattachment(&flow, l_in);
        if it % env("EVERY", 250) == 0 || it == 1 {
            println!(
                "it {it:5} res_u {:.2e} res_p {:.2e} du {:.2e} dnut {change:.2e} xr/h {xr:?}",
                info.res_u, info.res_p, info.max_du
            );
        }
        last = Some((info, xr));
        if !info.res_u.is_finite() {
            break;
        }
        if it > 100 && info.max_du < 1e-7 && change < 1e-6 {
            println!("converged at {it}");
            break;
        }
    }
    let (info, xr) = last.unwrap();
    println!(
        "RESULT k-epsilon {variant:?} D&S ER 1.125: du {:.2e} dnut {change:.2e} xr/h {xr:?} (D&S 6.26 +- 0.10)",
        info.max_du
    );
}
