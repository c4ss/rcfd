//! ADR 0147 (job 48M) process checks with `rcfd-runner` and `rcfd` of this profile next to the test's `rcfd-studio`
//! (`common::binaries`, as ADR 0141's tests): M4 (the case starter offers `channel/profile_inlet.toml` with its data file,
//! end to end through the Run button) and M7 (b) (a runner that exits early after Run shows its log line once).

mod common;
mod g4;
mod lj;
mod pv;

use rcfd_api::case::Case;
use rcfd_project::records::hash_bytes;
use rcfd_project::RunState;
use rcfd_studio::case_edit::EditorState;
use rcfd_studio::case_starter::{Example, EXAMPLES};
use rcfd_studio::case_starter_ui::StarterDialog;
use rcfd_studio::preview::Status;
use rcfd_studio::view_toggle::Side;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// ADR 0141 G1 (a)'s limit: the run selected 5.0 s after the click.
const SELECTED_S: f64 = 5.0;
/// The run (at most 2000 iterations of 480 cells) finished this long after the click.
const FINISHED_S: f64 = 60.0;

fn profile() -> &'static Example {
    EXAMPLES
        .iter()
        .find(|e| e.source == "channel/profile_inlet.toml")
        .expect("channel/profile_inlet.toml is not offered")
}

fn repo_file(source: &str) -> Vec<u8> {
    std::fs::read(pv::repo_root().join("cases").join(source)).unwrap()
}

/// M4: the case starter offers `channel/profile_inlet.toml` and copies its table, end to end (runner served). (e) The
/// dialog's line for it, drawn before anything exists: `creates cases/profile_inlet.toml and its data file
/// cases/profile_inlet.csv from the example channel/profile_inlet.toml`. (a) `create_case_from_example(profile_inlet,
/// "pi")` with the cavity open: within 30 s `cases/pi.toml` and `cases/profile_inlet.csv` hold the shipped bytes, both are
/// listed, the message is `created cases/pi.toml and its data file cases/profile_inlet.csv`, the editor holds the case
/// clean with the source's hash and `validate()` is true with no diagnostic, and the preview shows it on the Mesh side:
/// ready, 480 cells, 2D, the mesh hash of `build_mesh(Case::load(file))`. (b) Run clicked: the new run is selected within
/// 5.0 s, finished within 60 s (its pre-launch check read the table), and its store's mesh is the preview's (hash).
/// (c) A second case `pi2`: created, the data file used as it was (bytes and modification time unchanged), the message
/// says so. (d) The data file changed by hand, then `pi3`: refused before anything is written, the message names
/// `cases/profile_inlet.csv`; no `cases/pi3.toml`; the data file keeps its new bytes.
#[test]
fn m4_profile_inlet_from_the_starter_end_to_end() {
    let mut p = common::RunnerProject::new("m4", 3);
    p.serve();
    let ctx = egui::Context::default();
    let mut app = g4::app_on(&p, "cavity.toml", &ctx);
    let e = profile();
    assert_eq!(e.companions.len(), 1, "set-up");
    let data = format!("cases/{}", e.companions[0].name());
    assert_eq!(data, "cases/profile_inlet.csv", "set-up");
    // (e)
    let idx = EXAMPLES.iter().position(|x| x.source == e.source).unwrap();
    app.ui_state.starter = Some(StarterDialog {
        selected: idx,
        name: "profile_inlet".into(),
        typed: true,
        ..Default::default()
    });
    let line = "creates cases/profile_inlet.toml and its data file cases/profile_inlet.csv from the example \
                channel/profile_inlet.toml";
    let drawn = g4::texts(&common::frame(&mut app, &ctx));
    assert!(drawn.iter().any(|t| t == line), "(e) the dialog's line not drawn");
    app.ui_state.starter = None;
    // (a)
    let t0 = Instant::now();
    assert_eq!(app.st.create_case_from_example(e, "pi").unwrap(), "cases/pi.toml");
    let opened = common::frames_until(&mut app, &ctx, Duration::from_secs(30), |a| {
        a.st.editor.as_ref().is_some_and(|ed| ed.rel == "cases/pi.toml")
            && a.st.preview.case.as_deref() == Some("cases/pi.toml")
            && lj::settled(&a.st)
    });
    assert!(opened, "(a) not opened and previewed: {:?}", app.st.last_message());
    let dt = t0.elapsed().as_secs_f64();
    assert_eq!(
        std::fs::read(p.dir.join("cases/pi.toml")).unwrap(),
        repo_file(e.source),
        "(a) the case"
    );
    assert_eq!(
        std::fs::read(p.dir.join(&data)).unwrap(),
        repo_file(e.companions[0].source),
        "(a) the data file"
    );
    let cases = app.st.browser().unwrap().cases.clone();
    assert!(
        cases.contains(&"cases/pi.toml".to_string()) && cases.contains(&data),
        "(a) listed: {cases:?}"
    );
    let msg = "created cases/pi.toml and its data file cases/profile_inlet.csv";
    assert!(app.st.messages.iter().any(|m| m == msg), "(a) {:?}", app.st.messages);
    let ed = app.st.editor.as_mut().unwrap();
    assert_eq!(ed.state(), EditorState::Clean, "(a)");
    assert_eq!(ed.base_hash, hash_bytes(e.bytes()), "(a)");
    let ok = ed.validate();
    assert!(ok && ed.diagnostics.is_empty(), "(a) {:?}", ed.diagnostics);
    assert_eq!(app.st.preview.status, Status::Ready, "(a)");
    let b = lj::shown(&app.st);
    let want = rcfd_api::sim::build_mesh(&Case::load(p.dir.join("cases/pi.toml")).unwrap()).unwrap();
    assert_eq!((b.cells, b.two_d, b.mesh_hash), (480, true, want.content_hash()), "(a)");
    assert_eq!(app.st.view.side, Side::Mesh, "(a)");
    assert!(
        Arc::ptr_eq(app.st.viewport.surface.as_ref().unwrap(), &b.surface),
        "(a) not installed"
    );
    eprintln!(
        "M4 (a): cases/pi.toml and {data} written ({:.3} s), listed, '{msg}', editor clean and valid, preview 480 cells 2D",
        dt
    );
    // (b)
    let before = p.ids();
    let t = g4::click(&mut app, &ctx, "Run");
    let mut selected: Option<(String, f64)> = None;
    let end = t + Duration::from_secs(FINISHED_S as u64 + 30);
    let finished = loop {
        let _ = common::frame(&mut app, &ctx);
        if selected.is_none() {
            let ids = p.ids();
            if let Some(id) = ids.iter().find(|i| !before.contains(i)) {
                if app.st.selected_run.as_deref() == Some(id.as_str()) {
                    selected = Some((id.clone(), t.elapsed().as_secs_f64()));
                }
            }
        }
        if let Some((id, _)) = &selected {
            if p.status(id).is_some_and(|s| s.state.is_terminal()) {
                break Some(t.elapsed().as_secs_f64());
            }
        }
        if Instant::now() > end {
            break None;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let (id, sel_s) = selected.expect("(b) the run was not selected");
    let st = p.status(&id).unwrap();
    let fin_s = finished.expect("(b) the run did not end");
    let mut r = rcfd_tsstore::Reader::open(&p.run_dir(&id).join("fields")).expect("(b) the run's store");
    let stored = r.mesh().unwrap().content_hash();
    eprintln!(
        "M4 (b): run {id} selected {sel_s:.3} s after the click, {:?} at step {} {fin_s:.3} s after it; stored mesh \
         {stored:016x}, the preview's {:016x}",
        st.state, st.step, b.mesh_hash
    );
    assert!(sel_s <= SELECTED_S, "(b) selected after {sel_s:.3} s");
    assert_eq!(st.state, RunState::Finished, "(b) {:?}", st.reason);
    assert!(fin_s <= FINISHED_S, "(b) finished after {fin_s:.3} s");
    assert_eq!(stored, b.mesh_hash, "(b) the run's mesh");
    // (c)
    let path = p.dir.join(&data);
    let (bytes0, time0) = (
        std::fs::read(&path).unwrap(),
        std::fs::metadata(&path).unwrap().modified().unwrap(),
    );
    app.st.create_case_from_example(e, "pi2").unwrap();
    let msg2 = "created cases/pi2.toml; its data file cases/profile_inlet.csv was in the project already, with the \
                example's bytes";
    assert!(
        common::frames_until(&mut app, &ctx, Duration::from_secs(30), |a| a
            .st
            .messages
            .iter()
            .any(|m| m == msg2)),
        "(c) {:?}",
        app.st.last_message()
    );
    assert_eq!(
        std::fs::read(p.dir.join("cases/pi2.toml")).unwrap(),
        repo_file(e.source),
        "(c)"
    );
    assert_eq!(std::fs::read(&path).unwrap(), bytes0, "(c) the data file was rewritten");
    assert_eq!(
        std::fs::metadata(&path).unwrap().modified().unwrap(),
        time0,
        "(c) the data file was touched"
    );
    eprintln!("M4 (c): '{msg2}'; the data file unchanged (bytes and modification time)");
    // (d)
    let mine = b"y,u\n0,0\n1,0\n";
    std::fs::write(&path, mine).unwrap();
    app.st.create_case_from_example(e, "pi3").unwrap();
    assert!(
        common::frames_until(&mut app, &ctx, Duration::from_secs(30), |a| a
            .st
            .last_message()
            .is_some_and(|m| m.contains("cases/profile_inlet.csv exists with other content"))),
        "(d) {:?}",
        app.st.last_message()
    );
    let m = app.st.last_message().unwrap().to_string();
    assert!(!p.dir.join("cases/pi3.toml").exists(), "(d) cases/pi3.toml written");
    assert_eq!(std::fs::read(&path).unwrap(), mine, "(d) the data file changed");
    eprintln!("M4 (d): refused: '{m}'");
}

/// M4 control: the starter's route before (`starter_no_companions`, the case written alone) in a project served by a
/// runner: `cases/pa.toml` is written without a data file, the editor does not open it (the message names
/// `tables.inlet_u.file`), and a run of it fails at the runner's pre-launch check naming `tables.inlet_u.file`.
#[test]
fn m4_control_the_case_alone_does_not_open_or_run() {
    let mut p = common::RunnerProject::new("m4c", 3);
    p.serve();
    let mut st = pv::studio(&p.dir);
    st.view.switches.starter_no_companions = true;
    let n = st.messages.len();
    st.create_case_from_example(profile(), "pa").unwrap();
    assert!(
        st.drain_until(Duration::from_secs(30), |s| s
            .messages
            .iter()
            .skip(n)
            .any(|m| m.contains("tables.inlet_u.file"))),
        "control: the editor opened it: {:?}",
        st.messages
    );
    assert!(p.dir.join("cases/pa.toml").exists() && !p.dir.join("cases/profile_inlet.csv").exists());
    let opened = st.editor.as_ref().is_some_and(|e| e.rel == "cases/pa.toml");
    let id = p
        .client
        .start(&p.dir.join("cases/pa.toml"), &Default::default())
        .unwrap();
    let s = p.client.wait(&id, Duration::from_secs(60)).unwrap();
    let reason = s.reason.clone().unwrap_or_default();
    eprintln!("M4 control: editor opened {opened}; run {id} {:?}: {reason}", s.state);
    assert!(!opened, "control: opened");
    assert_eq!(s.state, RunState::Failed, "control: the run did not fail");
    assert!(reason.contains("tables.inlet_u.file"), "control: {reason}");
}

/// Occurrences of `needle` in `hay`.
fn count(hay: &str, needle: &str) -> usize {
    hay.matches(needle).count()
}

/// M7 (b): Run on a project whose runner exits before it holds the lock after the Studio's budgets check passed
/// (`.rcfd/runner.lock` is a folder, so `rcfd-runner serve` ends with `rcfd-runner: runner lock: ...`): within 5.0 s of
/// the click the start's error (the strip's line) holds the client's early-exit report (`... before it held the
/// project's lock; ...`) with the runner's line exactly once, and that text is drawn. Negative control: the route before
/// (`tail_always`): the line twice.
#[test]
fn m7_the_run_buttons_early_exit_shows_the_runner_line_once() {
    for always in [false, true] {
        let p = common::RunnerProject::new(if always { "m7bc" } else { "m7b" }, 3);
        std::fs::create_dir_all(p.dir.join(".rcfd/runner.lock")).unwrap();
        let ctx = egui::Context::default();
        let mut app = g4::app_with(&p, "cavity.toml", &ctx, |st| st.quick.switches.tail_always = always);
        let t = g4::click(&mut app, &ctx, "Run");
        let shown = common::frames_until(&mut app, &ctx, Duration::from_secs(30), |a| {
            a.st.quick.runner_error.is_some()
        });
        let dt = t.elapsed().as_secs_f64();
        assert!(shown, "no runner error: {:?}", app.st.last_message());
        let e = app.st.quick.runner_error.clone().unwrap();
        let n = count(&e, "rcfd-runner: runner lock: ");
        let drawn = g4::texts(&common::frame(&mut app, &ctx)).contains(&e);
        eprintln!(
            "M7 (b){}: {dt:.3} s after the click, the runner's line {n} time(s), drawn {drawn}: '{e}'",
            if always { " control" } else { "" }
        );
        if always {
            assert_eq!(n, 2, "control: {e}");
        } else {
            assert!(dt <= 5.0, "shown {dt:.3} s after the click");
            assert!(
                e.contains("before it held the project's lock; "),
                "not the early-exit report: {e}"
            );
            assert_eq!(n, 1, "{e}");
            assert!(drawn, "not drawn");
        }
    }
}
