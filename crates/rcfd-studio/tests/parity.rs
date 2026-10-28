//! ADR 0131 B9: the parity map of plan section 6 (docs/ui-restart-plan.md), one named test per item. Items that belong to a
//! later sub-wave are ignored `*_known_miss` tests that name it; they are not limits that were missed.

mod common;
#[macro_use]
mod vp;

use common::{b1_project, frame, frames_until, temp};
use rcfd_api::case::Case;
use rcfd_studio::case_edit::{CaseEditor, EditorState};
use rcfd_studio::layout::{self, Panel};
use rcfd_studio::schema_model::value_at;
use rcfd_studio::{Studio, StudioApp};
use serde_json::json;
use std::time::Duration;

const WAIT: Duration = Duration::from_secs(30);

/// Plan section 6, "open and save a case": through the drawn app, the Project panel's case opens in the editor, an edit is
/// saved by the worker and the file reads back with it.
#[test]
fn parity_open_and_save_a_case() {
    let root = temp("parity-open-save");
    b1_project(&root);
    let mut st = Studio::in_memory();
    st.open_project(&root);
    let mut app = StudioApp::new(st, None);
    let ctx = egui::Context::default();
    assert!(frames_until(&mut app, &ctx, WAIT, |a| a.st.browser().is_some()));
    app.st.open_case("cases/b.toml");
    assert!(frames_until(&mut app, &ctx, WAIT, |a| a.st.editor.is_some()));
    let schema = app.st.schema.clone();
    app.st
        .editor
        .as_mut()
        .unwrap()
        .edit_text(&schema, "physics.nu", "0.02")
        .unwrap();
    let _ = frame(&mut app, &ctx);
    app.st.save_case().unwrap();
    assert!(frames_until(&mut app, &ctx, WAIT, |a| a.st.editor_state()
        == Some(EditorState::Clean)));
    let text = std::fs::read_to_string(root.join("cases/b.toml")).unwrap();
    Case::parse_toml(&text).expect("the saved file parses");
    let back = CaseEditor::from_bytes(app.st.svc.clone(), "cases/b.toml", text.as_bytes()).unwrap();
    assert_eq!(value_at(&back.settings, "physics.nu"), Some(&json!(0.02)));
    assert_eq!(value_at(&back.settings, "mesh.nx"), Some(&json!(12)));
}

/// Plan section 6, "settings editor with schema validation": local range checks and the service's validation both refuse
/// with the path named; `Validate` reports a valid case.
#[test]
fn parity_settings_editor_with_schema_validation() {
    let root = temp("parity-validate");
    b1_project(&root);
    let mut st = Studio::in_memory();
    st.poll = None;
    st.open_project(&root);
    assert!(st.drain_until(WAIT, |s| s.browser().is_some()));
    st.open_case("cases/a.toml");
    assert!(st.drain_until(WAIT, |s| s.editor.is_some()));
    let schema = st.schema.clone();
    let ed = st.editor.as_mut().unwrap();
    assert!(ed.validate(), "{:?}", ed.diagnostics);
    assert!(ed
        .edit_text(&schema, "mesh.nx", "0")
        .unwrap_err()
        .starts_with("mesh.nx: "));
    assert!(ed
        .edit_text(&schema, "solver.algorithm", "nonsense")
        .unwrap_err()
        .starts_with("solver.algorithm: "));
    assert!(ed
        .set_value("physics.nu", json!("x"))
        .unwrap_err()
        .starts_with("physics.nu: "));
    assert_eq!(ed.state(), EditorState::Clean);
}

/// Plan section 6, "run with live residual and report monitors and run control" (48D, ADR 0133): through the drawn app
/// a run starts through `rcfd-runner`, the Plots panel shows its residuals and its `lid_drag` monitor while it runs, and
/// the last plotted monitor value is the solver's (`output/monitors.json`) bit for bit.
#[test]
fn parity_run_with_live_residual_and_report_monitors() {
    let mut p = common::RunnerProject::new("parity-live", 3);
    p.serve();
    let mut st = Studio::in_memory();
    st.runner_exe = p.runner.clone();
    st.open_project(&p.dir);
    let mut app = StudioApp::new(st, None);
    layout::float(&mut app.st.dock, Panel::Plots);
    let ctx = egui::Context::default();
    assert!(frames_until(&mut app, &ctx, WAIT, |a| a.st.browser().is_some()
        && a.st.runs.last_tick.is_some()));
    app.st.start_run(p.plan(3000, false)).unwrap();
    assert!(frames_until(&mut app, &ctx, Duration::from_secs(60), |_| p.ids().len() == 1));
    let id = p.ids()[0].clone();
    app.st.select_run(&id);
    // live: residuals and the monitor are plotted while the run is still running
    let live = frames_until(&mut app, &ctx, Duration::from_secs(120), |a| {
        a.st.runs
            .row(&id)
            .is_some_and(|r| r.state == rcfd_project::RunState::Running)
            && a.st.plots.series.get("res_u").is_some_and(|s| s.x.len() >= 10)
            && a.st.monitors.series.get("lid_drag").is_some_and(|s| s.x.len() >= 10)
    });
    assert!(live, "no live residual and monitor points while running");
    assert!(frames_until(&mut app, &ctx, Duration::from_secs(300), |a| a
        .st
        .runs
        .row(&id)
        .is_some_and(|r| r.state.is_terminal())));
    let last_step = p.status(&id).unwrap().step as f64;
    assert!(frames_until(&mut app, &ctx, Duration::from_secs(10), |a| a
        .st
        .monitors
        .series
        .get("lid_drag")
        .and_then(|s| s.x.last().copied())
        == Some(last_step)));
    let monitors: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(p.run_dir(&id).join("output/monitors.json")).unwrap()).unwrap();
    let recorded = monitors["series"]["lid_drag"]["y"]
        .as_array()
        .unwrap()
        .last()
        .unwrap()
        .as_f64()
        .unwrap();
    let plotted = *app.st.monitors.series["lid_drag"].y.last().unwrap();
    eprintln!("parity live monitors: {last_step} steps, last lid_drag {plotted:e} (monitors.json {recorded:e})");
    assert_eq!(plotted.to_bits(), recorded.to_bits());
}

/// Plan section 6, "2D field view with colour map, legend and hover" (48C, ADR 0132 integration amendment 1): through the
/// drawn app on the first adapter, a run's stored field of the 2D cavity is rendered, the legend's end labels are drawn,
/// and a pointer on face 0 draws the hover readout with that face's stored value.
#[test]
fn parity_2d_field_view_with_colour_map_legend_and_hover() {
    let a = &adapters_or_skip!()[0];
    let snaps = [(10, 0.1), (20, 0.2), (30, 0.3)];
    let (root, id, dir) = vp::store_project("parity-field", &snaps, vp::p_v4);
    let mut st = vp::studio_showing(&root, &id);
    st.viewport.set_gpu(rcfd_studio::viewport::ViewGpu::from_adapter(a));
    let mut app = StudioApp::new(st, None);
    let ctx = egui::Context::default();
    assert!(frames_until(&mut app, &ctx, WAIT, |a| a
        .st
        .viewport
        .gpu()
        .is_some_and(|g| g.renders >= 1)));
    let drawn = vp::texts(&vp::frame(&mut app, &ctx).shapes);
    let legend = app.st.viewport.legend().unwrap();
    for l in [&legend.labels[0], &legend.labels[legend.labels.len() - 1]] {
        assert!(drawn.contains(l), "legend label {l} not drawn");
    }
    // the pointer on the centroid of face 0
    let vpm = &app.st.viewport;
    let s = vpm.surface.clone().unwrap();
    let l = s.face(0);
    let c = l.iter().fold([0.0; 3], |a, &p| {
        [0, 1, 2].map(|d| a[d] + s.points[p][d] / l.len() as f64)
    });
    let (x, y) = vpm.pixel_of(c).unwrap();
    let rect = vpm.image_rect.unwrap();
    let ppp = ctx.pixels_per_point();
    let pos = egui::pos2(rect.min.x + x as f32 / ppp, rect.min.y + y as f32 / ppp);
    let _ = vp::frame_with(&mut app, &ctx, vec![egui::Event::PointerMoved(pos)]);
    let drawn = vp::texts(&vp::frame_with(&mut app, &ctx, vec![egui::Event::PointerMoved(pos)]).shapes);
    let stored = rcfd_tsstore::Reader::open(&dir).unwrap().read_field(2, "p").unwrap()[s.cell[0].unwrap()];
    let want = format!("value {stored}");
    assert!(
        drawn.iter().any(|t| t.starts_with(&want)),
        "no hover readout '{want}' among the drawn text (hover {:?})",
        app.st.viewport.hover
    );
    eprintln!(
        "parity 2D field view on {}: legend {:?}, hover '{}'",
        vp::adapter_name(a),
        legend.labels,
        app.st.viewport.hover.as_ref().unwrap().text()
    );
}

#[test]
#[ignore = "48E (ADR 0134): assistant access is the Claude Code sidebar; not in 48B"]
fn parity_assistant_access_known_miss() {
    panic!("the Claude Code chat arrives with 48E");
}

#[test]
#[ignore = "ADR 0131 B9: whether the tutorials that name the GUI are rewritten for the Studio or kept for the frozen GUI is the user's call at the parity step"]
fn parity_tutorials_name_the_gui_known_miss() {
    panic!("not decided in wave 48");
}
