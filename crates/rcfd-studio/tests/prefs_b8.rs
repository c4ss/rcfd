//! ADR 0131 B8: layout and preferences round trip exactly; a corrupt preferences file gives the default layout and a
//! message, no panic.

mod common;

use common::temp;
use egui_dock::DockState;
use rcfd_studio::layout::{self, Panel};
use rcfd_studio::prefs::{Prefs, FILE, MAX_RECENT};
use rcfd_studio::theme::Theme;
use std::path::Path;

fn changed_layout() -> DockState<Panel> {
    let mut d = layout::default_layout();
    layout::hide(&mut d, Panel::Chat);
    layout::float(&mut d, Panel::Plots);
    layout::hide(&mut d, Panel::Outputs);
    layout::show(&mut d, Panel::Outputs);
    d
}

#[test]
fn b8_preferences_round_trip_exactly() {
    let dir = temp("b8");
    let dock = changed_layout();
    let mut p = Prefs::capture(Theme::Dark, &dock, &[]);
    for i in 0..(MAX_RECENT + 3) {
        p.remember(Path::new(&format!("C:/projects/p{i}")));
    }
    p.remember(Path::new("C:/projects/p5"));
    assert_eq!(p.recent.len(), MAX_RECENT);
    assert_eq!(p.recent[0], "C:/projects/p5");
    p.save(&dir).unwrap();
    let back = Prefs::load(&dir);
    assert!(back.message.is_none(), "{:?}", back.message);
    assert_eq!(back.prefs, p);
    assert_eq!(layout::to_json(&back.dock).unwrap(), layout::to_json(&dock).unwrap());
    assert_eq!(back.prefs.theme, Theme::Dark);
    for q in Panel::ALL {
        assert_eq!(layout::count(&back.dock, q), layout::count(&dock, q), "{q:?}");
    }
    // saving what was loaded writes the same bytes
    let first = std::fs::read(dir.join(FILE)).unwrap();
    back.prefs.save(&dir).unwrap();
    assert_eq!(std::fs::read(dir.join(FILE)).unwrap(), first);
    eprintln!(
        "b8: {} bytes, {} recent, layout {} bytes, equal after round trip",
        first.len(),
        p.recent.len(),
        p.layout.as_ref().unwrap().len()
    );
}

#[test]
fn b8_missing_file_gives_defaults_without_a_message() {
    let dir = temp("b8m");
    let l = Prefs::load(&dir);
    assert!(l.message.is_none());
    assert_eq!(l.prefs, Prefs::default());
    assert_eq!(
        layout::to_json(&l.dock).unwrap(),
        layout::to_json(&layout::default_layout()).unwrap()
    );
}

#[test]
fn b8_control_corrupt_file_gives_the_default_layout_and_a_message() {
    let dir = temp("b8c");
    std::fs::write(dir.join(FILE), b"{\"theme\": \"dark\", \"layout\": 12, garbage").unwrap();
    let l = Prefs::load(&dir);
    let m = l.message.expect("a message");
    eprintln!("b8 control (garbage): {m}");
    assert!(m.contains(FILE), "{m}");
    assert_eq!(
        layout::to_json(&l.dock).unwrap(),
        layout::to_json(&layout::default_layout()).unwrap()
    );
    assert_eq!(l.prefs, Prefs::default());
}

#[test]
fn b8_control_layout_with_a_duplicated_panel_gives_the_default_layout_and_a_message() {
    let dir = temp("b8d");
    let mut d = layout::default_layout();
    d.push_to_focused_leaf(Panel::Runs);
    let p = Prefs {
        theme: Theme::Dark,
        layout: Some(layout::to_json(&d).unwrap()),
        recent: vec!["C:/projects/x".into()],
    };
    p.save(&dir).unwrap();
    let l = Prefs::load(&dir);
    let m = l.message.expect("a message");
    eprintln!("b8 control (duplicated panel): {m}");
    assert!(m.contains("layout"), "{m}");
    assert_eq!(layout::count(&l.dock, Panel::Runs), 1);
    assert_eq!(
        layout::to_json(&l.dock).unwrap(),
        layout::to_json(&layout::default_layout()).unwrap()
    );
    // the rest of the file is kept
    assert_eq!(l.prefs.theme, Theme::Dark);
    assert_eq!(l.prefs.recent, ["C:/projects/x"]);
}
