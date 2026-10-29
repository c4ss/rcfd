//! Drawing of Stop in Run's place (ADR 0161): a red Stop while the selected case's toolbar run is live (Ctrl+., Cmd+. on
//! macOS), a greyed-out "Stopping..." until it has ended. State and decisions are in [`crate::stop`].

use crate::stop::{self, View};
use crate::studio::Studio;
use egui::{Color32, RichText};

/// The Stop button's label (and the key of its rectangle in `QuickRun::rects`).
pub const STOP: &str = "Stop";
/// The label while the run stops.
pub const STOPPING: &str = "Stopping...";
/// Stop's shortcut, as the hover names it.
pub const SHORTCUT: &str = "Ctrl+.";

const RED: Color32 = Color32::from_rgb(200, 60, 60);

/// Stop's shortcut: Ctrl+. (Cmd+. on macOS). A modifier is needed, so typing a full stop or pressing Escape never stops a
/// run.
pub fn shortcut_pressed(ui: &mut egui::Ui) -> bool {
    ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Period))
}

/// Draw Stop or "Stopping..." where Run goes, when the selected case's toolbar run is live, and act on its click and
/// shortcut. False: nothing was drawn and the caller draws Run.
pub fn button(ui: &mut egui::Ui, st: &mut Studio) -> bool {
    let v = stop::view(st);
    let hover = stop::hover(st, &v);
    match &v {
        View::Run => false,
        View::Preparing { .. } | View::Live { .. } => {
            let enabled = matches!(v, View::Live { .. });
            let label = RichText::new(format!("  {STOP}  ")).strong().color(Color32::WHITE);
            let r = ui
                .add_enabled(enabled, egui::Button::new(label).fill(RED))
                .on_hover_text(hover.as_str())
                .on_disabled_hover_text(hover.as_str());
            st.quick.rects.insert(STOP.into(), r.rect);
            let key = enabled && shortcut_pressed(ui);
            if r.clicked() || key {
                stop::stop(st);
            }
            true
        }
        View::Stopping { .. } => {
            let label = RichText::new(format!("  {STOPPING}  ")).strong();
            let r = ui
                .add_enabled(false, egui::Button::new(label))
                .on_disabled_hover_text(hover);
            st.quick.rects.insert(STOPPING.into(), r.rect);
            // the run worker wakes the UI when the run ends; the hover's age counts meanwhile
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(250));
            true
        }
    }
}

/// Whether Run (its click or F5) may start the selected case now; when not, the status bar says why (ADR 0161: not while
/// the toolbar shows Stop or "Stopping..." for it).
pub fn allow_run(st: &mut Studio) -> bool {
    match stop::run_refusal(st) {
        Some(why) => {
            st.say(why);
            false
        }
        None => true,
    }
}
