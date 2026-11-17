//! ADR 0736 P1 (F-829 E-829a): the project trash tools `trash_run`, `trash_results`, `trash_case`, `list_trash`,
//! `restore_trash` and `empty_trash`, on run folders made with `rcfd-project` (no runner). T1 trash and restore give the
//! same files and the project validates before, between and after; T2 a run that has not ended is 1002, a run an
//! unfinished run continues from is 1003 naming it, allowed once that run has ended (Q2 (a)); T3 a stale case base is
//! 1200 and moves nothing, a run of the case keeps its snapshot; T4 a restore onto an occupied path is 1200 with
//! `data.path` and moves nothing; T5 one journal line and one feed entry per accepted call, `--read-only` 1100;
//! E `empty_trash` is the class `delete` and removes for good; A `empty_trash` is the one delete-class tool (Q3 (a), restated). The
//! tool lists are checked by `project_list_f270.rs`, `tool_list_f275.rs` and `project_w48f.rs`, restated there.

mod common;

use common::*;
use rcfd_mcp::policy::{Access, Policy};
use rcfd_project::records::{hash_bytes, RunState, RunStatus};
use rcfd_project::Project;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

/// Every file under `dir` (but `.rcfd/`) with its hash.
fn tree(dir: &Path) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            let rel = p.strip_prefix(dir).unwrap().to_string_lossy().replace('\\', "/");
            if rel == ".rcfd" {
                continue;
            }
            if p.is_dir() {
                stack.push(p);
            } else {
                out.insert(rel, hash_bytes(&std::fs::read(&p).unwrap()));
            }
        }
    }
    out
}

fn journal(dir: &Path) -> Vec<Value> {
    std::fs::read_to_string(dir.join(".rcfd/edits.jsonl"))
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

/// A run of `cases/cavity.toml` in `state`, with `settings`, a `fields/` and an `output/` folder.
fn run(dir: &Path, state: RunState, settings: Value) -> String {
    let p = Project::open(dir).unwrap();
    let rec = p.create_run("cases/cavity.toml", None, "test", settings).unwrap();
    set_state(dir, &rec.id, state);
    let d = dir.join("runs").join(&rec.id);
    std::fs::create_dir_all(d.join("fields")).unwrap();
    std::fs::write(d.join("fields/store.rts"), b"field bytes").unwrap();
    std::fs::create_dir_all(d.join("output")).unwrap();
    std::fs::write(d.join("output/restart.rcfd"), b"restart bytes").unwrap();
    std::fs::write(d.join("residuals.jsonl"), "{\"step\":1}\n").unwrap();
    rec.id
}

fn set_state(dir: &Path, id: &str, state: RunState) {
    Project::open(dir)
        .unwrap()
        .write_status(id, &RunStatus::new(state))
        .unwrap();
}

fn valid(dir: &Path) {
    let issues = Project::open(dir).unwrap().validate().unwrap();
    assert!(issues.is_empty(), "{issues:?}");
}

fn trash_folder_is_empty(dir: &Path) -> bool {
    std::fs::read_dir(dir.join(".rcfd/trash")).map_or(true, |mut r| r.next().is_none())
}

/// T1: a finished run trashed and restored is the same files; the project validates before, between and after; the run
/// is listed in the trash between and not after. Its results alone likewise.
#[test]
fn t1_trash_and_restore_a_run_and_its_results() {
    let dir = project("t1-0736");
    let s = server(&dir, Policy::default());
    let id = run(&dir, RunState::Finished, json!({}));
    let before = tree(&dir);
    valid(&dir);
    let r = ok(&call(&s, "trash_run", json!({"run": id, "reason": "an old try"})));
    let token = r["token"].as_str().unwrap().to_string();
    assert!(rcfd_project::trash::is_token(&token), "{r}");
    assert_eq!(r["kind"], "run");
    assert_eq!(r["path"], format!("runs/{id}"));
    assert!(r["bytes"].as_u64().unwrap() > 0);
    assert!(!dir.join("runs").join(&id).exists());
    valid(&dir);
    let l = ok(&call(&s, "list_trash", json!({})));
    assert_eq!(l["total"], 1);
    let item = &l["items"][0];
    assert_eq!(
        (
            item["token"].clone(),
            item["kind"].clone(),
            item["who"].clone(),
            item["reason"].clone()
        ),
        (json!(token), json!("run"), json!("trash_run"), json!("an old try"))
    );
    assert_eq!(item["actor"]["client"], "mcp");
    assert_eq!(l["bytes"], r["bytes"]);
    // the project's own run list no longer has it
    let runs = ok(&call(&s, "list_runs", json!({})));
    assert_eq!(runs["total"], 0, "{runs}");
    let back = ok(&call(&s, "restore_trash", json!({"token": token})));
    assert_eq!(back["restored"], true);
    assert_eq!(tree(&dir), before, "the run came back with other bytes");
    valid(&dir);
    assert!(trash_folder_is_empty(&dir));
    assert_eq!(ok(&call(&s, "list_trash", json!({})))["total"], 0);
    // its results alone: run.json, status.json, events.jsonl and residuals.jsonl stay
    let r = ok(&call(
        &s,
        "trash_results",
        json!({"run": id, "parts": ["fields", "output"]}),
    ));
    assert_eq!(r["kind"], "results");
    assert_eq!(r["parts"], json!(["fields", "output"]));
    let d = dir.join("runs").join(&id);
    assert!(!d.join("fields").exists() && !d.join("output").exists());
    for f in [
        "run.json",
        "status.json",
        "events.jsonl",
        "residuals.jsonl",
        "case.snapshot",
    ] {
        assert!(d.join(f).is_file(), "{f}");
    }
    valid(&dir);
    // a part that is not there: 1202, nothing moved
    let between = tree(&dir);
    assert_eq!(
        err_code(&call(&s, "trash_results", json!({"run": id, "parts": ["fields"]}))),
        1202
    );
    assert_eq!(tree(&dir), between);
    ok(&call(&s, "restore_trash", json!({"token": r["token"]})));
    assert_eq!(tree(&dir), before);
    valid(&dir);
    eprintln!(
        "T1: run {id} trashed as {token} and restored byte for byte ({} files); results likewise",
        before.len()
    );
}

/// T2: a queued, running or paused run is 1002 and nothing moves; a run that a queued run continues from is 1003 naming
/// that run, and is trashed once that run has ended (the maintainer, Q2 (a)).
#[test]
fn t2_only_ended_runs_that_no_unfinished_run_continues_from() {
    let dir = project("t2-0736");
    let s = server(&dir, Policy::default());
    for state in [RunState::Queued, RunState::Running, RunState::Paused] {
        let id = run(&dir, state, json!({}));
        let before = tree(&dir);
        for (tool, args) in [
            ("trash_run", json!({"run": id})),
            ("trash_results", json!({"run": id, "parts": ["fields"]})),
        ] {
            let r = call(&s, tool, args);
            assert_eq!(err_code(&r), 1002, "{tool} {state:?}: {r}");
        }
        assert_eq!(tree(&dir), before);
    }
    let a = run(&dir, RunState::Finished, json!({}));
    let b = run(&dir, RunState::Queued, json!({"continue_from": a}));
    let before = tree(&dir);
    let r = call(&s, "trash_run", json!({"run": a}));
    assert_eq!(err_code(&r), 1003, "{r}");
    assert_eq!(r["structuredContent"]["error"]["data"]["runs"], json!([b]));
    assert!(r["structuredContent"]["error"]["message"]
        .as_str()
        .unwrap()
        .contains(&b));
    let r = call(&s, "trash_results", json!({"run": a, "parts": ["output"]}));
    assert_eq!(err_code(&r), 1003, "{r}");
    assert_eq!(tree(&dir), before);
    // once b has ended, a can go (its restart file is gone, b's records stay)
    set_state(&dir, &b, RunState::Failed);
    ok(&call(&s, "trash_run", json!({"run": a})));
    assert!(dir.join("runs").join(&b).join("run.json").is_file());
    // an unknown run: 1202
    assert_eq!(
        err_code(&call(&s, "trash_run", json!({"run": "r-20260101T000000-0000"}))),
        1202
    );
    eprintln!("T2: queued, running, paused 1002; continued-from 1003 naming {b}, then allowed");
}

/// T3: a case with a stale base is 1200 with the current hash and stays; with its hash it moves, and a run made from it
/// keeps its own case.snapshot (the project validates). Only files under cases/.
#[test]
fn t3_trash_case_needs_its_base_and_leaves_runs_alone() {
    let dir = project("t3-0736");
    let s = server(&dir, Policy::default());
    let id = run(&dir, RunState::Finished, json!({}));
    let snap = std::fs::read(dir.join("runs").join(&id).join("case.snapshot")).unwrap();
    let h = ok(&call(&s, "read_project_file", json!({"path": "cases/cavity.toml"})))["hash"].clone();
    let before = tree(&dir);
    let r = call(
        &s,
        "trash_case",
        json!({"path": "cases/cavity.toml", "base_hash": "fnv64:0000000000000000"}),
    );
    assert_eq!(err_code(&r), 1200, "{r}");
    assert_eq!(r["structuredContent"]["error"]["data"]["current_hash"], h);
    assert_eq!(tree(&dir), before);
    for (path, code) in [
        ("notes/x.md", 1004),
        ("runs", 1004),
        ("cases/none.toml", 1202),
        ("../x.toml", 1004),
    ] {
        let r = call(&s, "trash_case", json!({"path": path, "base_hash": h}));
        assert_eq!(err_code(&r), code, "{path}: {r}");
    }
    assert_eq!(tree(&dir), before);
    let r = ok(&call(
        &s,
        "trash_case",
        json!({"path": "cases/cavity.toml", "base_hash": h}),
    ));
    assert_eq!((r["kind"].clone(), r["hash"].clone()), (json!("case"), h.clone()));
    assert!(!dir.join("cases/cavity.toml").exists());
    assert_eq!(
        std::fs::read(dir.join("runs").join(&id).join("case.snapshot")).unwrap(),
        snap
    );
    valid(&dir);
    eprintln!("T3: stale base 1200, moved with its hash; run {id} keeps its snapshot");
}

/// T4: a restore onto an occupied path is 1200 with `data.path` and moves nothing; once the path is free it restores.
#[test]
fn t4_restore_never_replaces() {
    let dir = project("t4-0736");
    let s = server(&dir, Policy::default());
    let original = std::fs::read(dir.join("cases/cavity.toml")).unwrap();
    let h = ok(&call(&s, "read_project_file", json!({"path": "cases/cavity.toml"})))["hash"].clone();
    let token = ok(&call(
        &s,
        "trash_case",
        json!({"path": "cases/cavity.toml", "base_hash": h}),
    ))["token"]
        .clone();
    std::fs::write(dir.join("cases/cavity.toml"), "title = \"new\"\n").unwrap();
    let before = tree(&dir);
    let r = call(&s, "restore_trash", json!({"token": token}));
    assert_eq!(err_code(&r), 1200, "{r}");
    assert_eq!(r["structuredContent"]["error"]["data"]["path"], "cases/cavity.toml");
    assert_eq!(tree(&dir), before);
    assert_eq!(ok(&call(&s, "list_trash", json!({})))["total"], 1);
    std::fs::rename(dir.join("cases/cavity.toml"), dir.join("cases/other.toml")).unwrap();
    ok(&call(&s, "restore_trash", json!({"token": token})));
    assert_eq!(std::fs::read(dir.join("cases/cavity.toml")).unwrap(), original);
    // a token that is not in the trash (any more): 1202; not a token: -32602
    assert_eq!(err_code(&call(&s, "restore_trash", json!({"token": token}))), 1202);
    assert_eq!(err_code(&call(&s, "restore_trash", json!({"token": "../x"}))), -32602);
    eprintln!("T4: occupied path 1200 data.path, nothing moved; restored once free");
}

/// T5: each accepted trash call leaves exactly one journal line and one feed entry (a trashed or restored case is not an
/// external change); under `--read-only` every trash write is 1100 and nothing changes.
#[test]
fn t5_one_line_and_one_feed_entry_per_call_and_read_only() {
    let dir = project("t5-0736");
    let s = server(&dir, Policy::default());
    let id = run(&dir, RunState::Cancelled, json!({}));
    let h = ok(&call(&s, "read_project_file", json!({"path": "cases/cavity.toml"})))["hash"].clone();
    let mut cursor = ok(&call(&s, "read_feed", json!({})))["cursor"].as_u64().unwrap();
    let mut tokens = Vec::new();
    let steps: Vec<(&str, Value, &str)> = vec![
        (
            "trash_results",
            json!({"run": id, "parts": ["fields"], "reason": "space"}),
            "trashed",
        ),
        ("trash_run", json!({"run": id}), "trashed"),
        (
            "trash_case",
            json!({"path": "cases/cavity.toml", "base_hash": h}),
            "trashed",
        ),
        ("restore_trash", json!({}), "restored"),
        ("restore_trash", json!({}), "restored"),
        ("empty_trash", json!({}), "trash_emptied"),
    ];
    for (tool, mut args, kind) in steps {
        if tool == "restore_trash" {
            args["token"] = tokens.pop().unwrap();
        }
        let lines = journal(&dir).len();
        let r = ok(&call(&s, tool, args));
        if tool.starts_with("trash_") {
            tokens.push(r["token"].clone());
        }
        let j = journal(&dir);
        assert_eq!(j.len(), lines + 1, "{tool}: one journal line");
        let l = j.last().unwrap();
        assert_eq!((l["kind"].as_str(), l["who"].as_str()), (Some(kind), Some(tool)), "{l}");
        assert_eq!(l["actor"]["client"], "mcp");
        std::thread::sleep(std::time::Duration::from_millis(600));
        let f = ok(&call(&s, "read_feed", json!({"since": cursor})));
        let entries = f["entries"].as_array().unwrap();
        assert_eq!(entries.len(), 1, "{tool}: {f}");
        assert_eq!(entries[0]["kind"], kind);
        eprintln!("T5 {tool}: {}", entries[0]["text"]);
        // a second poll after the external wait: still nothing (no external entry for the moved case)
        std::thread::sleep(std::time::Duration::from_millis(600));
        let f2 = ok(&call(&s, "read_feed", json!({"since": f["cursor"]})));
        assert_eq!(f2["entries"], json!([]), "{tool}: {f2}");
        cursor = f2["cursor"].as_u64().unwrap();
    }
    assert!(dir.join("runs").join(&id).join("run.json").is_file());
    assert!(dir.join("cases/cavity.toml").is_file());
    // the empty_trash above removed the results item (the fields), the one left
    assert!(!dir.join("runs").join(&id).join("fields").exists());
    // read-only: every trash write 1100, list_trash answers
    let id2 = run(&dir, RunState::Finished, json!({}));
    let h = ok(&call(&s, "read_project_file", json!({"path": "cases/cavity.toml"})))["hash"].clone();
    let t = ok(&call(&s, "trash_run", json!({"run": id2})))["token"].clone();
    let ro = server(&dir, Policy::read_only());
    let before = tree(&dir);
    let lines = journal(&dir).len();
    for (tool, args, access) in [
        ("trash_run", json!({"run": id}), "write_files"),
        ("trash_results", json!({"run": id, "parts": ["output"]}), "write_files"),
        (
            "trash_case",
            json!({"path": "cases/cavity.toml", "base_hash": h}),
            "write_files",
        ),
        ("restore_trash", json!({"token": t}), "write_files"),
        ("empty_trash", json!({}), "delete"),
    ] {
        let r = call(&ro, tool, args);
        assert_eq!(err_code(&r), 1100, "{tool}: {r}");
        assert_eq!(r["structuredContent"]["error"]["data"]["access"], access);
    }
    assert_eq!(ok(&call(&ro, "list_trash", json!({})))["total"], 1);
    assert_eq!(tree(&dir), before);
    assert_eq!(journal(&dir).len(), lines);
    eprintln!("T5: one line and one feed entry per call; read-only refuses the five writes with 1100");
}

/// E: `empty_trash` has the class `delete` (a hook sees it so), refuses unknown tokens without removing anything, keeps
/// newer items with `older_than_days`, and removes the rest for good.
#[test]
fn e_empty_trash_is_the_delete_class_and_removes_for_good() {
    let dir = project("e-0736");
    let s = server(&dir, Policy::default());
    let a = run(&dir, RunState::Finished, json!({}));
    let b = run(&dir, RunState::Finished, json!({}));
    let ta = ok(&call(&s, "trash_run", json!({"run": a})))["token"]
        .as_str()
        .unwrap()
        .to_string();
    let tb = ok(&call(&s, "trash_run", json!({"run": b})))["token"]
        .as_str()
        .unwrap()
        .to_string();
    let asked: Arc<Mutex<Vec<(String, Access)>>> = Arc::new(Mutex::new(Vec::new()));
    let seen = asked.clone();
    let refusing = server(
        &dir,
        Policy::with_hook(move |c| {
            seen.lock().unwrap().push((c.tool.to_string(), c.access));
            false
        }),
    );
    assert_eq!(err_code(&call(&refusing, "empty_trash", json!({}))), 1100);
    assert_eq!(*asked.lock().unwrap(), [("empty_trash".to_string(), Access::Delete)]);
    assert_eq!(ok(&call(&s, "list_trash", json!({})))["total"], 2);
    let r = call(&s, "empty_trash", json!({"tokens": [ta, "t-20200101T000000-0000"]}));
    assert_eq!(err_code(&r), 1202, "{r}");
    assert_eq!(ok(&call(&s, "list_trash", json!({})))["total"], 2);
    let r = ok(&call(&s, "empty_trash", json!({"older_than_days": 30})));
    assert_eq!(r["removed"], json!([]));
    let r = ok(&call(&s, "empty_trash", json!({"tokens": [ta]})));
    assert_eq!(r["removed"], json!([ta]));
    let l = ok(&call(&s, "list_trash", json!({})));
    assert_eq!(l["items"][0]["token"], json!(tb));
    let r = ok(&call(&s, "empty_trash", json!({})));
    assert_eq!(r["removed"], json!([tb]));
    assert!(r["bytes"].as_u64().unwrap() > 0);
    assert!(trash_folder_is_empty(&dir));
    assert!(!dir.join("runs").join(&a).exists() && !dir.join("runs").join(&b).exists());
    valid(&dir);
    eprintln!("E: empty_trash asked as delete, unknown token 1202, older_than_days kept both, then removed both");
}

/// A: the class `delete` (ADR 0736 Q3 (a)): the delete-class tools of the live list are exactly `empty_trash`, with
/// `destructiveHint` true and `readOnlyHint` false; every other tool has `destructiveHint` false. Restated by ADR 0801
/// Addendum 23, which removed the Studio's Claude Code allow-list: Claude Code's own permission prompts ask for every
/// call of a tool no rule of the user's allows, and these annotations are what the engine tells it.
#[test]
fn a_the_delete_class_is_empty_trash_alone() {
    let dir = project("a-0736");
    let s = server(&dir, Policy::default());
    let live = s.tool_list();
    let tools = live["tools"].as_array().unwrap();
    let delete: Vec<&str> = tools
        .iter()
        .filter(|t| t["_meta"]["rcfd/access"] == "delete")
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(delete, ["empty_trash"]);
    let mut others = 0;
    for t in tools {
        let d = t["_meta"]["rcfd/access"] == "delete";
        assert_eq!(t["annotations"]["destructiveHint"], d, "{}", t["name"]);
        if d {
            assert_eq!(t["annotations"]["readOnlyHint"], false);
        } else {
            others += 1;
        }
    }
    eprintln!(
        "A: {delete:?} the one delete-class tool, destructiveHint true; {others} other tools destructiveHint false"
    );
}
