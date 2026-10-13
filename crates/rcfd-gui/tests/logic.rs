//! Logic tests for the non-UI modules: schema tree, monitors, viewport, layout, controller, chat.

use rcfd_api::case::Case;
use rcfd_gui::chat::{CardState, ChatState, Entry, WorkerMsg};
use rcfd_gui::controller::Controller;
use rcfd_gui::layout::{self, Panel};
use rcfd_gui::monitors::MonitorStore;
use rcfd_gui::schema_model::{format_value, parse_input, value_at, Kind};
use rcfd_gui::viewport::{cell_square_size, colormap, legend_ticks};
use serde_json::json;

fn ctrl() -> Controller {
    let dir = std::env::temp_dir().join("rcfd-gui-test-cases");
    let mut c = Controller::new(None, dir);
    c.new_from_template().expect("template opens");
    c
}

#[test]
fn every_schema_leaf_is_reachable_and_parses_its_default() {
    let c = ctrl();
    let leaves = c.schema.leaf_paths();
    assert!(
        leaves.len() > 30,
        "schema should expose many settings, got {}",
        leaves.len()
    );
    for p in &leaves {
        let n = c.schema.find(p).unwrap_or_else(|| panic!("find {p}"));
        assert!(n.is_leaf());
        if matches!(n.kind, Kind::Text | Kind::NumberList) {
            continue;
        }
        let text = format_value(&n.default);
        if text.is_empty() || n.default.is_null() {
            continue;
        }
        parse_input(&n.kind, &text).unwrap_or_else(|e| panic!("default of {p} ({text}) does not parse: {e}"));
    }
}

#[test]
fn parse_input_rejects_out_of_range_and_garbage() {
    let k = Kind::Int {
        min: Some(1.0),
        max: Some(10.0),
    };
    assert_eq!(parse_input(&k, "5").unwrap(), json!(5));
    assert!(parse_input(&k, "0").is_err());
    assert!(parse_input(&k, "abc").is_err());
    assert!(parse_input(&Kind::Bool, "maybe").is_err());
}

#[test]
fn monitors_merge_is_idempotent_and_incremental() {
    let doc = json!({"series": {"res_u": {"x": [1.0, 2.0], "y": [0.1, 0.01]}}});
    let mut m = MonitorStore::default();
    assert_eq!(m.merge(&doc), 2);
    assert_eq!(m.merge(&doc), 0);
    assert_eq!(m.since(), Some(2.0));
    let pts = m.plot_points("res_u", true);
    assert_eq!(pts.len(), 2);
    assert!((pts[1][1] + 2.0).abs() < 1e-12);
}

#[test]
fn colormap_and_ticks_behave() {
    assert_ne!(colormap(0.0, 0.0, 1.0), colormap(1.0, 0.0, 1.0));
    assert_eq!(colormap(5.0, 1.0, 1.0).len(), 3); // degenerate range does not panic
    let t = legend_ticks(0.0, 1.0, 5);
    assert!(t.len() >= 2 && t.first().unwrap() >= &0.0 && t.last().unwrap() <= &1.0);
    assert!(cell_square_size((0.0, 1.0, 0.0, 1.0), 100) > 0.0);
}

#[test]
fn layout_keeps_exactly_one_instance_per_panel() {
    let mut d = layout::default_layout();
    for p in Panel::ALL {
        assert_eq!(layout::count(&d, p), 1, "{p:?}");
    }
    layout::hide(&mut d, Panel::Log);
    assert_eq!(layout::count(&d, Panel::Log), 0);
    layout::show(&mut d, Panel::Log);
    layout::show(&mut d, Panel::Log);
    assert_eq!(layout::count(&d, Panel::Log), 1);
    layout::float(&mut d, Panel::Monitors);
    assert_eq!(layout::count(&d, Panel::Monitors), 1);
    layout::toggle(&mut d, Panel::Assistant);
    layout::toggle(&mut d, Panel::Assistant);
    assert_eq!(layout::count(&d, Panel::Assistant), 1);
    let text = layout::to_json(&d).unwrap();
    let back = layout::from_json(&text).unwrap();
    for p in Panel::ALL {
        assert_eq!(layout::count(&back, p), 1);
    }
    assert!(layout::from_json("{not json").is_err());
}

#[test]
fn editing_validates_locally_and_on_the_server() {
    let mut c = ctrl();
    assert!(!c.edit_text("mesh.nx", "abc"));
    assert!(c.field_errors.contains_key("mesh.nx"));
    assert!(c.edit_text("mesh.nx", "12"));
    assert!(!c.field_errors.contains_key("mesh.nx"));
    assert_eq!(value_at(&c.settings, "mesh.nx"), Some(&json!(12)));
    assert!(c.dirty);
    assert!(!c.edit_text("mesh.nx", "-4"));
    assert_eq!(
        value_at(&c.settings, "mesh.nx"),
        Some(&json!(12)),
        "rejected edit leaves the value"
    );
}

#[test]
fn named_entries_can_be_added_and_removed() {
    let mut c = ctrl();
    assert!(c.add_named("reports", "m1"));
    assert!(value_at(&c.settings, "reports.m1").is_some());
    assert!(c.remove_named("reports.m1"));
    assert!(value_at(&c.settings, "reports.m1").is_none());
    assert!(!c.add_named("reports", ""), "empty names are refused");
}

#[test]
fn save_round_trip_preserves_the_case() {
    let mut c = ctrl();
    assert!(c.edit_text("mesh.nx", "10"));
    let dir = std::env::temp_dir().join("rcfd-gui-test-save");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("roundtrip.toml");
    c.save_to(&path).expect("save");
    assert!(!c.dirty);
    let text = std::fs::read_to_string(&path).unwrap();
    let back = Case::parse_toml(&text).expect("saved file parses");
    let again = Case::from_json(&c.settings).expect("settings convert");
    assert_eq!(back.tree.content_hash(), again.tree.content_hash());
    let mut c2 = Controller::new(None, dir.clone());
    c2.open_path(&path).expect("reopen");
    assert_eq!(value_at(&c2.settings, "mesh.nx"), Some(&json!(10)));
}

#[test]
fn run_progresses_and_monitors_fill_incrementally() {
    let mut c = ctrl();
    assert!(c.edit_text("solver.max_iterations", "15"));
    assert!(c.run_start());
    c.wait_idle(60_000);
    c.poll(true);
    assert!(c.status.iteration > 0, "iterations ran: {:?}", c.status);
    assert!(!c.monitors.residual_names().is_empty());
    let n = c.monitors.series.values().map(|s| s.x.len()).sum::<usize>();
    c.poll(false);
    assert_eq!(
        n,
        c.monitors.series.values().map(|s| s.x.len()).sum::<usize>(),
        "re-poll adds no duplicates"
    );
    assert!(c.grid.is_some(), "field grid fetched");
    assert!(!c.run_pause(), "cannot pause when idle");
}

#[test]
fn chat_state_folds_worker_messages() {
    let mut s = ChatState::default();
    s.begin_turn("hello");
    assert!(s.busy);
    s.apply(WorkerMsg::Done("hi there".into()));
    assert!(!s.busy);
    assert!(matches!(s.entries.last(), Some(Entry::Assistant(t)) if t == "hi there"));
    s.apply(WorkerMsg::Failed("boom".into()));
    assert!(matches!(s.entries.last(), Some(Entry::Notice(_))));
    let _ = CardState::Pending;
}
