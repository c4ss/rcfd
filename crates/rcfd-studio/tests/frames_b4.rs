//! ADR 0131 B4: 200 headless frames through every panel shown, hidden and floated, both themes, a project open; plus the
//! frozen GUI's `layout_keeps_exactly_one_instance_per_panel` (ported, W48-9).

mod common;

use common::{b1_project, frame, frames_until, temp};
use egui_dock::DockState;
use rcfd_studio::layout::{self, Panel};
use rcfd_studio::theme::Theme;
use rcfd_studio::{Studio, StudioApp};
use std::time::Duration;

#[test]
fn b4_two_hundred_frames_through_every_panel_and_both_themes() {
    let root = temp("b4");
    b1_project(&root);
    let mut st = Studio::in_memory();
    st.open_project(&root);
    let mut app = StudioApp::new(st, None);
    let ctx = egui::Context::default();
    assert!(frames_until(&mut app, &ctx, Duration::from_secs(30), |a| a
        .st
        .browser()
        .is_some()));
    app.st.open_case("cases/a.toml");
    assert!(frames_until(&mut app, &ctx, Duration::from_secs(30), |a| a
        .st
        .editor
        .is_some()));
    app.st.selected_run = app.st.browser().unwrap().runs.first().map(|r| r.id.clone());

    let n = Panel::ALL.len();
    let mut dark_frames = 0;
    let mut light_frames = 0;
    let mut min_shapes = usize::MAX;
    for i in 0..200usize {
        let p = Panel::ALL[i % n];
        match (i / n) % 3 {
            0 => layout::hide(&mut app.st.dock, p),
            1 => layout::show(&mut app.st.dock, p),
            _ => layout::float(&mut app.st.dock, p),
        }
        app.st.theme = if i % 2 == 0 { Theme::Dark } else { Theme::Light };
        let out = frame(&mut app, &ctx);
        min_shapes = min_shapes.min(out.shapes.len());
        match ctx.theme() {
            egui::Theme::Dark => dark_frames += 1,
            egui::Theme::Light => light_frames += 1,
        }
        assert_eq!(ctx.theme(), app.st.theme.egui(), "frame {i}");
        for q in Panel::ALL {
            assert!(layout::count(&app.st.dock, q) <= 1, "frame {i}: {q:?} twice");
        }
        let want = if (i / n).is_multiple_of(3) { 0 } else { 1 };
        assert_eq!(layout::count(&app.st.dock, p), want, "frame {i}: {p:?}");
    }
    eprintln!("b4: 200 frames, {dark_frames} dark, {light_frames} light, min shapes per frame {min_shapes}");
    assert_eq!((dark_frames, light_frames), (100, 100));
    assert!(min_shapes > 0);
}

#[test]
fn b4_control_a_layout_with_a_duplicated_panel_fails_the_instance_check() {
    let mut d: DockState<Panel> = layout::default_layout();
    d.push_to_focused_leaf(Panel::Browser);
    assert_eq!(
        layout::count(&d, Panel::Browser),
        2,
        "the instance check fails on this layout"
    );
    let text = layout::to_json(&d).unwrap();
    let Err(e) = layout::from_json(&text) else {
        panic!("a layout with a duplicated panel was accepted")
    };
    eprintln!("b4 control: count 2, from_json: {e}");
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
    layout::float(&mut d, Panel::Plots);
    assert_eq!(layout::count(&d, Panel::Plots), 1);
    layout::toggle(&mut d, Panel::Chat);
    layout::toggle(&mut d, Panel::Chat);
    assert_eq!(layout::count(&d, Panel::Chat), 1);
    let text = layout::to_json(&d).unwrap();
    let back = layout::from_json(&text).unwrap();
    for p in Panel::ALL {
        assert_eq!(layout::count(&back, p), 1);
    }
    assert!(layout::from_json("{not json").is_err());
}
