//! Chained run configurations (F-241, ADR 0031 D4): checks C1-C6 as registered (library level; the
//! command-line half of C1 is in `crates/rcfd-cli/tests/chain_w19c.rs`).

use rcfd_api::case::Case;
use rcfd_api::chain;
use rcfd_api::runctl::{self, RunOptions};
use rcfd_api::sim::Simulation;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const BASE: &str = r#"
title = "chain cavity"
[mesh]
generator = "cavity_2d"
nx = 16
ny = 16
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
max_iterations = 2000
tolerance_u = 1e-8
tolerance_p = 1e-8
"#;

const CHAIN: &str = r#"
[chain]
sequence = ["steady", "transient"]
[chain.stages.steady]
[chain.stages.transient]
set = ["solver.algorithm=\"piso\"", "solver.dt=0.05", "solver.end_time=1.0"]
"#;

const TRANSIENT_SOLVER: &str = r#"
algorithm = "piso"
dt = 0.05
end_time = 1.0
"#;

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_chain_w19c_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn chain_case() -> Case {
    Case::parse_toml(&format!("{BASE}{CHAIN}")).unwrap()
}

/// Run a chain, returning its outcome and all events.
fn run(case: &Case, out: &Path) -> (chain::ChainOutcome, Vec<Value>) {
    let mut events = Vec::new();
    let o = chain::run_chain(case, &RunOptions::default(), out, &|| false, &mut |e| {
        events.push(e.clone())
    });
    (o, events)
}

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

fn fields(s: &Simulation) -> (Vec<u64>, Vec<u64>, Vec<u64>, Vec<u64>, u64) {
    (
        bits(&s.field("ux").unwrap()),
        bits(&s.field("uy").unwrap()),
        bits(&s.field("p").unwrap()),
        bits(s.flux()),
        s.time().to_bits(),
    )
}

fn drive_to_end(sim: Simulation, checkpoint: Option<PathBuf>) -> (Simulation, runctl::RunOutcome) {
    let m = Mutex::new(sim);
    let opts = RunOptions {
        checkpoint,
        ..RunOptions::default()
    };
    let o = runctl::drive(&m, &opts, &|| false, &mut |_| {});
    (m.into_inner().unwrap(), o)
}

/// The manual procedure: steady run to its restart file, a separately written transient case,
/// `restart_from`, drive.
fn manual(dir: &Path) -> Simulation {
    let steady = Case::parse_toml(BASE).unwrap();
    let ck = dir.join("manual_steady.rcfd");
    let (_, o) = drive_to_end(Simulation::new(&steady).unwrap(), Some(ck.clone()));
    assert_eq!(o.exit_code, 0);
    let text = BASE.replace("algorithm = \"simplec\"", TRANSIENT_SOLVER.trim());
    let transient = Case::parse_toml(&text).unwrap();
    let mut sim = Simulation::new(&transient).unwrap();
    sim.restart_from(&ck, rcfd_post::remap::RemapMethod::Gradient, None)
        .unwrap();
    let (sim, o) = drive_to_end(sim, None);
    assert_eq!(o.exit_code, 0, "{}", o.reason);
    sim
}

fn load_final(case: &Case, file: &Path) -> Simulation {
    let (_, cases) = (0, chain::stage_cases(case).unwrap());
    let last = &cases.last().unwrap().1;
    let mut s = Simulation::new(last).unwrap();
    s.load_restart(file).unwrap();
    s
}

fn rel_l2(a: &[f64], b: &[f64], w: &[f64]) -> f64 {
    let num: f64 = a.iter().zip(b).zip(w).map(|((x, y), v)| v * (x - y).powi(2)).sum();
    let den: f64 = b.iter().zip(w).map(|(y, v)| v * y * y).sum();
    (num / den).sqrt()
}

#[test]
fn c1_c5_chain_equals_the_manual_procedure_bit_for_bit_and_events_are_ordered() {
    let dir = tmp("c1");
    let case = chain_case();
    let (o, events) = run(&case, &dir.join("out"));
    assert_eq!(o.exit_code, 0);
    assert_eq!(o.stages.len(), 2);
    let chained = load_final(&case, &dir.join("out/transient/restart.rcfd"));
    let by_hand = manual(&dir);
    assert!(
        fields(&chained) == fields(&by_hand),
        "chain differs from the manual procedure"
    );
    println!(
        "C1: chain final state is bit-identical to the manual procedure (time {})",
        by_hand.time()
    );

    // C5: event order
    let pos = |kind: &str, stage: &str| {
        events
            .iter()
            .position(|e| e["event"] == kind && e["stage"] == stage)
            .unwrap_or_else(|| panic!("no {kind} for {stage}"))
    };
    let (s0, f0, s1, f1) = (
        pos("stage_start", "steady"),
        pos("stage_finish", "steady"),
        pos("stage_start", "transient"),
        pos("stage_finish", "transient"),
    );
    let cf = events.iter().position(|e| e["event"] == "chain_finish").unwrap();
    assert!(s0 < f0 && f0 < s1 && s1 < f1 && f1 < cf && cf == events.len() - 1);
    assert_eq!(events[cf]["exit_code"], 0);
    for (i, e) in events.iter().enumerate() {
        let kind = e["event"].as_str().unwrap();
        if kind == "chain_finish" {
            continue;
        }
        assert!(e["stage"].is_string(), "event {i} ({kind}) has no stage: {e}");
        let st = e["stage"].as_str().unwrap();
        if i > s0 && i < f0 {
            assert_eq!(st, "steady", "event {i} {kind}");
        }
        if i > s1 && i < f1 {
            assert_eq!(st, "transient", "event {i} {kind}");
        }
    }
    assert!(events
        .iter()
        .any(|e| e["event"] == "restart" && e["stage"] == "transient" && e["report"]["restart"] == "initialised"));
    assert!(dir.join("out/steady/solution.vtu").exists() && dir.join("out/transient/solution.vtu").exists());
    println!(
        "C5: {} events, order stage_start/finish steady, stage_start/finish transient, chain_finish",
        events.len()
    );
}

#[test]
fn c2_acceptance_final_state_is_close_to_the_single_steady_run() {
    let dir = tmp("c2");
    let case = chain_case();
    let (o, _) = run(&case, &dir.join("out"));
    assert_eq!(o.exit_code, 0);
    let chained = load_final(&case, &dir.join("out/transient/restart.rcfd"));
    let (steady, so) = drive_to_end(Simulation::new(&Case::parse_toml(BASE).unwrap()).unwrap(), None);
    assert_eq!(so.exit_code, 0);
    let w: Vec<f64> = steady.geom.mesh.cell_volumes().to_vec();
    let (a, b) = (chained.field("speed").unwrap(), steady.field("speed").unwrap());
    let ds = rel_l2(&a, &b, &w);
    let mean = |p: &[f64]| p.iter().zip(&w).map(|(x, v)| x * v).sum::<f64>() / w.iter().sum::<f64>();
    let (pa, pb) = (chained.field("p").unwrap(), steady.field("p").unwrap());
    let (ma, mb) = (mean(&pa), mean(&pb));
    let pa0: Vec<f64> = pa.iter().map(|x| x - ma).collect();
    let pb0: Vec<f64> = pb.iter().map(|x| x - mb).collect();
    let dp = rel_l2(&pa0, &pb0, &w);
    println!("C2: speed relative L2 distance {ds:.3e}, pressure (mean removed) {dp:.3e}; limits 2e-2");
    assert!(ds <= 2e-2, "speed distance {ds}");
    assert!(dp <= 2e-2, "pressure distance {dp}");
}

#[test]
fn c3_mesh_change_between_stages_transfers_the_solution() {
    let dir = tmp("c3");
    let text = BASE.replace("nx = 16", "nx = 12").replace("ny = 16", "ny = 12")
        + r#"
[chain]
sequence = ["coarse", "fine"]
[chain.stages.coarse]
[chain.stages.fine]
set = ["mesh.nx=24", "mesh.ny=24"]
"#;
    let case = Case::parse_toml(&text).unwrap();
    let (o, events) = run(&case, &dir.join("out"));
    assert_eq!(o.exit_code, 0);
    let start = events
        .iter()
        .find(|e| e["event"] == "start" && e["stage"] == "fine")
        .expect("start of fine");
    assert_eq!(start["cells"], 576);
    let rep = events
        .iter()
        .find(|e| e["event"] == "restart" && e["stage"] == "fine")
        .expect("restart report");
    assert_eq!(rep["report"]["restart"], "transferred", "{rep}");
    let fin = events
        .iter()
        .rev()
        .find(|e| e["event"] == "finish" && e["stage"] == "fine")
        .unwrap();
    assert_eq!(fin["exit_code"], 0);
    println!(
        "C3: fine stage {} iterations after the transfer, report {}",
        fin["iterations"], rep["report"]
    );
}

#[test]
fn c4_failure_propagates_and_later_stages_never_start() {
    let dir = tmp("c4");
    let text = BASE.to_string()
        + r#"
[chain]
sequence = ["a", "b"]
[chain.stages.a]
set = ["solver.max_iterations=3"]
[chain.stages.b]
"#;
    let case = Case::parse_toml(&text).unwrap();
    let (o, events) = run(&case, &dir.join("out"));
    assert_eq!(o.exit_code, 5);
    assert_eq!(o.stages.len(), 1);
    assert!(!events.iter().any(|e| e["event"] == "stage_start" && e["stage"] == "b"));
    assert!(!dir.join("out/b").exists());
    let cf = events.last().unwrap();
    assert_eq!(cf["event"], "chain_finish");
    assert_eq!(cf["exit_code"], 5);
    // with allow_unconverged the same chain goes on
    let text2 = text.replace("[chain.stages.a]\n", "[chain.stages.a]\nallow_unconverged = true\n");
    let (o2, ev2) = run(&Case::parse_toml(&text2).unwrap(), &dir.join("out2"));
    assert_eq!(o2.stages.len(), 2, "{ev2:?}");
    println!(
        "C4: exit {} with stage b never started; with allow_unconverged both stages run (exit {})",
        o.exit_code, o2.exit_code
    );
}

#[test]
fn c6_static_checks_are_located() {
    let mk = |chain: &str| Case::parse_toml(&format!("{BASE}{chain}"));
    let paths = |r: Result<Case, Vec<rcfd_api::case::Diagnostic>>| -> Vec<String> {
        r.err().unwrap_or_default().into_iter().map(|d| d.path).collect()
    };
    let unknown = paths(mk(
        "[chain]\nsequence = [\"steady\", \"ghost\"]\n[chain.stages.steady]\n",
    ));
    assert_eq!(unknown, vec!["chain.sequence.1"]);
    let dup = paths(mk("[chain]\nsequence = [\"s\", \"s\"]\n[chain.stages.s]\n"));
    assert_eq!(dup, vec!["chain.sequence.1"]);
    let nosign = paths(mk(
        "[chain]\nsequence = [\"t\"]\n[chain.stages.t]\nset = [\"solver.dt=0.1\", \"solver.dt\"]\n",
    ));
    assert_eq!(nosign, vec!["chain.stages.t.set.1"]);
    let nopath = paths(mk(
        "[chain]\nsequence = [\"t\"]\n[chain.stages.t]\nset = [\"solver.no_such_key=1\"]\n",
    ));
    assert_eq!(nopath, vec!["chain.stages.t.set.0"]);
    let wrongtype = paths(mk(
        "[chain]\nsequence = [\"t\"]\n[chain.stages.t]\nset = [\"solver.dt=\\\"fast\\\"\"]\n",
    ));
    assert_eq!(wrongtype, vec!["chain.stages.t.set.0"]);
    let notjson = paths(mk(
        "[chain]\nsequence = [\"t\"]\n[chain.stages.t]\nset = [\"solver.dt={\"]\n",
    ));
    assert_eq!(notjson, vec!["chain.stages.t.set.0"]);
    let nested = paths(mk(
        "[chain]\nsequence = [\"t\"]\n[chain.stages.t]\nset = [\"chain.sequence=[]\"]\n",
    ));
    assert_eq!(nested, vec!["chain.stages.t.set.0"]);
    // a valid chain loads, with line numbers on the located diagnostics
    assert!(mk(CHAIN).is_ok());
    let line = Case::parse_toml(&format!("{BASE}[chain]\nsequence = [\"ghost\"]\n"))
        .err()
        .unwrap();
    println!("C6: located diagnostics: {line:?}");
}

/// ADR 0097 T1: two transient stages split at t = 0.5 (ten full steps of 0.05, no clipped step) against one transient
/// run from rest to 1.0.
#[test]
fn c7_chained_transient_segments_equal_the_single_transient_run_bit_for_bit() {
    let dir = tmp("c7");
    let transient = BASE.replace("algorithm = \"simplec\"", TRANSIENT_SOLVER.trim());
    let text = transient.clone()
        + "[chain]\nsequence = [\"first\", \"second\"]\n[chain.stages.first]\nset = [\"solver.end_time=0.5\"]\n[chain.stages.second]\nset = [\"solver.end_time=1.0\"]\n";
    let case = Case::parse_toml(&text).unwrap();
    let (o, events) = run(&case, &dir.join("out"));
    assert_eq!(o.exit_code, 0);
    assert_eq!(o.stages.len(), 2);
    let rep = events
        .iter()
        .find(|e| e["event"] == "restart" && e["stage"] == "second")
        .expect("restart report of the second stage");
    assert_eq!(rep["report"]["restart"], "exact", "{rep}");
    let steps = |stage: &str| {
        events
            .iter()
            .rev()
            .find(|e| e["event"] == "finish" && e["stage"] == stage)
            .unwrap()["iterations"]
            .clone()
    };
    assert_eq!(steps("first"), 10);
    assert_eq!(steps("second"), 20);
    let chained = load_final(&case, &dir.join("out/second/restart.rcfd"));
    let (single, so) = drive_to_end(Simulation::new(&Case::parse_toml(&transient).unwrap()).unwrap(), None);
    assert_eq!(so.exit_code, 0, "{}", so.reason);
    assert_eq!(single.iteration(), 20);
    let (a, b) = (fields(&chained), fields(&single));
    assert!(a.0 == b.0, "ux differs");
    assert!(a.1 == b.1, "uy differs");
    assert!(a.2 == b.2, "p differs");
    assert!(a.3 == b.3, "flux differs");
    assert_eq!(a.4, b.4, "time differs");
    println!(
        "C7: chained transient (0 to 0.5, restart, 0.5 to 1.0) bit-identical to the single transient run: ux, uy, p, flux, time {}",
        single.time()
    );
}
