//! F-474 (ADR 0369): the loss of definiteness of the AMG pressure preconditioner on the stopping system of ADR 0363 (T,
//! bounded immersed forcing, step 2951 corrector 1), checked first against the coarsest dense LU order defect of F-471.
//!
//! The heavy record H is an ignored test run once in release with `--ignored` and `F466_DIR` naming the folder of the states
//! written by ADR 0363 H1 (`w49i-work/f466-scratch/runs`). The fast check F1 (the M2 captures) is what the gate runs. The DFG
//! helpers and the checkpoint reader are copies of those of `immersed_bounded_stop_f466.rs` (not edited).

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
use rcfd_linalg::{cg, Amg, AmgSettings, CsrMatrix, LinearOperator, Preconditioner, SolveControl};
use rcfd_mesh::gen::{axis_coords, box_2d_coords, expansion_for_end_cell, Grading};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

// ---------------------------------------------------------------------------------------------------------------------------
// DFG channel (ADR 0218), copied from immersed_bounded_stop_f466.rs
// ---------------------------------------------------------------------------------------------------------------------------

const H: f64 = 0.41;
const NU: f64 = 1e-3;
const DIAM: f64 = 0.1;
const CENTRE: Vec3 = [0.2, 0.2, 0.0];
const DZ: f64 = 0.01;
const K_DFG: f64 = 1e7;

fn forcing_of(geom: &FvGeometry) -> ImmersedForcing {
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
    ImmersedForcing::build_second_order_bounded(geom, &[body], st, &gamma).unwrap()
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

fn solver(m: MeshId) -> (Arc<FvGeometry>, PimpleSolver) {
    let (xs, ys) = match m {
        MeshId::T => t_mesh(),
        MeshId::M2 => dfg_family(2),
    };
    let (geom, u, p) = dfg_fields(&xs, &ys, 1.5);
    let ib = forcing_of(&geom);
    let mut s = PimpleSolver::new(geom.clone(), u, p, settings());
    s.immersed = Some(ib);
    (geom, s)
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

fn system_of(g: &FvGeometry, c: &PressureCapture) -> FvMatrix {
    pressure_system_src(g, &c.gamma, &c.bc, &c.phi, &c.controls, c.src.as_deref()).expect("plain pressure path")
}

fn nonfinite(v: &[f64]) -> usize {
    v.iter().filter(|x| !x.is_finite()).count()
}

fn bits_differ(a: &[f64], b: &[f64]) -> usize {
    a.iter().zip(b).filter(|(x, y)| x.to_bits() != y.to_bits()).count()
}

fn opt_in() -> AmgSettings {
    AmgSettings {
        coarse_lu_interchanges_first: true,
        ..AmgSettings::default()
    }
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
// Fast check F1 (the gate)
// ---------------------------------------------------------------------------------------------------------------------------

/// ADR 0369 F1: on the pressure systems of three M2 steps, the opt-in is the default bit for bit wherever the coarsest
/// factorisation interchanges no rows; the interchanges are recorded.
#[test]
fn f1_m2_captures_opt_in_neutral_without_interchanges() {
    let (g, mut s) = solver(MeshId::M2);
    s.pressure_capture = Some(Vec::new());
    for _ in 0..3 {
        s.step(1e-3);
    }
    assert!(s.diverged.is_none());
    let caps = s.pressure_capture.take().unwrap();
    assert_eq!(caps.len(), 6);
    let mut neutral_checked = 0;
    for c in &caps {
        let pm = system_of(&g, c);
        let csr = pm.a.to_csr();
        let (def, opt) = (Amg::new(&csr, AmgSettings::default()), Amg::new(&csr, opt_in()));
        let ks = def.coarse_interchanges();
        println!(
            "F1 step {} corrector {}: levels {:?}, coarsest interchanges at {:?}",
            c.step,
            c.corrector,
            def.level_sizes(),
            ks
        );
        assert_eq!(ks, opt.coarse_interchanges());
        if ks.iter().all(|&k| k == 0) {
            let (mut x1, mut x2) = (c.p0.clone(), c.p0.clone());
            let s1 = cg(&pm.a, &pm.b, &mut x1, &def, &c.controls.solver);
            let s2 = cg(&pm.a, &pm.b, &mut x2, &opt, &c.controls.solver);
            assert_eq!(
                bits_differ(&x1, &x2),
                0,
                "F1: step {} corrector {}",
                c.step,
                c.corrector
            );
            assert_eq!(s1.iterations, s2.iterations, "F1: iterations");
            neutral_checked += 1;
        }
    }
    println!("F1: {neutral_checked} of 6 systems without an interchange at a step k >= 1 checked bit for bit");
}

// ---------------------------------------------------------------------------------------------------------------------------
// The heavy record H
// ---------------------------------------------------------------------------------------------------------------------------

fn f466_dir() -> PathBuf {
    PathBuf::from(std::env::var("F466_DIR").expect("F466_DIR must name the folder of ADR 0363 H1's states"))
}

/// Deterministic vectors of length `n` (a hash of the index and the vector number, in [-1, 1]).
fn det_vec(n: usize, k: usize) -> Vec<f64> {
    (0..n)
        .map(|i| {
            let h = (i as u64 + 1)
                .wrapping_mul(0x9E37_79B9_7F4A_7C15)
                .wrapping_add((k as u64 + 1).wrapping_mul(0xBF58_476D_1CE4_E5B9));
            let h = (h ^ (h >> 31)).wrapping_mul(0x94D0_49BB_1331_11EB);
            let h = h ^ (h >> 29);
            (h >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
        })
        .collect()
}

/// Double-double accumulation (TwoProd by a fused multiply-add, TwoSum), ADR 0369.
#[derive(Clone, Copy, Default)]
struct Dd {
    hi: f64,
    lo: f64,
}

impl Dd {
    fn add_prod(&mut self, x: f64, y: f64) {
        let p = x * y;
        let pe = x.mul_add(y, -p);
        let s = self.hi + p;
        let z = s - self.hi;
        let e = (self.hi - (s - z)) + (p - z);
        self.hi = s;
        self.lo += e + pe;
    }
    fn value(self) -> f64 {
        self.hi + self.lo
    }
}

fn dd_dot(a: &[f64], b: &[f64]) -> f64 {
    let mut s = Dd::default();
    for (x, y) in a.iter().zip(b) {
        s.add_prod(*x, *y);
    }
    s.value()
}

fn dd_matvec(a: &CsrMatrix, x: &[f64]) -> Vec<f64> {
    (0..a.n)
        .map(|i| {
            let mut s = Dd::default();
            for k in a.row_ptr[i]..a.row_ptr[i + 1] {
                s.add_prod(a.val[k], x[a.col[k]]);
            }
            s.value()
        })
        .collect()
}

fn max_abs(v: &[f64]) -> f64 {
    v.iter().fold(0.0f64, |m, x| m.max(x.abs()))
}

/// One forward Gauss-Seidel sweep (the pre-smoothing of the cycle, `amg.rs`), from the given `x`.
fn gs_forward(a: &CsrMatrix, b: &[f64], x: &mut [f64]) {
    for i in 0..a.n {
        let mut s = b[i];
        let mut d = 0.0;
        for k in a.row_ptr[i]..a.row_ptr[i + 1] {
            let j = a.col[k];
            if j == i {
                d = a.val[k];
            } else {
                s -= a.val[k] * x[j];
            }
        }
        x[i] = s / d;
    }
}

/// The right-hand side each level receives in the first descent of one application on `r` (level 0: `r` itself).
fn received(amg: &Amg, r: &[f64]) -> Vec<Vec<f64>> {
    let mut out = vec![r.to_vec()];
    for l in 0..amg.n_levels() - 1 {
        let a = amg.level_operator(l);
        let p = amg.level_prolongator(l).unwrap();
        let b = out[l].clone();
        let mut x = vec![0.0; a.n];
        gs_forward(a, &b, &mut x);
        let mut res = vec![0.0; a.n];
        a.residual(&b, &x, &mut res);
        let rt = p.transpose();
        let mut rc = vec![0.0; p.ncols];
        rt.matvec(&res, &mut rc);
        out.push(rc);
    }
    out
}

/// H3: per level, (min quotient, quotient on the received vector, max asymmetry).
fn cycle_quotients(amg: &Amg, recv: &[Vec<f64>]) -> Vec<(f64, f64, f64)> {
    (0..amg.n_levels())
        .map(|l| {
            let n = amg.level_operator(l).n;
            let apply = |v: &[f64]| {
                let mut z = vec![0.0; n];
                amg.cycle_at(l, v, &mut z);
                z
            };
            let q = |v: &[f64]| dot(v, &apply(v)) / dot(v, v);
            let q_recv = q(&recv[l]);
            let mut qmin = q_recv;
            let vs: Vec<Vec<f64>> = (0..10).map(|k| det_vec(n, 100 + k)).collect();
            for v in &vs {
                qmin = qmin.min(q(v));
            }
            let mut asym = 0.0f64;
            for pair in vs.chunks(2) {
                let (u, v) = (&pair[0], &pair[1]);
                let (bu, bv) = (apply(u), apply(v));
                let num = (dot(u, &bv) - dot(v, &bu)).abs();
                let den = norm2(u) * norm2(&bv) + norm2(v) * norm2(&bu);
                asym = asym.max(num / den);
            }
            (qmin, q_recv, asym)
        })
        .collect()
}

/// H2: the coarse solve's relative residual on `b`.
fn coarse_residual(amg: &Amg, b: &[f64]) -> f64 {
    let a = amg.level_operator(amg.n_levels() - 1);
    let mut x = vec![0.0; a.n];
    amg.coarse_solve(b, &mut x);
    let mut r = vec![0.0; a.n];
    a.residual(b, &x, &mut r);
    norm2(&r) / norm2(b)
}

/// Max |a_ij - a_ji| / max |a|.
fn asymmetry(a: &CsrMatrix) -> f64 {
    let t = a.transpose();
    let mut worst = 0.0f64;
    for i in 0..a.n {
        let row = |m: &CsrMatrix| -> Vec<(usize, f64)> {
            (m.row_ptr[i]..m.row_ptr[i + 1]).map(|k| (m.col[k], m.val[k])).collect()
        };
        let (ra, rt) = (row(a), row(&t));
        for &(j, v) in &ra {
            let w = rt.iter().find(|e| e.0 == j).map_or(0.0, |e| e.1);
            worst = worst.max((v - w).abs());
        }
        for &(j, w) in &rt {
            if !ra.iter().any(|e| e.0 == j) {
                worst = worst.max(w.abs());
            }
        }
    }
    worst / max_abs(&a.val)
}

/// The pivots of a dense Cholesky factorisation of a symmetric matrix (its lower triangle): (min pivot, max pivot, all > 0).
fn cholesky_pivots(a: &CsrMatrix) -> (f64, f64, bool) {
    let n = a.n;
    let mut m = vec![0.0; n * n];
    for i in 0..n {
        for k in a.row_ptr[i]..a.row_ptr[i + 1] {
            m[i * n + a.col[k]] = a.val[k];
        }
    }
    let (mut lo, mut hi, mut ok) = (f64::INFINITY, 0.0f64, true);
    for j in 0..n {
        let mut d = m[j * n + j];
        for k in 0..j {
            d -= m[j * n + k] * m[j * n + k];
        }
        lo = lo.min(d);
        hi = hi.max(d);
        if d <= 0.0 {
            ok = false;
            break;
        }
        let ljj = d.sqrt();
        m[j * n + j] = ljj;
        for i in j + 1..n {
            let mut s = m[i * n + j];
            for k in 0..j {
                s -= m[i * n + k] * m[j * n + k];
            }
            m[i * n + j] = s / ljj;
        }
    }
    (lo, hi, ok)
}

fn min_column_norm(p: &CsrMatrix) -> f64 {
    let mut s = vec![0.0; p.ncols];
    for k in 0..p.val.len() {
        s[p.col[k]] += p.val[k] * p.val[k];
    }
    s.iter().fold(f64::INFINITY, |m, x| m.min(x.sqrt()))
}

/// ADR 0369 H: the stopping system of ADR 0363 rebuilt from the state of step 2940, measured (H0 to H7).
#[test]
#[ignore = "heavy record (ADR 0369 H, minutes; needs ADR 0363 H1's states); run once in release with --ignored and F466_DIR set"]
fn h_stopping_system_definiteness() {
    let t0 = Instant::now();
    let dir = f466_dir();
    let (geom, mut s) = solver(MeshId::T);
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
    println!("H0: stop at step {stop:?} ({:.0} s)", t0.elapsed().as_secs_f64());
    assert_eq!(stop, Some(2951), "H0: stop step");
    let caps = s.pressure_capture.take().unwrap();
    let find = |st: usize, co: usize| {
        caps.iter()
            .find(|c| c.step == st && c.corrector == co && c.outer == 0)
            .unwrap()
    };
    let (cs, cs0, cg_) = (find(2951, 1), find(2951, 0), find(2950, 1));
    println!(
        "H0: captured S iterations {}, non-finite cells {}; S0 iterations {}; G iterations {}",
        cs.iters,
        nonfinite(&cs.p),
        cs0.iters,
        cg_.iters
    );
    assert!(nonfinite(&cs.p) > 0 && cs.iters == 3000, "H0: captured S");
    let ctl = cs.controls.solver;
    let pm = system_of(&geom, cs);
    let csr = pm.a.to_csr();
    let def = Amg::new(&csr, AmgSettings::default());
    let opt = Amg::new(&csr, opt_in());
    // H0: the default traced CG
    let mut xd = cs.p0.clone();
    let td = rz_cg(&pm.a, &pm.b, &mut xd, &def, &ctl);
    let rz1 = format!("{:.3e}", td.rz[1]);
    println!(
        "H0: default AMG-CG on S: iterations {}, converged {}, rz start {:.6e}, after iterations 1..5 {:?}; rz <= 0 at {} of {} \
         iterations; first rz <= 0 at {:?}",
        td.iterations,
        td.converged,
        td.rz[0],
        &td.rz[1..6.min(td.rz.len())],
        td.rz.iter().filter(|v| v.is_nan() || **v <= 0.0).count(),
        td.rz.len(),
        td.rz.iter().position(|v| v.is_nan() || *v <= 0.0)
    );
    assert_eq!(rz1, "-2.006e-9", "H0: rz after iteration 1");
    // H1: interchanges
    for (name, c) in [("S", cs), ("S0", cs0), ("G", cg_)] {
        let a = Amg::new(&system_of(&geom, c).a.to_csr(), AmgSettings::default());
        println!(
            "H1 {name} (step {} corrector {}): levels {:?}; coarsest interchanges {} ({} at a step k >= 1) at {:?}; pivots {:?}",
            c.step,
            c.corrector,
            a.level_sizes(),
            a.coarse_interchanges().len(),
            a.coarse_interchanges().iter().filter(|&&k| k >= 1).count(),
            a.coarse_interchanges(),
            a.coarse_pivots()
        );
    }
    // H2: the coarse solve
    let mut r0 = vec![0.0; csr.n];
    csr.residual(&pm.b, &cs.p0, &mut r0);
    let recv = received(&def, &r0);
    let nl = def.n_levels();
    let nc = def.level_operator(nl - 1).n;
    let mut h2_opt_max = 0.0f64;
    let mut h2_def = Vec::new();
    let mut vecs = vec![recv[nl - 1].clone()];
    vecs.extend((0..10).map(|k| det_vec(nc, k)));
    for v in &vecs {
        let (rd, ro) = (coarse_residual(&def, v), coarse_residual(&opt, v));
        h2_def.push(rd);
        h2_opt_max = h2_opt_max.max(ro);
    }
    println!(
        "H2: coarse solve relative residual, received vector: default {:.3e}; deterministic vectors default {:?}; opt-in max \
         over all eleven {h2_opt_max:.3e}",
        h2_def[0],
        h2_def[1..].iter().map(|v| format!("{v:.2e}")).collect::<Vec<_>>()
    );
    // H3: the cycle per level
    let qd = cycle_quotients(&def, &recv);
    let recv_o = received(&opt, &r0);
    let qo = cycle_quotients(&opt, &recv_o);
    for l in 0..nl {
        println!(
            "H3 level {l} ({} unknowns): default min quotient {:.4e}, on the received vector {:.4e}, asymmetry {:.3e}; opt-in min \
             {:.4e}, received {:.4e}, asymmetry {:.3e}",
            def.level_operator(l).n,
            qd[l].0,
            qd[l].1,
            qd[l].2,
            qo[l].0,
            qo[l].1,
            qo[l].2
        );
    }
    let first_bad = (0..nl).rev().find(|&l| qd[l].0 <= 0.0 || qd[l].2 > 1e-8);
    println!(
        "H3: coarsest level where the default cycle is indefinite or asymmetric: {first_bad:?}; finest such: {:?}",
        (0..nl).find(|&l| qd[l].0 <= 0.0 || qd[l].2 > 1e-8)
    );
    // H4: the coarse operators and the prolongators against double-double
    let mut galerkin_max = 0.0f64;
    let mut sym_max = 0.0f64;
    for l in 0..nl {
        let a = def.level_operator(l);
        let sym = asymmetry(a);
        sym_max = sym_max.max(sym);
        let qa = (0..10)
            .map(|k| {
                let v = det_vec(a.n, 200 + k);
                dd_dot(&v, &dd_matvec(a, &v)) / dd_dot(&v, &v)
            })
            .fold(f64::INFINITY, f64::min);
        let mut line = format!("H4 level {l}: symmetry {sym:.3e}; min dd quotient v^T A v / v^T v {qa:.4e}");
        if let Some(p) = def.level_prolongator(l) {
            let ac = def.level_operator(l + 1);
            let rt = p.transpose();
            let mut gd = 0.0f64;
            for k in 0..3 {
                let v = det_vec(p.ncols, 300 + k);
                let pv = dd_matvec(p, &v);
                let apv = dd_matvec(a, &pv);
                let ref_ = dd_matvec(&rt, &apv);
                let mut got = vec![0.0; ac.n];
                ac.matvec(&v, &mut got);
                let d = got.iter().zip(&ref_).map(|(x, y)| (x - y).abs()).fold(0.0, f64::max) / max_abs(&ref_);
                gd = gd.max(d);
            }
            galerkin_max = galerkin_max.max(gd);
            line.push_str(&format!(
                "; Galerkin defect {gd:.3e}; prolongator min column norm {:.4e}",
                min_column_norm(p)
            ));
        } else {
            let (lo, hi, ok) = cholesky_pivots(a);
            line.push_str(&format!("; Cholesky pivots {lo:.4e} to {hi:.4e}, all positive {ok}"));
        }
        println!("{line}");
    }
    // H5, H6: the repair
    let mut xo = cs.p0.clone();
    let to = rz_cg(&pm.a, &pm.b, &mut xo, &opt, &ctl);
    let to_pos = to.rz.iter().all(|v| *v > 0.0);
    println!(
        "H5: opt-in AMG-CG on S: iterations {}, converged {}, every rz > 0 {to_pos}, min rz {:.3e}",
        to.iterations,
        to.converged,
        to.rz.iter().fold(f64::INFINITY, |m, x| m.min(*x))
    );
    let pm0 = system_of(&geom, cs0);
    let opt0 = Amg::new(&pm0.a.to_csr(), opt_in());
    let mut x0 = cs0.p0.clone();
    let t0r = rz_cg(&pm0.a, &pm0.b, &mut x0, &opt0, &cs0.controls.solver);
    let t0_pos = t0r.rz.iter().all(|v| *v > 0.0);
    println!(
        "H6: opt-in AMG-CG on S0: iterations {} (default captured {}), converged {}, every rz > 0 {t0_pos}",
        t0r.iterations, cs0.iters, t0r.converged
    );
    // H7: against IC-CG
    let mut xi = cs.p0.clone();
    let si = pm.solve(&mut xi, &ctl);
    let dmax = xo.iter().zip(&xi).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max);
    let h7 = dmax / max_abs(&xi);
    println!(
        "H7: IC-CG on S: iterations {}, converged {}; max |p_amg - p_ic| / max |p_ic| = {h7:.3e} ({:.0} s)",
        si.iterations,
        si.converged,
        t0.elapsed().as_secs_f64()
    );
    // the registered limits
    assert!(h2_opt_max <= 1e-12, "H2: opt-in coarse residual {h2_opt_max:e}");
    for (l, q) in qo.iter().enumerate() {
        assert!(q.0 > 0.0 && q.1 > 0.0, "H3: opt-in level {l} quotient {q:?}");
        assert!(q.2 <= 1e-8, "H3: opt-in level {l} asymmetry {}", q.2);
    }
    assert!(galerkin_max <= 1e-12, "H4: Galerkin defect {galerkin_max:e}");
    assert!(sym_max <= 1e-12, "H4: symmetry {sym_max:e}");
    assert!(to.converged && to_pos && to.iterations <= 100, "H5: {to:?}");
    assert!(t0r.converged && t0_pos && t0r.iterations <= 100, "H6: {t0r:?}");
    assert!(si.converged && h7 <= 1e-4, "H7: {h7:e}");
}
