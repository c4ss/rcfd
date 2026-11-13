//! F-444 (ADR 0380): `rcfd serve --cosim <case>` hosts the co-simulation fluid participant of the case's `[cosim]`
//! table under protocol `rcfd-cosim/1`; the mock structure of ADR 0345 in a process of its own and the coupler
//! `rcfd_fv::cosim::coupled_step` in this process drive it over TCP, against the in-process run of the same case.
//!
//! The limits, set-ups and mutations were registered in ADR 0380 before this file existed. The model problem CWs, its
//! closed form and the crossings are those of `crates/rcfd-api/tests/cosim_service_f360.rs` (copied, not edited).
//!
//! Three processes: `rcfd serve` (the binary of this build), the structure (this test binary started a second time with
//! `RCFD_F444_ROLE` set, `child_structure_process`) and the coupler (the test). Every socket is on 127.0.0.1, port 0.

use rcfd_api::case::Case;
use rcfd_api::cosim_case::fluid_from_case;
use rcfd_api::cosim_service::{RemoteFluid, RemoteSettings, RemoteStructure, COSIM_PROTOCOL};
use rcfd_api::service::{Client, Service, ServiceConfig};
use rcfd_core::vec3;
use rcfd_fv::cosim::{
    coupled_step, piecewise_linear_rows, AtCap, CouplingSettings, Fluid, InterfaceMap, SprungWall, Structure,
};
use rcfd_fv::fsi::{relaxation_from_added_mass, Relaxation};
use rcfd_mesh::gen::{box_mesh, Grading};
use rcfd_mesh::motion::patch_points;
use serde_json::{json, Value};
use std::f64::consts::{PI, TAU};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

const RHO: f64 = 1.0;
const WALL: &str = "xmin";
const CW_L: f64 = 1.0;
const CW_W: f64 = 0.4;
const CW_LZ: f64 = 0.1;
const CW_MU: f64 = 4.0;
const CW_X: f64 = 0.002;
const CW_NU: f64 = 1e-4;
const CW_NODES: usize = 6;
const CW_DT: f64 = 0.01;
const CW_STEPS: usize = 650;
const FAST_STEPS: usize = 100;

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

fn rows_for(wall_y: &[f64]) -> Vec<Vec<(usize, f64)>> {
    piecewise_linear_rows(&cw_node_y(), wall_y).unwrap()
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

/// CWc (ADR 0380 set-up): CWs as a case, with `patch` as the moving patch of `[cosim]`.
fn cwc_json(patch: &str) -> Value {
    let mesh = box_mesh([20, 8, 2], [CW_L, CW_W, CW_LZ], [Grading::Uniform; 3]).unwrap();
    let wall_y: Vec<f64> = patch_points(&mesh, WALL)
        .unwrap()
        .iter()
        .map(|&q| mesh.points()[q][1])
        .collect();
    let q0 = cw_q0();
    let initial: Vec<f64> = rows_for(&wall_y)
        .iter()
        .flat_map(|row| {
            let x = row
                .iter()
                .fold(vec3::ZERO, |a, &(i, w)| vec3::add(a, vec3::scale([q0[i], 0.0, 0.0], w)));
            x.to_vec()
        })
        .collect();
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
            "patch": patch, "fixed_patches": ["xmax"], "smoothing_tolerance": 1e-13,
            "initial_displacement": initial
        }
    })
}

/// The case file of CWc (with the moving patch `patch`) in the test's temporary directory.
fn case_file(patch: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("cosim_serve_f444");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("cwc_{patch}_{}.json", std::process::id()));
    std::fs::write(&path, serde_json::to_string_pretty(&cwc_json(patch)).unwrap()).unwrap();
    path
}

fn remote() -> RemoteSettings {
    RemoteSettings {
        connect_timeout: Duration::from_secs(5),
        call_timeout: Duration::from_secs(60),
    }
}

// ---- the processes ---------------------------------------------------------------------------------------------------

/// `rcfd serve --addr 127.0.0.1:0 --cosim <case>`; killed when dropped.
struct Serve {
    child: Child,
    address: SocketAddr,
    stderr: Arc<Mutex<String>>,
}

impl Serve {
    fn start(case: &PathBuf) -> Serve {
        let mut child = Command::new(env!("CARGO_BIN_EXE_rcfd"))
            .args(["serve", "--addr", "127.0.0.1:0", "--cosim"])
            .arg(case)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let stderr = Arc::new(Mutex::new(String::new()));
        let (err, keep) = (child.stderr.take().unwrap(), stderr.clone());
        std::thread::spawn(move || {
            for line in BufReader::new(err).lines().map_while(Result::ok) {
                keep.lock().unwrap().push_str(&format!("{line}\n"));
            }
        });
        let out = child.stdout.take().unwrap();
        let (found, wait) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(out).lines().map_while(Result::ok) {
                if let Some(rest) = line.strip_prefix("rcfd service listening on ") {
                    let _ = found.send(rest.trim().to_string());
                }
            }
        });
        let address = match wait.recv_timeout(Duration::from_secs(120)) {
            Ok(a) => a.parse().unwrap(),
            Err(_) => {
                let _ = child.kill();
                panic!(
                    "rcfd serve did not report its address in 120 s: {}",
                    stderr.lock().unwrap()
                );
            }
        };
        Serve { child, address, stderr }
    }
}

impl Drop for Serve {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

const ROLE: &str = "RCFD_F444_ROLE";

/// The structure process: this test binary started with `RCFD_F444_ROLE`. Without the variable it does nothing.
#[test]
#[ignore = "helper: the structure process that the other tests start"]
fn child_structure_process() {
    let Ok(role) = std::env::var(ROLE) else {
        return;
    };
    let svc = Service::new(ServiceConfig::default());
    svc.host_structure("structure", move || {
        Ok(match role.as_str() {
            "structure" => Box::new(cw_structure()) as Box<dyn Structure>,
            "structure-stiff" => Box::new(cw_structure_with(1.0 + 1e-9)),
            other => return Err(format!("unknown role '{other}'")),
        })
    })
    .unwrap();
    let server = svc.serve_tcp("127.0.0.1:0").unwrap();
    println!("\nF444_CHILD {} {}", server.addr, std::process::id());
    std::io::stdout().flush().unwrap();
    let (done, wait) = channel();
    std::thread::spawn(move || {
        let mut sink = Vec::new();
        let _ = std::io::stdin().read_to_end(&mut sink);
        let _ = done.send(());
    });
    let _ = wait.recv_timeout(Duration::from_secs(900));
    std::process::exit(0);
}

/// A structure process; killed when dropped.
struct StructureProcess {
    child: Child,
    address: SocketAddr,
    pid: u32,
}

impl StructureProcess {
    fn start(role: &str) -> StructureProcess {
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["child_structure_process", "--exact", "--ignored", "--nocapture"])
            .env(ROLE, role)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let out = child.stdout.take().unwrap();
        let (found, wait) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(out).lines().map_while(Result::ok) {
                if let Some(rest) = line.strip_prefix("F444_CHILD ") {
                    let _ = found.send(rest.to_string());
                }
            }
        });
        let line = match wait.recv_timeout(Duration::from_secs(60)) {
            Ok(line) => line,
            Err(_) => {
                let _ = child.kill();
                panic!("the structure process did not report its address in 60 s");
            }
        };
        let mut parts = line.split_whitespace();
        StructureProcess {
            address: parts.next().unwrap().parse().unwrap(),
            pid: parts.next().unwrap().parse().unwrap(),
            child,
        }
    }
}

impl Drop for StructureProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

// ---- runs ------------------------------------------------------------------------------------------------------------

#[derive(Default)]
struct Run {
    t: Vec<f64>,
    q: Vec<Vec<f64>>,
    iterations: Vec<usize>,
    last: Vec<f64>,
    all_converged: bool,
    ended: String,
    pids: (u32, u32),
}

impl Run {
    fn max_iterations(&self) -> usize {
        self.iterations.iter().copied().max().unwrap_or(0)
    }
    fn mean_iterations(&self) -> f64 {
        self.iterations.iter().sum::<usize>() as f64 / self.iterations.len() as f64
    }
    fn largest_last(&self) -> f64 {
        self.last.iter().fold(0.0, |a: f64, b| a.max(*b))
    }
}

fn map_of(fluid: &dyn Fluid) -> InterfaceMap {
    let wall_y: Vec<f64> = fluid.point_coordinates().iter().map(|p| p[1]).collect();
    InterfaceMap::new(rows_for(&wall_y), CW_NODES, fluid.face_points()).unwrap()
}

fn run_cw(fluid: &mut dyn Fluid, structure: &mut dyn Structure, steps: usize) -> Run {
    let map = map_of(fluid);
    let mut run = Run {
        all_converged: true,
        ended: "completed".into(),
        ..Default::default()
    };
    run.t.push(0.0);
    run.q.push(cw_q0());
    for n in 1..=steps {
        let step = match coupled_step(fluid, structure, &map, CW_DT, &cw_settings()) {
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

fn cwc() -> Case {
    Case::from_json(&cwc_json(WALL)).unwrap()
}

/// The in-process run of CWc (the reference of c4).
fn in_process(steps: usize) -> Run {
    let started = Instant::now();
    let mut fluid = fluid_from_case(&cwc()).unwrap();
    let run = run_cw(&mut fluid, &mut cw_structure(), steps);
    println!(
        "in-process CWc: '{}', {} steps in {:.1} s, iterations max {} mean {:.3}",
        run.ended,
        run.iterations.len(),
        started.elapsed().as_secs_f64(),
        run.max_iterations(),
        run.mean_iterations()
    );
    run
}

/// The served run: the fluid in `rcfd serve`, the structure (role `role`) in its own process.
fn served(steps: usize, role: &str) -> Run {
    let started = Instant::now();
    let serve = Serve::start(&case_file(WALL));
    let structure = StructureProcess::start(role);
    let mut fluid = RemoteFluid::connect(serve.address, "fluid", remote()).unwrap();
    let mut s = RemoteStructure::connect(structure.address, "structure", remote()).unwrap();
    let mut run = run_cw(&mut fluid, &mut s, steps);
    run.pids = (serve.child.id(), structure.pid);
    println!(
        "served CWc ({role}): '{}', {} steps in {:.1} s, iterations max {} mean {:.3}; rcfd serve pid {}, structure pid {}, test pid {}",
        run.ended,
        run.iterations.len(),
        started.elapsed().as_secs_f64(),
        run.max_iterations(),
        run.mean_iterations(),
        run.pids.0,
        run.pids.1,
        std::process::id()
    );
    run
}

/// `max abs(q - q_ref)` over steps and nodes, and the steps whose iteration count differs.
fn against(run: &Run, reference: &Run) -> (f64, usize) {
    let mut worst: f64 = 0.0;
    for (a, b) in run.q.iter().zip(&reference.q) {
        for (x, y) in a.iter().zip(b) {
            worst = worst.max((x - y).abs());
        }
    }
    let counts = run
        .iterations
        .iter()
        .zip(&reference.iterations)
        .filter(|(a, b)| a != b)
        .count();
    (worst, counts)
}

fn reference_fast() -> &'static Run {
    static RUN: OnceLock<Run> = OnceLock::new();
    RUN.get_or_init(|| in_process(FAST_STEPS))
}

// ---- H1, H2: rcfd serve hosts the participant ------------------------------------------------------------------------

fn flat(v: &Value) -> Vec<f64> {
    v.as_array()
        .unwrap()
        .iter()
        .flat_map(|r| r.as_array().unwrap().iter().map(|x| x.as_f64().unwrap()))
        .collect()
}

/// H1 (c2): `rcfd serve --cosim` lists, opens and describes the participant of the case under `rcfd-cosim/1`.
#[test]
fn h1_rcfd_serve_hosts_the_participant_of_the_case() {
    let serve = Serve::start(&case_file(WALL));
    let mut c = Client::connect(serve.address).unwrap();
    let list = c.call("cosim.list", json!({})).unwrap();
    let open = c.call("cosim.open", json!({"participant": "fluid"})).unwrap();
    let d = c
        .call(
            "cosim.describe",
            json!({"participant": "fluid", "lease": open["lease"]}),
        )
        .unwrap();
    let local = fluid_from_case(&cwc()).unwrap();
    let points: Vec<f64> = local.point_coordinates().iter().flatten().copied().collect();
    let remote_points = flat(&d["points"]);
    let differ = points
        .iter()
        .zip(&remote_points)
        .filter(|(a, b)| a.to_bits() != b.to_bits())
        .count();
    let faces: Vec<Vec<usize>> = serde_json::from_value(d["faces"].clone()).unwrap();
    println!(
        "H1: list {list}; open {open}; {} points, {differ} bits differ; faces equal {}; stderr: {}",
        remote_points.len() / 3,
        faces == local.face_points(),
        serve.stderr.lock().unwrap().trim()
    );
    assert_eq!(list["protocol"], COSIM_PROTOCOL);
    assert_eq!(
        list["participants"],
        json!([{"participant": "fluid", "kind": "fluid", "name": "fluid"}])
    );
    assert_eq!(open["protocol"], COSIM_PROTOCOL);
    assert_eq!(open["lease"], 1);
    assert_eq!(open["time"].as_f64(), Some(0.0));
    assert_eq!(remote_points.len(), points.len());
    assert_eq!(differ, 0);
    assert_eq!(faces, local.face_points());
}

/// H2 (c2): a case whose patch does not exist stops `rcfd serve` with the located error; the help names `--cosim`.
#[test]
fn h2_a_bad_case_stops_rcfd_serve_with_a_located_error() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rcfd"))
        .args(["serve", "--addr", "127.0.0.1:0", "--cosim"])
        .arg(case_file("nope"))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let started = Instant::now();
    let status = loop {
        if let Some(s) = child.try_wait().unwrap() {
            break Some(s);
        }
        if started.elapsed() > Duration::from_secs(120) {
            let _ = child.kill();
            break None;
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let mut err = String::new();
    child.stderr.take().unwrap().read_to_string(&mut err).unwrap();
    let help = Command::new(env!("CARGO_BIN_EXE_rcfd")).arg("--help").output().unwrap();
    let help = String::from_utf8_lossy(&help.stdout).into_owned() + &String::from_utf8_lossy(&help.stderr);
    println!(
        "H2: status {status:?} after {:.1} s; stderr: {}",
        started.elapsed().as_secs_f64(),
        err.trim()
    );
    let status = status.expect("rcfd serve did not stop in 120 s");
    assert!(!status.success());
    assert!(err.contains("cosim.patch"), "{err}");
    assert!(help.contains("--cosim"));
}

// ---- D100, NC1: the served run against the in-process run ------------------------------------------------------------

/// D100 (c3, c4): 100 steps of the served run against the in-process run of CWc.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn d100_the_served_participant_moves_as_the_in_process_one() {
    let run = served(FAST_STEPS, "structure");
    let (worst, counts) = against(&run, reference_fast());
    println!(
        "D100: max |dq| = {worst:e} m = {:e} X; {counts} steps with another count; all converged {}",
        worst / CW_X,
        run.all_converged
    );
    assert_eq!(run.iterations.len(), FAST_STEPS, "{}", run.ended);
    assert!(worst <= 1e-12 * CW_X);
    assert_eq!(counts, 0);
    assert!(run.all_converged);
}

/// NC1 (control of D100): a structure stiffer by `1e-9` is seen.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn nc1_a_structure_stiffer_by_1e_9_is_seen() {
    let run = served(FAST_STEPS, "structure-stiff");
    let (worst, _) = against(&run, reference_fast());
    println!("NC1: max |dq| = {worst:e} m = {:e} X", worst / CW_X);
    assert_eq!(run.iterations.len(), FAST_STEPS, "{}", run.ended);
    assert!(worst >= 1e-10 * CW_X);
}

// ---- A1s, A2s, Ds: 650 steps (heavy records) -------------------------------------------------------------------------

fn long_runs() -> &'static (Run, Run) {
    static RUNS: OnceLock<(Run, Run)> = OnceLock::new();
    RUNS.get_or_init(|| (served(CW_STEPS, "structure"), in_process(CW_STEPS)))
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

/// A1s (c3): every step of the served run of 650 steps converges within the iteration bounds of ADR 0256 A1.
#[test]
#[ignore = "heavy record (ADR 0380): 650 steps served and in process; run once"]
fn a1s_served_every_step_converges() {
    let (run, _) = long_runs();
    println!(
        "A1s: '{}'; {} steps, all converged {}; iterations max {} mean {:.3}; largest last test {:e}",
        run.ended,
        run.iterations.len(),
        run.all_converged,
        run.max_iterations(),
        run.mean_iterations(),
        run.largest_last()
    );
    assert_eq!(run.iterations.len(), CW_STEPS);
    assert!(run.all_converged);
    assert!(run.max_iterations() <= 16 && run.mean_iterations() <= 12.0);
    assert!(run.largest_last() <= 1e-8);
    let me = std::process::id();
    assert!(run.pids.0 != me && run.pids.1 != me && run.pids.0 != run.pids.1);
}

/// A2s (c3): the two modes of the served run oscillate at the closed form (ADR 0256 A2).
#[test]
#[ignore = "heavy record (ADR 0380): 650 steps served and in process; run once"]
fn a2s_served_the_two_modes_oscillate_at_the_closed_form() {
    let (run, _) = long_runs();
    let (q0, q1): (Vec<f64>, Vec<f64>) = run.q.iter().map(|q| modal(q)).unzip();
    let (w0, n0) = measured_omega(&run.t, &q0);
    let (w1, n1) = measured_omega(&run.t, &q1);
    let (r0, r1) = (w0 / cw_omega(0), w1 / cw_omega(1));
    println!("A2s: omega_m/omega_0 = {r0:.6} ({n0} crossings), omega_m/omega_1 = {r1:.6} ({n1} crossings)");
    assert!((r0 - 1.0).abs() <= 0.005 && n0 >= 6);
    assert!((r1 - 1.0).abs() <= 0.03 && n1 >= 11);
}

/// Ds (c4): 650 steps of the served run against the in-process run of CWc.
#[test]
#[ignore = "heavy record (ADR 0380): 650 steps served and in process; run once"]
fn ds_served_displacements_are_those_of_the_in_process_run() {
    let (run, reference) = long_runs();
    let (worst, counts) = against(run, reference);
    println!(
        "Ds: max |dq| = {worst:e} m = {:e} X; {counts} steps with another count",
        worst / CW_X
    );
    assert_eq!(run.iterations.len(), CW_STEPS);
    assert_eq!(reference.iterations.len(), CW_STEPS);
    assert!(worst <= 1e-12 * CW_X);
    assert_eq!(counts, 0);
}
