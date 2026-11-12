//! F-360 (ADR 0345): the co-simulation calls of `rcfd_fv::cosim` over the JSON-RPC service (`rcfd_api::cosim_service`,
//! protocol `rcfd-cosim/1`, service protocol 1.1). A mock structural participant in another process, driven over TCP,
//! against the in-process run of the model problem of ADR 0256 (set-up CWs).
//!
//! The limits, set-ups and mutations were registered in ADR 0345 before this file existed. The set-up CWs, the crossings
//! and the bit comparisons are those of `crates/rcfd-fv/tests/cosim_f139.rs` (copied, that file is not edited).
//!
//! The out-of-process participant is this test binary started a second time with `RCFD_F360_ROLE` set
//! (`child_participant_process`). Every socket is bound to 127.0.0.1 port 0 and every read has a time-out.

use rcfd_api::cosim_service::{RemoteFluid, RemoteSettings, RemoteStructure, COSIM_PROTOCOL};
use rcfd_api::service::{code, Client, Service, ServiceConfig, METHODS};
use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::bc::{FixedValue, Symmetry, ZeroGradient};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::cosim::{
    coupled_step, piecewise_linear_rows, AtCap, CoupledStep, CouplingSettings, FlowWall, Fluid, InterfaceMap,
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
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex, OnceLock};
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

/// A channel along x whose wall `xmin` moves, with the smoothing of the interior (`cosim_f139.rs`, CWs: slip sides).
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

// ---- set-up CWs (ADR 0256 P3, G1, amendment 1) -----------------------------------------------------------------------

const CW_L: f64 = 1.0;
const CW_W: f64 = 0.4;
const CW_LZ: f64 = 0.1;
const CW_MU: f64 = 4.0;
const CW_X: f64 = 0.002;
const CW_NU: f64 = 1e-4;
const CW_NODES: usize = 6;
const CW_DT: f64 = 0.01;
const CW_STEPS: usize = 650;

fn cw_m() -> f64 {
    RHO * CW_L / CW_MU
}

fn cw_k() -> f64 {
    TAU * TAU * (cw_m() + RHO * CW_L)
}

fn cw_omega(j: usize) -> f64 {
    let l = if j == 0 {
        CW_L
    } else {
        let kappa = j as f64 * PI / CW_W;
        (kappa * CW_L).tanh() / kappa
    };
    (cw_k() / (cw_m() + RHO * l)).sqrt()
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

/// The structure of CWs with every spring multiplied by `spring_factor` (1: the registered structure).
fn cw_structure_with(spring_factor: f64) -> SprungWall {
    let t = cw_tributary();
    SprungWall::new(
        cw_node_y().iter().map(|y| [0.0, *y, 0.5 * CW_LZ]).collect(),
        [1.0, 0.0, 0.0],
        t.iter().map(|t| cw_m() * CW_LZ * t).collect(),
        t.iter().map(|t| cw_k() * CW_LZ * t * spring_factor).collect(),
        cw_q0(),
    )
    .unwrap()
}

fn cw_structure() -> SprungWall {
    cw_structure_with(1.0)
}

fn cw_rows(ch: &Channel) -> Vec<Vec<(usize, f64)>> {
    piecewise_linear_rows(&cw_node_y(), &ch.wall_y()).unwrap()
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

/// The fluid participant of CWs at rest on the mesh of the initial wall.
fn cw_fluid(ch: &Arc<Channel>, mesh_motion: MeshMotion) -> FlowWall {
    let rows = cw_rows(ch);
    let flow = ch.flow(&map_points(&rows, &along_x(&cw_q0())));
    FlowWall::new(flow, WALL, &ch.mesh0, RHO, CW_NU, mesh_motion).unwrap()
}

fn cw_map(ch: &Channel, fluid: &dyn Fluid) -> InterfaceMap {
    InterfaceMap::new(cw_rows(ch), CW_NODES, fluid.face_points()).unwrap()
}

/// The registered settings (ADR 0256 C3).
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

fn modal(q: &[f64]) -> (f64, f64) {
    let (t, y) = (cw_tributary(), cw_node_y());
    let c: Vec<f64> = y.iter().map(|y| (PI * y / CW_W).cos()).collect();
    let q0 = q.iter().zip(&t).map(|(q, t)| q * t).sum::<f64>() / t.iter().sum::<f64>();
    let q1 = q.iter().zip(&t).zip(&c).map(|((q, t), c)| q * t * c).sum::<f64>()
        / t.iter().zip(&c).map(|(t, c)| t * c * c).sum::<f64>();
    (q0, q1)
}

fn upward_crossings(t: &[f64], x: &[f64]) -> Vec<f64> {
    (1..x.len())
        .filter(|&i| x[i - 1] < 0.0 && x[i] >= 0.0)
        .map(|i| t[i - 1] + (t[i] - t[i - 1]) * (-x[i - 1]) / (x[i] - x[i - 1]))
        .collect()
}

fn measured_omega(t: &[f64], x: &[f64]) -> (f64, usize) {
    let c = upward_crossings(t, x);
    assert!(c.len() >= 2, "fewer than two upward crossings");
    (TAU * (c.len() - 1) as f64 / (c[c.len() - 1] - c[0]), c.len())
}

/// Record of a run of CWs: the nodal displacement (along x) after every step, entry 0 the initial one.
#[derive(Default)]
struct Run {
    t: Vec<f64>,
    q: Vec<Vec<f64>>,
    iterations: Vec<usize>,
    last: Vec<f64>,
    all_converged: bool,
    ended: String,
}

impl Run {
    fn max_iterations(&self) -> usize {
        self.iterations.iter().copied().max().unwrap_or(0)
    }
    fn mean_iterations(&self) -> f64 {
        self.iterations.iter().sum::<usize>() as f64 / self.iterations.len() as f64
    }
    fn histogram(&self) -> Vec<usize> {
        let mut h = vec![0; self.max_iterations() + 1];
        for &i in &self.iterations {
            h[i] += 1;
        }
        h
    }
    fn largest_last(&self) -> f64 {
        self.last.iter().fold(0.0, |a: f64, b| a.max(*b))
    }
    /// `omega_m/omega_j` and the upward crossings of `Q_0` and `Q_1`.
    fn frequencies(&self) -> ((f64, usize), (f64, usize)) {
        let (q0, q1): (Vec<f64>, Vec<f64>) = self.q.iter().map(|q| modal(q)).unzip();
        let (w0, n0) = measured_omega(&self.t, &q0);
        let (w1, n1) = measured_omega(&self.t, &q1);
        ((w0 / cw_omega(0), n0), (w1 / cw_omega(1), n1))
    }
}

fn run_cw(fluid: &mut dyn Fluid, structure: &mut dyn Structure, map: &InterfaceMap, steps: usize) -> Run {
    let mut run = Run {
        all_converged: true,
        ended: "completed".into(),
        ..Default::default()
    };
    run.t.push(0.0);
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
        run.last.push(*step.residuals.last().unwrap());
        run.t.push(structure.time());
        run.q.push(step.node_displacements.iter().map(|d| d[0]).collect());
    }
    run
}

fn in_process_run(steps: usize) -> Run {
    let ch = cw_channel();
    let mut fluid = cw_fluid(&ch, motion(&ch));
    let mut structure = cw_structure();
    let map = cw_map(&ch, &fluid);
    let started = Instant::now();
    let run = run_cw(&mut fluid, &mut structure, &map, steps);
    println!(
        "reference: ended '{}', {} steps in {:.1} s, iterations max {} mean {:.3}, histogram {:?}",
        run.ended,
        run.iterations.len(),
        started.elapsed().as_secs_f64(),
        run.max_iterations(),
        run.mean_iterations(),
        run.histogram()
    );
    run
}

/// The in-process run of CWs in this build, 650 steps: the reference of D (heavy records only).
fn reference_long() -> &'static Run {
    static RUN: OnceLock<Run> = OnceLock::new();
    RUN.get_or_init(|| in_process_run(CW_STEPS))
}

/// The first 100 steps of the in-process run of CWs (ADR 0345 amendment 2): the reference of the fast tests.
fn reference() -> &'static Run {
    static RUN: OnceLock<Run> = OnceLock::new();
    RUN.get_or_init(|| in_process_run(100))
}

/// `max abs(q - q_reference)` over the steps of `run` and the nodes, the steps whose iteration count differs, and the
/// last test quantities whose bits differ.
fn against_reference(run: &Run, r: &Run) -> (f64, usize, usize) {
    let n = run.iterations.len();
    assert!(n <= r.iterations.len());
    let mut worst: f64 = 0.0;
    for (a, b) in run.q.iter().zip(&r.q) {
        for (x, y) in a.iter().zip(b) {
            worst = worst.max((x - y).abs());
        }
    }
    let counts = (0..n).filter(|&i| run.iterations[i] != r.iterations[i]).count();
    let last = (0..n).filter(|&i| run.last[i].to_bits() != r.last[i].to_bits()).count();
    (worst, counts, last)
}

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

/// Every value of the fluid participant's state, as bits (`cosim_f139.rs`).
fn fluid_bits(f: &FlowWall) -> Vec<u64> {
    let mut out = Vec::new();
    out.extend(f.flow.u.values.iter().flatten().map(|x| x.to_bits()));
    out.extend(bits(&f.flow.p.values));
    out.extend(bits(&f.flow.phi));
    out.push(f.flow.time.to_bits());
    out.extend(f.flow.geom.mesh.points().iter().flatten().map(|x| x.to_bits()));
    out.extend(
        f.flow
            .u
            .boundary_values(&f.flow.geom)
            .iter()
            .flatten()
            .map(|x| x.to_bits()),
    );
    out.extend(bits(&f.wall_dpdn));
    out.push(f.flow.history.len() as u64);
    out
}

fn structure_bits(s: &SprungWall) -> Vec<u64> {
    let mut out = bits(&s.q);
    out.extend(bits(&s.v));
    out.push(s.t.to_bits());
    out
}

fn differing(a: &[u64], b: &[u64]) -> usize {
    assert_eq!(a.len(), b.len());
    a.iter().zip(b).filter(|(x, y)| x != y).count()
}

/// Bits of the node displacements of `step` that differ from the reference's step `n` (1-based), and whether the
/// iteration count is the reference's.
fn step_against_reference(step: &CoupledStep, n: usize) -> (usize, bool) {
    let r = reference();
    let q: Vec<f64> = step.node_displacements.iter().map(|d| d[0]).collect();
    (
        differing(&bits(&q), &bits(&r.q[n])),
        step.iterations == r.iterations[n - 1],
    )
}

fn remote_settings() -> RemoteSettings {
    RemoteSettings {
        connect_timeout: Duration::from_secs(5),
        call_timeout: Duration::from_secs(60),
    }
}

// ---- participants of the tests ---------------------------------------------------------------------------------------

/// The faulty structure of ADR 0256 E1 (a): an error at its `fail_at`-th `advance`.
struct FaultyStructure {
    inner: SprungWall,
    calls: usize,
    fail_at: usize,
    not_a_number: bool,
}

impl Participant for FaultyStructure {
    fn name(&self) -> &str {
        self.inner.name()
    }
    fn time(&self) -> f64 {
        self.inner.time()
    }
    fn checkpoint(&mut self) -> Result<(), String> {
        self.inner.checkpoint()
    }
    fn restore(&mut self) -> Result<(), String> {
        self.inner.restore()
    }
}

impl Structure for FaultyStructure {
    fn node_coordinates(&self) -> Vec<Vec3> {
        self.inner.node_coordinates()
    }
    fn advance(&mut self, dt: f64, forces: &[Vec3]) -> Result<Vec<Vec3>, String> {
        self.calls += 1;
        let hit = self.calls == self.fail_at;
        if hit && !self.not_a_number {
            return Err("injected failure of the structural solver".into());
        }
        let mut q = self.inner.advance(dt, forces)?;
        if hit {
            q[2][0] = f64::NAN;
        }
        Ok(q)
    }
}

/// A structure whose state the test can read while a service hosts it.
struct SharedStructure {
    inner: Arc<Mutex<SprungWall>>,
    name: String,
}

impl SharedStructure {
    fn new(inner: &Arc<Mutex<SprungWall>>) -> Self {
        SharedStructure {
            inner: inner.clone(),
            name: inner.lock().unwrap().name().to_string(),
        }
    }
}

impl Participant for SharedStructure {
    fn name(&self) -> &str {
        &self.name
    }
    fn time(&self) -> f64 {
        self.inner.lock().unwrap().time()
    }
    fn checkpoint(&mut self) -> Result<(), String> {
        self.inner.lock().unwrap().checkpoint()
    }
    fn restore(&mut self) -> Result<(), String> {
        self.inner.lock().unwrap().restore()
    }
}

impl Structure for SharedStructure {
    fn node_coordinates(&self) -> Vec<Vec3> {
        self.inner.lock().unwrap().node_coordinates()
    }
    fn advance(&mut self, dt: f64, forces: &[Vec3]) -> Result<Vec<Vec3>, String> {
        self.inner.lock().unwrap().advance(dt, forces)
    }
}

/// A fluid (not `Send`) that writes the bits of its state to a shared vector after every call that can change it.
struct RecordingFluid {
    inner: FlowWall,
    state: Arc<Mutex<Vec<u64>>>,
}

impl RecordingFluid {
    fn new(inner: FlowWall, state: &Arc<Mutex<Vec<u64>>>) -> Self {
        let f = RecordingFluid {
            inner,
            state: state.clone(),
        };
        f.record();
        f
    }
    fn record(&self) {
        *self.state.lock().unwrap() = fluid_bits(&self.inner);
    }
}

impl Participant for RecordingFluid {
    fn name(&self) -> &str {
        self.inner.name()
    }
    fn time(&self) -> f64 {
        self.inner.time()
    }
    fn checkpoint(&mut self) -> Result<(), String> {
        let r = self.inner.checkpoint();
        self.record();
        r
    }
    fn restore(&mut self) -> Result<(), String> {
        let r = self.inner.restore();
        self.record();
        r
    }
}

impl Fluid for RecordingFluid {
    fn point_coordinates(&self) -> Vec<Vec3> {
        self.inner.point_coordinates()
    }
    fn face_points(&self) -> Vec<Vec<usize>> {
        self.inner.face_points()
    }
    fn forces(&self) -> Result<Vec<Vec3>, String> {
        self.inner.forces()
    }
    fn advance(&mut self, dt: f64, displacements: &[Vec3]) -> Result<Vec<Vec3>, String> {
        let r = self.inner.advance(dt, displacements);
        self.record();
        r
    }
}

/// A service on 127.0.0.1 port 0 that hosts the fluid of CWs as `fluid`; `fail_motion_at` makes the mesh function fail
/// at that call (0: never). Returns the service, its address and the recorded state.
fn fluid_service(fail_motion_at: usize) -> (Arc<Service>, SocketAddr, Arc<Mutex<Vec<u64>>>) {
    let svc = Service::new(ServiceConfig::default());
    let state = Arc::new(Mutex::new(Vec::new()));
    let shared = state.clone();
    svc.host_fluid("fluid", move || {
        let ch = cw_channel();
        let inner = ch.clone();
        let mut calls = 0;
        let mesh_motion: MeshMotion = Box::new(move |d| {
            calls += 1;
            if calls == fail_motion_at {
                Err("injected failure of the mesh motion".into())
            } else {
                inner.points(d)
            }
        });
        Ok(Box::new(RecordingFluid::new(cw_fluid(&ch, mesh_motion), &shared)))
    })
    .unwrap();
    let server = svc.serve_tcp("127.0.0.1:0").unwrap();
    (svc, server.addr, state)
}

/// A service that hosts the structure of CWs as `structure`, readable through the returned handle.
fn structure_service() -> (Arc<Service>, SocketAddr, Arc<Mutex<SprungWall>>) {
    let svc = Service::new(ServiceConfig::default());
    let wall = Arc::new(Mutex::new(cw_structure()));
    let shared = SharedStructure::new(&wall);
    svc.host_structure("structure", move || Ok(Box::new(shared))).unwrap();
    let server = svc.serve_tcp("127.0.0.1:0").unwrap();
    (svc, server.addr, wall)
}

// ---- the out-of-process participant ----------------------------------------------------------------------------------

const ROLE: &str = "RCFD_F360_ROLE";

/// The participant process: this test binary started with `RCFD_F360_ROLE`. Without the variable it does nothing.
#[test]
#[ignore = "helper: the participant process that the other tests start"]
fn child_participant_process() {
    let Ok(role) = std::env::var(ROLE) else {
        return;
    };
    let svc = Service::new(ServiceConfig::default());
    svc.host_structure("structure", move || {
        Ok(match role.as_str() {
            "structure" => Box::new(cw_structure()) as Box<dyn Structure>,
            "structure-stiff" => Box::new(cw_structure_with(1.0 + 1e-9)),
            "structure-fails-2" => Box::new(FaultyStructure {
                inner: cw_structure(),
                calls: 0,
                fail_at: 2,
                not_a_number: false,
            }),
            other => return Err(format!("unknown role '{other}'")),
        })
    })
    .unwrap();
    let server = svc.serve_tcp("127.0.0.1:0").unwrap();
    println!("\nF360_CHILD {} {}", server.addr, std::process::id());
    std::io::stdout().flush().unwrap();
    // ends when the parent closes the standard input (or dies), or after 600 s
    let (done, wait) = channel();
    std::thread::spawn(move || {
        let mut sink = Vec::new();
        let _ = std::io::stdin().read_to_end(&mut sink);
        let _ = done.send(());
    });
    let _ = wait.recv_timeout(Duration::from_secs(600));
    std::process::exit(0);
}

/// A participant process; killed when dropped.
struct ChildProcess {
    child: std::process::Child,
    address: SocketAddr,
    pid: u32,
}

impl ChildProcess {
    fn start(role: &str) -> ChildProcess {
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["child_participant_process", "--exact", "--ignored", "--nocapture"])
            .env(ROLE, role)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let out = child.stdout.take().unwrap();
        let (found, wait) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(out).lines().map_while(Result::ok) {
                if let Some(rest) = line.strip_prefix("F360_CHILD ") {
                    let _ = found.send(rest.to_string());
                }
            }
        });
        let line = match wait.recv_timeout(Duration::from_secs(60)) {
            Ok(line) => line,
            Err(_) => {
                let _ = child.kill();
                panic!("the participant process did not report its address in 60 s");
            }
        };
        let mut parts = line.split_whitespace();
        let address = parts.next().unwrap().parse().unwrap();
        let pid = parts.next().unwrap().parse().unwrap();
        ChildProcess { child, address, pid }
    }

    fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for ChildProcess {
    fn drop(&mut self) {
        self.kill();
    }
}

// ---- the relay -------------------------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum Cut {
    /// Close both sockets instead of passing the request on.
    Before,
    /// Pass the request on, wait for the service's answer, discard it and close both sockets.
    After,
}

struct Relay {
    address: SocketAddr,
    accepted: Arc<AtomicUsize>,
}

/// Passes lines between a proxy and the service at `upstream`; cuts the connection at the `nth` request of `method`
/// (once). One connection at a time.
fn relay(upstream: SocketAddr, method: &'static str, nth: usize, cut: Cut) -> Relay {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let accepted = Arc::new(AtomicUsize::new(0));
    let count = accepted.clone();
    let needle = format!("\"method\":\"{method}\"");
    std::thread::spawn(move || {
        let wait = Some(Duration::from_secs(60));
        let mut seen = 0;
        for client in listener.incoming() {
            let Ok(client) = client else { break };
            count.fetch_add(1, Ordering::SeqCst);
            let Ok(server) = TcpStream::connect(upstream) else {
                break;
            };
            for s in [&client, &server] {
                s.set_read_timeout(wait).unwrap();
                s.set_write_timeout(wait).unwrap();
            }
            let mut from_client = BufReader::new(client.try_clone().unwrap());
            let mut from_server = BufReader::new(server.try_clone().unwrap());
            let (mut to_client, mut to_server) = (&client, &server);
            loop {
                let mut request = String::new();
                if from_client.read_line(&mut request).unwrap_or(0) == 0 {
                    break;
                }
                let mut hit = false;
                if request.contains(&needle) {
                    seen += 1;
                    hit = seen == nth;
                }
                if hit && cut == Cut::Before {
                    break;
                }
                if to_server.write_all(request.as_bytes()).is_err() {
                    break;
                }
                let mut reply = String::new();
                if from_server.read_line(&mut reply).unwrap_or(0) == 0 {
                    break;
                }
                if hit {
                    break;
                }
                if to_client.write_all(reply.as_bytes()).is_err() {
                    break;
                }
            }
            let _ = client.shutdown(Shutdown::Both);
            let _ = server.shutdown(Shutdown::Both);
        }
    });
    Relay { address, accepted }
}

// ---- S0, P1 to P3 ----------------------------------------------------------------------------------------------------

#[test]
#[ignore = "heavy record (ADR 0345 amendment 2): the 650-step reference; run in runs 1 and 2"]
fn s0_the_reference_is_the_run_of_adr_0256() {
    let run = reference_long();
    let ((r0, n0), (r1, n1)) = run.frequencies();
    println!(
        "S0: omega_m/omega Q0 {r0:.9} ({n0} crossings), Q1 {r1:.9} ({n1} crossings); iterations max {}, histogram {:?}",
        run.max_iterations(),
        run.histogram()
    );
    assert_eq!(run.ended, "completed");
    assert!((r0 - 0.998618).abs() <= 2e-6);
    assert!((r1 - 1.011453).abs() <= 2e-6);
    assert_eq!(run.max_iterations(), 10);
    assert_eq!(&run.histogram()[7..], &[13, 210, 343, 84]);
}

const COSIM_METHODS: [&str; 8] = [
    "cosim.list",
    "cosim.open",
    "cosim.describe",
    "cosim.time",
    "cosim.checkpoint",
    "cosim.restore",
    "cosim.forces",
    "cosim.advance",
];

#[test]
fn p1_the_service_reports_version_1_1_and_the_cosim_methods() {
    let svc = Service::new(ServiceConfig::default());
    let info = svc.call("service.info", &json!({})).unwrap();
    println!(
        "P1: protocol {}, protocol_version {}, cosim {}, {} methods",
        info["protocol"],
        info["protocol_version"],
        info["cosim"],
        METHODS.len()
    );
    assert_eq!(info["protocol"], "rcfd-service/1");
    assert_eq!(info["protocol_version"], "1.1");
    assert_eq!(info["cosim"], "rcfd-cosim/1");
    assert_eq!(COSIM_PROTOCOL, "rcfd-cosim/1");
    assert_eq!(METHODS.len(), 69);
    assert_eq!(METHODS[60], "design.ledger_rerun_check");
    assert_eq!(METHODS[0], "service.info");
    assert_eq!(&METHODS[61..], &COSIM_METHODS);
    assert_eq!(info["methods"].as_array().unwrap().len(), 69);
    // nothing hosted: an empty list
    let list = svc.call("cosim.list", &json!({})).unwrap();
    assert_eq!(list["participants"].as_array().unwrap().len(), 0);
    svc.host_structure("structure", || Ok(Box::new(cw_structure())))
        .unwrap();
    svc.host_fluid("fluid", || {
        let ch = cw_channel();
        Ok(Box::new(cw_fluid(&ch, motion(&ch))))
    })
    .unwrap();
    assert!(svc
        .host_structure("structure", || Ok(Box::new(cw_structure())))
        .is_err());
    let list = svc.call("cosim.list", &json!({})).unwrap();
    println!("P1: cosim.list {list}");
    assert_eq!(list["protocol"], "rcfd-cosim/1");
    assert_eq!(
        list["participants"],
        json!([
            {"participant": "fluid", "kind": "fluid", "name": "fluid"},
            {"participant": "structure", "kind": "structure", "name": "structure"},
        ])
    );
    assert!(svc.unhost("fluid"));
    assert!(!svc.unhost("fluid"));
    assert_eq!(
        svc.call("cosim.open", &json!({"participant": "fluid"}))
            .unwrap_err()
            .code,
        code::NO_PARTICIPANT
    );
}

fn flat(v: &Value) -> Vec<f64> {
    v.as_array()
        .unwrap()
        .iter()
        .flat_map(|row| {
            let row = row.as_array().unwrap();
            assert_eq!(row.len(), 3);
            row.iter().map(|x| x.as_f64().unwrap())
        })
        .collect()
}

fn flat3(v: &[Vec3]) -> Vec<f64> {
    v.iter().flatten().copied().collect()
}

fn as_json(v: &[Vec3]) -> Value {
    Value::Array(v.iter().map(|a| json!([a[0], a[1], a[2]])).collect())
}

#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn p2_the_six_calls_return_what_the_participant_returns_bit_for_bit() {
    let ch = cw_channel();
    // in-process twins
    let mut fluid = cw_fluid(&ch, motion(&ch));
    let mut structure = cw_structure();
    let map = cw_map(&ch, &fluid);
    // the same two, hosted
    let (svc, address, _) = fluid_service(0);
    svc.host_structure("structure", || Ok(Box::new(cw_structure())))
        .unwrap();
    let mut c = Client::connect(address).unwrap();
    let mut differ = 0;
    let mut compared = 0;
    let mut same = |a: &[f64], b: &[f64]| {
        compared += a.len();
        differ += differing(&bits(a), &bits(b));
    };

    let s = c.call("cosim.open", json!({"participant": "structure"})).unwrap();
    let f = c.call("cosim.open", json!({"participant": "fluid"})).unwrap();
    assert_eq!((&s["kind"], &f["kind"]), (&json!("structure"), &json!("fluid")));
    assert_eq!(
        (&s["protocol"], &f["protocol"]),
        (&json!("rcfd-cosim/1"), &json!("rcfd-cosim/1"))
    );
    let sp = |extra: Value| {
        let mut p = extra;
        p["participant"] = json!("structure");
        p["lease"] = s["lease"].clone();
        p
    };
    let fp = |extra: Value| {
        let mut p = extra;
        p["participant"] = json!("fluid");
        p["lease"] = f["lease"].clone();
        p
    };

    // describe
    let d = c.call("cosim.describe", sp(json!({}))).unwrap();
    same(&flat(&d["nodes"]), &flat3(&structure.node_coordinates()));
    assert_eq!(d["name"], "structure");
    let d = c.call("cosim.describe", fp(json!({}))).unwrap();
    same(&flat(&d["points"]), &flat3(&fluid.point_coordinates()));
    assert_eq!(d["faces"], json!(fluid.face_points()));
    assert_eq!(d["name"], "fluid");
    // time, checkpoint
    for (p, t) in [(sp(json!({})), structure.time()), (fp(json!({})), fluid.time())] {
        same(
            &[c.call("cosim.time", p.clone()).unwrap()["time"].as_f64().unwrap()],
            &[t],
        );
        same(
            &[c.call("cosim.checkpoint", p).unwrap()["time"].as_f64().unwrap()],
            &[t],
        );
    }
    structure.checkpoint().unwrap();
    fluid.checkpoint().unwrap();
    // forces
    let start = fluid.forces().unwrap();
    let r = c.call("cosim.forces", fp(json!({}))).unwrap();
    same(&flat(&r["forces"]), &flat3(&start));
    // advance: the structure under the predictor's forces, the fluid with the displacement that follows
    let nodal = map.forces_to_nodes(&start).unwrap();
    let q = structure.advance(CW_DT, &nodal).unwrap();
    let r = c
        .call("cosim.advance", sp(json!({"dt": CW_DT, "forces": as_json(&nodal)})))
        .unwrap();
    let first_q = flat(&r["displacements"]);
    same(&first_q, &flat3(&q));
    same(&[r["time"].as_f64().unwrap()], &[structure.time()]);
    let points = map.displacement_to_points(&q).unwrap();
    let forces = fluid.advance(CW_DT, &points).unwrap();
    let r = c
        .call(
            "cosim.advance",
            fp(json!({"dt": CW_DT, "displacements": as_json(&points)})),
        )
        .unwrap();
    let first_f = flat(&r["forces"]);
    same(&first_f, &flat3(&forces));
    same(&[r["time"].as_f64().unwrap()], &[fluid.time()]);
    for (p, t) in [(sp(json!({})), structure.time()), (fp(json!({})), fluid.time())] {
        same(&[c.call("cosim.time", p).unwrap()["time"].as_f64().unwrap()], &[t]);
    }
    assert_eq!(structure.time(), CW_DT);
    // restore, time, advance again
    structure.restore().unwrap();
    fluid.restore().unwrap();
    for (p, t) in [(sp(json!({})), structure.time()), (fp(json!({})), fluid.time())] {
        same(
            &[c.call("cosim.restore", p.clone()).unwrap()["time"].as_f64().unwrap()],
            &[t],
        );
        same(&[c.call("cosim.time", p).unwrap()["time"].as_f64().unwrap()], &[t]);
    }
    assert_eq!(structure.time(), 0.0);
    let r = c
        .call("cosim.advance", sp(json!({"dt": CW_DT, "forces": as_json(&nodal)})))
        .unwrap();
    same(&flat(&r["displacements"]), &first_q);
    let r = c
        .call(
            "cosim.advance",
            fp(json!({"dt": CW_DT, "displacements": as_json(&points)})),
        )
        .unwrap();
    same(&flat(&r["forces"]), &first_f);
    println!("P2: {differ} of {compared} values differ in their bits");
    assert_eq!(compared, 291);
    assert_eq!(differ, 0);
}

#[test]
fn p3_refusals_have_their_codes_and_a_superseded_lease_changes_nothing() {
    let (svc, address, wall) = structure_service();
    svc.host_structure("nan", || {
        Ok(Box::new(FaultyStructure {
            inner: cw_structure(),
            calls: 0,
            fail_at: 1,
            not_a_number: true,
        }))
    })
    .unwrap();
    let mut c = Client::connect(address).unwrap();
    let forces = as_json(&[[1e-3, 0.0, 0.0]; CW_NODES]);
    let report = std::cell::RefCell::new(Vec::new());
    let refused = |c: &mut Client, label: &str, method: &str, params: Value| {
        let e = c.call(method, params).unwrap_err();
        report.borrow_mut().push(format!("{label}: {} '{}'", e.code, e.message));
        e
    };

    assert_eq!(
        refused(&mut c, "unknown method", "cosim.nope", json!({})).code,
        code::METHOD_NOT_FOUND
    );
    assert_eq!(
        refused(
            &mut c,
            "unknown participant",
            "cosim.open",
            json!({"participant": "nobody"})
        )
        .code,
        code::NO_PARTICIPANT
    );
    let first = c.call("cosim.open", json!({"participant": "structure"})).unwrap()["lease"]
        .as_u64()
        .unwrap();
    assert_eq!(
        refused(&mut c, "no lease", "cosim.time", json!({"participant": "structure"})).code,
        code::INVALID_PARAMS
    );
    // restore before any checkpoint: the participant's own message
    let e = refused(
        &mut c,
        "restore without a checkpoint",
        "cosim.restore",
        json!({"participant": "structure", "lease": first}),
    );
    assert_eq!(e.code, code::PARTICIPANT);
    assert_eq!(e.message, "sprung wall: restore without a checkpoint");
    // a second open supersedes the first lease
    let second = c.call("cosim.open", json!({"participant": "structure"})).unwrap()["lease"]
        .as_u64()
        .unwrap();
    assert_eq!(second, first + 1);
    let before = structure_bits(&wall.lock().unwrap());
    let e = refused(
        &mut c,
        "superseded lease",
        "cosim.advance",
        json!({"participant": "structure", "lease": first, "dt": CW_DT, "forces": forces}),
    );
    assert_eq!(e.code, code::BAD_STATE);
    let stale = differing(&before, &structure_bits(&wall.lock().unwrap()));
    report.borrow_mut().push(format!(
        "after the superseded lease's advance {stale} bits of the structure differ"
    ));
    assert_eq!(stale, 0);
    let p = |extra: Value| {
        let mut p = extra;
        p["participant"] = json!("structure");
        p["lease"] = json!(second);
        p
    };
    assert_eq!(
        refused(&mut c, "forces to a structure", "cosim.forces", p(json!({}))).code,
        code::BAD_STATE
    );
    let short = as_json(&[[1e-3, 0.0, 0.0]; CW_NODES - 1]);
    let e = refused(
        &mut c,
        "one force too few",
        "cosim.advance",
        p(json!({"dt": CW_DT, "forces": short})),
    );
    assert_eq!(e.code, code::PARTICIPANT);
    assert_eq!(e.message, "sprung wall: 5 forces for 6 nodes");
    assert_eq!(
        refused(&mut c, "no dt", "cosim.advance", p(json!({"forces": forces}))).code,
        code::INVALID_PARAMS
    );
    assert_eq!(
        refused(
            &mut c,
            "a vector of two numbers",
            "cosim.advance",
            p(json!({"dt": CW_DT, "forces": [[1.0, 2.0]]}))
        )
        .code,
        code::INVALID_PARAMS
    );
    // none of the refusals moved the structure
    assert_eq!(differing(&before, &structure_bits(&wall.lock().unwrap())), 0);
    // a participant that returns a NaN
    let lease = c.call("cosim.open", json!({"participant": "nan"})).unwrap()["lease"].clone();
    let e = refused(
        &mut c,
        "a NaN",
        "cosim.advance",
        json!({"participant": "nan", "lease": lease, "dt": CW_DT, "forces": forces}),
    );
    assert_eq!(e.code, code::PARTICIPANT);
    assert!(e.message.contains("non-finite"));
    for line in report.borrow().iter() {
        println!("P3: {line}");
    }
}

// ---- A1r, A2r, D: the structure in another process -------------------------------------------------------------------

/// CWs with the structure out of process (role `structure`), the fluid in-process. Returns the run and the child's
/// process id.
fn remote_run() -> &'static (Run, u32) {
    static RUN: OnceLock<(Run, u32)> = OnceLock::new();
    RUN.get_or_init(|| {
        let child = ChildProcess::start("structure");
        let ch = cw_channel();
        let mut fluid = cw_fluid(&ch, motion(&ch));
        let mut structure = RemoteStructure::connect(child.address, "structure", remote_settings()).unwrap();
        let map = cw_map(&ch, &fluid);
        let started = Instant::now();
        let run = run_cw(&mut fluid, &mut structure, &map, CW_STEPS);
        println!(
            "remote: ended '{}', {} steps in {:.1} s, iterations max {} mean {:.3}, histogram {:?}; child process {} at {}, parent {}",
            run.ended,
            run.iterations.len(),
            started.elapsed().as_secs_f64(),
            run.max_iterations(),
            run.mean_iterations(),
            run.histogram(),
            child.pid,
            child.address,
            std::process::id()
        );
        assert_eq!(structure.reconnects(), 0);
        (run, child.pid)
    })
}

#[test]
#[ignore = "heavy record (ADR 0345 amendment 2): 650 steps out of process; run in runs 1 and 2"]
fn a1r_out_of_process_every_step_converges() {
    let (run, pid) = remote_run();
    println!(
        "A1r: converged {}, iterations max {} mean {:.3}, largest last test quantity {:.3e}",
        run.all_converged,
        run.max_iterations(),
        run.mean_iterations(),
        run.largest_last()
    );
    assert_eq!(run.ended, "completed");
    assert_eq!(run.iterations.len(), CW_STEPS);
    assert!(run.all_converged);
    assert!(run.max_iterations() <= 16);
    assert!(run.mean_iterations() <= 12.0);
    assert!(run.largest_last() <= 1e-8);
    assert_ne!(*pid, std::process::id());
}

#[test]
#[ignore = "heavy record (ADR 0345 amendment 2): 650 steps out of process; run in runs 1 and 2"]
fn a2r_out_of_process_the_two_modes_oscillate_at_the_closed_form() {
    let ((r0, n0), (r1, n1)) = remote_run().0.frequencies();
    println!("A2r: omega_m/omega: Q0 {r0:.6} ({n0} crossings), Q1 {r1:.6} ({n1} crossings)");
    assert!((r0 - 1.0).abs() <= 0.005);
    assert!((r1 - 1.0).abs() <= 0.03);
    assert!(n0 >= 6 && n1 >= 11);
}

#[test]
#[ignore = "heavy record (ADR 0345 amendment 2): 650 steps out of process; run in runs 1 and 2"]
fn d_out_of_process_displacements_are_those_of_the_in_process_run() {
    let run = &remote_run().0;
    assert_eq!(run.iterations.len(), CW_STEPS);
    let (worst, counts, last) = against_reference(run, reference_long());
    println!(
        "D: max |q_remote - q_in_process| = {worst:e} m ({:e} X); {counts} steps with another iteration count; {last} last test quantities differ in their bits",
        worst / CW_X
    );
    assert!(worst <= 1e-12 * CW_X);
    assert_eq!(counts, 0);
}

#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn d2_both_participants_remote() {
    let child = ChildProcess::start("structure");
    let (_svc, address, _) = fluid_service(0);
    let ch = cw_channel();
    let mut fluid = RemoteFluid::connect(address, "fluid", remote_settings()).unwrap();
    let mut structure = RemoteStructure::connect(child.address, "structure", remote_settings()).unwrap();
    let map = cw_map(&ch, &fluid);
    let run = run_cw(&mut fluid, &mut structure, &map, 100);
    let (worst, counts, last) = against_reference(&run, reference());
    println!(
        "D2: ended '{}', {} steps; max |q - q_in_process| = {worst:e} m ({:e} X); {counts} steps with another count; {last} last test quantities differ",
        run.ended,
        run.iterations.len(),
        worst / CW_X
    );
    assert_eq!(run.ended, "completed");
    assert_eq!(run.iterations.len(), 100);
    assert!(worst <= 1e-12 * CW_X);
    assert_eq!(counts, 0);
    assert_eq!((fluid.reconnects(), structure.reconnects()), (0, 0));
}

#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn nc1_a_structure_stiffer_by_1e_9_is_seen() {
    let child = ChildProcess::start("structure-stiff");
    let ch = cw_channel();
    let mut fluid = cw_fluid(&ch, motion(&ch));
    let mut structure = RemoteStructure::connect(child.address, "structure", remote_settings()).unwrap();
    let map = cw_map(&ch, &fluid);
    let run = run_cw(&mut fluid, &mut structure, &map, 100);
    let (worst, counts, _) = against_reference(&run, reference());
    println!(
        "NC1: ended '{}'; max |q - q_in_process| = {worst:e} m ({:e} X); {counts} steps with another count",
        run.ended,
        worst / CW_X
    );
    assert_eq!(run.iterations.len(), 100);
    assert!(worst >= 1e-10 * CW_X);
}

// ---- F1 to F5: failures ----------------------------------------------------------------------------------------------

#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn f1_an_error_of_the_out_of_process_structure_is_located_and_both_are_restored() {
    let child = ChildProcess::start("structure-fails-2");
    let ch = cw_channel();
    let mut fluid = cw_fluid(&ch, motion(&ch));
    let mut structure = RemoteStructure::connect(child.address, "structure", remote_settings()).unwrap();
    let map = cw_map(&ch, &fluid);
    let f0 = fluid_bits(&fluid);
    let e = coupled_step(&mut fluid, &mut structure, &map, CW_DT, &cw_settings()).unwrap_err();
    println!("F1: {e}");
    assert_eq!(e.participant, "structure");
    assert_eq!(e.time, 0.0);
    assert_eq!(e.iteration, 2);
    assert!(e.message.contains("injected failure"));
    let moved = differing(&f0, &fluid_bits(&fluid));
    let time = structure.query_time().unwrap();
    let step = coupled_step(&mut fluid, &mut structure, &map, CW_DT, &cw_settings()).unwrap();
    let (differ, same_count) = step_against_reference(&step, 1);
    println!(
        "F1: {moved} bits of the fluid differ; the structure's time {time}; the next step: converged {}, {} iterations, {differ} bits of the node displacements differ from the reference",
        step.converged, step.iterations
    );
    assert_eq!(moved, 0);
    assert_eq!(time, 0.0);
    assert!(step.converged && same_count);
    assert_eq!(differ, 0);
    assert_eq!(structure.reconnects(), 0);
}

#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn f2_an_error_of_the_hosted_fluid_is_located_and_both_are_restored() {
    let (_svc, address, state) = fluid_service(2);
    let ch = cw_channel();
    let mut fluid = RemoteFluid::connect(address, "fluid", remote_settings()).unwrap();
    let mut structure = cw_structure();
    let map = cw_map(&ch, &fluid);
    let (f0, s0) = (state.lock().unwrap().clone(), structure_bits(&structure));
    let e = coupled_step(&mut fluid, &mut structure, &map, CW_DT, &cw_settings()).unwrap_err();
    println!("F2: {e}");
    assert_eq!(e.participant, "fluid");
    assert_eq!(e.time, 0.0);
    assert_eq!(e.iteration, 2);
    assert!(e.message.contains("injected failure of the mesh motion"));
    let (fluid_moved, structure_moved) = (
        differing(&f0, &state.lock().unwrap()),
        differing(&s0, &structure_bits(&structure)),
    );
    println!("F2: {fluid_moved} bits of the hosted fluid differ, {structure_moved} of the structure");
    assert_eq!((fluid_moved, structure_moved), (0, 0));
}

#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn f3_a_dropped_connection_to_the_structure_is_located_and_both_are_restored() {
    for (cut, label) in [(Cut::Before, "a, cut before"), (Cut::After, "b, cut after")] {
        let (_svc, address, wall) = structure_service();
        let relay = relay(address, "cosim.advance", 2, cut);
        let ch = cw_channel();
        let mut fluid = cw_fluid(&ch, motion(&ch));
        let mut structure = RemoteStructure::connect(relay.address, "structure", remote_settings()).unwrap();
        let map = cw_map(&ch, &fluid);
        let (f0, s0) = (fluid_bits(&fluid), structure_bits(&wall.lock().unwrap()));
        let e = coupled_step(&mut fluid, &mut structure, &map, CW_DT, &cw_settings()).unwrap_err();
        println!("F3 ({label}): {e}");
        assert_eq!(e.participant, "structure");
        assert_eq!(e.time, 0.0);
        assert_eq!(e.iteration, 2);
        assert!(e.message.contains("connection"));
        let (fluid_moved, structure_moved) = (
            differing(&f0, &fluid_bits(&fluid)),
            differing(&s0, &structure_bits(&wall.lock().unwrap())),
        );
        let step = coupled_step(&mut fluid, &mut structure, &map, CW_DT, &cw_settings()).unwrap();
        let (differ, same_count) = step_against_reference(&step, 1);
        println!(
            "F3 ({label}): {fluid_moved} bits of the fluid differ, {structure_moved} of the hosted structure; {} reconnection(s), lease {}; the relay accepted {} connections; the next step: converged {}, {} iterations, {differ} bits differ from the reference",
            structure.reconnects(),
            structure.lease(),
            relay.accepted.load(Ordering::SeqCst),
            step.converged,
            step.iterations
        );
        assert_eq!((fluid_moved, structure_moved), (0, 0));
        assert_eq!(structure.reconnects(), 1);
        assert_eq!(relay.accepted.load(Ordering::SeqCst), 2);
        assert!(step.converged && same_count);
        assert_eq!(differ, 0);
    }
}

#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn f3c_a_dropped_connection_to_the_fluid_is_located_and_both_are_restored() {
    let (_svc, address, state) = fluid_service(0);
    let relay = relay(address, "cosim.advance", 2, Cut::After);
    let ch = cw_channel();
    let mut fluid = RemoteFluid::connect(relay.address, "fluid", remote_settings()).unwrap();
    let mut structure = cw_structure();
    let map = cw_map(&ch, &fluid);
    let (f0, s0) = (state.lock().unwrap().clone(), structure_bits(&structure));
    let e = coupled_step(&mut fluid, &mut structure, &map, CW_DT, &cw_settings()).unwrap_err();
    println!("F3 (c, the fluid, cut after): {e}");
    assert_eq!(e.participant, "fluid");
    assert_eq!(e.time, 0.0);
    assert_eq!(e.iteration, 2);
    assert!(e.message.contains("connection"));
    let (fluid_moved, structure_moved) = (
        differing(&f0, &state.lock().unwrap()),
        differing(&s0, &structure_bits(&structure)),
    );
    let step = coupled_step(&mut fluid, &mut structure, &map, CW_DT, &cw_settings()).unwrap();
    let (differ, same_count) = step_against_reference(&step, 1);
    println!(
        "F3 (c): {fluid_moved} bits of the hosted fluid differ, {structure_moved} of the structure; {} reconnection(s); the next step: converged {}, {} iterations, {differ} bits differ from the reference",
        fluid.reconnects(),
        step.converged,
        step.iterations
    );
    assert_eq!((fluid_moved, structure_moved), (0, 0));
    assert_eq!(fluid.reconnects(), 1);
    assert!(step.converged && same_count);
    assert_eq!(differ, 0);
}

#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn f4_a_connection_dropped_between_steps_is_located_and_nothing_moves() {
    let (_svc, address, wall) = structure_service();
    let relay = relay(address, "cosim.checkpoint", 2, Cut::Before);
    let ch = cw_channel();
    let mut fluid = cw_fluid(&ch, motion(&ch));
    let mut structure = RemoteStructure::connect(relay.address, "structure", remote_settings()).unwrap();
    let map = cw_map(&ch, &fluid);
    let first = coupled_step(&mut fluid, &mut structure, &map, CW_DT, &cw_settings()).unwrap();
    assert_eq!(step_against_reference(&first, 1), (0, true));
    let (f1, s1) = (fluid_bits(&fluid), structure_bits(&wall.lock().unwrap()));
    let e = coupled_step(&mut fluid, &mut structure, &map, CW_DT, &cw_settings()).unwrap_err();
    println!("F4: {e}");
    assert_eq!(e.participant, "structure");
    assert_eq!(e.iteration, 0);
    assert!(e.message.contains("connection"));
    let (fluid_moved, structure_moved) = (
        differing(&f1, &fluid_bits(&fluid)),
        differing(&s1, &structure_bits(&wall.lock().unwrap())),
    );
    assert!(!structure.is_connected());
    structure.reconnect().unwrap();
    let step = coupled_step(&mut fluid, &mut structure, &map, CW_DT, &cw_settings()).unwrap();
    let (differ, same_count) = step_against_reference(&step, 2);
    println!(
        "F4: {fluid_moved} bits of the fluid differ, {structure_moved} of the hosted structure; after reconnect() the second step: converged {}, {} iterations, {differ} bits differ from the reference",
        step.converged, step.iterations
    );
    assert_eq!((fluid_moved, structure_moved), (0, 0));
    assert!(step.converged && same_count);
    assert_eq!(differ, 0);
}

/// A remote structure whose process is killed before its `kill_at`-th `advance`.
struct KilledStructure {
    inner: RemoteStructure,
    child: ChildProcess,
    calls: usize,
    kill_at: usize,
}

impl Participant for KilledStructure {
    fn name(&self) -> &str {
        self.inner.name()
    }
    fn time(&self) -> f64 {
        self.inner.time()
    }
    fn checkpoint(&mut self) -> Result<(), String> {
        self.inner.checkpoint()
    }
    fn restore(&mut self) -> Result<(), String> {
        self.inner.restore()
    }
}

impl Structure for KilledStructure {
    fn node_coordinates(&self) -> Vec<Vec3> {
        self.inner.node_coordinates()
    }
    fn advance(&mut self, dt: f64, forces: &[Vec3]) -> Result<Vec<Vec3>, String> {
        self.calls += 1;
        if self.calls == self.kill_at {
            self.child.kill();
        }
        self.inner.advance(dt, forces)
    }
}

#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn f5_a_participant_process_that_dies_is_located_and_the_fluid_is_restored() {
    let child = ChildProcess::start("structure");
    let ch = cw_channel();
    let mut fluid = cw_fluid(&ch, motion(&ch));
    let inner = RemoteStructure::connect(child.address, "structure", remote_settings()).unwrap();
    let mut structure = KilledStructure {
        inner,
        child,
        calls: 0,
        kill_at: 2,
    };
    let map = cw_map(&ch, &fluid);
    let f0 = fluid_bits(&fluid);
    let started = Instant::now();
    let e = coupled_step(&mut fluid, &mut structure, &map, CW_DT, &cw_settings()).unwrap_err();
    let elapsed = started.elapsed().as_secs_f64();
    let moved = differing(&f0, &fluid_bits(&fluid));
    println!("F5: {e}");
    println!("F5: the failed step took {elapsed:.2} s; {moved} bits of the fluid differ");
    assert_eq!(e.participant, "structure");
    assert_eq!(e.time, 0.0);
    assert_eq!(e.iteration, 2);
    assert!(e.message.contains("connection"));
    assert!(e.message.contains("restore failed"));
    assert_eq!(moved, 0);
    assert!(elapsed <= 30.0);
}

// ---- T1 (amendment 1): the cost of a call --------------------------------------------------------------------------

#[test]
#[cfg_attr(debug_assertions, ignore = "timing: run in release")]
fn t1_a_call_over_the_loopback_interface_takes_less_than_a_millisecond() {
    let (_svc, address, _) = structure_service();
    let structure = RemoteStructure::connect(address, "structure", remote_settings()).unwrap();
    let calls = 2000;
    let started = Instant::now();
    for _ in 0..calls {
        assert_eq!(structure.query_time().unwrap(), 0.0);
    }
    let mean = started.elapsed().as_secs_f64() / calls as f64;
    println!("T1: {calls} calls of cosim.time, {:.4} ms each", mean * 1e3);
    assert!(mean <= 1e-3);
}

// ---- V: the documents ------------------------------------------------------------------------------------------------

#[test]
fn v_the_protocol_documents_are_versioned_and_name_the_methods() {
    let docs = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs");
    let cosim = std::fs::read_to_string(docs.join("cosimulation-protocol.md")).unwrap();
    let service = std::fs::read_to_string(docs.join("service-protocol.md")).unwrap();
    assert!(cosim.contains("rcfd-cosim/1"));
    assert!(cosim.contains("## Versions"));
    assert!(service.contains("version 1.1"));
    assert!(service.contains("rcfd-cosim/1"));
    for m in COSIM_METHODS {
        assert!(cosim.contains(m), "{m} is not in cosimulation-protocol.md");
        assert!(service.contains(m), "{m} is not in service-protocol.md");
    }
}
