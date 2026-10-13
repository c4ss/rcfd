//! Run-control contract (F-121): checkpoint round trip, drive outcomes and exit codes, check-only,
//! journal playback and the status-stream summary.
//!
//! Checks fixed before running: a run restored from a checkpoint and stepped k more times must
//! reproduce the uninterrupted run bit for bit (the restart state is complete for steady laminar
//! SIMPLEC); stop file -> state `interrupted`/exit 7; wall-clock limit 0 -> `time_limit`/exit 8;
//! tolerances reached -> `converged`/exit 0; iteration cap -> `not_converged`/exit 5.

use rcfd_api::case::Case;
use rcfd_api::runctl::{self, exit, RunOptions};
use rcfd_api::sim::Simulation;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Mutex;

const CAVITY: &str = r#"
title = "runctl cavity"
[mesh]
generator = "cavity_2d"
nx = 12
ny = 12
[physics]
nu = 0.01
[boundaries.lid]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.walls]
velocity = "fixed"
[solver]
algorithm = "simplec"
convection = "upwind"
max_iterations = 400
tolerance_u = 1e-7
tolerance_p = 1e-7
"#;

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_runctl_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn sim_with(extra: &str) -> Simulation {
    let text = CAVITY.to_string() + extra;
    Simulation::new(&Case::parse_toml(&text).unwrap()).unwrap()
}

fn drive(sim: Simulation, opts: &RunOptions, stop: bool) -> (Mutex<Simulation>, runctl::RunOutcome, Vec<Value>) {
    let m = Mutex::new(sim);
    let mut ev = Vec::new();
    let out = runctl::drive(&m, opts, &|| stop, &mut |e| ev.push(e.clone()));
    (m, out, ev)
}

#[test]
fn restored_checkpoint_reproduces_the_uninterrupted_run_bit_for_bit() {
    let dir = tmp("ckpt");
    let mut a = sim_with("");
    for _ in 0..40 {
        a.step();
    }
    let path = dir.join("c.rcfd");
    assert!(a.supports_checkpoint());
    a.save_checkpoint(&path).unwrap();
    for _ in 0..25 {
        a.step();
    }
    let mut b = sim_with("");
    b.load_checkpoint(&path).unwrap();
    for _ in 0..25 {
        b.step();
    }
    assert_eq!(a.iteration(), 65);
    assert_eq!(b.iteration(), 65);
    for f in ["ux", "uy", "p"] {
        assert_eq!(a.field(f).unwrap(), b.field(f).unwrap(), "{f} differs after restart");
    }
    // a checkpoint of another mesh is refused
    let other = Case::parse_toml(&CAVITY.replace("nx = 12", "nx = 10")).unwrap();
    let mut c = Simulation::new(&other).unwrap();
    assert!(c
        .load_checkpoint(&path)
        .unwrap_err()
        .to_string()
        .contains("different mesh"));
    // a damaged file is refused (checksummed container)
    let mut bytes = std::fs::read(&path).unwrap();
    let n = bytes.len();
    bytes[n / 2] ^= 0xff;
    let bad = dir.join("bad.rcfd");
    std::fs::write(&bad, bytes).unwrap();
    assert!(sim_with("").load_checkpoint(&bad).is_err());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn unsupported_runs_say_so_instead_of_writing_a_checkpoint() {
    let mut c = Case::parse_toml(CAVITY).unwrap();
    c.set("turbulence.model", json!("k_epsilon")).unwrap();
    c.set("boundaries.lid.turbulence_length", json!(0.01)).unwrap();
    c.set("boundaries.walls.turbulence_length", json!(0.01)).unwrap();
    let t = Simulation::new(&c).unwrap();
    assert!(!t.supports_checkpoint());
    let e = t.save_checkpoint(&tmp("unsup").join("x")).unwrap_err().to_string();
    assert!(e.contains("k-epsilon"), "{e}");
    // a transient run is not supported either
    let tr = Case::parse_toml(&CAVITY.replace("simplec", "piso")).unwrap();
    let t = Simulation::new(&tr).unwrap();
    assert!(!t.supports_checkpoint());
}

#[test]
fn drive_reports_converged_not_converged_interrupted_and_time_limit() {
    let dir = tmp("drive");
    let ck = dir.join("r.rcfd");
    let opts = RunOptions {
        checkpoint: Some(ck.clone()),
        ..Default::default()
    };
    // converged
    let (_, out, ev) = drive(sim_with(""), &opts, false);
    assert_eq!((out.state, out.exit_code), ("converged", exit::OK), "{out:?}");
    assert!(ev.first().unwrap()["event"] == "start" && ev.last().unwrap()["event"] == "finish");
    assert!(
        ev.iter().any(|e| e["event"] == "checkpoint"),
        "converged run writes a restart file"
    );
    assert_eq!(ev.last().unwrap()["exit_code"], 0);
    assert!(ck.exists());
    // sequence numbers increase by one
    for (i, e) in ev.iter().enumerate() {
        assert_eq!(e["seq"], i as u64 + 1);
        assert_eq!(e["format"], "rcfd-status");
    }
    // not converged: iteration cap before the tolerances
    let (_, out, _) = drive(sim_with("").tap_cap(15), &RunOptions::default(), false);
    assert_eq!(
        (out.state, out.exit_code),
        ("not_converged", exit::NOT_CONVERGED),
        "{out:?}"
    );
    // interrupted by the caller's stop flag, and by the stop file
    let (_, out, ev) = drive(sim_with(""), &opts, true);
    assert_eq!(
        (out.state, out.exit_code, out.iterations),
        ("interrupted", exit::INTERRUPTED, 0)
    );
    assert!(ev
        .iter()
        .any(|e| e["event"] == "checkpoint" && e["why"] == "interrupted"));
    let stop = dir.join("STOP");
    std::fs::write(&stop, "").unwrap();
    let o2 = RunOptions {
        stop_file: Some(stop.clone()),
        ..Default::default()
    };
    let (_, out, _) = drive(sim_with(""), &o2, false);
    assert_eq!(out.exit_code, exit::INTERRUPTED);
    assert!(out.reason.contains("stop file"), "{}", out.reason);
    // wall clock
    let o3 = RunOptions {
        max_elapsed: Some(0.0),
        ..Default::default()
    };
    let (_, out, _) = drive(sim_with(""), &o3, false);
    assert_eq!((out.state, out.exit_code), ("time_limit", exit::TIME_LIMIT));
    // step limit of this launch is not a failure and can be continued
    let o4 = RunOptions {
        max_steps: Some(7),
        ..Default::default()
    };
    let (m, out, _) = drive(sim_with(""), &o4, false);
    assert_eq!((out.state, out.exit_code, out.iterations), ("step_limit", exit::OK, 7));
    let (_, out, _) = {
        let sim = m.into_inner().unwrap();
        drive(sim, &RunOptions::default(), false)
    };
    assert_eq!(out.state, "converged");
    std::fs::remove_dir_all(&dir).ok();
}

trait Cap {
    fn tap_cap(self, n: usize) -> Self;
}
impl Cap for Simulation {
    fn tap_cap(self, n: usize) -> Self {
        let mut c = self.case.clone();
        c.set("solver.max_iterations", json!(n)).unwrap();
        Simulation::new(&c).unwrap()
    }
}

#[test]
fn interrupted_run_continued_from_its_checkpoint_matches_the_uninterrupted_result() {
    // uninterrupted reference
    let (full, fo, _) = drive(sim_with(""), &RunOptions::default(), false);
    assert_eq!(fo.state, "converged");
    // interrupted after 30 steps, then continued from the file
    let dir = tmp("cont");
    let ck = dir.join("r.rcfd");
    let o1 = RunOptions {
        max_steps: Some(30),
        checkpoint: Some(ck.clone()),
        ..Default::default()
    };
    let (_, o, _) = drive(sim_with(""), &o1, false);
    assert_eq!(o.iterations, 30);
    let mut s2 = sim_with("");
    s2.load_checkpoint(&ck).unwrap();
    let (cont, co, _) = drive(s2, &RunOptions::default(), false);
    assert_eq!(co.state, "converged");
    // residual normalisation restarts from the restored state, so the stopping iteration may differ
    // by a few steps; the converged fields must agree to well below the discretisation scale
    let (f, c) = (full.into_inner().unwrap(), cont.into_inner().unwrap());
    let (fu, cu) = (f.field("ux").unwrap(), c.field("ux").unwrap());
    let worst = fu.iter().zip(&cu).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max);
    eprintln!(
        "uninterrupted {} iterations, continued {} iterations, max |dux| = {worst:.3e}",
        fo.iterations, co.iterations
    );
    assert!(worst < 1e-5, "max |dux| = {worst}");
    assert!((fo.iterations as i64 - co.iterations as i64).abs() <= 15);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn check_only_classifies_problems() {
    let ok = Case::parse_toml(CAVITY).unwrap();
    let (code, ev) = runctl::check_only(&ok);
    assert_eq!(
        (code, ev["ok"].clone(), ev["cells"].clone()),
        (exit::OK, json!(true), json!(144))
    );
    // a patch the mesh does not have is a set-up error (exit 3)
    let mut missing = ok.clone();
    missing.set("boundaries.nonexistent.velocity", json!("fixed")).unwrap();
    let (code, ev) = runctl::check_only(&missing);
    assert_eq!(code, exit::INVALID, "{ev}");
    assert!(!ev["diagnostics"].as_array().unwrap().is_empty());
}

#[test]
fn journal_playback_and_stream_summary() {
    let mut c = Case::parse_toml(CAVITY).unwrap();
    c.set("mesh.nx", json!(10)).unwrap();
    let journal = json!({
        "format": "rcfd-journal", "version": 1,
        "entries": [
            {"op": "create", "source": {"toml": CAVITY}},
            {"op": "set", "path": "mesh.nx", "value": 10},
        ],
    });
    let back = runctl::case_from_journal(&journal, std::path::Path::new("."), None).unwrap();
    assert_eq!(back.tree.content_hash(), c.tree.content_hash());
    let mut bad = journal.clone();
    bad["version"] = json!(2);
    assert!(runctl::case_from_journal(&bad, std::path::Path::new("."), None).is_err());

    let (_, _, ev) = drive(sim_with(""), &RunOptions::default(), false);
    let text = ev.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("\n");
    let s = runctl::summarize_stream(&text).unwrap();
    assert_eq!(
        (s["state"].clone(), s["exit_code"].clone(), s["complete"].clone()),
        (json!("converged"), json!(0), json!(true))
    );
    assert!(s["iterations"].as_u64().unwrap() > 10);
    // truncated stream (run still going) and foreign documents
    let partial = ev[..3].iter().map(|e| e.to_string()).collect::<Vec<_>>().join("\n");
    let s = runctl::summarize_stream(&partial).unwrap();
    assert_eq!(
        (s["complete"].clone(), s["state"].clone()),
        (json!(false), json!("running"))
    );
    assert!(runctl::summarize_stream("{\"hello\": 1}").is_err());
    assert!(runctl::summarize_stream("not json").is_err());
}
