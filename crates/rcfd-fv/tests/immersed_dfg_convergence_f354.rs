//! F-354 (ADR 0268): DFG 2D-2 with the immersed cylinder, the force extrema under one refinement in space and time.
//!
//! The records R0 to R9 of ADR 0268 (each an ignored heavy test, run once in release with `--ignored`): 2D-2 on T, T refined
//! once (T2), M2 and M3 with the first-order forcing `ImmersedForcing::build` and the bounded second-order forcing
//! `ImmersedForcing::build_second_order_bounded`, and the part runs (the time step on T and M2, the core-refined Tc, the
//! extended channel Te). The fast checks S3 to S7 (the meshes, the run table, the window statistics, the judgement functions,
//! three steps of the run helper) are what the gate runs. The set-up helpers are copies of those of
//! `immersed_unsteady_force_f346.rs` and `immersed_piso_stop_f355.rs` (those files are not edited).

use rcfd_core::vec3::Vec3;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::ddt::TimeScheme;
use rcfd_fv::diffusion::uniform_gamma;
use rcfd_fv::immersed::{ImmersedBody, ImmersedForcing, ImmersedSettings, SolidShape};
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::{FixedValue, FvGeometry, Profile, ScalarField, VectorField, ZeroGradient};
use rcfd_mesh::gen::{axis_coords, box_2d_coords, expansion_for_end_cell, Grading};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;

// ---------------------------------------------------------------------------------------------------------------------------
// DFG channel (ADR 0218), copied from immersed_unsteady_force_f346.rs and immersed_piso_stop_f355.rs
// ---------------------------------------------------------------------------------------------------------------------------

const H: f64 = 0.41;
const NU: f64 = 1e-3;
const DIAM: f64 = 0.1;
const CENTRE: Vec3 = [0.2, 0.2, 0.0];
const DZ: f64 = 0.01;
const K_DFG: f64 = 1e7;
/// FeatFlow level 6, `k = 1/200` (docs/reference-data.md, the row "DFG 2D-2 set-up" of the F-219 section).
const CD_MAX_REF: f64 = 3.2200;
const CD_MIN_REF: f64 = 3.1569;
const CD_MEAN_REF: f64 = 3.1884;
const CL_MAX_REF: f64 = 0.9859;
const CL_MIN_REF: f64 = -1.0206;
const ST_REF: f64 = 0.301_88;

/// Which forcing a run uses.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Forcing {
    /// `ImmersedForcing::build` with `modified = true` (ADR 0218).
    First,
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
        Forcing::First => ImmersedForcing::build(geom, &[body], st, &gamma).unwrap(),
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

/// The intervals of `xs` that lie inside `[lo, hi]` bisected, the others kept (ADR 0268: the core-refined Tc).
fn bisect_within(xs: &[f64], lo: f64, hi: f64) -> Vec<f64> {
    let mut out = vec![xs[0]];
    for w in xs.windows(2) {
        if w[0] >= lo - 1e-12 && w[1] <= hi + 1e-12 {
            out.push(0.5 * (w[0] + w[1]));
        }
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

/// The meshes of ADR 0268.
#[derive(Clone, Copy, PartialEq, Debug)]
enum MeshId {
    /// ADR 0218, 324 x 164.
    T,
    /// T with every interval bisected, 648 x 328.
    T2,
    /// T with the intervals in the core `[0.12, 0.32] x [0.12, 0.28]` bisected, 404 x 228.
    Tc,
    /// T continued from `x = 2.2` to `x = 4.4` by 35 uniform cells, 359 x 164.
    Te,
    /// ADR 0218 M2, 176 x 104.
    M2,
    /// ADR 0218 M3, 352 x 208.
    M3,
}

fn coords(m: MeshId) -> (Vec<f64>, Vec<f64>) {
    match m {
        MeshId::T => t_mesh(),
        MeshId::T2 => {
            let (xs, ys) = t_mesh();
            (bisect(&xs), bisect(&ys))
        }
        MeshId::Tc => {
            let (xs, ys) = t_mesh();
            (bisect_within(&xs, 0.12, 0.32), bisect_within(&ys, 0.12, 0.28))
        }
        MeshId::Te => {
            let (mut xs, ys) = t_mesh();
            let n = 35;
            xs.extend((1..=n).map(|k| 2.2 + 2.2 * k as f64 / n as f64));
            (xs, ys)
        }
        MeshId::M2 => dfg_family(2),
        MeshId::M3 => dfg_family(3),
    }
}

fn inflow_mean(um: f64, y1: f64, y2: f64) -> f64 {
    let prim = |y: f64| H * y * y / 2.0 - y * y * y / 3.0;
    4.0 * um / (H * H) * (prim(y2) - prim(y1)) / (y2 - y1)
}

fn inflow_point(um: f64, y: f64) -> f64 {
    4.0 * um * y * (H - y) / (H * H)
}

/// The DFG channel on the given coordinates (ADR 0218): geometry, velocity with the start field of the 2D-2 runs of ADR 0218
/// and 0249 (the inflow profile plus the transverse blob), and pressure.
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

// ---------------------------------------------------------------------------------------------------------------------------
// The run table of ADR 0268
// ---------------------------------------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
struct RunSpec {
    id: &'static str,
    mesh: MeshId,
    forcing: Forcing,
    dt: f64,
    steps: usize,
}

const RUNS: [RunSpec; 10] = [
    RunSpec {
        id: "R0",
        mesh: MeshId::M2,
        forcing: Forcing::First,
        dt: 1e-3,
        steps: 8000,
    },
    RunSpec {
        id: "R1",
        mesh: MeshId::T,
        forcing: Forcing::Bounded,
        dt: 1e-3,
        steps: 8000,
    },
    RunSpec {
        id: "R2",
        mesh: MeshId::T2,
        forcing: Forcing::First,
        dt: 5e-4,
        steps: 16000,
    },
    RunSpec {
        id: "R3",
        mesh: MeshId::T2,
        forcing: Forcing::Bounded,
        dt: 5e-4,
        steps: 16000,
    },
    RunSpec {
        id: "R4",
        mesh: MeshId::M3,
        forcing: Forcing::First,
        dt: 5e-4,
        steps: 16000,
    },
    RunSpec {
        id: "R5",
        mesh: MeshId::M3,
        forcing: Forcing::Bounded,
        dt: 5e-4,
        steps: 16000,
    },
    RunSpec {
        id: "R6",
        mesh: MeshId::M2,
        forcing: Forcing::First,
        dt: 5e-4,
        steps: 16000,
    },
    RunSpec {
        id: "R7",
        mesh: MeshId::T,
        forcing: Forcing::First,
        dt: 5e-4,
        steps: 16000,
    },
    RunSpec {
        id: "R8",
        mesh: MeshId::Tc,
        forcing: Forcing::First,
        dt: 5e-4,
        steps: 16000,
    },
    RunSpec {
        id: "R9",
        mesh: MeshId::Te,
        forcing: Forcing::First,
        dt: 1e-3,
        steps: 8000,
    },
];

fn spec(id: &str) -> RunSpec {
    *RUNS.iter().find(|r| r.id == id).unwrap()
}

// ---------------------------------------------------------------------------------------------------------------------------
// Window statistics (copied from immersed_unsteady_force_f346.rs) and the judgement functions of ADR 0268
// ---------------------------------------------------------------------------------------------------------------------------

/// Upward crossings of `v - mean` (linear interpolation).
fn crossings(t: &[f64], v: &[f64], mean: f64) -> Vec<f64> {
    let mut out = Vec::new();
    for i in 1..v.len() {
        if v[i - 1] - mean < 0.0 && v[i] - mean >= 0.0 {
            let w = (mean - v[i - 1]) / (v[i] - v[i - 1]);
            out.push(t[i - 1] + w * (t[i] - t[i - 1]));
        }
    }
    out
}

/// The window statistics of ADR 0218 (its A3 and S2) of one pair of coefficient series.
#[derive(Debug, Clone, Copy)]
struct Window {
    crossings: usize,
    st: f64,
    period_spread: f64,
    max_spread: f64,
    amplitude: f64,
    cd_max: f64,
    cd_min: f64,
    cd_mean: f64,
    cl_max: f64,
    cl_min: f64,
}

fn window(ts: &[f64], cds: &[f64], cls: &[f64]) -> Window {
    let w0 = ts.iter().position(|&t| t >= 5.5 - 1e-12).unwrap();
    let (tw, dw, lw) = (&ts[w0..], &cds[w0..], &cls[w0..]);
    let mean = lw.iter().sum::<f64>() / lw.len() as f64;
    let cr = crossings(tw, lw, mean);
    let n = cr.len();
    let mut out = Window {
        crossings: n,
        st: f64::NAN,
        period_spread: f64::NAN,
        max_spread: f64::NAN,
        amplitude: lw.iter().copied().fold(f64::MIN, f64::max) - lw.iter().copied().fold(f64::MAX, f64::min),
        cd_max: f64::MIN,
        cd_min: f64::MAX,
        cd_mean: dw.iter().sum::<f64>() / dw.len() as f64,
        cl_max: f64::MIN,
        cl_min: f64::MAX,
    };
    if n < 2 {
        return out;
    }
    let f = (n - 1) as f64 / (cr[n - 1] - cr[0]);
    out.st = f * DIAM / 1.0;
    let periods: Vec<f64> = cr.windows(2).map(|w| w[1] - w[0]).collect();
    let pm = periods.iter().sum::<f64>() / periods.len() as f64;
    out.period_spread = periods.iter().map(|p| (p - pm).abs() / pm).fold(0.0, f64::max);
    let mut maxima = Vec::new();
    for w in cr.windows(2) {
        let idx: Vec<usize> = (0..tw.len()).filter(|&i| tw[i] >= w[0] && tw[i] < w[1]).collect();
        maxima.push(idx.iter().map(|&i| lw[i]).fold(f64::MIN, f64::max));
        for &i in &idx {
            out.cd_max = out.cd_max.max(dw[i]);
            out.cd_min = out.cd_min.min(dw[i]);
            out.cl_max = out.cl_max.max(lw[i]);
            out.cl_min = out.cl_min.min(lw[i]);
        }
    }
    let mm = maxima.iter().sum::<f64>() / maxima.len() as f64;
    out.max_spread = maxima.iter().map(|m| (m - mm).abs() / mm.abs()).fold(0.0, f64::max);
    out
}

fn pct(a: f64, r: f64) -> f64 {
    100.0 * (a - r) / r.abs()
}

/// The acceptance limit of ADR 0268 A1: `abs(a - r)/abs(r) <= 1.5 percent`.
fn within_limit(a: f64, r: f64) -> bool {
    (a - r).abs() / r.abs() <= 1.5e-2
}

/// ADR 0268 A2: the error decreases from D/40 to D/80 (strictly, in magnitude).
fn decreases(e40: f64, e80: f64) -> bool {
    e80.abs() < e40.abs()
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Verdict {
    RuledOut,
    Contributes,
    Identified,
}

/// ADR 0268 P1 to P5: the verdict on the change `d` (percentage points) that one part causes.
fn verdict(d: f64) -> Verdict {
    if d.abs() <= 0.3 {
        Verdict::RuledOut
    } else if d.abs() >= 1.0 {
        Verdict::Identified
    } else {
        Verdict::Contributes
    }
}

/// S2 of ADR 0218 on a window: the failures.
fn s2(name: &str, w: &Window) -> Vec<String> {
    let mut out = Vec::new();
    if w.crossings < 6 {
        out.push(format!("S2 {name}: {} crossings", w.crossings));
    }
    if w.period_spread.is_nan() || w.period_spread > 1e-2 {
        out.push(format!("S2 {name}: period spread {:.3e}", w.period_spread));
    }
    if w.max_spread.is_nan() || w.max_spread > 2e-2 {
        out.push(format!("S2 {name}: C_L maximum spread {:.3e}", w.max_spread));
    }
    if w.amplitude.is_nan() || w.amplitude < 1.0 {
        out.push(format!("S2 {name}: amplitude {:.4}", w.amplitude));
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------------------
// The run
// ---------------------------------------------------------------------------------------------------------------------------

struct Outcome {
    /// `None` when the run stopped (the divergence guard); the step and the message are printed.
    stopped: Option<usize>,
    ts: Vec<f64>,
    cds: Vec<f64>,
    cls: Vec<f64>,
}

/// The 2D-2 run of ADR 0218 and 0249 on mesh `m` with the given forcing, time step and number of steps; the coefficients from
/// `force_moment` after every step.
fn run_2d2(name: &str, m: MeshId, forcing: Forcing, dt: f64, steps: usize) -> Outcome {
    let (xs, ys) = coords(m);
    let (um, ubar) = (1.5, 1.0);
    let (geom, u, p) = dfg_fields(&xs, &ys, um);
    let ib = forcing_of(&geom, forcing);
    let images = ib.flux_faces.iter().filter(|f| f.image.is_some()).count();
    let fallback = ib
        .flux_faces
        .iter()
        .filter(|f| matches!(f.image, Some((d, _)) if d == f.fluid))
        .count();
    let mut first: Vec<usize> = ib.bodies[0]
        .terms
        .iter()
        .filter(|t| t.face.is_some())
        .map(|t| t.cell)
        .collect();
    first.sort_unstable();
    first.dedup();
    println!(
        "2D-2 {name} {m:?} {forcing:?} dt {dt:e} steps {steps}: {} cells, {} solid cells, {} first fluid cells, {} flux faces, \
         {images} with an image, {fallback} fall-back",
        geom.n_cells(),
        ib.bodies[0].solid_cells.len(),
        first.len(),
        ib.flux_faces.len()
    );
    let mut st = PimpleSettings::piso(NU);
    st.time_scheme = TimeScheme::Bdf2;
    st.convection = ConvScheme::LinearUpwind;
    st.amg_pressure = true;
    st.n_correctors = 2;
    st.momentum_solver.tol_rel = 1e-6;
    st.pressure_solver.tol_rel = 1e-6;
    let mut s = PimpleSolver::new(geom.clone(), u, p, st);
    s.immersed = Some(ib.clone());
    let q = 0.5 * ubar * ubar * DIAM * DZ;
    let t0 = Instant::now();
    let mut out = Outcome {
        stopped: None,
        ts: Vec::with_capacity(steps),
        cds: Vec::with_capacity(steps),
        cls: Vec::with_capacity(steps),
    };
    for n in 1..=steps {
        let info = s.step(dt);
        if let Some(d) = &s.diverged {
            println!(
                "2D-2 {name}: stopped at step {n}: {}",
                d.message(Some(geom.mesh.cell_centres()))
            );
            out.stopped = Some(n);
            return out;
        }
        let (f, _) = ib.force_moment(0, &s.u.values, 1.0, CENTRE);
        out.ts.push(n as f64 * dt);
        out.cds.push(f[0] / q);
        out.cls.push(f[1] / q);
        if n % 1000 == 0 {
            println!(
                "2D-2 {name} t {:.3}: C_D {:.5} C_L {:+.5}; Co {:.3} ({:.0} s)",
                n as f64 * dt,
                f[0] / q,
                f[1] / q,
                info.courant,
                t0.elapsed().as_secs_f64()
            );
        }
    }
    println!("2D-2 {name}: {steps} steps in {:.0} s", t0.elapsed().as_secs_f64());
    out
}

/// One record of ADR 0268: the run of the table, its window, S2; the result line.
fn record(id: &str) -> Window {
    let r = spec(id);
    let o = run_2d2(r.id, r.mesh, r.forcing, r.dt, r.steps);
    assert!(o.stopped.is_none(), "S2 {id}: the run stopped at step {:?}", o.stopped);
    let w = window(&o.ts, &o.cds, &o.cls);
    println!(
        "RECORD {id} {:?} {:?} dt {:e}: {} crossings, St {:.5} ({:+.3} %), period spread {:.3e}, C_L max spread {:.3e}, \
         amplitude {:.4}; C_D max {:.5} ({:+.3} %) min {:.5} ({:+.3} %) mean {:.5} ({:+.3} %); C_L max {:.5} ({:+.3} %) min \
         {:.5} ({:+.3} %)",
        r.mesh,
        r.forcing,
        r.dt,
        w.crossings,
        w.st,
        pct(w.st, ST_REF),
        w.period_spread,
        w.max_spread,
        w.amplitude,
        w.cd_max,
        pct(w.cd_max, CD_MAX_REF),
        w.cd_min,
        pct(w.cd_min, CD_MIN_REF),
        w.cd_mean,
        pct(w.cd_mean, CD_MEAN_REF),
        w.cl_max,
        pct(w.cl_max, CL_MAX_REF),
        w.cl_min,
        -pct(w.cl_min, CL_MIN_REF)
    );
    let fails = s2(id, &w);
    assert!(fails.is_empty(), "{fails:#?}");
    w
}

// ---------------------------------------------------------------------------------------------------------------------------
// The records R0 to R9 (heavy; each run once in release with --ignored)
// ---------------------------------------------------------------------------------------------------------------------------

/// ADR 0268 R0 and C0: M2 with `build`, the copied set-up reproduces ADR 0249 (3.22841, 0.98432, 0.30041).
#[test]
#[ignore = "heavy record (ADR 0268 R0, about 0.3 h); run once in release with --ignored"]
fn r0_m2_first_control() {
    let w = record("R0");
    let same = |a: f64, b: f64, half: f64| (a - b).abs() <= half;
    assert!(
        same(w.cd_max, 3.22841, 5e-6) && same(w.cl_max, 0.98432, 5e-6) && same(w.st, 0.30041, 5e-6),
        "C0: max C_D {:.6}, max C_L {:.6}, St {:.6} against 3.22841, 0.98432, 0.30041",
        w.cd_max,
        w.cl_max,
        w.st
    );
}

/// ADR 0268 R1: T with the bounded forcing (D/40 of the T family, bounded).
#[test]
#[ignore = "heavy record (ADR 0268 R1, about 0.65 h); run once in release with --ignored"]
fn r1_t_bounded() {
    record("R1");
}

/// ADR 0268 R2: T2 with `build` (D/80, dt 5e-4).
#[test]
#[ignore = "heavy record (ADR 0268 R2, about 6.5 h); run once in release with --ignored"]
fn r2_t2_first() {
    record("R2");
}

/// ADR 0268 R3: T2 with the bounded forcing (D/80, dt 5e-4).
#[test]
#[ignore = "heavy record (ADR 0268 R3, about 6.5 h); run once in release with --ignored"]
fn r3_t2_bounded() {
    record("R3");
}

/// ADR 0268 R4: M3 with `build` (D/80, dt 5e-4).
#[test]
#[ignore = "heavy record (ADR 0268 R4, about 2.2 h); run once in release with --ignored"]
fn r4_m3_first() {
    record("R4");
}

/// ADR 0268 R5: M3 with the bounded forcing (D/80, dt 5e-4).
#[test]
#[ignore = "heavy record (ADR 0268 R5, about 2.2 h); run once in release with --ignored"]
fn r5_m3_bounded() {
    record("R5");
}

/// ADR 0268 R6 (P1): M2 with `build` at dt 5e-4 (the time step alone, M family).
#[test]
#[ignore = "heavy record (ADR 0268 R6, about 0.65 h); run once in release with --ignored"]
fn r6_m2_first_half_step() {
    record("R6");
}

/// ADR 0268 R7 (P1, P2): T with `build` at dt 5e-4 (the time step alone, T family).
#[test]
#[ignore = "heavy record (ADR 0268 R7, about 1.25 h); run once in release with --ignored"]
fn r7_t_first_half_step() {
    record("R7");
}

/// ADR 0268 R8 (P2, P3): Tc (T refined in the core) with `build` at dt 5e-4.
#[test]
#[ignore = "heavy record (ADR 0268 R8, about 2.5 h); run once in release with --ignored"]
fn r8_tc_first_half_step() {
    record("R8");
}

/// ADR 0268 R9 (P5): Te (T with the outlet at x = 4.4) with `build` at dt 1e-3.
#[test]
#[ignore = "heavy record (ADR 0268 R9, about 0.7 h); run once in release with --ignored"]
fn r9_te_first_outlet() {
    record("R9");
}

// ---------------------------------------------------------------------------------------------------------------------------
// The fast checks S3 to S7 (the gate)
// ---------------------------------------------------------------------------------------------------------------------------

/// ADR 0268 S3: the six meshes, the spacing at the cylinder, the solid cells of both forcings and their symmetry, the image
/// faces of the bounded form, the coordinates of T2 and Tc against T, the outlet of Te.
#[test]
fn s3_meshes_and_forcings() {
    let expect = [
        (MeshId::T, 53_136, 0.0025, 1264),
        (MeshId::T2, 212_544, 0.00125, 5024),
        (MeshId::Tc, 92_112, 0.00125, 5024),
        (MeshId::Te, 58_876, 0.0025, 1264),
        (MeshId::M2, 18_304, 0.0025, 1264),
        (MeshId::M3, 73_216, 0.00125, 5024),
    ];
    let (tx, ty) = t_mesh();
    for (m, cells, h, solid) in expect {
        let (xs, ys) = coords(m);
        let n = (xs.len() - 1) * (ys.len() - 1);
        assert_eq!(n, cells, "S3 {m:?}: cell count");
        for i in 0..xs.len() - 1 {
            for j in 0..ys.len() - 1 {
                let (cx, cy) = (0.5 * (xs[i] + xs[i + 1]), 0.5 * (ys[j] + ys[j + 1]));
                if (cx - 0.2).hypot(cy - 0.2) <= 0.06 {
                    let (dx, dy) = (xs[i + 1] - xs[i], ys[j + 1] - ys[j]);
                    assert!(
                        (dx - h).abs() <= 1e-12 && (dy - h).abs() <= 1e-12,
                        "S3 {m:?}: spacing {dx:e} x {dy:e} at ({cx}, {cy}), expected {h:e}"
                    );
                }
            }
        }
        if matches!(m, MeshId::T2 | MeshId::Tc) {
            for (a, b) in [(&tx, &xs), (&ty, &ys)] {
                for v in a.iter() {
                    assert!(b.iter().any(|w| w == v), "S3 {m:?}: coordinate {v} of T missing");
                }
            }
        }
        if m == MeshId::Te {
            assert!(
                (xs[xs.len() - 1] - 4.4).abs() <= 1e-12,
                "S3 Te: outlet at {}",
                xs[xs.len() - 1]
            );
            assert!(
                xs.iter().any(|&x| (x - 2.2).abs() <= 1e-12),
                "S3 Te: x = 2.2 not a line"
            );
        }
        let geom = FvGeometry::new(Arc::new(box_2d_coords(&xs, &ys, DZ).unwrap())).unwrap();
        assert_eq!(geom.n_cells(), cells);
        if m == MeshId::Te {
            let cf = geom.mesh.face_centres();
            for f in geom.mesh.patch("xmax").unwrap().faces() {
                assert!((cf[f][0] - 4.4).abs() <= 1e-12, "S3 Te: xmax face at {}", cf[f][0]);
            }
        }
        let cc = geom.mesh.cell_centres();
        let key = |x: f64, y: f64| ((x * 1e8).round() as i64, (y * 1e8).round() as i64);
        for forcing in [Forcing::First, Forcing::Bounded] {
            let ib = forcing_of(&geom, forcing);
            let sc = &ib.bodies[0].solid_cells;
            assert_eq!(sc.len(), solid, "S3 {m:?} {forcing:?}: solid cells");
            let set: HashSet<(i64, i64)> = sc.iter().map(|&c| key(cc[c][0], cc[c][1])).collect();
            for &c in sc {
                let (x, y) = (cc[c][0], cc[c][1]);
                assert!(
                    set.contains(&key(0.4 - x, y)) && set.contains(&key(x, 0.4 - y)),
                    "S3 {m:?} {forcing:?}: solid cell at ({x}, {y}) has no mirror image"
                );
            }
            let images = ib.flux_faces.iter().filter(|f| f.image.is_some()).count();
            match forcing {
                Forcing::First => assert_eq!(images, 0, "S3 {m:?}: build has image faces"),
                Forcing::Bounded => assert!(images >= 10, "S3 {m:?}: {images} image faces"),
            }
        }
    }
}

/// ADR 0268 S4: the run table.
#[test]
fn s4_run_table() {
    use Forcing::{Bounded as B, First as F};
    use MeshId::*;
    let want = [
        ("R0", M2, F, 1e-3, 8000),
        ("R1", T, B, 1e-3, 8000),
        ("R2", T2, F, 5e-4, 16000),
        ("R3", T2, B, 5e-4, 16000),
        ("R4", M3, F, 5e-4, 16000),
        ("R5", M3, B, 5e-4, 16000),
        ("R6", M2, F, 5e-4, 16000),
        ("R7", T, F, 5e-4, 16000),
        ("R8", Tc, F, 5e-4, 16000),
        ("R9", Te, F, 1e-3, 8000),
    ];
    assert_eq!(RUNS.len(), want.len());
    for (r, w) in RUNS.iter().zip(want) {
        assert_eq!((r.id, r.mesh, r.forcing, r.steps), (w.0, w.1, w.2, w.4), "S4 {}", r.id);
        assert_eq!(r.dt, w.3, "S4 {}: dt", r.id);
        assert!((r.dt * r.steps as f64 - 8.0).abs() <= 1e-12, "S4 {}: end time", r.id);
    }
}

/// ADR 0268 S5: the window statistics on a synthetic series with known Strouhal number and extrema.
#[test]
fn s5_window_on_a_synthetic_series() {
    let dt = 1e-3;
    let ts: Vec<f64> = (1..=8000).map(|n| n as f64 * dt).collect();
    let tau = std::f64::consts::TAU;
    let cls: Vec<f64> = ts.iter().map(|t| 0.25 + (tau * 3.0 * t).sin()).collect();
    let cds: Vec<f64> = ts.iter().map(|t| 3.0 + 0.1 * (tau * 6.0 * t).cos()).collect();
    let w = window(&ts, &cds, &cls);
    println!("S5: {w:?}");
    assert!((w.st - 0.3).abs() <= 1e-6, "S5: St {}", w.st);
    assert!((w.cl_max - 1.25).abs() <= 1e-4, "S5: max C_L {}", w.cl_max);
    assert!((w.cd_max - 3.1).abs() <= 1e-4, "S5: max C_D {}", w.cd_max);
    assert!(w.period_spread <= 1e-4 && w.max_spread <= 1e-4, "S5: spreads {w:?}");
}

/// ADR 0268 S6: the judgement functions on given numbers.
#[test]
fn s6_judgement_functions() {
    assert!(within_limit(CD_MAX_REF * 1.0149, CD_MAX_REF));
    assert!(!within_limit(CD_MAX_REF * 1.0151, CD_MAX_REF));
    assert!(within_limit(CL_MAX_REF * 0.9851, CL_MAX_REF));
    assert!(!within_limit(CL_MAX_REF * 0.9849, CL_MAX_REF));
    assert!(decreases(0.6, 0.5));
    assert!(!decreases(0.6, 0.6));
    assert!(!decreases(0.6, -0.7));
    assert!(decreases(-0.6, 0.5));
    assert_eq!(verdict(0.29), Verdict::RuledOut);
    assert_eq!(verdict(-0.29), Verdict::RuledOut);
    assert_eq!(verdict(0.31), Verdict::Contributes);
    assert_eq!(verdict(1.0), Verdict::Identified);
    assert_eq!(verdict(-1.2), Verdict::Identified);
}

/// ADR 0268 S7: three steps of the run helper on M2 with each forcing.
#[test]
fn s7_run_helper_three_steps() {
    for forcing in [Forcing::First, Forcing::Bounded] {
        let o = run_2d2("S7", MeshId::M2, forcing, 1e-3, 3);
        assert!(o.stopped.is_none(), "S7 {forcing:?}: stopped at {:?}", o.stopped);
        assert_eq!(o.cds.len(), 3);
        assert!(
            o.cds.iter().chain(&o.cls).all(|v| v.is_finite()),
            "S7 {forcing:?}: {:?} {:?}",
            o.cds,
            o.cls
        );
    }
}
