//! F-351 (ADR 0252): the 1.25 percent added-mass deficit of the piston column of ADR 0211 and ADR 0250, separated into the
//! wall-pressure extrapolation of `patch_force_moment` and the pressure of the moving-mesh flow step, and the opt-in
//! evaluation with the prescribed wall gradient.
//!
//! The limits, set-ups, model arithmetic and mutations were registered in ADR 0252 before this file existed. The channel,
//! the crossings and the turning points are those of `strong_fsi_f251.rs` (copied with the cell count as a parameter; that
//! file is not edited).

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::ale::{move_mesh, set_follow_conditions};
use rcfd_fv::bc::{FixedValue, ZeroGradient};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::fsi::{
    relaxation_from_added_mass, strong_step, strong_step_with, BodyCoupling, CouplingStep, Relaxation, StrongCoupling,
    WallPressure,
};
use rcfd_fv::gradient::least_squares;
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::sixdof::{patch_force_moment, patch_force_moment_wall_gradient, Body, BodySpec, BodyState, Integrator};
use rcfd_fv::smoothing::{smooth_displacement, PatchClass, SmoothingMethod};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{box_mesh, Grading};
use rcfd_mesh::motion::{moved_mesh, patch_points};
use rcfd_mesh::Mesh;
use std::f64::consts::TAU;
use std::sync::Arc;

const RHO: f64 = 1.0;
const SP_A: f64 = 0.04;
const SP_L0: f64 = 1.0;
const SP_X: f64 = 0.01;
const SP_NU: f64 = 0.01;

fn tight(max_iter: usize) -> SolveControl {
    SolveControl {
        tol_abs: 1e-16,
        tol_rel: 1e-13,
        max_iter,
    }
}

fn sp_settings() -> PimpleSettings {
    let mut s = PimpleSettings::piso(SP_NU);
    s.convection = ConvScheme::Central;
    s.momentum_solver = tight(5000);
    s.pressure_solver = tight(5000);
    s
}

/// The channel of the set-up SPm of ADR 0250 with `n` cells along the column.
struct Sp {
    n: usize,
    mesh0: Mesh,
    g0: FvGeometry,
    classes: Vec<PatchClass>,
    piston: Vec<usize>,
}

impl Sp {
    fn new(n: usize) -> Self {
        let mesh0 = box_mesh([n, 2, 2], [SP_L0, 0.2, 0.2], [Grading::Uniform; 3]).unwrap();
        let g0 = FvGeometry::new(Arc::new(mesh0.clone())).unwrap();
        let classes = mesh0
            .patches()
            .iter()
            .map(|pt| match pt.name.as_str() {
                "xmin" => PatchClass::Prescribed,
                "xmax" => PatchClass::Fixed,
                _ => PatchClass::Slip,
            })
            .collect();
        let piston = patch_points(&mesh0, "xmin").unwrap();
        Sp {
            n,
            mesh0,
            g0,
            classes,
            piston,
        }
    }

    /// Points with the piston at `x_p` (Laplacian smoothing from the initial mesh).
    fn points(&self, x_p: f64) -> Vec<Vec3> {
        let mut presc = vec![vec3::ZERO; self.mesh0.n_points()];
        for &q in &self.piston {
            presc[q] = [x_p, 0.0, 0.0];
        }
        let sm = smooth_displacement(
            &self.g0,
            &self.classes,
            &presc,
            SmoothingMethod::Laplacian,
            &tight(5000),
        )
        .unwrap();
        self.mesh0
            .points()
            .iter()
            .zip(&sm.displacement)
            .map(|(p, d)| vec3::add(*p, *d))
            .collect()
    }

    fn geom_at(&self, x_p: f64) -> Arc<FvGeometry> {
        let mesh = moved_mesh(&self.mesh0, self.points(x_p)).unwrap();
        Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap())
    }

    /// Fluid at rest on the mesh with the piston at `X`, with the solver settings given.
    fn flow_with(&self, settings: PimpleSettings) -> PimpleSolver {
        let geom = self.geom_at(SP_X);
        let mut u = VectorField::uniform("U", &geom, vec3::ZERO);
        for name in ["ymin", "ymax", "zmin", "zmax"] {
            u.set_bc(&geom, name, Box::new(ZeroGradient)).unwrap();
        }
        let mut p = ScalarField::uniform("p", &geom, 0.0);
        p.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
        PimpleSolver::new(geom, u, p, settings)
    }
}

fn piston() -> Vec<String> {
    vec!["xmin".to_string()]
}

// =================================================================================================================
// DP: the prescribed piston (ADR 0252, the separating test)
// =================================================================================================================

/// The flow after one step of DP, with what the step was given.
struct DpState {
    step: usize,
    geom: Arc<FvGeometry>,
    u: VectorField,
    p: ScalarField,
    /// The vector of `set_follow_conditions` of the step.
    walls: Vec<f64>,
    /// Piston position after the step.
    x_p: f64,
    /// Piston position before the step.
    x_old: f64,
    /// `(w_new - w_old)/dt` of the prescribed positions.
    accel: f64,
}

/// DP: the piston at `X cos(2 pi t_n)`, `dt = 0.01`, `steps` steps from rest; `with_walls = false` takes the flow step without
/// the wall gradient (the control N3).
fn dp_run(sp: &Sp, steps: usize, with_walls: bool) -> Vec<DpState> {
    dp_run_with(sp, steps, 0.01, sp_settings(), with_walls)
}

/// [`dp_run`] with the step size and the solver settings given.
fn dp_run_with(sp: &Sp, steps: usize, dt: f64, settings: PimpleSettings, with_walls: bool) -> Vec<DpState> {
    let patches = piston();
    let mut flow = sp.flow_with(settings);
    let x_at = |n: usize| SP_X * (TAU * n as f64 * dt).cos();
    let mut w_old = 0.0;
    let mut out = Vec::new();
    for n in 1..=steps {
        let x_p = x_at(n);
        let w_new = (x_p - x_at(n - 1)) / dt;
        let st = move_mesh(&flow.geom, sp.points(x_p), dt).unwrap();
        let old = flow.geom.mesh.clone();
        let walls = set_follow_conditions(&mut flow.u, &st, &old, &patches, dt).unwrap();
        if with_walls {
            flow.step_ale_with_walls(dt, st.geom, st.mesh_flux, walls.clone())
                .unwrap();
        } else {
            flow.step_ale(dt, st.geom, st.mesh_flux).unwrap();
        }
        out.push(DpState {
            step: n,
            geom: flow.geom.clone(),
            u: flow.u.clone(),
            p: flow.p.clone(),
            walls,
            x_p,
            x_old: x_at(n - 1),
            accel: (w_new - w_old) / dt,
        });
        w_old = w_new;
    }
    out
}

/// The quantities of the hypotheses of ADR 0252 on one state of DP.
#[derive(Debug, Clone, Copy)]
struct Split {
    /// Prescribed outward gradient of the piston faces (mean).
    g: f64,
    /// `(grad p)_c . n / g` in the wall cells (the value farthest from 0.5).
    r: f64,
    d_ext: f64,
    d_flow: f64,
    d_tot: f64,
    /// Rebuilt `rho sum p_f S_x` against the x force of `patch_force_moment`, relative.
    rebuilt: f64,
    /// Force with `nu` against the force without, relative.
    viscous: f64,
    /// `g` against the acceleration of the prescribed positions, relative.
    accel: f64,
    /// Piston area against `A` (absolute).
    area: f64,
    /// Column length against `L0 - x_p` (absolute).
    length: f64,
    /// Spread of `g` and of `p_c` over the piston faces, relative.
    spread: f64,
}

/// Relative deficit `1 - F_x/F_exact` of a force against the force of the exact wall pressure `g L` on the piston.
fn deficit(s: &DpState, force_x: f64) -> f64 {
    let mesh = &s.geom.mesh;
    let ni = s.geom.n_internal;
    let faces: Vec<usize> = mesh.patch("xmin").unwrap().faces().collect();
    let g = faces.iter().map(|&f| s.walls[f - ni]).sum::<f64>() / faces.len() as f64;
    let sx: f64 = faces.iter().map(|&f| mesh.face_area_vectors()[f][0]).sum();
    1.0 - force_x / (RHO * g * (SP_L0 - s.x_p) * sx)
}

fn split(sp: &Sp, s: &DpState) -> Split {
    let geom = &s.geom;
    let mesh = &geom.mesh;
    let ni = geom.n_internal;
    let faces: Vec<usize> = mesh.patch("xmin").unwrap().faces().collect();
    let pb = s.p.boundary_values(geom);
    let grad = least_squares(geom, &s.p.values, &pb);
    let l = SP_L0 - s.x_p;
    let dx = l / sp.n as f64;
    let nf = faces.len() as f64;
    let gs: Vec<f64> = faces.iter().map(|&f| s.walls[f - ni]).collect();
    let g = gs.iter().sum::<f64>() / nf;
    let (mut r, mut d_ext, mut d_flow, mut rebuilt_fx, mut area) = (0.5f64, 0.0, 0.0, 0.0, 0.0);
    let mut pcs = Vec::new();
    for &f in &faces {
        let c = mesh.owner()[f];
        let sf = mesh.face_area_vectors()[f];
        let a = vec3::norm(sf);
        let n = vec3::scale(sf, 1.0 / a);
        let d = vec3::sub(mesh.face_centres()[f], mesh.cell_centres()[c]);
        let pc = s.p.values[c];
        let pf = pc + vec3::dot(grad[c], d);
        let rf = vec3::dot(grad[c], n) / g;
        if (rf - 0.5).abs() > (r - 0.5).abs() {
            r = rf;
        }
        d_ext += (pc + g * dx / 2.0 - pf) / (g * l) / nf;
        d_flow += (1.0 - pc / (g * (l - dx / 2.0))) / nf;
        rebuilt_fx += RHO * pf * sf[0];
        area += a;
        pcs.push(pc);
    }
    let force = |nu: f64| {
        patch_force_moment(geom, &s.u, &s.p, RHO, nu, &piston(), vec3::ZERO)
            .unwrap()
            .0[0]
    };
    let (fx, fx0) = (force(SP_NU), force(0.0));
    let x_wall = faces.iter().map(|&f| mesh.face_centres()[f][0]).sum::<f64>() / nf;
    let out: Vec<usize> = mesh.patch("xmax").unwrap().faces().collect();
    let x_out = out.iter().map(|&f| mesh.face_centres()[f][0]).sum::<f64>() / out.len() as f64;
    let spread = |v: &[f64]| {
        let (lo, hi) = v
            .iter()
            .fold((f64::MAX, f64::MIN), |(lo, hi), x| (lo.min(*x), hi.max(*x)));
        (hi - lo) / hi.abs().max(lo.abs())
    };
    Split {
        g,
        r,
        d_ext,
        d_flow,
        d_tot: deficit(s, fx),
        rebuilt: (rebuilt_fx - fx).abs() / fx.abs(),
        viscous: (fx - fx0).abs() / fx0.abs(),
        accel: (g - s.accel).abs() / s.accel.abs(),
        area: (area - SP_A).abs(),
        length: ((x_out - x_wall) - l).abs(),
        spread: spread(&gs).max(spread(&pcs)),
    }
}

/// The value of `v` farthest from `target`.
fn worst(v: &[f64], target: f64) -> f64 {
    v.iter().copied().fold(
        target,
        |w, x| {
            if (x - target).abs() > (w - target).abs() {
                x
            } else {
                w
            }
        },
    )
}

/// ADR 0252 G1 to G8: on the prescribed piston the deficit of the force is made by the extrapolation of `patch_force_moment`
/// (H1), not by the pressure of the flow step (H2), the viscous part (H4) or the reference (H5).
///
/// Run once on 784ea04e and missed (ADR 0252 amendment 1), the limits intact: G1 `r` 0.499766 (step 12; 0.500126 at step 2)
/// against `0.5 +- 1e-6`; G2 `d_ext` 1.250585e-2 against `0.0125 +- 1e-6`; G3 `d_flow` 4.256e-4 against `1e-4`; G4 `d_tot`
/// 1.292790e-2 against `0.0125 +- 2e-4`; G5 7.229e-6 against `1e-12` (the rebuilt force has no viscous part: a defect of
/// the check); G6 7.229e-6 against `1e-9`; G8 7.136e-6 against `1e-9`. G7 held (3.0e-14, 6.9e-18, 0, 3.2e-14).
#[test]
#[ignore = "known miss, kept as the record of the registered run (ADR 0252 amendment 1)"]
fn g1_to_g8_the_deficit_is_made_by_the_wall_extrapolation_known_miss() {
    let n = 20;
    let sp = Sp::new(n);
    let states = dp_run(&sp, 12, true);
    let all: Vec<Split> = states.iter().map(|s| split(&sp, s)).collect();
    for (s, q) in states.iter().zip(&all) {
        println!("DP N = {n} step {:2}: x_p {:.6e} g {:.6e} {q:?}", s.step, s.x_p, q.g);
    }
    let q = &all[1..];
    let col = |f: fn(&Split) -> f64| q.iter().map(f).collect::<Vec<f64>>();
    let quarter = 0.25 / n as f64;
    let r = worst(&col(|q| q.r), 0.5);
    let d_ext = worst(&col(|q| q.d_ext), quarter);
    let d_flow = worst(&col(|q| q.d_flow), 0.0);
    let d_tot = worst(&col(|q| q.d_tot), quarter);
    let rebuilt = worst(&col(|q| q.rebuilt), 0.0);
    let viscous = worst(&col(|q| q.viscous), 0.0);
    let accel = worst(&col(|q| q.accel), 0.0);
    let area = worst(&col(|q| q.area), 0.0);
    let length = worst(&col(|q| q.length), 0.0);
    let spread = worst(&col(|q| q.spread), 0.0);
    let identity = worst(
        &q.iter()
            .map(|q| q.d_tot - (q.d_ext + q.d_flow * (1.0 - 0.5 / n as f64)))
            .collect::<Vec<f64>>(),
        0.0,
    );
    println!("G1 gradient ratio r = {r:.9} (limit 0.5 +- 1e-6; zero gradient 0, weight 1/d 0.667, unweighted 0.8)");
    println!("G2 d_ext = {d_ext:.9e} (limit 1/80 = 0.0125 +- 1e-6)");
    println!("G3 d_flow = {d_flow:.3e} (limit 1e-4; H2 holds from 5e-3)");
    println!("G4 d_tot = {d_tot:.9e} (limit 0.0125 +- 2e-4)");
    println!("G5 rebuilt force against patch_force_moment: {rebuilt:.3e} (limit 1e-12)");
    println!("G6 viscous part: {viscous:.3e} (limit 1e-9)");
    println!(
        "G7 g against (w_new - w_old)/dt {accel:.3e} (1e-9); area {area:.3e} (1e-12); length {length:.3e} (1e-12); \
         spread {spread:.3e} (1e-9)"
    );
    println!("G8 d_tot - (d_ext + d_flow (1 - 1/(2N))) = {identity:.3e} (limit 1e-9)");
    assert!((r - 0.5).abs() <= 1e-6, "G1 {r}");
    assert!((d_ext - quarter).abs() <= 1e-6, "G2 {d_ext}");
    assert!(d_flow.abs() <= 1e-4, "G3 {d_flow}");
    assert!((d_tot - quarter).abs() <= 2e-4, "G4 {d_tot}");
    assert!(rebuilt <= 1e-12, "G5 {rebuilt}");
    assert!(viscous <= 1e-9, "G6 {viscous}");
    assert!(
        accel <= 1e-9 && area <= 1e-12 && length <= 1e-12 && spread <= 1e-9,
        "G7"
    );
    assert!(identity.abs() <= 1e-9, "G8 {identity}");
}

/// Worst `d_tot` over the steps 2 to 12 of DP with the default evaluation on `n` cells.
fn default_deficit(n: usize) -> f64 {
    let sp = Sp::new(n);
    let states = dp_run(&sp, 12, true);
    let d: Vec<f64> = states[1..]
        .iter()
        .map(|s| {
            let fx = patch_force_moment(&s.geom, &s.u, &s.p, RHO, SP_NU, &piston(), vec3::ZERO)
                .unwrap()
                .0[0];
            deficit(s, fx)
        })
        .collect();
    worst(&d, 0.25 / n as f64)
}

/// ADR 0252 K1, K2: the deficit of the default evaluation is `1/(4N)` on 20, 40 and 80 cells: first order in the cell size.
///
/// Run once on 784ea04e and missed (ADR 0252 amendment 1), the limits intact: K1 `d_tot` 1.292790e-2, 6.697305e-3,
/// 2.330438e-3 against `1/(4N)` within 2 percent (3.4, 7.2 and 25 percent off); K2 orders 0.948836 and 1.522980 against
/// `[0.95, 1.05]`.
#[test]
#[ignore = "known miss, kept as the record of the registered run (ADR 0252 amendment 1)"]
fn k1_k2_the_default_deficit_is_first_order_in_the_cell_size_known_miss() {
    let d: Vec<f64> = [20, 40, 80].iter().map(|&n| default_deficit(n)).collect();
    let orders = [(d[0] / d[1]).log2(), (d[1] / d[2]).log2()];
    println!(
        "K1 default d_tot: N = 20 {:.9e}, 40 {:.9e}, 80 {:.9e} (1/(4N) = 1.25e-2, 6.25e-3, 3.125e-3, each within 2 percent)",
        d[0], d[1], d[2]
    );
    println!(
        "K2 observed orders {:.6} and {:.6} (limit [0.95, 1.05])",
        orders[0], orders[1]
    );
    for (n, d) in [20, 40, 80].iter().zip(&d) {
        let q = 0.25 / *n as f64;
        assert!((d - q).abs() <= 0.02 * q, "K1 N = {n}: {d}");
    }
    for o in orders {
        assert!((0.95..=1.05).contains(&o), "K2 {o}");
    }
}

/// ADR 0252 N3 (control of G3): a flow step taken without the wall gradient has a wrong wall-cell pressure, and `d_flow`
/// sees it.
///
/// Run once on 784ea04e: the control did not hold (ADR 0252 amendment 1): `abs(d_flow)` 1.58e-3 (step 2) to 7.2e-3 (step 12)
/// against `>= 5e-3` at every step. A defective control: where the flux of the wall is prescribed the cell pressures do
/// not depend on the wall coefficient of the pressure equation.
#[test]
#[ignore = "defective control, kept as the record of the registered run (ADR 0252 amendment 1)"]
fn n3_without_the_wall_gradient_the_flow_pressure_is_wrong_and_seen_registered_record() {
    let sp = Sp::new(20);
    let states = dp_run(&sp, 12, false);
    let d: Vec<f64> = states[1..].iter().map(|s| split(&sp, s).d_flow).collect();
    let smallest = d.iter().fold(f64::MAX, |m, x| m.min(x.abs()));
    println!(
        "N3 d_flow without the wall gradient, steps 2 to 12: {d:.4?}; smallest magnitude {smallest:.4e} (limit >= 5e-3)"
    );
    assert!(smallest >= 5e-3, "N3 {smallest}");
}

// =================================================================================================================
// Exploration (ADR 0252 amendment 1): prints only, no limit, decides nothing
// =================================================================================================================

/// One line per step of a DP run: the parts of the deficit against the column of the new and of the old mesh, the pressure
/// difference of the first two cells against `g dx`, the algebraic gradient ratio, the viscous part and the velocity defect
/// of the wall cell.
fn explore(label: &str, n: usize, steps: usize, dt: f64, settings: PimpleSettings, every: usize) {
    let sp = Sp::new(n);
    let nu = settings.nu;
    let states = dp_run_with(&sp, steps, dt, settings, true);
    println!("X1 {label}: N = {n}, dt = {dt}, nu = {nu}");
    println!(
        "X1   step  g           d_tot       d_tot(nu=0) d_ext       d_flow      d_flow_old  s-1         r_alg-0.5   viscous     \
         u_c/w-1"
    );
    for s in &states {
        if s.step % every != 0 && s.step > 4 {
            continue;
        }
        let geom = &s.geom;
        let mesh = &geom.mesh;
        let ni = geom.n_internal;
        let faces: Vec<usize> = mesh.patch("xmin").unwrap().faces().collect();
        let f = faces[0];
        let c = mesh.owner()[f];
        // the cell behind the wall cell: the neighbour of the wall cell along x
        let c1 = (0..ni)
            .filter_map(|i| {
                let (o, nb) = (mesh.owner()[i], mesh.neighbour()[i]);
                let other = if o == c {
                    nb
                } else if nb == c {
                    o
                } else {
                    return None;
                };
                let d = vec3::sub(mesh.cell_centres()[other], mesh.cell_centres()[c]);
                (d[0].abs() > 10.0 * (d[1].abs() + d[2].abs())).then_some(other)
            })
            .next()
            .unwrap();
        let g = s.walls[f - ni];
        let pb = s.p.boundary_values(geom);
        let grad = least_squares(geom, &s.p.values, &pb);
        let sf = mesh.face_area_vectors()[f];
        let nrm = vec3::scale(sf, 1.0 / vec3::norm(sf));
        let d = vec3::sub(mesh.face_centres()[f], mesh.cell_centres()[c]);
        let (pc, p1) = (s.p.values[c], s.p.values[c1]);
        let dxc = (mesh.cell_centres()[c1][0] - mesh.cell_centres()[c][0]).abs();
        let pf = pc + vec3::dot(grad[c], d);
        let l = SP_L0 - s.x_p;
        let dx = l / n as f64;
        let l_old = SP_L0 - s.x_old;
        let force = |nu: f64| {
            patch_force_moment(geom, &s.u, &s.p, RHO, nu, &piston(), vec3::ZERO)
                .unwrap()
                .0[0]
        };
        let (fx, fx0) = (force(SP_NU), force(0.0));
        let w = (s.x_p - s.x_old) / dt;
        println!(
            "X1   {:4}  {:+.4e} {:+.4e} {:+.4e} {:+.4e} {:+.4e} {:+.4e} {:+.4e} {:+.4e} {:+.4e} {:+.4e}",
            s.step,
            g,
            deficit(s, fx),
            deficit(s, fx0),
            (pc + g * dx / 2.0 - pf) / (g * l),
            1.0 - pc / (g * (l - dx / 2.0)),
            1.0 - pc / (g * (l_old - l_old / n as f64 / 2.0)),
            (pc - p1) / (g * dxc) - 1.0,
            vec3::dot(grad[c], nrm) * dxc / (pc - p1) - 0.5,
            (fx - fx0) / fx0,
            s.u.values[c][0] / w - 1.0,
        );
    }
}

/// ADR 0252 amendment 1, X1: exploration of the flow-step part of the deficit (cell count, step size, correctors, outer
/// iterations, viscosity). Prints only.
#[test]
#[ignore = "exploration, prints only (ADR 0252 amendment 1)"]
fn x1_exploration_of_the_flow_step_part_prints_only() {
    for n in [20, 40, 80] {
        explore("base", n, 30, 0.01, sp_settings(), 3);
    }
    for n in [20, 80] {
        explore("half step", n, 60, 0.005, sp_settings(), 6);
    }
    let mut four = sp_settings();
    four.n_correctors = 4;
    explore("four correctors", 80, 30, 0.01, four, 3);
    let mut outer = sp_settings();
    outer.n_outer = 4;
    explore("four outer iterations", 80, 30, 0.01, outer, 3);
    let mut inviscid = sp_settings();
    inviscid.nu = 0.0;
    explore("nu = 0 in the flow", 80, 30, 0.01, inviscid, 3);
}

// =================================================================================================================
// The corrected diagnosis (ADR 0252 amendment 2): the reference is the column of the old mesh
// =================================================================================================================

/// The settings of DPc: the registered ones with four outer iterations, so that the flow step is converged.
fn dpc_settings() -> PimpleSettings {
    let mut s = sp_settings();
    s.n_outer = 4;
    s
}

/// Relative deficit `1 - F_x/F_exact` against the force of the wall pressure `g L_old` (the column before the step).
fn deficit_old(s: &DpState, force_x: f64) -> f64 {
    let mesh = &s.geom.mesh;
    let ni = s.geom.n_internal;
    let faces: Vec<usize> = mesh.patch("xmin").unwrap().faces().collect();
    let g = faces.iter().map(|&f| s.walls[f - ni]).sum::<f64>() / faces.len() as f64;
    let sx: f64 = faces.iter().map(|&f| mesh.face_area_vectors()[f][0]).sum();
    1.0 - force_x / (RHO * g * (SP_L0 - s.x_old) * sx)
}

/// The cell behind a wall cell: its neighbour along x.
fn behind(mesh: &Mesh, n_internal: usize, c: usize) -> usize {
    (0..n_internal)
        .filter_map(|i| {
            let (o, nb) = (mesh.owner()[i], mesh.neighbour()[i]);
            let other = if o == c {
                nb
            } else if nb == c {
                o
            } else {
                return None;
            };
            let d = vec3::sub(mesh.cell_centres()[other], mesh.cell_centres()[c]);
            (d[0].abs() > 10.0 * (d[1].abs() + d[2].abs())).then_some(other)
        })
        .next()
        .unwrap()
}

/// The quantities of amendment 2 on one state (the worst piston face where a face quantity is meant).
#[derive(Debug, Clone, Copy)]
struct SplitOld {
    /// `(grad p)_c . n dx_c/(p_0 - p_1)`: the factor of the fit, apart from the field.
    r_alg: f64,
    d_ext: f64,
    d_flow_old: f64,
    /// Of `patch_force_moment` with the viscosity of the case.
    d_tot_old: f64,
    /// Rebuilt pressure force against `patch_force_moment` with `nu = 0`, relative.
    rebuilt: f64,
    /// Viscous part of the force, relative.
    viscous: f64,
}

/// `d_ext_start` is the value the search for the worst `d_ext` starts from: the target `1/(4N)`. The first version started
/// from 0, which no face value can displace (the defect of ADR 0252 amendment 2a); the records of that run pass 0.
fn split_old(n: usize, s: &DpState, d_ext_start: f64) -> SplitOld {
    let geom = &s.geom;
    let mesh = &geom.mesh;
    let ni = geom.n_internal;
    let faces: Vec<usize> = mesh.patch("xmin").unwrap().faces().collect();
    let pb = s.p.boundary_values(geom);
    let grad = least_squares(geom, &s.p.values, &pb);
    let l = SP_L0 - s.x_p;
    let dx = l / n as f64;
    let l_old = SP_L0 - s.x_old;
    let (mut r_alg, mut d_ext, mut d_flow_old, mut rebuilt_fx) = (0.5f64, d_ext_start, 0.0f64, 0.0);
    for &f in &faces {
        let c = mesh.owner()[f];
        let c1 = behind(mesh, ni, c);
        let g = s.walls[f - ni];
        let sf = mesh.face_area_vectors()[f];
        let nrm = vec3::scale(sf, 1.0 / vec3::norm(sf));
        let d = vec3::sub(mesh.face_centres()[f], mesh.cell_centres()[c]);
        let (pc, p1) = (s.p.values[c], s.p.values[c1]);
        let dxc = (mesh.cell_centres()[c1][0] - mesh.cell_centres()[c][0]).abs();
        let pf = pc + vec3::dot(grad[c], d);
        let ra = vec3::dot(grad[c], nrm) * dxc / (pc - p1);
        if (ra - 0.5).abs() > (r_alg - 0.5).abs() {
            r_alg = ra;
        }
        let de = (pc + g * dx / 2.0 - pf) / (g * l);
        if (de - 0.25 / n as f64).abs() > (d_ext - 0.25 / n as f64).abs() {
            d_ext = de;
        }
        let df = 1.0 - pc / (g * (l_old - l_old / n as f64 / 2.0));
        if df.abs() > d_flow_old.abs() {
            d_flow_old = df;
        }
        rebuilt_fx += RHO * pf * sf[0];
    }
    let force = |nu: f64| {
        patch_force_moment(geom, &s.u, &s.p, RHO, nu, &piston(), vec3::ZERO)
            .unwrap()
            .0[0]
    };
    let (fx, fx0) = (force(SP_NU), force(0.0));
    SplitOld {
        r_alg,
        d_ext,
        d_flow_old,
        d_tot_old: deficit_old(s, fx),
        rebuilt: (rebuilt_fx - fx0).abs() / fx0.abs(),
        viscous: (fx - fx0).abs() / fx0.abs(),
    }
}

/// ADR 0252 amendment 2, G1b to G6b, K1b, K2b: with the flow step converged (DPc) and the column of the old mesh as the
/// reference, the deficit of `patch_force_moment` is the factor one half of its least-squares gradient in the wall cell:
/// `1/(4N)` on 20, 40 and 80 cells, first order, and the pressure of the flow step carries none of it.
///
/// First run on 7d36f5b4 (ADR 0252 amendment 2a): the test's `d_ext` was never set (a defect of the test code), so G2b
/// failed with 0 on 20 cells and the run stopped there; kept as the record. Seen on 20 cells: G1b `-1.1e-16`, G3b
/// `2.086e-6`, K1b `1.2509688e-2`, G5b 0, G6b `7.472e-6`.
#[test]
#[ignore = "defective test code, kept as the record of its run (ADR 0252 amendment 2a)"]
fn g1b_to_g6b_k1b_k2b_the_deficit_is_the_half_gradient_of_the_wall_cell_defective_record() {
    half_gradient_checks(|_| 0.0);
}

/// ADR 0252 amendment 2a: G1b to G6b, K1b, K2b with the test's `d_ext` computed.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn g1c_to_g6c_k1c_k2c_the_deficit_is_the_half_gradient_of_the_wall_cell() {
    half_gradient_checks(|n| 0.25 / n as f64);
}

fn half_gradient_checks(d_ext_start: fn(usize) -> f64) {
    let mut at12 = Vec::new();
    for n in [20usize, 40, 80] {
        let sp = Sp::new(n);
        let states = dp_run_with(&sp, 12, 0.01, dpc_settings(), true);
        let q: Vec<SplitOld> = states[1..].iter().map(|s| split_old(n, s, d_ext_start(n))).collect();
        let quarter = 0.25 / n as f64;
        let col = |f: fn(&SplitOld) -> f64| q.iter().map(f).collect::<Vec<f64>>();
        let r_alg = worst(&col(|q| q.r_alg), 0.5);
        let d_ext = worst(&col(|q| q.d_ext), quarter);
        let d_flow = worst(&col(|q| q.d_flow_old), 0.0);
        let d_tot = worst(&col(|q| q.d_tot_old), quarter);
        let rebuilt = worst(&col(|q| q.rebuilt), 0.0);
        let viscous = worst(&col(|q| q.viscous), 0.0);
        println!(
            "DPc N = {n}: G1b r_alg - 0.5 = {:.3e} (1e-12); G2b d_ext {d_ext:.7e} ({:+.3} percent of 1/(4N); 0.5); G3b \
             d_flow_old {d_flow:.3e} (2e-5); K1b d_tot_old {d_tot:.7e} ({:+.3} percent; 1); G5b {rebuilt:.3e} (1e-12); G6b \
             viscous {viscous:.3e} (1e-4); step 12: d_tot_old {:.7e}",
            r_alg - 0.5,
            100.0 * (d_ext / quarter - 1.0),
            100.0 * (d_tot / quarter - 1.0),
            q[10].d_tot_old
        );
        at12.push(q[10].d_tot_old);
        assert!((r_alg - 0.5).abs() <= 1e-12, "G1b N = {n}: {r_alg}");
        assert!((d_ext - quarter).abs() <= 0.005 * quarter, "G2b N = {n}: {d_ext}");
        assert!(d_flow.abs() <= 2e-5, "G3b N = {n}: {d_flow}");
        assert!((d_tot - quarter).abs() <= 0.01 * quarter, "K1b N = {n}: {d_tot}");
        assert!(rebuilt <= 1e-12, "G5b N = {n}: {rebuilt}");
        assert!(viscous <= 1e-4, "G6b N = {n}: {viscous}");
    }
    let orders = [(at12[0] / at12[1]).log2(), (at12[1] / at12[2]).log2()];
    println!(
        "K2b orders of d_tot_old at step 12: {:.6} and {:.6} (limit [0.98, 1.02])",
        orders[0], orders[1]
    );
    for o in orders {
        assert!((0.98..=1.02).contains(&o), "K2b {o}");
    }
}

/// ADR 0252 amendment 2, G2p, G3p: the same split with the registered settings of the coupled runs (two correctors, one outer
/// iteration) on 20 cells, past the start (steps 3 to 12).
///
/// First run on 7d36f5b4 (ADR 0252 amendment 2a): G2p failed with 0, the same defect of the test code; kept as the record.
/// Seen: G3p `7.063e-6`.
#[test]
#[ignore = "defective test code, kept as the record of its run (ADR 0252 amendment 2a)"]
fn g2p_g3p_the_split_with_the_registered_piso_settings_defective_record() {
    registered_settings_checks(0.0);
}

/// ADR 0252 amendment 2a: G2p, G3p with the test's `d_ext` computed.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn g2q_g3q_the_split_with_the_registered_piso_settings() {
    registered_settings_checks(0.25 / 20.0);
}

fn registered_settings_checks(d_ext_start: f64) {
    let n = 20;
    let sp = Sp::new(n);
    let states = dp_run(&sp, 12, true);
    let q: Vec<SplitOld> = states[2..].iter().map(|s| split_old(n, s, d_ext_start)).collect();
    let quarter = 0.25 / n as f64;
    let d_ext = worst(&q.iter().map(|q| q.d_ext).collect::<Vec<_>>(), quarter);
    let d_flow = worst(&q.iter().map(|q| q.d_flow_old).collect::<Vec<_>>(), 0.0);
    let d_tot = worst(&q.iter().map(|q| q.d_tot_old).collect::<Vec<_>>(), quarter);
    println!(
        "DP N = 20, steps 3 to 12: G2p d_ext {d_ext:.7e} (within 0.5 percent of 0.0125); G3p d_flow_old {d_flow:.3e} (1e-4); \
         d_tot_old {d_tot:.7e} (printed)"
    );
    assert!((d_ext - quarter).abs() <= 0.005 * quarter, "G2p {d_ext}");
    assert!(d_flow.abs() <= 1e-4, "G3p {d_flow}");
}

/// ADR 0252 amendment 2, N3b (control of G3b): the measure of the flow part sees a flow-step pressure that is not converged
/// (80 cells, two correctors, the second step from rest).
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn n3b_the_flow_part_sees_an_unconverged_step() {
    let n = 80;
    let sp = Sp::new(n);
    let states = dp_run(&sp, 2, true);
    let d = split_old(n, &states[1], 0.25 / n as f64).d_flow_old;
    println!("N3b d_flow_old at step 2, N = 80, two correctors: {d:.4e} (limit abs >= 5e-4)");
    assert!(d.abs() >= 5e-4, "N3b {d}");
}

// =================================================================================================================
// The corrected evaluation (ADR 0252 amendment 3)
// =================================================================================================================

/// Added mass of the column, `rho A L0`.
const M_A: f64 = RHO * SP_A * SP_L0;

/// Piston mass at the mass ratio `mu`.
fn mass(mu: f64) -> f64 {
    M_A / mu
}

/// Spring of period 1 at the mass ratio `mu`.
fn stiffness(mu: f64) -> f64 {
    TAU * TAU * (mass(mu) + M_A)
}

/// The piston at rest at `x = X` on its spring.
fn body(mu: f64) -> Body {
    let mut spec = BodySpec::new(mass(mu), [1.0, 1.0, 1.0]);
    spec.translation_locked = [false, true, true];
    spec.stiffness = [stiffness(mu), 0.0, 0.0];
    Body::new(spec, BodyState::at([SP_X, 0.0, 0.0])).unwrap()
}

fn coupling() -> BodyCoupling {
    BodyCoupling {
        rho: RHO,
        nu: SP_NU,
        force_patches: piston(),
        follow_patches: piston(),
        integrator: Integrator::Rk4,
    }
}

/// The registered strong settings of ADR 0250: Aitken from the added-mass factor, tolerance `1e-8`, floor `k X`, cap 20.
fn registered(mu: f64) -> StrongCoupling {
    StrongCoupling {
        relaxation: Relaxation::Aitken {
            initial: relaxation_from_added_mass(mass(mu), M_A),
            max: 2.0,
        },
        tolerance: 1e-8,
        force_scale: stiffness(mu) * SP_X,
        moment_scale: 0.0,
        max_iterations: 20,
    }
}

/// Upward zero crossings by linear interpolation (ADR 0033 R2).
fn upward_crossings(t: &[f64], x: &[f64]) -> Vec<f64> {
    (1..x.len())
        .filter(|&i| x[i - 1] < 0.0 && x[i] >= 0.0)
        .map(|i| t[i - 1] + (t[i] - t[i - 1]) * (-x[i - 1]) / (x[i] - x[i - 1]))
        .collect()
}

/// `omega_m = 2 pi (N - 1)/(t_N - t_1)` over all upward crossings, and their number.
fn measured_omega(t: &[f64], x: &[f64]) -> (f64, usize) {
    let c = upward_crossings(t, x);
    assert!(c.len() >= 2, "fewer than two upward crossings");
    (TAU * (c.len() - 1) as f64 / (c[c.len() - 1] - c[0]), c.len())
}

/// Largest ratio of successive maxima and of the magnitudes of successive minima (turning points refined by the parabola
/// through three samples; the start, at rest, is the first maximum).
fn worst_growth(x: &[f64]) -> f64 {
    let mut maxima = vec![x[0]];
    let mut minima = Vec::new();
    for i in 1..x.len() - 1 {
        let (a, b, c) = (x[i - 1], x[i], x[i + 1]);
        let curv = a - 2.0 * b + c;
        if b > a && b >= c && curv != 0.0 {
            maxima.push(b - (c - a).powi(2) / (8.0 * curv));
        } else if b < a && b <= c && curv != 0.0 {
            minima.push(b - (c - a).powi(2) / (8.0 * curv));
        }
    }
    let mut worst: f64 = 0.0;
    for w in maxima.windows(2) {
        worst = worst.max(w[1] / w[0]);
    }
    for w in minima.windows(2) {
        worst = worst.max(w[1].abs() / w[0].abs());
    }
    worst
}

/// A strongly coupled run of SPm.
struct Run {
    t: Vec<f64>,
    x: Vec<f64>,
    iterations: Vec<usize>,
    all_converged: bool,
}

impl Run {
    /// `omega_m/omega` (the period of the spring with the exact added mass is 1) and the number of crossings.
    fn omega_ratio(&self) -> (f64, usize) {
        let (om, n) = measured_omega(&self.t, &self.x);
        (om / TAU, n)
    }
    fn max_iterations(&self) -> usize {
        self.iterations.iter().copied().max().unwrap_or(0)
    }
    fn mean_iterations(&self) -> f64 {
        self.iterations.iter().sum::<usize>() as f64 / self.iterations.len() as f64
    }
}

/// One step of SPm through `strong_step_with`.
fn step_with(
    sp: &Sp,
    flow: &mut PimpleSolver,
    body: &mut Body,
    settings: &StrongCoupling,
    wall_pressure: WallPressure,
    walls: &mut Vec<f64>,
) -> Result<CouplingStep, String> {
    strong_step_with(
        flow,
        body,
        0.01,
        &coupling(),
        settings,
        wall_pressure,
        walls,
        &mut |b: &Body| Ok(sp.points(b.state.x[0])),
    )
}

/// SPm on `n` cells at the mass ratio `mu`, `T/100`, `periods` periods, with the flow settings and the strong settings given.
fn spm_run(
    n: usize,
    mu: f64,
    periods: f64,
    flow_settings: PimpleSettings,
    settings: &StrongCoupling,
    wall_pressure: WallPressure,
) -> Run {
    let sp = Sp::new(n);
    let mut flow = sp.flow_with(flow_settings);
    let mut body = body(mu);
    let mut walls = Vec::new();
    let mut run = Run {
        t: vec![0.0],
        x: vec![SP_X],
        iterations: Vec::new(),
        all_converged: true,
    };
    let steps = (periods * 100.0).round() as usize;
    for k in 1..=steps {
        let info = step_with(&sp, &mut flow, &mut body, settings, wall_pressure, &mut walls)
            .unwrap_or_else(|e| panic!("step {k}: {e}"));
        run.iterations.push(info.iterations);
        run.all_converged &= info.converged;
        run.t.push(k as f64 * 0.01);
        run.x.push(body.state.x[0]);
    }
    run
}

/// The x force of the corrected evaluation on a state of DP, with the vector of its step.
fn corrected_force(s: &DpState, walls: &[f64]) -> f64 {
    patch_force_moment_wall_gradient(&s.geom, &s.u, &s.p, RHO, SP_NU, &piston(), vec3::ZERO, walls)
        .unwrap()
        .0[0]
}

/// ADR 0252 K3b, N1b: on the prescribed piston the corrected evaluation has no deficit against the column of the old mesh
/// on 20, 40 and 80 cells; handed a zero gradient it has the half cell of the cell value.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn k3b_n1b_the_corrected_evaluation_has_no_deficit() {
    for n in [20usize, 40, 80] {
        let sp = Sp::new(n);
        let states = dp_run_with(&sp, 12, 0.01, dpc_settings(), true);
        let d: Vec<f64> = states[1..]
            .iter()
            .map(|s| deficit_old(s, corrected_force(s, &s.walls)))
            .collect();
        let w = worst(&d, 0.0);
        println!(
            "K3b DPc N = {n}: d_tot_old of the corrected evaluation, worst of steps 2 to 12: {w:.4e} (limit 5e-5)"
        );
        assert!(w.abs() <= 5e-5, "K3b N = {n}: {w}");
        if n == 20 {
            let ni = states[0].geom.n_internal;
            let faces: Vec<usize> = states[0].geom.mesh.patch("xmin").unwrap().faces().collect();
            let z: Vec<f64> = states[1..]
                .iter()
                .map(|s| {
                    let mut zero = s.walls.clone();
                    for &f in &faces {
                        zero[f - ni] = 0.0;
                    }
                    deficit_old(s, corrected_force(s, &zero))
                })
                .collect();
            let wz = worst(&z, 0.025);
            println!("N1b DPc N = 20: d_tot_old with a zero gradient: {wz:.6e} (limit within 2 percent of 0.025)");
            assert!((wz - 0.025).abs() <= 0.02 * 0.025, "N1b {wz}");
        }
    }
    let sp = Sp::new(20);
    let states = dp_run(&sp, 12, true);
    let d: Vec<f64> = states[2..]
        .iter()
        .map(|s| deficit_old(s, corrected_force(s, &s.walls)))
        .collect();
    let w = worst(&d, 0.0);
    println!("K3b DP (two correctors) N = 20, steps 3 to 12: {w:.4e} (limit 1e-4)");
    assert!(w.abs() <= 1e-4, "K3b DP {w}");
}

/// A static box of `n` cells along x with the cell pressure `f(x_c)`, a zero velocity and the default (zero-gradient) wall
/// condition of the pressure on `xmin`; returns the geometry, the fields and a wall vector with `g` on `xmin`.
fn static_box(n: usize, f: impl Fn(f64) -> f64, g: f64) -> (FvGeometry, VectorField, ScalarField, Vec<f64>) {
    let mesh = box_mesh([n, 2, 2], [1.0, 0.2, 0.2], [Grading::Uniform; 3]).unwrap();
    let geom = FvGeometry::new(Arc::new(mesh)).unwrap();
    let u = VectorField::uniform("U", &geom, vec3::ZERO);
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    for (c, x) in geom.mesh.cell_centres().iter().enumerate() {
        p.values[c] = f(x[0]);
    }
    let mut walls = vec![f64::NAN; geom.mesh.n_boundary_faces()];
    for face in geom.mesh.patch("xmin").unwrap().faces() {
        walls[face - geom.n_internal] = g;
    }
    (geom, u, p, walls)
}

/// ADR 0252 U1, U2: on a static box the corrected wall pressure is second order and exact for a linear field; the default
/// is first order and low by `1/(4N)` on the linear field.
#[test]
fn u1_u2_order_of_the_wall_pressure() {
    let curved = |x: f64| (2.0 * x).cos() + 3.0 * (1.0 - x);
    let linear = |x: f64| 3.0 * (1.0 - x);
    let (mut ec, mut ed) = (Vec::new(), Vec::new());
    for n in [20usize, 40, 80] {
        let (geom, u, p, walls) = static_box(n, curved, 3.0);
        let exact = -RHO * SP_A * curved(0.0);
        let c = patch_force_moment_wall_gradient(&geom, &u, &p, RHO, 0.0, &piston(), vec3::ZERO, &walls)
            .unwrap()
            .0[0];
        let d = patch_force_moment(&geom, &u, &p, RHO, 0.0, &piston(), vec3::ZERO)
            .unwrap()
            .0[0];
        ec.push((c - exact).abs() / exact.abs());
        ed.push((d - exact).abs() / exact.abs());
        let (geom, u, p, walls) = static_box(n, linear, 3.0);
        let exact = -RHO * SP_A * 3.0;
        let c = patch_force_moment_wall_gradient(&geom, &u, &p, RHO, 0.0, &piston(), vec3::ZERO, &walls)
            .unwrap()
            .0[0];
        let d = patch_force_moment(&geom, &u, &p, RHO, 0.0, &piston(), vec3::ZERO)
            .unwrap()
            .0[0];
        let (lc, ld) = ((c - exact).abs() / exact.abs(), 1.0 - d / exact);
        println!(
            "U2 N = {n}: linear field: corrected error {lc:.3e} (limit 1e-12); default deficit {ld:.12e} against 1/(4N) = {:.12e} \
             (limit 1e-12)",
            0.25 / n as f64
        );
        assert!(lc <= 1e-12, "U2 corrected N = {n}: {lc}");
        assert!((ld - 0.25 / n as f64).abs() <= 1e-12, "U2 default N = {n}: {ld}");
    }
    let order = |e: &[f64]| [(e[0] / e[1]).log2(), (e[1] / e[2]).log2()];
    let (oc, od) = (order(&ec), order(&ed));
    let (ec, ed) = (
        format!("{:.4e}, {:.4e}, {:.4e}", ec[0], ec[1], ec[2]),
        format!("{:.4e}, {:.4e}, {:.4e}", ed[0], ed[1], ed[2]),
    );
    println!("U1 corrected errors {ec}, orders {oc:.4?} (limit [1.8, 2.2]); default errors {ed}, orders {od:.4?} (limit [0.8, 1.2])");
    for o in oc {
        assert!((1.8..=2.2).contains(&o), "U1 corrected {o}");
    }
    for o in od {
        assert!((0.8..=1.2).contains(&o), "U1 default {o}");
    }
}

/// ADR 0252 U3, U4: a vector without a finite entry gives `patch_force_moment` bit for bit; a vector of the wrong length is
/// refused, by the force and by the coupled step, which leaves the body and the flow as they were.
#[test]
fn u3_u4_fall_back_and_refusals() {
    let (geom, mut u, p, walls) = static_box(20, |x: f64| (2.0 * x).cos() + 3.0 * (1.0 - x), 3.0);
    for (c, x) in geom.mesh.cell_centres().iter().enumerate() {
        u.values[c] = [0.3 * x[0], 0.1 * x[1], -0.2 * x[0] * x[2]];
    }
    let none = vec![f64::NAN; walls.len()];
    let x_ref = [0.3, 0.05, -0.02];
    let patches = vec!["xmin".to_string(), "ymin".to_string()];
    let a = patch_force_moment_wall_gradient(&geom, &u, &p, RHO, SP_NU, &patches, x_ref, &none).unwrap();
    let b = patch_force_moment(&geom, &u, &p, RHO, SP_NU, &patches, x_ref).unwrap();
    let bits = |v: (Vec3, Vec3)| v.0.iter().chain(&v.1).map(|x| x.to_bits()).collect::<Vec<u64>>();
    println!("U3 force and moment without a finite entry: {a:?} against {b:?}");
    assert_eq!(bits(a), bits(b), "U3");
    let c = patch_force_moment_wall_gradient(&geom, &u, &p, RHO, SP_NU, &patches, x_ref, &walls).unwrap();
    assert_ne!(bits(c), bits(b), "U3: a finite entry changes the force");
    let short = &walls[1..];
    let e = patch_force_moment_wall_gradient(&geom, &u, &p, RHO, SP_NU, &patches, x_ref, short).unwrap_err();
    println!("U4 force: {e}");
    let sp = Sp::new(20);
    let mut flow = sp.flow_with(sp_settings());
    let mut body = body(4.0);
    let (x0, v0, t0) = (body.state.x, body.state.v, flow.time);
    let mut bad = vec![f64::NAN; flow.geom.mesh.n_boundary_faces() - 1];
    let e = step_with(
        &sp,
        &mut flow,
        &mut body,
        &registered(4.0),
        WallPressure::PrescribedGradient,
        &mut bad,
    )
    .unwrap_err();
    println!("U4 step: {e}");
    assert_eq!((body.state.x, body.state.v, flow.time), (x0, v0, t0), "U4 state");
    assert_eq!(bad.len(), flow.geom.mesh.n_boundary_faces() - 1, "U4 vector");
}

/// ADR 0252 A1 (acceptance): with the corrected force the gain of the sub-iteration at a mass ratio of 4 is within 0.2
/// percent of the model's 2.977 (ADR 0250 C1 with the corrected evaluation).
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn a1_the_gain_of_the_sub_iteration_is_the_model_gain() {
    let mu = 4.0;
    let sp = Sp::new(20);
    let mut all = Vec::new();
    for w in [0.25, 0.5, 1.0] {
        let mut flow = sp.flow_with(sp_settings());
        let mut body = body(mu);
        let settings = StrongCoupling {
            relaxation: Relaxation::Fixed(w),
            tolerance: 1e-300,
            force_scale: stiffness(mu) * SP_X,
            moment_scale: 0.0,
            max_iterations: 8,
        };
        let info = step_with(
            &sp,
            &mut flow,
            &mut body,
            &settings,
            WallPressure::PrescribedGradient,
            &mut Vec::new(),
        )
        .unwrap();
        let r: Vec<f64> = info.force_residuals.iter().map(|v| v[0]).collect();
        let gains: Vec<f64> = (1..=5).map(|k| (1.0 - r[k] / r[k - 1]) / w).collect();
        println!("A1 w = {w}: gains {gains:.5?} (limit [2.971046, 2.982954]; model 2.977)");
        all.extend(gains);
    }
    for g in all {
        assert!((2.971046..=2.982954).contains(&g), "A1 gain {g}");
    }
}

/// ADR 0252 A2, A3 (acceptance): with the corrected force the strongly coupled piston at a mass ratio of 4 on 20 cells
/// oscillates at `omega` within 0.1 percent.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn a2_a3_the_frequency_with_the_corrected_force_at_mass_ratio_4() {
    let run = spm_run(
        20,
        4.0,
        11.5,
        sp_settings(),
        &registered(4.0),
        WallPressure::PrescribedGradient,
    );
    let (ratio, crossings) = run.omega_ratio();
    let growth = worst_growth(&run.x);
    println!(
        "A2 corrected, mu = 4, N = 20: omega_m/omega = {ratio:.6} (limit abs(. - 1) <= 1e-3), {crossings} crossings (>= 11)"
    );
    println!(
        "A3 converged {}; iterations largest {} (<= 8), mean {:.3} (<= 5); largest turning ratio {growth:.5} (<= 1.001)",
        run.all_converged,
        run.max_iterations(),
        run.mean_iterations()
    );
    assert!((ratio - 1.0).abs() <= 1e-3, "A2 {ratio}");
    assert!(crossings >= 11, "A2 crossings {crossings}");
    assert!(run.all_converged, "A3 converged");
    assert!(
        run.max_iterations() <= 8 && run.mean_iterations() <= 5.0,
        "A3 iterations"
    );
    assert!(growth <= 1.0 + 1e-3, "A3 {growth}");
}

/// ADR 0252 A4, A5: the same at mass ratios of 20 and 0.2, and for the loose loop (one iteration) at 0.2.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn a4_a5_the_frequency_with_the_corrected_force_at_other_mass_ratios() {
    let heavy = spm_run(
        20,
        20.0,
        11.5,
        sp_settings(),
        &registered(20.0),
        WallPressure::PrescribedGradient,
    );
    let light = spm_run(
        20,
        0.2,
        6.5,
        sp_settings(),
        &registered(0.2),
        WallPressure::PrescribedGradient,
    );
    let mut one = registered(0.2);
    one.max_iterations = 1;
    let loose = spm_run(20, 0.2, 6.5, sp_settings(), &one, WallPressure::PrescribedGradient);
    let (rh, rl, ro) = (heavy.omega_ratio().0, light.omega_ratio().0, loose.omega_ratio().0);
    println!("A4 corrected: mu = 20: {rh:.6}; mu = 0.2: {rl:.6} (limit abs(. - 1) <= 1e-3 each)");
    println!("A5 corrected, one iteration (the loose loop), mu = 0.2: {ro:.6} (limit abs(. - 1) <= 1e-3)");
    assert!((rh - 1.0).abs() <= 1e-3, "A4 mu = 20: {rh}");
    assert!((rl - 1.0).abs() <= 1e-3, "A4 mu = 0.2: {rl}");
    assert!((ro - 1.0).abs() <= 1e-3, "A5 {ro}");
}

/// ADR 0252 K4, N2b: with the default force the frequency of the coupled piston at a mass ratio of 4 carries the deficit
/// `1/(4N)` on 20, 40 and 80 cells (the model's values); the 20-cell run is the control of A2.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn k4_n2b_the_frequency_with_the_default_force_on_20_40_80_cells() {
    let windows = [(20usize, 1.0049, 1.0052), (40, 1.0024, 1.0027), (80, 1.0012, 1.0014)];
    let mut ratios = Vec::new();
    for (n, _, _) in windows {
        let run = spm_run(
            n,
            4.0,
            11.5,
            sp_settings(),
            &registered(4.0),
            WallPressure::Extrapolated,
        );
        let (ratio, _) = run.omega_ratio();
        println!(
            "K4 default, mu = 4, N = {n}: omega_m/omega = {ratio:.6}; converged {}, iterations largest {}, mean {:.3}",
            run.all_converged,
            run.max_iterations(),
            run.mean_iterations()
        );
        ratios.push(ratio);
    }
    println!("K4 windows [1.0049, 1.0052], [1.0024, 1.0027], [1.0012, 1.0014]; N2b: N = 20 minus 1 >= 4e-3");
    assert!(ratios[0] - 1.0 >= 4e-3, "N2b {}", ratios[0]);
    for ((n, lo, hi), r) in windows.iter().zip(&ratios) {
        assert!((*lo..=*hi).contains(r), "K4 N = {n}: {r}");
    }
}

/// ADR 0252 K5: with the corrected force the frequency is the model's on 40 and 80 cells too.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn k5_the_frequency_with_the_corrected_force_on_40_and_80_cells() {
    let mut ratios = Vec::new();
    for n in [40usize, 80] {
        let run = spm_run(
            n,
            4.0,
            11.5,
            sp_settings(),
            &registered(4.0),
            WallPressure::PrescribedGradient,
        );
        let (ratio, _) = run.omega_ratio();
        println!(
            "K5 corrected, mu = 4, N = {n}: omega_m/omega = {ratio:.6} (limit abs(. - 1) <= 1e-3); converged {}, iterations \
             largest {}, mean {:.3}",
            run.all_converged,
            run.max_iterations(),
            run.mean_iterations()
        );
        ratios.push((n, ratio));
    }
    for (n, r) in ratios {
        assert!((r - 1.0).abs() <= 1e-3, "K5 N = {n}: {r}");
    }
}

/// ADR 0252 K4c, K5c: the 80-cell runs with the flow step converged (four outer iterations).
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn k4c_k5c_the_80_cell_runs_with_a_converged_flow_step() {
    let d = spm_run(
        80,
        4.0,
        11.5,
        dpc_settings(),
        &registered(4.0),
        WallPressure::Extrapolated,
    );
    let c = spm_run(
        80,
        4.0,
        11.5,
        dpc_settings(),
        &registered(4.0),
        WallPressure::PrescribedGradient,
    );
    let (rd, rc) = (d.omega_ratio().0, c.omega_ratio().0);
    println!("K4c default, N = 80, four outer iterations: {rd:.6} (limit [1.0012, 1.0014])");
    println!("K5c corrected, N = 80, four outer iterations: {rc:.6} (limit abs(. - 1) <= 1e-3)");
    assert!((1.0012..=1.0014).contains(&rd), "K4c {rd}");
    assert!((rc - 1.0).abs() <= 1e-3, "K5c {rc}");
}

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

/// ADR 0252 B2: `strong_step` is `strong_step_with` with the default wall pressure, bit for bit.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn b2_the_default_entry_is_unchanged() {
    let mu = 4.0;
    let sp = Sp::new(20);
    let settings = registered(mu);
    let cpl = coupling();
    let (mut flow_a, mut body_a) = (sp.flow_with(sp_settings()), body(mu));
    let (mut flow_b, mut body_b) = (sp.flow_with(sp_settings()), body(mu));
    let mut walls = Vec::new();
    let mut differing = 0usize;
    for _ in 0..40 {
        strong_step(&mut flow_a, &mut body_a, 0.01, &cpl, &settings, &mut |b: &Body| {
            Ok(sp.points(b.state.x[0]))
        })
        .unwrap();
        step_with(
            &sp,
            &mut flow_b,
            &mut body_b,
            &settings,
            WallPressure::Extrapolated,
            &mut walls,
        )
        .unwrap();
        differing += (body_a.state.x[0].to_bits() != body_b.state.x[0].to_bits()) as usize
            + (body_a.state.v[0].to_bits() != body_b.state.v[0].to_bits()) as usize;
    }
    let flat = |f: &PimpleSolver| {
        let mut v: Vec<f64> = f.u.values.iter().flatten().copied().collect();
        v.extend(&f.p.values);
        v.extend(&f.phi);
        v
    };
    differing += bits(&flat(&flow_a))
        .iter()
        .zip(&bits(&flat(&flow_b)))
        .filter(|(a, b)| a != b)
        .count();
    println!(
        "B2: 40 steps, values differing in x, v after any step and in u, p, phi at the end: {differing} (limit 0); x = {:.9e}",
        body_a.state.x[0]
    );
    assert_eq!(differing, 0, "B2");
}

/// ADR 0252 W1: the vector carried from step to step is the wall gradient of the last flow step.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn w1_the_carried_vector_is_the_gradient_of_the_last_step() {
    let mu = 4.0;
    let sp = Sp::new(20);
    let mut flow = sp.flow_with(sp_settings());
    let mut body = body(mu);
    let mut walls = Vec::new();
    let (mut x_old, mut w_old) = (SP_X, 0.0);
    let mut worst_rel: f64 = 0.0;
    for k in 1..=5 {
        step_with(
            &sp,
            &mut flow,
            &mut body,
            &registered(mu),
            WallPressure::PrescribedGradient,
            &mut walls,
        )
        .unwrap();
        let x = body.state.x[0];
        let w = (x - x_old) / 0.01;
        let a = (w - w_old) / 0.01;
        let ni = flow.geom.n_internal;
        assert_eq!(walls.len(), flow.geom.mesh.n_boundary_faces(), "W1 length at step {k}");
        let on: Vec<usize> = flow.geom.mesh.patch("xmin").unwrap().faces().map(|f| f - ni).collect();
        for (i, g) in walls.iter().enumerate() {
            if on.contains(&i) {
                worst_rel = worst_rel.max((g - a).abs() / a.abs());
            } else {
                assert!(g.is_nan(), "W1 entry {i} off the piston at step {k}: {g}");
            }
        }
        (x_old, w_old) = (x, w);
    }
    println!("W1: 5 steps, piston entries against (w_n - w_(n-1))/dt: {worst_rel:.3e} (limit 1e-9); the others NaN");
    assert!(worst_rel <= 1e-9, "W1 {worst_rel}");
}

/// ADR 0252 amendment 4, A1s: the gain of the sub-iteration with the corrected force at the third step of a run (a moving
/// flow, a carried wall gradient), where the force of an iterate must be evaluated with the gradient of that iterate's own
/// flow step and not with the carried one (the mutation MU4b, which A1 on the first step cannot see).
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn a1s_the_gain_at_a_later_step_is_the_model_gain() {
    let mu = 4.0;
    let sp = Sp::new(20);
    let mut all = Vec::new();
    for w in [0.25, 0.5] {
        let mut flow = sp.flow_with(sp_settings());
        let mut body = body(mu);
        let mut walls = Vec::new();
        for _ in 0..2 {
            let info = step_with(
                &sp,
                &mut flow,
                &mut body,
                &registered(mu),
                WallPressure::PrescribedGradient,
                &mut walls,
            )
            .unwrap();
            assert!(info.converged, "A1s set-up");
        }
        let settings = StrongCoupling {
            relaxation: Relaxation::Fixed(w),
            tolerance: 1e-300,
            force_scale: stiffness(mu) * SP_X,
            moment_scale: 0.0,
            max_iterations: 6,
        };
        let info = step_with(
            &sp,
            &mut flow,
            &mut body,
            &settings,
            WallPressure::PrescribedGradient,
            &mut walls,
        )
        .unwrap();
        let r: Vec<f64> = info.force_residuals.iter().map(|v| v[0]).collect();
        let gains: Vec<f64> = (1..=3).map(|k| (1.0 - r[k] / r[k - 1]) / w).collect();
        println!("A1s step 3, w = {w}: residuals {r:?}, gains {gains:.5?} (limit [2.971046, 2.982954])");
        all.extend(gains);
    }
    for g in all {
        assert!((2.971046..=2.982954).contains(&g), "A1s gain {g}");
    }
}
