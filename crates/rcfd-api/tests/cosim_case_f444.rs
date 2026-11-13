//! F-444 (ADR 0380): the co-simulation fluid participant built from the case (`[cosim]`, `rcfd_api::cosim_case`), the
//! elements of a structure in `cosim.describe`, and `service::Client`'s single write. In-process checks; the `rcfd serve`
//! process is driven by `crates/rcfd-cli/tests/cosim_serve_f444.rs`.
//!
//! The limits, set-ups and mutations were registered in ADR 0380 (with amendment 1) before this file existed. The set-up
//! CWs is copied from `crates/rcfd-api/tests/cosim_service_f360.rs` (not edited); CWc is CWs written as a case.

use rcfd_api::case::Case;
use rcfd_api::cosim_case::{fluid_from_case, present};
use rcfd_api::cosim_service::{RemoteFluid, RemoteSettings, RemoteStructure};
use rcfd_api::service::{Client, Service, ServiceConfig};
use rcfd_api::sim::Simulation;
use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::bc::{FixedValue, Symmetry, ZeroGradient};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::cosim::{
    coupled_step, piecewise_linear_rows, AtCap, CouplingSettings, FlowWall, Fluid, InterfaceMap, MapSettings,
    MeshMotion, Participant, SprungWall, Structure,
};
use rcfd_fv::fsi::{relaxation_from_added_mass, Relaxation};
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::smoothing::{smooth_displacement, PatchClass, SmoothingMethod};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{box_mesh, Grading};
use rcfd_mesh::motion::{moved_mesh, patch_points};
use rcfd_mesh::Mesh;
use serde_json::{json, Value};
use std::f64::consts::{PI, TAU};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

const RHO: f64 = 1.0;
const WALL: &str = "xmin";

fn tight(max_iter: usize) -> SolveControl {
    SolveControl {
        tol_abs: 1e-16,
        tol_rel: 1e-13,
        max_iter,
    }
}

// ---- the hand-built CWs (`cosim_service_f360.rs`) ---------------------------------------------------------------------

struct Channel {
    mesh0: Mesh,
    g0: FvGeometry,
    classes: Vec<PatchClass>,
    wall: Vec<usize>,
    nu: f64,
}

impl Channel {
    fn new(cells: [usize; 3], size: [f64; 3], nu: f64) -> Arc<Self> {
        let mesh0 = box_mesh(cells, size, [Grading::Uniform; 3]).unwrap();
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
        let wall = patch_points(&mesh0, WALL).unwrap();
        Arc::new(Channel {
            mesh0,
            g0,
            classes,
            wall,
            nu,
        })
    }

    fn points(&self, d: &[Vec3]) -> Result<Vec<Vec3>, String> {
        let mut presc = vec![vec3::ZERO; self.mesh0.n_points()];
        for (&q, v) in self.wall.iter().zip(d) {
            presc[q] = *v;
        }
        let sm = smooth_displacement(
            &self.g0,
            &self.classes,
            &presc,
            SmoothingMethod::Laplacian,
            &tight(5000),
        )
        .map_err(|e| e.to_string())?;
        Ok(self
            .mesh0
            .points()
            .iter()
            .zip(&sm.displacement)
            .map(|(p, d)| vec3::add(*p, *d))
            .collect())
    }

    fn settings(&self) -> PimpleSettings {
        let mut s = PimpleSettings::piso(self.nu);
        s.convection = ConvScheme::Central;
        s.momentum_solver = tight(5000);
        s.pressure_solver = tight(5000);
        s
    }

    fn flow(&self, d: &[Vec3]) -> PimpleSolver {
        let mesh = moved_mesh(&self.mesh0, self.points(d).unwrap()).unwrap();
        let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
        let mut u = VectorField::uniform("U", &geom, vec3::ZERO);
        for name in ["ymin", "ymax"] {
            u.set_bc(&geom, name, Box::new(Symmetry)).unwrap();
        }
        for name in ["zmin", "zmax"] {
            u.set_bc(&geom, name, Box::new(ZeroGradient)).unwrap();
        }
        let mut p = ScalarField::uniform("p", &geom, 0.0);
        p.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
        PimpleSolver::new(geom, u, p, self.settings())
    }

    fn wall_y(&self) -> Vec<f64> {
        self.wall.iter().map(|&q| self.mesh0.points()[q][1]).collect()
    }
}

fn motion(ch: &Arc<Channel>) -> MeshMotion {
    let ch = ch.clone();
    Box::new(move |d| ch.points(d))
}

const CW_L: f64 = 1.0;
const CW_W: f64 = 0.4;
const CW_LZ: f64 = 0.1;
const CW_MU: f64 = 4.0;
const CW_X: f64 = 0.002;
const CW_NU: f64 = 1e-4;
const CW_NODES: usize = 6;
const CW_DT: f64 = 0.01;

fn cw_m() -> f64 {
    RHO * CW_L / CW_MU
}

fn cw_k() -> f64 {
    TAU * TAU * (cw_m() + RHO * CW_L)
}

fn cw_channel() -> Arc<Channel> {
    Channel::new([20, 8, 2], [CW_L, CW_W, CW_LZ], CW_NU)
}

fn cw_node_y() -> Vec<f64> {
    (0..CW_NODES).map(|i| CW_W * i as f64 / (CW_NODES - 1) as f64).collect()
}

fn cw_tributary() -> Vec<f64> {
    let h = CW_W / (CW_NODES - 1) as f64;
    (0..CW_NODES)
        .map(|i| if i == 0 || i == CW_NODES - 1 { 0.5 * h } else { h })
        .collect()
}

fn cw_q0() -> Vec<f64> {
    cw_node_y()
        .iter()
        .map(|y| CW_X * (1.0 + 0.5 * (PI * y / CW_W).cos()))
        .collect()
}

fn cw_structure() -> SprungWall {
    let t = cw_tributary();
    SprungWall::new(
        cw_node_y().iter().map(|y| [0.0, *y, 0.5 * CW_LZ]).collect(),
        [1.0, 0.0, 0.0],
        t.iter().map(|t| cw_m() * CW_LZ * t).collect(),
        t.iter().map(|t| cw_k() * CW_LZ * t).collect(),
        cw_q0(),
    )
    .unwrap()
}

fn along_x(q: &[f64]) -> Vec<Vec3> {
    q.iter().map(|x| [*x, 0.0, 0.0]).collect()
}

fn map_points(rows: &[Vec<(usize, f64)>], q: &[Vec3]) -> Vec<Vec3> {
    rows.iter()
        .map(|row| {
            row.iter()
                .fold(vec3::ZERO, |a, &(i, w)| vec3::add(a, vec3::scale(q[i], w)))
        })
        .collect()
}

fn rows_for(wall_y: &[f64]) -> Vec<Vec<(usize, f64)>> {
    piecewise_linear_rows(&cw_node_y(), wall_y).unwrap()
}

fn cw_fluid(ch: &Arc<Channel>) -> FlowWall {
    let rows = rows_for(&ch.wall_y());
    let flow = ch.flow(&map_points(&rows, &along_x(&cw_q0())));
    FlowWall::new(flow, WALL, &ch.mesh0, RHO, CW_NU, motion(ch)).unwrap()
}

fn cw_settings() -> CouplingSettings {
    CouplingSettings {
        relaxation: Relaxation::Aitken {
            initial: relaxation_from_added_mass(cw_m(), RHO * CW_L),
            max: 2.0,
        },
        tolerance: 1e-8,
        force_scale: cw_k() * CW_X * CW_W * CW_LZ,
        max_iterations: 20,
        at_cap: AtCap::Stop,
    }
}

// ---- CWc: CWs as a case (ADR 0380 set-up) ------------------------------------------------------------------------------

/// The case of CWc with `smoothing_tolerance`; `initial` the 81 numbers of `H q_0`.
fn cwc_json(smoothing_tolerance: f64) -> Value {
    let ch = cw_channel();
    let d0 = map_points(&rows_for(&ch.wall_y()), &along_x(&cw_q0()));
    let initial: Vec<f64> = d0.iter().flatten().copied().collect();
    json!({
        "mesh": {"generator": "box_3d", "nx": 20, "ny": 8, "nz": 2, "lx": CW_L, "ly": CW_W, "lz": CW_LZ},
        "physics": {"nu": CW_NU, "rho": RHO},
        "boundaries": {
            "xmin": {"velocity": "zero_gradient"},
            "xmax": {"velocity": "zero_gradient", "pressure": "fixed", "pressure_value": 0.0},
            "ymin": {"velocity": "symmetry"},
            "ymax": {"velocity": "symmetry"},
            "zmin": {"velocity": "zero_gradient"},
            "zmax": {"velocity": "zero_gradient"}
        },
        "solver": {
            "algorithm": "piso", "convection": "central", "n_correctors": 2, "alpha_u": 1.0, "alpha_p": 1.0,
            "momentum_rel_tol": 1e-13, "pressure_rel_tol": 1e-13, "pressure_abs_tol": 1e-16,
            "momentum_max_iter": 5000, "pressure_max_iter": 5000, "dt": CW_DT
        },
        "cosim": {
            "patch": WALL, "fixed_patches": ["xmax"], "smoothing_tolerance": smoothing_tolerance,
            "initial_displacement": initial
        }
    })
}

fn case_of(v: &Value) -> Case {
    Case::from_json(v).unwrap_or_else(|e| panic!("CWc does not parse: {e:?}"))
}

fn cwc() -> Case {
    case_of(&cwc_json(1e-13))
}

fn cwc_fluid() -> FlowWall {
    fluid_from_case(&cwc()).unwrap_or_else(|e| panic!("CWc does not build: {e:?}"))
}

fn map_of(fluid: &dyn Fluid) -> InterfaceMap {
    let wall_y: Vec<f64> = fluid.point_coordinates().iter().map(|p| p[1]).collect();
    InterfaceMap::new(rows_for(&wall_y), CW_NODES, fluid.face_points()).unwrap()
}

#[derive(Default)]
struct Run {
    q: Vec<Vec<f64>>,
    iterations: Vec<usize>,
    all_converged: bool,
    ended: String,
}

fn run_cw(fluid: &mut dyn Fluid, structure: &mut dyn Structure, map: &InterfaceMap, steps: usize) -> Run {
    let mut run = Run {
        all_converged: true,
        ended: "completed".into(),
        ..Default::default()
    };
    run.q.push(cw_q0());
    for n in 1..=steps {
        let step = match coupled_step(fluid, structure, map, CW_DT, &cw_settings()) {
            Ok(s) => s,
            Err(e) => {
                run.ended = format!("step {n}: {e}");
                break;
            }
        };
        run.iterations.push(step.iterations);
        run.all_converged &= step.converged;
        run.q.push(step.node_displacements.iter().map(|d| d[0]).collect());
    }
    run
}

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

fn differing(a: &[u64], b: &[u64]) -> usize {
    assert_eq!(a.len(), b.len());
    a.iter().zip(b).filter(|(x, y)| x != y).count()
}

fn flat3(v: &[Vec3]) -> Vec<f64> {
    v.iter().flatten().copied().collect()
}

/// Bits of the initial state: the mesh points, `u`, `p`, `phi`.
fn state_bits(f: &FlowWall) -> (Vec<u64>, Vec<u64>) {
    let mesh = bits(&flat3(f.flow.geom.mesh.points()));
    let mut fields = bits(&flat3(&f.flow.u.values));
    fields.extend(bits(&f.flow.p.values));
    fields.extend(bits(&f.flow.phi));
    (mesh, fields)
}

fn errors_of(v: &Value) -> Vec<rcfd_api::case::Diagnostic> {
    match fluid_from_case(&case_of(v)) {
        Ok(_) => Vec::new(),
        Err(e) => e,
    }
}

fn has_error_at(v: &Value, path: &str, needle: &str) -> bool {
    let e = errors_of(v);
    println!("  {path}: {e:?}");
    e.iter().any(|d| d.path == path && d.message.contains(needle))
}

// ---- K1, K2, NC2: the participant of the case --------------------------------------------------------------------------

/// K1 (c1): CWc parses and builds, its wall is CWs' wall, and the static and build errors are located.
#[test]
fn k1_the_case_names_the_wall_patch_and_errors_are_located() {
    assert!(!present(&Case::new()));
    assert!(Case::new().tree.to_json().get("cosim").is_none());
    let case = cwc();
    assert!(present(&case));
    let fluid = cwc_fluid();
    let hand = cw_fluid(&cw_channel());
    let (pc, hc) = (fluid.point_coordinates(), hand.point_coordinates());
    let differ = differing(&bits(&flat3(&pc)), &bits(&flat3(&hc)));
    println!(
        "K1: {} points, {} faces, {} point bits differ; faces equal: {}",
        pc.len(),
        fluid.face_points().len(),
        differ,
        fluid.face_points() == hand.face_points()
    );
    assert_eq!((pc.len(), fluid.face_points().len(), differ), (27, 16, 0));
    assert_eq!(fluid.face_points(), hand.face_points());
    assert_eq!(fluid.name(), "fluid");

    let with = |path: &[&str], value: Value| {
        let mut v = cwc_json(1e-13);
        let mut node = &mut v;
        for p in &path[..path.len() - 1] {
            node = node.as_object_mut().unwrap().entry(p.to_string()).or_insert(json!({}));
        }
        node[path[path.len() - 1]] = value;
        v
    };
    assert!(has_error_at(
        &with(&["cosim", "patch"], json!("")),
        "cosim.patch",
        "name the moving wall"
    ));
    assert!(has_error_at(
        &with(&["cosim", "patch"], json!("nope")),
        "cosim.patch",
        "no patch 'nope'"
    ));
    assert!(has_error_at(
        &with(&["cosim", "fixed_patches"], json!(["nope"])),
        "cosim.fixed_patches",
        "no patch 'nope'"
    ));
    assert!(has_error_at(
        &with(&["cosim", "fixed_patches"], json!(["xmin"])),
        "cosim.fixed_patches",
        "moving patch"
    ));
    assert!(has_error_at(
        &with(&["cosim", "initial_displacement"], json!(vec![0.0; 80])),
        "cosim.initial_displacement",
        "80 numbers for 27 wall points"
    ));
    assert!(has_error_at(
        &with(&["solver", "algorithm"], json!("simplec")),
        "solver.algorithm",
        "\"simplec\""
    ));
    assert!(has_error_at(
        &with(&["dynamic_mesh", "enabled"], json!(true)),
        "dynamic_mesh.enabled",
        "moves the mesh"
    ));
    assert!(has_error_at(
        &with(&["turbulence", "model"], json!("smagorinsky")),
        "cosim",
        "a turbulence model"
    ));
    let run = Simulation::new(&case)
        .err()
        .expect("Simulation::new refuses a [cosim] case");
    println!("  Simulation::new: {run:?}");
    assert!(run
        .iter()
        .any(|d| d.path == "cosim" && d.message.contains("rcfd serve --cosim")));
}

/// K2 (set-up, c1): before any step the case-built participant is the hand-built one bit for bit, and the flow settings
/// differ only in the momentum absolute tolerance.
#[test]
fn k2_the_case_built_participant_starts_as_the_hand_built_one() {
    let mut fluid = cwc_fluid();
    let hand = cw_fluid(&cw_channel());
    let (m, f) = state_bits(&fluid);
    let (hm, hf) = state_bits(&hand);
    let (dm, df) = (differing(&m, &hm), differing(&f, &hf));
    println!(
        "K2: mesh {dm} of {} bits differ, fields {df} of {} bits differ; momentum tol_abs {:e} (hand-built {:e})",
        m.len(),
        f.len(),
        fluid.flow.settings.momentum_solver.tol_abs,
        hand.flow.settings.momentum_solver.tol_abs
    );
    fluid.flow.settings.momentum_solver.tol_abs = 1e-16;
    let (a, b) = (
        format!("{:?}", fluid.flow.settings),
        format!("{:?}", hand.flow.settings),
    );
    if a != b {
        println!("  case-built settings: {a}\n  hand-built settings: {b}");
    }
    assert_eq!((dm, df), (0, 0));
    assert_eq!(a, b);
}

/// NC2 (control of K2): with `smoothing_tolerance = 1e-12` the initial mesh is not CWs' mesh.
#[test]
fn nc2_another_smoothing_tolerance_is_seen() {
    let fluid = fluid_from_case(&case_of(&cwc_json(1e-12))).unwrap();
    let hand = cw_fluid(&cw_channel());
    let dm = differing(&state_bits(&fluid).0, &state_bits(&hand).0);
    println!("NC2: {dm} mesh bits differ");
    assert!(dm > 0);
}

/// K3 (set-up, c3): 100 steps of CWc in process against 100 steps of the hand-built CWs.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn k3_the_case_runs_as_the_hand_built_model_problem() {
    let steps = 100;
    let started = Instant::now();
    let mut fluid = cwc_fluid();
    let map = map_of(&fluid);
    let case_run = run_cw(&mut fluid, &mut cw_structure(), &map, steps);
    let ch = cw_channel();
    let mut hand = cw_fluid(&ch);
    let hmap = map_of(&hand);
    let hand_run = run_cw(&mut hand, &mut cw_structure(), &hmap, steps);
    let mut worst: f64 = 0.0;
    let mut differ = 0;
    for (a, b) in case_run.q.iter().zip(&hand_run.q) {
        for (x, y) in a.iter().zip(b) {
            worst = worst.max((x - y).abs());
            differ += usize::from(x.to_bits() != y.to_bits());
        }
    }
    let counts = case_run
        .iterations
        .iter()
        .zip(&hand_run.iterations)
        .filter(|(a, b)| a != b)
        .count();
    println!(
        "K3: case '{}', hand '{}', {:.1} s; max |dq| = {worst:e} m = {:e} X; {differ} values differ in bits; {counts} steps with another count; converged {} {}",
        case_run.ended,
        hand_run.ended,
        started.elapsed().as_secs_f64(),
        worst / CW_X,
        case_run.all_converged,
        hand_run.all_converged
    );
    assert_eq!(case_run.iterations.len(), steps);
    assert_eq!(hand_run.iterations.len(), steps);
    assert!(worst <= 1e-6 * CW_X);
}

// ---- E1, E2: the elements of a structure -------------------------------------------------------------------------------

/// The structure of CWs with the five segments between its nodes as elements.
struct WithElements(SprungWall);

impl Participant for WithElements {
    fn name(&self) -> &str {
        self.0.name()
    }
    fn time(&self) -> f64 {
        self.0.time()
    }
    fn checkpoint(&mut self) -> Result<(), String> {
        self.0.checkpoint()
    }
    fn restore(&mut self) -> Result<(), String> {
        self.0.restore()
    }
}

impl Structure for WithElements {
    fn node_coordinates(&self) -> Vec<Vec3> {
        self.0.node_coordinates()
    }
    fn advance(&mut self, dt: f64, forces: &[Vec3]) -> Result<Vec<Vec3>, String> {
        self.0.advance(dt, forces)
    }
    fn elements(&self) -> Vec<Vec<usize>> {
        (0..CW_NODES - 1).map(|i| vec![i, i + 1]).collect()
    }
}

fn remote() -> RemoteSettings {
    RemoteSettings {
        connect_timeout: Duration::from_secs(5),
        call_timeout: Duration::from_secs(60),
    }
}

fn service() -> &'static (Arc<Service>, std::net::SocketAddr) {
    static SVC: OnceLock<(Arc<Service>, std::net::SocketAddr)> = OnceLock::new();
    SVC.get_or_init(|| {
        let svc = Service::new(ServiceConfig::default());
        svc.host_structure("with-elements", || Ok(Box::new(WithElements(cw_structure()))))
            .unwrap();
        svc.host_structure("plain", || Ok(Box::new(cw_structure()))).unwrap();
        let case = cwc();
        svc.host_fluid("fluid", move || {
            fluid_from_case(&case)
                .map(|f| Box::new(f) as Box<dyn Fluid>)
                .map_err(|e| format!("{e:?}"))
        })
        .unwrap();
        let server = svc.serve_tcp("127.0.0.1:0").unwrap();
        (svc, server.addr)
    })
}

/// E1 (c5): the elements cross in `describe`, and the map builders of ADR 0263 on the coupler's side give the map of the
/// in-process objects.
#[test]
fn e1_the_elements_cross_and_the_map_is_built_on_the_coupler_side() {
    let (_, addr) = service();
    let s = RemoteStructure::connect(*addr, "with-elements", remote()).unwrap();
    let f = RemoteFluid::connect(*addr, "fluid", remote()).unwrap();
    let (local_s, local_f) = (WithElements(cw_structure()), cwc_fluid());
    println!("E1: remote elements {:?}", s.elements());
    assert_eq!(s.elements(), local_s.elements());
    let settings = MapSettings::projection(0.06);
    let (remote_map, remote_report) = InterfaceMap::build(
        &s.node_coordinates(),
        &s.elements(),
        &f.point_coordinates(),
        f.face_points(),
        &settings,
    )
    .unwrap();
    let (local_map, _) = InterfaceMap::build(
        &local_s.node_coordinates(),
        &local_s.elements(),
        &local_f.point_coordinates(),
        local_f.face_points(),
        &settings,
    )
    .unwrap();
    let q0 = along_x(&cw_q0());
    let a = flat3(&remote_map.displacement_to_points(&q0).unwrap());
    let b = flat3(&local_map.displacement_to_points(&q0).unwrap());
    let forces = local_f.forces().unwrap();
    let remote_forces = f.forces().unwrap();
    let c = flat3(&remote_map.forces_to_nodes(&remote_forces).unwrap());
    let e = flat3(&local_map.forces_to_nodes(&forces).unwrap());
    let (d1, d2) = (differing(&bits(&a), &bits(&b)), differing(&bits(&c), &bits(&e)));
    println!(
        "E1: displacement_to_points {d1} of {} bits differ, forces_to_nodes {d2} of {} bits differ; largest distance {:e}, precision defect {:e}; maps equal: {}",
        a.len(),
        c.len(),
        remote_report.distances.iter().fold(0.0f64, |m, x| m.max(*x)),
        remote_report.precision_defect,
        remote_map == local_map
    );
    assert_eq!((d1, d2), (0, 0));
}

/// E2 (c5): a structure without elements answers `describe` as before (no member `elements`).
#[test]
fn e2_a_structure_without_elements_answers_as_before() {
    let (_, addr) = service();
    let mut c = Client::connect(*addr).unwrap();
    let open = c.call("cosim.open", json!({"participant": "plain"})).unwrap();
    let d = c
        .call(
            "cosim.describe",
            json!({"participant": "plain", "lease": open["lease"]}),
        )
        .unwrap();
    let keys: Vec<&String> = d.as_object().unwrap().keys().collect();
    println!("E2: describe members {keys:?}");
    assert!(d.get("elements").is_none());
    let s = RemoteStructure::connect(*addr, "plain", remote()).unwrap();
    assert!(s.elements().is_empty());
}

// ---- C2: the time of a request of service::Client ----------------------------------------------------------------------

/// C2 (c6): 2000 calls of `cosim.list` through `service::Client` on the loopback interface, mean time per call.
#[test]
#[cfg_attr(debug_assertions, ignore = "timing: run in release")]
fn c2_a_client_call_over_the_loopback_interface_takes_less_than_a_millisecond() {
    let (_, addr) = service();
    let mut c = Client::connect(*addr).unwrap();
    c.call("cosim.list", json!({})).unwrap();
    let n = 2000;
    let started = Instant::now();
    for _ in 0..n {
        c.call("cosim.list", json!({})).unwrap();
    }
    let per_call = started.elapsed().as_secs_f64() / n as f64;
    println!("C2: {:.4} ms per call ({n} calls)", per_call * 1e3);
    assert!(per_call <= 1e-3);
}
