//! F-333 (ADR 0351): unit checks of the linear acoustic wave solver. U1 conservation of the pressure integral in a closed
//! warped box; U2 the factored least-squares gradient against `gradient::least_squares`; U3 the sponge source against the
//! RK3 amplification; U4 refusals; U5 a pulse leaving through the open boundary; U6 the propagation speed of a pulse; U7
//! the state at rest stays at rest. Limits are the registered ones of ADR 0351.

use rcfd_fv::acoustic_wave::{AcousticWaveSolver, Medium, WaveBc, WaveSponge, WaveSpongeForm};
use rcfd_fv::gradient;
use rcfd_fv::FvGeometry;
use rcfd_mesh::gen::{box_2d, box_mesh, warp, Grading};
use std::f64::consts::PI;
use std::sync::Arc;

const AIR: Medium = Medium { rho0: 1.2, c: 340.0 };

fn warped_box() -> FvGeometry {
    let l = [1.2, 1.0, 0.8];
    let m = box_mesh([12, 10, 8], l, [Grading::Uniform; 3]).unwrap();
    let m = warp(&m, |p| {
        let b = (PI * p[0] / l[0]).sin() * (PI * p[1] / l[1]).sin() * (PI * p[2] / l[2]).sin();
        [
            p[0] + 0.03 * b * (2.0 * PI * p[1]).sin(),
            p[1] + 0.025 * b * (2.0 * PI * p[2] / l[2]).cos(),
            p[2] + 0.02 * b * (PI * p[0]).sin(),
        ]
    })
    .unwrap();
    FvGeometry::new(Arc::new(m)).unwrap()
}

fn all_walls_3d() -> Vec<(&'static str, WaveBc)> {
    ["xmin", "xmax", "ymin", "ymax", "zmin", "zmax"]
        .map(|n| (n, WaveBc::Wall))
        .to_vec()
}

/// U1: closed warped box with hard walls, smooth initial pressure, 200 steps: the integral of p is constant within 1e-12
/// of the integral of |p0|.
#[test]
fn u1_closed_box_conserves_the_pressure_integral() {
    let geom = warped_box();
    let cc = geom.mesh.cell_centres().to_vec();
    let mut s = AcousticWaveSolver::new(geom, AIR, &all_walls_3d()).unwrap();
    for (c, x) in cc.iter().enumerate() {
        s.p[c] = 1.0 + 0.5 * (3.0 * x[0]).sin() * (2.0 * x[1]).cos() + 0.3 * x[2];
    }
    let abs0: f64 =
        s.p.iter()
            .zip(s.geometry().mesh.cell_volumes())
            .map(|(p, v)| p.abs() * v)
            .sum();
    let int0 = s.pressure_integral();
    let dt = s.max_dt(0.48);
    for _ in 0..200 {
        s.step(dt).unwrap();
    }
    let rel = (s.pressure_integral() - int0).abs() / abs0;
    println!(
        "U1: relative change of the pressure integral {rel:.3e}; energy {:.6e}",
        s.energy()
    );
    assert!(rel <= 1e-12, "U1: {rel:e}");
}

/// U2: the solver's gradient equals `gradient::least_squares` within 1e-12 of the largest gradient.
#[test]
fn u2_gradient_equals_least_squares() {
    let geom = warped_box();
    let f = |x: [f64; 3]| (3.0 * x[0]).sin() + x[1] * x[1] * x[2] - 0.7 * x[2];
    let cells: Vec<f64> = geom.mesh.cell_centres().iter().map(|&x| f(x)).collect();
    let bvals: Vec<f64> = (geom.n_internal..geom.mesh.n_faces())
        .map(|i| f(geom.mesh.face_centres()[i]))
        .collect();
    let reference = gradient::least_squares(&geom, &cells, &bvals);
    let s = AcousticWaveSolver::new(geom, AIR, &all_walls_3d()).unwrap();
    let mine = s.gradient(&cells, &bvals);
    let big = reference
        .iter()
        .flat_map(|g| g.iter())
        .fold(0.0f64, |a, b| a.max(b.abs()));
    let diff = reference
        .iter()
        .zip(&mine)
        .flat_map(|(a, b)| (0..3).map(move |k| (a[k] - b[k]).abs()))
        .fold(0.0f64, f64::max);
    println!("U2: largest difference {diff:.3e} against largest gradient {big:.3e}");
    assert!(diff <= 1e-12 * big, "U2: {diff:e}");
}

fn walls_2d() -> Vec<(&'static str, WaveBc)> {
    ["xmin", "xmax", "ymin", "ymax"].map(|n| (n, WaveBc::Wall)).to_vec()
}

/// U3: uniform p0, u = 0, uniform rate in every cell, sigma dt = 0.1, 100 steps: p = p0 G^n within 1e-12 (plain and
/// matched); u stays exactly 0.
#[test]
fn u3_sponge_source_against_the_rk3_amplification() {
    let sigma = 1000.0;
    let dt = 0.1 / sigma;
    let z: f64 = 0.1;
    let g = 1.0 - z + z * z / 2.0 - z * z * z / 6.0;
    for form in [
        WaveSpongeForm::Plain,
        WaveSpongeForm::Matched {
            normal: [1.0, 0.0, 0.0],
        },
    ] {
        let geom = FvGeometry::new(Arc::new(box_2d(8, 8, 0.4, 0.4, false, false).unwrap())).unwrap();
        let n = geom.mesh.n_cells();
        let mut s = AcousticWaveSolver::new(geom, AIR, &walls_2d()).unwrap();
        s.set_sponge(Some(WaveSponge {
            rate: vec![sigma; n],
            form,
        }))
        .unwrap();
        s.p.iter_mut().for_each(|p| *p = 2.5);
        for _ in 0..100 {
            s.step(dt).unwrap();
        }
        let exact = 2.5 * g.powi(100);
        let worst = s.p.iter().map(|p| (p / exact - 1.0).abs()).fold(0.0f64, f64::max);
        let umax = s.u.iter().flat_map(|u| u.iter()).fold(0.0f64, |a, b| a.max(b.abs()));
        println!("U3 {form:?}: worst relative {worst:.3e}, largest |u| {umax:e}");
        assert!(worst <= 1e-12, "U3 {form:?}: {worst:e}");
        assert!(umax == 0.0, "U3 {form:?}: u = {umax:e}");
    }
}

/// U4: refusals.
#[test]
fn u4_refusals() {
    let geom = || FvGeometry::new(Arc::new(box_2d(4, 4, 1.0, 1.0, false, false).unwrap())).unwrap();
    let bad_media = [
        Medium { rho0: 0.0, c: 340.0 },
        Medium { rho0: 1.2, c: -1.0 },
        Medium {
            rho0: f64::NAN,
            c: 340.0,
        },
    ];
    for m in bad_media {
        assert!(AcousticWaveSolver::new(geom(), m, &walls_2d()).is_err(), "{m:?}");
    }
    let mut missing = walls_2d();
    missing.pop();
    assert!(
        AcousticWaveSolver::new(geom(), AIR, &missing).is_err(),
        "a patch without a condition"
    );
    let mut unknown = walls_2d();
    unknown.push(("nowhere", WaveBc::Open));
    assert!(
        AcousticWaveSolver::new(geom(), AIR, &unknown).is_err(),
        "an unknown patch"
    );
    let mut twice = walls_2d();
    twice.push(("xmin", WaveBc::Open));
    assert!(
        AcousticWaveSolver::new(geom(), AIR, &twice).is_err(),
        "a patch named twice"
    );
    let periodic = FvGeometry::new(Arc::new(box_2d(4, 4, 1.0, 1.0, true, false).unwrap())).unwrap();
    assert!(
        AcousticWaveSolver::new(periodic, AIR, &walls_2d()).is_err(),
        "a cyclic patch"
    );
    let mut s = AcousticWaveSolver::new(geom(), AIR, &walls_2d()).unwrap();
    let n = s.p.len();
    for rate in [vec![-1.0; n], vec![f64::INFINITY; n], vec![1.0; n - 1]] {
        assert!(s
            .set_sponge(Some(WaveSponge {
                rate,
                form: WaveSpongeForm::Plain
            }))
            .is_err());
    }
    assert!(s
        .set_sponge(Some(WaveSponge {
            rate: vec![1.0; n],
            form: WaveSpongeForm::Matched { normal: [0.0; 3] }
        }))
        .is_err());
    for dt in [0.0, -1e-6, f64::NAN, f64::INFINITY] {
        assert!(s.step(dt).is_err(), "dt = {dt}");
    }
    assert!(s.step(1e-6).is_ok());
}

/// A tube of 400 x 1 cells with a right-going Gaussian pulse (width 20 cells) at cell 200.
fn tube(end: WaveBc) -> (AcousticWaveSolver, f64) {
    let dx = 0.01;
    let geom = FvGeometry::new(Arc::new(box_2d(400, 1, 400.0 * dx, dx, false, false).unwrap())).unwrap();
    let cc = geom.mesh.cell_centres().to_vec();
    let mut s = AcousticWaveSolver::new(
        geom,
        AIR,
        &[
            ("xmin", WaveBc::Wall),
            ("xmax", end),
            ("ymin", WaveBc::Wall),
            ("ymax", WaveBc::Wall),
        ],
    )
    .unwrap();
    let (x0, w) = (200.0 * dx, 20.0 * dx);
    let z = AIR.rho0 * AIR.c;
    for (c, x) in cc.iter().enumerate() {
        let p = (-((x[0] - x0) / w).powi(2)).exp();
        s.p[c] = p;
        s.u[c] = [p / z, 0.0, 0.0];
    }
    (s, dx)
}

/// U5: the pulse leaves through the open end; afterwards max |p| <= 1e-2 of the peak.
#[test]
fn u5_pulse_leaves_through_the_open_boundary() {
    let (mut s, dx) = tube(WaveBc::Open);
    let dt = 0.25 * dx / AIR.c;
    for _ in 0..1200 {
        s.step(dt).unwrap();
    }
    let left = s.p.iter().fold(0.0f64, |a, b| a.max(b.abs()));
    println!("U5: largest |p| after the pulse left {left:.3e}");
    assert!(left <= 1e-2, "U5: {left:e}");
}

/// U6: between two walls, after t = 100 dx/c the peak is within 1 dx of x0 + c t.
#[test]
fn u6_pulse_speed() {
    let (mut s, dx) = tube(WaveBc::Wall);
    let dt = 0.25 * dx / AIR.c;
    for _ in 0..400 {
        s.step(dt).unwrap();
    }
    let (i, _) =
        s.p.iter()
            .enumerate()
            .fold((0, f64::MIN), |b, (i, &p)| if p > b.1 { (i, p) } else { b });
    let (a, b, c) = (s.p[i - 1], s.p[i], s.p[i + 1]);
    let peak = (i as f64 + 0.5 + 0.5 * (a - c) / (a - 2.0 * b + c)) * dx;
    let expected = 200.0 * dx + AIR.c * s.t;
    println!(
        "U6: peak at {peak:.6} m, expected {expected:.6} m, difference {:.3} dx",
        (peak - expected) / dx
    );
    assert!((peak - expected).abs() <= dx, "U6");
}

/// U7: the state at rest with wall boundaries stays exactly at rest.
#[test]
fn u7_rest_stays_at_rest() {
    let geom = warped_box();
    let mut s = AcousticWaveSolver::new(geom, AIR, &all_walls_3d()).unwrap();
    let dt = s.max_dt(0.48);
    for _ in 0..10 {
        s.step(dt).unwrap();
    }
    assert!(
        s.p.iter().all(|&p| p == 0.0) && s.u.iter().all(|u| u.iter().all(|&v| v == 0.0)),
        "U7"
    );
}
