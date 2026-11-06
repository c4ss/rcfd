//! ADR 0195 C5-F (P7, ADR 0188 Q7 (b)): `read_feed {from_seq}` replays the project journal from its start, so a reader
//! made later sees what happened before it; each entry is the one a live feed gave for the same line, without `n`.

use rcfd_project::records::hash_bytes;
use rcfd_project::Project;
use rcfd_workspace::feed::{Feed, JOURNAL};
use rcfd_workspace::journal::{Actor, MCP};
use rcfd_workspace::{Answer, RpcError, Workspace};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

const CAVITY: &str = r#"title = "f275 cavity"
[mesh]
generator = "cavity_2d"
nx = 16
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
max_iterations = 30
"#;
const REL: &str = "cases/a.toml";

/// A line as main 5fdde2d wrote it, before ADR 0188 (no `seq`, no actor).
const OLD_LINE: &str = r#"{"kind":"edit","new_hash":"fnv64:00000000000000b2","old_hash":"fnv64:00000000000000a1","path":"cases/a.toml","t":"2026-11-05T10:00:00.000Z","who":"edit_case"}"#;

fn project(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir()
        .join("rcfd-workspace-tests")
        .join(format!("f275-{name}-{}-{nanos}", std::process::id()));
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

fn sample(root: &Path) {
    std::fs::create_dir_all(root.join("incoming")).unwrap();
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../rcfd-io/tests/data/hex_v4.1_ascii.msh"),
        root.join("incoming/hex.msh"),
    )
    .unwrap();
}

#[test]
fn c5f_from_seq_replays_the_journal_as_the_live_feed_gave_it() {
    let root = project("c5f");
    std::fs::create_dir_all(root.join(".rcfd")).unwrap();
    std::fs::write(root.join(".rcfd/edits.jsonl"), format!("{OLD_LINE}\n")).unwrap();
    sample(&root);
    let mut live = Feed::open(&root).unwrap();
    let ws = Workspace::open(&root).unwrap().with_actor(Actor::this_process(MCP));
    let h0 = hash_bytes(CAVITY.as_bytes());
    let e1 = call(
        &ws,
        "edit_case",
        json!({"path": REL, "base_hash": h0, "set": {"physics.nu": 0.02}, "reason": "to reach Re 50"}),
    )
    .unwrap();
    call(&ws, "write_note", json!({"path": "notes/a.md", "text": "hello"})).unwrap();
    call(
        &ws,
        "new_case",
        json!({"name": "fresh", "template": "lid_driven_cavity", "reason": "a second case"}),
    )
    .unwrap();
    call(&ws, "import_mesh", json!({"path": "incoming/hex.msh"})).unwrap();
    call(
        &ws,
        "edit_case",
        json!({"path": REL, "base_hash": e1["new_hash"], "set": {"mesh.nx": 10}}),
    )
    .unwrap();
    let mut seen = Vec::new();
    for _ in 0..8 {
        seen.extend(live.poll().unwrap().changes.into_iter().filter(|c| c.source == JOURNAL));
        if seen.len() >= 5 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert_eq!(seen.len(), 5, "the live feed's journal entries: {seen:?}");
    let want: Vec<Value> = seen
        .iter()
        .map(|c| {
            let mut j = c.to_json();
            j.as_object_mut().unwrap().remove("n");
            j
        })
        .collect();
    // a workspace made afterwards replays the five numbered lines (the old line has no seq)
    let later = Workspace::open(&root).unwrap();
    let r = call(&later, "read_feed", json!({"from_seq": 1})).unwrap();
    let entries = r["entries"].as_array().unwrap();
    let seqs: Vec<u64> = entries.iter().map(|e| e["seq"].as_u64().unwrap()).collect();
    assert_eq!(seqs, [1, 2, 3, 4, 5]);
    assert!(entries.iter().all(|e| e.get("n").is_none()), "a replayed entry has n");
    assert_eq!(entries, &want, "replayed entries differ from the live feed's");
    assert_eq!((r["total"].clone(), r["next_seq"].clone()), (json!(5), Value::Null));
    assert_eq!(r["skipped_lines"], 0);
    assert_eq!(entries[0]["reason"], "to reach Re 50");
    let cursor = call(&later, "read_feed", json!({})).unwrap()["cursor"].clone();
    assert_eq!(r["cursor"], cursor);
    // a page of two from seq 3
    let p = call(&later, "read_feed", json!({"from_seq": 3, "limit": 2})).unwrap();
    let seqs: Vec<u64> = p["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["seq"].as_u64().unwrap())
        .collect();
    assert_eq!(seqs, [3, 4]);
    assert_eq!((p["total"].clone(), p["next_seq"].clone()), (json!(3), json!(5)));
    // past the end
    let z = call(&later, "read_feed", json!({"from_seq": 6})).unwrap();
    assert_eq!(
        (z["entries"].clone(), z["total"].clone(), z["next_seq"].clone()),
        (json!([]), json!(0), Value::Null)
    );
    // refused arguments
    for bad in [
        json!({"since": 0, "from_seq": 1}),
        json!({"from_seq": 0}),
        json!({"from_seq": "1"}),
    ] {
        let e = call(&later, "read_feed", bad.clone()).unwrap_err();
        assert_eq!(e.code, -32602, "{bad}: {}", e.message);
    }
    eprintln!(
        "C5-F: 5 lines replayed equal to the live feed's entries (the old line without seq left out); page 3..4 next_seq 5; past the end empty; 3 refusals"
    );
}
