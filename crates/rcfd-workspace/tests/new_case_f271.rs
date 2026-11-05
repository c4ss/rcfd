//! ADR 0186 C-N: the workspace command `new_case` makes a case file from each of its five sources and refuses, with
//! nothing written, what it must refuse; `start_run` with `new_case` and a malformed `set` entry writes no case and
//! queues no run. Plus the start plan's values for hand-derived cases (`rcfd_workspace::plan`, ADR 0186 G4); its parity
//! with the Studio's plan is `crates/rcfd-studio/tests/new_simulations_f271.rs`.

use rcfd_api::case::Case;
use rcfd_project::records::hash_bytes;
use rcfd_project::Project;
use rcfd_workspace::plan::{Every, Plan};
use rcfd_workspace::templates::LID_DRIVEN_CAVITY;
use rcfd_workspace::{Answer, RpcError, Workspace};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn repo_case(source: &str) -> Vec<u8> {
    std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../cases").join(source)).unwrap()
}

fn project(name: &str) -> (PathBuf, Workspace) {
    let dir = std::env::temp_dir().join(format!("rcfd-ws-f271-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    Project::create(&dir, name, "test").unwrap();
    let ws = Workspace::open(&dir).unwrap();
    (dir, ws)
}

fn call(ws: &Workspace, name: &str, args: Value) -> Result<Value, RpcError> {
    match ws.call(name, &args).expect("a workspace command") {
        Ok(Answer::Value(v)) => Ok(v),
        Ok(other) => panic!("{name}: not a value: {other:?}"),
        Err(e) => Err(e),
    }
}

/// The names under `cases/` and `runs/`, sorted.
fn listing(dir: &Path) -> Vec<String> {
    let mut v = Vec::new();
    for sub in ["cases", "runs"] {
        if let Ok(rd) = std::fs::read_dir(dir.join(sub)) {
            for e in rd {
                v.push(format!("{sub}/{}", e.unwrap().file_name().to_string_lossy()));
            }
        }
    }
    v.sort();
    v
}

#[test]
fn cn_each_source_makes_its_file() {
    let (dir, ws) = project("sources");
    // a shipped example, byte for byte
    let r = call(&ws, "new_case", json!({"name": "sod", "example": "sod/sod.toml"})).unwrap();
    let want = repo_case("sod/sod.toml");
    assert_eq!(std::fs::read(dir.join("cases/sod.toml")).unwrap(), want);
    assert_eq!(r["path"], "cases/sod.toml");
    assert_eq!(r["hash"], hash_bytes(&want));
    assert_eq!(r["bytes"], want.len());
    assert_eq!(r["from"], "example sod/sod.toml");
    assert_eq!(r["data_files"], json!([]));
    // an example with a data file: both byte for byte, the data file written next to the case
    let r = call(
        &ws,
        "new_case",
        json!({"name": "profile_inlet", "example": "channel/profile_inlet.toml"}),
    )
    .unwrap();
    assert_eq!(
        std::fs::read(dir.join("cases/profile_inlet.toml")).unwrap(),
        repo_case("channel/profile_inlet.toml")
    );
    assert_eq!(
        std::fs::read(dir.join("cases/profile_inlet.csv")).unwrap(),
        repo_case("channel/profile_inlet.csv")
    );
    assert_eq!(
        r["data_files"],
        json!([{"path": "cases/profile_inlet.csv", "written": true}])
    );
    // the built-in template
    let r = call(&ws, "new_case", json!({"name": "lid", "template": "lid_driven_cavity"})).unwrap();
    assert_eq!(
        std::fs::read(dir.join("cases/lid.toml")).unwrap(),
        LID_DRIVEN_CAVITY.as_bytes()
    );
    assert_eq!(r["from"], "template lid_driven_cavity");
    // TOML text, as given (its comment kept)
    let text = LID_DRIVEN_CAVITY.replace("nu = 0.01", "nu = 0.02 # a comment");
    let r = call(&ws, "new_case", json!({"name": "t1", "toml": text})).unwrap();
    assert_eq!(std::fs::read_to_string(dir.join("cases/t1.toml")).unwrap(), text);
    assert_eq!(r["from"], "toml");
    // a settings object, written as JSON whose tree is the object
    let small = json!({
        "mesh": {"generator": "cavity_2d", "nx": 8, "ny": 8},
        "physics": {"nu": 0.03},
        "boundaries": {"lid": {"velocity": "fixed", "velocity_value": [1.0, 0.0, 0.0]}, "walls": {"velocity": "fixed"}},
        "solver": {"algorithm": "simplec", "max_iterations": 50}
    });
    let r = call(&ws, "new_case", json!({"name": "s1", "settings": small})).unwrap();
    assert_eq!(r["path"], "cases/s1.json");
    let written: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("cases/s1.json")).unwrap()).unwrap();
    assert_eq!(written, small);
    // a copy of a project case, byte for byte
    let r = call(&ws, "new_case", json!({"name": "sod-copy", "copy": "cases/sod.toml"})).unwrap();
    assert_eq!(std::fs::read(dir.join("cases/sod-copy.toml")).unwrap(), want);
    assert_eq!(r["from"], "copy cases/sod.toml");
    // no journal line for a creation (ADR 0186 G3)
    assert!(!dir.join(".rcfd/edits.jsonl").exists());
    eprintln!("C-N sources: {:?}", listing(&dir));
}

#[test]
fn cn_refusals_write_nothing() {
    let (dir, ws) = project("refusals");
    call(&ws, "new_case", json!({"name": "sod", "example": "sod/sod.toml"})).unwrap();
    std::fs::write(dir.join("cases/broken.toml"), "[mesh\n").unwrap();
    let cases: &[(&str, Value, i64)] = &[
        (
            "invalid TOML",
            json!({"name": "a", "toml": "title = \"x\"\n[mesh\n"}),
            1003,
        ),
        (
            "invalid value",
            json!({"name": "b", "toml": LID_DRIVEN_CAVITY.replace("nu = 0.01", "nu = \"thick\"")}),
            1003,
        ),
        (
            "existing name, other case",
            json!({"name": "SOD", "example": "sod/sod.toml"}),
            1004,
        ),
        (
            "a space",
            json!({"name": "my case", "template": "lid_driven_cavity"}),
            1004,
        ),
        (
            "two sources",
            json!({"name": "c", "template": "lid_driven_cavity", "toml": LID_DRIVEN_CAVITY}),
            -32602,
        ),
        ("no source", json!({"name": "c"}), -32602),
        (
            "unknown example",
            json!({"name": "c", "example": "nope/nope.toml"}),
            -32602,
        ),
        ("missing copy", json!({"name": "c", "copy": "cases/missing.toml"}), 1202),
        ("invalid copy", json!({"name": "c", "copy": "cases/broken.toml"}), 1003),
        (
            "path out of the project",
            json!({"name": "c", "copy": "../x.toml"}),
            1004,
        ),
    ];
    let before = listing(&dir);
    for (what, args, code) in cases {
        let e = call(&ws, "new_case", args.clone()).expect_err(what);
        eprintln!("C-N {what}: {} {}", e.code, e.message);
        assert_eq!(e.code, *code, "{what}: {e:?}");
        assert_eq!(listing(&dir), before, "{what}: something was written");
    }
    // the located diagnostics of an invalid TOML text name its line
    let e = call(&ws, "new_case", cases[0].1.clone()).unwrap_err();
    let line = e.data.as_ref().unwrap()["diagnostics"][0]["line"].as_u64();
    assert_eq!(line, Some(2), "{e:?}");
    // the invalid value is located at its setting
    let e = call(&ws, "new_case", cases[1].1.clone()).unwrap_err();
    assert!(e.message.contains("physics.nu"), "{e:?}");
    assert_eq!(
        std::fs::read(dir.join("cases/sod.toml")).unwrap(),
        repo_case("sod/sod.toml")
    );
}

#[test]
fn cn_start_run_with_a_malformed_set_entry_writes_no_case_and_queues_no_run() {
    // no runner is needed: the refusal comes before anything is written or queued
    let (dir, ws) = project("malformed");
    let ws = ws.with_runner_exe(dir.join("no-runner-here"));
    let before = listing(&dir);
    for set in [json!(["solver.nosuch=1"]), json!(["solver.max_iterations=1 2"])] {
        let e = call(
            &ws,
            "start_run",
            json!({"new_case": {"name": "m", "example": "sod/sod.toml"}, "set": set}),
        )
        .unwrap_err();
        eprintln!("C-N malformed set {set}: {} {}", e.code, e.message);
        assert_eq!(e.code, 1003, "{e:?}");
        assert_eq!(listing(&dir), before);
    }
    // control: the same call without the entry gets past the plan and writes the case (the run then cannot start here)
    let r = call(
        &ws,
        "start_run",
        json!({"new_case": {"name": "m", "example": "sod/sod.toml"}}),
    );
    assert!(dir.join("cases/m.toml").is_file(), "{r:?}");
    // case and new_case together, or neither: refused
    for args in [
        json!({}),
        json!({"case": "cases/m.toml", "new_case": {"name": "n", "example": "sod/sod.toml"}}),
    ] {
        assert_eq!(call(&ws, "start_run", args).unwrap_err().code, -32602);
    }
    assert_eq!(
        call(&ws, "start_run", json!({"case": "cases/m.toml", "store": "some"}))
            .unwrap_err()
            .code,
        -32602
    );
}

fn cavity(alg: &str, extra: &str) -> String {
    format!(
        "[mesh]\ngenerator = \"cavity_2d\"\nnx = 12\nny = 12\n[physics]\nnu = 0.01\n[boundaries.lid]\nvelocity = \"fixed\"\nvelocity_value = [1.0, 0.0, 0.0]\n[boundaries.walls]\nvelocity = \"fixed\"\n[solver]\nalgorithm = \"{alg}\"\n{extra}"
    )
}

fn plan(text: &str) -> Plan {
    Plan::of(&Case::parse_toml(text).unwrap())
}

/// G4's rule on hand-derived cases.
#[test]
fn plan_values() {
    let p = plan(&cavity("simplec", "max_iterations = 30\n"));
    assert_eq!(p.every, Some(Every::Steps(1)));
    assert_eq!(p.entries(), ["output.store_interval=1", "output.store_precision=f32"]);
    assert_eq!((p.snapshots(), p.estimated_bytes()), (Some(30), Some(144 * 4 * 30 * 4)));
    let p = plan(&cavity("simplec", "max_iterations = 250\n"));
    assert_eq!((p.every, p.snapshots()), (Some(Every::Steps(3)), Some(84)));
    let p = plan(&cavity("piso", "max_iterations = 400\nend_time = 0.5\ndt = 0.01\n"));
    assert_eq!(
        p.entries(),
        ["output.store_time_interval=0.005", "output.store_precision=f32"]
    );
    assert_eq!(p.snapshots(), Some(100));
    // piso without end_time: the schema default end_time (1 s) is planned for
    assert_eq!(Case::new().get("solver.end_time").unwrap(), json!(1.0));
    let p = plan(&cavity("piso", "max_iterations = 400\ndt = 0.01\n"));
    assert_eq!(p.every, Some(Every::Time(0.01)));
    // a steady algorithm ignores end_time: the step rule
    let p = plan(&cavity("simplec", "max_iterations = 400\nend_time = 0.5\n"));
    assert_eq!(p.every, Some(Every::Steps(4)));
    // the case's own store: no plan, and the note says why
    let p = plan(&cavity(
        "simplec",
        "max_iterations = 30\n[output]\nstore_interval = 5\n",
    ));
    assert!(p.entries().is_empty() && p.snapshots().is_none());
    assert_eq!(
        p.note.as_deref(),
        Some("the case sets its own field store (output.store_*)")
    );
    // at its schema default the key is not the case's own (ADR 0152)
    let p = plan(&cavity(
        "simplec",
        "max_iterations = 30\n[output]\nstore_interval = 0\n",
    ));
    assert_eq!(p.every, Some(Every::Steps(1)));
    // the case's own precision: no precision entry
    let p = plan(&cavity(
        "simplec",
        "max_iterations = 30\n[output]\nstore_precision = \"f32\"\n",
    ));
    assert_eq!(p.entries(), ["output.store_interval=1"]);
}
