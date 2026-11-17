//! ADR 0733 (F-837): the runner writes its own budgets file. R1 `config set` then `Config::load`, R2 refusals leave the
//! file byte-identical, R3 a stale base is exit 4, R4 a runner started after `config set` records the written values, R5
//! one journal line per accepted write; plus Q4 (a file the runner cannot read needs `--replace`) and the read
//! (`rcfd-runner config`). Every test has its own folder; the one runner (R4) is shut down by its request file.

use rcfd_runner::config::{Base, ConfigError, SetOptions, CONFIG_KEYS};
use rcfd_runner::runner::{config_path, Config};
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

const RUNNER: &str = env!("CARGO_BIN_EXE_rcfd-runner");

fn folder(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rcfd_runner_adr0733_{}_{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn cli(dir: &Path, args: &[&str]) -> Output {
    Command::new(RUNNER)
        .args(args)
        .arg("--project")
        .arg(dir)
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

fn json_out(o: &Output) -> Value {
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    serde_json::from_slice(&o.stdout).unwrap()
}

fn journal(dir: &Path) -> Vec<Value> {
    std::fs::read_to_string(dir.join(".rcfd/edits.jsonl"))
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

/// R1: the values set are what `Config::load` reads; `--unset` gives the default for that key.
#[test]
fn r1_set_then_load_gives_the_values_and_unset_the_default() {
    let dir = folder("r1");
    let st = json_out(&cli(&dir, &["config"]));
    assert_eq!(st["exists"], false);
    assert_eq!(st["hash"], Value::Null);
    assert_eq!(st["applies"], "next_start");
    let st = json_out(&cli(
        &dir,
        &["config", "set", "max_runs=3", "mem_base_mb=12.5", "poll_ms=50"],
    ));
    assert_eq!(st["values"], json!({"max_runs": 3, "mem_base_mb": 12.5, "poll_ms": 50}));
    assert!(st["hash"].as_str().unwrap().starts_with("fnv64:"));
    let c = Config::load(&dir).unwrap();
    let d = Config::default();
    assert_eq!((c.max_runs, c.mem_base_mb, c.poll_ms), (3, 12.5, 50));
    assert_eq!(c.cpu_threads, d.cpu_threads);
    let base = st["hash"].as_str().unwrap().to_string();
    let st = json_out(&cli(&dir, &["config", "set", "--unset", "poll_ms", "--base", &base]));
    assert_eq!(st["values"], json!({"max_runs": 3, "mem_base_mb": 12.5}));
    assert_eq!(Config::load(&dir).unwrap().poll_ms, d.poll_ms);
    // the file: the runner's key order, two-space indent, a final newline
    assert_eq!(
        std::fs::read_to_string(config_path(&dir)).unwrap(),
        "{\n  \"max_runs\": 3,\n  \"mem_base_mb\": 12.5\n}\n"
    );
    // every key unset: an empty object (the runner's defaults), the file stays
    let st = json_out(&cli(
        &dir,
        &["config", "set", "--unset", "max_runs", "--unset", "mem_base_mb"],
    ));
    assert_eq!(st["values"], json!({}));
    assert_eq!(std::fs::read_to_string(config_path(&dir)).unwrap(), "{}\n");
    assert_eq!(Config::load(&dir).unwrap(), d);
    eprintln!("R1: set, load, unset and the empty file as Config::load reads them");
}

/// R2: each refused value or key leaves the file byte-identical and gives the runner's message (exit 1).
#[test]
fn r2_refusals_write_nothing_and_give_the_runners_message() {
    let dir = folder("r2");
    json_out(&cli(&dir, &["config", "set", "max_runs=2"]));
    let before = std::fs::read(config_path(&dir)).unwrap();
    let lines = journal(&dir).len();
    let cases: [(&str, &str); 7] = [
        ("max_runs=0", "must be positive"),
        ("cpu_threads=0", "must be positive"),
        ("poll_ms=0", "must be positive"),
        ("mem_budget_mb=-1", "expected u64"),
        ("cpu_threads=1.5", "expected usize"),
        ("gpu=1", "unknown field `gpu`"),
        ("max_runs=two", "needs a number"),
    ];
    for (arg, want) in cases {
        let o = cli(&dir, &["config", "set", arg]);
        let err = String::from_utf8_lossy(&o.stderr);
        assert_eq!(o.status.code(), Some(1), "{arg}: {err}");
        assert!(err.contains(want), "{arg}: {err}");
        assert_eq!(
            std::fs::read(config_path(&dir)).unwrap(),
            before,
            "{arg} changed the file"
        );
    }
    assert_eq!(journal(&dir).len(), lines, "a refusal left a journal line");
    // the library locates a refusal at its key
    let set: Map<String, Value> = [("poll_ms".to_string(), json!(0))].into_iter().collect();
    match Config::set(&dir, &set, Base::Any, SetOptions::default()) {
        Err(ConfigError::Refused { key, .. }) => assert_eq!(key.as_deref(), Some("poll_ms")),
        other => panic!("{other:?}"),
    }
    eprintln!("R2: {} refusals, file and journal unchanged", cases.len());
}

/// R3: a stale `--base` is exit 4 and writes nothing; Q4: a file the runner cannot read is refused unless replaced whole.
#[test]
fn r3_stale_base_is_exit_4_and_q4_unreadable_needs_replace() {
    let dir = folder("r3");
    let st = json_out(&cli(&dir, &["config", "set", "max_runs=2"]));
    let old = st["hash"].as_str().unwrap().to_string();
    json_out(&cli(&dir, &["config", "set", "max_runs=3", "--base", &old]));
    let before = std::fs::read(config_path(&dir)).unwrap();
    let o = cli(&dir, &["config", "set", "max_runs=4", "--base", &old]);
    let err = String::from_utf8_lossy(&o.stderr);
    assert_eq!(o.status.code(), Some(4), "{err}");
    assert!(err.contains("changed since it was read"), "{err}");
    assert_eq!(std::fs::read(config_path(&dir)).unwrap(), before);
    // no file expected, but there is one: a conflict naming the current hash
    let set: Map<String, Value> = [("max_runs".to_string(), json!(5))].into_iter().collect();
    match Config::set(&dir, &set, Base::Absent, SetOptions::default()) {
        Err(ConfigError::Conflict { current_hash }) => {
            assert_eq!(current_hash, Some(rcfd_project::records::hash_bytes(&before)))
        }
        other => panic!("{other:?}"),
    }
    // Q4 (a): unknown key in the file
    std::fs::write(config_path(&dir), "{\"gpu\": 1}\n").unwrap();
    let st = json_out(&cli(&dir, &["config"]));
    assert_eq!(st["needs_replace"], true);
    assert!(st["problem"].as_str().unwrap().contains("unknown field `gpu`"));
    let o = cli(&dir, &["config", "set", "max_runs=2"]);
    assert_eq!(o.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&o.stderr).contains("cannot read the file as it is"));
    // --replace without every key: refused, naming the first missing one
    let o = cli(&dir, &["config", "set", "max_runs=2", "--replace"]);
    assert_eq!(o.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&o.stderr).contains("cpu_threads is missing"));
    assert_eq!(std::fs::read_to_string(config_path(&dir)).unwrap(), "{\"gpu\": 1}\n");
    let mut args = vec!["config", "set", "max_runs=2", "--replace"];
    for k in &CONFIG_KEYS[1..] {
        args.extend(["--unset", k]);
    }
    let st = json_out(&cli(&dir, &args));
    assert_eq!(st["values"], json!({"max_runs": 2}));
    assert_eq!(st["problem"], Value::Null);
    // a file that is not JSON: the same
    std::fs::write(config_path(&dir), "{").unwrap();
    assert_eq!(cli(&dir, &["config", "set", "max_runs=2"]).status.code(), Some(1));
    // a readable file with a value the runner refuses is fixed by a set of that key, without replace
    std::fs::write(config_path(&dir), "{\"max_runs\": 0}\n").unwrap();
    let st = json_out(&cli(&dir, &["config"]));
    assert_eq!(
        (st["needs_replace"].clone(), st["problem"].is_string()),
        (json!(false), true)
    );
    json_out(&cli(&dir, &["config", "set", "max_runs=1"]));
    eprintln!("R3: stale base exit 4; Q4: unreadable files need --replace with every key");
}

/// R4: a runner started after `config set` records the written values in `runner.json`; `config` shows them in use.
#[test]
fn r4_runner_started_after_set_uses_the_written_values() {
    let dir = folder("r4");
    json_out(&cli(
        &dir,
        &["config", "set", "max_runs=3", "cpu_threads=2", "poll_ms=20"],
    ));
    let mut child = Command::new(RUNNER)
        .args(["serve", "--project"])
        .arg(&dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let rec = dir.join(".rcfd/runner.json");
    let t0 = Instant::now();
    let info: Value = loop {
        if let Some(v) = std::fs::read(&rec)
            .ok()
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        {
            break v;
        }
        assert!(t0.elapsed() < Duration::from_secs(30), "no runner.json");
        std::thread::sleep(Duration::from_millis(20));
    };
    let st = json_out(&cli(&dir, &["config"]));
    std::fs::write(dir.join(".rcfd/shutdown"), b"").unwrap();
    let t0 = Instant::now();
    while child.try_wait().unwrap().is_none() {
        assert!(t0.elapsed() < Duration::from_secs(30), "the runner did not shut down");
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(info["config"]["max_runs"], 3);
    assert_eq!(info["config"]["cpu_threads"], 2);
    assert_eq!(info["config"]["poll_ms"], 20);
    assert_eq!(info["config_source"], config_path(&dir).display().to_string());
    assert_eq!(st["in_use"]["values"]["max_runs"], 3);
    assert_eq!(st["in_use"]["source"], info["config_source"]);
    eprintln!("R4: runner.json records the written values");
}

/// R5: each accepted write leaves one journal line naming the command and each key's old and new value.
#[test]
fn r5_one_journal_line_per_accepted_write() {
    let dir = folder("r5");
    json_out(&cli(&dir, &["config", "set", "max_runs=2"]));
    json_out(&cli(&dir, &["config", "set", "max_runs=3", "--unset", "poll_ms"]));
    let j = journal(&dir);
    assert_eq!(j.len(), 2);
    for l in &j {
        assert_eq!(l["kind"], "edit");
        assert_eq!(l["who"], "rcfd-runner config set");
        assert_eq!(l["path"], ".rcfd/runner-config.json");
    }
    assert_eq!(j[0]["old_hash"], Value::Null);
    assert_eq!(j[1]["old_hash"], j[0]["new_hash"]);
    assert_eq!(
        j[1]["changes"],
        json!([{"path": "max_runs", "old": 2, "new": 3}, {"path": "poll_ms", "old": null, "new": null}])
    );
    let help = Command::new(RUNNER).arg("--help").output().unwrap();
    let help = String::from_utf8_lossy(&help.stdout);
    assert!(help.contains("config set <key>=<number>"), "{help}");
    eprintln!("R5: {} journal lines", j.len());
}
