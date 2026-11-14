//! F-531 (ADR 0455 D5): F-401's check VS repeated with the corrected component diagonal. VS (ADR 0434) runs QF1, the elastic
//! bar on the open column with four `Symmetry` side walls (ratio 4, matching wall, Aitken, 25 steps), at `nu = 1e-2` against
//! `nu = 1e-4`, limit `1e-6 U` on the tip signal; it missed with `8.28e-5 U` (`vs_viscosity_times_100_known_miss`).
//! ADR 0455 D1 attributes a part of it to the averaged component diagonal of the PISO correctors next to the `Symmetry`
//! walls; here the same runs are made with `PimpleSettings::component_diagonal = true`.
//!
//! The limits were registered in ADR 0455 before this file existed. The bar, the column and the coupled run are those of
//! `si_fsi_piston.rs` (ADR 0434; copied, reduced to what QF1 needs; that file is not edited).

use rcfd_core::vec3::{self, Vec3};
use rcfd_fem::builders::hex_box;
use rcfd_fem::transient::{LoadHistory, LoadTable, TransientOptions};
use rcfd_fem::{Constraint, Load, Material, SolidModel};
use rcfd_fv::bc::{FixedValue, Symmetry};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::cosim::{coupled_step, AtCap, CouplingSettings, FlowWall, Fluid, InterfaceMap, MapSettings, Structure};
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::{box_mesh, Grading};
use rcfd_mesh::motion::moved_mesh;
use rcfd_mesh::Mesh;
use rcfd_post::solid_dynamic::DynamicSolid;
use rcfd_post::solid_fsi::ChannelMotion;
use std::f64::consts::PI;

const L_S: f64 = 4.0;
const W: f64 = 1.0;
const NX: usize = 20;
const NF: usize = 4;
const E: f64 = 2e11;
const RHO_S: f64 = 7850.0;
const L_F: f64 = 32.0;
const NXF: usize = 24;
const EXPANSION: f64 = 15.0;
const NU: f64 = 1e-4;
const U: f64 = 1e-3;
const STEPS: usize = 1001;
const TOL: f64 = 1e-6;
const WALL: &str = "xmin";
/// ADR 0434's measured VS deviation (three printed digits).
const VS_ADR0434: f64 = 8.28e-5;

/// Measured values against their limits; every value is printed, the test fails at the end if any missed.
struct Checks(Vec<String>);

impl Checks {
    fn new() -> Self {
        Checks(Vec::new())
    }
    fn at_most(&mut self, id: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{id}: {value:e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{id}: {value:e} > {limit:e}"));
        }
    }
    fn holds(&mut self, id: &str, ok: bool, what: &str) {
        println!("{id}: {what} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{id}: {what}"));
        }
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:#?}", self.0);
    }
}

/// The first root of `x tan x = rhs` on `(0, pi/2)`.
fn root(rhs: f64) -> f64 {
    let (mut lo, mut hi) = (0.0f64, PI / 2.0);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if mid * mid.tan() < rhs {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

#[derive(Debug, Clone, Copy)]
struct Params {
    dt: f64,
    rho_f: f64,
    /// The release force `P = E A U/L_s`.
    p: f64,
}

fn params(ratio: f64) -> Params {
    let kl = root(1.0 / ratio);
    let omega = kl * (E / RHO_S).sqrt() / L_S;
    let dt = 2.0 * PI / omega / 100.0;
    let a = W * W;
    Params {
        dt,
        rho_f: ratio * RHO_S * L_S / L_F,
        p: E * a * U / L_S,
    }
}

/// The bar: Hex8 20 x 4 x 4, nu = 0, clamped on xmin, the release traction `P/A` in `+x` on xmax.
fn bar(p: &Params) -> SolidModel {
    let mut model = SolidModel::new(hex_box([0.0; 3], [L_S, W, W], [NX, NF, NF]).unwrap());
    model
        .add_material("all", Material::isotropic_elastic(E, 0.0, RHO_S, 0.0).unwrap())
        .unwrap();
    for c in 0..3 {
        model.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    model
        .add_load(Load::traction("xmax", [p.p / (W * W), 0.0, 0.0]))
        .unwrap();
    model
}

/// The solid at the static displaced state, at rest, its release table ending at `end_steps` steps.
fn solid(p: &Params, end_steps: usize) -> DynamicSolid {
    let model = bar(p);
    let table = LoadTable::new(vec![(0.0, 1.0), (p.dt, 0.0), (end_steps as f64 * p.dt, 0.0)]).unwrap();
    let loads = [LoadHistory::model_loads(&model, table).unwrap()];
    let mut s = DynamicSolid::new("solid", &model, "xmax", &loads, &TransientOptions::new(p.dt), 1).unwrap();
    let dofs = model.dof_map();
    let n = s.integrator().n_unknowns();
    let mut u0 = vec![0.0; n];
    for (node, x) in model.mesh().nodes().iter().enumerate() {
        if let Some(e) = dofs.equation(node, 0) {
            u0[e] = U * x[0] / L_S;
        }
    }
    s.start(0.0, &u0, &vec![0.0; n]).unwrap();
    s
}

/// The channel in the reference configuration: `NF x NF` cells across, 24 graded along `x`, the wall `xmin` at `x = L_s`.
fn channel() -> Mesh {
    let mesh = box_mesh(
        [NXF, NF, NF],
        [L_F, W, W],
        [Grading::Expansion(EXPANSION), Grading::Uniform, Grading::Uniform],
    )
    .unwrap();
    let points = mesh.points().iter().map(|p| [p[0] + L_S, p[1], p[2]]).collect();
    moved_mesh(&mesh, points).unwrap()
}

fn settings(nu: f64, corrected: bool) -> PimpleSettings {
    let mut s = PimpleSettings::piso(nu);
    s.convection = ConvScheme::Central;
    for c in [&mut s.momentum_solver, &mut s.pressure_solver] {
        c.tol_abs = 1e-16;
        c.tol_rel = 1e-13;
        c.max_iter = 5000;
    }
    s.component_diagonal = corrected;
    s
}

/// Fluid at rest, `p = 0`, on `mesh` (points as given).
fn flow(mesh0: &Mesh, points: Vec<Vec3>, nu: f64, corrected: bool) -> PimpleSolver {
    let mesh = moved_mesh(mesh0, points).unwrap();
    let geom = std::sync::Arc::new(FvGeometry::new(std::sync::Arc::new(mesh)).unwrap());
    let mut u = VectorField::uniform("U", &geom, vec3::ZERO);
    for name in ["ymin", "ymax", "zmin", "zmax"] {
        u.set_bc(&geom, name, Box::new(Symmetry)).unwrap();
    }
    let mut pr = ScalarField::uniform("p", &geom, 0.0);
    pr.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
    PimpleSolver::new(geom, u, pr, settings(nu, corrected))
}

fn quad_area(points: &[Vec3], f: &[usize]) -> f64 {
    let d1 = vec3::sub(points[f[2]], points[f[0]]);
    let d2 = vec3::sub(points[f[3]], points[f[1]]);
    0.5 * vec3::norm(vec3::cross(d1, d2))
}

fn interface_displacement(s: &DynamicSolid) -> Vec<Vec3> {
    let model = bar(&params(1.0));
    let dofs = model.dof_map();
    let u = s.displacement();
    s.interface()
        .nodes()
        .iter()
        .map(|&n| {
            let at = |c: usize| dofs.equation(n, c).map_or(0.0, |e| u[e]);
            [at(0), at(1), at(2)]
        })
        .collect()
}

fn mean_x(q: &[Vec3]) -> f64 {
    q.iter().map(|v| v[0]).sum::<f64>() / q.len() as f64
}

/// QF1 of ADR 0434 (ratio 4, matching wall, Aitken, 25 steps) at `nu`: the signal (mean interface `x` displacement) at
/// `t_0 .. t_n`; `None` if a coupled step failed.
fn qf1(nu: f64, corrected: bool) -> Option<Vec<f64>> {
    let p = params(4.0);
    let mut solid = solid(&p, STEPS + 2);
    let mesh0 = channel();
    let motion = ChannelMotion::new(&mesh0, WALL, 0, L_F, 1e-9).unwrap();
    let probe = FlowWall::new(
        flow(&mesh0, mesh0.points().to_vec(), nu, corrected),
        WALL,
        &mesh0,
        p.rho_f,
        nu,
        motion.clone().into_motion(),
    )
    .unwrap();
    let (map, _): (InterfaceMap, _) = InterfaceMap::build(
        &solid.node_coordinates(),
        &solid.elements(),
        &probe.point_coordinates(),
        probe.face_points(),
        &MapSettings::projection(1e-9),
    )
    .unwrap();
    let q0 = interface_displacement(&solid);
    let initial_points = map.displacement_to_points(&q0).unwrap();
    let start = motion.positions(&initial_points).unwrap();
    let mut fluid = FlowWall::new(
        flow(&mesh0, start, nu, corrected),
        WALL,
        &mesh0,
        p.rho_f,
        nu,
        motion.into_motion(),
    )
    .unwrap();
    let nodes = solid.node_coordinates();
    let mut release = vec![vec3::ZERO; nodes.len()];
    for el in solid.elements() {
        let a = quad_area(&nodes, &el);
        for &i in &el {
            release[i][0] += p.p / (W * W) * a / 4.0;
        }
    }
    let mut coupling = CouplingSettings::aitken(0.5);
    coupling.tolerance = TOL;
    coupling.force_scale = release.iter().flatten().map(|x| x * x).sum::<f64>().sqrt();
    coupling.max_iterations = 10;
    coupling.at_cap = AtCap::Stop;
    let mut signal = vec![mean_x(&q0)];
    for k in 0..25 {
        match coupled_step(&mut fluid, &mut solid, &map, p.dt, &coupling) {
            Ok(s) => signal.push(mean_x(&s.node_displacements)),
            Err(e) => {
                println!("QF1 nu {nu:e} corrected {corrected}: step {} failed: {e:?}", k + 1);
                return None;
            }
        }
    }
    Some(signal)
}

fn deviation(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).fold(0.0f64, |w, (x, y)| w.max((x - y).abs())) / U
}

#[test]
#[cfg_attr(debug_assertions, ignore = "run in release")]
fn v0_v1_v2_vs_repeated_with_the_corrected_component_diagonal() {
    let mut c = Checks::new();
    let runs: Vec<Option<Vec<f64>>> = [(NU, false), (1e-2, false), (NU, true), (1e-2, true), (1e-3, true)]
        .into_iter()
        .map(|(nu, corrected)| qf1(nu, corrected))
        .collect();
    let complete = runs.iter().all(|r| r.as_ref().is_some_and(|s| s.len() == 26));
    c.holds("V runs complete", complete, "25 steps each");
    if let [Some(d4), Some(d2), Some(c4), Some(c2), Some(c3)] = &runs[..] {
        let v0 = deviation(d2, d4);
        println!("V0 default nu 1e-2 against 1e-4: {v0:e} U (ADR 0434: {VS_ADR0434:e} U)");
        c.at_most("V0 default against ADR 0434's VS", (v0 / VS_ADR0434 - 1.0).abs(), 1e-3);
        let v1 = deviation(c2, c4);
        c.at_most("V1 corrected: VS viscosity x 100 [U]", v1, 1e-6);
        let v2 = deviation(c3, c4);
        println!("V2 (record) corrected nu 1e-3 against 1e-4: {v2:e} U");
        println!("corrected against default at nu 1e-4: {:e} U", deviation(c4, d4));
    }
    c.finish();
}
