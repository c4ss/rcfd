//! ADR 0188 (F-273, ADR 0173 R3, W6, C3, D4): the project journal's entries and the change feed, in one process.
//! C3-J the lines of each workspace command (actor, `seq`, changes, reason) and the calls that write none; C3-D the
//! changes of a Studio save and the cut of long lists; C3-O lines written before F-273 read by the feed; C3-F the feed
//! (an edit explained by its line, an outside write labelled `external`, the late line of J6, run events, nothing
//! written, entries no longer kept). The two-process check C3 and the runs through the runner are
//! `crates/rcfd-mcp/tests/feed_f273.rs`; the Studio's reactions `crates/rcfd-studio/tests/feed_f273.rs`.

use rcfd_api::case::Case;
use rcfd_project::records::{hash_bytes, EventKind, RunEvent, RunState};
use rcfd_project::Project;
use rcfd_workspace::feed::{Change, Feed, EXTERNAL, JOURNAL, KEEP, RUN};
use rcfd_workspace::journal::{put_list, Actor, Origin, Reader, MAX_CHANGES, MCP, STUDIO};
use rcfd_workspace::save::save_case_by;
use rcfd_workspace::{Answer, RpcError, SaveForm, Workspace};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

const CAVITY: &str = r#"title = "f273 cavity" # a comment edit_case keeps
[mesh]
generator = "cavity_2d"
nx = 16
ny = 12
[physics]
nu = 0.01   # viscosity
[boundaries.lid]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.walls]
velocity = "fixed"
[solver]
algorithm = "simplec"
max_iterations = 30
"#;
const REL: &str = "cases/a.toml";
const PERIOD: Duration = Duration::from_millis(250);

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn project(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir()
        .join("rcfd-workspace-tests")
        .join(format!("f273-{name}-{}-{nanos}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    Project::create(&dir, name, "test").unwrap();
    let root = Project::open(&dir).unwrap().root().to_path_buf();
    std::fs::write(root.join(REL), CAVITY).unwrap();
    root
}

fn call(ws: &Workspace, name: &str, args: Value) -> Result<Value, RpcError> {
    match ws.call(name, &args).expect("a workspace command")? {
        Answer::Value(v) => Ok(v),
        other => panic!("{name}: not a value: {other:?}"),
    }
}

fn journal(root: &Path) -> Vec<Value> {
    std::fs::read_to_string(root.join(".rcfd/edits.jsonl"))
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

fn keys(v: &Value) -> Vec<String> {
    let mut k: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
    k.sort();
    k
}

fn mcp_actor() -> Actor {
    Actor::this_process(MCP)
}

#[test]
fn c3j_each_command_writes_one_numbered_line_with_its_actor_changes_and_reason() {
    let root = project("c3j");
    let ws = Workspace::open(&root).unwrap().with_actor(mcp_actor());
    let h0 = hash_bytes(CAVITY.as_bytes());
    let why = "to reach Re 50";
    // nothing is written by: a dry run, a refused value, a conflict, a reason too long or not a string
    let dry = call(
        &ws,
        "edit_case",
        json!({"path": REL, "base_hash": h0, "set": {"physics.nu": 0.02}, "dry_run": true, "reason": why}),
    )
    .unwrap();
    assert_eq!(dry["written"], false);
    let refused = [
        (
            json!({"path": REL, "base_hash": h0, "set": {"physics.nu": -1}, "reason": why}),
            1003,
        ),
        (
            json!({"path": REL, "base_hash": "fnv64:0000000000000000", "set": {"physics.nu": 0.02}}),
            1200,
        ),
        (
            json!({"path": REL, "base_hash": h0, "set": {"physics.nu": 0.02}, "reason": "x".repeat(501)}),
            -32602,
        ),
        (
            json!({"path": REL, "base_hash": h0, "set": {"physics.nu": 0.02}, "reason": 7}),
            -32602,
        ),
    ];
    for (args, code) in refused {
        let e = call(&ws, "edit_case", args.clone()).unwrap_err();
        assert_eq!(e.code, code, "{args}: {}", e.message);
        assert_eq!(std::fs::read(root.join(REL)).unwrap(), CAVITY.as_bytes(), "{args}");
    }
    let e = call(
        &ws,
        "new_case",
        json!({"name": "bad name", "template": "lid_driven_cavity", "reason": why}),
    )
    .unwrap_err();
    assert_eq!(e.code, 1004);
    assert!(
        journal(&root).is_empty(),
        "a refused call wrote a line: {:?}",
        journal(&root)
    );
    // edit_case: one edit line with seq 1, the actor, each setting with its old and new value, and the reason
    let r = call(
        &ws,
        "edit_case",
        json!({"path": REL, "base_hash": h0, "set": {"physics.nu": 0.02, "solver.max_iterations": 40}, "reason": why}),
    )
    .unwrap();
    assert_eq!(
        keys(&r),
        ["form", "new_hash", "old_hash", "path", "valid", "values", "written"],
        "edit_case's result keys"
    );
    // write_note with a blank reason; new_case with a data file; import_mesh twice
    let n = call(
        &ws,
        "write_note",
        json!({"path": "notes/a.md", "text": "hello", "reason": "  "}),
    )
    .unwrap();
    assert_eq!(keys(&n), ["created", "new_hash", "old_hash", "path"]);
    let c = call(
        &ws,
        "new_case",
        json!({"name": "profile_inlet", "example": "channel/profile_inlet.toml", "reason": "a profile case"}),
    )
    .unwrap();
    assert_eq!(keys(&c), ["bytes", "data_files", "from", "hash", "path"]);
    std::fs::create_dir_all(root.join("incoming")).unwrap();
    std::fs::copy(
        repo().join("crates/rcfd-io/tests/data/hex_v4.1_ascii.msh"),
        root.join("incoming/hex.msh"),
    )
    .unwrap();
    let m = call(
        &ws,
        "import_mesh",
        json!({"path": "incoming/hex.msh", "reason": "the box"}),
    )
    .unwrap();
    assert_eq!(m["copied"], true);
    let again = call(&ws, "import_mesh", json!({"path": "incoming/hex.msh"})).unwrap();
    assert_eq!(again["copied"], false);
    let j = journal(&root);
    for l in &j {
        eprintln!("C3-J line: {l}");
    }
    assert_eq!(j.len(), 4, "{j:#?}");
    let seqs: Vec<u64> = j.iter().map(|l| l["seq"].as_u64().unwrap()).collect();
    assert_eq!(seqs, [1, 2, 3, 4]);
    let actor = mcp_actor().to_json();
    assert!(
        j.iter().all(|l| l["actor"] == actor),
        "every line names the workspace's actor"
    );
    assert_eq!(actor["instance"]["pid"], std::process::id());
    // the edit
    assert_eq!(
        (j[0]["kind"].as_str(), j[0]["who"].as_str()),
        (Some("edit"), Some("edit_case"))
    );
    assert_eq!(j[0]["path"], REL);
    assert_eq!(
        (j[0]["old_hash"].clone(), j[0]["new_hash"].clone()),
        (json!(h0), r["new_hash"].clone())
    );
    assert_eq!(
        j[0]["changes"],
        json!([{"path": "physics.nu", "old": 0.01, "new": 0.02}, {"path": "solver.max_iterations", "old": 30, "new": 40}])
    );
    assert_eq!(j[0]["reason"], why);
    // the note: no changes, no reason (blank)
    assert_eq!(
        (j[1]["kind"].as_str(), j[1]["who"].as_str()),
        (Some("edit"), Some("write_note"))
    );
    assert!(
        j[1].get("changes").is_none() && j[1].get("reason").is_none(),
        "{}",
        j[1]
    );
    // the creation
    let csv = std::fs::read(root.join("cases/profile_inlet.csv")).unwrap();
    let case = std::fs::read(root.join("cases/profile_inlet.toml")).unwrap();
    assert_eq!(
        (j[2]["kind"].as_str(), j[2]["who"].as_str()),
        (Some("case_created"), Some("new_case"))
    );
    assert_eq!(j[2]["path"], "cases/profile_inlet.toml");
    assert_eq!(j[2]["hash"], hash_bytes(&case));
    assert_eq!(j[2]["bytes"], case.len());
    assert_eq!(j[2]["from"], "example channel/profile_inlet.toml");
    assert_eq!(
        j[2]["data_files"],
        json!([{"path": "cases/profile_inlet.csv", "hash": hash_bytes(&csv), "written": true}])
    );
    assert_eq!(j[2]["reason"], "a profile case");
    // the import (the second one wrote nothing and left no line)
    let mesh = rcfd_io::read_msh_file(root.join("incoming/hex.msh")).unwrap();
    assert_eq!(
        (j[3]["kind"].as_str(), j[3]["who"].as_str()),
        (Some("mesh_imported"), Some("import_mesh"))
    );
    assert_eq!(j[3]["path"], "meshes/hex.msh");
    assert_eq!(j[3]["format"], "gmsh");
    assert_eq!(j[3]["cells"], mesh.n_cells());
    assert_eq!(j[3]["faces"], mesh.n_faces());
    assert_eq!(j[3]["patches"], mesh.patches().len());
    assert_eq!(
        j[3]["files"][0]["hash"],
        hash_bytes(&std::fs::read(root.join("meshes/hex.msh")).unwrap())
    );
    // a workspace without an actor writes no actor field
    let plain = Workspace::open(&root).unwrap();
    let h1 = r["new_hash"].as_str().unwrap().to_string();
    call(
        &plain,
        "edit_case",
        json!({"path": REL, "base_hash": h1, "set": {"physics.nu": 0.03}}),
    )
    .unwrap();
    let last = journal(&root).pop().unwrap();
    assert_eq!(last["seq"], 5);
    assert!(last.get("actor").is_none() && last.get("reason").is_none(), "{last}");
    eprintln!(
        "C3-J: 5 lines, seq 1..5; refused calls, the dry run and the second import wrote none; actor {}",
        actor
    );
}

/// The text the Studio's editor saves for `CAVITY` with `edit` applied: the whole tree by `Case::to_toml`.
fn studio_bytes(edit: impl FnOnce(&mut Case)) -> Vec<u8> {
    let mut c = Case::parse_toml(CAVITY).unwrap();
    edit(&mut c);
    Case::from_json(&c.tree.to_json()).unwrap().to_toml().into_bytes()
}

#[test]
fn c3d_a_studio_save_lists_only_the_settings_it_changed() {
    let root = project("c3d");
    let studio = Origin::new("save_case", Some(Actor::this_process(STUDIO)), None);
    let save = studio_bytes(|c| c.set("mesh.nx", json!(10)).unwrap());
    // the saved text spells out every default: far longer than the file it replaces
    assert!(save.len() > 2 * CAVITY.len(), "{} vs {}", save.len(), CAVITY.len());
    let s = save_case_by(
        &root,
        REL,
        &hash_bytes(CAVITY.as_bytes()),
        &save,
        SaveForm::Whole,
        &studio,
    )
    .unwrap();
    let j = journal(&root);
    eprintln!("C3-D: the save's line: {}", j[0]);
    assert_eq!(j.len(), 1);
    assert_eq!(j[0]["who"], "save_case");
    assert_eq!(j[0]["actor"]["client"], STUDIO);
    assert_eq!(j[0]["new_hash"], s.new_hash);
    assert_eq!(j[0]["changes"], json!([{"path": "mesh.nx", "old": 16, "new": 10}]));
    // the free function names no actor
    let again = studio_bytes(|c| c.set("mesh.nx", json!(11)).unwrap());
    rcfd_workspace::save_case(&root, REL, &s.new_hash, &again, SaveForm::Whole).unwrap();
    let j = journal(&root);
    assert!(j[1].get("actor").is_none());
    assert_eq!(j[1]["changes"], json!([{"path": "mesh.nx", "old": 10, "new": 11}]));
}

#[test]
fn c3d_long_lists_are_cut_and_large_values_hashed() {
    let long: Vec<Value> = (0..120)
        .map(|i| rcfd_workspace::journal::change(&format!("s{i}"), Some(json!(i)), Some(json!(i + 1))))
        .collect();
    let mut line = serde_json::Map::new();
    put_list(&mut line, "changes", long);
    assert_eq!(line["changes"].as_array().unwrap().len(), MAX_CHANGES);
    assert_eq!(line["changes_total"], 120);
    let big = json!("x".repeat(1998)); // 2000 bytes of JSON text with its quotes
    assert_eq!(big.to_string().len(), 2000);
    let c = rcfd_workspace::journal::change("profile", Some(big.clone()), Some(json!(1)));
    assert_eq!(
        c["old"],
        json!({"bytes": 2000, "hash": hash_bytes(big.to_string().as_bytes())})
    );
    assert_eq!(c["new"], 1);
    let short = rcfd_workspace::journal::change("x", Some(json!("x".repeat(998))), None);
    assert_eq!(
        short["old"].as_str().unwrap().len(),
        998,
        "a value of 1000 bytes is kept"
    );
    assert_eq!(short["new"], Value::Null);
    eprintln!("C3-D cut: 120 entries kept 100 with changes_total 120; a 2000-byte value written as its size and hash");
}

/// Poll `feed` every `PERIOD` until `done` holds for the entries so far or `polls` polls were made; the entries and the
/// number of polls made.
fn poll_until(feed: &mut Feed, polls: usize, done: impl Fn(&[Change]) -> bool) -> (Vec<Change>, usize) {
    let mut all = Vec::new();
    for k in 1..=polls {
        std::thread::sleep(PERIOD);
        all.extend(feed.poll().unwrap().changes);
        if done(&all) {
            return (all, k);
        }
    }
    (all, polls)
}

#[test]
fn c3o_lines_written_before_f273_are_read_as_they_are() {
    let root = project("c3o");
    let mut feed = Feed::open(&root).unwrap();
    // byte for byte as main 5fdde2d writes them, then a line that is not JSON and a torn line
    let old = concat!(
        r#"{"kind":"edit","new_hash":"fnv64:00000000000000b2","old_hash":"fnv64:00000000000000a1","path":"cases/a.toml","t":"2026-11-05T10:00:00.000Z","who":"edit_case"}"#,
        "\n",
        r#"{"age_s":31.5,"kind":"stale_lock_broken","path":"cases/a.toml","t":"2026-11-05T10:00:01.000Z","who":"save_case"}"#,
        "\n",
        "this is not json\n",
        r#"{"kind":"edit","who":"torn"#
    );
    std::fs::create_dir_all(root.join(".rcfd")).unwrap();
    std::fs::write(root.join(".rcfd/edits.jsonl"), old).unwrap();
    let got = feed.poll().unwrap().changes;
    for c in &got {
        eprintln!("C3-O entry: {}", c.to_json());
    }
    assert_eq!(got.len(), 2);
    assert_eq!(
        (got[0].kind.as_str(), got[1].kind.as_str()),
        ("edit", "stale_lock_broken")
    );
    assert!(got
        .iter()
        .all(|c| c.source == JOURNAL && c.seq.is_none() && c.actor.is_none()));
    assert_eq!(got[0].other["new_hash"], "fnv64:00000000000000b2");
    assert_eq!(feed.skipped_lines(), 1);
    // a reader from the start gives the same, and leaves the torn line
    let mut r = Reader::at_start(&root);
    assert_eq!(r.read().len(), 2);
    assert_eq!(r.skipped(), 1);
    // the next append is numbered 1 and stands on its own line
    let ws = Workspace::open(&root).unwrap().with_actor(mcp_actor());
    call(&ws, "write_note", json!({"path": "notes/n.md", "text": "x"})).unwrap();
    let got = feed.poll().unwrap().changes;
    let j: Vec<&Change> = got.iter().filter(|c| c.source == JOURNAL).collect();
    assert_eq!(j.len(), 1, "{got:?}");
    assert_eq!((j[0].seq, j[0].who.as_deref()), (Some(1), Some("write_note")));
    // amendment 1: the torn line, closed by the append's newline, is now a complete line that is not JSON
    assert_eq!(feed.skipped_lines(), 2);
    assert_eq!(r.read().len(), 1);
    assert_eq!(r.skipped(), 2);
    eprintln!("C3-O: two old entries without seq or actor, one bad line skipped, the torn line left; next seq 1 on its own line, the torn line then skipped");
}

#[test]
fn c3f1_an_edit_is_explained_by_its_line_and_an_outside_write_is_external() {
    let root = project("c3f1");
    let ws = Workspace::open(&root).unwrap().with_actor(mcp_actor());
    let mut feed = Feed::open(&root).unwrap();
    let r = call(
        &ws,
        "edit_case",
        json!({"path": REL, "base_hash": hash_bytes(CAVITY.as_bytes()), "set": {"physics.nu": 0.02}, "reason": "why"}),
    )
    .unwrap();
    // polls for twice the wait: one journal entry, no external one
    let (got, _) = poll_until(&mut feed, 6, |_| false);
    assert_eq!(got.len(), 1, "{got:?}");
    let c = &got[0];
    eprintln!("C3-F (1): {}", c.text);
    assert_eq!(
        (c.source.as_str(), c.kind.as_str(), c.path.as_deref()),
        (JOURNAL, "edit", Some(REL))
    );
    assert_eq!(c.who.as_deref(), Some("edit_case"));
    assert_eq!(c.actor(), Some(mcp_actor()));
    assert_eq!(c.reason.as_deref(), Some("why"));
    assert_eq!(
        c.changes,
        Some(json!([{"path": "physics.nu", "old": 0.01, "new": 0.02}]))
    );
    assert_eq!(c.other["new_hash"], r["new_hash"]);
    assert!(
        c.text.contains("physics.nu 0.01 -> 0.02") && c.text.contains("why: why"),
        "{}",
        c.text
    );
    // (2) a plain write: external, path only, within 4 polls
    std::fs::create_dir_all(root.join("notes")).unwrap();
    std::fs::write(root.join("notes/x.md"), b"outside").unwrap();
    let (got, polls) = poll_until(&mut feed, 4, |c| c.iter().any(|c| c.source == EXTERNAL));
    eprintln!(
        "C3-F (2): external after {polls} polls of 250 ms: {:?}",
        got.iter().map(|c| &c.text).collect::<Vec<_>>()
    );
    assert_eq!(got.len(), 1, "{got:?}");
    let e = &got[0];
    assert_eq!((e.kind.as_str(), e.path.as_deref()), (EXTERNAL, Some("notes/x.md")));
    assert!(e.actor.is_none() && e.changes.is_none() && e.who.is_none() && e.other.is_empty());
    // the API of the workspace numbers the same entries
    let (all, cursor) = ws.changes(0).unwrap();
    assert_eq!(cursor as usize, all.len());
}

#[test]
fn c3f3_a_line_that_comes_after_the_watcher_saw_its_file_explains_it() {
    let root = project("c3f3");
    let mut feed = Feed::open(&root).unwrap();
    let new = CAVITY.replace("nu = 0.01", "nu = 0.05");
    rcfd_project::atomic::atomic_write(&root.join(REL), new.as_bytes()).unwrap();
    // the race of J6, made on purpose: the watcher sees the file before its journal line exists. The file's time is set
    // ahead so that the order of this test does not depend on how fast it runs.
    let f = std::fs::OpenOptions::new().write(true).open(root.join(REL)).unwrap();
    f.set_modified(SystemTime::now() + Duration::from_secs(30)).unwrap();
    drop(f);
    let first = feed.poll().unwrap();
    assert_eq!(first.paths, [REL]);
    assert!(first.changes.is_empty(), "{:?}", first.changes);
    rcfd_project::journal::append(
        &root,
        json!({"t": rcfd_project::time::now(), "kind": "edit", "who": "edit_case", "path": REL,
            "old_hash": hash_bytes(CAVITY.as_bytes()), "new_hash": hash_bytes(new.as_bytes())}),
    );
    let (got, _) = poll_until(&mut feed, 4, |_| false);
    eprintln!("C3-F (3): {:?}", got.iter().map(|c| &c.text).collect::<Vec<_>>());
    assert_eq!(got.len(), 1, "{got:?}");
    assert_eq!((got[0].source.as_str(), got[0].path.as_deref()), (JOURNAL, Some(REL)));
}

#[test]
fn c3f4_run_events_enter_by_kind_and_run_files_are_never_external() {
    let root = project("c3f4");
    let mut feed = Feed::open(&root).unwrap();
    let p = Project::open(&root).unwrap();
    let rec = p.create_run(REL, None, "test", json!({})).unwrap();
    let ev = |kind| RunEvent {
        t: rcfd_project::time::now(),
        kind,
    };
    p.append_event(
        &rec.id,
        &ev(EventKind::State {
            from: Some(RunState::Queued),
            to: RunState::Running,
            reason: None,
        }),
    )
    .unwrap();
    for i in 1..=2 {
        p.append_event(
            &rec.id,
            &ev(EventKind::Other {
                kind: "solver".into(),
                body: json!({"solver_seq": i, "solver": {"event": "start"}}),
            }),
        )
        .unwrap();
    }
    p.append_event(
        &rec.id,
        &ev(EventKind::Warning {
            message: "a warning".into(),
        }),
    )
    .unwrap();
    p.append_event(
        &rec.id,
        &ev(EventKind::Stop {
            reason: "converged".into(),
        }),
    )
    .unwrap();
    let (got, _) = poll_until(&mut feed, 6, |_| false);
    for c in &got {
        eprintln!("C3-F (4): {}", c.text);
    }
    let kinds: Vec<&str> = got.iter().map(|c| c.kind.as_str()).collect();
    assert_eq!(kinds, ["run_state", "run_state", "run_warning", "run_stop"]);
    assert!(got
        .iter()
        .all(|c| c.source == RUN && c.run.as_deref() == Some(rec.id.as_str())));
    assert_eq!(got[0].other["to"], "queued");
    assert_eq!(
        (got[1].other["from"].as_str(), got[1].other["to"].as_str()),
        (Some("queued"), Some("running"))
    );
    assert_eq!(got[0].other["case"], REL);
}

/// Every file under `dir` with its bytes.
fn files(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if e.file_type().is_ok_and(|t| t.is_dir()) {
                stack.push(p);
            } else {
                out.insert(
                    p.strip_prefix(dir).unwrap().to_string_lossy().into_owned(),
                    std::fs::read(&p).unwrap(),
                );
            }
        }
    }
    out
}

#[test]
fn c3f5_reading_writes_nothing_and_c3f6_old_entries_are_dropped() {
    let root = project("c3f5");
    let ws = Workspace::open(&root).unwrap().with_actor(mcp_actor());
    call(&ws, "write_note", json!({"path": "notes/a.md", "text": "a"})).unwrap();
    std::fs::write(root.join("notes/b.md"), b"b").unwrap();
    let before = files(&root);
    let mut feed = Feed::open(&root).unwrap();
    let t0 = Instant::now();
    for _ in 0..20 {
        feed.poll().unwrap();
        let _ = ws.changes(0).unwrap();
    }
    assert_eq!(files(&root), before, "a poll wrote to the project");
    eprintln!(
        "C3-F (5): 40 polls in {:.2} s, the project's {} files unchanged",
        t0.elapsed().as_secs_f64(),
        before.len()
    );
    // (6) more entries than are kept
    for i in 0..KEEP + 5 {
        rcfd_project::journal::append(&root, json!({"kind": "test", "who": "test", "i": i}));
    }
    let got = feed.poll().unwrap().changes;
    assert_eq!(got.len(), KEEP + 5);
    let (kept, dropped) = feed.since(0);
    assert_eq!((kept.len(), dropped), (KEEP, 5));
    assert_eq!(kept[0].n, 6);
    let (rest, dropped) = feed.since(feed.cursor() - 3);
    assert_eq!((rest.len(), dropped), (3, 0));
}

/// Added to C3-F (ADR 0188 addendum 1): an entry too large for a page of `read_feed` is returned without its setting
/// values and other fields, keeping who, the path and the reason (the line stays whole in the journal), and `next`
/// moves past it.
#[test]
fn c3f7_read_feed_returns_a_too_large_entry_without_its_values() {
    let root = project("c3f7");
    let ws = Workspace::open(&root).unwrap();
    let cursor = call(&ws, "read_feed", json!({})).unwrap()["cursor"].as_u64().unwrap();
    let big: Vec<Value> = (0..30)
        .map(|i| json!({"path": format!("s{i}"), "old": "x".repeat(990), "new": "y".repeat(990)}))
        .collect();
    rcfd_project::journal::append(
        &root,
        json!({"kind": "edit", "who": "test", "path": REL, "changes": big, "reason": "large"}),
    );
    rcfd_project::journal::append(&root, json!({"kind": "edit", "who": "test", "path": "notes/after.md"}));
    let f = call(&ws, "read_feed", json!({"since": cursor})).unwrap();
    let e = f["entries"].as_array().unwrap();
    eprintln!(
        "C3-F (7): {} bytes of result; first entry {}",
        f.to_string().len(),
        e[0]
    );
    assert_eq!(e.len(), 2, "{f}");
    assert!(f.to_string().len() <= rcfd_workspace::TEXT_BUDGET);
    assert!(e[0].get("changes").is_none(), "{}", e[0]);
    assert_eq!(e[0]["reason"], "large");
    assert!(e[0]["omitted"].as_str().unwrap().contains(".rcfd/edits.jsonl"));
    assert_eq!(
        (e[0]["kind"].as_str(), e[0]["path"].as_str()),
        (Some("edit"), Some(REL))
    );
    assert_eq!(f["next"], e[1]["n"]);
    assert_eq!(journal(&root)[0]["changes"].as_array().unwrap().len(), 30);
}
