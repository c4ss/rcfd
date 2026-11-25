//! ADR 0794 (F-274), the rows without a runner: the journal line of a tagged submission and of a refused one (J7), that a
//! `design_recorded` line is written after the store holds the record (J9), a journal that cannot be appended (J10), the lines of
//! a study and a map evaluated in process (J12), and the temporary file of a design tool's write in the feed (F3).

use rcfd_api::service::{Service, ServiceConfig};
use rcfd_project::records::hash_bytes;
use rcfd_project::Project;
use rcfd_runner::records::{LaunchSettings, RunSettings};
use rcfd_runner::store::RunTags;
use rcfd_workspace::feed::{Change, Feed};
use rcfd_workspace::journal::{append, Actor, Origin};
use rcfd_workspace::runs::RunService;
use rcfd_workspace::templates::LID_DRIVEN_CAVITY;
use rcfd_workspace::Workspace;
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

/// A folder that holds the projects of one test and the base case outside them.
fn outer(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rcfd-ws-f274j-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("base.toml"), small_case()).unwrap();
    dir
}

/// The template at 16 x 16 cells with `max_iterations = 3000`.
fn small_case() -> String {
    LID_DRIVEN_CAVITY.replace("max_iterations = 600", "max_iterations = 3000")
}

/// A project `name` in `outer` with `cases/small.toml`.
fn project(outer: &Path, name: &str) -> PathBuf {
    let dir = outer.join(name);
    Project::create(&dir, name, "test").unwrap();
    std::fs::create_dir_all(dir.join("cases")).unwrap();
    std::fs::write(dir.join("cases/small.toml"), small_case()).unwrap();
    dir
}

fn workspace(dir: &Path) -> Workspace {
    Workspace::open(dir).unwrap().with_actor(Actor::this_process("mcp"))
}

/// The design tools' service with the sandbox `outer`.
fn service(outer: &Path) -> Arc<Service> {
    Service::new(ServiceConfig {
        root: Some(outer.to_path_buf()),
        ..Default::default()
    })
}

fn study(dir: &str, method: Value) -> Value {
    json!({
        "dir": dir, "base": "base.toml", "name": "s",
        "parameters": [{"name": "nu", "lo": 0.01, "hi": 0.02, "path": "physics.nu"}],
        "objectives": [{"metric": "lid_drag", "sense": "min"}],
        "method": method, "workers": 2
    })
}

fn nu3(dir: &str) -> Value {
    study(dir, json!({"type": "factorial", "levels": [3]}))
}

fn journal(dir: &Path) -> Vec<Value> {
    std::fs::read_to_string(dir.join(".rcfd/edits.jsonl"))
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

fn recorded(dir: &Path, study: &str) -> Vec<Value> {
    journal(dir)
        .into_iter()
        .filter(|l| l["kind"] == "design_recorded" && l["study"] == study)
        .collect()
}

fn run_count(dir: &Path) -> usize {
    std::fs::read_dir(dir.join("runs"))
        .map(|d| {
            d.flatten()
                .filter(|e| e.file_name().to_string_lossy().starts_with("r-"))
                .count()
        })
        .unwrap_or(0)
}

/// The lines of study `rel` are one per record of its store, in order, each with the hash of the store up to it.
fn check_records(dir: &Path, rel: &str, n: usize) {
    let bytes = std::fs::read(dir.join(rel).join("evaluations.jsonl")).unwrap();
    let mut at = 0;
    let mut want = Vec::new();
    for l in bytes.split_inclusive(|&b| b == b'\n') {
        at += l.len();
        let v: Value = serde_json::from_slice(l).unwrap();
        if let Some(i) = v["index"].as_u64() {
            want.push((i, hash_bytes(&bytes[..at]), format!("nu={}", v["params"]["nu"])));
        }
    }
    let have: Vec<(u64, String, String)> = recorded(dir, rel)
        .iter()
        .map(|l| {
            (
                l["index"].as_u64().unwrap(),
                l["hash"].as_str().unwrap().to_string(),
                l["variant"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    assert_eq!(have, want);
    assert_eq!(have.len(), n);
}

#[test]
fn j7_a_refused_submission_writes_no_line() {
    let out = outer("j7");
    let dir = project(&out, "proj");
    let actor = Actor::this_process("mcp");
    let plain = RunService::new(&dir, "no-such-runner");
    let runs = plain.clone().with_actor(Some(actor.clone()));
    let case = dir.join("cases/small.toml");
    let settings = RunSettings::default();
    let tags = RunTags {
        study: Some("designs/s".into()),
        variant: Some("nu=0.01".into()),
        condition: Some("base".into()),
    };
    let launch = |c: Option<&str>, m: Option<u64>| LaunchSettings {
        continue_from: c.map(String::from),
        max_steps: m,
    };
    let none = launch(None, None);
    let missing = dir.join("cases/none.toml");
    for (what, r) in [
        (
            "continue_from",
            runs.submit_with(&case, &settings, &launch(Some("r-20260101T000000-abcd"), None), &tags),
        ),
        (
            "max_steps",
            runs.submit_with(&case, &settings, &launch(None, Some(0)), &tags),
        ),
        ("case", runs.submit_with(&missing, &settings, &none, &tags)),
        ("case", runs.submit_tagged(&missing, &settings, &tags)),
    ] {
        eprintln!("J7 {what}: {}", r.as_ref().unwrap_err());
        assert!(r.is_err(), "{what}");
        assert_eq!(run_count(&dir), 0, "{what}");
        assert!(!dir.join(".rcfd/edits.jsonl").exists(), "{what}: {:?}", journal(&dir));
    }
    // control: accepted, one line each
    let id = runs
        .by("x", Some("why".into()))
        .submit_with(&case, &settings, &launch(None, Some(7)), &tags)
        .unwrap();
    let id2 = plain.submit_tagged(&case, &settings, &tags).unwrap();
    let l = journal(&dir);
    assert_eq!(l.len(), 2, "{l:#?}");
    for (x, run) in l.iter().zip([&id, &id2]) {
        assert_eq!(x["kind"], "run_submitted");
        assert_eq!(x["run"], **run);
        assert_eq!(x["case"], "cases/small.toml");
        assert_eq!(x["study"], "designs/s");
        assert_eq!(x["variant"], "nu=0.01");
        assert_eq!(x["condition"], "base");
    }
    assert_eq!(
        (&l[0]["who"], &l[0]["reason"], &l[0]["max_steps"]),
        (&json!("x"), &json!("why"), &json!(7))
    );
    assert_eq!(l[0]["actor"], actor.to_json());
    assert!(l[1].get("actor").is_none() && l[1].get("reason").is_none(), "{}", l[1]);
}

/// Make `f` read-only or writable.
#[allow(clippy::permissions_set_readonly_false)]
fn read_only(f: &Path, on: bool) {
    let mut p = std::fs::metadata(f).unwrap().permissions();
    p.set_readonly(on);
    std::fs::set_permissions(f, p).unwrap();
}

#[test]
fn j9_the_line_is_written_after_the_store_holds_the_record() {
    let out = outer("j9");
    let dir = project(&out, "proj");
    let (ws, svc) = (workspace(&dir), service(&out));
    for s in ["proj/designs/a", "proj/designs/b"] {
        ws.design_call(&svc, "design.create_study", &nu3(s)).unwrap();
    }
    let run = |s: &str| ws.design_call(&svc, "design.run_study", &json!({"dir": s}));
    let v = run("proj/designs/a").unwrap();
    assert_eq!(v["evaluator"], "process", "{v}");
    // the header of the same definition, in a store that cannot be appended
    let a = std::fs::read_to_string(dir.join("designs/a/evaluations.jsonl")).unwrap();
    let header = format!("{}\n", a.lines().next().unwrap());
    let store = dir.join("designs/b/evaluations.jsonl");
    std::fs::write(&store, &header).unwrap();
    read_only(&store, true);
    let e = run("proj/designs/b").unwrap_err();
    eprintln!("J9: {}", e.message);
    assert_eq!(std::fs::read_to_string(&store).unwrap(), header);
    assert_eq!(recorded(&dir, "designs/b"), Vec::<Value>::new());
    // control
    read_only(&store, false);
    let mut feed = Feed::open(&dir.canonicalize().unwrap()).unwrap();
    run("proj/designs/b").unwrap();
    check_records(&dir, "designs/b", 3);
    // and the feed takes each of the three lines, and no `external` change of the store
    let mut seen: Vec<Change> = Vec::new();
    for k in 0..3 {
        if k > 0 {
            std::thread::sleep(Duration::from_millis(600));
        }
        seen.extend(feed.poll().unwrap().changes);
    }
    let sources: Vec<&str> = seen.iter().map(|c| c.source.as_str()).collect();
    assert_eq!(sources, ["journal"; 3], "{seen:#?}");
    check_records(&dir, "designs/a", 3);
}

#[test]
fn j10_a_journal_that_cannot_be_appended_changes_no_call() {
    let out = outer("j10");
    let svc = service(&out);
    let (p, q, r) = (project(&out, "p"), project(&out, "q"), project(&out, "r"));
    // (a) the journal's path is a folder
    std::fs::create_dir_all(q.join(".rcfd/edits.jsonl")).unwrap();
    let call = |dir: &Path, method: &str, args: &Value| workspace(dir).design_call(&svc, method, args).unwrap();
    let mut replies: Vec<(Value, Value)> = Vec::new();
    for d in [&p, &q, &r] {
        let s = format!("{}/designs/s", d.file_name().unwrap().to_string_lossy());
        call(d, "design.create_study", &nu3(&s));
        let run = call(d, "design.run_study", &json!({"dir": s}));
        if d == &r {
            assert_eq!(run, replies[0].0);
            break;
        }
        let eval = call(d, "design.evaluate", &json!({"dir": s, "params": {"nu": 0.0175}}));
        replies.push((run, eval));
    }
    assert_eq!(replies[0], replies[1]);
    let bytes = |d: &Path| std::fs::read(d.join("designs/s/evaluations.jsonl")).unwrap();
    assert_eq!(bytes(&p), bytes(&q));
    check_records(&p, "designs/s", 4);
    assert_eq!(std::fs::read_dir(q.join(".rcfd/edits.jsonl")).unwrap().count(), 0);
    // (b) the journal's lock is held (its time ahead, so that it is never taken to be stale)
    let lock = r.join(rcfd_project::journal::LOCK);
    std::fs::create_dir_all(lock.parent().unwrap()).unwrap();
    let f = std::fs::File::create(&lock).unwrap();
    f.set_modified(SystemTime::now() + Duration::from_secs(3600)).unwrap();
    drop(f);
    let at = journal(&r).len();
    let v = call(
        &r,
        "design.evaluate",
        &json!({"dir": "r/designs/s", "params": {"nu": 0.0175}}),
    );
    assert_eq!(v, replies[0].1);
    assert_eq!(bytes(&r), bytes(&p));
    let l = journal(&r);
    assert_eq!(l.len(), at + 1, "{l:#?}");
    eprintln!("J10 (b): {}", l[at]);
    assert_eq!(l[at]["kind"], "design_recorded");
    assert!(l[at].get("seq").is_none(), "{}", l[at]);
    assert!(lock.exists());
}

#[test]
fn j12_a_study_and_a_map_evaluated_in_process_have_their_lines() {
    let out = outer("j12");
    let dir = project(&out, "proj");
    let (ws, svc) = (workspace(&dir), service(&out));
    let call = |method: &str, args: &Value| ws.design_call(&svc, method, args).unwrap();
    call("design.create_study", &nu3("proj/designs/nu3"));
    call("design.run_study", &json!({"dir": "proj/designs/nu3"}));
    check_records(&dir, "designs/nu3", 3);
    call(
        "design.create_study",
        &study("proj/designs/nm", json!({"type": "nelder_mead", "budget": 4})),
    );
    call("design.run_study", &json!({"dir": "proj/designs/nm"}));
    let n = std::fs::read_to_string(dir.join("designs/nm/evaluations.jsonl"))
        .unwrap()
        .lines()
        .count()
        - 1;
    assert!(n >= 3, "{n}");
    check_records(&dir, "designs/nm", n);
    let at = journal(&dir).len();
    let v = call(
        "design.run_map",
        &json!({"base": "base.toml", "out_dir": "proj/designs/map", "axes": [
            {"path": "boundaries.lid.velocity_value", "values": [[1.0, 0.0, 0.0], [0.75, 0.0, 0.0], [0.5, 0.0, 0.0]]}]}),
    );
    assert_eq!(v["evaluator"], "process", "{v}");
    let l = journal(&dir)[at..].to_vec();
    assert_eq!(l.len(), 3, "{l:#?}");
    let map = hash_bytes(&std::fs::read(dir.join("designs/map/map.json")).unwrap());
    for (i, x) in l.iter().enumerate() {
        assert_eq!(x["kind"], "design_recorded");
        assert_eq!(x["who"], "design_run_operating_map");
        assert_eq!(x["point"], i);
        assert_eq!(x["hash"], map, "{x}");
        assert_eq!(x["files"].as_array().unwrap().len(), if i == 2 { 2 } else { 1 }, "{x}");
    }
    // a study outside the project
    let at = journal(&dir).len();
    call("design.create_study", &nu3("out/nu3"));
    call("design.run_study", &json!({"dir": "out/nu3"}));
    assert!(out.join("out/nu3/evaluations.jsonl").is_file());
    assert_eq!(journal(&dir).len(), at);
    assert_eq!(run_count(&dir), 0);
    assert!(journal(&dir).iter().all(|l| l["kind"] != "run_submitted"));
    eprintln!("J12: {at} lines, none of a run; {n} records of nm");
}

#[test]
fn f3_the_temporary_file_of_a_design_write_is_not_external() {
    let out = outer("f3");
    let dir = project(&out, "proj").canonicalize().unwrap();
    let s = dir.join("designs/s");
    std::fs::create_dir_all(&s).unwrap();
    let mut feed = Feed::open(&dir).unwrap();
    let ahead = |f: &Path| {
        let h = std::fs::OpenOptions::new().write(true).open(f).unwrap();
        h.set_modified(SystemTime::now() + Duration::from_secs(30)).unwrap();
    };
    // the scan sees both temporary files
    for f in ["study.tmp", "other.tmp"] {
        std::fs::write(s.join(f), "{}").unwrap();
        ahead(&s.join(f));
    }
    let first = feed.poll().unwrap();
    assert_eq!(first.paths, ["designs/s/other.tmp", "designs/s/study.tmp"]);
    assert!(first.changes.is_empty(), "{:?}", first.changes);
    // one is renamed and has its line, the other (control) is removed and has none
    std::fs::rename(s.join("study.tmp"), s.join("study.json")).unwrap();
    let h = std::fs::OpenOptions::new()
        .write(true)
        .open(s.join("study.json"))
        .unwrap();
    h.set_modified(SystemTime::now()).unwrap();
    drop(h);
    std::fs::remove_file(s.join("other.tmp")).unwrap();
    let mut m = Map::new();
    m.insert("path".into(), json!("designs/s/study.json"));
    m.insert("hash".into(), json!(hash_bytes(b"{}")));
    append(
        &dir,
        "design_created",
        &Origin::new("design_create_study", None, None),
        m,
    );
    let mut seen: Vec<Change> = Vec::new();
    for _ in 0..4 {
        seen.extend(feed.poll().unwrap().changes);
        std::thread::sleep(Duration::from_millis(400));
    }
    seen.extend(feed.poll().unwrap().changes);
    let ext: Vec<&str> = seen
        .iter()
        .filter(|c| c.source == "external")
        .map(|c| c.path.as_deref().unwrap())
        .collect();
    eprintln!("F3: {} entries, external {ext:?}", seen.len());
    assert_eq!(ext, ["designs/s/other.tmp"], "{seen:#?}");
    assert_eq!(seen.iter().filter(|c| c.source == "journal").count(), 1, "{seen:#?}");
}
