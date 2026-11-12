//! F-355 (ADR 0254): the second-order immersed forcing in transient PISO at Re 100.
//!
//! ADR 0249's printed run of DFG 2D-2 on M2 with `ImmersedForcing::build_second_order` stopped at step 6 (the Courant guard).
//! The first part of this file is the registered diagnosis: the reproduction (D0), one variant run per hypothesis with its
//! control (the same run with `ImmersedForcing::build`), and the estimate `G` of ADR 0254 (the extrapolation weight of a
//! fluid-solid face times the share of the time term in the diagonal of its fluid cell) against the verdict of every run (D1
//! to D8). The set-up helpers are copies of those of `immersed_unsteady_force_f346.rs` and `immersed_second_order_f309.rs`
//! (those files are not edited). The heavy tests run in release only.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::ddt::TimeScheme;
use rcfd_fv::diffusion::uniform_gamma;
use rcfd_fv::immersed::{ImmersedBody, ImmersedForcing, ImmersedSettings, SolidShape};
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::{FixedValue, FvGeometry, Profile, ScalarField, VectorField, ZeroGradient};
use rcfd_mesh::gen::{axis_coords, box_2d, box_2d_coords, expansion_for_end_cell, Grading};
use std::sync::Arc;

/// Which forcing a run uses.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Forcing {
    /// `ImmersedForcing::build` with `modified = true` (ADR 0218).
    First,
    /// `ImmersedForcing::build_second_order` (ADR 0234).
    Second,
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
        Forcing::Second => ImmersedForcing::build_second_order(geom, bodies, st, &gamma).unwrap(),
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
    let ib = if forcing == Forcing::First {
        first.clone()
    } else {
        second.clone()
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
    let geom = Arc::new(FvGeometry::new(Arc::new(box_2d(n, n, 2.4, 2.4, false, false).unwrap())).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    for w in ["xmin", "xmax", "ymin", "ymax"] {
        u.set_bc(&geom, w, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    let p = ScalarField::uniform("p", &geom, 0.0);
    let ib = build(&geom, &cc_bodies(), 1e8, 1.0, 1e-2, forcing);
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
