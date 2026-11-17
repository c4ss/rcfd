//! ADR 0734 (F-828 E-828a, E-828d, E-828e): the launch record, the launch options and rerun of `rcfd-runner`, and the
//! mesh hash at submit.
//!
//! - L1: a launch writes `runs/<id>/launch.json` (argv, cwd, solver path, version and build) before the spawn; that argv,
//!   its run-folder paths moved to a copy of the folder, run by hand gives byte-identical `fields/store.rts`, `output/`
//!   files and residual lines.
//! - L3: `rcfd-runner start --continue-from <id> --max-steps 3` writes the `run.json` `settings` `Client::submit_with`
//!   writes; a bad id or 0 is refused with nothing written.
//! - L4: `rcfd-runner rerun` of a finished run gives equal stored fields and residuals, `rerun_of` set, `case` and
//!   `case_dir` equal, and the overrides given.
//! - L6 (the runner part): a generator run's `run.json` has the keys of before ADR 0734 (no `mesh`, `mesh_hash` or
//!   `rerun_of`) and its `events.jsonl` the kinds of before; a file-mesh run's `run.json` gains `mesh` and `mesh_hash`.
//!
//! The solver binary is `rcfd` next to `rcfd-runner` in the same target directory (`cargo build -p rcfd-cli`).

use rcfd_runner::client::Client;
use rcfd_runner::records::{LaunchSettings, RunSettings, RunState};
use rcfd_runner::store::RunTags;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

const RUNNER: &str = env!("CARGO_BIN_EXE_rcfd-runner");

/// A 10 x 10 cavity of 30 steps (tolerances 1e-14: only the cap ends it) that stores its fields every 5 steps.
const CASE: &str = r#"
title = "adr0734 cavity"
[mesh]
generator = "cavity_2d"
nx = 10
ny = 10
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
max_iterations = 30
tolerance_u = 1e-14
tolerance_p = 1e-14
[output]
store_interval = 5
"#;

/// The keys of `run.json` that `Client::submit` wrote before ADR 0734 (ADR 0189 C-RR), still the keys of a generator run.
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
        let rcfd = solver();
        assert!(rcfd.is_file(), "{} missing (cargo build -p rcfd-cli)", rcfd.display());
        let dir = std::env::temp_dir().join(format!("rcfd_runner_adr0734_{}_{name}", std::process::id()));
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

    fn run_dir(&self, id: &str) -> PathBuf {
        self.dir.join("runs").join(id)
    }

    fn json(&self, id: &str, file: &str) -> Value {
        let p = self.run_dir(id).join(file);
        serde_json::from_slice(&std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))).unwrap()
    }

    fn lines(&self, id: &str, file: &str) -> Vec<Value> {
        lines(&self.run_dir(id).join(file))
    }

    fn folders(&self) -> usize {
        std::fs::read_dir(self.dir.join("runs")).map_or(0, |rd| rd.count())
    }

    /// `rcfd-runner <args> --project <dir>`: (exit code, standard output, standard error).
    fn cli(&self, args: &[&str]) -> (i32, String, String) {
        let o = Command::new(RUNNER)
            .args(args)
            .arg("--project")
            .arg(&self.dir)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        (
            o.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&o.stdout).trim().to_string(),
            String::from_utf8_lossy(&o.stderr).trim().to_string(),
        )
    }

    fn wait_finished(&self, id: &str) {
        let st = self.client.wait(id, Duration::from_secs(300)).unwrap();
        assert_eq!(st.state, RunState::Finished, "{id}: {st:?}");
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = self.client.shutdown(Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(200));
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn solver() -> PathBuf {
    Path::new(RUNNER).with_file_name(format!("rcfd{}", std::env::consts::EXE_SUFFIX))
}

fn lines(p: &Path) -> Vec<Value> {
    std::fs::read_to_string(p)
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

/// Sorted relative paths and bytes of every file under `dir`.
fn files(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                let rel = p.strip_prefix(dir).unwrap().to_string_lossy().replace('\\', "/");
                out.push((rel, std::fs::read(&p).unwrap()));
            }
        }
    }
    out.sort();
    out
}

/// The residual lines of a solver status stream, as the runner forms them (`rcfd_runner::runner::residual_line`).
fn stream_residuals(stream: &Path) -> Vec<Value> {
    lines(stream)
        .iter()
        .filter(|v| v["event"] == "iteration")
        .map(|v| rcfd_runner::runner::residual_line(v, v["index"].as_u64().unwrap()))
        .collect()
}

fn sorted_keys(v: &Value) -> Vec<String> {
    let mut k: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
    k.sort();
    k
}

/// L1, L4, L6 (generator part).
#[test]
fn l1_l4_l6_launch_record_replay_and_rerun() {
    let p = Project::new("l1");
    let settings = RunSettings {
        mem_mb: Some(10),
        ..RunSettings::default()
    };
    let a = p.client.submit(&p.dir.join("cases/cavity.toml"), &settings).unwrap();
    // L6: a generator run's run.json has the keys of before, and no launch record before its launch
    let rec_a = p.json(&a, "run.json");
    assert_eq!(sorted_keys(&rec_a), MAIN_KEYS, "{rec_a}");
    assert_eq!(p.client.launch(&a).unwrap(), None);
    p.client.ensure_runner().unwrap();
    p.wait_finished(&a);

    // L1: the launch record
    let launch = p.json(&a, "launch.json");
    eprintln!("L1: launch.json {launch}");
    assert_eq!(sorted_keys(&launch), ["argv", "cwd", "launch_seq", "solver", "started"]);
    let argv: Vec<String> = launch["argv"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a.as_str().unwrap().to_string())
        .collect();
    let dir_a = p.run_dir(&a).display().to_string();
    assert_eq!(argv[0], solver().display().to_string());
    assert_eq!(argv[1], "run");
    assert_eq!(argv[2], p.run_dir(&a).join("case.snapshot").display().to_string());
    for flag in [
        "--out",
        "--status-file",
        "--store-dir",
        "--stop-file",
        "--pause-file",
        "--status-monitors",
    ] {
        assert!(argv.iter().any(|x| x == flag), "{flag} missing from {argv:?}");
    }
    assert_eq!(launch["cwd"], json!(dir_a));
    assert_eq!(launch["launch_seq"], 1);
    let s = &launch["solver"];
    assert_eq!(s["path"], json!(argv[0]));
    assert_eq!(s["version"], json!(format!("rcfd {}", env!("CARGO_PKG_VERSION"))));
    assert_eq!(s["version"], rec_a["solver_version"]);
    let exe = std::fs::read(solver()).unwrap();
    assert_eq!(s["build"]["exe_hash"], json!(rcfd_project::records::hash_bytes(&exe)));
    assert_eq!(
        s["build"]["git_rev"],
        json!(option_env!("RCFD_GIT_REV").filter(|r| !r.is_empty()))
    );
    assert_eq!(p.client.launch(&a).unwrap(), Some(launch.clone()));
    // the launched event is as before: pid, launch_seq, threads and mem_mb
    let ev = p.lines(&a, "events.jsonl");
    let launched = ev.iter().find(|v| v["kind"] == "launched").unwrap();
    let mut lk = sorted_keys(launched);
    lk.retain(|k| !["kind", "event", "t"].contains(&k.as_str()));
    assert_eq!(lk, ["launch_seq", "mem_mb", "pid", "threads"], "{launched}");
    // the argv by hand, in a copy of the run folder
    let copy = p.dir.join("replay");
    std::fs::create_dir_all(&copy).unwrap();
    std::fs::copy(p.run_dir(&a).join("case.snapshot"), copy.join("case.snapshot")).unwrap();
    let moved: Vec<String> = argv[1..]
        .iter()
        .map(|x| match x.strip_prefix(&dir_a) {
            Some(rest) => format!("{}{rest}", copy.display()),
            None => x.clone(),
        })
        .collect();
    let st = Command::new(&argv[0])
        .args(&moved)
        .current_dir(&copy)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap();
    // the same exit as the runner's solver (5: not converged within the cap of 30 steps)
    assert_eq!(
        st.code(),
        p.client.status(&a).unwrap().exit_code,
        "the replayed argv exited {st:?}"
    );
    let store_a = std::fs::read(p.run_dir(&a).join("fields/store.rts")).unwrap();
    assert!(!store_a.is_empty());
    assert_eq!(
        std::fs::read(copy.join("fields/store.rts")).unwrap(),
        store_a,
        "store.rts differs"
    );
    let (out_a, out_c) = (files(&p.run_dir(&a).join("output")), files(&copy.join("output")));
    assert_eq!(
        out_a.iter().map(|f| &f.0).collect::<Vec<_>>(),
        out_c.iter().map(|f| &f.0).collect::<Vec<_>>()
    );
    assert_eq!(out_a, out_c, "output/ differs");
    let res_a = p.lines(&a, "residuals.jsonl");
    assert_eq!(res_a.len(), 30);
    assert_eq!(
        stream_residuals(&copy.join("solver-status.jsonl")),
        res_a,
        "residuals differ"
    );
    eprintln!(
        "L1: replayed argv of {} elements: store.rts {} bytes, {} output files, {} residual lines equal",
        argv.len(),
        store_a.len(),
        out_a.len(),
        res_a.len()
    );

    // L4: rerun through the CLI, the priority overridden
    let (code, id, err) = p.cli(&["rerun", &a, "--priority", "3", "--no-runner"]);
    assert_eq!(code, 0, "{err}");
    let b = id;
    p.wait_finished(&b);
    let rec_b = p.json(&b, "run.json");
    eprintln!("L4: {rec_b}");
    assert_eq!(rec_b["rerun_of"], json!(a));
    for k in [
        "case",
        "case_dir",
        "case_source",
        "case_snapshot_hash",
        "solver_version",
    ] {
        assert_eq!(rec_b[k], rec_a[k], "{k}");
    }
    assert_eq!(rec_b["settings"]["priority"], 3);
    assert_eq!(rec_b["settings"]["threads"], 1);
    assert_eq!(rec_b["settings"]["mem_mb"], 10);
    let mut want = MAIN_KEYS.map(String::from).to_vec();
    want.push("rerun_of".into());
    want.sort();
    assert_eq!(sorted_keys(&rec_b), want);
    assert_eq!(
        std::fs::read(p.run_dir(&b).join("fields/store.rts")).unwrap(),
        store_a,
        "the rerun's store.rts differs"
    );
    assert_eq!(p.lines(&b, "residuals.jsonl"), res_a, "the rerun's residuals differ");
    // a rerun of what is not a run of the project: refused, nothing written
    let before = p.folders();
    for bad in ["r-20000101T000000-abcd", "../x"] {
        let (code, _, err) = p.cli(&["rerun", bad, "--no-runner"]);
        eprintln!("L4: rerun {bad}: exit {code}: {err}");
        assert_eq!(code, 1);
    }
    assert_eq!(p.folders(), before);
    // L1: status prints the launch record
    let (code, out, err) = p.cli(&["status", &a]);
    assert_eq!(code, 0, "{err}");
    let st: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(st["state"], "finished");
    assert_eq!(st["launch"], launch);
}

/// L3: the start options and their refusals; L6 (file-mesh part): `mesh` and `mesh_hash` at submit.
#[test]
fn l3_start_options_and_l6_mesh_hash_at_submit() {
    let p = Project::new("l3");
    let src = p
        .client
        .submit(&p.dir.join("cases/cavity.toml"), &RunSettings::default())
        .unwrap();
    let case = p.dir.join("cases/cavity.toml").display().to_string();
    let (code, id, err) = p.cli(&[
        "start",
        &case,
        "--continue-from",
        &src,
        "--max-steps",
        "3",
        "--no-runner",
    ]);
    assert_eq!(code, 0, "{err}");
    let by_cli = p.json(&id, "run.json")["settings"].clone();
    let lib = p
        .client
        .submit_with(
            &p.dir.join("cases/cavity.toml"),
            &RunSettings::default(),
            &LaunchSettings {
                continue_from: Some(src.clone()),
                max_steps: Some(3),
            },
            &RunTags::default(),
        )
        .unwrap();
    assert_eq!(by_cli, p.json(&lib, "run.json")["settings"]);
    assert_eq!(by_cli["continue_from"], json!(src));
    assert_eq!(by_cli["max_steps"], 3);
    let before = p.folders();
    for (args, what) in [
        (vec!["--continue-from", "r-20000101T000000-abcd"], "no such run"),
        (vec!["--continue-from", "../x"], "not a run id"),
        (vec!["--max-steps", "0"], "max_steps 0"),
        (vec!["--max-steps", "x"], "not a number"),
    ] {
        let mut a = vec!["start", case.as_str()];
        a.extend(args);
        a.push("--no-runner");
        let (code, _, err) = p.cli(&a);
        eprintln!("L3 refused ({what}): exit {code}: {err}");
        assert_eq!(code, 1, "{what}");
    }
    assert_eq!(p.folders(), before, "a refused start wrote a run");

    // L6: a file-mesh case records its mesh and the mesh's hash (a .msh file and an OpenFOAM folder)
    std::fs::create_dir_all(p.dir.join("meshes/foam/polyMesh")).unwrap();
    std::fs::write(p.dir.join("meshes/a.msh"), b"$MeshFormat\n2.2 0 8\n$EndMeshFormat\n").unwrap();
    std::fs::write(p.dir.join("meshes/foam/polyMesh/points"), b"()").unwrap();
    std::fs::write(
        p.dir.join("cases/file.toml"),
        "[mesh]\nsource = \"file\"\nfile = \"../meshes/a.msh\"\n",
    )
    .unwrap();
    let f = p
        .client
        .submit(&p.dir.join("cases/file.toml"), &RunSettings::default())
        .unwrap();
    let rec = p.json(&f, "run.json");
    assert_eq!(rec["mesh"], "meshes/a.msh", "{rec}");
    let bytes = std::fs::read(p.dir.join("meshes/a.msh")).unwrap();
    assert_eq!(rec["mesh_hash"], json!(rcfd_project::records::hash_bytes(&bytes)));
    let settings = RunSettings {
        set: vec!["mesh.file=\"../meshes/foam\"".into()],
        ..RunSettings::default()
    };
    let g = p.client.submit(&p.dir.join("cases/file.toml"), &settings).unwrap();
    let rec = p.json(&g, "run.json");
    assert_eq!(rec["mesh"], "meshes/foam", "{rec}");
    let want = rcfd_project::filehash::hash_path(&p.dir.join("meshes/foam")).unwrap();
    assert_eq!((rec["mesh_hash"].clone(), want.kind), (json!(want.hash), "folder"));
    // a mesh file that is not there: the path, no hash (the solver's check reports it)
    std::fs::write(
        p.dir.join("cases/gone.toml"),
        "[mesh]\nsource = \"file\"\nfile = \"none.msh\"\n",
    )
    .unwrap();
    let h = p
        .client
        .submit(&p.dir.join("cases/gone.toml"), &RunSettings::default())
        .unwrap();
    let rec = p.json(&h, "run.json");
    assert_eq!(
        rec["mesh"],
        json!(p.dir.join("cases").join("none.msh").display().to_string()),
        "{rec}"
    );
    assert!(rec.get("mesh_hash").is_none(), "{rec}");
    eprintln!(
        "L6: file-mesh runs record {} and {}",
        p.json(&f, "run.json")["mesh_hash"],
        p.json(&g, "run.json")["mesh_hash"]
    );
}
