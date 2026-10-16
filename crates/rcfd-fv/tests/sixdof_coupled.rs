//! Flow-coupled rigid-body checks C1 and C2 of ADR 0033 (loose explicit coupling): a plate on a fluid layer whose
//! decay is set by the Couette shear force, and a piston that pushes a fluid column whose added mass shrinks.
//! The limits are those registered in the ADR before the code ran.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::ale::{move_mesh, set_follow_conditions};
use rcfd_fv::bc::{FixedValue, ZeroGradient};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::sixdof::{patch_force_moment, Body, BodySpec, BodyState, Integrator};
use rcfd_fv::smoothing::{smooth_displacement, PatchClass, SmoothingMethod};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{box_2d, box_mesh, Grading};
use std::sync::Arc;

const TIGHT: SolveControl = SolveControl {
    tol_abs: 1e-16,
    tol_rel: 1e-13,
    max_iter: 5000,
};

fn settings(nu: f64) -> PimpleSettings {
    let mut s = PimpleSettings::piso(nu);
    s.convection = ConvScheme::Central;
    s.momentum_solver = TIGHT;
    s.pressure_solver = TIGHT;
    s
}

/// Plate on a fluid layer. `periodic`: the registered set-up (cyclic x faces); otherwise the x faces are open
/// (`ZeroGradient` velocity, `p = 0` at `xmax`), which is exact for the x-uniform Couette flow.
fn c1_run(periodic: bool) -> f64 {
    let (rho, nu, h, mass, u0) = (1.0, 0.1, 0.1, 100.0, 1.0);
    let mesh = box_2d(4, 20, 1.0, h, periodic, false).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let area = 1.0 * 0.25;
    let mut u = VectorField::from_fn("U", &geom, |c| [u0 * c[1] / h, 0.0, 0.0]);
    u.set_bc(&geom, "ymin", Box::new(FixedValue::vector(vec3::ZERO)))
        .unwrap();
    u.set_bc(&geom, "ymax", Box::new(FixedValue::vector([u0, 0.0, 0.0])))
        .unwrap();
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    if !periodic {
        for name in ["xmin", "xmax"] {
            u.set_bc(&geom, name, Box::new(ZeroGradient)).unwrap();
        }
        p.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
    }
    let mut flow = PimpleSolver::new(geom.clone(), u, p, settings(nu));
    let mut spec = BodySpec::new(mass, [1.0, 1.0, 1.0]);
    spec.translation_locked = [false, true, true];
    let mut state = BodyState::at([0.5, h, 0.0]);
    state.v = [u0, 0.0, 0.0];
    let mut body = Body::new(spec, state).unwrap();
    let tau = mass * h / (rho * nu * area);
    let dt = 1.0;
    let patches = ["ymax".to_string()];
    for _ in 0..(tau / dt).round() as usize {
        let (f, m) = patch_force_moment(&flow.geom, &flow.u, &flow.p, rho, nu, &patches, body.state.x).unwrap();
        body.advance(dt, Integrator::Rk4, (f, m)).unwrap();
        let g = flow.geom.clone();
        flow.u
            .set_bc(&g, "ymax", Box::new(FixedValue::vector([body.state.v[0], 0.0, 0.0])))
            .unwrap();
        flow.step(dt);
    }
    body.state.v[0] / (u0 * (-1.0f64).exp())
}

/// As registered (periodic in x). The baseline transient solver is unstable here as soon as the plate velocity changes
/// in time: the lagged coupling of the cyclic faces leaves an x-nonuniformity of the order of the velocity change,
/// which seeds a vertical velocity that grows by a factor of about 15 per step (reproduced without the body, with a
/// prescribed ramp of the wall velocity on a fixed mesh; ADR 0033 addendum).
#[test]
#[ignore = "baseline PISO instability with cyclic faces and a time-dependent wall (fixed-mesh control reproduces it)"]
fn c1_plate_on_a_fluid_layer_decays_with_the_couette_force_known_miss() {
    let ratio = c1_run(true);
    println!("C1 periodic: U(tau) / (U0 e^-1) = {ratio:.5}");
    assert!((ratio - 1.0).abs() <= 1e-2, "{ratio}");
}

/// Corrected set-up: open x faces instead of cyclic ones; everything else as registered.
#[test]
fn c1b_plate_on_a_fluid_layer_decays_with_the_couette_force() {
    let ratio = c1_run(false);
    println!("C1b: U(tau) / (U0 e^-1) = {ratio:.5} (limit |ratio - 1| <= 1e-2)");
    assert!((ratio - 1.0).abs() <= 1e-2, "{ratio}");
}

/// The corrected piston channel of the G5 checks (side planes `ZeroGradient`).
struct Piston {
    mesh0: rcfd_mesh::Mesh,
    solver: PimpleSolver,
    classes: Vec<PatchClass>,
}

impl Piston {
    fn new(nx: usize, l0: f64, w: f64, dz: f64) -> Self {
        let mesh0 = box_mesh([nx, 2, 2], [l0, w, dz], [Grading::Uniform; 3]).unwrap();
        let geom = Arc::new(FvGeometry::new(Arc::new(mesh0.clone())).unwrap());
        let mut u = VectorField::uniform("U", &geom, vec3::ZERO);
        for name in ["ymin", "ymax", "zmin", "zmax"] {
            u.set_bc(&geom, name, Box::new(ZeroGradient)).unwrap();
        }
        let mut p = ScalarField::uniform("p", &geom, 0.0);
        p.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
        let classes = mesh0
            .patches()
            .iter()
            .map(|pt| match pt.name.as_str() {
                "xmin" => PatchClass::Prescribed,
                "xmax" => PatchClass::Fixed,
                _ => PatchClass::Slip,
            })
            .collect();
        let solver = PimpleSolver::new(geom, u, p, settings(0.01));
        Piston { mesh0, solver, classes }
    }

    fn step(&mut self, x_p: f64, dt: f64) -> rcfd_fv::pimple::StepInfo {
        let g0 = FvGeometry::new(Arc::new(self.mesh0.clone())).unwrap();
        let mut presc = vec![vec3::ZERO; self.mesh0.n_points()];
        for q in rcfd_mesh::motion::patch_points(&self.mesh0, "xmin").unwrap() {
            presc[q] = [x_p, 0.0, 0.0];
        }
        let sm = smooth_displacement(&g0, &self.classes, &presc, SmoothingMethod::Laplacian, &TIGHT).unwrap();
        let pts: Vec<Vec3> = self
            .mesh0
            .points()
            .iter()
            .zip(&sm.displacement)
            .map(|(p, d)| vec3::add(*p, *d))
            .collect();
        let st = move_mesh(&self.solver.geom, pts, dt).unwrap();
        let old = self.solver.geom.mesh.clone();
        let walls = set_follow_conditions(&mut self.solver.u, &st, &old, &["xmin".to_string()], dt).unwrap();
        self.solver
            .step_ale_with_walls(dt, st.geom, st.mesh_flux, walls)
            .unwrap()
    }
}

#[test]
fn c2_piston_with_a_fluid_column() {
    let (rho, area, l0, mass, force) = (1.0, 0.04, 1.0, 0.2, 1.0);
    let mut pc = Piston::new(20, l0, 0.2, 0.2);
    let mut spec = BodySpec::new(mass, [1.0, 1.0, 1.0]);
    spec.translation_locked = [false, true, true];
    let mut body = Body::new(spec, BodyState::at([0.0, 0.0, 0.0])).unwrap();
    body.set_hook(Box::new(move |_s, _t| ([force, 0.0, 0.0], vec3::ZERO)));
    let dt = 0.005;
    let patches = ["xmin".to_string()];
    let mut worst_cont: f64 = 0.0;
    let mut steps = 0;
    while body.state.x[0] < 0.5 && steps < 2000 {
        let (f, m) = patch_force_moment(
            &pc.solver.geom,
            &pc.solver.u,
            &pc.solver.p,
            rho,
            0.01,
            &patches,
            body.state.x,
        )
        .unwrap();
        body.advance(dt, Integrator::Rk4, (f, m)).unwrap();
        let info = pc.step(body.state.x[0], dt);
        worst_cont = worst_cont.max(info.continuity);
        steps += 1;
    }
    let (x, v) = (body.state.x[0], body.state.v[0]);
    assert!(x >= 0.5, "the piston did not reach x = 0.5 in {steps} steps");
    let m0 = mass + rho * area * l0;
    let v2_ex = -(2.0 * force / (rho * area)) * (1.0 - rho * area * x / m0).ln();
    let ratio = v * v / v2_ex;
    println!("C2: x = {x:.4} after {steps} steps, v^2 / v_exact^2 = {ratio:.5} (limit |ratio - 1| <= 2e-2), continuity {worst_cont:.2e}");
    assert!((ratio - 1.0).abs() <= 2e-2, "{ratio}");
    assert!(worst_cont <= 1e-9, "{worst_cont}");
}
