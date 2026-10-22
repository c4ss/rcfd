//! ADR 0106: height-function curvature of the VOF solver (`VofSettings::height_function`). H1 / H2: curvature of an exact
//! circle / ball under refinement (construction only); H3: Laplace jump of a static bubble (ADR 0103 D1 set-up, exact initial
//! fractions). Limits registered in ADR 0106 before any run.

use rcfd_fv::vof::{VofSettings, VofSolver};
use rcfd_fv::{FixedValue, FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::{box_2d, box_mesh, Grading};
use std::sync::Arc;

/// Area of the disc of radius `r` centred at the origin intersected with `[x0, x1] x [y0, y1]`: the clipped chord integrated
/// with the antiderivative of `sqrt(r^2 - x^2)`, split where the clipping changes (exact up to round-off).
fn disc_rect(x0: f64, x1: f64, y0: f64, y1: f64, r: f64) -> f64 {
    let f = |x: f64| {
        let x = x.clamp(-r, r);
        0.5 * (x * (r * r - x * x).max(0.0).sqrt() + r * r * (x / r).asin())
    };
    let (a, b) = (x0.max(-r), x1.min(r));
    if a >= b || y0 >= y1 {
        return 0.0;
    }
    let mut pts = vec![a, b];
    for y in [y0, y1] {
        if y.abs() < r {
            let xb = (r * r - y * y).sqrt();
            pts.extend([-xb, xb].into_iter().filter(|&x| x > a && x < b));
        }
    }
    pts.sort_by(f64::total_cmp);
    let mut area = 0.0;
    for w in pts.windows(2) {
        let (p, q) = (w[0], w[1]);
        let m = 0.5 * (p + q);
        let sm = (r * r - m * m).max(0.0).sqrt();
        if y1.min(sm) <= y0.max(-sm) {
            continue;
        }
        let up = if y1 < sm { y1 * (q - p) } else { f(q) - f(p) };
        let lo = if y0 > -sm { y0 * (q - p) } else { -(f(q) - f(p)) };
        area += up - lo;
    }
    area
}

/// Volume of the ball of radius `r` centred at the origin intersected with a box: `disc_rect` of the slice radius integrated
/// over z, 5-point Gauss-Legendre on 16 sub-intervals between the points where the slice changes shape.
fn ball_box(lo: [f64; 3], hi: [f64; 3], r: f64) -> f64 {
    let (a, b) = (lo[2].max(-r), hi[2].min(r));
    if a >= b {
        return 0.0;
    }
    let mut pts = vec![a, b];
    let mut d = vec![lo[0].abs(), hi[0].abs(), lo[1].abs(), hi[1].abs()];
    for x in [lo[0], hi[0]] {
        for y in [lo[1], hi[1]] {
            d.push(x.hypot(y));
        }
    }
    for di in d {
        if di < r {
            let zb = (r * r - di * di).sqrt();
            pts.extend([-zb, zb].into_iter().filter(|&z| z > a && z < b));
        }
    }
    pts.sort_by(f64::total_cmp);
    const X: [f64; 5] = [
        0.0,
        0.538_469_310_105_683_1,
        -0.538_469_310_105_683_1,
        0.906_179_845_938_664,
        -0.906_179_845_938_664,
    ];
    const W: [f64; 5] = [
        0.568_888_888_888_888_9,
        0.478_628_670_499_366_5,
        0.478_628_670_499_366_5,
        0.236_926_885_056_189_1,
        0.236_926_885_056_189_1,
    ];
    let mut vol = 0.0;
    for w in pts.windows(2) {
        let hs = (w[1] - w[0]) / 16.0;
        for i in 0..16 {
            let mid = w[0] + (i as f64 + 0.5) * hs;
            for (x, wt) in X.iter().zip(W) {
                let z = mid + 0.5 * hs * x;
                let rho = (r * r - z * z).max(0.0).sqrt();
                vol += 0.5 * hs * wt * disc_rect(lo[0], hi[0], lo[1], hi[1], rho);
            }
        }
    }
    vol
}

/// Solver on `mesh` with alpha = 1 outside a circle (2-D) / ball (3-D) of radius `r` at `c`, exact fractions; the phase-volume
/// check of ADR 0106 is asserted.
fn solver(mesh: rcfd_mesh::Mesh, h: f64, three_d: bool, c: [f64; 3], r: f64, st: VofSettings) -> VofSolver {
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let cc = geom.mesh.cell_centres();
    let inside: Vec<f64> = cc
        .iter()
        .map(|p| {
            let lo = [0, 1, 2].map(|d| p[d] - 0.5 * h - c[d]);
            let hi = [0, 1, 2].map(|d| p[d] + 0.5 * h - c[d]);
            if three_d {
                ball_box(lo, hi, r)
            } else {
                disc_rect(lo[0], hi[0], lo[1], hi[1], r)
            }
        })
        .collect();
    let (total, exact, tol) = if three_d {
        (
            inside.iter().sum::<f64>(),
            4.0 / 3.0 * std::f64::consts::PI * r.powi(3),
            1e-8,
        )
    } else {
        (inside.iter().sum::<f64>(), std::f64::consts::PI * r * r, 1e-10)
    };
    assert!((total / exact - 1.0).abs() <= tol, "phase volume {total} vs {exact}");
    let cell = if three_d { h * h * h } else { h * h };
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    let names: &[&str] = if three_d {
        &["xmin", "xmax", "ymin", "ymax", "zmin", "zmax"]
    } else {
        &["xmin", "xmax", "ymin", "ymax"]
    };
    for p in names {
        u.set_bc(&geom, p, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    let p_rgh = ScalarField::uniform("p_rgh", &geom, 0.0);
    let mut alpha = ScalarField::uniform("alpha", &geom, 1.0);
    alpha.values = inside.iter().map(|v| (1.0 - v / cell).clamp(0.0, 1.0)).collect();
    VofSolver::new(geom, u, p_rgh, alpha, st).unwrap()
}

fn settings(height_function: bool) -> VofSettings {
    let mut st = VofSettings::new([1000.0, 100.0], [10.0, 1.0]);
    st.sigma = 24.5;
    st.n_correctors = 3;
    st.amg_pressure = true;
    st.height_function = height_function;
    st
}

/// (rms, max) relative error of `-kappa` against `exact` over the cells with 0.05 < alpha < 0.95.
fn errors(s: &VofSolver, exact: f64) -> (f64, f64) {
    let k = s.curvature();
    let a = &s.transport.alpha.values;
    let e: Vec<f64> = (0..a.len())
        .filter(|&c| a[c] > 0.05 && a[c] < 0.95)
        .map(|c| -k[c] / exact - 1.0)
        .collect();
    let rms = (e.iter().map(|x| x * x).sum::<f64>() / e.len() as f64).sqrt();
    (rms, e.iter().map(|x| x.abs()).fold(0.0, f64::max))
}

fn circle(n: usize, hf: bool) -> VofSolver {
    let mesh = box_2d(n, n, 1.0, 1.0, false, false).unwrap();
    solver(mesh, 1.0 / n as f64, false, [0.51, 0.503, 0.0], 0.25, settings(hf))
}

fn ball(n: usize) -> VofSolver {
    let mesh = box_mesh([n; 3], [1.0; 3], [Grading::Uniform; 3]).unwrap();
    solver(mesh, 1.0 / n as f64, true, [0.51, 0.503, 0.497], 0.25, settings(true))
}

/// H1 (not ignored): circle curvature converges at order >= 1.5 (rms) and >= 1 (max) with no CSF fallback.
#[test]
fn h1_circle_curvature_converges() {
    let grids = [40, 80, 160, 320];
    let mut rms = Vec::new();
    let mut max = Vec::new();
    for n in grids {
        let s = circle(n, true);
        let (r, m) = errors(&s, 4.0);
        let fb = s.curvature_fallbacks();
        let csf = if n <= 160 {
            errors(&circle(n, false), 4.0)
        } else {
            (f64::NAN, f64::NAN)
        };
        println!(
            "H1 {n} x {n}: height function rms {r:.3e}, max {m:.3e}, fallbacks {fb:?}; CSF (n_smooth 2) rms {:.3e}, max {:.3e}",
            csf.0, csf.1
        );
        assert_eq!(fb[1], 0, "H1d {n}: CSF fallback cells");
        rms.push(r);
        max.push(m);
    }
    assert!(rms.windows(2).all(|w| w[1] < w[0]), "H1a rms not decreasing: {rms:?}");
    assert!(rms[3] <= rms[1] / 8.0, "H1b rms order: {rms:?}");
    assert!(max[3] <= max[1] / 4.0, "H1c max order: {max:?}");
}

/// H2 (not ignored): ball curvature converges (order >= 1 over 16 -> 64) with no CSF fallback on 32 and 64.
#[test]
fn h2_ball_curvature_converges() {
    let mut rms = Vec::new();
    for n in [16, 32, 64] {
        let s = ball(n);
        let (r, m) = errors(&s, 8.0);
        let fb = s.curvature_fallbacks();
        println!("H2 {n}^3: rms {r:.3e}, max {m:.3e}, fallbacks {fb:?}");
        if n >= 32 {
            assert_eq!(fb[1], 0, "H2c {n}: CSF fallback cells");
        }
        rms.push(r);
    }
    assert!(rms.windows(2).all(|w| w[1] < w[0]), "H2a rms not decreasing: {rms:?}");
    assert!(rms[2] <= rms[0] / 4.0, "H2b rms order: {rms:?}");
}

/// The ADR 0103 D1 static bubble (Hysing fluids, g = 0, no-slip walls, dt = 0.1/n) with exact initial fractions to `t_end`:
/// (signed Laplace error, max|U| at t_end).
fn static_bubble(n: usize, hf: bool, t_end: f64) -> (f64, f64) {
    let mesh = box_2d(n, n, 1.0, 1.0, false, false).unwrap();
    let dx = 1.0 / n as f64;
    let mut s = solver(mesh, dx, false, [0.5, 0.5, 0.0], 0.25, settings(hf));
    let dt = 0.1 / n as f64;
    let t0 = std::time::Instant::now();
    while s.time < t_end - 1e-9 {
        s.step(dt).unwrap();
    }
    let cc = s.geom.mesh.cell_centres();
    let a = &s.transport.alpha.values;
    let (mut pin, mut nin, mut pout, mut nout) = (0.0, 0, 0.0, 0);
    for c in 0..a.len() {
        let r = (cc[c][0] - 0.5).hypot(cc[c][1] - 0.5);
        if a[c] < 0.01 && r < 0.25 - 3.0 * dx {
            pin += s.p[c];
            nin += 1;
        }
        if a[c] > 0.99 && r > 0.25 + 4.0 * dx {
            pout += s.p[c];
            nout += 1;
        }
    }
    let dp = pin / nin as f64 - pout / nout as f64;
    let err = (dp - 98.0) / 98.0;
    let umax = s.u.values.iter().map(|v| v[0].hypot(v[1])).fold(0.0, f64::max);
    let finite = s.u.values.iter().all(|v| v.iter().all(|x| x.is_finite()));
    println!(
        "H3 {n} x {n}, height function {hf}, t {t_end}: dp {dp:.4} vs 98 (signed rel. error {err:.5}), max|U| {umax:.3e}, fallbacks {:?}, finite {finite}, {:.1} s",
        s.curvature_fallbacks(),
        t0.elapsed().as_secs_f64()
    );
    (err, umax)
}

/// H3: Laplace jump of the static bubble approaches sigma / R (noise floor 0.002).
#[test]
#[ignore = "ADR 0106 H3, heavy (160 x 160 to t = 1), run once"]
fn h3_static_bubble_laplace_jump_converges() {
    for n in [40, 80] {
        static_bubble(n, false, 1.0);
    }
    let e: Vec<f64> = [40, 80, 160]
        .into_iter()
        .map(|n| static_bubble(n, true, 1.0).0.abs())
        .collect();
    println!("H3 |e| (40, 80, 160): {e:?}");
    for w in e.windows(2) {
        assert!(w[1] < w[0] || w[1] <= 0.002, "H3a: {e:?}");
    }
    assert!(e[2] <= 0.5 * e[0] || e[2] <= 0.002, "H3b: {e:?}");
}

/// S1 smoke (ADR 0106 rule d): construction on the smallest grids, H3 on 40 x 40 to t = 0.05.
#[test]
#[ignore = "ADR 0106 smoke"]
fn s1_smoke() {
    let s = circle(20, true);
    println!(
        "S1 circle 20: {:?}, fallbacks {:?}",
        errors(&s, 4.0),
        s.curvature_fallbacks()
    );
    let s = ball(8);
    println!(
        "S1 ball 8: {:?}, fallbacks {:?}",
        errors(&s, 8.0),
        s.curvature_fallbacks()
    );
    static_bubble(40, true, 0.05);
}
