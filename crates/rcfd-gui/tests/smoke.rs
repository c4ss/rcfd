//! Headless smoke tests: the real UI code runs frames without a window, and the assistant sidebar
//! drives a full turn with the mock provider. Nothing here checks how the app looks.

use rcfd_assistant::mock::MockProvider;
use rcfd_assistant::ChatResponse;
use rcfd_gui::assistant_setup::AssistantSetup;
use rcfd_gui::chat::{CardState, Entry};
use rcfd_gui::controller::Controller;
use rcfd_gui::layout::{self, Panel};
use rcfd_gui::schema_model::value_at;
use rcfd_gui::state::AppState;
use rcfd_gui::theme::Theme;
use rcfd_gui::RcfdApp;
use serde_json::json;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn app() -> RcfdApp {
    let dir = std::env::temp_dir().join("rcfd-gui-smoke-cases");
    let mut ctrl = Controller::new(None, dir);
    ctrl.new_from_template().unwrap();
    RcfdApp::new(AppState::new(ctrl, AssistantSetup::in_memory(), Theme::Light))
}

fn frame(app: &mut RcfdApp, ctx: &egui::Context) {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 800.0))),
        ..Default::default()
    };
    let _ = ctx.run_ui(input, |ui| app.ui(ui));
}

#[test]
fn frames_render_and_theme_switches() {
    let mut a = app();
    let ctx = egui::Context::default();
    for _ in 0..4 {
        frame(&mut a, &ctx);
    }
    assert!(!ctx.global_style().visuals.dark_mode);
    a.st.theme = Theme::Dark;
    frame(&mut a, &ctx);
    assert!(ctx.global_style().visuals.dark_mode);
}

#[test]
fn every_panel_can_be_hidden_floated_and_shown_while_drawing() {
    let mut a = app();
    let ctx = egui::Context::default();
    frame(&mut a, &ctx);
    for p in Panel::ALL {
        layout::hide(&mut a.dock, p);
        frame(&mut a, &ctx);
        layout::show(&mut a.dock, p);
        frame(&mut a, &ctx);
        layout::float(&mut a.dock, p);
        frame(&mut a, &ctx);
        assert_eq!(layout::count(&a.dock, p), 1);
    }
}

#[test]
fn run_from_the_controller_draws_monitors_and_viewport() {
    let mut a = app();
    let ctx = egui::Context::default();
    a.st.ctrl.edit_text("solver.max_iterations", "10");
    assert!(a.st.ctrl.run_start());
    a.st.ctrl.wait_idle(60_000);
    a.st.ctrl.poll(true);
    for _ in 0..3 {
        frame(&mut a, &ctx);
    }
    assert!(a.st.ctrl.status.iteration > 0);
}

#[test]
fn assistant_turn_with_mock_provider_edits_runs_and_summarises() {
    let mut a = app();
    let ctx = egui::Context::default();
    frame(&mut a, &ctx);
    let mock = Arc::new(MockProvider::empty());
    mock.push(ChatResponse::tool_calls(
        "Shorter run.",
        vec![(
            "t1",
            "set_setting",
            json!({"path": "solver.max_iterations", "value": 12}),
        )],
    ));
    mock.push(ChatResponse::tool_calls(
        "Running.",
        vec![("t2", "run", json!({"max_steps": 12, "wait_seconds": 120}))],
    ));
    mock.push(ChatResponse::text("All done: 12 iterations ran."));
    a.st.install_provider(mock.clone(), "mock-model");
    a.st.chat.input = "Shorten the run to 12 iterations and run it".into();
    a.st.send_message();

    let deadline = Instant::now() + Duration::from_secs(120);
    let mut approved: Vec<String> = vec![];
    while a.st.chat.busy && Instant::now() < deadline {
        frame(&mut a, &ctx);
        if let Some(p) = a.st.chat.pending.clone() {
            if !approved.contains(&p.id) {
                approved.push(p.id.clone());
                a.st.approve();
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(!a.st.chat.busy, "turn finished");
    assert_eq!(approved.len(), 2, "set_setting and run each needed approval");
    let tools: Vec<_> =
        a.st.chat
            .entries
            .iter()
            .filter_map(|e| match e {
                Entry::Tool { name, state, .. } => Some((name.clone(), state.clone())),
                _ => None,
            })
            .collect();
    assert_eq!(
        tools,
        vec![("set_setting".into(), CardState::Done), ("run".into(), CardState::Done)]
    );
    assert!(matches!(a.st.chat.entries.last(), Some(Entry::Assistant(t)) if t.contains("12 iterations")));
    // The GUI sees the assistant's edit and the run.
    a.st.ctrl.sync_with_assistant();
    a.st.ctrl.refresh_settings();
    assert_eq!(value_at(&a.st.ctrl.settings, "solver.max_iterations"), Some(&json!(12)));
    a.st.ctrl.poll(true);
    assert!(a.st.ctrl.status.iteration > 0);
}

#[test]
fn denied_tool_leaves_the_case_unchanged() {
    let mut a = app();
    let ctx = egui::Context::default();
    let before = a.st.ctrl.settings.clone();
    let mock = Arc::new(MockProvider::empty());
    mock.push(ChatResponse::tool_calls(
        "Changing.",
        vec![("t1", "set_setting", json!({"path": "mesh.nx", "value": 99}))],
    ));
    mock.push(ChatResponse::text("Understood, I left it alone."));
    a.st.install_provider(mock, "mock-model");
    a.st.chat.input = "make the mesh finer".into();
    a.st.send_message();
    let deadline = Instant::now() + Duration::from_secs(30);
    while a.st.chat.busy && Instant::now() < deadline {
        frame(&mut a, &ctx);
        if a.st.chat.pending.is_some() {
            a.st.deny();
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(!a.st.chat.busy);
    a.st.ctrl.refresh_settings();
    assert_eq!(a.st.ctrl.settings, before);
}

#[test]
fn quick_actions_run_without_a_model() {
    let mut a = app();
    for q in rcfd_gui::state::QuickAction::ALL {
        a.st.quick_action(q);
    }
    let n =
        a.st.chat
            .entries
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    Entry::Tool {
                        state: CardState::Done,
                        ..
                    }
                )
            })
            .count();
    assert!(n >= 3, "setup tools answer without a provider, got {n} done cards");
}
