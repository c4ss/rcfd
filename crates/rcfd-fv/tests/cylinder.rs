//! Laminar vortex shedding behind a circular cylinder at Re = 100 (F-029).
//!
//! Acceptance: the Strouhal number `St = f D / U` is within 5 percent of the
//! accepted value for the unbounded 2D flow. Reference: St = 0.165 at Re = 100
//! (Williamson, Annu. Rev. Fluid Mech. 28 (1996), as tabulated in arXiv:2303.09262;
//! converged 2D computations give 0.164: Kravchenko et al., Posdziech and
//! Grundmann, Sivakumar et al. in the same table). The earlier attributions to
//! Williamson 1989 / Henderson 1995 could not be confirmed online. See
//! docs/reference-data.md. The long run is `#[ignore]`d
//! (`cargo test --release -- --ignored cylinder`).

use rcfd_fv::convection::ConvScheme;
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::{FixedValue, Freestream, FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::{cylinder_ogrid, Grading};
use std::sync::Arc;

const ST_REF: f64 = 0.165;

fn setup(n_theta: usize, n_r: usize, r_out: f64, ratio: f64, nu: f64) -> PimpleSolver {
    let mesh = Arc::new(cylinder_ogrid(n_theta, n_r, 0.5, r_out, Grading::Expansion(ratio), 0.1).unwrap());
    let geom = Arc::new(FvGeometry::new(mesh).unwrap());
    let uinf = [1.0, 0.0, 0.0];
    let mut u = VectorField::from_fn("U", &geom, |c| {
        // small asymmetric disturbance in the near wake triggers the instability
        let blob = 0.1 * (-((c[0] - 2.0).powi(2) + (c[1] - 0.3).powi(2))).exp();
        [1.0, blob, 0.0]
    });
    u.set_bc(&geom, "cylinder", Box::new(FixedValue::vector([0.0; 3])))
        .unwrap();
    u.set_bc(
        &geom,
        "farfield",
        Box::new(Freestream {
            u_ref: uinf,
            value: uinf,
            fixed_on_inflow: true,
        }),
    )
    .unwrap();
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    p.set_bc(
        &geom,
        "farfield",
        Box::new(Freestream {
            u_ref: uinf,
            value: [0.0; 3],
            fixed_on_inflow: false,
        }),
    )
    .unwrap();
    let mut st = PimpleSettings::piso(nu);
    st.convection = ConvScheme::LinearUpwind;
    st.n_outer = 2;
    st.amg_pressure = true;
    st.alpha_u = 0.9;
    st.alpha_p = 0.9;
    PimpleSolver::new(geom, u, p, st)
}

/// Index of the cell nearest to `x`.
fn nearest(s: &PimpleSolver, x: [f64; 3]) -> usize {
    let c = s.geom.mesh.cell_centres();
    (0..c.len())
        .min_by(|&a, &b| {
            let d = |i: usize| (c[i][0] - x[0]).powi(2) + (c[i][1] - x[1]).powi(2);
            d(a).total_cmp(&d(b))
        })
        .unwrap()
}

/// Frequency from upward zero crossings (linear interpolation) of `v(t)`,
/// using the last `n_use` crossings.
fn frequency(t: &[f64], v: &[f64], n_use: usize) -> Option<f64> {
    let mean = v.iter().sum::<f64>() / v.len() as f64;
    let mut cross = Vec::new();
    for i in 1..v.len() {
        if v[i - 1] - mean < 0.0 && v[i] - mean >= 0.0 {
            let w = (mean - v[i - 1]) / (v[i] - v[i - 1]);
            cross.push(t[i - 1] + w * (t[i] - t[i - 1]));
        }
    }
    if cross.len() < n_use + 1 {
        return None;
    }
    let c = &cross[cross.len() - n_use - 1..];
    Some(n_use as f64 / (c[n_use] - c[0]))
}

#[test]
fn zero_crossing_frequency_is_exact_for_a_sine() {
    let f = 0.2;
    let t: Vec<f64> = (0..4000).map(|i| i as f64 * 0.05).collect();
    let v: Vec<f64> = t
        .iter()
        .map(|&t| (2.0 * std::f64::consts::PI * f * t + 0.3).sin())
        .collect();
    assert!((frequency(&t, &v, 10).unwrap() - f).abs() < 1e-4);
}

#[test]
fn cylinder_smoke_runs_with_conservative_flux() {
    let mut s = setup(32, 16, 10.0, 8.0, 1.0 / 100.0);
    for _ in 0..10 {
        let i = s.step(0.05);
        assert!(i.continuity < 1e-5, "{i:?}");
    }
    assert!(s.u.values.iter().all(|v| v[0].is_finite() && v[1].is_finite()));
}

#[test]
#[ignore = "heavy: cargo test --release -- --ignored cylinder"]
fn cylinder_re100_strouhal_within_five_percent() {
    let mut s = setup(128, 100, 20.0, 150.0, 1.0 / 100.0);
    let probe = nearest(&s, [3.0, 0.0, 0.05]);
    let (mut t, mut v) = (Vec::new(), Vec::new());
    let dt = 0.05;
    while s.time < 160.0 {
        let info = s.step(dt);
        if s.history.len() % 100 == 0 {
            println!(
                "t = {:.1}  Co = {:.2}  continuity = {:.1e}  v_probe = {:.4}  p_iters = {}",
                s.time, info.courant, info.continuity, s.u.values[probe][1], info.p_iters
            );
        }
        if s.time > 60.0 {
            t.push(s.time);
            v.push(s.u.values[probe][1]);
        }
    }
    let st = frequency(&t, &v, 8).expect("no sustained shedding detected");
    println!("Strouhal number {st:.4} (reference {ST_REF})");
    assert!((st - ST_REF).abs() / ST_REF < 0.05, "St = {st}");
}
