//! F-471 (ADR 0743): the corrected order of the coarsest dense LU solve as the default of the scalar AMG, on flow cases.
//!
//! The fast checks P1 (a flow case without an interchange at a step k >= 1 keeps its bits) and P2 (`FvMatrix::solve_amg` uses
//! the corrected order) are what the gate runs. The records O1 to O3 (the stop of ADR 0363 and the breakdown of ADR 0369
//! reproduced under the order of before, asked for with `LinearOptions::amg_coarse_lu_interleaved`), N4 and N5 (the same
//! set-ups under the new default) are ignored tests that need the files of ADR 0363 H1 in `F466_DIR`; N6 (ADR 0268 R1 with
//! the new default, from t = 0 to t = 8) is the ignored heavy record. They replace the records of
//! `immersed_bounded_stop_f466.rs` and `amg_definiteness_f474.rs` that assert the old default. The DFG helpers, the state
//! reader, the window and the recorded conjugate gradients are copies of those of these two files (not edited).

use rcfd_core::vec3::Vec3;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::ddt::{History, TimeScheme};
use rcfd_fv::diffusion::uniform_gamma;
use rcfd_fv::fvmatrix::FvMatrix;
use rcfd_fv::immersed::{ImmersedBody, ImmersedForcing, ImmersedSettings, SolidShape};
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver, PimpleState, PressureCapture, StepInfo};
use rcfd_fv::pressure::pressure_system_src;
use rcfd_fv::{FixedValue, FvGeometry, Profile, ScalarField, VectorField, ZeroGradient};
use rcfd_linalg::vecops::{axpy, dot, norm2, xpby};
use rcfd_linalg::{Amg, AmgSettings, LduAddressing, LduMatrix, LinearOperator, Preconditioner, SolveControl};
use rcfd_mesh::gen::{axis_coords, box_2d_coords, expansion_for_end_cell, Grading};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

// ---------------------------------------------------------------------------------------------------------------------------
// DFG channel (ADR 0218), copied from immersed_dfg_convergence_f354.rs
// ---------------------------------------------------------------------------------------------------------------------------

const H: f64 = 0.41;
const NU: f64 = 1e-3;
const DIAM: f64 = 0.1;
const CENTRE: Vec3 = [0.2, 0.2, 0.0];
const DZ: f64 = 0.01;
const K_DFG: f64 = 1e7;
/// FeatFlow level 6 (docs/reference-data.md, the row "DFG 2D-2 set-up" of the F-219 section).
const CD_MAX_REF: f64 = 3.2200;
const CL_MAX_REF: f64 = 0.9859;
const ST_REF: f64 = 0.301_88;
/// ADR 0268 R3 (T2, bounded, D/80): max C_D and max C_L.
const T2_BOUNDED: (f64, f64) = (3.22439, 0.98762);

/// ADR 0363 H3 (T, bounded, the IC repeat, from the state of step 2950): max C_D, max C_L, St.
const H3_RECORD: (f64, f64, f64) = (3.22695, 1.00253, 0.30144);

/// The message of ADR 0268 R1 (`f354-scratch/rec_r1_t_bounded.log`).
const R1_MESSAGE: &str =
    "PIMPLE divergence guard: pressure is NaN (not finite) at step 2951, outer iteration 1, pressure solve, \
                          cell 0 at (1.2500e-3, 1.2500e-3, 5.0000e-3); lower solver.dt or the relaxation factors";
/// ADR 0268 R1's printed coefficients at t = 1 and t = 2.
const R1_PRINTED: [(usize, f64, f64); 2] = [(1000, 2.98321, -0.35990), (2000, 3.13571, -0.74178)];

#[derive(Clone, Copy, PartialEq, Debug)]
enum Forcing {
    /// `ImmersedForcing::build_second_order_bounded` (ADR 0254).
    Bounded,
}

fn forcing_of(geom: &FvGeometry, forcing: Forcing) -> ImmersedForcing {
    let gamma = uniform_gamma(geom, NU);
    let st = ImmersedSettings::new(K_DFG);
    let body = ImmersedBody::fixed(
        "cylinder",
        SolidShape::Cylinder {
            centre: CENTRE,
            axis: [0.0, 0.0, 1.0],
            radius: 0.5 * DIAM,
        },
    );
    match forcing {
        Forcing::Bounded => ImmersedForcing::build_second_order_bounded(geom, &[body], st, &gamma).unwrap(),
    }
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

/// ADR 0218 set-up: M1 coordinates, bisected `level - 1` times.
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

/// ADR 0218 T mesh of 2D-2.
fn t_mesh() -> (Vec<f64>, Vec<f64>) {
    let xs = join(&[uniform(0.0, 0.6, 240, 0.2), geo_start(84, 1.6, 0.0025)], &[0.0, 0.6]);
    let ys = uniform(0.0, H, 164, 0.2);
    (xs, ys)
}

fn inflow_mean(um: f64, y1: f64, y2: f64) -> f64 {
    let prim = |y: f64| H * y * y / 2.0 - y * y * y / 3.0;
    4.0 * um / (H * H) * (prim(y2) - prim(y1)) / (y2 - y1)
}

fn inflow_point(um: f64, y: f64) -> f64 {
    4.0 * um * y * (H - y) / (H * H)
}

/// The DFG channel on the given coordinates (ADR 0218) with the start field of the 2D-2 runs.
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
    let cc = geom.mesh.cell_centres();
    for (c, x) in cc.iter().enumerate() {
        let blob = 0.2 * (-((x[0] - 0.35).powi(2) + (x[1] - 0.2).powi(2)) / (0.05 * 0.05)).exp();
        u.values[c] = [inflow_point(um, x[1]), blob, 0.0];
    }
    (geom, u, p)
}

/// The settings of ADR 0268's `run_2d2`.
fn settings() -> PimpleSettings {
    let mut st = PimpleSettings::piso(NU);
    st.time_scheme = TimeScheme::Bdf2;
    st.convection = ConvScheme::LinearUpwind;
    st.amg_pressure = true;
    st.n_correctors = 2;
    st.momentum_solver.tol_rel = 1e-6;
    st.pressure_solver.tol_rel = 1e-6;
    st
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum MeshId {
    T,
    M2,
}

/// A fresh 2D-2 solver at `t = 0` on mesh `m` with the given forcing (ADR 0268 `run_2d2`).
fn solver(m: MeshId, forcing: Forcing) -> (Arc<FvGeometry>, ImmersedForcing, PimpleSolver) {
    let (xs, ys) = match m {
        MeshId::T => t_mesh(),
        MeshId::M2 => dfg_family(2),
    };
    let (geom, u, p) = dfg_fields(&xs, &ys, 1.5);
    let ib = forcing_of(&geom, forcing);
    let mut s = PimpleSolver::new(geom.clone(), u, p, settings());
    s.immersed = Some(ib.clone());
    (geom, ib, s)
}

fn coefficients(ib: &ImmersedForcing, s: &PimpleSolver) -> (f64, f64) {
    let q = 0.5 * DIAM * DZ;
    let (f, _) = ib.force_moment(0, &s.u.values, 1.0, CENTRE);
    (f[0] / q, f[1] / q)
}

// ---------------------------------------------------------------------------------------------------------------------------
// The state on disk (ADR 0363 reading 4), the reader copied from immersed_bounded_stop_f466.rs
// ---------------------------------------------------------------------------------------------------------------------------

const MAGIC: &[u8; 8] = b"F466CKP1";

struct In<'a>(&'a [u8], usize);

impl In<'_> {
    fn u64(&mut self) -> u64 {
        let v = u64::from_le_bytes(self.0[self.1..self.1 + 8].try_into().unwrap());
        self.1 += 8;
        v
    }
    fn f64(&mut self) -> f64 {
        f64::from_bits(self.u64())
    }
    fn vec(&mut self) -> Vec<f64> {
        let n = self.u64() as usize;
        (0..n).map(|_| self.f64()).collect()
    }
    fn opt_vec(&mut self) -> Option<Vec<f64>> {
        (self.u64() == 1).then(|| self.vec())
    }
    fn opt_f64(&mut self) -> Option<f64> {
        (self.u64() == 1).then(|| self.f64())
    }
    fn history(&mut self) -> History {
        History {
            old: self.vec(),
            older: self.opt_vec(),
            dt_old: self.opt_f64(),
            oldest: self.opt_vec(),
            dt_older: self.opt_f64(),
        }
    }
}

fn read_state(path: &Path) -> PimpleState {
    let mut bytes = Vec::new();
    std::fs::File::open(path).unwrap().read_to_end(&mut bytes).unwrap();
    assert_eq!(&bytes[..8], MAGIC, "{}: not a state file", path.display());
    let mut i = In(&bytes, 8);
    let flat = i.vec();
    let u: Vec<Vec3> = flat.chunks(3).map(|c| [c[0], c[1], c[2]]).collect();
    let p = i.vec();
    let phi = i.vec();
    let time = i.f64();
    let step_no = i.u64() as usize;
    let hist_u = [i.history(), i.history(), i.history()];
    let phi_hist = i.history();
    let last = (i.u64() == 1).then(|| StepInfo {
        step: i.u64() as usize,
        time: i.f64(),
        dt: i.f64(),
        courant: i.f64(),
        continuity: i.f64(),
        res_u: i.f64(),
        res_p: i.f64(),
        p_iters: i.u64() as usize,
    });
    assert_eq!(i.1, bytes.len(), "{}: trailing bytes", path.display());
    PimpleState {
        u,
        p,
        phi,
        time,
        step_no,
        hist_u,
        phi_hist,
        last,
    }
}

fn bits_equal(a: &[f64], b: &[f64]) -> usize {
    assert_eq!(a.len(), b.len());
    a.iter().zip(b).filter(|(x, y)| x.to_bits() != y.to_bits()).count()
}

fn flat(u: &[Vec3]) -> Vec<f64> {
    u.iter().flat_map(|v| v.iter().copied()).collect()
}

/// Number of differing values of `u`, `p`, `phi` between two solvers (0 = bit-equal).
fn state_diff(a: &PimpleSolver, b: &PimpleSolver) -> usize {
    bits_equal(&flat(&a.u.values), &flat(&b.u.values))
        + bits_equal(&a.p.values, &b.p.values)
        + bits_equal(&a.phi, &b.phi)
}

/// (step, Courant bits) of H1's CSV.
fn read_csv_courant(path: &Path) -> Vec<(usize, u64)> {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .skip(1)
        .map(|l| {
            let f: Vec<&str> = l.split(',').collect();
            (f[0].parse().unwrap(), u64::from_str_radix(f[2], 16).unwrap())
        })
        .collect()
}

fn system_of(g: &FvGeometry, c: &PressureCapture) -> FvMatrix {
    pressure_system_src(g, &c.gamma, &c.bc, &c.phi, &c.controls, c.src.as_deref()).expect("plain pressure path")
}

fn nonfinite(v: &[f64]) -> usize {
    v.iter().filter(|x| !x.is_finite()).count()
}

/// The window of ADR 0268 (copied): crossings, St, extrema.
fn window(ts: &[f64], cds: &[f64], cls: &[f64]) -> (usize, f64, f64, f64, f64, f64, f64) {
    let w0 = ts.iter().position(|&t| t >= 5.5 - 1e-12).unwrap();
    let (tw, dw, lw) = (&ts[w0..], &cds[w0..], &cls[w0..]);
    let mean = lw.iter().sum::<f64>() / lw.len() as f64;
    let mut cr = Vec::new();
    for i in 1..lw.len() {
        if lw[i - 1] - mean < 0.0 && lw[i] - mean >= 0.0 {
            let w = (mean - lw[i - 1]) / (lw[i] - lw[i - 1]);
            cr.push(tw[i - 1] + w * (tw[i] - tw[i - 1]));
        }
    }
    let n = cr.len();
    let amplitude = lw.iter().copied().fold(f64::MIN, f64::max) - lw.iter().copied().fold(f64::MAX, f64::min);
    if n < 2 {
        return (n, f64::NAN, f64::NAN, f64::NAN, amplitude, f64::NAN, f64::NAN);
    }
    let st = (n - 1) as f64 / (cr[n - 1] - cr[0]) * DIAM;
    let periods: Vec<f64> = cr.windows(2).map(|w| w[1] - w[0]).collect();
    let pm = periods.iter().sum::<f64>() / periods.len() as f64;
    let period_spread = periods.iter().map(|p| (p - pm).abs() / pm).fold(0.0, f64::max);
    let (mut cd_max, mut cl_max, mut maxima) = (f64::MIN, f64::MIN, Vec::new());
    for w in cr.windows(2) {
        let idx: Vec<usize> = (0..tw.len()).filter(|&i| tw[i] >= w[0] && tw[i] < w[1]).collect();
        maxima.push(idx.iter().map(|&i| lw[i]).fold(f64::MIN, f64::max));
        for &i in &idx {
            cd_max = cd_max.max(dw[i]);
            cl_max = cl_max.max(lw[i]);
        }
    }
    let mm = maxima.iter().sum::<f64>() / maxima.len() as f64;
    let max_spread = maxima.iter().map(|m| (m - mm).abs() / mm.abs()).fold(0.0, f64::max);
    (n, st, period_spread, max_spread, amplitude, cd_max, cl_max)
}

// ---------------------------------------------------------------------------------------------------------------------------
// Conjugate gradients by the operations of `rcfd_linalg::cg`, recording `r^T z` per iteration
// ---------------------------------------------------------------------------------------------------------------------------

#[derive(Debug, Default)]
struct CgRec {
    iterations: usize,
    converged: bool,
    /// `r^T z` of the start residual, then after each iteration.
    rz: Vec<f64>,
}

fn rz_cg<A: LinearOperator, M: Preconditioner>(a: &A, b: &[f64], x: &mut [f64], m: &M, c: &SolveControl) -> CgRec {
    let n = a.size();
    let mut t = CgRec::default();
    let done = |res: f64, res0: f64| res <= c.tol_abs || res <= c.tol_rel * res0;
    let (mut r, mut z, mut p, mut q) = (vec![0.0; n], vec![0.0; n], vec![0.0; n], vec![0.0; n]);
    a.apply(x, &mut r);
    for (ri, bi) in r.iter_mut().zip(b) {
        *ri = bi - *ri;
    }
    let res0 = norm2(&r);
    if done(res0, res0) {
        t.converged = true;
        return t;
    }
    m.apply(&r, &mut z);
    p.copy_from_slice(&z);
    let mut rz = dot(&r, &z);
    t.rz.push(rz);
    let mut it = 0;
    while it < c.max_iter {
        a.apply(&p, &mut q);
        let pq = dot(&p, &q);
        if pq == 0.0 {
            break;
        }
        let alpha = rz / pq;
        axpy(alpha, &p, x);
        axpy(-alpha, &q, &mut r);
        it += 1;
        if done(norm2(&r), res0) {
            t.iterations = it;
            t.converged = true;
            return t;
        }
        m.apply(&r, &mut z);
        let rz_new = dot(&r, &z);
        let beta = rz_new / rz;
        t.rz.push(rz_new);
        rz = rz_new;
        xpby(&z, beta, &mut p);
    }
    t.iterations = it;
    t
}
// ---------------------------------------------------------------------------------------------------------------------------
// The two orders of the coarsest dense LU solve (ADR 0743 D2)
// ---------------------------------------------------------------------------------------------------------------------------

/// Ask a flow solver for the order of before ADR 0743 (the interleaved coarsest solve) in its pressure AMG.
fn old_order(s: &mut PimpleSolver) {
    s.settings.linear.amg_coarse_lu_interleaved = true;
}

/// The AMG settings of before ADR 0743.
fn old_settings() -> AmgSettings {
    AmgSettings {
        coarse_lu_interchanges_first: false,
        ..AmgSettings::default()
    }
}

fn f466_dir() -> PathBuf {
    PathBuf::from(std::env::var("F466_DIR").expect("F466_DIR must name the folder of ADR 0363 H1's files"))
}

// ---------------------------------------------------------------------------------------------------------------------------
// The fast checks P1 and P2 (the gate)
// ---------------------------------------------------------------------------------------------------------------------------

/// ADR 0743 P1: a flow case whose coarsest factorisations interchange no row at a step k >= 1 is the same bits under the
/// old order and under the new default (M2, bounded forcing, 3 steps).
#[test]
fn p1_m2_old_order_is_the_default_bit_for_bit() {
    let (g, _, mut new) = solver(MeshId::M2, Forcing::Bounded);
    new.pressure_capture = Some(Vec::new());
    let (_, _, mut old) = solver(MeshId::M2, Forcing::Bounded);
    old_order(&mut old);
    for _ in 0..3 {
        new.step(1e-3);
        old.step(1e-3);
    }
    assert!(new.diverged.is_none() && old.diverged.is_none());
    let caps = new.pressure_capture.take().unwrap();
    assert_eq!(caps.len(), 6, "P1: captures");
    for c in &caps {
        let amg = Amg::new(&system_of(&g, c).a.to_csr(), AmgSettings::default());
        let ks = amg.coarse_interchanges();
        println!(
            "P1 step {} corrector {}: levels {:?}, coarsest interchanges at {ks:?}",
            c.step,
            c.corrector,
            amg.level_sizes()
        );
        assert!(
            ks.iter().all(|&k| k == 0),
            "P1: step {} corrector {}: an interchange at a step k >= 1: {ks:?}",
            c.step,
            c.corrector
        );
    }
    assert_eq!(
        state_diff(&new, &old),
        0,
        "P1: the old order and the default differ on M2"
    );
}

/// The 20 x 20 SPD tridiagonal of ADR 0369 L3 (diagonal alternating 1e-4 and 1, off-diagonals -0.4 sqrt(d_i d_j)) on a
/// chain of 20 cells, with the right-hand side `A x`, `x_i = 1 + i / 20`.
fn tri20() -> FvMatrix {
    let n = 20;
    let d = |i: usize| -> f64 {
        if i % 2 == 0 {
            1e-4
        } else {
            1.0
        }
    };
    let addr = Arc::new(LduAddressing {
        n,
        lower: (0..n - 1).collect(),
        upper: (1..n).collect(),
    });
    let mut a = LduMatrix::zeros(addr);
    for i in 0..n {
        a.diag[i] = d(i);
    }
    for f in 0..n - 1 {
        let o = -0.4 * (d(f) * d(f + 1)).sqrt();
        a.lower[f] = o;
        a.upper[f] = o;
    }
    let x: Vec<f64> = (0..n).map(|i| 1.0 + i as f64 / 20.0).collect();
    let mut b = vec![0.0; n];
    a.matvec(&x, &mut b);
    FvMatrix { a, b }
}

/// ADR 0743 P2 and P2a: `FvMatrix::solve_amg` uses the corrected order; `solve_amg_with` gives the old one when asked.
#[test]
fn p2_fvmatrix_solve_amg_uses_the_corrected_order() {
    let m = tri20();
    let ctl = SolveControl {
        tol_abs: 0.0,
        tol_rel: 1e-10,
        max_iter: 50,
    };
    let ks = Amg::new(&m.a.to_csr(), AmgSettings::default()).coarse_interchanges();
    println!("P2: coarsest interchanges at {ks:?}");
    assert!(
        ks.iter().any(|&k| k >= 1),
        "P2: no interchange at a step k >= 1: {ks:?}"
    );
    let (mut x1, mut x2, mut x3) = (vec![0.0; 20], vec![0.0; 20], vec![0.0; 20]);
    let s1 = m.solve_amg(&mut x1, &ctl);
    let s2 = m.solve_amg_with(&mut x2, &ctl, AmgSettings::default());
    let s3 = m.solve_amg_with(&mut x3, &ctl, old_settings());
    let res = norm2(&m.residual(&x1)) / norm2(&m.b);
    let res_old = norm2(&m.residual(&x3)) / norm2(&m.b);
    println!(
        "P2: solve_amg {} iterations, converged {}, relative residual {res:.3e}; old order {} iterations, converged {}, \
         relative residual {res_old:.3e}, differing values {}",
        s1.iterations,
        s1.converged,
        s3.iterations,
        s3.converged,
        bits_equal(&x1, &x3)
    );
    assert!(s1.converged && res <= 1e-10, "P2: relative residual {res:e}");
    assert_eq!(bits_equal(&x1, &x2), 0, "P2: solve_amg_with (default settings) differs");
    assert_eq!(s1.iterations, s2.iterations, "P2: iterations");
    assert!(
        bits_equal(&x1, &x3) >= 1,
        "P2a: the old order gives the bits of the default"
    );
}

// ---------------------------------------------------------------------------------------------------------------------------
// The records on the T run of ADR 0363 (ignored: they need the files of ADR 0363 H1)
// ---------------------------------------------------------------------------------------------------------------------------

/// The outcome of the steps 2901 to 2960 from the state of step 2900.
struct From2900 {
    stop: Option<(usize, String)>,
    /// Steps 2901 to 2950 whose Courant number differs in bits from H1's, and the first of them.
    courant_diff: usize,
    first_diff: Option<usize>,
    /// Non-finite pressure values summed over the steps that did not stop.
    nonfinite_p: usize,
    /// The largest pressure iteration count of a step and its step.
    max_p_iters: (usize, usize),
}

fn from_2900(old: bool) -> From2900 {
    let dir = f466_dir();
    let dt = 1e-3;
    let h1 = read_csv_courant(&dir.join("h1_steps.csv"));
    let (geom, _, mut s) = solver(MeshId::T, Forcing::Bounded);
    if old {
        old_order(&mut s);
    }
    s.restore(&read_state(&dir.join("state_2900.bin"))).unwrap();
    let mut out = From2900 {
        stop: None,
        courant_diff: 0,
        first_diff: None,
        nonfinite_p: 0,
        max_p_iters: (0, 0),
    };
    for n in 2901..=2960 {
        let info = s.step(dt);
        if let Some(d) = &s.diverged {
            let msg = d.message(Some(geom.mesh.cell_centres()));
            println!("stopped at step {n}: {msg}");
            out.stop = Some((n, msg));
            break;
        }
        out.nonfinite_p += nonfinite(&s.p.values);
        if info.p_iters > out.max_p_iters.0 {
            out.max_p_iters = (info.p_iters, n);
        }
        if n <= 2950 {
            let bits = h1.iter().find(|r| r.0 == n).expect("H1's record of the step").1;
            if bits != info.courant.to_bits() {
                out.courant_diff += 1;
                out.first_diff.get_or_insert(n);
            }
        }
    }
    println!(
        "from the state of step 2900 ({}): stop {:?}; Courant numbers of steps 2901 to 2950 differing from H1: {} (first at \
         {:?}); non-finite pressure values {}; largest pressure iteration count of a step {} at step {}",
        if old { "old order" } else { "default" },
        out.stop.as_ref().map(|s| s.0),
        out.courant_diff,
        out.first_diff,
        out.nonfinite_p,
        out.max_p_iters.0,
        out.max_p_iters.1
    );
    out
}

/// ADR 0743 O1 (ADR 0363 M2 under the old order): from the state of step 2900 the old order stops at step 2951 with the
/// message of ADR 0268 R1, and every Courant number before it is H1's bit for bit.
#[test]
#[ignore = "needs the files of ADR 0363 H1 (ADR 0743 O1, about a minute); run in release with --ignored and F466_DIR set"]
fn o1_old_order_reproduces_the_stop_from_the_state_of_step_2900() {
    let r = from_2900(true);
    let (n, msg) = r.stop.expect("O1: the old order did not stop");
    assert_eq!(n, 2951, "O1: stop step");
    assert_eq!(msg, R1_MESSAGE, "O1: stop message");
    assert_eq!(r.courant_diff, 0, "O1: Courant numbers differ from H1");
}

/// ADR 0743 N4 and N4a (ADR 0363 H2 under the new default): from the state of step 2900 the default does not stop to step
/// 2960, and its run is not H1's.
#[test]
#[ignore = "needs the files of ADR 0363 H1 (ADR 0743 N4, about a minute); run in release with --ignored and F466_DIR set"]
fn n4_default_does_not_stop_from_the_state_of_step_2900() {
    let r = from_2900(false);
    assert!(r.stop.is_none(), "N4: stopped: {:?}", r.stop);
    assert_eq!(r.nonfinite_p, 0, "N4: non-finite pressure values");
    assert!(r.courant_diff >= 1, "N4a: every Courant number equals H1's");
}

/// T with the bounded forcing restored from H1's state of step 2950, with the IC repeat of ADR 0363 amendment 1 on.
fn step_2951_with_fallback(old: bool) -> (Option<String>, Vec<(usize, usize, usize)>, usize) {
    let dir = f466_dir();
    let (geom, _, mut s) = solver(MeshId::T, Forcing::Bounded);
    s.settings.pressure_ic_fallback = true;
    if old {
        old_order(&mut s);
    }
    s.restore(&read_state(&dir.join("state_2950.bin"))).unwrap();
    s.step(1e-3);
    let stop = s.diverged.as_ref().map(|d| d.message(Some(geom.mesh.cell_centres())));
    println!(
        "step 2951 with the IC repeat ({}): stop {stop:?}; repeats {:?}",
        if old { "old order" } else { "default" },
        s.pressure_fallbacks
    );
    (stop, s.pressure_fallbacks.clone(), nonfinite(&s.p.values))
}

/// ADR 0743 O2 (ADR 0363 H3a under the old order): one IC repeat, at corrector 1 of step 2951.
#[test]
#[ignore = "needs the files of ADR 0363 H1 (ADR 0743 O2, seconds); run in release with --ignored and F466_DIR set"]
fn o2_old_order_repeats_once_at_step_2951() {
    let (stop, repeats, bad) = step_2951_with_fallback(true);
    assert!(stop.is_none(), "O2: stopped: {stop:?}");
    assert_eq!(repeats, vec![(2951, 0, 1)], "O2: repeats");
    assert_eq!(bad, 0, "O2: pressure");
}

/// ADR 0743 N5 (ADR 0363 H3a under the new default): no IC repeat.
#[test]
#[ignore = "needs the files of ADR 0363 H1 (ADR 0743 N5, seconds); run in release with --ignored and F466_DIR set"]
fn n5_default_needs_no_repeat_at_step_2951() {
    let (stop, repeats, bad) = step_2951_with_fallback(false);
    assert!(stop.is_none(), "N5: stopped: {stop:?}");
    assert!(repeats.is_empty(), "N5: repeats {repeats:?}");
    assert_eq!(bad, 0, "N5: pressure");
}

/// ADR 0743 O3 (ADR 0369 H0, H1 and H5): the stopping system rebuilt from the state of step 2940 under the old order has
/// the registered breakdown; the default hierarchy solves the same system.
#[test]
#[ignore = "needs the files of ADR 0363 H1 (ADR 0743 O3, about half a minute); run in release with --ignored and F466_DIR set"]
fn o3_stopping_system_old_order_and_default() {
    let dir = f466_dir();
    let (geom, _, mut s) = solver(MeshId::T, Forcing::Bounded);
    old_order(&mut s);
    s.restore(&read_state(&dir.join("state_2940.bin"))).unwrap();
    let mut stop = None;
    for n in 2941..=2960 {
        if n == 2950 {
            s.pressure_capture = Some(Vec::new());
        }
        s.step(1e-3);
        if s.diverged.is_some() {
            stop = Some(n);
            break;
        }
    }
    println!("O3: old order, stop at step {stop:?}");
    assert_eq!(stop, Some(2951), "O3: stop step");
    let caps = s.pressure_capture.take().unwrap();
    let cs = caps
        .iter()
        .find(|c| c.step == 2951 && c.corrector == 1 && c.outer == 0)
        .expect("O3: the capture of step 2951 corrector 1");
    println!(
        "O3: captured S iterations {}, non-finite cells {}",
        cs.iters,
        nonfinite(&cs.p)
    );
    assert!(nonfinite(&cs.p) > 0 && cs.iters == 3000, "O3: captured S");
    let ctl = cs.controls.solver;
    let pm = system_of(&geom, cs);
    let csr = pm.a.to_csr();
    let old = Amg::new(&csr, old_settings());
    let new = Amg::new(&csr, AmgSettings::default());
    println!(
        "O3: levels {:?}; coarsest interchanges at {:?}",
        old.level_sizes(),
        old.coarse_interchanges()
    );
    assert_eq!(old.coarse_interchanges(), vec![13, 16, 17], "O3: interchanges of S");
    assert_eq!(
        new.coarse_interchanges(),
        old.coarse_interchanges(),
        "O3: the factorisation"
    );
    let mut xo = cs.p0.clone();
    let to = rz_cg(&pm.a, &pm.b, &mut xo, &old, &ctl);
    let rz1 = format!("{:.3e}", to.rz[1]);
    println!(
        "O3: old-order AMG-CG on S: iterations {}, converged {}, rz start {:.6e}, after iteration 1 {rz1}",
        to.iterations, to.converged, to.rz[0]
    );
    assert_eq!(rz1, "-2.006e-9", "O3: rz after iteration 1");
    let mut xn = cs.p0.clone();
    let tn = rz_cg(&pm.a, &pm.b, &mut xn, &new, &ctl);
    let pos = tn.rz.iter().all(|v| *v > 0.0);
    println!(
        "O3: default AMG-CG on S: iterations {}, converged {}, every rz > 0 {pos}, min rz {:.3e}",
        tn.iterations,
        tn.converged,
        tn.rz.iter().fold(f64::INFINITY, |m, x| m.min(*x))
    );
    assert!(tn.converged && pos && tn.iterations <= 100, "O3: default on S: {tn:?}");
}

/// ADR 0743 N6: ADR 0268 R1 with the new default, T with the bounded forcing from t = 0 to t = 8; no stop, S2 of ADR 0268.
#[test]
#[ignore = "heavy record (ADR 0743 N6, about 80 minutes); run once in release with --ignored"]
fn n6_t_bounded_to_t8_default() {
    let dt = 1e-3;
    let (geom, ib, mut s) = solver(MeshId::T, Forcing::Bounded);
    println!(
        "N6 T bounded dt 1e-3, default AMG: {} cells, {} solid cells, {} flux faces",
        geom.n_cells(),
        ib.bodies[0].solid_cells.len(),
        ib.flux_faces.len()
    );
    let t0 = Instant::now();
    let (mut ts, mut cds, mut cls) = (Vec::new(), Vec::new(), Vec::new());
    let mut max_p_iters = (0usize, 0usize);
    for n in 1..=8000 {
        let info = s.step(dt);
        if let Some(d) = &s.diverged {
            panic!("N6: stopped at step {n}: {}", d.message(Some(geom.mesh.cell_centres())));
        }
        if info.p_iters > max_p_iters.0 {
            max_p_iters = (info.p_iters, n);
        }
        let (cd, cl) = coefficients(&ib, &s);
        ts.push(n as f64 * dt);
        cds.push(cd);
        cls.push(cl);
        if n % 1000 == 0 {
            let r1 = R1_PRINTED.iter().find(|p| p.0 == n);
            println!(
                "N6 t {:.3}: C_D {cd:.5} C_L {cl:+.5}; Co {:.3}; largest pressure iteration count of a step so far {} at \
                 step {}; ADR 0268 R1 at this time {:?} ({:.0} s)",
                n as f64 * dt,
                info.courant,
                max_p_iters.0,
                max_p_iters.1,
                r1.map(|p| (p.1, p.2)),
                t0.elapsed().as_secs_f64()
            );
        }
    }
    let (nc, st, ps, ms, amp, cd, cl) = window(&ts, &cds, &cls);
    let e = |a: f64, r: f64| 100.0 * (a - r) / r.abs();
    let (ed, el) = (e(cd, CD_MAX_REF), e(cl, CL_MAX_REF));
    let (ed2, el2) = (e(T2_BOUNDED.0, CD_MAX_REF), e(T2_BOUNDED.1, CL_MAX_REF));
    println!(
        "RECORD N6 T bounded dt 1e-3 (default AMG, from t = 0): {nc} crossings, St {st:.5} ({:+.3} %), period spread {ps:.3e}, \
         C_L max spread {ms:.3e}, amplitude {amp:.4}; C_D max {cd:.5} ({ed:+.3} %); C_L max {cl:.5} ({el:+.3} %); A2-T-2 drag \
         {ed:.3} -> {ed2:.3}: {}; lift {el:.3} -> {el2:.3}: {}; against ADR 0363 H3 (3.22695, 1.00253, 0.30144): C_D max \
         {:+.3e}, C_L max {:+.3e}, St {:+.3e} (relative); largest pressure iteration count of a step {} at step {}",
        e(st, ST_REF),
        ed2.abs() < ed.abs(),
        el2.abs() < el.abs(),
        (cd - H3_RECORD.0) / H3_RECORD.0,
        (cl - H3_RECORD.1) / H3_RECORD.1,
        (st - H3_RECORD.2) / H3_RECORD.2,
        max_p_iters.0,
        max_p_iters.1
    );
    assert!(nc >= 6, "S2 N6: {nc} crossings");
    assert!(ps <= 1e-2, "S2 N6: period spread {ps:e}");
    assert!(ms <= 2e-2, "S2 N6: maximum spread {ms:e}");
    assert!(amp >= 1.0, "S2 N6: amplitude {amp}");
}
