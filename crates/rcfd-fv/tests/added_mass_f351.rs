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
use rcfd_fv::gradient::least_squares;
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::sixdof::patch_force_moment;
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

    /// Fluid at rest on the mesh with the piston at `X`.
    fn flow(&self) -> PimpleSolver {
        let geom = self.geom_at(SP_X);
        let mut u = VectorField::uniform("U", &geom, vec3::ZERO);
        for name in ["ymin", "ymax", "zmin", "zmax"] {
            u.set_bc(&geom, name, Box::new(ZeroGradient)).unwrap();
        }
        let mut p = ScalarField::uniform("p", &geom, 0.0);
        p.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
        PimpleSolver::new(geom, u, p, sp_settings())
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
    /// `(w_new - w_old)/dt` of the prescribed positions.
    accel: f64,
}

/// DP: the piston at `X cos(2 pi t_n)`, `dt = 0.01`, `steps` steps from rest; `with_walls = false` takes the flow step without
/// the wall gradient (the control N3).
fn dp_run(sp: &Sp, steps: usize, with_walls: bool) -> Vec<DpState> {
    let dt = 0.01;
    let patches = piston();
    let mut flow = sp.flow();
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
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn g1_to_g8_the_deficit_is_made_by_the_wall_extrapolation() {
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
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn k1_k2_the_default_deficit_is_first_order_in_the_cell_size() {
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
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn n3_without_the_wall_gradient_the_flow_pressure_is_wrong_and_seen() {
    let sp = Sp::new(20);
    let states = dp_run(&sp, 12, false);
    let d: Vec<f64> = states[1..].iter().map(|s| split(&sp, s).d_flow).collect();
    let smallest = d.iter().fold(f64::MAX, |m, x| m.min(x.abs()));
    println!(
        "N3 d_flow without the wall gradient, steps 2 to 12: {d:.4?}; smallest magnitude {smallest:.4e} (limit >= 5e-3)"
    );
    assert!(smallest >= 5e-3, "N3 {smallest}");
}
