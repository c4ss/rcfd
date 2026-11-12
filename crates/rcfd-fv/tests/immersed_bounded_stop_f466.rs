//! F-466 (ADR 0363): the stop of DFG 2D-2 on the T mesh with `ImmersedForcing::build_second_order_bounded` (ADR 0268 R1,
//! step 2951, pressure NaN in the pressure solve).
//!
//! The heavy records H1 (R1 again from `t = 0`, with per-step records and the state written to disk) and H2 (a restart from
//! the state of step 2900, the pressure systems of the last steps captured and examined) are ignored tests run once in release
//! with `--ignored` and the environment variable `F466_DIR` naming the folder of their files. The fast checks S1 to S6 (the
//! restart through a file, the capture, its replay, the traced conjugate gradients, the traced AMG cycle, the classifier) are
//! what the gate runs. The DFG helpers are copies of those of `immersed_dfg_convergence_f354.rs` (not edited).

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
use rcfd_linalg::{Amg, AmgSettings, LinearOperator, Preconditioner, SolveControl};
use rcfd_mesh::gen::{axis_coords, box_2d_coords, expansion_for_end_cell, Grading};
use std::io::{Read, Write};
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
// The state on disk (ADR 0363 reading 4): a little-endian dump of `PimpleState`
// ---------------------------------------------------------------------------------------------------------------------------

const MAGIC: &[u8; 8] = b"F466CKP1";

struct Out(Vec<u8>);

impl Out {
    fn u64(&mut self, v: u64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn f64(&mut self, v: f64) {
        self.0.extend_from_slice(&v.to_bits().to_le_bytes());
    }
    fn vec(&mut self, v: &[f64]) {
        self.u64(v.len() as u64);
        for &x in v {
            self.f64(x);
        }
    }
    fn opt_vec(&mut self, v: Option<&Vec<f64>>) {
        match v {
            Some(v) => {
                self.u64(1);
                self.vec(v);
            }
            None => self.u64(0),
        }
    }
    fn opt_f64(&mut self, v: Option<f64>) {
        match v {
            Some(v) => {
                self.u64(1);
                self.f64(v);
            }
            None => self.u64(0),
        }
    }
    fn history(&mut self, h: &History) {
        self.vec(&h.old);
        self.opt_vec(h.older.as_ref());
        self.opt_f64(h.dt_old);
        self.opt_vec(h.oldest.as_ref());
        self.opt_f64(h.dt_older);
    }
}

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

fn write_state(path: &Path, st: &PimpleState) {
    let mut o = Out(Vec::new());
    o.0.extend_from_slice(MAGIC);
    let flat: Vec<f64> = st.u.iter().flat_map(|v| v.iter().copied()).collect();
    o.vec(&flat);
    o.vec(&st.p);
    o.vec(&st.phi);
    o.f64(st.time);
    o.u64(st.step_no as u64);
    for h in &st.hist_u {
        o.history(h);
    }
    o.history(&st.phi_hist);
    match st.last {
        Some(i) => {
            o.u64(1);
            o.u64(i.step as u64);
            o.f64(i.time);
            o.f64(i.dt);
            o.f64(i.courant);
            o.f64(i.continuity);
            o.f64(i.res_u);
            o.f64(i.res_p);
            o.u64(i.p_iters as u64);
        }
        None => o.u64(0),
    }
    let mut f = std::fs::File::create(path).unwrap();
    f.write_all(&o.0).unwrap();
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

// ---------------------------------------------------------------------------------------------------------------------------
// The traced conjugate gradients (ADR 0363 D3): `rcfd_linalg::cg` by the same operations, with a record per iteration
// ---------------------------------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
struct CgIter {
    pq: f64,
    alpha: f64,
    res: f64,
    rz: f64,
    beta: f64,
}

#[derive(Debug, Default)]
struct CgTrace {
    iterations: usize,
    converged: bool,
    res0: f64,
    iters: Vec<CgIter>,
    /// The first non-finite quantity: (iteration, name).
    first_bad: Option<(usize, &'static str)>,
    /// The first iteration whose preconditioner output was not finite for a finite residual, with the AMG trace of it.
    precond_bad: Option<(usize, Option<(usize, &'static str)>)>,
}

fn traced_cg<A: LinearOperator>(a: &A, b: &[f64], x: &mut [f64], m: &Amg, c: &SolveControl) -> CgTrace {
    let n = a.size();
    let mut t = CgTrace::default();
    let finite = |v: &[f64]| v.iter().all(|x| x.is_finite());
    let note = |t: &mut CgTrace, it: usize, ok: bool, name: &'static str| {
        if !ok && t.first_bad.is_none() {
            t.first_bad = Some((it, name));
        }
    };
    let done = |res: f64, res0: f64| res <= c.tol_abs || res <= c.tol_rel * res0;
    let (mut r, mut z, mut p, mut q) = (vec![0.0; n], vec![0.0; n], vec![0.0; n], vec![0.0; n]);
    a.apply(x, &mut r);
    for (ri, bi) in r.iter_mut().zip(b) {
        *ri = bi - *ri;
    }
    let res0 = norm2(&r);
    t.res0 = res0;
    note(&mut t, 0, res0.is_finite(), "initial residual");
    let mut res = res0;
    if done(res, res0) {
        t.converged = true;
        return t;
    }
    m.apply(&r, &mut z);
    if finite(&r) && !finite(&z) && t.precond_bad.is_none() {
        let mut zz = vec![0.0; n];
        t.precond_bad = Some((0, m.apply_traced(&r, &mut zz)));
    }
    note(&mut t, 0, finite(&z), "z");
    p.copy_from_slice(&z);
    let mut rz = dot(&r, &z);
    note(&mut t, 0, rz.is_finite(), "rz");
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
        res = norm2(&r);
        note(&mut t, it, pq.is_finite(), "pq");
        note(&mut t, it, alpha.is_finite(), "alpha");
        note(&mut t, it, finite(x), "x");
        note(&mut t, it, res.is_finite(), "residual norm");
        if done(res, res0) {
            t.iters.push(CgIter {
                pq,
                alpha,
                res,
                rz,
                beta: f64::NAN,
            });
            t.iterations = it;
            t.converged = true;
            return t;
        }
        m.apply(&r, &mut z);
        if finite(&r) && !finite(&z) && t.precond_bad.is_none() {
            let mut zz = vec![0.0; n];
            t.precond_bad = Some((it, m.apply_traced(&r, &mut zz)));
        }
        note(&mut t, it, finite(&z), "z");
        let rz_new = dot(&r, &z);
        let beta = rz_new / rz;
        note(&mut t, it, rz_new.is_finite(), "rz");
        note(&mut t, it, beta.is_finite(), "beta");
        t.iters.push(CgIter {
            pq,
            alpha,
            res,
            rz: rz_new,
            beta,
        });
        rz = rz_new;
        xpby(&z, beta, &mut p);
    }
    t.iterations = it;
    t.converged = false;
    t
}

// ---------------------------------------------------------------------------------------------------------------------------
// The class G of ADR 0363
// ---------------------------------------------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Debug)]
enum Class {
    Sudden,
    Growth,
    Intermediate,
}

/// ADR 0363 class G from the Courant number of step 2950 and `max |u|`(2950) / `max |u|`(2000).
fn classify(courant: f64, ratio: f64) -> Class {
    if courant <= 1.5 && ratio <= 2.0 {
        Class::Sudden
    } else if courant >= 10.0 || ratio >= 5.0 {
        Class::Growth
    } else {
        Class::Intermediate
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// Per-step records
// ---------------------------------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
struct StepRec {
    step: usize,
    courant: f64,
    p_iters: usize,
    res_p: f64,
    umax: f64,
    umax_cell: usize,
    pmax: f64,
}

fn step_rec(n: usize, info: &StepInfo, s: &PimpleSolver) -> StepRec {
    let (mut umax, mut cell) = (0.0f64, 0);
    for (c, v) in s.u.values.iter().enumerate() {
        let m = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        if m > umax {
            umax = m;
            cell = c;
        }
    }
    StepRec {
        step: n,
        courant: info.courant,
        p_iters: info.p_iters,
        res_p: info.res_p,
        umax,
        umax_cell: cell,
        pmax: s.p.values.iter().fold(0.0f64, |m, x| m.max(x.abs())),
    }
}

fn write_csv(path: &Path, recs: &[StepRec]) {
    let mut s = String::from("step,courant,courant_bits,p_iters,res_p,umax,umax_cell,pmax\n");
    for r in recs {
        s.push_str(&format!(
            "{},{:e},{:016x},{},{:e},{:e},{},{:e}\n",
            r.step,
            r.courant,
            r.courant.to_bits(),
            r.p_iters,
            r.res_p,
            r.umax,
            r.umax_cell,
            r.pmax
        ));
    }
    std::fs::write(path, s).unwrap();
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

fn f466_dir() -> PathBuf {
    let d = PathBuf::from(std::env::var("F466_DIR").expect("F466_DIR must name the folder of the H1/H2 files"));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// The checkpoint steps of H1.
fn checkpoints() -> Vec<usize> {
    let mut v: Vec<usize> = (2000..=2900).step_by(100).collect();
    v.extend([2940, 2950]);
    v
}

// ---------------------------------------------------------------------------------------------------------------------------
// The fast checks S1 to S6 (the gate)
// ---------------------------------------------------------------------------------------------------------------------------

/// ADR 0363 S1: the restart through a file is bit-exact (M2, bounded forcing, 4 steps against 2 + file + 2).
#[test]
fn s1_restart_through_a_file() {
    let dt = 1e-3;
    let (_, _, mut a) = solver(MeshId::M2, Forcing::Bounded);
    for _ in 0..4 {
        a.step(dt);
    }
    let (_, _, mut b) = solver(MeshId::M2, Forcing::Bounded);
    for _ in 0..2 {
        b.step(dt);
    }
    let path = std::env::temp_dir().join(format!("f466_s1_{}.bin", std::process::id()));
    write_state(&path, &b.snapshot());
    drop(b);
    let (_, _, mut c) = solver(MeshId::M2, Forcing::Bounded);
    c.restore(&read_state(&path)).unwrap();
    let _ = std::fs::remove_file(&path);
    for _ in 0..2 {
        c.step(dt);
    }
    assert!(a.diverged.is_none() && c.diverged.is_none());
    assert_eq!(state_diff(&a, &c), 0, "S1: u, p, phi after the restart differ");
    assert_eq!(a.time.to_bits(), c.time.to_bits(), "S1: time");
    assert_eq!(a.snapshot().step_no, c.snapshot().step_no, "S1: step counter");
}

/// Three steps of M2 with the bounded forcing, with or without the capture.
fn captured_m2() -> (Arc<FvGeometry>, PimpleSolver) {
    let (g, _, mut s) = solver(MeshId::M2, Forcing::Bounded);
    s.pressure_capture = Some(Vec::new());
    for _ in 0..3 {
        s.step(1e-3);
    }
    (g, s)
}

/// ADR 0363 S2: the capture changes nothing and records every pressure solve.
#[test]
fn s2_capture_is_neutral_and_complete() {
    let (_, s) = captured_m2();
    let (_, _, mut plain) = solver(MeshId::M2, Forcing::Bounded);
    for _ in 0..3 {
        plain.step(1e-3);
    }
    assert!(plain.pressure_capture.is_none());
    assert_eq!(state_diff(&s, &plain), 0, "S2: the capture changed the run");
    let cap = s.pressure_capture.as_ref().unwrap();
    let labels: Vec<(usize, usize, usize)> = cap.iter().map(|c| (c.step, c.outer, c.corrector)).collect();
    assert_eq!(
        labels,
        vec![(1, 0, 0), (1, 0, 1), (2, 0, 0), (2, 0, 1), (3, 0, 0), (3, 0, 1)],
        "S2: captures"
    );
    assert!(cap.iter().all(|c| c.src.is_some()), "S2: a capture without the source");
}

fn system_of(g: &FvGeometry, c: &PressureCapture) -> FvMatrix {
    pressure_system_src(g, &c.gamma, &c.bc, &c.phi, &c.controls, c.src.as_deref()).expect("plain pressure path")
}

/// ADR 0363 S3: every captured solve replays bit for bit from its start pressure.
#[test]
fn s3_replay_is_bit_exact() {
    let (g, s) = captured_m2();
    for c in s.pressure_capture.as_ref().unwrap() {
        let pm = system_of(&g, c);
        let mut x = c.p0.clone();
        let st = pm.solve_amg(&mut x, &c.controls.solver);
        assert_eq!(
            bits_equal(&x, &c.p),
            0,
            "S3: step {} corrector {}: replay differs",
            c.step,
            c.corrector
        );
        assert_eq!(
            st.iterations, c.iters,
            "S3: step {} corrector {}: iterations",
            c.step, c.corrector
        );
    }
}

/// ADR 0363 S4: the traced CG is `rcfd_linalg::cg` (via `solve_amg`) bit for bit.
#[test]
fn s4_traced_cg_is_cg() {
    let (g, s) = captured_m2();
    let c = &s.pressure_capture.as_ref().unwrap()[5];
    assert_eq!((c.step, c.corrector), (3, 1));
    let pm = system_of(&g, c);
    let mut x1 = c.p0.clone();
    let st = pm.solve_amg(&mut x1, &c.controls.solver);
    let amg = Amg::new(&pm.a.to_csr(), AmgSettings::default());
    let mut x2 = c.p0.clone();
    let t = traced_cg(&pm.a, &pm.b, &mut x2, &amg, &c.controls.solver);
    assert_eq!(bits_equal(&x1, &x2), 0, "S4: traced CG differs");
    assert_eq!(t.iterations, st.iterations, "S4: iterations");
    assert!(t.converged && t.first_bad.is_none() && t.precond_bad.is_none());
}

/// ADR 0363 S5: the traced AMG application is the application bit for bit and reports nothing on a finite vector.
#[test]
fn s5_traced_amg_is_amg() {
    let (g, s) = captured_m2();
    let c = &s.pressure_capture.as_ref().unwrap()[5];
    let pm = system_of(&g, c);
    let amg = Amg::new(&pm.a.to_csr(), AmgSettings::default());
    let (mut z1, mut z2) = (vec![1.0; pm.b.len()], vec![2.0; pm.b.len()]);
    amg.apply(&pm.b, &mut z1);
    let first = amg.apply_traced(&pm.b, &mut z2);
    assert_eq!(bits_equal(&z1, &z2), 0, "S5: traced AMG differs");
    assert_eq!(first, None, "S5: a finite vector reported");
    assert!(amg.level_sizes().len() >= 2 && amg.coarse_pivots().is_some());
}

/// ADR 0363 S6: the classifier on given numbers.
#[test]
fn s6_classifier() {
    assert_eq!(classify(1.2, 1.5), Class::Sudden);
    assert_eq!(classify(1.2, 2.5), Class::Intermediate);
    assert_eq!(classify(12.0, 1.0), Class::Growth);
    assert_eq!(classify(1.2, 6.0), Class::Growth);
    assert_eq!(classify(3.0, 2.5), Class::Intermediate);
}

// ---------------------------------------------------------------------------------------------------------------------------
// The heavy records H1 and H2
// ---------------------------------------------------------------------------------------------------------------------------

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

/// ADR 0363 H1: ADR 0268 R1 again from t = 0 (T, bounded, dt 1e-3), per-step records, the state written at the checkpoint
/// steps; M1 (the reproduction from t = 0) and the class G.
#[test]
#[ignore = "heavy record (ADR 0363 H1, 0.5 to 1 h); run once in release with --ignored and F466_DIR set"]
fn h1_t_bounded_reproduce_and_checkpoint() {
    let dir = f466_dir();
    let dt = 1e-3;
    let (geom, ib, mut s) = solver(MeshId::T, Forcing::Bounded);
    let images = ib.flux_faces.iter().filter(|f| f.image.is_some()).count();
    println!(
        "H1 T Bounded dt 1e-3: {} cells, {} solid cells, {} flux faces, {images} with an image",
        geom.n_cells(),
        ib.bodies[0].solid_cells.len(),
        ib.flux_faces.len()
    );
    let cps = checkpoints();
    let t0 = Instant::now();
    let mut recs: Vec<StepRec> = Vec::new();
    let (mut ts, mut cds, mut cls) = (Vec::new(), Vec::new(), Vec::new());
    let mut printed: Vec<(usize, f64, f64)> = Vec::new();
    let mut stop: Option<(usize, String)> = None;
    for n in 1..=8000 {
        let info = s.step(dt);
        if let Some(d) = &s.diverged {
            let msg = d.message(Some(geom.mesh.cell_centres()));
            println!("H1: stopped at step {n}: {msg}");
            stop = Some((n, msg));
            break;
        }
        recs.push(step_rec(n, &info, &s));
        let (cd, cl) = coefficients(&ib, &s);
        ts.push(n as f64 * dt);
        cds.push(cd);
        cls.push(cl);
        if n % 1000 == 0 {
            println!(
                "H1 t {:.3}: C_D {cd:.5} C_L {cl:+.5}; Co {:.3} ({:.0} s)",
                n as f64 * dt,
                info.courant,
                t0.elapsed().as_secs_f64()
            );
            printed.push((n, cd, cl));
        }
        if cps.contains(&n) {
            write_state(&dir.join(format!("state_{n}.bin")), &s.snapshot());
            write_csv(&dir.join("h1_steps.csv"), &recs);
            println!("H1: state of step {n} written ({:.0} s)", t0.elapsed().as_secs_f64());
        }
    }
    write_csv(&dir.join("h1_steps.csv"), &recs);
    println!(
        "H1: {} steps recorded in {:.0} s",
        recs.len(),
        t0.elapsed().as_secs_f64()
    );
    // the steps before the stop, for the class
    for r in recs.iter().filter(|r| r.step >= 2900 || r.step % 100 == 0) {
        println!(
            "H1 step {}: Co {:.6e} p_iters {} res_p {:.3e} max|u| {:.6e} (cell {}) max|p| {:.6e}",
            r.step, r.courant, r.p_iters, r.res_p, r.umax, r.umax_cell, r.pmax
        );
    }
    let at = |k: usize| recs.iter().find(|r| r.step == k).copied();
    if let (Some(a), Some(b)) = (at(2000), at(2950)) {
        let ratio = b.umax / a.umax;
        let class = classify(b.courant, ratio);
        println!(
            "H1 class G: Courant(2950) {:.6}, max|u|(2950)/max|u|(2000) = {:.6e}/{:.6e} = {ratio:.4}: {class:?}",
            b.courant, b.umax, a.umax
        );
        if class == Class::Growth {
            let cc = geom.mesh.cell_centres();
            if let Some(o) = recs.iter().find(|r| r.step > 2000 && r.courant > 1.5) {
                let x = cc[o.umax_cell];
                let dist = ((x[0] - CENTRE[0]).powi(2) + (x[1] - CENTRE[1]).powi(2)).sqrt();
                let fluid = ib.flux_faces.iter().any(|f| f.fluid == o.umax_cell);
                let image = ib
                    .flux_faces
                    .iter()
                    .any(|f| matches!(f.image, Some((d, _)) if d == o.umax_cell));
                println!(
                    "H1 onset: step {}, Courant {:.4}, cell {} at ({:.5}, {:.5}), {dist:.5} from the centre; fluid cell of a \
                     flux face {fluid}, image cell {image}",
                    o.step, o.courant, o.umax_cell, x[0], x[1]
                );
            }
        }
    }
    match &stop {
        Some((n, msg)) => {
            println!("H1 M1: stop at step {n}; message equal to R1's: {}", msg == R1_MESSAGE);
        }
        None => {
            let (nc, st, ps, ms, amp, cd, cl) = window(&ts, &cds, &cls);
            let e = |a: f64, r: f64| 100.0 * (a - r) / r.abs();
            let et2 = (e(T2_BOUNDED.0, CD_MAX_REF), e(T2_BOUNDED.1, CL_MAX_REF));
            println!(
                "H1 (not stopped): {nc} crossings, St {st:.5} ({:+.3} %), period spread {ps:.3e}, max spread {ms:.3e}, \
                 amplitude {amp:.4}; C_D max {cd:.5} ({:+.3} %), C_L max {cl:.5} ({:+.3} %); A2-T-2 drag {}, lift {}",
                e(st, ST_REF),
                e(cd, CD_MAX_REF),
                e(cl, CL_MAX_REF),
                et2.0.abs() < e(cd, CD_MAX_REF).abs(),
                et2.1.abs() < e(cl, CL_MAX_REF).abs()
            );
        }
    }
    for (k, cd, cl) in R1_PRINTED {
        let got = printed.iter().find(|p| p.0 == k).copied();
        let same = got.is_some_and(|(_, a, b)| {
            format!("{a:.5}") == format!("{cd:.5}") && format!("{b:+.5}") == format!("{cl:+.5}")
        });
        println!(
            "H1 M1: t = {}: {got:?} against R1 ({cd:.5}, {cl:+.5}): {same}",
            k / 1000
        );
        assert!(same, "M1: t = {}: {got:?} against R1 ({cd}, {cl})", k / 1000);
    }
    let (n, msg) = stop.expect("M1: H1 did not stop (the stop of R1 is not reproduced)");
    assert_eq!(n, 2951, "M1: stop step");
    assert_eq!(msg, R1_MESSAGE, "M1: stop message");
}

fn nonfinite(v: &[f64]) -> usize {
    v.iter().filter(|x| !x.is_finite()).count()
}

/// ADR 0363 H2: a fresh solver from the state of step 2900 (written by H1), the steps to the stop with the pressure systems of
/// steps 2949 on captured; M2 (the reproduction from a restart) and M3 to M7 (the inputs, the extent, the iteration, the
/// preconditioner or the system, the source of the solid cells).
#[test]
#[ignore = "heavy record (ADR 0363 H2, minutes; needs H1's files); run once in release with --ignored and F466_DIR set"]
fn h2_restart_capture_analysis() {
    let dir = f466_dir();
    let dt = 1e-3;
    let h1 = read_csv_courant(&dir.join("h1_steps.csv"));
    let (geom, _, mut s) = solver(MeshId::T, Forcing::Bounded);
    s.restore(&read_state(&dir.join("state_2900.bin"))).unwrap();
    let mut courant_diff = 0;
    let mut stop: Option<(usize, String)> = None;
    for n in 2901..=2960 {
        if n == 2949 {
            s.pressure_capture = Some(Vec::new());
        }
        let info = s.step(dt);
        if let Some(d) = &s.diverged {
            let msg = d.message(Some(geom.mesh.cell_centres()));
            println!("H2: stopped at step {n}: {msg}");
            stop = Some((n, msg));
            break;
        }
        match h1.iter().find(|r| r.0 == n) {
            Some(&(_, bits)) if bits == info.courant.to_bits() => {}
            other => {
                courant_diff += 1;
                println!("H2: step {n}: Courant {:e} against H1 {other:?}", info.courant);
            }
        }
    }
    println!(
        "H2 M2: stop {:?}; Courant numbers differing from H1 at steps 2901 to the stop: {courant_diff}",
        stop.as_ref().map(|s| s.0)
    );
    let caps = s.pressure_capture.take().unwrap_or_default();
    let n_cells = geom.n_cells();
    let cc = geom.mesh.cell_centres();
    // M7 and the overview of every captured solve
    for c in &caps {
        let src = c.src.as_deref().unwrap_or(&[]);
        let (mut smax, mut scell) = (0.0f64, 0);
        for (i, v) in src.iter().enumerate() {
            if v.abs() > smax {
                smax = v.abs();
                scell = i;
            }
        }
        let ssum: f64 = src.iter().sum();
        println!(
            "H2 capture step {} outer {} corrector {}: iters {} res0 {:.4e}; non-finite gamma {} phi {} src {} p0 {} p {}; \
             max|src| {smax:.6e} at cell {scell} ({:.5}, {:.5}); sum src {ssum:.3e} ({:.3e} of max); max|p0| {:.6e}",
            c.step,
            c.outer,
            c.corrector,
            c.iters,
            c.res0,
            nonfinite(&c.gamma),
            nonfinite(&c.phi),
            nonfinite(src),
            nonfinite(&c.p0),
            nonfinite(&c.p),
            cc[scell][0],
            cc[scell][1],
            if smax > 0.0 { ssum.abs() / smax } else { 0.0 },
            c.p0.iter().fold(0.0f64, |m, x| m.max(x.abs()))
        );
    }
    let Some(bad) = caps.iter().find(|c| nonfinite(&c.p) > 0) else {
        println!("H2: no captured solve returned a non-finite pressure");
        assert!(stop.is_some(), "M2: H2 did not stop");
        return;
    };
    println!(
        "H2 M3 (step {} corrector {}): non-finite gamma {}, phi {}, src {}, p0 {}",
        bad.step,
        bad.corrector,
        nonfinite(&bad.gamma),
        nonfinite(&bad.phi),
        nonfinite(bad.src.as_deref().unwrap_or(&[])),
        nonfinite(&bad.p0)
    );
    let nb = nonfinite(&bad.p);
    println!(
        "H2 M4: non-finite cells in the solution {nb} of {n_cells} (all: {})",
        nb == n_cells
    );
    // the system and its replay
    let pm = system_of(&geom, bad);
    let mut xr = bad.p0.clone();
    let st = pm.solve_amg(&mut xr, &bad.controls.solver);
    let same_pattern = xr
        .iter()
        .zip(&bad.p)
        .all(|(a, b)| (a.is_finite() == b.is_finite()) && (!a.is_finite() || a.to_bits() == b.to_bits()));
    println!(
        "H2 replay: iterations {} (captured {}), converged {}, same solution (non-finite pattern and finite bits) {same_pattern}",
        st.iterations, bad.iters, st.converged
    );
    // M5: the traced CG
    let amg = Amg::new(&pm.a.to_csr(), AmgSettings::default());
    let mut xt = bad.p0.clone();
    let t = traced_cg(&pm.a, &pm.b, &mut xt, &amg, &bad.controls.solver);
    println!(
        "H2 M5: traced CG: iterations {}, converged {}, res0 {:.6e}, first non-finite {:?}, preconditioner {:?}",
        t.iterations, t.converged, t.res0, t.first_bad, t.precond_bad
    );
    let k = t.iters.len();
    for (i, r) in t.iters.iter().enumerate() {
        if i < 5 || i + 8 >= k {
            println!(
                "H2 M5 iteration {}: pq {:.6e} alpha {:.6e} res {:.6e} rz {:.6e} beta {:.6e}",
                i + 1,
                r.pq,
                r.alpha,
                r.res,
                r.rz,
                r.beta
            );
        }
    }
    // M6: the system and the preconditioner
    let a = &pm.a;
    let diag_bad = a.diag.iter().filter(|d| d.is_nan() || **d <= 0.0).count();
    let off_pos = a.lower.iter().chain(&a.upper).filter(|v| **v > 0.0).count();
    let asym = a
        .lower
        .iter()
        .zip(&a.upper)
        .filter(|(l, u)| l.to_bits() != u.to_bits())
        .count();
    let n_int = geom.n_internal;
    let gi = &bad.gamma[..n_int];
    let (gmin, gmax) = gi
        .iter()
        .fold((f64::INFINITY, 0.0f64), |(lo, hi), g| (lo.min(*g), hi.max(*g)));
    let rhs_bad = nonfinite(&pm.b);
    println!(
        "H2 M6 matrix: diagonal not positive {diag_bad}, off-diagonal positive {off_pos}, asymmetric pairs {asym}; internal \
         face coefficient {gmin:.6e} to {gmax:.6e}; non-finite right-hand side {rhs_bad}; max|b| {:.6e}",
        pm.b.iter().fold(0.0f64, |m, x| m.max(x.abs()))
    );
    println!(
        "H2 M6 AMG: level sizes {:?}; min diagonals {:?}; coarse pivots (min, max, guarded) {:?}",
        amg.level_sizes(),
        amg.level_min_diagonals(),
        amg.coarse_pivots()
    );
    let mut xi = bad.p0.clone();
    let si = pm.solve(&mut xi, &bad.controls.solver);
    println!(
        "H2 M6 IC-CG: iterations {}, converged {}, final residual {:.3e}, non-finite cells {}",
        si.iterations,
        si.converged,
        si.final_residual,
        nonfinite(&xi)
    );
    // the healthy solve one step before, for comparison
    if let Some(good) = caps
        .iter()
        .rfind(|c| c.step + 1 == bad.step && c.corrector == bad.corrector)
    {
        let pg = system_of(&geom, good);
        let ag = Amg::new(&pg.a.to_csr(), AmgSettings::default());
        println!(
            "H2 M6 AMG of step {} corrector {}: level sizes {:?}; min diagonals {:?}; coarse pivots {:?}",
            good.step,
            good.corrector,
            ag.level_sizes(),
            ag.level_min_diagonals(),
            ag.coarse_pivots()
        );
    }
    let (n, msg) = stop.expect("M2: H2 did not stop");
    assert_eq!(n, 2951, "M2: stop step");
    assert_eq!(msg, R1_MESSAGE, "M2: stop message");
    assert_eq!(courant_diff, 0, "M2: Courant numbers differ from H1");
}

// ---------------------------------------------------------------------------------------------------------------------------
// Amendment 1: the opt-in incomplete-Cholesky repeat of a non-finite pressure solve (S7, H3a, H3)
// ---------------------------------------------------------------------------------------------------------------------------

/// ADR 0363 amendment 1 S7: with `pressure_ic_fallback` on, a run whose solves are finite is the default bit for bit.
#[test]
fn s7_fallback_is_neutral_on_finite_solves() {
    let (_, _, mut on) = solver(MeshId::M2, Forcing::Bounded);
    on.settings.pressure_ic_fallback = true;
    let (_, _, mut off) = solver(MeshId::M2, Forcing::Bounded);
    for _ in 0..3 {
        on.step(1e-3);
        off.step(1e-3);
    }
    assert!(on.diverged.is_none() && off.diverged.is_none());
    assert_eq!(state_diff(&on, &off), 0, "S7: the option changed a finite run");
    assert!(
        on.pressure_fallbacks.is_empty(),
        "S7: repeats {:?}",
        on.pressure_fallbacks
    );
}

/// T with the bounded forcing restored from H1's state of step 2950, with the option on.
fn restored_2950_with_fallback() -> (Arc<FvGeometry>, ImmersedForcing, PimpleSolver) {
    let dir = f466_dir();
    let (geom, ib, mut s) = solver(MeshId::T, Forcing::Bounded);
    s.settings.pressure_ic_fallback = true;
    s.restore(&read_state(&dir.join("state_2950.bin"))).unwrap();
    (geom, ib, s)
}

/// ADR 0363 amendment 1 H3a: from the state of step 2950 with the option, step 2951 does not stop and is repeated once, at
/// corrector 1.
#[test]
#[ignore = "needs H1's files (ADR 0363 H3a, seconds); run in release with --ignored and F466_DIR set"]
fn h3a_fallback_at_step_2951() {
    let (geom, _, mut s) = restored_2950_with_fallback();
    s.step(1e-3);
    if let Some(d) = &s.diverged {
        panic!("H3a: stopped: {}", d.message(Some(geom.mesh.cell_centres())));
    }
    println!("H3a: repeats {:?}", s.pressure_fallbacks);
    assert_eq!(s.pressure_fallbacks, vec![(2951, 0, 1)], "H3a: repeats");
    assert_eq!(nonfinite(&s.p.values), 0, "H3a: pressure");
}

/// ADR 0363 amendment 1 H3: from the state of step 2950 with the option to t = 8; S2 of ADR 0268 and A2-T-2.
#[test]
#[ignore = "heavy record (ADR 0363 H3, 0.5 to 2.5 h; needs H1's files); run once in release with --ignored and F466_DIR set"]
fn h3_t_bounded_to_t8_with_fallback() {
    let dt = 1e-3;
    let (geom, ib, mut s) = restored_2950_with_fallback();
    let t0 = Instant::now();
    let (mut ts, mut cds, mut cls) = (Vec::new(), Vec::new(), Vec::new());
    for n in 2951..=8000 {
        let info = s.step(dt);
        if let Some(d) = &s.diverged {
            panic!("H3: stopped at step {n}: {}", d.message(Some(geom.mesh.cell_centres())));
        }
        let (cd, cl) = coefficients(&ib, &s);
        ts.push(n as f64 * dt);
        cds.push(cd);
        cls.push(cl);
        if n % 1000 == 0 {
            println!(
                "H3 t {:.3}: C_D {cd:.5} C_L {cl:+.5}; Co {:.3}; repeats so far {} ({:.0} s)",
                n as f64 * dt,
                info.courant,
                s.pressure_fallbacks.len(),
                t0.elapsed().as_secs_f64()
            );
        }
    }
    println!(
        "H3: repeats {} at {:?}",
        s.pressure_fallbacks.len(),
        s.pressure_fallbacks
    );
    let (nc, st, ps, ms, amp, cd, cl) = window(&ts, &cds, &cls);
    let e = |a: f64, r: f64| 100.0 * (a - r) / r.abs();
    let (ed, el) = (e(cd, CD_MAX_REF), e(cl, CL_MAX_REF));
    let (ed2, el2) = (e(T2_BOUNDED.0, CD_MAX_REF), e(T2_BOUNDED.1, CL_MAX_REF));
    println!(
        "RECORD H3 T bounded dt 1e-3 (IC repeat): {nc} crossings, St {st:.5} ({:+.3} %), period spread {ps:.3e}, C_L max \
         spread {ms:.3e}, amplitude {amp:.4}; C_D max {cd:.5} ({ed:+.3} %); C_L max {cl:.5} ({el:+.3} %); A2-T-2 drag \
         {ed:.3} -> {ed2:.3}: {}; lift {el:.3} -> {el2:.3}: {}",
        e(st, ST_REF),
        ed2.abs() < ed.abs(),
        el2.abs() < el.abs()
    );
    assert!(nc >= 6, "S2 H3: {nc} crossings");
    assert!(ps <= 1e-2, "S2 H3: period spread {ps:e}");
    assert!(ms <= 2e-2, "S2 H3: maximum spread {ms:e}");
    assert!(amp >= 1.0, "S2 H3: amplitude {amp}");
}
