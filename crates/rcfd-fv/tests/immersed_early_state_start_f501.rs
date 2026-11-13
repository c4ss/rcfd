//! F-501 (ADR 0421): the early-state pressure modes of the single corrector, from a start without flow through the cylinder.
//!
//! ADR 0405 found one-step eigenvalues 1.77 to 1.91 at steps 16 to 20 for the first-order forcing F of DFG 2D-2 on M2 with
//! one PISO corrector at `dt = 1e-3`, 0.97 at step 100, and ten-step growth per step 1.88 for the bounded form B and 1.31 for
//! F, all from the start field of ADR 0249 (the inflow profile in every cell, through the body). Here the same linearisations
//! are repeated from that field with the solid cells set to zero and projected onto the fluid region with no flux through the
//! cylinder (D1, a discrete Helmholtz projection by Jacobi-preconditioned CG), and the bounded form is run from it (D3: its
//! stop, if any, located by the eigenvalue per step of the steps before it). Heavy records (`#[ignore]`, run once in
//! release): Q1 (F: R0, S1, S2, G0', A1, A2), Q2 (B: S1, S2, B1). Fast checks: U5 (the projection on M1), U6 (the rule and
//! the window), U3' (the k-step map from the projected start). The linear algebra, the DFG helpers, the state layout and the
//! analysis are copies of those of `immersed_multistep_linearisation_f494.rs` (that file is not edited).

use rcfd_core::vec3::Vec3;
use rcfd_fv::convection::{face_flux, ConvScheme};
use rcfd_fv::ddt::{courant_numbers, TimeScheme};
use rcfd_fv::diffusion::uniform_gamma;
use rcfd_fv::immersed::{ImmersedBody, ImmersedForcing, ImmersedSettings, SolidShape};
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver, PimpleState};
use rcfd_fv::{FixedValue, FvGeometry, Profile, ScalarField, VectorField, ZeroGradient};
use rcfd_mesh::gen::{axis_coords, box_2d_coords, expansion_for_end_cell, Grading};
use std::sync::Arc;
use std::time::Instant;

// ---------------------------------------------------------------------------------------------------------------------------
// Complex numbers (ADR 0392 D4)
// ---------------------------------------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
struct C {
    re: f64,
    im: f64,
}

impl C {
    const ZERO: C = C { re: 0.0, im: 0.0 };
    fn new(re: f64, im: f64) -> C {
        C { re, im }
    }
    fn abs(self) -> f64 {
        self.re.hypot(self.im)
    }
    fn conj(self) -> C {
        C::new(self.re, -self.im)
    }
    fn scale(self, a: f64) -> C {
        C::new(self.re * a, self.im * a)
    }
    fn div(self, o: C) -> C {
        let d = o.re * o.re + o.im * o.im;
        C::new(
            (self.re * o.re + self.im * o.im) / d,
            (self.im * o.re - self.re * o.im) / d,
        )
    }
    fn sqrt(self) -> C {
        let r = self.abs();
        if r == 0.0 {
            return C::ZERO;
        }
        let re = (0.5 * (r + self.re)).sqrt();
        let im = (0.5 * (r - self.re)).sqrt();
        C::new(re, if self.im < 0.0 { -im } else { im })
    }
}

impl std::ops::Add for C {
    type Output = C;
    fn add(self, o: C) -> C {
        C::new(self.re + o.re, self.im + o.im)
    }
}

impl std::ops::Sub for C {
    type Output = C;
    fn sub(self, o: C) -> C {
        C::new(self.re - o.re, self.im - o.im)
    }
}

impl std::ops::Mul for C {
    type Output = C;
    fn mul(self, o: C) -> C {
        C::new(self.re * o.re - self.im * o.im, self.re * o.im + self.im * o.re)
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// Eigenvalues of a small Hessenberg matrix: single-shift complex QR (ADR 0392 D4)
// ---------------------------------------------------------------------------------------------------------------------------

/// The eigenvalue of `[[a, b], [c, d]]` nearest `d` (the Wilkinson shift).
fn wilkinson(a: C, b: C, c: C, d: C) -> C {
    let half = (a + d).scale(0.5);
    let q = (a - d).scale(0.5);
    let disc = (q * q + b * c).sqrt();
    let (l1, l2) = (half + disc, half - disc);
    if (l1 - d).abs() <= (l2 - d).abs() {
        l1
    } else {
        l2
    }
}

/// Eigenvalues of the real upper Hessenberg matrix `h` (`n x n`, row-major `h[i][j]`).
fn hess_eigs(h: &[Vec<f64>]) -> Vec<C> {
    let n = h.len();
    let mut a: Vec<Vec<C>> = h.iter().map(|r| r.iter().map(|&x| C::new(x, 0.0)).collect()).collect();
    let mut eig = Vec::with_capacity(n);
    let mut hi = n;
    let mut since = 0usize;
    let mut iters = 0usize;
    while hi > 0 {
        if hi == 1 {
            eig.push(a[0][0]);
            break;
        }
        // The start of the unreduced block that ends at hi - 1.
        let mut l = hi - 1;
        while l > 0 {
            let s = a[l - 1][l - 1].abs() + a[l][l].abs();
            if a[l][l - 1].abs() <= 1e-14 * s.max(f64::MIN_POSITIVE) {
                a[l][l - 1] = C::ZERO;
                break;
            }
            l -= 1;
        }
        if l == hi - 1 {
            eig.push(a[hi - 1][hi - 1]);
            hi -= 1;
            since = 0;
            continue;
        }
        iters += 1;
        assert!(iters < 100 * n.max(1), "hess_eigs: no convergence");
        since += 1;
        let mu = if since % 10 == 0 {
            a[hi - 1][hi - 1] + C::new(0.75 * a[hi - 1][hi - 2].abs(), 0.0)
        } else {
            wilkinson(
                a[hi - 2][hi - 2],
                a[hi - 2][hi - 1],
                a[hi - 1][hi - 2],
                a[hi - 1][hi - 1],
            )
        };
        for (i, row) in a.iter_mut().enumerate().take(hi).skip(l) {
            row[i] = row[i] - mu;
        }
        // QR of the block by Givens rotations G_k = [[conj c, conj s], [-s, c]] from the left ...
        let mut rots: Vec<(C, C)> = Vec::with_capacity(hi - l);
        for k in l..hi - 1 {
            let (x, y) = (a[k][k], a[k + 1][k]);
            let r = x.abs().hypot(y.abs());
            let (c, s) = if r == 0.0 {
                (C::new(1.0, 0.0), C::ZERO)
            } else {
                (x.scale(1.0 / r), y.scale(1.0 / r))
            };
            for j in k..hi {
                let (ak, ak1) = (a[k][j], a[k + 1][j]);
                a[k][j] = c.conj() * ak + s.conj() * ak1;
                a[k + 1][j] = c * ak1 - s * ak;
            }
            rots.push((c, s));
        }
        // ... then R Q: the conjugate transposes from the right.
        for (idx, k) in (l..hi - 1).enumerate() {
            let (c, s) = rots[idx];
            for row in a.iter_mut().take((k + 2).min(hi)).skip(l) {
                let (ak, ak1) = (row[k], row[k + 1]);
                row[k] = ak * c + ak1 * s;
                row[k + 1] = ak1 * c.conj() - ak * s.conj();
            }
        }
        for (i, row) in a.iter_mut().enumerate().take(hi).skip(l) {
            row[i] = row[i] + mu;
        }
    }
    eig
}

/// Solve the complex system `m y = b` by Gaussian elimination with partial pivoting (`m` consumed).
fn csolve(mut m: Vec<Vec<C>>, mut b: Vec<C>) -> Vec<C> {
    let n = b.len();
    for k in 0..n {
        let p = (k..n).max_by(|&i, &j| m[i][k].abs().total_cmp(&m[j][k].abs())).unwrap();
        m.swap(k, p);
        b.swap(k, p);
        let piv = if m[k][k].abs() == 0.0 {
            C::new(1e-300, 0.0)
        } else {
            m[k][k]
        };
        for i in k + 1..n {
            let f = m[i][k].div(piv);
            if f.abs() == 0.0 {
                continue;
            }
            for j in k..n {
                let t = m[k][j];
                m[i][j] = m[i][j] - f * t;
            }
            let t = b[k];
            b[i] = b[i] - f * t;
        }
    }
    let mut y = vec![C::ZERO; n];
    for k in (0..n).rev() {
        let mut s = b[k];
        for j in k + 1..n {
            s = s - m[k][j] * y[j];
        }
        let piv = if m[k][k].abs() == 0.0 {
            C::new(1e-300, 0.0)
        } else {
            m[k][k]
        };
        y[k] = s.div(piv);
    }
    y
}

fn cnormalise(y: &mut [C]) {
    let n = y.iter().map(|v| v.abs().powi(2)).sum::<f64>().sqrt();
    for v in y.iter_mut() {
        *v = v.scale(1.0 / n);
    }
}

/// The eigenvector of `h` (`m x m`) for the eigenvalue `theta` by two steps of inverse iteration (ADR 0392 D4).
fn inverse_iteration(h: &[Vec<f64>], theta: C) -> Vec<C> {
    let m = h.len();
    let shift = theta - C::new(1e-10 * theta.abs().max(1e-300), 0.0);
    let mat = || -> Vec<Vec<C>> {
        (0..m)
            .map(|i| {
                (0..m)
                    .map(|j| {
                        let v = C::new(h[i][j], 0.0);
                        if i == j {
                            v - shift
                        } else {
                            v
                        }
                    })
                    .collect()
            })
            .collect()
    };
    let mut y = csolve(mat(), vec![C::new(1.0, 0.0); m]);
    cnormalise(&mut y);
    let mut y = csolve(mat(), y);
    cnormalise(&mut y);
    y
}

// ---------------------------------------------------------------------------------------------------------------------------
// Arnoldi (ADR 0392 D3)
// ---------------------------------------------------------------------------------------------------------------------------

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn norm(a: &[f64]) -> f64 {
    dot(a, a).sqrt()
}

/// A fixed start vector: a linear congruential sequence with the given seed, entries uniform in `[-1, 1]`, normalised.
fn lcg_vector(n: usize, seed: u64) -> Vec<f64> {
    let mut s = seed;
    let mut v: Vec<f64> = (0..n)
        .map(|_| {
            s = s
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            ((s >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0
        })
        .collect();
    let nv = norm(&v);
    v.iter_mut().for_each(|x| *x /= nv);
    v
}

/// The Arnoldi basis `V` (`m + 1` vectors) and the Hessenberg matrix `H` (`(m + 1) x m`) of `apply` from `v1`.
fn arnoldi(apply: &mut dyn FnMut(&[f64]) -> Vec<f64>, v1: Vec<f64>, m: usize) -> (Vec<Vec<f64>>, Vec<Vec<f64>>) {
    let mut v = vec![v1];
    let mut h = vec![vec![0.0; m]; m + 1];
    for j in 0..m {
        let mut w = apply(&v[j]);
        for _pass in 0..2 {
            for (i, vi) in v.iter().enumerate() {
                let c = dot(vi, &w);
                h[i][j] += c;
                w.iter_mut().zip(vi).for_each(|(x, y)| *x -= c * y);
            }
        }
        let nw = norm(&w);
        h[j + 1][j] = nw;
        if nw > 1e-300 {
            w.iter_mut().for_each(|x| *x /= nw);
        }
        v.push(w);
    }
    (v, h)
}

/// A Ritz value, its coefficients in the basis and its estimated residual `|h_{m+1,m}| |y_m| / |y|`.
#[derive(Clone, Debug)]
struct Ritz {
    theta: C,
    y: Vec<C>,
    est: f64,
}

fn ritz(h: &[Vec<f64>], m: usize) -> Vec<Ritz> {
    let hm: Vec<Vec<f64>> = h[..m].iter().map(|r| r[..m].to_vec()).collect();
    hess_eigs(&hm)
        .into_iter()
        .map(|theta| {
            let y = inverse_iteration(&hm, theta);
            let est = h[m][m - 1].abs() * y[m - 1].abs() / theta.abs().max(1e-300);
            Ritz { theta, y, est }
        })
        .collect()
}

/// ADR 0392 D3: a Ritz value is real when `|Im theta| <= 1e-3 |theta|`.
fn is_real(theta: C) -> bool {
    theta.im.abs() <= 1e-3 * theta.abs()
}

/// The Ritz vector `V_m y` as its real and imaginary parts; for a real Ritz value the phase is turned so that the largest
/// coefficient is real and the imaginary part is dropped. Normalised.
fn ritz_vector(v: &[Vec<f64>], r: &Ritz) -> (Vec<f64>, Option<Vec<f64>>) {
    let n = v[0].len();
    let mut y = r.y.clone();
    let real = is_real(r.theta);
    if real {
        let k = (0..y.len()).max_by(|&i, &j| y[i].abs().total_cmp(&y[j].abs())).unwrap();
        let ph = y[k].conj().scale(1.0 / y[k].abs());
        y.iter_mut().for_each(|c| *c = *c * ph);
    }
    let mut zr = vec![0.0; n];
    let mut zi = vec![0.0; n];
    for (yj, vj) in y.iter().zip(v) {
        for i in 0..n {
            zr[i] += yj.re * vj[i];
            zi[i] += yj.im * vj[i];
        }
    }
    if real {
        let nz = norm(&zr);
        zr.iter_mut().for_each(|x| *x /= nz);
        (zr, None)
    } else {
        let nz = (dot(&zr, &zr) + dot(&zi, &zi)).sqrt();
        zr.iter_mut().for_each(|x| *x /= nz);
        zi.iter_mut().for_each(|x| *x /= nz);
        (zr, Some(zi))
    }
}

/// The true residual `|J z - theta z| / (|theta| |z|)` of a Ritz pair; returns it with `J Re z`.
fn true_residual(
    apply: &mut dyn FnMut(&[f64]) -> Vec<f64>,
    theta: C,
    zr: &[f64],
    zi: Option<&[f64]>,
) -> (f64, Vec<f64>) {
    let jr = apply(zr);
    let (num, den) = match zi {
        None => {
            let r: f64 = jr.iter().zip(zr).map(|(a, b)| (a - theta.re * b).powi(2)).sum();
            (r, dot(zr, zr))
        }
        Some(zi) => {
            let ji = apply(zi);
            let mut r = 0.0;
            for i in 0..zr.len() {
                let re = jr[i] - (theta.re * zr[i] - theta.im * zi[i]);
                let im = ji[i] - (theta.re * zi[i] + theta.im * zr[i]);
                r += re * re + im * im;
            }
            (r, dot(zr, zr) + dot(zi, zi))
        }
    };
    ((num / den).sqrt() / theta.abs(), jr)
}

fn matvec(t: &[Vec<f64>], x: &[f64]) -> Vec<f64> {
    t.iter().map(|r| dot(r, x)).collect()
}

// ---------------------------------------------------------------------------------------------------------------------------
// DFG channel (ADR 0218), copied from immersed_single_corrector_f363.rs
// ---------------------------------------------------------------------------------------------------------------------------

const H: f64 = 0.41;
const NU: f64 = 1e-3;
const DIAM: f64 = 0.1;
const CENTRE: Vec3 = [0.2, 0.2, 0.0];
const DZ: f64 = 0.01;
const K_DFG: f64 = 1e7;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Forcing {
    /// `build`, `theta_min = 0.01`.
    F,
    /// `build_second_order_bounded`, `theta_min = 0.01`.
    B,
}

fn forcing(geom: &FvGeometry, bodies: &[ImmersedBody], f: Forcing) -> ImmersedForcing {
    let gamma = uniform_gamma(geom, NU);
    let mut s = ImmersedSettings::new(K_DFG);
    s.theta_min = 1e-2;
    let mut ib = match f {
        Forcing::F => ImmersedForcing::build(geom, bodies, s, &gamma),
        Forcing::B => ImmersedForcing::build_second_order_bounded(geom, bodies, s, &gamma),
    }
    .unwrap();
    ib.flux_weight_check = false;
    ib
}

fn geo_end(n: usize, len: f64, h: f64) -> Vec<f64> {
    let r = expansion_for_end_cell(n, len, h).unwrap();
    axis_coords(n, len, Grading::Expansion(r)).unwrap()
}

fn geo_start(n: usize, len: f64, h: f64) -> Vec<f64> {
    let r = expansion_for_end_cell(n, len, h).unwrap();
    axis_coords(n, len, Grading::Expansion(1.0 / r)).unwrap()
}

fn uniform(a: f64, b: f64, n: usize, centre: f64) -> Vec<f64> {
    let h = (b - a) / n as f64;
    let i0 = ((centre - a) / h).round() as i64;
    (0..=n).map(|i| centre + (i as i64 - i0) as f64 * h).collect()
}

fn join(parts: &[Vec<f64>], offsets: &[f64]) -> Vec<f64> {
    let mut out: Vec<f64> = Vec::new();
    for (p, &o) in parts.iter().zip(offsets) {
        let start = usize::from(!out.is_empty());
        out.extend(p[start..].iter().map(|x| x + o));
    }
    out
}

fn bisect(xs: &[f64]) -> Vec<f64> {
    let mut out = vec![xs[0]];
    for w in xs.windows(2) {
        out.push(0.5 * (w[0] + w[1]));
        out.push(w[1]);
    }
    out
}

fn dfg_family(level: usize) -> (Vec<f64>, Vec<f64>) {
    let h = 0.005;
    let mut xs = join(
        &[
            geo_end(10, 0.12, h),
            uniform(0.12, 0.32, 40, 0.2),
            geo_start(38, 1.88, h),
        ],
        &[0.0, 0.0, 0.32],
    );
    let mut ys = join(
        &[
            geo_end(10, 0.12, h),
            uniform(0.12, 0.28, 32, 0.2),
            geo_start(10, 0.13, h),
        ],
        &[0.0, 0.0, 0.28],
    );
    for _ in 1..level {
        xs = bisect(&xs);
        ys = bisect(&ys);
    }
    (xs, ys)
}

fn inflow_mean(um: f64, y1: f64, y2: f64) -> f64 {
    let prim = |y: f64| H * y * y / 2.0 - y * y * y / 3.0;
    4.0 * um / (H * H) * (prim(y2) - prim(y1)) / (y2 - y1)
}

fn inflow_point(um: f64, y: f64) -> f64 {
    4.0 * um * y * (H - y) / (H * H)
}

fn dfg_body() -> ImmersedBody {
    ImmersedBody::fixed(
        "cylinder",
        SolidShape::Cylinder {
            centre: CENTRE,
            axis: [0.0, 0.0, 1.0],
            radius: 0.5 * DIAM,
        },
    )
}

fn dfg_fields(xs: &[f64], ys: &[f64], um: f64) -> (Arc<FvGeometry>, VectorField, ScalarField) {
    let geom = Arc::new(FvGeometry::new(Arc::new(box_2d_coords(xs, ys, DZ).unwrap())).unwrap());
    let mut u = VectorField::from_fn("U", &geom, |c| [inflow_point(um, c[1]), 0.0, 0.0]);
    let cf = geom.mesh.face_centres();
    let prof: Vec<Vec3> = geom
        .mesh
        .patch("xmin")
        .unwrap()
        .faces()
        .map(|f| {
            let y = cf[f][1];
            let j = ys.partition_point(|&v| v < y) - 1;
            [inflow_mean(um, ys[j], ys[j + 1]), 0.0, 0.0]
        })
        .collect();
    u.set_bc(&geom, "xmin", Box::new(FixedValue(Profile::PerFace(prof))))
        .unwrap();
    for w in ["ymin", "ymax"] {
        u.set_bc(&geom, w, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    u.set_bc(&geom, "xmax", Box::new(ZeroGradient)).unwrap();
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    p.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
    (geom, u, p)
}

fn two_d_two_start(geom: &FvGeometry, u: &mut VectorField, um: f64) {
    let cc = geom.mesh.cell_centres();
    for (c, x) in cc.iter().enumerate() {
        let blob = 0.2 * (-((x[0] - 0.35).powi(2) + (x[1] - 0.2).powi(2)) / (0.05 * 0.05)).exp();
        u.values[c] = [inflow_point(um, x[1]), blob, 0.0];
    }
}

/// The set-up R of ADR 0374 on `dfg_family(level)` with `n_correctors` correctors and the forcing `f` (check off).
fn r_solver(level: usize, f: Forcing, n_correctors: usize) -> (Arc<FvGeometry>, PimpleSolver) {
    let (xs, ys) = dfg_family(level);
    let um = 1.5;
    let (geom, mut u, p) = dfg_fields(&xs, &ys, um);
    two_d_two_start(&geom, &mut u, um);
    let mut st = PimpleSettings::piso(NU);
    st.n_correctors = n_correctors;
    st.time_scheme = TimeScheme::Bdf2;
    st.convection = ConvScheme::LinearUpwind;
    st.amg_pressure = true;
    st.momentum_solver.tol_rel = 1e-6;
    st.pressure_solver.tol_rel = 1e-6;
    let mut s = PimpleSolver::new(geom.clone(), u, p, st);
    s.immersed = Some(forcing(&geom, &[dfg_body()], f));
    (geom, s)
}

fn max_abs_u(s: &PimpleSolver) -> f64 {
    s.u.values.iter().map(|v| v[0].hypot(v[1])).fold(0.0, f64::max)
}

// ---------------------------------------------------------------------------------------------------------------------------
// The state vector, the step map and its linearisation (ADR 0392 D1, D2)
// ---------------------------------------------------------------------------------------------------------------------------

/// The layout of D1: `u_n` (active components per cell), `u_{n-1}`, `p_n`, `phi_n / |S|`, `phi_{n-1} / |S|` (non-empty faces).
struct Layout {
    comps: Vec<usize>,
    nc: usize,
    faces: Vec<usize>,
    area: Vec<f64>,
}

impl Layout {
    fn new(geom: &FvGeometry) -> Layout {
        let comps: Vec<usize> = (0..3).filter(|&k| geom.active[k]).collect();
        let faces: Vec<usize> = (0..geom.mesh.n_faces()).filter(|&f| !geom.empty[f]).collect();
        let area = faces.iter().map(|&f| geom.mesh.face_area(f)).collect();
        Layout {
            comps,
            nc: geom.n_cells(),
            faces,
            area,
        }
    }
    fn nu(&self) -> usize {
        self.comps.len() * self.nc
    }
    /// Block ranges: `u_n`, `u_{n-1}`, `p`, `phi_n`, `phi_{n-1}`.
    fn blocks(&self) -> [std::ops::Range<usize>; 5] {
        let (nu, nc, nf) = (self.nu(), self.nc, self.faces.len());
        [
            0..nu,
            nu..2 * nu,
            2 * nu..2 * nu + nc,
            2 * nu + nc..2 * nu + nc + nf,
            2 * nu + nc + nf..2 * nu + nc + 2 * nf,
        ]
    }
    fn len(&self) -> usize {
        2 * self.nu() + self.nc + 2 * self.faces.len()
    }
    fn pack(&self, st: &PimpleState) -> Vec<f64> {
        let nk = self.comps.len();
        let mut x = Vec::with_capacity(self.len());
        for c in 0..self.nc {
            for &k in &self.comps {
                x.push(st.u[c][k]);
            }
        }
        for c in 0..self.nc {
            for &k in &self.comps {
                x.push(st.hist_u[k].older.as_ref().expect("pack: no older level")[c]);
            }
        }
        debug_assert_eq!(x.len(), 2 * nk * self.nc);
        x.extend_from_slice(&st.p);
        for (i, &f) in self.faces.iter().enumerate() {
            x.push(st.phi[f] / self.area[i]);
        }
        let older = st.phi_hist.older.as_ref().expect("pack: no older flux");
        for (i, &f) in self.faces.iter().enumerate() {
            x.push(older[f] / self.area[i]);
        }
        x
    }
    /// `base + scale * dx` (D1): `u` and `hist_u[k].old` together, `phi` and `phi_hist.old` together.
    fn perturbed(&self, base: &PimpleState, dx: &[f64]) -> PimpleState {
        let mut st = base.clone();
        let nk = self.comps.len();
        let [bu, bo, bp, bf, bfo] = self.blocks();
        for c in 0..self.nc {
            for (j, &k) in self.comps.iter().enumerate() {
                let d = dx[bu.start + c * nk + j];
                st.u[c][k] += d;
                st.hist_u[k].old[c] += d;
                st.hist_u[k].older.as_mut().unwrap()[c] += dx[bo.start + c * nk + j];
            }
        }
        for c in 0..self.nc {
            st.p[c] += dx[bp.start + c];
        }
        for (i, &f) in self.faces.iter().enumerate() {
            let d = dx[bf.start + i] * self.area[i];
            st.phi[f] += d;
            st.phi_hist.old[f] += d;
            st.phi_hist.older.as_mut().unwrap()[f] += dx[bfo.start + i] * self.area[i];
        }
        st
    }
}

/// The linear solves of the map (D2): momentum and pressure `tol_rel = 1e-12`, `tol_abs = 0`.
fn tighten(s: &mut PimpleSolver) {
    s.settings.momentum_solver.tol_rel = 1e-12;
    s.settings.momentum_solver.tol_abs = 0.0;
    s.settings.pressure_solver.tol_rel = 1e-12;
    s.settings.pressure_solver.tol_abs = 0.0;
}

const EPS: f64 = 1e-4;
const M_ARNOLDI: usize = 40;

// ---------------------------------------------------------------------------------------------------------------------------
// The geometry of the body
// ---------------------------------------------------------------------------------------------------------------------------

/// Per cell: fluid, and first fluid (a fluid cell with an interior face to a cell that is not fluid).
fn fluid_cells(geom: &FvGeometry, ib: &ImmersedForcing) -> (Vec<bool>, Vec<bool>) {
    let fluid: Vec<bool> = ib.body_of_cell.iter().map(|b| b.is_none()).collect();
    let mut first = vec![false; fluid.len()];
    let (own, nei) = (geom.mesh.owner(), geom.mesh.neighbour());
    for f in 0..geom.n_internal {
        let (a, b) = (own[f], nei[f]);
        if fluid[a] && !fluid[b] {
            first[a] = true;
        }
        if fluid[b] && !fluid[a] {
            first[b] = true;
        }
    }
    (fluid, first)
}

fn surface_distance(x: Vec3) -> f64 {
    ((x[0] - CENTRE[0]).hypot(x[1] - CENTRE[1]) - 0.5 * DIAM).abs()
}

// ---------------------------------------------------------------------------------------------------------------------------
// The k-step map along the trajectory and its linearisation (ADR 0405 D1, D2)
// ---------------------------------------------------------------------------------------------------------------------------

/// `S^k` from a state (D1): one restore, then `k` steps without a restore in between. A stop panics.
fn k_step_map(s: &mut PimpleSolver, lay: &Layout, st: &PimpleState, dt: f64, k: usize) -> Vec<f64> {
    s.restore(st).unwrap();
    for j in 0..k {
        s.step(dt);
        assert!(
            s.diverged.is_none(),
            "k-step map: stopped at step {j} of {k}: {:?}",
            s.diverged
        );
    }
    lay.pack(&s.snapshot())
}

/// `P_k v = (S^k(e v) - S^k(-e v)) / (2 e)` about `base` (D2).
#[allow(clippy::too_many_arguments)]
fn pkv(s: &mut PimpleSolver, lay: &Layout, base: &PimpleState, dt: f64, k: usize, v: &[f64], e: f64) -> Vec<f64> {
    let plus: Vec<f64> = v.iter().map(|x| e * x).collect();
    let minus: Vec<f64> = v.iter().map(|x| -e * x).collect();
    let sp = k_step_map(s, lay, &lay.perturbed(base, &plus), dt, k);
    let sm = k_step_map(s, lay, &lay.perturbed(base, &minus), dt, k);
    let two_e = 2.0 * e;
    sp.iter().zip(&sm).map(|(a, b)| (a - b) / two_e).collect()
}

/// The eigenvalue per step `abs(mu)^(1/k)` (ADR 0405).
fn per_step(mu: C, k: usize) -> f64 {
    mu.abs().powf(1.0 / k as f64)
}

/// The largest singular value of the `(m + 1) x m` Arnoldi matrix `Hbar_m` (D3): 4000 power iterations on `Hbar^T Hbar` from
/// the all-ones vector.
fn sigma_max(h: &[Vec<f64>], m: usize) -> f64 {
    let mut g = vec![vec![0.0; m]; m];
    for (i, gi) in g.iter_mut().enumerate() {
        for (j, gij) in gi.iter_mut().enumerate() {
            *gij = (0..=m).map(|r| h[r][i] * h[r][j]).sum();
        }
    }
    let mut x = vec![1.0; m];
    let mut lam = 0.0;
    for _ in 0..4000 {
        let y = matvec(&g, &x);
        let ny = norm(&y);
        if ny == 0.0 {
            return 0.0;
        }
        lam = dot(&x, &y) / dot(&x, &x);
        x = y.iter().map(|v| v / ny).collect();
    }
    lam.max(0.0).sqrt()
}

/// The largest cell Courant number of the present state and its cell (D4).
fn c_max(geom: &FvGeometry, s: &PimpleSolver, dt: f64) -> (f64, usize) {
    let c = courant_numbers(geom, &s.snapshot().phi, dt);
    let i = (0..c.len()).max_by(|&a, &b| c[a].total_cmp(&c[b])).unwrap();
    (c[i], i)
}

/// Run `s` on to step `k` (no stop allowed), printing the largest `abs(u)` of every step.
fn run_to(label: &str, s: &mut PimpleSolver, dt: f64, k: usize, from: usize) {
    for n in from + 1..=k {
        s.step(dt);
        assert!(s.diverged.is_none(), "{label}: stopped at step {n}: {:?}", s.diverged);
        println!("{label}: step {n} max |u| {:.5}", max_abs_u(s));
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// One linearisation of k steps (D3) and its report
// ---------------------------------------------------------------------------------------------------------------------------

#[allow(dead_code)]
struct Analysis {
    ritz: Vec<Ritz>,
    mu: C,
    mu_res: f64,
    g: f64,
    /// Ritz values with `abs(theta)^(1/k) >= 1` (conjugate partners once) with their true residuals.
    big: Vec<(C, f64)>,
    /// Ritz values with `abs(theta)^(1/k) >= 1` whose residual was not computed (above the cap for `k > 1`).
    unchecked: usize,
    g0: Option<f64>,
    cmax: f64,
}

/// Linearise `k` steps of `s` from its present state at `dt` (D1 to D3); the state and settings are restored afterwards. With
/// `g0` the linearity check G0 on the dominant vector; for `k > 1` the nonlinear propagation is printed.
fn analyse(label: &str, geom: &FvGeometry, s: &mut PimpleSolver, dt: f64, k: usize, g0: bool) -> Analysis {
    let t0 = Instant::now();
    let base = s.snapshot();
    let saved = s.settings.clone();
    let lay = Layout::new(geom);
    for kk in 0..3 {
        for c in 0..lay.nc {
            assert_eq!(
                base.hist_u[kk].old[c].to_bits(),
                base.u[c][kk].to_bits(),
                "{label}: hist_u old is not u"
            );
        }
    }
    let (cmax, ccell) = c_max(geom, s, dt);
    let xc = geom.mesh.cell_centres()[ccell];
    println!(
        "{label}: C_max {cmax:.5} in cell {ccell} at ({:.5}, {:.5}), distance to the surface {:.5}",
        xc[0],
        xc[1],
        surface_distance(xc)
    );
    let x0 = lay.pack(&base);
    let s_r = k_step_map(s, &lay, &base, dt, k);
    tighten(s);
    let s_0 = k_step_map(s, &lay, &base, dt, k);
    let dstep: Vec<f64> = s_0.iter().zip(&x0).map(|(a, b)| a - b).collect();
    let dtol: Vec<f64> = s_r.iter().zip(&s_0).map(|(a, b)| a - b).collect();
    println!(
        "{label}: k = {k}, state dimension {}, |S_R(0) - S(0)| / |S(0) - x_0| = {:.3e}",
        lay.len(),
        norm(&dtol) / norm(&dstep)
    );
    let mut apply = |v: &[f64]| pkv(s, &lay, &base, dt, k, v, EPS);
    let (v, h) = arnoldi(&mut apply, lcg_vector(lay.len(), 482), M_ARNOLDI);
    let mut rz = ritz(&h, M_ARNOLDI);
    rz.sort_by(|a, b| b.theta.abs().total_cmp(&a.theta.abs()));
    println!("{label}: Ritz values (theta, |theta|, per step, estimated residual), the twelve largest and all with per step > 0.9:");
    for (i, r) in rz.iter().enumerate() {
        if i < 12 || per_step(r.theta, k) > 0.9 {
            println!(
                "{label}:   {:+.6e} {:+.6e}i  |{:.6e}|  per step {:.6}  est {:.3e}",
                r.theta.re,
                r.theta.im,
                r.theta.abs(),
                per_step(r.theta, k),
                r.est
            );
        }
    }
    let sig = sigma_max(&h, M_ARNOLDI);
    println!(
        "{label}: s_max(Hbar) {sig:.6e}, per step {:.6} (a lower bound of the largest singular value of P_k in the scaled norm)",
        sig.powf(1.0 / k as f64)
    );
    let dom = rz[0].clone();
    let (zr, zi) = ritz_vector(&v, &dom);
    let (res, pz) = true_residual(&mut apply, dom.theta, &zr, zi.as_deref());
    let g = per_step(dom.theta, k);
    println!(
        "{label}: dominant mu {:+.6e} {:+.6e}i (|{:.6e}|), per step g = {g:.6}, real {}, true residual {res:.3e}",
        dom.theta.re,
        dom.theta.im,
        dom.theta.abs(),
        is_real(dom.theta)
    );
    let cap = if k == 1 { usize::MAX } else { 6 };
    let mut big = Vec::new();
    let mut unchecked = 0;
    for r in rz.iter().filter(|r| per_step(r.theta, k) >= 1.0) {
        if r.theta == dom.theta {
            big.push((r.theta, res));
            continue;
        }
        if r.theta.im < 0.0 && rz.iter().any(|q| q.theta == r.theta.conj()) {
            continue;
        }
        if big.len() >= cap {
            unchecked += 1;
            continue;
        }
        let (a, b) = ritz_vector(&v, r);
        let (rr, _) = true_residual(&mut apply, r.theta, &a, b.as_deref());
        big.push((r.theta, rr));
    }
    for (t, r) in &big {
        println!(
            "{label}: per step >= 1: {:+.6e} {:+.6e}i (|{:.6e}|, per step {:.6}), true residual {:.3e}",
            t.re,
            t.im,
            t.abs(),
            per_step(*t, k),
            r
        );
    }
    if unchecked > 0 {
        println!("{label}: {unchecked} further Ritz values with per step >= 1 not checked (cap {cap})");
    }
    let g0v = if g0 {
        let p5 = pkv(s, &lay, &base, dt, k, &zr, 1e-5);
        let d: Vec<f64> = pz.iter().zip(&p5).map(|(a, b)| a - b).collect();
        let r = norm(&d) / norm(&pz);
        println!("{label}: G0 |P_1e-4 z - P_1e-5 z| / |P_1e-4 z| = {r:.3e}");
        Some(r)
    } else {
        None
    };
    // The parts of the dominant vector.
    let nk = lay.comps.len();
    let blocks = lay.blocks();
    let sq = |i: usize| zr[i] * zr[i] + zi.as_ref().map_or(0.0, |z| z[i] * z[i]);
    let cell_u = |c: usize| (0..nk).map(|j| sq(blocks[0].start + c * nk + j)).sum::<f64>();
    let total: f64 = (0..lay.len()).map(sq).sum();
    let shares: Vec<f64> = blocks.iter().map(|b| b.clone().map(sq).sum::<f64>() / total).collect();
    let mode_cell = (0..lay.nc).max_by(|&a, &b| cell_u(a).total_cmp(&cell_u(b))).unwrap();
    let pcell = (0..lay.nc)
        .max_by(|&a, &b| sq(blocks[2].start + a).total_cmp(&sq(blocks[2].start + b)))
        .unwrap();
    let cc = geom.mesh.cell_centres();
    let (xm, xp) = (cc[mode_cell], cc[pcell]);
    let (fluid, first) = match s.immersed.as_ref() {
        Some(ib) => fluid_cells(geom, ib),
        None => (vec![true; lay.nc], vec![false; lay.nc]),
    };
    println!(
        "{label}: dominant shares of |z|^2: u_n {:.4}, u_n-1 {:.4}, p {:.4}, phi_n {:.4}, phi_n-1 {:.4}",
        shares[0], shares[1], shares[2], shares[3], shares[4]
    );
    println!(
        "{label}: dominant mode cell (u) {mode_cell} at ({:.5}, {:.5}), fluid {}, first fluid {}, distance to the surface {:.5}; largest |p| cell {pcell} at ({:.5}, {:.5}), fluid {}, distance to the surface {:.5}, distance to the C_max cell {:.5}",
        xm[0],
        xm[1],
        fluid[mode_cell],
        first[mode_cell],
        surface_distance(xm),
        xp[0],
        xp[1],
        fluid[pcell],
        surface_distance(xp),
        (xp[0] - xc[0]).hypot(xp[1] - xc[1])
    );
    s.settings = saved;
    if k > 1 {
        // The nonlinear propagation of e v_1 with the settings of R (printed).
        let e = 1e-4;
        let v1 = lcg_vector(lay.len(), 482);
        let pert: Vec<f64> = v1.iter().map(|x| e * x).collect();
        let mut xs_base = Vec::with_capacity(k);
        s.restore(&base).unwrap();
        for _ in 0..k {
            s.step(dt);
            assert!(s.diverged.is_none());
            xs_base.push(lay.pack(&s.snapshot()));
        }
        s.restore(&lay.perturbed(&base, &pert)).unwrap();
        let mut line = String::new();
        for (j, xb) in xs_base.iter().enumerate() {
            s.step(dt);
            assert!(s.diverged.is_none());
            let xq = lay.pack(&s.snapshot());
            let d: Vec<f64> = xq.iter().zip(xb).map(|(a, b)| a - b).collect();
            line.push_str(&format!(" {}:{:.4e}", j + 1, norm(&d) / e));
        }
        println!("{label}: nonlinear propagation |x'_n - x_n| / e by step:{line}");
    }
    println!("{label}: wall time {:.1} s", t0.elapsed().as_secs_f64());
    s.restore(&base).unwrap();
    Analysis {
        ritz: rz,
        mu: dom.theta,
        mu_res: res,
        g,
        big,
        unchecked,
        g0: g0v,
        cmax,
    }
}

// D1: the start without flow through the cylinder, projected onto the fluid region (ADR 0421)
// ---------------------------------------------------------------------------------------------------------------------------

/// Per cell: solid (the forcing's body holds the cell centre).
fn solid_cells(ib: &ImmersedForcing) -> Vec<bool> {
    ib.body_of_cell.iter().map(|b| b.is_some()).collect()
}

/// The faces of the projection (D1 3): the open internal faces `(face, owner, neighbour, a_f)` and the outlet faces
/// `(face, owner, a_f)`; every other face is closed.
struct OpenFaces {
    internal: Vec<(usize, usize, usize, f64)>,
    outlet: Vec<(usize, usize, f64)>,
}

fn open_faces(geom: &FvGeometry, solid: &[bool]) -> OpenFaces {
    let mesh = &geom.mesh;
    let (own, nei) = (mesh.owner(), mesh.neighbour());
    let cc = mesh.cell_centres();
    let fc = mesh.face_centres();
    let closed = |f: usize| solid[own[f]] || solid[nei[f]];
    let mut internal = Vec::new();
    for f in 0..geom.n_internal {
        if closed(f) {
            continue;
        }
        let (o, n) = (own[f], nei[f]);
        let d = [cc[n][0] - cc[o][0], cc[n][1] - cc[o][1], cc[n][2] - cc[o][2]];
        let dist = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        internal.push((f, o, n, mesh.face_area(f) / dist));
    }
    let mut outlet = Vec::new();
    for f in mesh.patch("xmax").unwrap().faces() {
        let o = own[f];
        let d = [fc[f][0] - cc[o][0], fc[f][1] - cc[o][1], fc[f][2] - cc[o][2]];
        let dist = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        outlet.push((f, o, mesh.face_area(f) / dist));
    }
    OpenFaces { internal, outlet }
}

/// Per cell `sum_f phi_f` out of the cell (empty faces carry zero flux).
fn divergence(geom: &FvGeometry, phi: &[f64]) -> Vec<f64> {
    let mesh = &geom.mesh;
    let (own, nei) = (mesh.owner(), mesh.neighbour());
    let mut d = vec![0.0; geom.n_cells()];
    for f in 0..mesh.n_faces() {
        d[own[f]] += phi[f];
        if f < geom.n_internal {
            d[nei[f]] -= phi[f];
        }
    }
    d
}

/// S1: `max_P abs(sum_f phi_f) / max_P sum_f abs(phi_f)`.
fn divergence_ratio(geom: &FvGeometry, phi: &[f64]) -> f64 {
    let mesh = &geom.mesh;
    let (own, nei) = (mesh.owner(), mesh.neighbour());
    let mut scale = vec![0.0; geom.n_cells()];
    for f in 0..mesh.n_faces() {
        scale[own[f]] += phi[f].abs();
        if f < geom.n_internal {
            scale[nei[f]] += phi[f].abs();
        }
    }
    let d = divergence(geom, phi);
    let dmax = d.iter().fold(0.0f64, |a, x| a.max(x.abs()));
    let smax = scale.iter().fold(0.0f64, |a, x| a.max(*x));
    dmax / smax
}

/// D1 3: the projection of a face flux onto the fluid region (closed faces kept), by Jacobi-preconditioned CG on
/// `sum_f a_f (psi_P - psi_N) = -sum_f phi*_f`. Returns the corrected flux, `psi` and the CG iterations.
fn project_flux(geom: &FvGeometry, open: &OpenFaces, phi_star: &[f64]) -> (Vec<f64>, Vec<f64>, usize) {
    let nc = geom.n_cells();
    let mut diag = vec![0.0; nc];
    for &(_, o, n, a) in &open.internal {
        diag[o] += a;
        diag[n] += a;
    }
    for &(_, o, a) in &open.outlet {
        diag[o] += a;
    }
    // Cells without an open face (the solid ones) are decoupled: psi = 0.
    for d in diag.iter_mut() {
        if *d == 0.0 {
            *d = 1.0;
        }
    }
    let apply = |x: &[f64]| -> Vec<f64> {
        let mut y: Vec<f64> = x.iter().zip(&diag).map(|(a, b)| a * b).collect();
        for &(_, o, n, a) in &open.internal {
            y[o] -= a * x[n];
            y[n] -= a * x[o];
        }
        y
    };
    let div = divergence(geom, phi_star);
    let b: Vec<f64> = div.iter().map(|d| -d).collect();
    let nb = norm(&b);
    let mut psi = vec![0.0; nc];
    let mut iters = 0;
    if nb > 0.0 {
        let mut r = b.clone();
        let mut z: Vec<f64> = r.iter().zip(&diag).map(|(a, d)| a / d).collect();
        let mut p = z.clone();
        let mut rz = dot(&r, &z);
        loop {
            if norm(&r) <= 1e-13 * nb {
                break;
            }
            assert!(
                iters < 50000,
                "project_flux: CG did not converge ({:.3e})",
                norm(&r) / nb
            );
            let ap = apply(&p);
            let alpha = rz / dot(&p, &ap);
            for i in 0..nc {
                psi[i] += alpha * p[i];
                r[i] -= alpha * ap[i];
            }
            z = r.iter().zip(&diag).map(|(a, d)| a / d).collect();
            let rz_new = dot(&r, &z);
            let beta = rz_new / rz;
            rz = rz_new;
            for i in 0..nc {
                p[i] = z[i] + beta * p[i];
            }
            iters += 1;
        }
    }
    let mut phi = phi_star.to_vec();
    for &(f, o, n, a) in &open.internal {
        phi[f] -= a * (psi[n] - psi[o]);
    }
    for &(f, o, a) in &open.outlet {
        phi[f] += a * psi[o];
    }
    (phi, psi, iters)
}

/// The projected start of D1 and what was measured on the way.
struct Start {
    u: Vec<Vec3>,
    phi: Vec<f64>,
    solid: Vec<bool>,
    open: OpenFaces,
    /// `sum abs(phi*)` over the fluid-solid faces before they are closed (the flow into the body of the old start).
    into_solid: f64,
    iters: usize,
}

/// D1 1 to 4 for the solver `s` of set-up R (its start velocity, boundary conditions and forcing).
fn projected_start(geom: &FvGeometry, s: &PimpleSolver) -> Start {
    let solid = solid_cells(s.immersed.as_ref().expect("projected_start: no forcing"));
    let mesh = &geom.mesh;
    let (own, nei) = (mesh.owner(), mesh.neighbour());
    let mut field = s.u.clone();
    for (c, v) in field.values.iter_mut().enumerate() {
        if solid[c] {
            *v = [0.0; 3];
        }
    }
    let bu = field.boundary_values(geom);
    let mut phi_star = face_flux(geom, &field.values, &bu);
    let mut into_solid = 0.0;
    for f in 0..geom.n_internal {
        let (so, sn) = (solid[own[f]], solid[nei[f]]);
        if so != sn {
            into_solid += phi_star[f].abs();
        }
        if so || sn {
            phi_star[f] = 0.0;
        }
    }
    let open = open_faces(geom, &solid);
    let (phi, psi, iters) = project_flux(geom, &open, &phi_star);
    // D1 4: the Gauss gradient of psi.
    let sf = mesh.face_area_vectors();
    let vol = mesh.cell_volumes();
    let mut g = vec![[0.0; 3]; geom.n_cells()];
    for f in 0..mesh.n_faces() {
        if geom.empty[f] {
            continue;
        }
        let o = own[f];
        if f < geom.n_internal {
            let n = nei[f];
            let (psi_o, psi_n) = match (solid[o], solid[n]) {
                (false, false) => {
                    let w = geom.weights[f];
                    let v = w * psi[o] + (1.0 - w) * psi[n];
                    (v, v)
                }
                _ => (psi[o], psi[n]),
            };
            for k in 0..3 {
                g[o][k] += psi_o * sf[f][k];
                g[n][k] -= psi_n * sf[f][k];
            }
        } else {
            let pf = if mesh.patch("xmax").unwrap().faces().contains(&f) {
                0.0
            } else {
                psi[o]
            };
            for k in 0..3 {
                g[o][k] += pf * sf[f][k];
            }
        }
    }
    let u: Vec<Vec3> = (0..geom.n_cells())
        .map(|c| {
            if solid[c] {
                [0.0; 3]
            } else {
                let x = field.values[c];
                [x[0] - g[c][0] / vol[c], x[1] - g[c][1] / vol[c], x[2]]
            }
        })
        .collect();
    Start {
        u,
        phi,
        solid,
        open,
        into_solid,
        iters,
    }
}

/// S2: `u` zero in every solid cell and `phi` zero on every face with a solid cell on either side (exactly).
fn solid_is_closed(geom: &FvGeometry, st: &Start) -> bool {
    let (own, nei) = (geom.mesh.owner(), geom.mesh.neighbour());
    let cells = (0..geom.n_cells()).all(|c| !st.solid[c] || st.u[c] == [0.0; 3]);
    let faces = (0..geom.n_internal).all(|f| !(st.solid[own[f]] || st.solid[nei[f]]) || st.phi[f] == 0.0);
    cells && faces
}

/// Put `s` at the projected start (D1 6) and return the start with S1's ratio.
fn start_new(label: &str, geom: &FvGeometry, s: &mut PimpleSolver) -> (Start, f64) {
    let st = projected_start(geom, s);
    let ratio = divergence_ratio(geom, &st.phi);
    let n_solid = st.solid.iter().filter(|&&b| b).count();
    println!(
        "{label}: projected start: {n_solid} solid cells, flux into the solid of the old start {:.6e}, CG iterations {}, S1 ratio {ratio:.3e}, S2 {}",
        st.into_solid,
        st.iters,
        solid_is_closed(geom, &st)
    );
    let p = vec![0.0; geom.n_cells()];
    s.reinitialise(st.u.clone(), p, Some(st.phi.clone()), 0.0).unwrap();
    (st, ratio)
}

// ---------------------------------------------------------------------------------------------------------------------------
// The attribution rule T and the window of a stop (ADR 0421)
// ---------------------------------------------------------------------------------------------------------------------------

/// The limit of A1 on the converged one-step Ritz values at steps 16 and 20.
const A1_LIMIT: f64 = 1.2;
/// The limit of A2 on the ten-step eigenvalue per step of F'.
const A2_LIMIT: f64 = 1.10;

/// T: the modes attributed to the start state (A1 and A2; `d16`, `d20` the largest converged one-step magnitudes, `g` the
/// ten-step value per step).
fn attributed(d16: f64, d20: f64, g: f64) -> bool {
    d16 <= A1_LIMIT && d20 <= A1_LIMIT && g <= A2_LIMIT
}

/// D3: the window `(n0, k)` before a stop at step `n_s`; `None` when it has no step.
fn stop_window(n_s: usize) -> Option<(usize, usize)> {
    let n0 = n_s.saturating_sub(11).max(2);
    if n_s < n0 + 2 {
        None
    } else {
        Some((n0, n_s - 1 - n0))
    }
}

/// The largest magnitude of a converged (true residual `<= 0.03`) Ritz value of magnitude at least one (0 if there is none).
fn largest_converged(a: &Analysis) -> f64 {
    a.big
        .iter()
        .filter(|(_, r)| *r <= 0.03)
        .map(|(t, _)| t.abs())
        .fold(0.0, f64::max)
}

// ---------------------------------------------------------------------------------------------------------------------------
// Fast checks U5, U6, U3'
// ---------------------------------------------------------------------------------------------------------------------------

/// ADR 0421 U5: the projection D1 on DFG M1 with F.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn u5_the_projected_start_on_m1() {
    let dt = 1e-3;
    let (geom, mut s) = r_solver(1, Forcing::F, 1);
    let (st, ratio) = start_new("U5", &geom, &mut s);
    assert!(ratio <= 1e-10, "U5 (a): divergence ratio {ratio:.3e}");
    assert!(solid_is_closed(&geom, &st), "U5 (b): flow in the solid");
    let (again, _, it2) = project_flux(&geom, &st.open, &st.phi);
    let pmax = st.phi.iter().fold(0.0f64, |a, x| a.max(x.abs()));
    let dmax = again.iter().zip(&st.phi).fold(0.0f64, |a, (x, y)| a.max((x - y).abs()));
    println!(
        "U5: (c) re-projection changes phi by {:.3e} of max |phi| ({it2} CG iterations)",
        dmax / pmax
    );
    assert!(dmax <= 1e-10 * pmax, "U5 (c): {:.3e}", dmax / pmax);
    assert!(st.into_solid > 0.0, "U5 (d): no flux into the solid of the old start");
    for n in 1..=2 {
        s.step(dt);
        assert!(s.diverged.is_none(), "U5 (d): stopped at step {n}");
    }
    println!("U5: two steps from the projected start, max |u| {:.5}", max_abs_u(&s));
}

/// ADR 0421 U6: the attribution rule on the registered limits and the window of a stop.
#[test]
fn u6_the_attribution_rule_and_the_stop_window() {
    assert!(!attributed(1.767, 1.911, 1.311), "U6 (i)");
    assert!(attributed(0.97, 0.98, 0.99), "U6 (ii)");
    assert!(!attributed(1.19, 1.19, 1.11), "U6 (iii) a");
    assert!(attributed(1.19, 1.19, 1.09), "U6 (iii) b");
    assert!(!attributed(1.21, 1.0, 1.0), "U6 (iii) c");
    assert_eq!(stop_window(34), Some((23, 10)));
    assert_eq!(stop_window(13), Some((2, 10)));
    assert_eq!(stop_window(5), Some((2, 2)));
    assert_eq!(stop_window(3), None);
}

/// ADR 0421 U3': the copied k-step map on M1 from the projected start (F, after step 2, `k = 3`) is the trajectory to the bit.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn u3p_the_k_step_map_from_the_projected_start_on_m1() {
    let dt = 1e-3;
    let k = 3;
    let (geom, mut s) = r_solver(1, Forcing::F, 1);
    let _ = start_new("U3'", &geom, &mut s);
    run_to("U3'", &mut s, dt, 2, 0);
    let base = s.snapshot();
    run_to("U3'", &mut s, dt, 2 + k, 2);
    let direct = s.snapshot();
    let lay = Layout::new(&geom);
    let (_, mut t) = r_solver(1, Forcing::F, 1);
    let zero = vec![0.0; lay.len()];
    let _ = k_step_map(&mut t, &lay, &lay.perturbed(&base, &zero), dt, k);
    let bits = |st: &PimpleState| -> Vec<u64> {
        st.u.iter()
            .flat_map(|v| v.iter().map(|x| x.to_bits()))
            .chain(st.p.iter().map(|x| x.to_bits()))
            .chain(st.phi.iter().map(|x| x.to_bits()))
            .collect()
    };
    assert!(
        bits(&t.snapshot()) == bits(&direct),
        "U3': the k-step map at zero is not the trajectory"
    );
}

// ---------------------------------------------------------------------------------------------------------------------------
// Record Q1: F from the old start (R0) and from the projected start (S, PF', XF', RF')
// ---------------------------------------------------------------------------------------------------------------------------

/// ADR 0421 record Q1. Heavy (several minutes in release): run once.
#[test]
#[ignore = "heavy record of ADR 0421 (R0, S1, S2, G0', A1, A2; F' on to step 200 printed): run once in release"]
fn record_q1_first_order_from_the_projected_start() {
    let t0 = Instant::now();
    let dt = 1e-3;
    let mut misses = Vec::new();
    {
        let (geom, mut s) = r_solver(2, Forcing::F, 1);
        run_to("R0", &mut s, dt, 16, 0);
        let r0 = analyse("R0 F16 old", &geom, &mut s, dt, 1, false);
        let rel = (r0.mu.abs() - 1.767).abs() / 1.767;
        println!("R0: dominant |mu| {:.6}, relative to 1.767 {rel:.3e}", r0.mu.abs());
        if rel > 1e-3 {
            misses.push(format!("R0: {:.6} against 1.767 (relative {rel:.3e})", r0.mu.abs()));
        }
    }
    let (geom, mut s) = r_solver(2, Forcing::F, 1);
    let (st, ratio) = start_new("S F", &geom, &mut s);
    if ratio > 1e-10 {
        misses.push(format!("S1 F: {ratio:.3e}"));
    }
    if !solid_is_closed(&geom, &st) {
        misses.push("S2 F: flow in the solid".into());
    }
    run_to("F'", &mut s, dt, 15, 0);
    let pf = analyse("PF'", &geom, &mut s, dt, 10, true);
    let g0 = pf.g0.unwrap();
    if g0 > 1e-3 {
        misses.push(format!("G0' PF': {g0:.3e} > 1e-3"));
    }
    let mut d = Vec::new();
    for (from, to) in [(15, 16), (16, 20), (20, 25)] {
        run_to("F'", &mut s, dt, to, from);
        let a = analyse(&format!("XF' F{to}"), &geom, &mut s, dt, 1, false);
        d.push(largest_converged(&a));
    }
    println!(
        "A: largest converged one-step |theta| >= 1 (0 = none): step 16 {:.6}, step 20 {:.6}, step 25 {:.6} (old 1.767, 1.911, 1.358)",
        d[0], d[1], d[2]
    );
    if d[0] > A1_LIMIT || d[1] > A1_LIMIT {
        misses.push(format!(
            "A1: {:.6} (step 16), {:.6} (step 20) against {A1_LIMIT}",
            d[0], d[1]
        ));
    }
    if pf.mu_res > 0.03 || pf.g > A2_LIMIT {
        misses.push(format!(
            "A2: PF' per step {:.6}, true residual {:.3e} (limit {A2_LIMIT}, 0.03)",
            pf.g, pf.mu_res
        ));
    }
    let reading_b = (d[0] >= 1.5 || d[1] >= 1.5) && pf.g >= 1.2;
    println!(
        "T: attributed {} (A1 and A2), reading B {reading_b}; PF' per step {:.6} (old 1.311)",
        attributed(d[0], d[1], pf.g) && pf.mu_res <= 0.03,
        pf.g
    );
    let mut stopped = None;
    for n in 26..=200 {
        s.step(dt);
        if let Some(dv) = &s.diverged {
            stopped = Some((n, dv.quantity, dv.value));
            break;
        }
        if n % 10 == 0 {
            println!("F': step {n} max |u| {:.5}", max_abs_u(&s));
        }
        if n == 100 {
            let _ = analyse("XF' F100", &geom, &mut s, dt, 1, false);
        }
    }
    println!(
        "RF': stop {stopped:?}, max |u| at the end {:.5} (F from the old start: 2.210 at step 200)",
        max_abs_u(&s)
    );
    println!("Q1: wall time {:.1} s", t0.elapsed().as_secs_f64());
    assert!(misses.is_empty(), "record Q1: {misses:?}");
}

// ---------------------------------------------------------------------------------------------------------------------------
// Record Q2: the bounded form from the projected start (S, RB', LB', PB')
// ---------------------------------------------------------------------------------------------------------------------------

/// ADR 0421 record Q2. Heavy (several minutes in release): run once.
#[test]
#[ignore = "heavy record of ADR 0421 (S1, S2, B1; PB' printed): run once in release"]
fn record_q2_bounded_from_the_projected_start() {
    let t0 = Instant::now();
    let dt = 1e-3;
    let mut misses = Vec::new();
    let n_s = {
        let (geom, mut s) = r_solver(2, Forcing::B, 1);
        let (st, ratio) = start_new("S B", &geom, &mut s);
        if ratio > 1e-10 {
            misses.push(format!("S1 B: {ratio:.3e}"));
        }
        if !solid_is_closed(&geom, &st) {
            misses.push("S2 B: flow in the solid".into());
        }
        let mut stop = None;
        for n in 1..=200 {
            s.step(dt);
            if let Some(dv) = &s.diverged {
                stop = Some((n, dv.step, dv.quantity, dv.value));
                break;
            }
            println!("RB': step {n} max |u| {:.5}", max_abs_u(&s));
        }
        println!("RB': stop {stop:?} (B from the old start: step 34, Courant guard)");
        stop.map(|x| x.0)
    };
    match n_s {
        None => println!("B1: B' runs 200 steps from the projected start"),
        Some(n_s) => match stop_window(n_s) {
            None => misses.push(format!("B1: stop at step {n_s}, no window before it")),
            Some((n0, k)) => {
                let (geom, mut s) = r_solver(2, Forcing::B, 1);
                let _ = start_new("LB'", &geom, &mut s);
                run_to("LB'", &mut s, dt, n0, 0);
                let lb = analyse(&format!("LB' steps {}-{}", n0 + 1, n0 + k), &geom, &mut s, dt, k, false);
                println!(
                    "B1: stop at step {n_s}, window from step {n0}, k = {k}: per step {:.6}, true residual {:.3e}",
                    lb.g, lb.mu_res
                );
                if lb.mu_res > 0.03 || lb.g <= 1.0 {
                    misses.push(format!(
                        "B1: stop at step {n_s} not located: per step {:.6}, true residual {:.3e}",
                        lb.g, lb.mu_res
                    ));
                }
            }
        },
    }
    if n_s.is_none_or(|n| n > 25) {
        let (geom, mut s) = r_solver(2, Forcing::B, 1);
        let _ = start_new("PB'", &geom, &mut s);
        run_to("PB'", &mut s, dt, 15, 0);
        let pb = analyse("PB'", &geom, &mut s, dt, 10, false);
        println!("PB': per step {:.6} (old 1.881), true residual {:.3e}", pb.g, pb.mu_res);
    }
    println!("Q2: wall time {:.1} s", t0.elapsed().as_secs_f64());
    assert!(misses.is_empty(), "record Q2: {misses:?}");
}
