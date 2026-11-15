//! F-579 (ADR 0538): the radiation impedance matrix of several velocity fields of a baffled plane, two small pistons, and the
//! light-fluid added mass, damping and first-frequency shift of a clamped plate. Checks Y1, Y2, Q1, D1, D2, M1 to M3, R1, S1 to
//! S3, P1, C1, F1 to F5, the negative controls NC1 to NC3, the refusals N1 and the times T1 of the registration. Every largest
//! error propagates NaN and every compared value must be finite (ADR 0488 amendment 1).

use rcfd_core::vec3::Vec3;
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::modal::{modal_analysis_with, ModalOptions};
use rcfd_fem::{BlockProperty, ElementKind, Material, SolidMesh, SolidModel};
use rcfd_post::solid_acoustic::{
    light_fluid_first_frequency, radiation_impedance_matrix, BaffledPlane, LightFluidOptions, Phasor, RayleighOptions,
    VibratingSurface,
};
use std::f64::consts::PI;
use std::time::Instant;

const RHO: f64 = 1.2;
const C: f64 = 340.0;
const U: f64 = 1e-3;

struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value <= limit;
        println!(
            "{name}: {value:.6e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} > {limit:e}"));
        }
    }
    fn above(&mut self, name: &str, value: f64, bound: f64) {
        let ok = value.is_finite() && value > bound;
        println!(
            "{name}: {value:.6e} (must be > {bound:e}) {}",
            if ok { "as registered" } else { "NOT AS REGISTERED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} <= {bound:e}"));
        }
    }
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {detail} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name}: {detail}"));
        }
    }
    fn finite(&mut self, name: &str, values: &[f64]) {
        let ok = values.iter().all(|v| v.is_finite());
        self.holds(&format!("{name} finite"), ok, "");
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:?}", self.0);
    }
}

/// The larger of two errors, NaN if either is NaN.
fn nmax(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else {
        a.max(b)
    }
}

fn cs(a: Phasor, b: Phasor) -> Phasor {
    Phasor::new(a.re - b.re, a.im - b.im)
}

fn cm(a: Phasor, b: Phasor) -> Phasor {
    Phasor::new(a.re * b.re - a.im * b.im, a.re * b.im + a.im * b.re)
}

fn conj(a: Phasor) -> Phasor {
    Phasor::new(a.re, -a.im)
}

/// `|a - b|/|b|`.
fn crel(a: Phasor, b: Phasor) -> f64 {
    cs(a, b).abs() / b.abs()
}

fn rel(a: f64, b: f64) -> f64 {
    (a / b - 1.0).abs()
}

fn zvel(v: Phasor) -> [Phasor; 3] {
    [Phasor::default(), Phasor::default(), v]
}

fn opts() -> RayleighOptions {
    RayleighOptions::new()
}

// ------------------------------------------------------------------------------------------------------------ pistons

/// Two squares of side `l`, 4 x 4 Quad4 each, centres `(-d/2, 0)` and `(d/2, 0)`; the square of each node.
fn two_squares(l: f64, d: f64) -> (Vec<Vec3>, Vec<Vec<usize>>, Vec<usize>) {
    let n = 4;
    let mut nodes = Vec::new();
    let mut faces = Vec::new();
    let mut which = Vec::new();
    for (s, cx) in [-d / 2.0, d / 2.0].iter().enumerate() {
        let off = nodes.len();
        for j in 0..=n {
            for i in 0..=n {
                nodes.push([
                    cx - l / 2.0 + l * i as f64 / n as f64,
                    -l / 2.0 + l * j as f64 / n as f64,
                    0.0,
                ]);
                which.push(s);
            }
        }
        let idx = |i: usize, j: usize| off + j * (n + 1) + i;
        for j in 0..n {
            for i in 0..n {
                faces.push(vec![idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1)]);
            }
        }
    }
    (nodes, faces, which)
}

fn plane_of(nodes: &[Vec3], faces: &[Vec<usize>], vel: Vec<[Phasor; 3]>) -> BaffledPlane {
    BaffledPlane::new(
        &VibratingSurface::new(nodes.to_vec(), faces.to_vec(), vel).unwrap(),
        &opts(),
    )
    .unwrap()
}

fn piston_fields(which: &[usize]) -> [Vec<[Phasor; 3]>; 2] {
    let f = |s: usize| {
        which
            .iter()
            .map(|&w| zvel(Phasor::new(if w == s { U } else { 0.0 }, 0.0)))
            .collect::<Vec<_>>()
    };
    [f(0), f(1)]
}

/// Y1 measure: `max |Z_mn - Z_nm| / max |Z_mm|`.
fn asymmetry(p: &[Vec<Phasor>]) -> f64 {
    let mut a: f64 = 0.0;
    let mut d: f64 = 0.0;
    for m in 0..p.len() {
        d = nmax(d, p[m][m].abs());
        for n in 0..p.len() {
            a = nmax(a, cs(p[m][n], p[n][m]).abs());
        }
    }
    a / d
}

/// D1 and D2 of the field `vel` against F-551's `radiation_impedance` of a plane built afresh.
fn diag_errors(
    nodes: &[Vec3],
    faces: &[Vec<usize>],
    vel: &[[Phasor; 3]],
    zmm: Phasor,
    omega: f64,
    rho: f64,
    c: f64,
) -> (f64, f64) {
    let r = plane_of(nodes, faces, vel.to_vec())
        .radiation_impedance(rho, c, omega)
        .unwrap();
    let d1 = crel(zmm, r.complex_power);
    let z = Phasor::new(
        2.0 * zmm.re / r.mean_square_velocity,
        2.0 * zmm.im / r.mean_square_velocity,
    );
    (d1, crel(z, r.impedance))
}

#[test]
fn y1_y2_q1_d1_d2_m1_m3_pistons_and_complex_fields() {
    let mut c = Checks::new();
    let l = 0.01;
    let d = 30.0 * l;
    let (nodes, faces, which) = two_squares(l, d);
    let base = plane_of(&nodes, &faces, piston_fields(&which)[0].clone());
    let mut y1: f64 = 0.0;
    let mut d1: f64 = 0.0;
    let mut d2: f64 = 0.0;
    for kd in [1.0, 3.0] {
        let k = kd / d;
        let omega = k * C;
        let fields = piston_fields(&which);
        let planes: Vec<BaffledPlane> = fields.iter().map(|v| base.with_velocity(v.clone()).unwrap()).collect();
        let t = Instant::now();
        let z = radiation_impedance_matrix(&planes, RHO, C, omega).unwrap();
        println!("T1 pistons kd={kd}: matrix {:.3} s", t.elapsed().as_secs_f64());
        y1 = nmax(y1, asymmetry(&z.power));
        for (m, v) in fields.iter().enumerate() {
            let (a, b) = diag_errors(&nodes, &faces, v, z.power[m][m], omega, RHO, C);
            d1 = nmax(d1, a);
            d2 = nmax(d2, b);
        }
        // M1, M2: z = 2 Z/U^2 vs (i omega rho/(2 pi)) S^2 e^{-ikd}/d
        let amp = omega * RHO / (2.0 * PI) * l.powi(4) / d;
        let f = Phasor::new(amp * kd.sin(), amp * kd.cos()); // i e^{-ikd} = sin + i cos
        let z12 = Phasor::new(2.0 * z.power[0][1].re / (U * U), 2.0 * z.power[0][1].im / (U * U));
        let z21 = Phasor::new(2.0 * z.power[1][0].re / (U * U), 2.0 * z.power[1][0].im / (U * U));
        println!(
            "M kd={kd}: z12 = {:.9e} {:+.9e} i, z21 = {:.9e} {:+.9e} i, formula {:.9e} {:+.9e} i",
            z12.re, z12.im, z21.re, z21.im, f.re, f.im
        );
        c.at_most(&format!("M1 kd={kd} |z12 - f|/|f|"), crel(z12, f), 1e-2);
        c.at_most(&format!("M1 kd={kd} |z21 - f|/|f|"), crel(z21, f), 1e-2);
        c.at_most(&format!("M2 kd={kd} Re z12 rel"), rel(z12.re, f.re), 1e-2);
        c.at_most(&format!("M2 kd={kd} Im z12 rel"), rel(z12.im, f.im), 1e-2);
        // M3: the F-418 power route
        let both: Vec<[Phasor; 3]> = which.iter().map(|_| zvel(Phasor::new(U, 0.0))).collect();
        let wp = base.with_velocity(both).unwrap().power(RHO, C, omega).unwrap().power;
        let w1 = planes[0].power(RHO, C, omega).unwrap().power;
        let w2 = planes[1].power(RHO, C, omega).unwrap().power;
        c.at_most(
            &format!("M3 kd={kd} Re z12 vs (W_pair - W1 - W2)/U^2"),
            rel(z12.re, (wp - w1 - w2) / (U * U)),
            1e-6,
        );
    }
    c.at_most("Y1 pistons (both kd) asymmetry", y1, 1e-10);
    // complex fields at kd = 3
    let omega = 3.0 / d * C;
    let f1: Vec<[Phasor; 3]> = nodes
        .iter()
        .map(|x| zvel(Phasor::new(U * (1.0 + x[0] / d), 0.3 * U * (1.0 + x[0] / d))))
        .collect();
    let f2: Vec<[Phasor; 3]> = f1.iter().map(|v| zvel(Phasor::new(-v[2].im, v[2].re))).collect();
    let f3: Vec<[Phasor; 3]> = nodes
        .iter()
        .zip(&which)
        .map(|(x, &w)| {
            zvel(if w == 0 {
                Phasor::new(U * (0.5 + 0.7 * x[1] / l), U * (-0.2 + 0.4 * x[1] / l))
            } else {
                Phasor::new(0.2 * U, 0.9 * U)
            })
        })
        .collect();
    let cf = [f1, f2, f3];
    let planes: Vec<BaffledPlane> = cf.iter().map(|v| base.with_velocity(v.clone()).unwrap()).collect();
    let z = radiation_impedance_matrix(&planes, RHO, C, omega).unwrap();
    let conj_planes: Vec<BaffledPlane> = cf
        .iter()
        .map(|v| {
            base.with_velocity(v.iter().map(|a| zvel(conj(a[2]))).collect())
                .unwrap()
        })
        .collect();
    let zc = radiation_impedance_matrix(&conj_planes, RHO, C, omega).unwrap();
    let mut y2: f64 = 0.0;
    let mut nc3: f64 = 0.0;
    let mut dmax: f64 = 0.0;
    let mut zmax: f64 = 0.0;
    for m in 0..3 {
        dmax = nmax(dmax, z.power[m][m].abs());
        for n in 0..3 {
            zmax = nmax(zmax, z.power[m][n].abs());
            y2 = nmax(y2, cs(z.power[n][m], zc.power[m][n]).abs());
            nc3 = nmax(nc3, cs(z.power[n][m], z.power[m][n]).abs());
        }
    }
    c.at_most("Y2 complex fields |Z_nm[F] - Z_mn[conj F]|/max|Z_mm|", y2 / dmax, 1e-10);
    c.above(
        "NC3 complex fields |Z_nm - Z_mn|/max|Z_mm| (no conjugation)",
        nc3 / dmax,
        1e-6,
    );
    let i = Phasor::new(0.0, 1.0);
    let q1a = cs(z.power[1][2], cm(i, z.power[0][2])).abs() / zmax;
    let q1b = cs(z.power[2][1], cm(conj(i), z.power[2][0])).abs() / zmax;
    c.at_most("Q1 Z(F2, F3) = i Z(F1, F3)", q1a, 1e-12);
    c.at_most("Q1 Z(F3, F2) = -i Z(F3, F1)", q1b, 1e-12);
    for (m, v) in cf.iter().enumerate() {
        let (a, b) = diag_errors(&nodes, &faces, v, z.power[m][m], omega, RHO, C);
        d1 = nmax(d1, a);
        d2 = nmax(d2, b);
    }
    c.at_most("D1 pistons and complex fields |Z_mm - Pi_F551|/|Pi_F551|", d1, 1e-10);
    c.at_most(
        "D2 pistons and complex fields |2 Z_mm/<|V|^2> - Z_F551|/|Z_F551|",
        d2,
        1e-10,
    );
    c.finish();
}

#[test]
fn nc1_pistons_not_small() {
    let mut c = Checks::new();
    let l = 0.01;
    let d = 2.0 * l;
    let (nodes, faces, which) = two_squares(l, d);
    let fields = piston_fields(&which);
    let planes: Vec<BaffledPlane> = fields.iter().map(|v| plane_of(&nodes, &faces, v.clone())).collect();
    for kd in [1.0, 3.0] {
        let omega = kd / d * C;
        let z = radiation_impedance_matrix(&planes, RHO, C, omega).unwrap();
        let amp = omega * RHO / (2.0 * PI) * l.powi(4) / d;
        let f = Phasor::new(amp * kd.sin(), amp * kd.cos());
        let z12 = Phasor::new(2.0 * z.power[0][1].re / (U * U), 2.0 * z.power[0][1].im / (U * U));
        c.above(&format!("NC1 d = 2 L, kd={kd}: |z12 - f|/|f|"), crel(z12, f), 1e-2);
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ plate

const NX: usize = 24;
const NY: usize = 16;
const PA: f64 = 0.3;
const PB: f64 = 0.2;

struct Plate {
    nodes: Vec<Vec3>,
    faces: Vec<Vec<usize>>,
    /// `w` of each mode at each node (node `j (NX + 1) + i`).
    modes: Vec<Vec<f64>>,
    omegas: Vec<f64>,
}

fn plate() -> Plate {
    let idx = |i: usize, j: usize| j * (NX + 1) + i;
    let mut nodes = Vec::new();
    for j in 0..=NY {
        for i in 0..=NX {
            nodes.push([
                -PA / 2.0 + PA * i as f64 / NX as f64,
                -PB / 2.0 + PB * j as f64 / NY as f64,
                0.0,
            ]);
        }
    }
    let mut conn = Vec::new();
    let mut faces = Vec::new();
    for j in 0..NY {
        for i in 0..NX {
            let f = [idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1)];
            conn.extend(f);
            faces.push(f.to_vec());
        }
    }
    let mut pres = Vec::new();
    for j in 0..=NY {
        for i in 0..=NX {
            if i == 0 || i == NX || j == 0 || j == NY {
                for comp in 0..6 {
                    pres.push(Prescribed::new(idx(i, j), comp, 0.0));
                }
            }
        }
    }
    let nn = nodes.len();
    let mut mesh = SolidMesh::new(nodes.clone()).unwrap();
    mesh.add_block(ElementKind::Shell4, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::shell(1e-3).unwrap()).unwrap();
    mesh.add_element_set("all", (0..NX * NY).collect()).unwrap();
    mesh.add_node_set("all", (0..nn).collect()).unwrap();
    let mut model = SolidModel::new(mesh);
    model
        .add_material("all", Material::isotropic_elastic(2e11, 0.3, 7850.0, 0.0).unwrap())
        .unwrap();
    let t = Instant::now();
    let r = modal_analysis_with(&model, &pres, &ModalOptions::new(6)).unwrap();
    println!("T1 plate modal analysis {:.3} s", t.elapsed().as_secs_f64());
    let dofs = r.dofs();
    let modes: Vec<Vec<f64>> = r
        .modes
        .iter()
        .map(|m| (0..nn).map(|n| dofs.equation(n, 2).map_or(0.0, |e| m[e])).collect())
        .collect();
    let omegas: Vec<f64> = r.eigenvalues.iter().map(|l| l.sqrt()).collect();
    println!(
        "R1 FE frequencies (Hz): {:?}",
        r.frequencies
            .iter()
            .map(|f| (f * 100.0).round() / 100.0)
            .collect::<Vec<_>>()
    );
    Plate {
        nodes,
        faces,
        modes,
        omegas,
    }
}

impl Plate {
    fn planes(&self) -> Vec<BaffledPlane> {
        self.modes
            .iter()
            .map(|m| {
                plane_of(
                    &self.nodes,
                    &self.faces,
                    m.iter().map(|&w| zvel(Phasor::new(w, 0.0))).collect(),
                )
            })
            .collect()
    }
    /// Parity class (x, y) of mode `m`: the sign of `sum phi(x) phi(mirror x)`.
    fn parity(&self, m: usize) -> (bool, bool) {
        let w = &self.modes[m];
        let at = |i: usize, j: usize| w[j * (NX + 1) + i];
        let (mut sx, mut sy) = (0.0, 0.0);
        for j in 0..=NY {
            for i in 0..=NX {
                sx += at(i, j) * at(NX - i, j);
                sy += at(i, j) * at(i, NY - j);
            }
        }
        (sx > 0.0, sy > 0.0)
    }
}

fn gauss_legendre(n: usize) -> Vec<(f64, f64)> {
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let mut x = (PI * (i as f64 + 0.75) / (n as f64 + 0.5)).cos();
        let mut dp = 1.0;
        for _ in 0..100 {
            let (mut p0, mut p1) = (1.0, x);
            for k in 2..=n {
                let p2 = ((2 * k - 1) as f64 * x * p1 - (k - 1) as f64 * p0) / k as f64;
                p0 = p1;
                p1 = p2;
            }
            dp = n as f64 * (x * p1 - p0) / (x * x - 1.0);
            let dx = p1 / dp;
            x -= dx;
            if dx.abs() < 1e-15 {
                break;
            }
        }
        out.push((x, 2.0 / ((1.0 - x * x) * dp * dp)));
    }
    out
}

/// Points `(kappa, weight)` on `[lo, hi]` by an `n`-point Gauss rule, the weight times `jac(kappa)`.
fn panel(lo: f64, hi: f64, n: usize) -> Vec<(f64, f64)> {
    gauss_legendre(n)
        .into_iter()
        .map(|(g, w)| ((lo + hi) / 2.0 + g * (hi - lo) / 2.0, w * (hi - lo) / 2.0))
        .collect()
}

/// The spectral reference D5 of `Zr_mn = int P[phi_m] phi_n dS` of the plate modes at `k`.
fn spectral(p: &Plate, k: f64, rho: f64, omega: f64) -> Vec<Vec<Phasor>> {
    let h = PA / NX as f64;
    let hy = PB / NY as f64;
    assert!((h - hy).abs() < 1e-15);
    let nm = p.modes.len();
    for m in &p.modes {
        for j in 0..=NY {
            for i in 0..=NX {
                if i == 0 || i == NX || j == 0 || j == NY {
                    assert_eq!(m[j * (NX + 1) + i], 0.0, "boundary value not zero");
                }
            }
        }
    }
    // radial rules: (kappa, weight including the Jacobian), real part (kappa < k) and imaginary part
    let mut real_part = Vec::new();
    for (t, w) in panel(0.0, PI / 2.0, 64) {
        real_part.push((k * t.sin(), k * t.sin() * w));
    }
    let mut imag_part = Vec::new();
    for (u, w) in panel(0.0, 2.0_f64.acosh(), 64) {
        imag_part.push((k * u.cosh(), k * u.cosh() * w));
    }
    let kmax = 16.0 * PI / h;
    let mut edges = vec![2.0 * k];
    let lo = (4.0 * k).max(5.0);
    for e in 0..50 {
        edges.push(lo * (kmax / lo).powf(e as f64 / 49.0));
    }
    for e in 0..edges.len() - 1 {
        for (kap, w) in panel(edges[e], edges[e + 1], 8) {
            imag_part.push((kap, w * kap / (kap * kap - k * k).sqrt()));
        }
    }
    let nth = 256;
    let wth = 2.0 * PI / nth as f64;
    let hat = |kap: f64, x: f64| -> Phasor {
        let q = kap * h / 2.0;
        let s = if q == 0.0 { 1.0 } else { q.sin() / q };
        let a = h * s * s;
        Phasor::new(a * (kap * x).cos(), -a * (kap * x).sin())
    };
    let xs: Vec<f64> = (0..=NX).map(|i| -PA / 2.0 + PA * i as f64 / NX as f64).collect();
    let ys: Vec<f64> = (0..=NY).map(|j| -PB / 2.0 + PB * j as f64 / NY as f64).collect();
    let sum = |rule: &[(f64, f64)]| -> Vec<Vec<Phasor>> {
        let mut acc = vec![vec![Phasor::default(); nm]; nm];
        for &(kap, w) in rule {
            for l in 0..nth {
                let th = (l as f64 + 0.5) * wth;
                let (kx, ky) = (kap * th.cos(), kap * th.sin());
                let hxs: Vec<Phasor> = xs.iter().map(|&x| hat(kx, x)).collect();
                let hys: Vec<Phasor> = ys.iter().map(|&y| hat(ky, y)).collect();
                let vh: Vec<Phasor> = p
                    .modes
                    .iter()
                    .map(|m| {
                        let mut s = Phasor::default();
                        for j in 1..NY {
                            let mut r = Phasor::default();
                            for i in 1..NX {
                                let v = m[j * (NX + 1) + i];
                                r.re += v * hxs[i].re;
                                r.im += v * hxs[i].im;
                            }
                            let t = cm(r, hys[j]);
                            s.re += t.re;
                            s.im += t.im;
                        }
                        s
                    })
                    .collect();
                for a in 0..nm {
                    for b in 0..nm {
                        let t = cm(vh[a], conj(vh[b]));
                        acc[a][b].re += w * wth * t.re;
                        acc[a][b].im += w * wth * t.im;
                    }
                }
            }
        }
        acc
    };
    let r = sum(&real_part);
    let x = sum(&imag_part);
    let f = rho * omega / (4.0 * PI * PI);
    // (R + i X), R and X being Hermitian sums
    (0..nm)
        .map(|a| {
            (0..nm)
                .map(|b| Phasor::new(f * (r[a][b].re - x[a][b].im), f * (r[a][b].im + x[a][b].re)))
                .collect()
        })
        .collect()
}

/// The smallest eigenvalue of `diag(kd) q = lambda m q` by Cholesky of `m` and cyclic Jacobi rotations (independent of the
/// library's inverse iteration).
fn lambda_min_jacobi(kd: &[f64], m: &[Vec<f64>]) -> f64 {
    let n = kd.len();
    let mut l = vec![vec![0.0; n]; n];
    for j in 0..n {
        let s: f64 = m[j][j] - (0..j).map(|q| l[j][q] * l[j][q]).sum::<f64>();
        l[j][j] = s.sqrt();
        for i in j + 1..n {
            l[i][j] = (m[i][j] - (0..j).map(|q| l[i][q] * l[j][q]).sum::<f64>()) / l[j][j];
        }
    }
    // A = L^{-1} K L^{-T}: columns of L^{-1} by forward substitution
    let mut linv = vec![vec![0.0; n]; n];
    for c in 0..n {
        for i in 0..n {
            let mut s = if i == c { 1.0 } else { 0.0 };
            for q in 0..i {
                s -= l[i][q] * linv[q][c];
            }
            linv[i][c] = s / l[i][i];
        }
    }
    let mut a = vec![vec![0.0; n]; n];
    for i in 0..n {
        for j in 0..n {
            a[i][j] = (0..n).map(|q| linv[i][q] * kd[q] * linv[j][q]).sum();
        }
    }
    for _ in 0..100 {
        let off: f64 = (0..n)
            .flat_map(|i| (0..n).filter(move |&j| j != i).map(move |j| (i, j)))
            .map(|(i, j)| a[i][j] * a[i][j])
            .sum();
        if off.sqrt() < 1e-18 * (0..n).map(|i| a[i][i].abs()).sum::<f64>() {
            break;
        }
        for p in 0..n {
            for q in p + 1..n {
                if a[p][q] == 0.0 {
                    continue;
                }
                let theta = (a[q][q] - a[p][p]) / (2.0 * a[p][q]);
                let t = theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt());
                let t = if theta == 0.0 { 1.0 } else { t };
                let cth = 1.0 / (t * t + 1.0).sqrt();
                let sth = t * cth;
                for r in 0..n {
                    let (arp, arq) = (a[r][p], a[r][q]);
                    a[r][p] = cth * arp - sth * arq;
                    a[r][q] = sth * arp + cth * arq;
                }
                for r in 0..n {
                    let (apr, aqr) = (a[p][r], a[q][r]);
                    a[p][r] = cth * apr - sth * aqr;
                    a[q][r] = sth * apr + cth * aqr;
                }
            }
        }
    }
    (0..n).map(|i| a[i][i]).fold(f64::INFINITY, f64::min)
}

#[test]
fn r1_s1_s3_p1_c1_y1_d1_plate_modal_impedance() {
    let mut c = Checks::new();
    let p = plate();
    let planes = p.planes();
    let w1 = p.omegas[0];
    let t = Instant::now();
    let z = radiation_impedance_matrix(&planes, RHO, C, w1).unwrap();
    println!("T1 plate matrix (6 modes) {:.3} s", t.elapsed().as_secs_f64());
    let n = planes.len();
    // R1: report
    println!("R1 added-mass matrix at omega_1 (kg):");
    for i in 0..n {
        println!(
            "   {:?}",
            (0..n)
                .map(|j| format!("{:+.5e}", z.added_mass(i, j)))
                .collect::<Vec<_>>()
        );
    }
    println!("R1 damping matrix at omega_1 (kg/s):");
    for i in 0..n {
        println!(
            "   {:?}",
            (0..n).map(|j| format!("{:+.5e}", z.damping(i, j))).collect::<Vec<_>>()
        );
    }
    let mut rep = Vec::new();
    for (j, pl) in planes.iter().enumerate() {
        let wj = p.omegas[j];
        let zj = radiation_impedance_matrix(std::slice::from_ref(pl), RHO, C, wj).unwrap();
        let (ma, cr) = (zj.added_mass(0, 0), zj.damping(0, 0));
        let zeta = cr / (2.0 * wj * (1.0 + ma));
        println!(
            "R1 mode {}: f = {:.3} Hz, parity {:?}, added mass {:.6e} kg, damping {:.6e} kg/s, zeta {:.6e}",
            j + 1,
            wj / (2.0 * PI),
            p.parity(j),
            ma,
            cr,
            zeta
        );
        rep.extend([ma, cr, zeta]);
    }
    c.finite("R1 report", &rep);
    // Y1, D1, D2, C1
    c.at_most("Y1 plate asymmetry", asymmetry(&z.power), 1e-10);
    let (mut d1, mut d2, mut c1): (f64, f64, f64) = (0.0, 0.0, 0.0);
    for (m, pl) in planes.iter().enumerate() {
        let vel: Vec<[Phasor; 3]> = p.modes[m].iter().map(|&w| zvel(Phasor::new(w, 0.0))).collect();
        let (a, b) = diag_errors(&p.nodes, &p.faces, &vel, z.power[m][m], w1, RHO, C);
        d1 = nmax(d1, a);
        d2 = nmax(d2, b);
        let wn = pl.power(RHO, C, w1).unwrap().power;
        c1 = nmax(c1, rel(z.damping(m, m), 2.0 * wn));
    }
    c.at_most("D1 plate modes |Z_mm - Pi_F551|/|Pi_F551|", d1, 1e-10);
    c.at_most("D2 plate modes |2 Z_mm/<|V|^2> - Z_F551|/|Z_F551|", d2, 1e-10);
    c.at_most("C1 C_nn vs 2 W_F418 (largest)", c1, 1e-6);
    // P1
    let mut p1: f64 = 0.0;
    for a in 0..n {
        for b in 0..n {
            if p.parity(a) != p.parity(b) {
                let v = z.impedance(a, b).abs() / (z.impedance(a, a).abs() * z.impedance(b, b).abs()).sqrt();
                p1 = nmax(p1, v);
            }
        }
    }
    c.at_most("P1 different parity |Zr_mn|/sqrt(|Zr_mm Zr_nn|)", p1, 1e-6);
    // S1 to S3
    let t = Instant::now();
    let s = spectral(&p, w1 / C, RHO, w1);
    println!("T1 spectral reference {:.3} s", t.elapsed().as_secs_f64());
    for m in 0..3 {
        let zr = z.impedance(m, m);
        println!(
            "S mode {}: Zr = {:.9e} {:+.9e} i, spectral {:.9e} {:+.9e} i",
            m + 1,
            zr.re,
            zr.im,
            s[m][m].re,
            s[m][m].im
        );
        c.at_most(&format!("S1 mode {} Im Zr rel", m + 1), rel(zr.im, s[m][m].im), 1e-3);
        c.at_most(&format!("S2 mode {} Re Zr rel", m + 1), rel(zr.re, s[m][m].re), 1e-6);
    }
    let js = (1..n).find(|&j| p.parity(j) == p.parity(0));
    c.holds(
        "S3 a mode of the parity of mode 1 among the six",
        js.is_some(),
        &format!("{js:?}"),
    );
    if let Some(j) = js {
        let zr = z.impedance(0, j);
        println!(
            "S3 Zr_1{} = {:.9e} {:+.9e} i, spectral {:.9e} {:+.9e} i",
            j + 1,
            zr.re,
            zr.im,
            s[0][j].re,
            s[0][j].im
        );
        c.at_most(
            &format!("S3 |Zr_1{} - ref|/|Zr_11|", j + 1),
            cs(zr, s[0][j]).abs() / z.impedance(0, 0).abs(),
            1e-3,
        );
    }
    c.finish();
}

#[test]
fn f1_f5_nc2_plate_light_fluid_frequency() {
    let mut c = Checks::new();
    let p = plate();
    let planes = p.planes();
    let w1 = p.omegas[0];
    let mu = vec![1.0; planes.len()];
    let o = LightFluidOptions::new();
    let t = Instant::now();
    let r6 = light_fluid_first_frequency(&planes, &p.omegas, &mu, RHO, C, &o).unwrap();
    println!(
        "T1 coupled N = 6 {:.3} s, {} steps",
        t.elapsed().as_secs_f64(),
        r6.iterations
    );
    let r1 = light_fluid_first_frequency(&planes[..1], &p.omegas[..1], &mu[..1], RHO, C, &o).unwrap();
    let eps = r6.added_mass / mu[0];
    let d_first = r6.omega_first_order - w1;
    let d6 = r6.omega_coupled - w1;
    let d1 = r1.omega_coupled - w1;
    println!(
        "F omega_1 {:.9e} rad/s ({:.4} Hz); eps {:.6e}; damping {:.6e} kg/s, zeta {:.6e}",
        w1,
        w1 / (2.0 * PI),
        eps,
        r6.damping,
        r6.damping_ratio
    );
    println!(
        "F shifts: first-order {:.6e}, coupled N=6 {:.6e}, N=1 {:.6e} (relative to omega_1); shape {:?}",
        d_first / w1,
        d6 / w1,
        d1 / w1,
        r6.shape
    );
    c.finite("F results", &[eps, d_first, d6, d1, r6.damping, r6.damping_ratio]);
    c.at_most(
        "F1 |Delta_c/Delta_first - 1| (air, N = 6)",
        (d6 / d_first - 1.0).abs(),
        3e-2,
    );
    println!(
        "F5 Delta_c/Delta_first - 1: N=6 {:.6e}, N=1 {:.6e}; -3 eps/4 = {:.6e}; zeta^2/2 = {:.3e}",
        d6 / d_first - 1.0,
        d1 / d_first - 1.0,
        -0.75 * eps,
        r6.damping_ratio * r6.damping_ratio / 2.0
    );
    // F2: N = 1 root by F-551
    let wc = r1.omega_coupled;
    let pi = planes[0].radiation_impedance(RHO, C, wc).unwrap().complex_power;
    let ma = 2.0 * pi.im / wc;
    c.at_most(
        "F2 N=1 |omega_1 sqrt(mu/(mu + M_a(omega_c))) - omega_c|/omega_c",
        rel(w1 * (mu[0] / (mu[0] + ma)).sqrt(), wc),
        1e-10,
    );
    // F3: N = 6 root by the matrix and Jacobi
    let wc = r6.omega_coupled;
    let z = radiation_impedance_matrix(&planes, RHO, C, wc).unwrap();
    let n = planes.len();
    let m: Vec<Vec<f64>> = (0..n)
        .map(|i| {
            (0..n)
                .map(|j| 0.5 * (z.added_mass(i, j) + z.added_mass(j, i)) + if i == j { mu[i] } else { 0.0 })
                .collect()
        })
        .collect();
    let kd: Vec<f64> = p.omegas.iter().map(|w| w * w).collect();
    c.at_most(
        "F3 N=6 |lambda_min(K, Mu + M_a(omega_c)) - omega_c^2|/omega_c^2",
        rel(lambda_min_jacobi(&kd, &m), wc * wc),
        1e-10,
    );
    // F4
    let pi1 = planes[0].radiation_impedance(RHO, C, w1).unwrap().complex_power;
    c.at_most(
        "F4 omega_first_order vs omega_1 (1 - M_a,11/(2 mu_1))",
        rel(r6.omega_first_order, w1 * (1.0 - 2.0 * pi1.im / w1 / (2.0 * mu[0]))),
        1e-12,
    );
    // NC2: water
    let t = Instant::now();
    let rw = light_fluid_first_frequency(&planes, &p.omegas, &mu, 1000.0, 1500.0, &o).unwrap();
    println!(
        "T1 water N = 6 {:.3} s, {} steps; eps {:.4}, first-order {:.4}, coupled {:.4}",
        t.elapsed().as_secs_f64(),
        rw.iterations,
        rw.added_mass,
        rw.omega_first_order / w1 - 1.0,
        rw.omega_coupled / w1 - 1.0
    );
    c.above(
        "NC2 water |Delta_c/Delta_first - 1|",
        ((rw.omega_coupled - w1) / (rw.omega_first_order - w1) - 1.0).abs(),
        3e-2,
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ N1

#[test]
fn n1_refusals() {
    let mut c = Checks::new();
    let l = 0.01;
    let (nodes, faces, which) = two_squares(l, 30.0 * l);
    let fields = piston_fields(&which);
    let a = plane_of(&nodes, &faces, fields[0].clone());
    let b = a.with_velocity(fields[1].clone()).unwrap();
    let mut moved = nodes.clone();
    moved[3][0] += 1e-4;
    let other_nodes = plane_of(&moved, &faces, fields[1].clone());
    let mut fl = faces.clone();
    fl[0] = vec![fl[0][1], fl[0][2], fl[0][3], fl[0][0]];
    let other_faces = plane_of(&nodes, &fl, fields[1].clone());
    let other_opts = BaffledPlane::new(
        &VibratingSurface::new(nodes.clone(), faces.clone(), fields[1].clone()).unwrap(),
        &RayleighOptions::new().order(4),
    )
    .unwrap();
    let w = 1000.0;
    let mut r: Vec<(&str, bool)> = vec![
        (
            "matrix of no plane",
            radiation_impedance_matrix(&[], RHO, C, w).is_err(),
        ),
        (
            "matrix: other nodes",
            radiation_impedance_matrix(&[a.clone(), other_nodes], RHO, C, w).is_err(),
        ),
        (
            "matrix: other faces",
            radiation_impedance_matrix(&[a.clone(), other_faces], RHO, C, w).is_err(),
        ),
        (
            "matrix: other options",
            radiation_impedance_matrix(&[a.clone(), other_opts], RHO, C, w).is_err(),
        ),
        (
            "matrix: rho 0",
            radiation_impedance_matrix(&[a.clone()], 0.0, C, w).is_err(),
        ),
        (
            "matrix: c NaN",
            radiation_impedance_matrix(&[a.clone()], RHO, f64::NAN, w).is_err(),
        ),
        (
            "matrix: omega -1",
            radiation_impedance_matrix(&[a.clone()], RHO, C, -1.0).is_err(),
        ),
        (
            "with_velocity: short",
            a.with_velocity(fields[0][1..].to_vec()).is_err(),
        ),
        ("with_velocity: NaN", {
            let mut v = fields[0].clone();
            v[2][2] = Phasor::new(f64::NAN, 0.0);
            a.with_velocity(v).is_err()
        }),
    ];
    let o = LightFluidOptions::new();
    let two = [a.clone(), b.clone()];
    r.push((
        "light: no mode",
        light_fluid_first_frequency(&[], &[], &[], RHO, C, &o).is_err(),
    ));
    r.push((
        "light: lengths",
        light_fluid_first_frequency(&two, &[100.0], &[1.0, 1.0], RHO, C, &o).is_err(),
    ));
    r.push((
        "light: omega 0",
        light_fluid_first_frequency(&two, &[0.0, 200.0], &[1.0, 1.0], RHO, C, &o).is_err(),
    ));
    r.push((
        "light: omega NaN",
        light_fluid_first_frequency(&two, &[100.0, f64::NAN], &[1.0, 1.0], RHO, C, &o).is_err(),
    ));
    r.push((
        "light: first not lowest",
        light_fluid_first_frequency(&two, &[200.0, 100.0], &[1.0, 1.0], RHO, C, &o).is_err(),
    ));
    r.push((
        "light: modal mass 0",
        light_fluid_first_frequency(&two, &[100.0, 200.0], &[1.0, 0.0], RHO, C, &o).is_err(),
    ));
    r.push((
        "light: max_iterations 1",
        light_fluid_first_frequency(&two, &[100.0, 200.0], &[1e-6, 1e-6], RHO, C, &o.max_iterations(1)).is_err(),
    ));
    let total = r.len();
    let mut ok = 0;
    for (name, refused) in r {
        c.holds(&format!("N1 {name} refused"), refused, "");
        ok += refused as usize;
    }
    println!("N1 {ok} of {total} refused, no panic");
    c.finish();
}
