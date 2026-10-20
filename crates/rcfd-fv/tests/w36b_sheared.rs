//! ADR 0081 part X: diagnostics of the SIMPLEC divergence on uniformly sheared meshes from 55 degrees (ADR 0077 finding B-X). Ignored
//! diagnostic runs (one run per registered variant and angle); their output is recorded in the ADR. The registered run classification: 1000
//! iterations with a `SteadyGuard` observing; UNSTABLE iff the guard trips, or the largest cell speed over 10..1000 exceeds 3 U_ref, or the
//! continuity is non-finite, or its maximum over 900..1000 exceeds its value at iteration 10.

use rcfd_core::vec3;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::simple::{BodyForce, Coupling, SimpleSettings, SimpleSolver, SteadyGuard};
use rcfd_fv::{FixedValue, FvGeometry, ScalarField, VectorField, ZeroGradient};
use rcfd_mesh::gen::{box_2d, channel, warp, Grading};
use std::sync::Arc;

const THETA: [f64; 10] = [45.0, 50.0, 52.5, 55.0, 57.5, 60.0, 65.0, 70.0, 75.0, 79.0];

/// Solver settings of a variant.
#[derive(Clone, Copy)]
struct Var {
    coupling: Coupling,
    alpha_u: f64,
    alpha_p: f64,
    correctors: usize,
    compensation: bool,
}

const BASE: Var = Var {
    coupling: Coupling::Simplec,
    alpha_u: 0.9,
    alpha_p: 1.0,
    correctors: 0,
    compensation: false,
};

fn settings(nu: f64, v: Var) -> SimpleSettings {
    let mut st = SimpleSettings::new(nu, v.coupling);
    st.convection = ConvScheme::Upwind;
    st.alpha_u = v.alpha_u;
    st.alpha_p = v.alpha_p;
    st.non_orth_correctors = v.correctors;
    st.simplec_non_orth_compensation = v.compensation;
    st
}

fn sheared(m: rcfd_mesh::Mesh, deg: f64) -> Arc<FvGeometry> {
    let t = deg.to_radians().tan();
    let m = warp(&m, |p| [p[0] + t * p[1], p[1], p[2]]).unwrap();
    Arc::new(FvGeometry::new(Arc::new(m)).unwrap())
}

/// Set-up B (the ADR 0077 S3 channel) at viscosity `nu`; U_ref 1.
fn channel_b(deg: f64, nu: f64, v: Var) -> (SimpleSolver, f64) {
    let geom = sheared(
        channel([40, 10, 1], [4.0, 1.0, 0.1], [Grading::Uniform; 3], true).unwrap(),
        deg,
    );
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    u.set_bc(&geom, "inlet", Box::new(FixedValue::vector([1.0, 0.0, 0.0])))
        .unwrap();
    u.set_bc(&geom, "walls", Box::new(FixedValue::vector([0.0; 3])))
        .unwrap();
    u.set_bc(&geom, "outlet", Box::new(ZeroGradient)).unwrap();
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    p.set_bc(&geom, "outlet", Box::new(FixedValue::scalar(0.0))).unwrap();
    (SimpleSolver::new(geom, u, p, settings(nu, v)), 1.0)
}

/// V6 / V6': x-periodic walled channel, body force 12 nu (exact u = 6 y (1 - y), largest 1.5); U_ref 1.5.
fn periodic_x(deg: f64, nu: f64, v: Var) -> (SimpleSolver, f64) {
    let geom = sheared(box_2d(40, 10, 4.0, 1.0, true, false).unwrap(), deg);
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    for w in ["ymin", "ymax"] {
        u.set_bc(&geom, w, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut s = SimpleSolver::new(geom, u, p, settings(nu, v));
    s.body_force = Some(BodyForce::uniform(&s.geom, [12.0 * nu, 0.0, 0.0]));
    (s, 1.5)
}

/// V7: doubly periodic Kolmogorov box, body force (nu k^2 sin(k y), 0), k = 2 pi (exact u = (sin(k y), 0)); U_ref 1.
fn kolmogorov(deg: f64, nu: f64, v: Var) -> (SimpleSolver, f64) {
    let geom = sheared(box_2d(40, 10, 4.0, 1.0, true, true).unwrap(), deg);
    let u = VectorField::uniform("U", &geom, [0.0; 3]);
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut s = SimpleSolver::new(geom, u, p, settings(nu, v));
    let k = 2.0 * std::f64::consts::PI;
    let cells: Vec<[f64; 3]> = s
        .geom
        .mesh
        .cell_centres()
        .iter()
        .map(|c| [nu * k * k * (k * c[1]).sin(), 0.0, 0.0])
        .collect();
    s.body_force = Some(BodyForce::from_cells(&s.geom, cells));
    (s, 1.0)
}

fn max_speed(s: &SimpleSolver) -> f64 {
    s.u.values
        .iter()
        .map(|u| vec3::norm(*u))
        .fold(0.0f64, |a, x| if x.is_nan() { f64::INFINITY } else { a.max(x) })
}

/// The registered classification; (stable, first iteration with res_u and res_p below 1e-6, summary).
fn classify(mut s: SimpleSolver, u_ref: f64) -> (bool, Option<usize>, String) {
    let mut g = SteadyGuard::new(true);
    let (mut umax, mut c10, mut c_late, mut finite) = (0.0f64, f64::NAN, 0.0f64, true);
    let mut conv = None;
    for it in 1..=1000 {
        let info = s.iterate();
        if let Some(d) = g.check(&s.geom.clone(), &info, &s.u, &s.p.values, &s.phi) {
            return (false, conv, format!("trip {} at {it}", d.kind));
        }
        if it >= 10 {
            umax = umax.max(max_speed(&s));
        }
        if !info.continuity.is_finite() {
            finite = false;
        }
        if it == 10 {
            c10 = info.continuity;
        }
        if it >= 900 {
            c_late = c_late.max(info.continuity);
        }
        if conv.is_none() && info.res_u < 1e-6 && info.res_p < 1e-6 {
            conv = Some(it);
        }
    }
    let stable = umax <= 3.0 * u_ref && finite && c_late <= c10;
    (
        stable,
        conv,
        format!("umax {umax:.2e}, c10 {c10:.1e}, c900+ {c_late:.1e}"),
    )
}

/// One variant over THETA: prints one line per angle and returns theta_c (None = "> 79").
fn scan(name: &str, make: impl Fn(f64) -> (SimpleSolver, f64)) -> Option<f64> {
    scan_on(name, &THETA, make)
}

fn scan_on(name: &str, angles: &[f64], make: impl Fn(f64) -> (SimpleSolver, f64)) -> Option<f64> {
    let mut theta_c = None;
    let mut row = String::new();
    for &deg in angles {
        let (s, u_ref) = make(deg);
        let (stable, conv, why) = classify(s, u_ref);
        row.push_str(&format!(
            "\n    {deg:>5}: {} conv {:?} ({why})",
            if stable { "S" } else { "U" },
            conv
        ));
        if !stable && theta_c.is_none() {
            theta_c = Some(deg);
        }
    }
    println!(
        "X {name}: theta_c = {}{row}",
        theta_c.map_or("> 79".into(), |t| format!("{t}"))
    );
    theta_c
}

#[test]
#[ignore = "diagnostic of ADR 0081 part X (prints the scan)"]
fn part_x_scan() {
    let v = |coupling, alpha_u, alpha_p| Var {
        coupling,
        alpha_u,
        alpha_p,
        ..BASE
    };
    scan("V0 base SIMPLEC 0.9/1.0", |d| channel_b(d, 0.01, BASE));
    for ap in [0.7, 0.5, 0.3] {
        scan(&format!("V1 SIMPLEC 0.9/{ap}"), |d| {
            channel_b(d, 0.01, v(Coupling::Simplec, 0.9, ap))
        });
    }
    for au in [0.8, 0.7, 0.5] {
        scan(&format!("V2 SIMPLEC {au}/1.0"), |d| {
            channel_b(d, 0.01, v(Coupling::Simplec, au, 1.0))
        });
    }
    scan("V3 SIMPLE 0.7/0.3", |d| {
        channel_b(d, 0.01, v(Coupling::Simple, 0.7, 0.3))
    });
    scan("V3 SIMPLEC 0.7/0.3", |d| {
        channel_b(d, 0.01, v(Coupling::Simplec, 0.7, 0.3))
    });
    let corr = |compensation| Var {
        correctors: 2,
        compensation,
        ..BASE
    };
    scan("V4a 2 correctors, no compensation", |d| channel_b(d, 0.01, corr(false)));
    scan("V4b 2 correctors + compensation", |d| channel_b(d, 0.01, corr(true)));
    scan("V5 base at nu 0.5", |d| channel_b(d, 0.5, BASE));
    scan("V6 x-periodic walled, nu 0.01", |d| periodic_x(d, 0.01, BASE));
    scan("V6' x-periodic walled, nu 0.5", |d| periodic_x(d, 0.5, BASE));
    scan("V7 doubly periodic Kolmogorov, nu 0.5", |d| kolmogorov(d, 0.5, BASE));
}

/// Amendment 2: the controls without shear (0, 15, 30 degrees).
#[test]
#[ignore = "diagnostic of ADR 0081 part X amendment 2 (prints the controls)"]
fn part_x_controls_without_shear() {
    let a = [0.0, 15.0, 30.0];
    let v = |coupling, alpha_u, alpha_p| Var {
        coupling,
        alpha_u,
        alpha_p,
        ..BASE
    };
    scan_on("C0 V0 base", &a, |d| channel_b(d, 0.01, BASE));
    for au in [0.8, 0.7, 0.5] {
        scan_on(&format!("C0 V2 SIMPLEC {au}/1.0"), &a, |d| {
            channel_b(d, 0.01, v(Coupling::Simplec, au, 1.0))
        });
    }
    scan_on("C0 V3 SIMPLE 0.7/0.3", &a, |d| {
        channel_b(d, 0.01, v(Coupling::Simple, 0.7, 0.3))
    });
    let v4a = Var { correctors: 2, ..BASE };
    scan_on("C0 V4a 2 correctors, no compensation", &a, |d| channel_b(d, 0.01, v4a));
    scan_on("C0 V5 base at nu 0.5", &a, |d| channel_b(d, 0.5, BASE));
    scan_on("C0 V6 x-periodic walled, nu 0.01", &a, |d| periodic_x(d, 0.01, BASE));
    scan_on("C0 V6' x-periodic walled, nu 0.5", &a, |d| periodic_x(d, 0.5, BASE));
    scan_on("C0 V7 doubly periodic Kolmogorov, nu 0.5", &a, |d| {
        kolmogorov(d, 0.5, BASE)
    });
}

/// V9: the mode of the base iteration about a converged state at 60 degrees.
#[test]
#[ignore = "diagnostic of ADR 0081 part X V9 (prints the mode)"]
fn part_x_v9_mode_at_60_degrees() {
    v9(false);
}

/// V9b (amendment 2): V9 with the converged state perturbed by 1e-6.
#[test]
#[ignore = "diagnostic of ADR 0081 part X V9b (prints the mode)"]
fn part_x_v9b_perturbed_mode_at_60_degrees() {
    v9(true);
}

fn v9(perturb: bool) {
    let deg = 60.0;
    let cands = [
        ("V1 SIMPLEC 0.9/0.3", Coupling::Simplec, 0.9, 0.3),
        ("V2 SIMPLEC 0.5/1.0", Coupling::Simplec, 0.5, 1.0),
        ("V3 SIMPLE 0.7/0.3", Coupling::Simple, 0.7, 0.3),
    ];
    let mut base_state = None;
    for (name, c, au, ap) in cands {
        let var = Var {
            coupling: c,
            alpha_u: au,
            alpha_p: ap,
            ..BASE
        };
        let (stable, _, why) = classify(channel_b(deg, 0.01, var).0, 1.0);
        let mut s = channel_b(deg, 0.01, var).0;
        let mut conv = None;
        for it in 1..=5000 {
            let i = s.iterate();
            if i.res_u <= 1e-8 && i.res_p <= 1e-8 {
                conv = Some(it);
                break;
            }
        }
        println!("V9 candidate {name}: STABLE {stable} ({why}), res <= 1e-8 at {conv:?}");
        if stable && conv.is_some() {
            base_state = Some((name, s));
            break;
        }
    }
    let Some((name, mut s)) = base_state else {
        println!("V9: NOT RUNNABLE (no candidate gives a converged state at 60 degrees)");
        return;
    };
    s.settings = settings(0.01, BASE);
    if perturb {
        for (c, u) in s.u.values.iter_mut().enumerate() {
            u[0] *= 1.0 + 1e-6 * (c as f64).sin();
        }
    }
    let t = deg.to_radians().tan();
    let cc = s.geom.mesh.cell_centres().to_vec();
    let ij: Vec<(usize, usize)> = cc
        .iter()
        .map(|c| {
            let j = ((c[1] / 0.1).floor() as usize).min(9);
            let i = (((c[0] - t * c[1]) / 0.1).floor() as usize).min(39);
            (i, j)
        })
        .collect();
    let mut hist = Vec::new();
    let mut last = None;
    for k in 1..=400 {
        let (u0, p0) = (s.u.values.clone(), s.p.values.clone());
        let i = s.iterate();
        hist.push(i.max_du);
        let du: Vec<f64> =
            s.u.values
                .iter()
                .zip(&u0)
                .map(|(a, b)| vec3::norm(vec3::sub(*a, *b)))
                .collect();
        let dp: Vec<f64> = s.p.values.iter().zip(&p0).map(|(a, b)| a - b).collect();
        if (1e-9..=1e-2).contains(&i.max_du) || last.is_none() {
            last = Some((k, du, dp));
        }
        if !i.max_du.is_finite() || i.max_du > 1.0 {
            break;
        }
    }
    let inwin: Vec<(usize, f64)> = hist
        .iter()
        .enumerate()
        .filter(|(_, &m)| (1e-9..=1e-2).contains(&m))
        .map(|(k, &m)| (k + 1, m))
        .collect();
    let (k0, m0, k1, m1) = if inwin.len() >= 2 {
        (
            inwin[0].0,
            inwin[0].1,
            inwin[inwin.len() - 1].0,
            inwin[inwin.len() - 1].1,
        )
    } else {
        (1, hist[0], hist.len(), hist[hist.len() - 1])
    };
    let rho = (m1 / m0).powf(1.0 / (k1 - k0).max(1) as f64);
    let (kk, du, dp) = last.unwrap();
    let tot: f64 = du.iter().map(|x| x * x).sum();
    let (mut wall, mut ends, mut inner) = (0.0, 0.0, 0.0);
    let (mut chk, mut abs) = (0.0, 0.0);
    for (c, &(i, j)) in ij.iter().enumerate() {
        let e = du[c] * du[c];
        if j == 0 || j == 9 {
            wall += e;
        } else if !(2..=37).contains(&i) {
            ends += e;
        } else {
            inner += e;
        }
        let sign = if (i + j) % 2 == 0 { 1.0 } else { -1.0 };
        chk += sign * dp[c];
        abs += dp[c].abs();
    }
    println!(
        "V9{} (from {name}, base 0.9/1.0 at 60 deg): max_du window {k0}..{k1} ({m0:.3e} -> {m1:.3e}), rho = {rho:.5}; increment at {kk}: wall rows {:.3}, inlet/outlet columns {:.3}, interior {:.3}; pressure checkerboard index {:.3}; max_du at 1, 10, 100: {:.3e} {:.3e} {:.3e}",
        if perturb { "b" } else { "" },
        wall / tot,
        ends / tot,
        inner / tot,
        chk.abs() / abs.max(1e-300),
        hist[0],
        hist.get(9).copied().unwrap_or(f64::NAN),
        hist.get(99).copied().unwrap_or(f64::NAN)
    );
}
