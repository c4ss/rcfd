//! ADR 0188 (F-273) through the `rcfd-mcp` binary. C3 (ADR 0173 C3, two processes): an `edit_case` with a reason made
//! by the server appears in the feed of this test process, polled every 250 ms, within 4 polls, with the server's actor
//! (its process id), the changed setting's old and new value and the reason; a plain write by this process appears as
//! `external` within 4 polls; a Studio save made here appears in the server's `read_feed`. C3-R: `start_run` and
//! `cancel_run` with reasons, their journal lines and the run's state changes in `read_feed`; they need `rcfd` and
//! `rcfd-runner` next to `rcfd-mcp`. C-M: `read_feed` under `Policy::read_only()`.

mod common;

use common::*;
use rcfd_mcp::policy::Policy;
use rcfd_project::records::hash_bytes;
use rcfd_project::Project;
use rcfd_workspace::feed::{Change, Feed, EXTERNAL, JOURNAL};
use rcfd_workspace::journal::{Actor, Origin, STUDIO};
use rcfd_workspace::save::save_case_by;
use rcfd_workspace::SaveForm;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// ADR 0188 C3: the reader's poll period.
const PERIOD: Duration = Duration::from_millis(250);
/// ADR 0188 C3: the limit, in polls.
const POLLS: usize = 4;
const CASE: &str = "cases/cavity.toml";

fn root_of(dir: &Path) -> PathBuf {
    Project::open(dir).unwrap().root().to_path_buf()
}

/// Poll `feed` at once and then every `PERIOD`, at most `POLLS` times, until an entry satisfies `want`; the entry, the
/// number of polls and every entry seen.
fn first_within(feed: &mut Feed, want: impl Fn(&Change) -> bool) -> (Option<Change>, usize, Vec<Change>) {
    let mut seen = Vec::new();
    for k in 1..=POLLS {
        if k > 1 {
            std::thread::sleep(PERIOD);
        }
        let got = feed.poll().unwrap().changes;
        let hit = got.iter().find(|c| want(c)).cloned();
        seen.extend(got);
        if hit.is_some() {
            return (hit, k, seen);
        }
    }
    (None, POLLS, seen)
}

#[test]
fn c3_an_edit_by_the_server_reaches_a_second_process_within_four_polls() {
    let dir = project("f273c3");
    let root = root_of(&dir);
    let d = dir.to_string_lossy().to_string();
    let mut m = Mcp::start(&["--project", &d]);
    m.request("initialize", json!({"protocolVersion": "2025-06-18"}));
    let mut feed = Feed::open(&root).unwrap();
    assert!(feed.poll().unwrap().changes.is_empty());
    let h = m.call("read_project_file", json!({"path": CASE}))["structuredContent"]["hash"].clone();
    let why = "to halve the Reynolds number";
    let r = m.call(
        "edit_case",
        json!({"path": CASE, "base_hash": h, "set": {"physics.nu": 0.02}, "reason": why}),
    );
    assert_eq!(r["isError"], false, "{r}");
    let t0 = Instant::now();
    let (hit, polls, seen) = first_within(&mut feed, |c| c.source == JOURNAL && c.path.as_deref() == Some(CASE));
    let took = t0.elapsed();
    let c = hit.unwrap_or_else(|| panic!("not in the feed after {POLLS} polls: {seen:?}"));
    eprintln!(
        "C3: the server's edit_case in this process's feed at poll {polls} of {POLLS} ({:.3} s): {}",
        took.as_secs_f64(),
        c.to_json()
    );
    assert_eq!(c.kind, "edit");
    assert_eq!(c.who.as_deref(), Some("edit_case"));
    let a = c.actor.clone().unwrap();
    assert_eq!(a["client"], "mcp");
    assert_eq!(a["instance"]["pid"], m.pid(), "the actor is not the server's process");
    assert_ne!(m.pid(), std::process::id());
    assert_eq!(
        c.changes,
        Some(json!([{"path": "physics.nu", "old": 0.01, "new": 0.02}]))
    );
    assert_eq!(c.reason.as_deref(), Some(why));
    assert_eq!(c.other["new_hash"], r["structuredContent"]["new_hash"]);
    // a plain write by this process: external, with its path only
    std::fs::write(root.join("notes/c3.md"), b"written outside").unwrap();
    let t1 = Instant::now();
    let (hit, polls, seen) = first_within(&mut feed, |c| c.source == EXTERNAL);
    let e = hit.unwrap_or_else(|| panic!("no external entry after {POLLS} polls: {seen:?}"));
    eprintln!(
        "C3: the plain write as external at poll {polls} of {POLLS} ({:.3} s): {}",
        t1.elapsed().as_secs_f64(),
        e.text
    );
    assert_eq!(e.path.as_deref(), Some("notes/c3.md"));
    assert!(e.actor.is_none() && e.changes.is_none() && e.reason.is_none() && e.other.is_empty());
    assert!(
        seen.iter()
            .all(|c| c.source == EXTERNAL && c.path.as_deref() == Some("notes/c3.md")),
        "{seen:?}"
    );
    // the other way: a Studio save in this process, in the server's read_feed
    let cursor = m.call("read_feed", json!({}))["structuredContent"]["cursor"].clone();
    let now = std::fs::read(root.join(CASE)).unwrap();
    let text = String::from_utf8(now.clone()).unwrap().replace("nx = 12", "nx = 10");
    let studio = Origin::new("save_case", Some(Actor::this_process(STUDIO)), None);
    save_case_by(
        &root,
        CASE,
        &hash_bytes(&now),
        text.as_bytes(),
        SaveForm::Whole,
        &studio,
    )
    .unwrap();
    let f = m.call("read_feed", json!({"since": cursor}))["structuredContent"].clone();
    eprintln!("C3: the server's read_feed after a Studio save here: {f}");
    let entries = f["entries"].as_array().unwrap();
    let s = entries
        .iter()
        .find(|e| e["source"] == JOURNAL)
        .unwrap_or_else(|| panic!("no journal entry: {f}"));
    assert_eq!((s["who"].as_str(), s["path"].as_str()), (Some("save_case"), Some(CASE)));
    assert_eq!(s["actor"]["client"], STUDIO);
    assert_eq!(s["actor"]["instance"]["pid"], std::process::id());
    assert_eq!(s["changes"], json!([{"path": "mesh.nx", "old": 12, "new": 10}]));
    assert!(
        entries.iter().all(|e| e["source"] != EXTERNAL),
        "the save was labelled external: {f}"
    );
    // control: an edit without a reason has no reason field
    let h = hash_bytes(text.as_bytes());
    let r = m.call(
        "edit_case",
        json!({"path": CASE, "base_hash": h, "set": {"physics.nu": 0.03}}),
    );
    assert_eq!(r["isError"], false, "{r}");
    let (hit, _, seen) = first_within(&mut feed, |c| {
        c.source == JOURNAL && c.who.as_deref() == Some("edit_case")
    });
    let c = hit.unwrap_or_else(|| panic!("{seen:?}"));
    assert!(c.reason.is_none(), "{}", c.to_json());
}

/// Stops the project's runner on drop.
struct Runner(PathBuf);

impl Drop for Runner {
    fn drop(&mut self) {
        let exe = Path::new(env!("CARGO_BIN_EXE_rcfd-mcp"))
            .with_file_name(format!("rcfd-runner{}", std::env::consts::EXE_SUFFIX));
        let _ = rcfd_runner::client::Client::new(&self.0, exe).shutdown(Duration::from_secs(10));
    }
}

fn journal(root: &Path) -> Vec<Value> {
    std::fs::read_to_string(root.join(".rcfd/edits.jsonl"))
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

fn state(m: &mut Mcp, run: &str) -> String {
    m.call("run_status", json!({"run": run, "events": 0}))["structuredContent"]["status"]["state"]
        .as_str()
        .unwrap()
        .to_string()
}

#[test]
fn c3r_run_submissions_and_requests_are_journaled_and_their_states_are_in_the_feed() {
    binaries();
    let dir = project("f273c3r");
    let root = root_of(&dir);
    let _runner = Runner(dir.clone());
    let d = dir.to_string_lossy().to_string();
    let mut m = Mcp::start(&["--project", &d]);
    let cursor = m.call("read_feed", json!({}))["structuredContent"]["cursor"].clone();
    let r = m.call(
        "start_run",
        json!({"case": CASE, "set": ["solver.max_iterations=10000000"], "reason": "a long run to stop"}),
    );
    assert_eq!(r["isError"], false, "{r}");
    let run = r["structuredContent"]["run"].as_str().unwrap().to_string();
    let t0 = Instant::now();
    while state(&mut m, &run) != "running" {
        assert!(t0.elapsed() < Duration::from_secs(120), "run {run} not running");
        std::thread::sleep(Duration::from_millis(50));
    }
    let c = m.call("cancel_run", json!({"run": run, "reason": "enough"}));
    assert_eq!(c["isError"], false, "{c}");
    let t0 = Instant::now();
    while !["cancelled", "finished", "failed"].contains(&state(&mut m, &run).as_str()) {
        assert!(t0.elapsed() < Duration::from_secs(60), "run {run} not ended");
        std::thread::sleep(Duration::from_millis(50));
    }
    let j = journal(&root);
    for l in &j {
        eprintln!("C3-R line: {l}");
    }
    assert_eq!(j.len(), 2, "{j:#?}");
    assert_eq!(
        (j[0]["kind"].as_str(), j[0]["who"].as_str()),
        (Some("run_submitted"), Some("start_run"))
    );
    assert_eq!(
        (j[0]["run"].as_str(), j[0]["case"].as_str()),
        (Some(run.as_str()), Some(CASE))
    );
    assert_eq!(j[0]["set"], json!(["solver.max_iterations=10000000"]));
    assert_eq!(j[0]["reason"], "a long run to stop");
    assert_eq!(j[0]["actor"]["client"], "mcp");
    assert_eq!(j[0]["actor"]["instance"]["pid"], m.pid());
    assert_eq!(
        (j[1]["kind"].as_str(), j[1]["who"].as_str()),
        (Some("run_control"), Some("cancel_run"))
    );
    assert_eq!(
        (j[1]["run"].as_str(), j[1]["requested"].as_str()),
        (Some(run.as_str()), Some("cancel"))
    );
    assert_eq!(j[1]["reason"], "enough");
    assert_eq!((j[0]["seq"].clone(), j[1]["seq"].clone()), (json!(1), json!(2)));
    // the feed: the two journal entries and the run's state changes, ending cancelled
    let f = m.call("read_feed", json!({"since": cursor, "limit": 200}))["structuredContent"].clone();
    let entries = f["entries"].as_array().unwrap();
    for e in entries {
        eprintln!("C3-R entry: {}", e["text"]);
    }
    let kinds: Vec<&str> = entries.iter().map(|e| e["kind"].as_str().unwrap()).collect();
    assert!(
        kinds.contains(&"run_submitted") && kinds.contains(&"run_control"),
        "{kinds:?}"
    );
    let states: Vec<&str> = entries
        .iter()
        .filter(|e| e["kind"] == "run_state" && e["run"] == run.as_str())
        .map(|e| e["to"].as_str().unwrap())
        .collect();
    assert_eq!(states.first(), Some(&"queued"), "{states:?}");
    assert_eq!(states.last(), Some(&"cancelled"), "{states:?}");
    assert!(states.contains(&"running"), "{states:?}");
    assert!(entries.iter().all(|e| e["source"] != EXTERNAL), "{f}");
    // a request refused for an ended run writes no line
    let p = m.call("pause_run", json!({"run": run, "reason": "too late"}));
    assert_eq!(err_code(&p), 1002, "{p}");
    assert_eq!(journal(&root).len(), 2);
    eprintln!("C3-R: run {run}: states {states:?}; 2 journal lines; the refused pause wrote none");
}

#[test]
fn cm_read_feed_answers_read_only_and_a_too_long_reason_is_refused() {
    let dir = project("f273ro");
    let ro = server(&dir, Policy::read_only());
    let r = ok(&call(&ro, "read_feed", json!({})));
    assert_eq!((r["cursor"].clone(), r["total"].clone()), (json!(0), json!(0)));
    let r = ok(&call(&ro, "read_feed", json!({"since": 0})));
    assert_eq!(r["entries"], json!([]));
    let s = server(&dir, Policy::default());
    let h = hash_bytes(&std::fs::read(dir.join(CASE)).unwrap());
    let long = call(
        &s,
        "edit_case",
        json!({"path": CASE, "base_hash": h, "set": {"physics.nu": 0.02}, "reason": "x".repeat(501)}),
    );
    assert_eq!(err_code(&long), -32602, "{long}");
    assert!(!dir.join(".rcfd/edits.jsonl").exists());
    let fine = call(
        &s,
        "edit_case",
        json!({"path": CASE, "base_hash": h, "set": {"physics.nu": 0.02}, "reason": "x".repeat(500)}),
    );
    assert_eq!(fine["isError"], false, "{fine}");
    assert_eq!(journal(&dir)[0]["reason"].as_str().unwrap().len(), 500);
}
