//! ADR 0140 SF6 (a store at the pre-48A location `runs/<id>/output/fields/` is found) and SF7 (the viewport says why a
//! run has nothing to show, exact registered texts). Fixture runs without a runner.

#[macro_use]
mod vp;

use rcfd_project::{Project, RunState, RunStatus};
use rcfd_studio::run_fields::{EmptyKind, EmptyState, Explainer, NOT_STORED_HINT};
use rcfd_studio::viewport::{Status, StoreView};
use rcfd_studio::Studio;
use rcfd_tsstore::{Location, Precision, WriteOptions, Writer};
use serde_json::json;
use std::path::{Path, PathBuf};

const CAVITY: &str = include_str!("fixtures/cavity.toml");

/// R10 (f)'s moving-mesh piston (`crates/rcfd-api/tests/dynamic_mesh.rs`).
const PISTON: &str = "[mesh]\ngenerator = \"box_2d\"\nnx = 20\nny = 2\nlx = 1.0\nly = 0.2\n[physics]\nnu = 0.01\nrho = 1.0\n[boundaries.xmin]\nvelocity = \"fixed\"\n[boundaries.xmax]\nvelocity = \"zero_gradient\"\npressure = \"fixed\"\npressure_value = 0.0\n[boundaries.ymin]\nvelocity = \"zero_gradient\"\n[boundaries.ymax]\nvelocity = \"zero_gradient\"\n[solver]\nalgorithm = \"piso\"\nconvection = \"central\"\ndt = 0.005\nend_time = 0.5\n\n[dynamic_mesh]\nenabled = true\nmoving_patches = [\"xmin\"]\ndeformation = \"smoothing\"\ndisplacement_expression_x = \"1 [m/s^2] * t^2\"\nfixed_patches = [\"xmax\"]\n";

const LEGACY: &str = "r-20261028T010219-cc06";

fn legacy_src() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../rcfd-project/tests/fixtures/legacy_runner_1248a76")
        .join(LEGACY)
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap().map(|e| e.unwrap()) {
        std::fs::copy(e.path(), to.join(e.file_name())).unwrap();
    }
}

/// A project (canonical root) with `cases/cavity.toml` and the other fixture cases.
fn project(name: &str) -> (PathBuf, Project) {
    let root = vp::temp(name).join("proj");
    let p = Project::create(&root, name, "rcfd test").unwrap();
    std::fs::write(root.join("cases/cavity.toml"), CAVITY).unwrap();
    std::fs::write(
        root.join("cases/off.toml"),
        format!("{CAVITY}[output]\nstore_interval = 0\n"),
    )
    .unwrap();
    std::fs::write(root.join("cases/piston.toml"), PISTON).unwrap();
    std::fs::write(
        root.join("cases/chain.toml"),
        format!("{CAVITY}[chain]\nsequence = [\"coarse\", \"fine\"]\n"),
    )
    .unwrap();
    let root = rcfd_project::paths::canonicalize(&root).unwrap();
    (root, p)
}

fn run(p: &Project, case: &str, set: &[&str], state: RunState, reason: Option<&str>) -> String {
    let rec = p
        .create_run(
            case,
            None,
            "rcfd test",
            json!({"priority": 0, "threads": 1, "mem_mb": null, "set": set}),
        )
        .unwrap();
    if state != RunState::Queued {
        let st = RunStatus {
            reason: reason.map(String::from),
            ..RunStatus::new(state)
        };
        p.write_status(&rec.id, &st).unwrap();
    }
    rec.id
}

fn looked(root: &Path, id: &str) -> String {
    let d = root.join("runs").join(id);
    format!(
        "Looked for {} and {}.",
        d.join("fields").join("store.rts").display(),
        d.join("output").join("fields").join("store.rts").display()
    )
}

/// The 48C viewport's one text (copied; SF7's negative control).
fn text_48c(root: &Path, id: &str) -> String {
    format!(
        "no stored fields: {} not found (start the run with \"Store fields\" on)",
        root.join("runs").join(id).join("fields").join("store.rts").display()
    )
}

const RERUN_CHANGED_TAIL: &str =
    "changed since this run, so it is not re-run; start a new run with Runs > Start run... (\"Store fields\" is on by default).";
const RERUN_MISSING_TAIL: &str =
    "is not found, so it is not re-run; start a new run with Runs > Start run... (\"Store fields\" is on by default).";

struct Want {
    kind: EmptyKind,
    text: String,
    hint: Option<&'static str>,
    rerun: bool,
    rerun_line: Option<String>,
}

fn check(name: &str, got: &EmptyState, w: &Want, root: &Path, id: &str) {
    eprintln!("SF7 {name}: {:?}: {}", got.kind, got.text);
    assert_eq!(got.kind, w.kind, "{name}");
    assert_eq!(got.text, w.text, "{name}");
    assert_eq!(got.hint.as_deref(), w.hint, "{name}");
    assert_eq!(got.rerun, w.rerun, "{name}");
    assert_eq!(got.rerun_line, w.rerun_line, "{name}");
    assert_eq!(got.looked, looked(root, id), "{name}");
    assert_eq!(got.message(), format!("{}\n{}", w.text, looked(root, id)), "{name}");
}

fn studio_on(root: &Path) -> Studio {
    let mut st = Studio::in_memory();
    st.poll = None;
    st.run_poll = None;
    st.open_project(root);
    assert!(st.drain_until(vp::WAIT, |s| s.browser().is_some()));
    st
}

#[test]
fn sf7_empty_states_exact_texts() {
    let (root, p) = project("sf7");
    let s4 = ["output.store_interval=4"];
    let mut fixtures: Vec<(&str, String, Want, bool)> = Vec::new();
    let mut add = |name: &'static str, id: String, kind: EmptyKind, text: String, store: bool| {
        fixtures.push((
            name,
            id,
            Want {
                kind,
                text,
                hint: None,
                rerun: false,
                rerun_line: None,
            },
            store,
        ));
    };
    let id = run(&p, "cases/cavity.toml", &s4, RunState::Queued, None);
    add(
        "queued",
        id.clone(),
        EmptyKind::Queued,
        format!("Run {id} has not started yet. Its fields appear here once it stores its first snapshot."),
        false,
    );
    let id = run(&p, "cases/cavity.toml", &s4, RunState::Paused, None);
    add(
        "paused",
        id.clone(),
        EmptyKind::Paused,
        format!("Run {id} is paused and has not stored a snapshot yet. Its fields appear here once it stores one."),
        false,
    );
    let id = run(&p, "cases/cavity.toml", &s4, RunState::Running, None);
    add(
        "running",
        id.clone(),
        EmptyKind::Waiting,
        format!("Run {id} is running and stores its fields every 4 iterations. Waiting for the first snapshot."),
        false,
    );
    let id = run(&p, "cases/cavity.toml", &[], RunState::Running, None);
    add(
        "not stored",
        id.clone(),
        EmptyKind::NotStored,
        format!("Run {id} was started without storing fields, so there is nothing to show."),
        false,
    );
    let id = run(&p, "cases/off.toml", &[], RunState::Finished, None);
    add(
        "turned off by the case",
        id.clone(),
        EmptyKind::TurnedOff,
        format!(
            "Run {id} was started with the field store turned off (output.store_interval = 0), so there is nothing to \
             show."
        ),
        false,
    );
    let id = run(&p, "cases/cavity.toml", &s4, RunState::Failed, Some("solver exploded"));
    add(
        "failed",
        id.clone(),
        EmptyKind::Failed,
        format!("Run {id} failed before it stored its first snapshot: solver exploded"),
        false,
    );
    let id = run(
        &p,
        "cases/cavity.toml",
        &s4,
        RunState::Cancelled,
        Some("cancelled before launch"),
    );
    add(
        "cancelled",
        id.clone(),
        EmptyKind::Cancelled,
        format!("Run {id} was cancelled before it stored its first snapshot."),
        false,
    );
    let id = run(&p, "cases/cavity.toml", &s4, RunState::Failed, Some("killed"));
    {
        // a header-only store: created, never committed a snapshot
        let w = Writer::create(
            &root.join("runs").join(&id).join("fields"),
            &vp::cavity8(),
            WriteOptions::default(),
        )
        .unwrap();
        drop(w);
    }
    add(
        "failed, header-only store",
        id.clone(),
        EmptyKind::Failed,
        format!("Run {id} failed before it stored its first snapshot: killed"),
        true,
    );
    let id = run(&p, "cases/cavity.toml", &s4, RunState::Finished, None);
    let warning = "field store disabled: output.store_fields: unknown field 'nosuch'";
    {
        let ev = json!({"t": "2026-10-28T12:00:00.000Z", "kind": "solver", "event": "solver", "solver_seq": 2,
            "solver": {"format": "rcfd-status", "version": 1, "event": "warning", "seq": 2, "wall_s": 0.0,
                       "message": warning}});
        let path = root.join("runs").join(&id).join("events.jsonl");
        let mut text = std::fs::read_to_string(&path).unwrap();
        text.push_str(&format!("{ev}\n"));
        std::fs::write(&path, text).unwrap();
    }
    add(
        "store refused by the solver",
        id.clone(),
        EmptyKind::Disabled,
        format!("The solver did not store fields for run {id}: {warning}"),
        false,
    );
    let id = run(&p, "cases/piston.toml", &[], RunState::Finished, None);
    add(
        "moving mesh",
        id.clone(),
        EmptyKind::MovingMesh,
        format!(
            "Run {id} uses a moving mesh (dynamic_mesh.enabled), which the field store does not support, so there is \
             nothing to show."
        ),
        false,
    );
    let id = run(&p, "cases/chain.toml", &s4, RunState::Finished, None);
    add(
        "chain",
        id.clone(),
        EmptyKind::Chain,
        format!(
            "Run {id} is a chain run: each stage stores its fields in runs/{id}/output/<stage>/fields/, which the \
             Viewport does not read yet."
        ),
        false,
    );
    let id = run(&p, "cases/cavity.toml", &s4, RunState::Finished, None);
    add(
        "missing",
        id.clone(),
        EmptyKind::Missing,
        format!("Run {id} asked for stored fields, but no store was found."),
        false,
    );
    copy_dir(&legacy_src(), &root.join("runs").join(LEGACY));
    // amendment 1: the fixture's case_source names a folder of the 48A host, which may exist; point it at a path that
    // does not, so that "case not found" holds on every host (the record keeps its legacy form)
    let gone = root.join("gone").join("cavity.toml");
    {
        let path = root.join("runs").join(LEGACY).join("run.json");
        let mut rec: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        rec["case_source"] = json!(gone.display().to_string());
        rec["case_dir"] = json!(gone.parent().unwrap().display().to_string());
        assert!(rec.get("case").is_none(), "the legacy record has no case key");
        std::fs::write(&path, serde_json::to_vec_pretty(&rec).unwrap()).unwrap();
    }
    add(
        "legacy, not stored",
        LEGACY.to_string(),
        EmptyKind::NotStored,
        format!("Run {LEGACY} was started without storing fields, so there is nothing to show."),
        false,
    );
    // the re-run lines
    let legacy_case = gone.display().to_string();
    for (name, _, w, _) in fixtures.iter_mut() {
        match *name {
            "not stored" => {
                w.hint = Some(NOT_STORED_HINT);
                w.rerun = true;
                w.rerun_line = Some(
                    "Starts a new run of cases/cavity.toml (unchanged since this run) with the field store on.".into(),
                );
            }
            "turned off by the case" => {
                w.rerun = true;
                w.rerun_line = Some(
                    "Starts a new run of cases/off.toml (unchanged since this run) with the field store on.".into(),
                );
            }
            "legacy, not stored" => {
                w.hint = Some(NOT_STORED_HINT);
                w.rerun_line = Some(format!("{legacy_case} {RERUN_MISSING_TAIL}"));
            }
            _ => {}
        }
    }
    assert_eq!(fixtures.len(), 13);
    for (name, id, w, store) in &fixtures {
        let got = Explainer::default().explain(&root, id, *store);
        check(name, &got, w, &root, id);
    }
    // the hint text as registered
    assert_eq!(
        NOT_STORED_HINT,
        "Runs started from Claude Code or the command line store fields only when the case or an override sets \
         output.store_interval or output.store_time_interval."
    );
    // through the Studio model's viewport (Status::NoStore holds the main text and the "Looked for" line)
    let mut st = studio_on(&root);
    for (name, id, w, _) in fixtures
        .iter()
        .filter(|f| matches!(f.0, "queued" | "not stored" | "failed" | "moving mesh"))
    {
        st.select_run(id);
        assert!(
            st.drain_until(vp::WAIT, |s| matches!(&s.viewport.status, Status::NoStore(_))
                && s.viewport.empty.is_some()),
            "{name}: {:?}",
            st.viewport.status
        );
        let Status::NoStore(m) = &st.viewport.status else {
            unreachable!()
        };
        eprintln!("SF7 model {name}: {m:?}");
        assert_eq!(*m, format!("{}\n{}", w.text, looked(&root, id)), "{name}");
        check(name, st.viewport.empty.as_ref().unwrap(), w, &root, id);
    }
    // the header-only store through the model: Ready with no snapshot, the state's text
    let (name, id, w, _) = fixtures.iter().find(|f| f.0 == "failed, header-only store").unwrap();
    st.select_run(id);
    assert!(st.drain_until(vp::WAIT, |s| s.viewport.status == Status::Ready
        && s.viewport.empty.is_some()));
    let got = st.viewport.empty.as_ref().unwrap();
    eprintln!(
        "SF7 model {name}: Ready, {} snapshots: {}",
        st.viewport.snaps.len(),
        got.text
    );
    assert!(st.viewport.snaps.is_empty());
    assert_eq!(got.text, w.text);
    // the case-changed variant of the re-run line
    let id = run(&p, "cases/cavity.toml", &[], RunState::Finished, None);
    std::fs::write(root.join("cases/cavity.toml"), format!("{CAVITY}# edited\n")).unwrap();
    let got = Explainer::default().explain(&root, &id, false);
    eprintln!("SF7 case changed: {:?}", got.rerun_line);
    assert!(!got.rerun);
    assert_eq!(got.rerun_line, Some(format!("cases/cavity.toml {RERUN_CHANGED_TAIL}")));
    // negative control: the 48C text is one text for every state and equals none of the registered ones
    let mut differ = 0;
    for (_, id, w, _) in &fixtures {
        let old = text_48c(&root, id);
        assert!(old.starts_with("no stored fields: "));
        if old != w.text && old != format!("{}\n{}", w.text, looked(&root, id)) {
            differ += 1;
        }
    }
    eprintln!(
        "SF7 control: the 48C text differs from {differ} of {} registered texts",
        fixtures.len()
    );
    assert_eq!(differ, fixtures.len());
}

/// SF6: the 12 x 12 cavity, `p` and `ux` at `n` snapshots (steps 10, 20, ...).
fn write_store(dir: &Path, n: usize) {
    let mesh = rcfd_mesh::gen::cavity_2d(12, 1.0).unwrap();
    std::fs::create_dir_all(dir).unwrap();
    let mut w = Writer::create(dir, &mesh, WriteOptions::default()).unwrap();
    let cells = mesh.n_cells();
    for k in 0..n {
        let p: Vec<f64> = (0..cells).map(|c| vp::p_v4(c, k)).collect();
        let ux: Vec<f64> = (0..cells).map(|c| (0.11 * c as f64).cos() * k as f64).collect();
        let mut s = w.snapshot(10 * (k as u64 + 1), 0.0).unwrap();
        s.field("p", "unknown", Location::Cell, &p, Precision::F64).unwrap();
        s.field("ux", "m/s", Location::Cell, &ux, Precision::F64).unwrap();
        s.commit().unwrap();
    }
    w.finish().unwrap();
}

#[test]
fn sf6_store_at_the_pre_48a_location_is_found() {
    let (root, _) = project("sf6");
    let run_dir = root.join("runs").join(LEGACY);
    copy_dir(&legacy_src(), &run_dir);
    let legacy = run_dir.join("output").join("fields");
    write_store(&legacy, 3);
    // negative control: the pre-change lookup (only runs/<id>/fields/store.rts) finds nothing here
    let pre = run_dir.join("fields").join(rcfd_tsstore::STORE_FILE).is_file();
    eprintln!("SF6 control: runs/<id>/fields/store.rts exists: {pre}");
    assert!(!pre);
    let mut st = studio_on(&root);
    st.select_run(LEGACY);
    assert!(
        st.drain_until(vp::WAIT, |s| s.viewport.frame.is_some() && !s.viewport.pending()),
        "{:?}",
        st.viewport.status
    );
    assert_eq!(st.viewport.status, Status::Ready);
    assert_eq!(st.viewport.snaps.len(), 3);
    let note = format!(
        "Fields read from {}: runs made before wave 48A keep their store there.",
        legacy.display()
    );
    eprintln!("SF6: Ready, 3 snapshots; note {:?}", st.viewport.store_note);
    assert_eq!(st.viewport.store_note.as_deref(), Some(note.as_str()));
    // values bit-equal to the stored ones mapped to faces (as V4)
    let sv = StoreView::open(&legacy).unwrap();
    let mut r = rcfd_tsstore::Reader::open(&legacy).unwrap();
    for k in 0..3 {
        vp::show(&mut st, k);
        let want = sv
            .surface
            .face_values_from_cells(&r.read_field(k, "p").unwrap())
            .unwrap();
        let got = &st.viewport.frame.as_ref().unwrap().faces;
        assert_eq!(got.len(), want.len());
        assert!(
            got.iter().zip(&want).all(|(a, b)| a.to_bits() == b.to_bits()),
            "snapshot {k}"
        );
    }
    eprintln!("SF6: 3 snapshots x {} faces bit-equal", sv.surface.n_faces());
    // the contract location wins over the legacy one
    write_store(&run_dir.join("fields"), 2);
    let mut st = studio_on(&root);
    st.select_run(LEGACY);
    assert!(st.drain_until(vp::WAIT, |s| s.viewport.frame.is_some() && !s.viewport.pending()));
    eprintln!(
        "SF6: with both stores: {} snapshots, note {:?}",
        st.viewport.snaps.len(),
        st.viewport.store_note
    );
    assert_eq!(st.viewport.snaps.len(), 2);
    assert_eq!(st.viewport.store_note, None);
}
