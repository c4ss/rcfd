//! Drawing of the one-click path (ADR 0141): the Run toolbar under the menu bar (Run button, F5, the start dialog for
//! options, where `rcfd-runner` and `rcfd` were found) and the status strip of the selected run above the status bar
//! (state, step, time, residuals, snapshot, Pause, Resume, Cancel). State and decisions are in [`crate::quick_run`].

use crate::quick_run::{self, Control, Tone};
use crate::studio::Studio;
use crate::ui::{StartDialog, UiState};
use egui::{Color32, RichText};

const ERR: Color32 = Color32::from_rgb(200, 60, 60);
const WARN: Color32 = Color32::from_rgb(200, 140, 0);
const GO: Color32 = Color32::from_rgb(40, 140, 70);

/// Draw the toolbar (top) and the strip (bottom); call after the menu bar and the status bar so they sit inside them.
pub fn show(ui: &mut egui::Ui, st: &mut Studio, us: &mut UiState) {
    egui::Panel::top("rcfd-studio-quickrun").show(ui, |ui| toolbar(ui, st, us));
    if st.project.is_some() || st.quick.error.is_some() {
        egui::Panel::bottom("rcfd-studio-runstrip").show(ui, |ui| strip(ui, st));
    }
}

/// The Run toolbar. F5 is the Run button's shortcut.
pub fn toolbar(ui: &mut egui::Ui, st: &mut Studio, us: &mut UiState) {
    let case = quick_run::selected_case(st);
    let blocked = quick_run::blocked(st);
    let mut go = false;
    ui.horizontal(|ui| {
        let label = RichText::new("  Run  ").strong().color(Color32::WHITE);
        let r = ui
            .add_enabled(blocked.is_none(), egui::Button::new(label).fill(GO))
            .on_hover_text("start the selected case with the default plan and follow it in the viewport (F5)")
            .on_disabled_hover_text(blocked.clone().unwrap_or_default());
        st.quick.rects.insert("Run".into(), r.rect);
        go |= r.clicked();
        match &case {
            Some(c) => ui.label(RichText::new(c).monospace()),
            None => ui.weak("no case selected"),
        };
        ui.weak("F5");
        let r = ui
            .add_enabled(st.browser().is_some(), egui::Button::new("Options..."))
            .on_hover_text("the Start run dialog: priority, threads, memory, field store, overrides");
        st.quick.rects.insert("Options...".into(), r.rect);
        if r.clicked() {
            us.start_dialog = Some(StartDialog {
                case: case.clone().unwrap_or_default(),
                ..Default::default()
            });
        }
        ui.separator();
        match &st.quick.bins {
            Some(Ok(f)) => {
                ui.weak(f.describe());
            }
            Some(Err(_)) => {
                ui.colored_label(ERR, "rcfd-runner and rcfd not found (below)");
            }
            None => {
                ui.weak("rcfd-runner and rcfd: not looked for yet");
            }
        }
        let busy = st.quick.locating();
        if ui
            .add_enabled(
                !busy,
                egui::Button::new(if busy { "searching..." } else { "Search again" }),
            )
            .on_hover_text("look for rcfd-runner and rcfd again (after a build)")
            .clicked()
        {
            let wake = st.wake.clone();
            st.quick.locate(&wake);
        }
    });
    // the full text once: after a Run it is the strip's error line
    if let (Some(Err(e)), None) = (&st.quick.bins, &st.quick.error) {
        ui.colored_label(ERR, e.as_str());
    }
    if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F5)) {
        go = true;
    }
    if go {
        if let Err(m) = quick_run::run_selected_case(st) {
            st.say(m);
        }
    }
}

/// The status strip of the selected run.
pub fn strip(ui: &mut egui::Ui, st: &mut Studio) {
    let v = quick_run::strip_view(st);
    let mut act: Option<Control> = None;
    ui.horizontal(|ui| {
        for (c, enabled) in v.controls {
            let r = ui.add_enabled(enabled, egui::Button::new(c.label()));
            st.quick.rects.insert(c.label().into(), r.rect);
            if r.clicked() {
                act = Some(c);
            }
        }
        ui.separator();
        ui.vertical(|ui| {
            for l in &v.lines {
                match l.tone {
                    Tone::Normal => ui.monospace(&l.text),
                    Tone::Weak => ui.weak(&l.text),
                    Tone::Warn => ui.colored_label(WARN, &l.text),
                    Tone::Error => ui.colored_label(ERR, &l.text),
                };
            }
        });
    });
    if let Some(c) = act {
        if let Err(m) = quick_run::control(st, c) {
            st.say(m);
        }
    }
    // a start in flight (the runner may take seconds to start): keep its age counting, no worker wakes the UI then
    if st.quick.starting.is_some() {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(250));
    }
}
