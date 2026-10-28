//! ADR 0144 (job 48J) process check with a real runner (`rcfd-runner` and `rcfd` of this profile next to the test's
//! `rcfd-studio`, as ADR 0141's tests): J2, starting a run switches the Viewport to Run, a case click switches it back to
//! Mesh, and Run still refuses a case with unsaved edits.

mod common;
mod g4;
mod lj;
mod pv;

use g4::{app_on, click, project, steps};
use rcfd_studio::view_toggle::Side;
use rcfd_studio::StudioApp;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// ADR 0141 G1 (a)'s limit: the run selected 5.0 s after the click.
const SELECTED_S: f64 = 5.0;

fn mesh_shown(app: &StudioApp) -> bool {
    app.st.view.side == Side::Mesh
        && app.st.viewport.run.is_none()
        && app.st.viewport.surface.as_ref().is_some_and(|s| {
            app.st
                .preview
                .shown
                .as_ref()
                .is_some_and(|b| Arc::ptr_eq(s, &b.surface))
        })
}

/// Click Run and wait until a new run (`n` in all) is queued, selected and shown on the Run side; the seconds from the
/// click's release and the run's id. The new run is the id that was not there before the click (amendment 5: ids of the
/// same second sort by their random suffix, so the last id in name order need not be the newest).
fn run_and_follow(app: &mut StudioApp, ctx: &egui::Context, p: &common::RunnerProject, n: usize) -> (f64, String) {
    let before = p.ids();
    let t0 = click(app, ctx, "Run");
    let deadline = t0 + Duration::from_secs(60);
    loop {
        let _ = common::frame(app, ctx);
        let ids = p.ids();
        let new: Vec<&String> = ids.iter().filter(|i| !before.contains(i)).collect();
        if ids.len() == n && new.len() == 1 {
            let id = new[0].clone();
            let st = &app.st;
            if st.selected_run.as_deref() == Some(id.as_str())
                && st.view.side == Side::Run
                && st.viewport.run.as_deref() == Some(id.as_str())
            {
                return (t0.elapsed().as_secs_f64(), id);
            }
        }
        assert!(Instant::now() < deadline, "the run was not selected within 60 s");
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// J2: the 12 x 12 lid cavity (300 steps), runner served directly (ADR 0128 rule). (a) Opened: the Mesh side shows its
/// preview. (b) Run clicked: the new run is selected and shown on the Run side (`view.side`, `viewport.run`) within 5.0 s
/// of the click. (c) The case clicked again (the Project panel's call): Mesh, the preview shown, the run still
/// selected. (d) Run clicked again: the second run selected and shown on the Run side within 5.0 s. (e) A committed
/// unsaved edit: Run is refused with "save it first" (ADR 0141, unchanged); a field still being typed is reported.
/// Negative control: with ADR 0141's auto-select off (`no_select`) a third Run leaves the Mesh side 5.0 s after the run
/// is queued.
#[test]
fn j2_a_run_start_switches_to_run_and_a_case_click_back_to_mesh() {
    let p = project("j2", "live.toml", &g4::cavity(steps(300)), true);
    let ctx = egui::Context::default();
    let mut app = app_on(&p, "live.toml", &ctx);
    // (a)
    assert!(
        common::frames_until(&mut app, &ctx, lj::WAIT, mesh_shown),
        "(a) the preview was not shown"
    );
    // (b)
    let (dt1, id1) = run_and_follow(&mut app, &ctx, &p, 1);
    // (c)
    app.st.open_case("cases/live.toml");
    assert!(
        common::frames_until(&mut app, &ctx, lj::WAIT, mesh_shown),
        "(c) the case click did not show the mesh"
    );
    assert_eq!(
        app.st.selected_run.as_deref(),
        Some(id1.as_str()),
        "(c) the run was deselected"
    );
    // (d)
    assert!(
        common::frames_until(&mut app, &ctx, lj::WAIT, |a| rcfd_studio::quick_run::blocked(&a.st)
            .is_none()),
        "(d) Run stayed blocked: {:?}",
        rcfd_studio::quick_run::blocked(&app.st)
    );
    let (dt2, id2) = run_and_follow(&mut app, &ctx, &p, 2);
    eprintln!(
        "J2: run {id1} selected and shown {dt1:.3} s after the click; Mesh after the case click; run {id2} {dt2:.3} s"
    );
    assert!(
        dt1 <= SELECTED_S && dt2 <= SELECTED_S,
        "selected after {dt1:.3} / {dt2:.3} s"
    );
    assert_ne!(id1, id2);
    // (e) Run and unsaved edits (unchanged): a committed edit is refused; a field being typed is reported
    lj::type_text(&mut app.st, "mesh.nx", "13");
    let typing = rcfd_studio::quick_run::blocked(&app.st);
    let schema = app.st.schema.clone();
    app.st
        .editor
        .as_mut()
        .unwrap()
        .edit_text(&schema, "mesh.nx", "13")
        .unwrap();
    let committed = rcfd_studio::quick_run::blocked(&app.st);
    eprintln!("J2 (e): Run with a field being typed: {typing:?}; with the edit committed: {committed:?}");
    assert!(
        committed.as_deref().is_some_and(|m| m.contains("save it first")),
        "(e) {committed:?}"
    );
    // control: no auto-select, no switch
    app.st
        .editor
        .as_mut()
        .unwrap()
        .edit_text(&schema, "mesh.nx", "12")
        .unwrap();
    app.st.quick.switches.no_select = true;
    app.st.open_case("cases/live.toml");
    assert!(common::frames_until(&mut app, &ctx, lj::WAIT, mesh_shown));
    assert!(common::frames_until(&mut app, &ctx, lj::WAIT, |a| {
        rcfd_studio::quick_run::blocked(&a.st).is_none()
    }));
    let _ = click(&mut app, &ctx, "Run");
    assert!(
        common::frames_until(&mut app, &ctx, Duration::from_secs(60), |_| p.ids().len() == 3),
        "control: not queued"
    );
    let t = Instant::now();
    let switched = common::frames_until(&mut app, &ctx, Duration::from_secs(5), |a| a.st.view.side == Side::Run);
    eprintln!(
        "J2 control (no auto-select): switched to Run within 5 s {switched} ({:.3} s); side {:?}, viewport run {:?}",
        t.elapsed().as_secs_f64(),
        app.st.view.side,
        app.st.viewport.run
    );
    assert!(
        !switched && app.st.viewport.run.is_none(),
        "control: switched without the auto-select"
    );
    for id in p.ids() {
        p.client.cancel(&id).ok();
    }
}
