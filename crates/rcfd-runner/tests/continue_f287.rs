//! ADR 0200 R-1: the run settings `continue_from` and `max_steps` (`run.json` `settings`), passed by the runner to
//! `rcfd run --continue-from` and `--max-steps`. A run continued from a run of 25 steps ends, step for step, where a run of 50
//! steps ends; a run without the settings keeps its `run.json` keys and its launch; bad settings are refused at submission,
//! and a run whose source left no restart file ends before launch.
//!
//! The solver binary is `rcfd` next to `rcfd-runner` in the same target directory (`cargo test --workspace` builds it).
//!
//! ADR 0200 amendment 1: R-1 as registered asserted B's `start` event `start_iteration` 25; the event says 0 for every
//! continued run (`Simulation::iteration()` is the last step taken, which `load_restart` clears). The registered test stays
//! as an ignored record; the corrected one checks the event's 0, B's first step 26 and the iteration of A's restart file.

use rcfd_runner::client::Client;
use rcfd_runner::records::{LaunchSettings, RunSettings, RunState};
use rcfd_runner::store::RunTags;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const RUNNER: &str = env!("CARGO_BIN_EXE_rcfd-runner");

/// The 12 x 12 cavity of `runner_w47b.rs`: tolerances 1e-14, so only a cap ends a run.
const CASE: &str = r#"
title = "run-service cavity"
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
tolerance_u = 1e-14
tolerance_p = 1e-14
"#;

/// The keys of `run.json` that `Client::submit` writes (ADR 0189 C-RR).
const MAIN_KEYS: [&str; 8] = [
    "case",
    "case_dir",
    "case_snapshot_hash",
    "case_source",
    "created",
    "id",
    "settings",
    "solver_version",
];

struct Project {
    dir: PathBuf,
    client: Client,
}

impl Project {
    fn new(name: &str) -> Project {
        let rcfd = Path::new(RUNNER).with_file_name(format!("rcfd{}", std::env::consts::EXE_SUFFIX));
        assert!(rcfd.is_file(), "{} missing (cargo build -p rcfd-cli)", rcfd.display());
        let dir = std::env::temp_dir().join(format!("rcfd_runner_f287_{}_{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("cases")).unwrap();
        std::fs::create_dir_all(dir.join(".rcfd")).unwrap();
        std::fs::write(dir.join("cases/cavity.toml"), CASE).unwrap();
        let config = json!({"max_runs": 2, "cpu_threads": 8, "mem_budget_mb": 100_000, "poll_ms": 100,
            "adopt_probe_ms": 500, "cancel_grace_ms": 10_000});
        std::fs::write(dir.join(".rcfd/runner-config.json"), config.to_string()).unwrap();
        Project {
            client: Client::new(&dir, RUNNER),
            dir,
        }
    }

    fn submit(&self, launch: &LaunchSettings, mem_mb: u64) -> Result<String, String> {
        let settings = RunSettings {
            mem_mb: Some(mem_mb),
            ..RunSettings::default()
        };
        self.client.submit_with(
            &self.dir.join("cases/cavity.toml"),
            &settings,
            launch,
            &RunTags::default(),
        )
    }

    fn run_dir(&self, id: &str) -> PathBuf {
        self.dir.join("runs").join(id)
    }

    fn json(&self, id: &str, file: &str) -> Value {
        let p = self.run_dir(id).join(file);
        serde_json::from_slice(&std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))).unwrap()
    }

    fn lines(&self, id: &str, file: &str) -> Vec<Value> {
        std::fs::read_to_string(self.run_dir(id).join(file))
            .unwrap_or_default()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    fn wait_ended(&self, id: &str) -> rcfd_runner::records::RunStatus {
        self.client.wait(id, Duration::from_secs(300)).unwrap()
    }

    fn folders(&self) -> usize {
        std::fs::read_dir(self.dir.join("runs")).map_or(0, |rd| rd.count())
    }

    fn start_event(&self, id: &str) -> Value {
        self.lines(id, "solver-status.jsonl")
            .into_iter()
            .find(|v| v["event"] == "start")
            .unwrap_or_else(|| panic!("{id}: no start event"))
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = self.client.shutdown(Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(200));
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn steps(p: &Project, id: &str) -> Vec<u64> {
    p.lines(id, "residuals.jsonl")
        .iter()
        .map(|l| l["step"].as_u64().unwrap())
        .collect()
}

/// The `meta` section of a restart file, read by the container layout of `rcfd-io` (`container.rs`: `RCFDCASE`, u32 version,
/// u32 section count, then per section u32 name length, name, u64 payload length, u64 checksum, payload; little-endian).
fn restart_meta(path: &Path) -> Value {
    let b = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert_eq!(&b[..8], b"RCFDCASE", "{}", path.display());
    let u32_at = |i: usize| u32::from_le_bytes(b[i..i + 4].try_into().unwrap()) as usize;
    let u64_at = |i: usize| u64::from_le_bytes(b[i..i + 8].try_into().unwrap()) as usize;
    let (n, mut at) = (u32_at(12), 16);
    for _ in 0..n {
        let nl = u32_at(at);
        let name = std::str::from_utf8(&b[at + 4..at + 4 + nl]).unwrap().to_string();
        at += 4 + nl;
        let len = u64_at(at);
        at += 16;
        if name == "meta" {
            return serde_json::from_slice(&b[at..at + len]).unwrap();
        }
        at += len;
    }
    panic!("{}: no meta section", path.display())
}

/// ADR 0200 R-1 as registered: fails at B's `start_iteration` (0, not 25; amendment 1).
#[test]
#[ignore = "ADR 0200 amendment 1: B's start event says start_iteration 0, not 25 (Simulation::iteration() is the last step taken, cleared by load_restart); the corrected check is r1c_continue_from_and_max_steps_on_the_runner"]
fn r1_continue_from_and_max_steps_on_the_runner() {
    r1("r1", true);
}

/// ADR 0200 R-1, corrected by amendment 1.
#[test]
fn r1c_continue_from_and_max_steps_on_the_runner() {
    r1("r1c", false);
}

fn r1(name: &str, registered: bool) {
    let p = Project::new(name);
    let cap = |n: u64| LaunchSettings {
        max_steps: Some(n),
        ..LaunchSettings::default()
    };
    // refusals at submission: nothing is written
    let before = p.folders();
    for (launch, what) in [
        (
            LaunchSettings {
                continue_from: Some("../x".into()),
                ..LaunchSettings::default()
            },
            "not a run id",
        ),
        (
            LaunchSettings {
                continue_from: Some("r-20000101T000000-abcd".into()),
                ..LaunchSettings::default()
            },
            "no such run",
        ),
        (cap(0), "max_steps 0"),
    ] {
        let e = p.submit(&launch, 10).expect_err(what);
        eprintln!("R-1 refused ({what}): {e}");
        assert_eq!(p.folders(), before, "{what}: a run folder was made");
    }
    let a = p.submit(&cap(25), 10).unwrap();
    let c = p.submit(&cap(50), 10).unwrap();
    let d = p.submit(&LaunchSettings::default(), 10).unwrap();
    // a run that ends before launch (memory budget), so it leaves no restart file
    let f = p.submit(&LaunchSettings::default(), 10_000_000).unwrap();
    p.client.ensure_runner().unwrap();
    let t0 = Instant::now();
    assert_eq!(p.wait_ended(&a).state, RunState::Finished);
    let b = p
        .submit(
            &LaunchSettings {
                continue_from: Some(a.clone()),
                max_steps: Some(25),
            },
            10,
        )
        .unwrap();
    assert_eq!(p.wait_ended(&f).state, RunState::Failed);
    let e = p
        .submit(
            &LaunchSettings {
                continue_from: Some(f.clone()),
                ..LaunchSettings::default()
            },
            10,
        )
        .unwrap();
    for id in [&b, &c, &d] {
        assert_eq!(p.wait_ended(id).state, RunState::Finished, "{id}");
    }
    let st_e = p.wait_ended(&e);
    eprintln!("R-1: runs ended after {:.1} s", t0.elapsed().as_secs_f64());

    // A and C: the step cap, state step_limit
    for (id, n) in [(&a, 25u64), (&c, 50)] {
        let m = p.json(id, "metrics.json");
        assert_eq!(m["solver_finish"]["state"], "step_limit", "{id}: {m}");
        assert_eq!(m["solver_finish"]["exit_code"], 0, "{id}");
        assert_eq!(steps(&p, id), (1..=n).collect::<Vec<_>>(), "{id}");
        assert_eq!(p.json(id, "run.json")["settings"]["max_steps"], n, "{id}");
        assert_eq!(p.start_event(id)["info"]["max_steps"], n, "{id}");
    }
    // B: continued from A's restart file, steps 26 to 50, its last line C's line of step 50
    let start = p.start_event(&b);
    let restart = p.run_dir(&a).join("output").join("restart.rcfd");
    if registered {
        assert_eq!(start["start_iteration"], 25, "{start}");
    } else {
        // amendment 1: the event says 0; the restored counter is the restart file's iteration, and B's first step follows it
        assert_eq!(start["start_iteration"], 0, "{start}");
        let meta = restart_meta(&restart);
        eprintln!("R-1: A's restart file: {meta}");
        assert_eq!(meta["iteration"], 25, "{meta}");
        assert_eq!(steps(&p, &b).first(), Some(&26));
    }
    assert_eq!(
        start["info"]["continue_from"],
        json!(restart.display().to_string()),
        "{start}"
    );
    assert_eq!(p.json(&b, "run.json")["settings"]["continue_from"], json!(a));
    assert_eq!(steps(&p, &b), (26..=50).collect::<Vec<_>>());
    let last_b = p.lines(&b, "residuals.jsonl").pop().unwrap();
    let c50 = p
        .lines(&c, "residuals.jsonl")
        .into_iter()
        .find(|l| l["step"] == 50)
        .unwrap();
    eprintln!("R-1: B step 50 {last_b}\nR-1: C step 50 {c50}");
    assert_eq!(
        last_b, c50,
        "the continued run does not end where the uninterrupted run does"
    );
    assert_eq!(p.json(&b, "metrics.json")["solver_finish"]["iterations"], 50);
    // D: no launch settings: the keys of today and an unchanged launch
    let rec = p.json(&d, "run.json");
    let mut keys: Vec<&str> = rec.as_object().unwrap().keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, MAIN_KEYS, "{rec}");
    let mut sk: Vec<&str> = rec["settings"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    sk.sort_unstable();
    assert_eq!(sk, ["mem_mb", "priority", "set", "threads"], "{rec}");
    let sd = p.start_event(&d);
    assert_eq!(sd["info"]["continue_from"], Value::Null, "{sd}");
    assert!(sd["info"].get("max_steps").is_none(), "{sd}");
    // E: its source run left no restart file: failed before launch, the reason naming the run and the file
    assert_eq!(st_e.state, RunState::Failed);
    let why = st_e.reason.clone().unwrap_or_default();
    eprintln!("R-1: E {why}");
    assert!(why.contains(&f) && why.contains("restart.rcfd"), "{why}");
    let launched = p.lines(&e, "events.jsonl").iter().any(|v| v["event"] == "launched");
    assert!(!launched, "E was launched");
}
