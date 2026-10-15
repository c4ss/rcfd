//! F-072 registered checks D1 to D5, S1 and S2 (ADR 0020, part C). Limits are the registered ones.

use rcfd_core::vec3::Vec3;
use rcfd_fv::gradient::least_squares;
use rcfd_fv::FvGeometry;
use rcfd_mesh::gen::{box_mesh, Grading};
use rcfd_post::derived::*;
use std::sync::Arc;

fn geom(m: rcfd_mesh::mesh::Mesh) -> FvGeometry {
    FvGeometry::new(Arc::new(m)).unwrap()
}

/// Field values at cell centres and at boundary face centres.
fn eval_vec(g: &FvGeometry, f: impl Fn(Vec3) -> Vec3) -> (Vec<Vec3>, Vec<Vec3>) {
    let cc = g.mesh.cell_centres();
    let fc = g.mesh.face_centres();
    (
        cc.iter().map(|&p| f(p)).collect(),
        (g.n_internal..g.mesh.n_faces()).map(|i| f(fc[i])).collect(),
    )
}

fn eval_scalar(g: &FvGeometry, f: impl Fn(Vec3) -> f64) -> (Vec<f64>, Vec<f64>) {
    let cc = g.mesh.cell_centres();
    let fc = g.mesh.face_centres();
    (
        cc.iter().map(|&p| f(p)).collect(),
        (g.n_internal..g.mesh.n_faces()).map(|i| f(fc[i])).collect(),
    )
}

const A: [[f64; 3]; 3] = [[0.3, -1.2, 0.5], [0.9, 0.1, -0.4], [-0.7, 0.6, -0.2]];
const B: Vec3 = [0.1, -0.2, 0.3];

fn linear_u(p: Vec3) -> Vec3 {
    let mut u = B;
    for k in 0..3 {
        for j in 0..3 {
            u[k] += A[k][j] * p[j];
        }
    }
    u
}

fn graded_cube() -> FvGeometry {
    geom(
        box_mesh(
            [10, 8, 6],
            [1.0, 1.0, 1.0],
            [Grading::Expansion(3.0), Grading::Symmetric(2.0), Grading::Uniform],
        )
        .unwrap(),
    )
}

#[test]
fn d1_linear_field_vorticity_and_q() {
    let g = graded_cube();
    let (u, ub) = eval_vec(&g, linear_u);
    let grad = velocity_gradient(&g, &u, &ub);
    let w = vorticity(&grad);
    let q = q_criterion(&grad);
    let (mut ew, mut eq) = (0.0f64, 0.0f64);
    for c in 0..g.n_cells() {
        for (k, e) in [1.0, 1.2, 2.1].iter().enumerate() {
            ew = ew.max((w[c][k] - e).abs());
        }
        eq = eq.max((q[c] - 1.6).abs());
    }
    println!("D1 max vorticity error {ew:e}, max Q error {eq:e}");
    assert!(ew <= 1e-10 && eq <= 1e-10);
}

fn taylor_green(n: usize) -> (f64, f64, f64) {
    let l = 2.0 * std::f64::consts::PI;
    let g = geom(box_mesh([n, n, 1], [l, l, 1.0], [Grading::Uniform; 3]).unwrap());
    let (u, ub) = eval_vec(&g, |p| [p[0].sin() * p[1].cos(), -p[0].cos() * p[1].sin(), 0.0]);
    let grad = velocity_gradient(&g, &u, &ub);
    let w = vorticity(&grad);
    let q = q_criterion(&grad);
    let h = l / n as f64;
    let (mut ew, mut eq) = (0.0f64, 0.0f64);
    for (c, p) in g.mesh.cell_centres().iter().enumerate() {
        let (i, j) = ((p[0] / h) as usize, (p[1] / h) as usize);
        if i < 2 || j < 2 || i + 3 > n || j + 3 > n {
            continue;
        }
        let wz = 2.0 * p[0].sin() * p[1].sin();
        let qe = (p[0].sin() * p[1].sin()).powi(2) - (p[0].cos() * p[1].cos()).powi(2);
        ew = ew.max((w[c][2] - wz).abs());
        ew = ew.max(w[c][0].abs()).max(w[c][1].abs());
        eq = eq.max((q[c] - qe).abs());
    }
    (ew, eq, h)
}

#[test]
fn d2_taylor_green_second_order() {
    let (ew16, eq16, h16) = taylor_green(16);
    let (ew32, eq32, h32) = taylor_green(32);
    println!(
        "D2 N=16: vorticity err {ew16:e} (limit {:e}), Q err {eq16:e} (limit {:e})",
        h16 * h16 / 3.0,
        h16 * h16
    );
    println!(
        "D2 N=32: vorticity err {ew32:e} (limit {:e}), Q err {eq32:e} (limit {:e})",
        h32 * h32 / 3.0,
        h32 * h32
    );
    println!("D2 ratio {}", ew16 / ew32);
    assert!(ew16 <= h16 * h16 / 3.0 && ew32 <= h32 * h32 / 3.0);
    assert!(eq16 <= h16 * h16 && eq32 <= h32 * h32);
    assert!(ew16 / ew32 >= 3.5);
}

const RHO: f64 = 1.2;
const NU: f64 = 1e-3;
const H: f64 = 0.2;
const N: usize = 16;

fn channel() -> FvGeometry {
    geom(box_mesh([8, N, 1], [1.0, H, 0.1], [Grading::Uniform; 3]).unwrap())
}

/// Boundary velocities: given profile at all faces, but the wall patches `ymin`/`ymax` set to the wall values.
fn wall_values(g: &FvGeometry, profile: &dyn Fn(f64) -> f64, upper: f64) -> Vec<Vec3> {
    let fc = g.mesh.face_centres();
    let mut ub: Vec<Vec3> = (g.n_internal..g.mesh.n_faces())
        .map(|i| [profile(fc[i][1]), 0.0, 0.0])
        .collect();
    for (name, v) in [("ymin", 0.0), ("ymax", upper)] {
        for f in g.mesh.patch(name).unwrap().faces() {
            ub[f - g.n_internal] = [v, 0.0, 0.0];
        }
    }
    ub
}

#[test]
fn d3_couette_wall_shear_and_y_plus() {
    let g = channel();
    let uw = 2.0;
    let profile = |y: f64| uw * y / H;
    let (u, _) = eval_vec(&g, |p| [profile(p[1]), 0.0, 0.0]);
    let ub = wall_values(&g, &profile, uw);
    let mut worst = 0.0f64;
    for (patch, sign) in [("ymin", 1.0), ("ymax", -1.0)] {
        let tau = wall_shear(&g, patch, &u, &ub, RHO, NU).unwrap();
        let yp = y_plus(&g, patch, &tau, RHO, NU).unwrap();
        assert_eq!(tau.len(), 8);
        for (t, y) in tau.iter().zip(&yp) {
            worst = worst
                .max((t[0] - sign * 0.012).abs() / 0.012)
                .max(t[1].abs() / 0.012)
                .max(t[2].abs() / 0.012)
                .max((y - 0.625).abs() / 0.625);
        }
    }
    println!("D3 worst relative error {worst:e}");
    assert!(worst <= 1e-12);
}

#[test]
fn d4_poiseuille_wall_shear_estimate() {
    let g = channel();
    let gp = 0.5;
    let profile = |y: f64| gp / (2.0 * NU) * y * (H - y);
    let (u, _) = eval_vec(&g, |p| [profile(p[1]), 0.0, 0.0]);
    let ub = wall_values(&g, &profile, 0.0);
    let dn = H / (2.0 * N as f64);
    let est = RHO * gp * (H - dn) / 2.0;
    let exact = RHO * gp * H / 2.0;
    let tau = wall_shear(&g, "ymin", &u, &ub, RHO, NU).unwrap();
    let (mut e1, mut e2) = (0.0f64, 0.0f64);
    for t in &tau {
        e1 = e1.max((t[0] - est).abs() / est);
        e2 = e2.max(((t[0] - exact) / exact).abs());
    }
    println!(
        "D4 estimate rel err {e1:e}; relative difference to the exact wall shear {e2} (limit {})",
        0.5 / N as f64
    );
    assert!(e1 <= 1e-12);
    assert!(e2 <= 1.0 / (2.0 * N as f64) * (1.0 + 1e-9));
}

#[test]
fn d5_mach_and_total_pressure() {
    let (gamma, r, t, p, v) = (1.4, 287.0, 288.15, 101325.0, 120.0);
    let c = speed_of_sound(gamma, r, t);
    assert!(((c - (gamma * r * t).sqrt()) / c).abs() <= 1e-12);
    let m = mach(&[[v, 0.0, 0.0], [0.0, -v, 0.0]], &[c, c]);
    let m_exact = v / (gamma * r * t).sqrt();
    assert!(((m[0] - m_exact) / m_exact).abs() <= 1e-12 && ((m[1] - m_exact) / m_exact).abs() <= 1e-12);
    let p0 = total_pressure_isentropic(p, m[0], gamma);
    let p0_exact = p * (1.0 + 0.2 * m_exact * m_exact).powf(3.5);
    assert!(((p0 - p0_exact) / p0_exact).abs() <= 1e-12);
    let rho = p / (r * t);
    let pt = total_pressure_incompressible(p, rho, [v, 0.0, 0.0]);
    assert!(((pt - (p + 0.5 * rho * v * v)) / pt).abs() <= 1e-12);
    // low Mach consistency
    let m_low = 0.05;
    let u_low = m_low * c;
    let p0_low = total_pressure_isentropic(p, mach(&[[u_low, 0.0, 0.0]], &[c])[0], gamma);
    let pt_low = total_pressure_incompressible(p, rho, [u_low, 0.0, 0.0]);
    let diff = (p0_low - pt_low).abs() / p;
    println!(
        "D5 low-Mach difference {diff:e}, limit {:e}",
        0.13 * gamma * m_low.powi(4)
    );
    assert!(diff <= 0.13 * gamma * m_low.powi(4));
}

fn phi(p: Vec3) -> f64 {
    2.0 * p[0] - 3.0 * p[1] + 0.5 * p[2] + 1.0
}

#[test]
fn s1_line_sampling_and_csv() {
    let g = graded_cube();
    let (cells, bnd) = eval_scalar(&g, phi);
    let grad = least_squares(&g, &cells, &bnd);
    let (p0, p1) = ([0.05, 0.1, 0.07], [0.95, 0.8, 0.55]);
    let mut set = sample_line(&g, p0, p1, 41).unwrap();
    set.add_field(&g, "phi", &cells, Some(&grad));
    assert_eq!(set.len(), 41);
    assert_eq!(set.n_inside(), 41);
    let len = ((p1[0] - p0[0]).powi(2) + (p1[1] - p0[1]).powi(2) + (p1[2] - p0[2]).powi(2)).sqrt();
    let (mut ev, mut es) = (0.0f64, 0.0f64);
    for i in 0..41 {
        ev = ev.max((set.values[0][i] - phi(set.points[i])).abs());
        es = es.max((set.s[i] - i as f64 / 40.0 * len).abs());
    }
    println!("S1 max value error {ev:e}, max arc-length error {es:e}");
    assert!(ev <= 1e-10 && es <= 1e-12);
    let csv = set.to_csv();
    let mut lines = csv.lines();
    assert_eq!(lines.next().unwrap(), "s,x,y,z,phi,inside");
    let rows: Vec<&str> = lines.collect();
    assert_eq!(rows.len(), 41);
    for (i, row) in rows.iter().enumerate() {
        let f: Vec<&str> = row.split(',').collect();
        assert_eq!(f.len(), 6);
        assert_eq!(f[0].parse::<f64>().unwrap().to_bits(), set.s[i].to_bits());
        for d in 0..3 {
            assert_eq!(f[1 + d].parse::<f64>().unwrap().to_bits(), set.points[i][d].to_bits());
        }
        assert_eq!(f[4].parse::<f64>().unwrap().to_bits(), set.values[0][i].to_bits());
        assert_eq!(f[5], "1");
    }
    // a line that leaves the unit cube
    let mut out = sample_line(&g, [0.5, 0.5, 0.5], [1.5, 0.5, 0.5], 11).unwrap();
    out.add_field(&g, "phi", &cells, Some(&grad));
    let inside: Vec<bool> = out.cells.iter().map(|c| c.is_some()).collect();
    assert_eq!(inside, (0..11).map(|i| i <= 5).collect::<Vec<_>>());
    let csv = out.to_csv();
    let last = csv.lines().last().unwrap();
    assert!(last.ends_with(",,0"), "{last}");
}

#[test]
fn s2_plane_integral() {
    let g = graded_cube();
    let (cells, bnd) = eval_scalar(&g, phi);
    let grad = least_squares(&g, &cells, &bnd);
    let mut set = sample_plane(&g, [0.0, 0.0, 0.5], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], 1.0, 1.0, 20, 20).unwrap();
    set.add_field(&g, "phi", &cells, Some(&grad));
    assert_eq!(set.len(), 400);
    assert_eq!(set.n_inside(), 400);
    let integral = set.plane_integral(0);
    println!(
        "S2 plane integral {integral} (exact 0.75), error {:e}",
        (integral - 0.75).abs()
    );
    assert!((integral - 0.75).abs() <= 1e-12);
}
