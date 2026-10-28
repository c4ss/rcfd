//! ADR 0130 A6 (`rcfd run --store-dir`) and A7 (`rcfd run --status-monitors`): both opt-in; without the flags the
//! output is unchanged.

use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn rcfd(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rcfd")).args(args).output().unwrap()
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_w48a_cli_{}_{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Every file under `dir` (relative path -> bytes).
fn files(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else {
                let rel = p.strip_prefix(dir).unwrap().display().to_string().replace('\\', "/");
                out.insert(rel, std::fs::read(&p).unwrap());
            }
        }
    }
    out
}

/// The steady cavity of ADR 0124 H1 with the store on every 7 iterations.
const STORE_CASE: &str = r#"
title = "store-dir cavity"
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
max_iterations = 30
tolerance_u = 1e-14
tolerance_p = 1e-14
[output]
store_interval = 7
"#;

/// A6: with `--store-dir` the store is in the given folder, byte-equal to the store written to `<out>/fields` without
/// the flag, and every other output file is byte-equal. Negative control: without the flag nothing is at the folder.
#[test]
fn a6_store_dir_moves_the_store_and_changes_nothing_else() {
    let d = tmp("a6");
    let case = d.join("case.toml");
    std::fs::write(&case, STORE_CASE).unwrap();
    let case = case.display().to_string();
    let (plain, flagged, store) = (d.join("plain"), d.join("flagged"), d.join("runs-id-fields"));
    let a = rcfd(&["run", &case, "--out", &plain.display().to_string(), "--quiet"]);
    assert_eq!(a.status.code(), Some(5), "{}", String::from_utf8_lossy(&a.stderr));
    // negative control: the store written without the flag is not at the store folder
    assert!(!store.exists() && plain.join("fields/store.rts").exists());
    let b = rcfd(&[
        "run",
        &case,
        "--out",
        &flagged.display().to_string(),
        "--quiet",
        "--store-dir",
        &store.display().to_string(),
    ]);
    assert_eq!(b.status.code(), Some(5), "{}", String::from_utf8_lossy(&b.stderr));
    assert!(
        !flagged.join("fields").exists(),
        "the flag left a store in <out>/fields"
    );
    let (pa, pb, ps) = (files(&plain), files(&flagged), files(&store));
    let store_plain: BTreeMap<String, Vec<u8>> = pa
        .iter()
        .filter_map(|(k, v)| k.strip_prefix("fields/").map(|k| (k.to_string(), v.clone())))
        .collect();
    let rest_plain: BTreeMap<String, Vec<u8>> = pa.into_iter().filter(|(k, _)| !k.starts_with("fields/")).collect();
    eprintln!(
        "A6: store files {:?} ({} bytes); other output files {:?}",
        ps.keys().collect::<Vec<_>>(),
        ps.values().map(Vec::len).sum::<usize>(),
        pb.keys().collect::<Vec<_>>()
    );
    assert!(ps.contains_key("store.rts"));
    assert!(ps == store_plain, "the store at --store-dir differs from <out>/fields");
    assert!(pb == rest_plain, "an output file other than the store differs");
}

/// The `lid_driven_cavity` template of `crates/rcfd-mcp/src/lib.rs` (`LID_DRIVEN_CAVITY`), with its `lid_drag` report.
const LID_CASE: &str = r#"title = "lid-driven cavity Re=100"
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
max_iterations = 600
tolerance_u = 1e-6
tolerance_p = 1e-6
[reports.lid_drag]
type = "force"
patch = "lid"
direction = [1.0, 0.0, 0.0]
component = "viscous"
"#;

/// Status lines without their wall-clock fields (`wall_s` everywhere, `elapsed_s` of `finish`; ADR 0130 amendment 2).
fn stream(path: &Path) -> Vec<Value> {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|l| {
            let mut v: Value = serde_json::from_str(l).unwrap();
            let o = v.as_object_mut().unwrap();
            o.remove("wall_s");
            o.remove("elapsed_s");
            v
        })
        .collect()
}

/// A7: with `--status-monitors` every iteration event carries `monitors.lid_drag`, the last one bit-equal to the final
/// value of `monitors.json`; without the flag the stream equals the flagged one without `monitors`. Negative control:
/// the stream without the flag has no `monitors` key.
#[test]
fn a7_status_monitors_add_report_values_and_nothing_else() {
    let d = tmp("a7");
    let case = d.join("lid.toml");
    std::fs::write(&case, LID_CASE).unwrap();
    let case = case.display().to_string();
    let out = d.join("out").display().to_string();
    let run = |status: &str, flag: bool| {
        let s = d.join(status).display().to_string();
        let mut args = vec![
            "run",
            &case,
            "--out",
            &out,
            "--quiet",
            "--status-every",
            "1",
            "--status-file",
            &s,
        ];
        if flag {
            args.push("--status-monitors");
        }
        let o = rcfd(&args);
        assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
        let monitors: Value =
            serde_json::from_str(&std::fs::read_to_string(d.join("out/monitors.json")).unwrap()).unwrap();
        (stream(&d.join(status)), monitors)
    };
    let (plain, plain_monitors) = run("plain.jsonl", false);
    let (flagged, monitors) = run("flagged.jsonl", true);
    let iterations: Vec<&Value> = flagged.iter().filter(|e| e["event"] == "iteration").collect();
    let last = iterations.last().unwrap();
    let streamed = last["monitors"]["lid_drag"].as_f64().unwrap();
    let recorded = monitors["series"]["lid_drag"]["y"]
        .as_array()
        .unwrap()
        .last()
        .unwrap()
        .as_f64()
        .unwrap();
    eprintln!(
        "A7: {} iteration events, last index {}: monitors.lid_drag {streamed:e}, monitors.json final {recorded:e}",
        iterations.len(),
        last["index"]
    );
    assert!(iterations
        .iter()
        .all(|e| e["monitors"]["lid_drag"].as_f64().is_some_and(f64::is_finite)));
    assert_eq!(streamed.to_bits(), recorded.to_bits());
    assert_eq!(monitors, plain_monitors, "monitors.json depends on the flag");
    // negative control: no `monitors` key without the flag
    assert!(plain.iter().all(|e| e.get("monitors").is_none()));
    let stripped: Vec<Value> = flagged
        .into_iter()
        .map(|mut e| {
            e.as_object_mut().unwrap().remove("monitors");
            e
        })
        .collect();
    assert_eq!(stripped.len(), plain.len());
    assert!(stripped == plain, "the stream differs beyond `monitors`");
}
