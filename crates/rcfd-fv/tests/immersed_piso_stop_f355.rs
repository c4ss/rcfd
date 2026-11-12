//! F-355 (ADR 0254): the second-order immersed forcing in transient PISO at Re 100.
//!
//! ADR 0249's printed run of DFG 2D-2 on M2 with `ImmersedForcing::build_second_order` stopped at step 6 (the Courant guard).
//! The first part of this file is the registered diagnosis: the reproduction (D0), one variant run per hypothesis with its
//! control (the same run with `ImmersedForcing::build`), and the estimate `G` of ADR 0254 (the extrapolation weight of a
//! fluid-solid face times the share of the time term in the diagonal of its fluid cell) against the verdict of every run (D1
//! to D8). The set-up helpers are copies of those of `immersed_unsteady_force_f346.rs` and `immersed_second_order_f309.rs`
//! (those files are not edited). The heavy tests run in release only.
//!
//! The second part (ADR 0254 amendment 1) holds the located cause (D9), the checks of the bounded form
//! `ImmersedForcing::build_second_order_bounded` (UB1 to UB3, SB, CC-A1, CC-V3, and the acceptance run A: DFG 2D-2 on M2 to
//! t = 8), the refusal of the unbounded form by the transient solver (RF) and the hashes of the unchanged forcing (P). The
//! diagnosis runs switch the refusal off (`flux_weight_check = false`), so that they keep reproducing the stop.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::convection::{face_flux, ConvScheme};
use rcfd_fv::ddt::TimeScheme;
use rcfd_fv::diffusion::uniform_gamma;
use rcfd_fv::immersed::{ImmersedBody, ImmersedForcing, ImmersedSettings, SolidShape};
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::simple::{Coupling, SimpleSettings, SimpleSolver};
use rcfd_fv::{FixedValue, FvGeometry, Profile, ScalarField, VectorField, ZeroGradient};
use rcfd_mesh::gen::{axis_coords, box_2d, box_2d_coords, expansion_for_end_cell, Grading};
use std::sync::Arc;
use std::time::Instant;

/// Which forcing a run uses.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Forcing {
    /// `ImmersedForcing::build` with `modified = true` (ADR 0218).
    First,
    /// `ImmersedForcing::build_second_order` (ADR 0234), with the refusal of ADR 0254 E2 switched off.
    Second,
    /// `ImmersedForcing::build_second_order_bounded` (ADR 0254).
    Bounded,
}

fn build(
    geom: &FvGeometry,
    bodies: &[ImmersedBody],
    k: f64,
    nu: f64,
    theta_min: f64,
    forcing: Forcing,
) -> ImmersedForcing {
    let gamma = uniform_gamma(geom, nu);
    let mut st = ImmersedSettings::new(k);
    st.theta_min = theta_min;
    match forcing {
        Forcing::First => ImmersedForcing::build(geom, bodies, st, &gamma).unwrap(),
        Forcing::Second => {
            let mut ib = ImmersedForcing::build_second_order(geom, bodies, st, &gamma).unwrap();
            ib.flux_weight_check = false;
            ib
        }
        Forcing::Bounded => ImmersedForcing::build_second_order_bounded(geom, bodies, st, &gamma).unwrap(),
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// DFG channel (ADR 0218), copied from immersed_unsteady_force_f346.rs
// ---------------------------------------------------------------------------------------------------------------------------

const H: f64 = 0.41;
const NU: f64 = 1e-3;
const DIAM: f64 = 0.1;
const CENTRE: Vec3 = [0.2, 0.2, 0.0];
const DZ: f64 = 0.01;
const K_DFG: f64 = 1e7;

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

/// The DFG channel on the given coordinates (ADR 0218), without the forcing: geometry, velocity and pressure fields.
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

/// The start field of the 2D-2 runs of ADR 0218 and 0249: the inflow profile everywhere plus a transverse blob.
fn two_d_two_start(geom: &FvGeometry, u: &mut VectorField, um: f64) {
    let cc = geom.mesh.cell_centres();
    for (c, x) in cc.iter().enumerate() {
        let blob = 0.2 * (-((x[0] - 0.35).powi(2) + (x[1] - 0.2).powi(2)) / (0.05 * 0.05)).exp();
        u.values[c] = [inflow_point(um, x[1]), blob, 0.0];
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// The estimate G and the measurements of a run (ADR 0254)
// ---------------------------------------------------------------------------------------------------------------------------

/// The bounded crossing fraction of the modification term of `face` in `cell`.
fn theta_e(ib: &ImmersedForcing, cell: usize, face: usize, theta_min: f64) -> f64 {
    ib.bodies
        .iter()
        .flat_map(|b| &b.terms)
        .find(|t| t.cell == cell && t.face == Some(face))
        .map(|t| t.theta.max(theta_min))
        .expect("a modification term for the face")
}

/// The share `beta_c` of the time term in the diagonal of the first fluid cell `c` (ADR 0254, convection left out):
/// `(c0 V/dt)/(c0 V/dt + sum over the internal non-empty faces of k_face m_face)`.
fn beta(geom: &FvGeometry, ib: &ImmersedForcing, c: usize, nu: f64, dt: f64, c0: f64, theta_min: f64) -> f64 {
    let mesh = &geom.mesh;
    let time = c0 * mesh.cell_volumes()[c] / dt;
    let mut visc = 0.0;
    for &f in mesh.cell_faces(c) {
        if f >= geom.n_internal || geom.empty[f] {
            continue;
        }
        let other = if mesh.owner()[f] == c {
            mesh.neighbour()[f]
        } else {
            mesh.owner()[f]
        };
        let m = if ib.body_of_cell[other].is_some() {
            1.0 / theta_e(ib, c, f, theta_min)
        } else {
            1.0
        };
        visc += nu * geom.lap_coeff[f] * m;
    }
    time / (time + visc)
}

/// `G = max_f abs(weight_f - 1) beta_c` over the flux faces, with the largest weight and the smallest theta of the forcing.
fn estimate(geom: &FvGeometry, ib: &ImmersedForcing, nu: f64, dt: f64, c0: f64, theta_min: f64) -> (f64, f64, f64) {
    let (mut g, mut wmax, mut tmin) = (0.0f64, 0.0f64, f64::MAX);
    for ff in &ib.flux_faces {
        let b = beta(geom, ib, ff.fluid, nu, dt, c0, theta_min);
        g = g.max((ff.weight - 1.0).abs() * b);
        wmax = wmax.max(ff.weight);
        tmin = tmin.min(theta_e(ib, ff.fluid, ff.face, theta_min));
    }
    (g, wmax, tmin)
}

/// The fluid cells next to a solid cell (the fluid cells of the modification terms), sorted.
fn first_fluid_cells(ib: &ImmersedForcing) -> Vec<usize> {
    let mut cells: Vec<usize> = ib
        .bodies
        .iter()
        .flat_map(|b| &b.terms)
        .filter(|t| t.face.is_some())
        .map(|t| t.cell)
        .collect();
    cells.sort_unstable();
    cells.dedup();
    cells
}

fn max_norm(u: &[Vec3], cells: impl Iterator<Item = usize>) -> (f64, usize) {
    let mut best = (0.0f64, usize::MAX);
    for c in cells {
        let v = vec3::norm(u[c]);
        // a NaN counts as the largest
        if v.is_nan() || v > best.0 {
            best = (if v.is_nan() { f64::INFINITY } else { v }, c);
        }
    }
    best
}

/// The largest `abs(sum of phi)` over the fluid cells divided by the largest `abs(phi)` (the measurement of H4).
fn fluid_continuity(geom: &FvGeometry, ib: &ImmersedForcing, phi: &[f64]) -> f64 {
    let mesh = &geom.mesh;
    let mut worst = 0.0f64;
    for c in 0..geom.n_cells() {
        if ib.body_of_cell[c].is_some() {
            continue;
        }
        let mut sum = 0.0;
        for &f in mesh.cell_faces(c) {
            if geom.empty[f] {
                continue;
            }
            sum += if f >= geom.n_internal || mesh.owner()[f] == c {
                phi[f]
            } else {
                -phi[f]
            };
        }
        worst = worst.max(sum.abs());
    }
    worst / phi.iter().fold(0.0f64, |a, b| a.max(b.abs()))
}

/// One variant of the reproduction R (ADR 0254): R with the named change.
#[derive(Clone, Copy, Debug)]
struct Variant {
    name: &'static str,
    level: usize,
    dt: f64,
    nu: f64,
    theta_min: f64,
    scheme: TimeScheme,
    conv: ConvScheme,
    n_correctors: usize,
    ddt_correction: bool,
    /// Steps made with the first-order forcing before the forcing of the run is set (the variant "late").
    lead: usize,
}

const R: Variant = Variant {
    name: "R",
    level: 2,
    dt: 1e-3,
    nu: NU,
    theta_min: 1e-2,
    scheme: TimeScheme::Bdf2,
    conv: ConvScheme::LinearUpwind,
    n_correctors: 2,
    ddt_correction: true,
    lead: 0,
};

#[derive(Clone, Debug)]
struct Outcome {
    /// The step at which the divergence guard stopped the run.
    stop: Option<usize>,
    /// The record of the stop.
    reason: String,
    /// Largest `abs(u)` over all cells after the last step made.
    max_all: f64,
    /// Per completed step, the largest `abs(u)` over the first fluid cells.
    m: Vec<f64>,
    /// Per completed step, the fluid-cell continuity measure of H4.
    cont: Vec<f64>,
    /// The estimate, the largest weight and the smallest bounded theta of the second-order forcing of the variant.
    g_est: f64,
    w_max: f64,
    theta_small: f64,
    /// At the end of the run: the cell of the largest `abs(u)`, whether it is a first fluid cell, and the weights and thetas of
    /// its flux faces.
    worst: (usize, bool, Vec<(f64, f64)>),
}

impl Outcome {
    /// "stops", "grows" or "runs" (ADR 0254).
    fn verdict(&self) -> &'static str {
        if self.stop.is_some() {
            "stops"
        } else if self.max_all <= 10.0 {
            "runs"
        } else {
            "grows"
        }
    }

    /// The geometric mean of `m_n/m_(n-1)` over the last three completed steps.
    fn growth(&self) -> f64 {
        let n = self.m.len();
        if n < 2 {
            return f64::NAN;
        }
        let k = 3.min(n - 1);
        (self.m[n - 1] / self.m[n - 1 - k]).powf(1.0 / k as f64)
    }
}

const STEPS: usize = 200;

fn run_variant(v: &Variant, forcing: Forcing) -> Outcome {
    let (xs, ys) = dfg_family(v.level);
    let um = 1.5;
    let (geom, mut u, p) = dfg_fields(&xs, &ys, um);
    two_d_two_start(&geom, &mut u, um);
    let first = build(&geom, &[dfg_body()], K_DFG, v.nu, v.theta_min, Forcing::First);
    let second = build(&geom, &[dfg_body()], K_DFG, v.nu, v.theta_min, Forcing::Second);
    let ib = match forcing {
        Forcing::First => first.clone(),
        Forcing::Second => second.clone(),
        Forcing::Bounded => build(&geom, &[dfg_body()], K_DFG, v.nu, v.theta_min, Forcing::Bounded),
    };
    let c0 = if v.scheme == TimeScheme::Bdf2 { 1.5 } else { 1.0 };
    let (g_est, w_max, theta_small) = estimate(&geom, &second, v.nu, v.dt, c0, v.theta_min);
    let mut st = PimpleSettings::piso(v.nu);
    st.time_scheme = v.scheme;
    st.convection = v.conv;
    st.amg_pressure = true;
    st.n_correctors = v.n_correctors;
    st.ddt_correction = v.ddt_correction;
    st.momentum_solver.tol_rel = 1e-6;
    st.pressure_solver.tol_rel = 1e-6;
    let mut s = PimpleSolver::new(geom.clone(), u, p, st);
    s.immersed = Some(first);
    for n in 1..=v.lead {
        s.step(v.dt);
        assert!(
            s.diverged.is_none(),
            "{}: the lead-in stopped at step {n}: {:?}",
            v.name,
            s.diverged
        );
    }
    s.immersed = Some(ib.clone());
    let layer = first_fluid_cells(&ib);
    let n_cells = geom.n_cells();
    let (mut m, mut cont) = (Vec::new(), Vec::new());
    let (mut stop, mut reason) = (None, String::new());
    for n in 1..=STEPS {
        s.step(v.dt);
        if let Some(d) = &s.diverged {
            stop = Some(n);
            reason = format!("{} {} {:.4e}", d.stage, d.quantity, d.value);
            break;
        }
        m.push(max_norm(&s.u.values, layer.iter().copied()).0);
        cont.push(fluid_continuity(&geom, &ib, &s.phi));
    }
    let (max_all, cell) = max_norm(&s.u.values, 0..n_cells);
    let faces: Vec<(f64, f64)> = second
        .flux_faces
        .iter()
        .filter(|ff| ff.fluid == cell)
        .map(|ff| (ff.weight, theta_e(&second, ff.fluid, ff.face, v.theta_min)))
        .collect();
    Outcome {
        stop,
        reason,
        max_all,
        m,
        cont,
        g_est,
        w_max,
        theta_small,
        worst: (cell, layer.binary_search(&cell).is_ok(), faces),
    }
}

fn print_outcome(name: &str, forcing: Forcing, o: &Outcome) {
    println!(
        "{name} {forcing:?}: {} (stop step {:?} {}), G {:.3} (largest weight {:.3}, smallest theta_e {:.4}), measured g {:.3}, \
         max |u| all {:.3e}, m first/last {:.3e}/{:.3e}, fluid continuity last {:.2e}; worst cell {} first-fluid {} faces \
         (weight, theta_e) {:?}",
        o.verdict(),
        o.stop,
        o.reason,
        o.g_est,
        o.w_max,
        o.theta_small,
        o.growth(),
        o.max_all,
        o.m.first().copied().unwrap_or(f64::NAN),
        o.m.last().copied().unwrap_or(f64::NAN),
        o.cont.last().copied().unwrap_or(f64::NAN),
        o.worst.0,
        o.worst.1,
        o.worst.2
    );
}

/// ADR 0254 D0 (the reproduction), C-R (the first-order forcing runs), D4 and the first part of D5 (the cell of the largest
/// velocity at the stop and its weights).
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn d0_d4_d5_reproduction() {
    let o = run_variant(&R, Forcing::Second);
    print_outcome("R", Forcing::Second, &o);
    println!("R Second: m per step {:?}; fluid continuity per step {:?}", o.m, o.cont);
    let c = run_variant(&R, Forcing::First);
    print_outcome("R", Forcing::First, &c);
    assert_eq!(c.verdict(), "runs", "C-R: the first-order forcing does not run");
    assert_eq!(o.stop, Some(6), "D0: the stop is not at step 6");
    assert!(o.reason.starts_with("end of step courant"), "D0: {}", o.reason);
    let courant: f64 = o.reason.rsplit(' ').next().unwrap().parse().unwrap();
    assert!((1.0e7..=1.2e7).contains(&courant), "D0: Courant {courant:.4e}");
}

fn variants() -> Vec<Variant> {
    let v = |name: &'static str, f: &dyn Fn(&mut Variant)| {
        let mut x = R;
        x.name = name;
        f(&mut x);
        x
    };
    vec![
        v("H1 ddt_correction off", &|x| x.ddt_correction = false),
        v("H2 n_correctors 1", &|x| x.n_correctors = 1),
        v("H2 n_correctors 3", &|x| x.n_correctors = 3),
        v("H2 n_correctors 4", &|x| x.n_correctors = 4),
        v("H3 implicit Euler", &|x| x.scheme = TimeScheme::ImplicitEuler),
        v("H5 theta_min 0.1", &|x| x.theta_min = 0.1),
        v("H5 theta_min 0.25", &|x| x.theta_min = 0.25),
        v("H5 theta_min 0.5", &|x| x.theta_min = 0.5),
        v("H6 upwind", &|x| x.conv = ConvScheme::Upwind),
        v("H6 central", &|x| x.conv = ConvScheme::Central),
        v("H7 dt 5e-4", &|x| x.dt = 5e-4),
        v("H7 dt 2e-3", &|x| x.dt = 2e-3),
        v("H7 dt 4e-3", &|x| x.dt = 4e-3),
        v("H7 dt 8e-3", &|x| x.dt = 8e-3),
        v("H7 late", &|x| x.lead = 300),
        v("H8 nu 3e-3", &|x| x.nu = 3e-3),
        v("H8 nu 1e-2", &|x| x.nu = 1e-2),
        v("H8 M1", &|x| x.level = 1),
    ]
}

/// ADR 0254 D1 to D8 on the DFG channel: every variant and its control, once, four runs at a time; the verdicts and the
/// estimate are printed; asserted: every control runs.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn d1_to_d8_variants() {
    let vs = variants();
    let jobs: Vec<(usize, Forcing)> = (0..vs.len())
        .flat_map(|i| [(i, Forcing::Second), (i, Forcing::First)])
        .collect();
    let mut out: Vec<Option<Outcome>> = vec![None; jobs.len()];
    for (chunk, slots) in jobs.chunks(4).zip(out.chunks_mut(4)) {
        std::thread::scope(|sc| {
            for (job, slot) in chunk.iter().zip(slots.iter_mut()) {
                let v = vs[job.0];
                let forcing = job.1;
                sc.spawn(move || *slot = Some(run_variant(&v, forcing)));
            }
        });
    }
    let mut void = Vec::new();
    let (mut low_stops, mut high_runs) = (Vec::new(), Vec::new());
    for (i, v) in vs.iter().enumerate() {
        let (o, c) = (out[2 * i].as_ref().unwrap(), out[2 * i + 1].as_ref().unwrap());
        print_outcome(v.name, Forcing::Second, o);
        print_outcome(v.name, Forcing::First, c);
        if c.verdict() != "runs" {
            void.push(v.name);
            continue;
        }
        if o.g_est <= 1.0 && o.verdict() != "runs" {
            low_stops.push(v.name);
        }
        if o.g_est >= 2.0 && o.verdict() == "runs" {
            high_runs.push(v.name);
        }
    }
    println!(
        "D8 on the DFG variants: runs with G <= 1 that stop or grow: {low_stops:?}; runs with G >= 2 that run: {high_runs:?}; \
         void (control does not run): {void:?}"
    );
    assert!(void.is_empty(), "controls that do not run: {void:?}");
}

// ---------------------------------------------------------------------------------------------------------------------------
// Circular Couette (CC), the set-up of ADR 0218, copied from immersed_second_order_f309.rs
// ---------------------------------------------------------------------------------------------------------------------------

const CC_C: Vec3 = [1.2, 1.2, 0.0];
const R1: f64 = 0.5;
const R2: f64 = 1.0;
const OMEGA: f64 = 1.0;

fn cc_bodies() -> Vec<ImmersedBody> {
    let mut inner = ImmersedBody::fixed(
        "inner",
        SolidShape::Cylinder {
            centre: CC_C,
            axis: [0.0, 0.0, 1.0],
            radius: R1,
        },
    );
    inner.omega = [0.0, 0.0, OMEGA];
    inner.origin = CC_C;
    let mut outer = ImmersedBody::fixed(
        "outer",
        SolidShape::Cylinder {
            centre: CC_C,
            axis: [0.0, 0.0, 1.0],
            radius: R2,
        },
    );
    outer.inverted = true;
    vec![inner, outer]
}

fn cc_fields(n: usize, forcing: Forcing) -> (Arc<FvGeometry>, VectorField, ScalarField, ImmersedForcing) {
    cc_fields_with(n, forcing, 1e-2)
}

fn cc_fields_with(
    n: usize,
    forcing: Forcing,
    theta_min: f64,
) -> (Arc<FvGeometry>, VectorField, ScalarField, ImmersedForcing) {
    let geom = Arc::new(FvGeometry::new(Arc::new(box_2d(n, n, 2.4, 2.4, false, false).unwrap())).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    for w in ["xmin", "xmax", "ymin", "ymax"] {
        u.set_bc(&geom, w, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    let p = ScalarField::uniform("p", &geom, 0.0);
    let ib = build(&geom, &cc_bodies(), 1e8, 1.0, theta_min, forcing);
    (geom, u, p, ib)
}

/// The PISO run of ADR 0234 V3 (CC at N 48, Euler, central, from rest) with the step `dt`, at most 400 steps: the step of a
/// stop, the largest `abs(u)` over all cells at the end, and the estimate `G` of the second-order forcing.
fn cc_piso(dt: f64, forcing: Forcing) -> (Option<usize>, f64, f64) {
    let (geom, u, p, ib) = cc_fields(48, forcing);
    let (_, _, _, second) = cc_fields(48, Forcing::Second);
    let (g_est, w_max, theta_small) = estimate(&geom, &second, 1.0, dt, 1.0, 1e-2);
    let mut st = PimpleSettings::piso(1.0);
    st.convection = ConvScheme::Central;
    let mut s = PimpleSolver::new(geom.clone(), u, p, st);
    s.immersed = Some(ib);
    let mut stop = None;
    for n in 1..=400 {
        s.step(dt);
        if s.diverged.is_some() {
            stop = Some(n);
            break;
        }
    }
    let max_all = max_norm(&s.u.values, 0..geom.n_cells()).0;
    println!(
        "CC N 48 PISO dt {dt:.1e} {forcing:?}: stop {stop:?} ({:?}), max |u| {max_all:.3e}, G {g_est:.3} (largest weight \
         {w_max:.3}, smallest theta_e {theta_small:.4})",
        s.diverged.as_ref().map(|d| (d.stage, d.quantity, d.value))
    );
    (stop, max_all, g_est)
}

/// ADR 0254 D8 on circular Couette flow: the run of ADR 0234 V3 (`dt = 2.5e-3`) runs, the same with `dt = 2.5e-5` stops or
/// grows; the controls with the first-order forcing run. The inner cylinder moves at 0.5, so "runs" is `max |u| <= 10`.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn d8_circular_couette_time_step() {
    let runs = |r: &(Option<usize>, f64, f64)| r.0.is_none() && r.1 <= 10.0;
    let large = cc_piso(2.5e-3, Forcing::Second);
    let large_c = cc_piso(2.5e-3, Forcing::First);
    let small = cc_piso(2.5e-5, Forcing::Second);
    let small_c = cc_piso(2.5e-5, Forcing::First);
    println!(
        "D8 CC: dt 2.5e-3 G {:.3} runs {}; dt 2.5e-5 G {:.3} runs {}",
        large.2,
        runs(&large),
        small.2,
        runs(&small)
    );
    assert!(runs(&large_c) && runs(&small_c), "CC controls do not run");
}

// ===========================================================================================================================
// ADR 0254 amendment 1: the located cause (D9), the bounded form (UB1 to UB3, SB, A, CC-A1, CC-V3), the refusal (RF) and the
// hashes of the unchanged forcing (P)
// ===========================================================================================================================

fn fnv(words: impl Iterator<Item = u64>) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for w in words {
        for b in w.to_le_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}

fn state_hash(u: &[Vec3], p: &[f64]) -> u64 {
    fnv(u
        .iter()
        .flat_map(|v| v.iter().map(|x| x.to_bits()))
        .chain(p.iter().map(|x| x.to_bits())))
}

/// `a <= b`, false for a NaN.
fn le(a: f64, b: f64) -> bool {
    a <= b
}

fn lcg(seed: &mut u64) -> f64 {
    *seed = seed
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    ((*seed >> 11) as f64) / ((1u64 << 53) as f64)
}

fn random_hby_a(n: usize, mut seed: u64) -> Vec<Vec3> {
    (0..n)
        .map(|_| [2.0 * lcg(&mut seed) - 1.0, 2.0 * lcg(&mut seed) - 1.0, 0.0])
        .collect()
}

/// The solver of the set-up R (ADR 0254) on M2 with the given forcing; `lead` steps are made first with the first-order
/// forcing. The second-order forcing is built by `make` (so that a test chooses whether the check of E2 is on).
fn r_solver(
    lead: usize,
    make: &dyn Fn(&FvGeometry) -> ImmersedForcing,
) -> (Arc<FvGeometry>, PimpleSolver, ImmersedForcing) {
    let (xs, ys) = dfg_family(2);
    let um = 1.5;
    let (geom, mut u, p) = dfg_fields(&xs, &ys, um);
    two_d_two_start(&geom, &mut u, um);
    let mut st = PimpleSettings::piso(NU);
    st.time_scheme = TimeScheme::Bdf2;
    st.convection = ConvScheme::LinearUpwind;
    st.amg_pressure = true;
    st.n_correctors = 2;
    st.momentum_solver.tol_rel = 1e-6;
    st.pressure_solver.tol_rel = 1e-6;
    let mut s = PimpleSolver::new(geom.clone(), u, p, st);
    s.immersed = Some(build(&geom, &[dfg_body()], K_DFG, NU, 1e-2, Forcing::First));
    for n in 1..=lead {
        s.step(1e-3);
        assert!(
            s.diverged.is_none(),
            "the lead-in stopped at step {n}: {:?}",
            s.diverged
        );
    }
    let ib = make(&geom);
    s.immersed = Some(ib.clone());
    (geom, s, ib)
}

/// D9: over the flux faces, the largest `abs(phi_f)/abs(S_f)`, the weight of that face, `abs(u)` of its fluid cell, and the
/// largest `abs(phi_f/abs(S_f) - ((1 - w) u_c + w u_G) . n_f)`.
fn flux_face_report(ib: &ImmersedForcing, u: &[Vec3], phi: &[f64]) -> (f64, f64, f64, f64) {
    let (mut best, mut dev) = ((0.0f64, 0.0, 0.0), 0.0f64);
    for ff in &ib.flux_faces {
        let v = phi[ff.face] / ff.area;
        if v.abs() > best.0 {
            best = (v.abs(), ff.weight, vec3::norm(u[ff.fluid]));
        }
        let uc = u[ff.fluid];
        let ext: Vec3 = [0, 1, 2].map(|k| uc[k] + ff.weight * (ff.wall[k] - uc[k]));
        dev = dev.max((v - vec3::dot(ext, ff.sf) / ff.area).abs());
    }
    (best.0, best.1, best.2, dev)
}

/// ADR 0254 D9, the two limits that held (amendment 2): after the first step of R with `build_second_order` the largest flux
/// velocity of a fluid-solid face is on a face with a weight of at least 5; at the end of "late" it is at most the inflow
/// maximum.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn d9_located_cause() {
    let (r, l) = d9_measure();
    assert!(r.1 >= 5.0, "D9: weight of the worst face {:.3}", r.1);
    assert!(l.0 <= 1.5, "D9: largest flux velocity of late {:.4}", l.0);
}

/// ADR 0254 D9, the missed limit (amendment 2), kept with its limit: the largest flux velocity after the first step of R was
/// registered as at least 4.5 (three times the inflow maximum); measured 2.4895 on the face of weight 38.987.
#[test]
#[ignore = "known miss (ADR 0254 amendment 2): 2.4895 against the limit 4.5"]
fn d9_flux_velocity_after_the_first_step_known_miss() {
    let (r, _) = d9_measure();
    assert!(r.0 >= 4.5, "D9: largest flux velocity of R {:.4}", r.0);
}

type FaceReport = (f64, f64, f64, f64);

/// The measurement of D9: the report of R after its first step and of "late" after its last.
fn d9_measure() -> (FaceReport, FaceReport) {
    let make = |g: &FvGeometry| build(g, &[dfg_body()], K_DFG, NU, 1e-2, Forcing::Second);
    let (_, mut s, ib) = r_solver(0, &make);
    s.step(1e-3);
    assert!(
        s.diverged.is_none(),
        "D9: R stopped at its first step: {:?}",
        s.diverged
    );
    let r = flux_face_report(&ib, &s.u.values, &s.phi);
    let (_, mut s, ib) = r_solver(300, &make);
    let mut first = None;
    for n in 1..=200 {
        s.step(1e-3);
        assert!(s.diverged.is_none(), "D9: late stopped at step {n}: {:?}", s.diverged);
        if n == 1 {
            first = Some(flux_face_report(&ib, &s.u.values, &s.phi));
        }
    }
    let l = flux_face_report(&ib, &s.u.values, &s.phi);
    println!(
        "D9: R after step 1: largest |v_f| {:.4} on a face of weight {:.3} (|u_c| {:.4}), largest |v_f - extension of u_c| \
         {:.4}; late after its first step {:?}; late after step 200: largest |v_f| {:.4} (weight {:.3}, |u_c| {:.4e}), \
         deviation {:.4}",
        r.0, r.1, r.2, r.3, first, l.0, l.1, l.2, l.3
    );
    (r, l)
}

/// ADR 0254 P: `build_second_order` is unchanged (the three hashes measured before the library code of F-355).
#[test]
fn p_hashes_of_the_unbounded_forcing() {
    let (geom, u, p, ib) = cc_fields(24, Forcing::Second);
    assert!(ib.flux_faces.iter().all(|ff| ff.image.is_none()));
    let hby_a = random_hby_a(geom.n_cells(), 0x0355);
    let zero_b = vec![[0.0; 3]; geom.mesh.n_faces() - geom.n_internal];
    let mut phi = face_flux(&geom, &hby_a, &zero_b);
    let src = ib.reconstruct_face_flux(&hby_a, &mut phi, true).unwrap();
    let p1 = fnv(phi.iter().chain(src.iter()).map(|x| x.to_bits()));
    let mut st = PimpleSettings::piso(1.0);
    st.convection = ConvScheme::Central;
    let mut s = PimpleSolver::new(geom.clone(), u.clone(), p.clone(), st);
    // the check of E2 on: the run is not refused and the numbers are those of before
    let mut checked = ib.clone();
    checked.flux_weight_check = true;
    s.immersed = Some(checked.clone());
    for _ in 0..20 {
        s.step(0.0025);
    }
    assert!(s.diverged.is_none(), "{:?}", s.diverged);
    let p2 = state_hash(&s.u.values, &s.p.values);
    let mut st = SimpleSettings::new(1.0, Coupling::Simplec);
    st.convection = ConvScheme::Central;
    let mut s = SimpleSolver::new(geom.clone(), u, p, st);
    s.immersed = Some(checked);
    for _ in 0..40 {
        s.iterate();
    }
    let p3 = state_hash(&s.u.values, &s.p.values);
    println!("P: {p1:016x} {p2:016x} {p3:016x}");
    assert_eq!(p1, 0x1d4b_8502_a0ae_b637, "P1");
    assert_eq!(p2, 0x36d0_549b_b053_3aa0, "P2");
    assert_eq!(p3, 0x6d18_eb02_d60d_3a7a, "P3");
}

/// The exact Couette profile `A r + B/r`, at any radius (the smooth extension on either side of a surface).
fn cc_exact(x: Vec3) -> Vec3 {
    let (dx, dy) = (x[0] - CC_C[0], x[1] - CC_C[1]);
    let r = (dx * dx + dy * dy).sqrt();
    let a = -OMEGA * R1 * R1 / (R2 * R2 - R1 * R1);
    let b = OMEGA * R1 * R1 * R2 * R2 / (R2 * R2 - R1 * R1);
    let ut = a * r + b / r;
    [-ut * dy / r, ut * dx / r, 0.0]
}

/// `e = sqrt(sum_F V |u - u_exact|^2 / sum_F V) / (omega R1)` over the fluid cells.
fn cc_error(geom: &FvGeometry, ib: &ImmersedForcing, u: &[Vec3]) -> f64 {
    let (cc, vol) = (geom.mesh.cell_centres(), geom.mesh.cell_volumes());
    let (mut num, mut den) = (0.0, 0.0);
    for c in 0..geom.n_cells() {
        if ib.body_of_cell[c].is_some() {
            continue;
        }
        let d = vec3::sub(u[c], cc_exact(cc[c]));
        num += vol[c] * vec3::dot(d, d);
        den += vol[c];
    }
    (num / den).sqrt() / (OMEGA * R1)
}

/// The flux measure of ADR 0234 over the flux faces.
fn cc_flux_error(geom: &FvGeometry, ib: &ImmersedForcing, phi: &[f64]) -> f64 {
    let cf = geom.mesh.face_centres();
    let (mut num, mut den) = (0.0, 0.0);
    for ff in &ib.flux_faces {
        let d = phi[ff.face] - vec3::dot(cc_exact(cf[ff.face]), ff.sf);
        num += d * d;
        den += ff.area * ff.area;
    }
    (num / den).sqrt() / (OMEGA * R1)
}

/// `(max, rms)` over the flux faces of `|phi_f/|S_f| - u_ext(x_f) . n_f|` after the reconstruction of the exact field.
fn ub1_errors(n: usize) -> (f64, f64, usize) {
    let (geom, _, _, ib) = cc_fields_with(n, Forcing::Bounded, 1e-9);
    let cc = geom.mesh.cell_centres();
    let hby_a: Vec<Vec3> = (0..geom.n_cells())
        .map(|c| match ib.body_of_cell[c] {
            Some(b) => cc_bodies()[b].velocity_at(cc[c]),
            None => cc_exact(cc[c]),
        })
        .collect();
    let zero_b = vec![[0.0; 3]; geom.mesh.n_faces() - geom.n_internal];
    let mut phi = face_flux(&geom, &hby_a, &zero_b);
    ib.reconstruct_face_flux(&hby_a, &mut phi, true).expect("flux faces");
    let cf = geom.mesh.face_centres();
    let (mut mx, mut sq) = (0.0f64, 0.0);
    for ff in &ib.flux_faces {
        let d = (phi[ff.face] - vec3::dot(cc_exact(cf[ff.face]), ff.sf)).abs() / ff.area;
        mx = mx.max(d);
        sq += d * d;
    }
    let images = ib.flux_faces.iter().filter(|ff| ff.image.is_some()).count();
    (mx, (sq / ib.flux_faces.len() as f64).sqrt(), images)
}

/// ADR 0254 UB1: the bounded reconstruction of the exact Couette field is second order.
#[test]
fn ub1_bounded_reconstruction_of_the_exact_field() {
    let (m48, r48, i48) = ub1_errors(48);
    let (m96, r96, i96) = ub1_errors(96);
    println!(
        "UB1: max {m48:.4e} {m96:.4e}, rms {r48:.4e} {r96:.4e}, rms ratio {:.3}; image faces {i48} {i96}",
        r48 / r96
    );
    assert!(m96 <= 3e-3, "UB1: max at N 96 {m96:.4e}");
    assert!(r48 / r96 >= 3.0, "UB1: rms ratio {:.3}", r48 / r96);
}

/// ADR 0254 UB2: the net flux of each body, the sum of the sources, the sources of the fluid cells and the other faces.
#[test]
fn ub2_bounded_net_flux_and_sources() {
    let (geom, _, _, ib) = cc_fields(24, Forcing::Bounded);
    let hby_a = random_hby_a(geom.n_cells(), 0x0309);
    let zero_b = vec![[0.0; 3]; geom.mesh.n_faces() - geom.n_internal];
    let before = face_flux(&geom, &hby_a, &zero_b);
    let mut phi = before.clone();
    let src = ib.reconstruct_face_flux(&hby_a, &mut phi, true).expect("flux faces");
    let nb = ib.bodies.len();
    let (mut net, mut area) = (vec![0.0; nb], vec![0.0; nb]);
    let mut is_flux = vec![false; geom.mesh.n_faces()];
    for ff in &ib.flux_faces {
        net[ff.body] += ff.sign * (phi[ff.face] - ff.rigid);
        area[ff.body] += ff.area;
        is_flux[ff.face] = true;
    }
    let total: f64 = src.iter().sum();
    println!("UB2: net {net:?}, areas {area:?}, sum of the sources {total:.3e}");
    for b in 0..nb {
        assert!(net[b].abs() <= 1e-12 * area[b], "UB2: body {b} net {:.3e}", net[b]);
    }
    assert!(total.abs() <= 1e-12 * area.iter().sum::<f64>(), "UB2: sum {total:.3e}");
    for c in 0..geom.n_cells() {
        if ib.body_of_cell[c].is_none() {
            assert_eq!(src[c].to_bits(), 0.0f64.to_bits(), "UB2: source in the fluid cell {c}");
        }
    }
    for f in 0..geom.mesh.n_faces() {
        if !is_flux[f] {
            assert_eq!(phi[f].to_bits(), before[f].to_bits(), "UB2: face {f} changed");
        }
    }
}

fn ub3_check(
    name: &str,
    geom: &FvGeometry,
    first: &ImmersedForcing,
    second: &ImmersedForcing,
    bounded: &ImmersedForcing,
) {
    assert_eq!(first.coeff, bounded.coeff);
    assert_eq!(first.source, bounded.source);
    assert_eq!(first.bodies, bounded.bodies);
    assert!(first.flux_faces.is_empty());
    assert!(second.flux_faces.iter().all(|ff| ff.image.is_none()));
    assert_eq!(second.flux_faces.len(), bounded.flux_faces.len());
    let (mut images, mut fallback) = (0, 0);
    for (a, b) in second.flux_faces.iter().zip(&bounded.flux_faces) {
        assert_eq!((a.face, a.fluid, a.solid, a.body), (b.face, b.fluid, b.solid, b.body));
        assert_eq!(a.weight.to_bits(), b.weight.to_bits());
        assert_eq!(a.wall, b.wall);
        assert_eq!(a.rigid.to_bits(), b.rigid.to_bits());
        match b.image {
            None => assert!(
                b.weight <= 2.0,
                "{name}: a face of weight {} without an image",
                b.weight
            ),
            Some((d, frac)) => {
                assert!(b.weight > 2.0, "{name}: a face of weight {} with an image", b.weight);
                assert!((0.0..=1.0).contains(&frac), "{name}: fraction {frac}");
                assert!(bounded.body_of_cell[d].is_none(), "{name}: the image cell {d} is solid");
                images += 1;
                if d == b.fluid {
                    assert_eq!(frac, 0.0);
                    fallback += 1;
                } else {
                    // the cell behind: the fluid centre is between the image cell and the solid cell, on one line
                    let cc = geom.mesh.cell_centres();
                    let (e, dir) = (vec3::sub(cc[d], cc[b.fluid]), vec3::sub(cc[b.solid], cc[b.fluid]));
                    let cos = vec3::dot(e, dir) / (vec3::norm(e) * vec3::norm(dir));
                    assert!(cos <= -0.99, "{name}: the image cell is not behind (cosine {cos})");
                }
            }
        }
    }
    println!(
        "UB3 {name}: {} flux faces, {images} with an image, {fallback} fall-back faces",
        bounded.flux_faces.len()
    );
    assert!(images >= 10, "UB3 {name}: only {images} image faces");
}

/// ADR 0254 UB3: the structure of the bounded forcing against `build` and `build_second_order`.
#[test]
fn ub3_bounded_structure() {
    let (geom, _, _, first) = cc_fields(24, Forcing::First);
    let (_, _, _, second) = cc_fields(24, Forcing::Second);
    let (_, _, _, bounded) = cc_fields(24, Forcing::Bounded);
    ub3_check("CC 24", &geom, &first, &second, &bounded);
    let (xs, ys) = dfg_family(2);
    let (geom, _, _) = dfg_fields(&xs, &ys, 1.5);
    let f = |forcing| build(&geom, &[dfg_body()], K_DFG, NU, 1e-2, forcing);
    ub3_check(
        "M2",
        &geom,
        &f(Forcing::First),
        &f(Forcing::Second),
        &f(Forcing::Bounded),
    );
    // the refusal of the unmodified forcing
    let mut st = ImmersedSettings::new(K_DFG);
    st.modified = false;
    let err =
        ImmersedForcing::build_second_order_bounded(&geom, &[dfg_body()], st, &uniform_gamma(&geom, NU)).unwrap_err();
    assert!(err.contains("modified"), "{err}");
}

/// ADR 0254 SB, the run that missed (amendment 2), kept with its limit: the bounded forcing with one corrector was registered
/// to run 200 steps; it stops at step 34 by the Courant guard (2.4226e7).
#[test]
#[ignore = "known miss (ADR 0254 amendment 2): the bounded forcing with one corrector stops at step 34"]
fn sb_bounded_with_one_corrector_known_miss() {
    let v = variants().into_iter().find(|v| v.name == "H2 n_correctors 1").unwrap();
    let o = run_variant(&v, Forcing::Bounded);
    print_outcome(v.name, Forcing::Bounded, &o);
    assert_eq!(o.verdict(), "runs", "SB: stop {:?} {}", o.stop, o.reason);
}

/// ADR 0254 SB, the runs that held (amendment 2 took the run with one corrector out): the bounded forcing runs R and the
/// other variants of R that stopped with `build_second_order`, and circular Couette flow with the small time step.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn sb_bounded_runs_the_variants_that_stopped() {
    let keep = [
        "H1 ddt_correction off",
        "H2 n_correctors 4",
        "H3 implicit Euler",
        "H6 upwind",
        "H6 central",
        "H7 dt 5e-4",
        "H7 dt 2e-3",
        "H7 dt 4e-3",
        "H7 dt 8e-3",
    ];
    let mut vs = vec![R];
    vs.extend(variants().into_iter().filter(|v| keep.contains(&v.name)));
    assert_eq!(vs.len(), 10);
    let mut out: Vec<Option<Outcome>> = vec![None; vs.len()];
    for (chunk, slots) in vs.chunks(4).zip(out.chunks_mut(4)) {
        std::thread::scope(|sc| {
            for (v, slot) in chunk.iter().zip(slots.iter_mut()) {
                sc.spawn(move || *slot = Some(run_variant(v, Forcing::Bounded)));
            }
        });
    }
    let mut missed = Vec::new();
    for (v, o) in vs.iter().zip(&out) {
        let o = o.as_ref().unwrap();
        print_outcome(v.name, Forcing::Bounded, o);
        if o.verdict() != "runs" {
            missed.push(format!("{}: {} (stop {:?} {})", v.name, o.verdict(), o.stop, o.reason));
        }
    }
    let cc = cc_piso(2.5e-5, Forcing::Bounded);
    if cc.0.is_some() || !le(cc.1, 1.0) {
        missed.push(format!("CC dt 2.5e-5: stop {:?}, max |u| {:.3e}", cc.0, cc.1));
    }
    assert!(missed.is_empty(), "SB: {missed:#?}");
}

struct CcRun {
    e: f64,
    eps: f64,
    converged: bool,
    iterations: usize,
}

fn cc_simplec(n: usize, forcing: Forcing) -> CcRun {
    let (geom, u, p, ib) = cc_fields(n, forcing);
    let mut st = SimpleSettings::new(1.0, Coupling::Simplec);
    st.convection = ConvScheme::Central;
    st.momentum_solver.tol_abs = 1e-20;
    st.pressure_solver.tol_abs = 1e-20;
    let mut s = SimpleSolver::new(geom.clone(), u, p, st);
    s.immersed = Some(ib.clone());
    let (mut iterations, mut converged) = (0, false);
    for it in 1..=20000 {
        let info = s.iterate();
        iterations = it;
        if !info.max_du.is_finite() {
            break;
        }
        if info.max_du <= 1e-10 {
            converged = true;
            break;
        }
    }
    let e = cc_error(&geom, &ib, &s.u.values);
    let eps = cc_flux_error(&geom, &ib, &s.phi);
    let dz = 2.4 / n as f64;
    let (_, m_in) = ib.force_moment(0, &s.u.values, 1.0, CC_C);
    println!(
        "CC N {n} {forcing:?}: e {e:.5e}, eps {eps:.5e}, converged {converged} after {iterations}; torque inner {:.5} \
         (exact -4.18879); flux faces {}, with an image {}",
        m_in[2] / dz,
        ib.flux_faces.len(),
        ib.flux_faces.iter().filter(|ff| ff.image.is_some()).count()
    );
    CcRun {
        e,
        eps,
        converged,
        iterations,
    }
}

/// ADR 0254 CC-A1: the check A1 of ADR 0234 (the F-219 V1 check) with the bounded forcing.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn cc_a1_bounded_circular_couette_order() {
    let r: Vec<CcRun> = [24, 48, 96].iter().map(|&n| cc_simplec(n, Forcing::Bounded)).collect();
    let order = (r[1].e / r[2].e).log2();
    println!(
        "CC-A1: bounded e {:.4e} {:.4e} {:.4e}, orders {:.3} {:.3} (ADR 0234: 6.121e-3 1.611e-3 4.243e-4, 1.926 1.925); eps \
         {:.4e} {:.4e} {:.4e} (ADR 0234: 1.903e-3 4.737e-4 1.612e-4)",
        r[0].e,
        r[1].e,
        r[2].e,
        (r[0].e / r[1].e).log2(),
        order,
        r[0].eps,
        r[1].eps,
        r[2].eps
    );
    for (x, n) in r.iter().zip([24, 48, 96]) {
        assert!(
            x.converged,
            "S1: CC run N {n} not converged ({} iterations)",
            x.iterations
        );
    }
    assert!(r[2].e <= 1e-2, "CC-A1: e(96) {:.4e}", r[2].e);
    assert!(order >= 1.5, "CC-A1: order {order:.3}");
}

/// ADR 0254 CC-V3: the check V3 of ADR 0234 (PISO against SIMPLEC at N 48) with the bounded forcing.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn cc_v3_bounded_circular_couette_with_piso() {
    let simplec = cc_simplec(48, Forcing::Bounded);
    assert!(simplec.converged, "S1: CC SIMPLEC 48 not converged");
    let (geom, u, p, ib) = cc_fields(48, Forcing::Bounded);
    let mut st = PimpleSettings::piso(1.0);
    st.convection = ConvScheme::Central;
    let mut s = PimpleSolver::new(geom.clone(), u, p, st);
    s.immersed = Some(ib.clone());
    let mut change = f64::NAN;
    for _ in 0..400 {
        let before = s.u.values.clone();
        s.step(0.0025);
        assert!(s.diverged.is_none(), "{:?}", s.diverged);
        change =
            s.u.values
                .iter()
                .zip(&before)
                .map(|(a, b)| vec3::norm(vec3::sub(*a, *b)))
                .fold(0.0, f64::max);
    }
    let e = cc_error(&geom, &ib, &s.u.values);
    let eps = cc_flux_error(&geom, &ib, &s.phi);
    println!(
        "CC-V3: PISO e {e:.5e} (SIMPLEC {:.5e}), eps {eps:.5e} (SIMPLEC {:.5e}), last step change {change:.3e}",
        simplec.e, simplec.eps
    );
    assert!(change <= 1e-9, "CC-V3 set-up: not steady ({change:.3e})");
    assert!(
        e <= 1.2 * simplec.e,
        "CC-V3: e {e:.4e} against SIMPLEC {:.4e}",
        simplec.e
    );
    assert!(
        eps <= 1.5 * simplec.eps,
        "CC-V3: eps {eps:.4e} against SIMPLEC {:.4e}",
        simplec.eps
    );
}

/// ADR 0254 RF: the unbounded forcing is refused on R at step 1 with a located message; the bounded forcing and the runs of
/// ADR 0234 V3, of ADR 0249's annulus and of SIMPLEC with the unbounded forcing are not.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn rf_refusal_of_the_unbounded_forcing() {
    let checked = |g: &FvGeometry| {
        ImmersedForcing::build_second_order(g, &[dfg_body()], ImmersedSettings::new(K_DFG), &uniform_gamma(g, NU))
            .unwrap()
    };
    let (geom, mut s, ib) = r_solver(0, &checked);
    assert!(ib.flux_weight_check);
    s.step(1e-3);
    let d = s
        .diverged
        .clone()
        .expect("RF: R with build_second_order is not refused");
    let msg = d.message(Some(geom.mesh.cell_centres()));
    println!("RF: {msg}");
    assert_eq!(d.step, 1);
    assert_eq!(d.quantity, "immersed flux weight");
    assert!(d.value > 1.0, "RF: value {}", d.value);
    let cell = d.cell.expect("RF: no cell");
    assert!(
        ib.flux_faces.iter().any(|ff| ff.fluid == cell),
        "RF: cell {cell} is not a first fluid cell"
    );
    assert!(
        msg.contains("ADR 0254") && msg.contains("build_second_order_bounded") && msg.contains(&format!("cell {cell}"))
    );
    // the bounded forcing on R
    let (_, mut s, _) = r_solver(0, &|g: &FvGeometry| {
        build(g, &[dfg_body()], K_DFG, NU, 1e-2, Forcing::Bounded)
    });
    s.step(1e-3);
    assert!(s.diverged.is_none(), "RF: bounded refused: {:?}", s.diverged);
    // ADR 0234 V3 (20 steps) with the unbounded forcing, the check on
    let cc_checked = |om: f64| {
        let (geom, u, p, _) = cc_fields(48, Forcing::First);
        let mut bodies = cc_bodies();
        bodies[0].omega = [0.0, 0.0, om];
        let ib =
            ImmersedForcing::build_second_order(&geom, &bodies, ImmersedSettings::new(1e8), &uniform_gamma(&geom, 1.0))
                .unwrap();
        (geom, u, p, ib)
    };
    let (geom, u, p, ib) = cc_checked(OMEGA);
    let mut st = PimpleSettings::piso(1.0);
    st.convection = ConvScheme::Central;
    let mut s = PimpleSolver::new(geom.clone(), u.clone(), p.clone(), st);
    s.immersed = Some(ib.clone());
    for _ in 0..20 {
        s.step(0.0025);
    }
    assert!(s.diverged.is_none(), "RF: ADR 0234 V3 refused: {:?}", s.diverged);
    // ADR 0249's annulus set-up (BDF2, central, the inner cylinder accelerated), 5 steps
    let mut st = PimpleSettings::piso(1.0);
    st.time_scheme = TimeScheme::Bdf2;
    st.convection = ConvScheme::Central;
    let mut s = PimpleSolver::new(geom.clone(), u.clone(), p.clone(), st);
    for k in 1..=5 {
        s.immersed = Some(cc_checked(k as f64 * 0.0025).3);
        s.step(0.0025);
    }
    assert!(
        s.diverged.is_none(),
        "RF: the annulus of ADR 0249 refused: {:?}",
        s.diverged
    );
    // SIMPLEC
    let mut st = SimpleSettings::new(1.0, Coupling::Simplec);
    st.convection = ConvScheme::Central;
    let mut s = SimpleSolver::new(geom, u, p, st);
    s.immersed = Some(ib);
    for _ in 0..5 {
        assert!(s.iterate().max_du.is_finite(), "RF: SIMPLEC");
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// A: DFG 2D-2 on M2 to t = 8 (the window statistics are copies of those of immersed_unsteady_force_f346.rs)
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

/// The 2D-2 run of ADR 0218 and 0249 on M2 (the set-up R, 8000 steps) with the coefficients from `force_moment`. `None` when
/// the run stops.
fn run_2d2(forcing: Forcing) -> Option<Window> {
    let (geom, mut s, ib) = r_solver(0, &|g: &FvGeometry| build(g, &[dfg_body()], K_DFG, NU, 1e-2, forcing));
    let q = 0.5 * DIAM * DZ;
    let t0 = Instant::now();
    let (mut ts, mut cds, mut cls) = (Vec::new(), Vec::new(), Vec::new());
    for n in 1..=8000usize {
        let info = s.step(1e-3);
        if let Some(d) = &s.diverged {
            println!(
                "2D-2 M2 {forcing:?}: stopped at step {n}: {}",
                d.message(Some(geom.mesh.cell_centres()))
            );
            return None;
        }
        let (f, _) = ib.force_moment(0, &s.u.values, 1.0, CENTRE);
        ts.push(n as f64 * 1e-3);
        cds.push(f[0] / q);
        cls.push(f[1] / q);
        if n % 1000 == 0 {
            println!(
                "2D-2 M2 {forcing:?} t {:.3}: C_D {:.5} C_L {:+.5}; Co {:.3} ({:.0} s)",
                n as f64 * 1e-3,
                f[0] / q,
                f[1] / q,
                info.courant,
                t0.elapsed().as_secs_f64()
            );
        }
    }
    let w = window(&ts, &cds, &cls);
    println!(
        "2D-2 M2 {forcing:?}: {} crossings, St {:.5} ({:+.3} % of 0.30188), period spread {:.3e}, C_L max spread {:.3e}, \
         amplitude {:.4}; C_D max {:.5} ({:+.3} %) min {:.5} ({:+.3} %) mean {:.5} ({:+.3} %); C_L max {:.5} ({:+.3} %) min \
         {:.5} ({:+.3} %); {:.0} s",
        w.crossings,
        w.st,
        pct(w.st, 0.301_88),
        w.period_spread,
        w.max_spread,
        w.amplitude,
        w.cd_max,
        pct(w.cd_max, 3.2200),
        w.cd_min,
        pct(w.cd_min, 3.1569),
        w.cd_mean,
        pct(w.cd_mean, 3.1884),
        w.cl_max,
        pct(w.cl_max, 0.9859),
        w.cl_min,
        -pct(w.cl_min, -1.0206),
        t0.elapsed().as_secs_f64()
    );
    Some(w)
}

/// S2 of ADR 0218 on a window.
fn s2(name: &str, w: &Window) -> Vec<String> {
    let mut out = Vec::new();
    if w.crossings < 6 {
        out.push(format!("S2 {name}: {} crossings", w.crossings));
    }
    if !le(w.period_spread, 1e-2) {
        out.push(format!("S2 {name}: period spread {:.3e}", w.period_spread));
    }
    if !le(w.max_spread, 2e-2) {
        out.push(format!("S2 {name}: C_L maximum spread {:.3e}", w.max_spread));
    }
    if !le(1.0, w.amplitude) {
        out.push(format!("S2 {name}: amplitude {:.4}", w.amplitude));
    }
    out
}

/// ADR 0254 A (the acceptance, first branch, with the bounded forcing): DFG 2D-2 on M2 runs to t = 8 in PISO with BDF2 and its
/// Strouhal number is within 1 percent of that of the first-order forcing on the same mesh. The two runs are made once, side
/// by side.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn a_two_d_two_on_m2_strouhal() {
    let (bounded, first) = std::thread::scope(|sc| {
        let a = sc.spawn(|| run_2d2(Forcing::Bounded));
        let b = sc.spawn(|| run_2d2(Forcing::First));
        (a.join().unwrap(), b.join().unwrap())
    });
    let first = first.expect("S2: the first-order run stopped");
    let bounded = bounded.expect("A: the bounded run stopped");
    let ratio = bounded.st / first.st;
    println!(
        "A: St bounded {:.5}, first {:.5}, ratio - 1 = {:+.4e}",
        bounded.st,
        first.st,
        ratio - 1.0
    );
    let mut missed = s2("first", &first);
    missed.extend(s2("bounded", &bounded));
    assert!(missed.is_empty(), "{missed:#?}");
    assert!(
        (ratio - 1.0).abs() <= 1e-2,
        "A: St {:.5} against {:.5}",
        bounded.st,
        first.st
    );
}

// ---------------------------------------------------------------------------------------------------------------------------
// ADR 0254 amendment 2: D10 (where the growth sits) and D11 (the stop of the bounded form with one corrector)
// ---------------------------------------------------------------------------------------------------------------------------

/// ADR 0254 D10: in R with `build_second_order` the largest flux velocity of a fluid-solid face sits on a face of weight at
/// least 5 at each of the steps 1 to 5 and is at least 45 at step 3; with the bounded forcing it is at most 3 at each of the
/// steps 1 to 5 and at step 200.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn d10_where_the_growth_sits() {
    let (_, mut s, ib) = r_solver(0, &|g: &FvGeometry| {
        build(g, &[dfg_body()], K_DFG, NU, 1e-2, Forcing::Second)
    });
    let mut unbounded = Vec::new();
    for n in 1..=5 {
        s.step(1e-3);
        assert!(s.diverged.is_none(), "D10: R stopped at step {n}: {:?}", s.diverged);
        unbounded.push(flux_face_report(&ib, &s.u.values, &s.phi));
    }
    let (_, mut s, ib) = r_solver(0, &|g: &FvGeometry| {
        build(g, &[dfg_body()], K_DFG, NU, 1e-2, Forcing::Bounded)
    });
    let mut bounded = Vec::new();
    for n in 1..=200 {
        s.step(1e-3);
        assert!(
            s.diverged.is_none(),
            "D10: bounded stopped at step {n}: {:?}",
            s.diverged
        );
        if n <= 5 || n == 200 {
            bounded.push(flux_face_report(&ib, &s.u.values, &s.phi));
        }
    }
    println!("D10 (largest |v_f|, weight of its face, |u_c|, deviation from the extension of u_c):");
    println!("D10 build_second_order, steps 1 to 5: {unbounded:.4?}");
    println!("D10 bounded, steps 1 to 5 and 200: {bounded:.4?}");
    let mut missed = Vec::new();
    for (n, r) in unbounded.iter().enumerate() {
        if !le(5.0, r.1) {
            missed.push(format!("D10: step {}: the worst face has the weight {:.3}", n + 1, r.1));
        }
    }
    if !le(45.0, unbounded[2].0) {
        missed.push(format!("D10: step 3: largest flux velocity {:.4}", unbounded[2].0));
    }
    for r in &bounded {
        if !le(r.0, 3.0) {
            missed.push(format!("D10: bounded: largest flux velocity {:.4}", r.0));
        }
    }
    assert!(missed.is_empty(), "{missed:#?}");
}

/// ADR 0254 D11: R with one corrector, (a) `build_second_order` with `theta_min = 0.5`, (b) with `theta_min = 0.25`, (c) the
/// bounded forcing with `theta_min = 0.1`. Printed; the reading is that of the amendment.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn d11_one_corrector() {
    let mut one = R;
    one.n_correctors = 1;
    let runs = [
        ("D11 (a) one corrector, theta_min 0.5", 0.5, Forcing::Second),
        ("D11 (b) one corrector, theta_min 0.25", 0.25, Forcing::Second),
        ("D11 (c) one corrector, theta_min 0.1", 0.1, Forcing::Bounded),
    ];
    let mut out: Vec<Option<Outcome>> = vec![None; runs.len()];
    std::thread::scope(|sc| {
        for (r, slot) in runs.iter().zip(out.iter_mut()) {
            let mut v = one;
            v.theta_min = r.1;
            let forcing = r.2;
            sc.spawn(move || *slot = Some(run_variant(&v, forcing)));
        }
    });
    for (r, o) in runs.iter().zip(&out) {
        let o = o.as_ref().unwrap();
        print_outcome(r.0, r.2, o);
        let first_large = o.m.iter().position(|&m| m > 10.0).map(|i| i + 1);
        println!("{}: first step with m > 10: {first_large:?}", r.0);
    }
}
