//! Drawing of ADR 0140's two parts: the field-store section of the start dialog and the viewport's empty state. State
//! and decisions are in [`crate::store_plan`] and [`crate::run_fields`].

use crate::run_fields::{EmptyState, RERUN_LABEL};
use crate::runs::{StartPlan, StoreEvery};
use crate::store_plan;
use egui::Color32;

const WARN: Color32 = Color32::from_rgb(200, 140, 0);

/// The field-store section of the start dialog: the switch (ticked by default), interval and precision, the store and
/// size lines, the case's own output settings, and the warnings with their one-click fixes (D6).
pub fn store_section(ui: &mut egui::Ui, plan: &mut StartPlan) {
    if let Some(n) = &plan.note {
        ui.weak(n.as_str());
    }
    let e = store_plan::effective(plan);
    let label = format!("Store fields for scrubbing ({})", e.fields.join(", "));
    let case_precision = plan.facts.sets_precision;
    match plan.store.as_mut() {
        Some(s) => {
            ui.checkbox(&mut s.on, label);
            if s.on {
                ui.horizontal(|ui| match &mut s.every {
                    StoreEvery::Steps(k) => {
                        ui.label("every");
                        ui.add(egui::DragValue::new(k).range(1..=1_000_000_000u64));
                        ui.label("steps");
                    }
                    StoreEvery::Time(dt) => {
                        ui.label("every");
                        ui.add(egui::DragValue::new(dt).range(1e-12..=1e12).speed(0.0));
                        ui.label("s of simulated time");
                    }
                });
                ui.horizontal(|ui| {
                    if case_precision {
                        let mut shown = e.f32;
                        ui.add_enabled_ui(false, |ui| {
                            ui.radio_value(&mut shown, true, "f32 (lossy, half size)");
                            ui.radio_value(&mut shown, false, "f64 (exact)");
                        });
                        ui.weak("(set by the case)");
                    } else {
                        ui.radio_value(&mut s.f32, true, "f32 (lossy, half size)");
                        ui.radio_value(&mut s.f32, false, "f64 (exact)");
                    }
                });
            }
        }
        None => {
            // the case's own store, or none possible: shown, not switchable here
            let mut on = e.on();
            ui.add_enabled(false, egui::Checkbox::new(&mut on, label));
        }
    }
    if let Some(l) = store_plan::store_line(plan) {
        ui.label(l);
    }
    if let Some(l) = store_plan::size_line(plan) {
        ui.label(l);
    }
    for l in store_plan::case_lines(plan) {
        ui.weak(l);
    }
    let mut fix = None;
    for w in store_plan::warnings(plan) {
        ui.colored_label(WARN, w.text.as_str());
        if let Some((label, f)) = w.fix {
            if ui.button(label).clicked() {
                fix = Some(f);
            }
        }
    }
    if let Some(f) = fix {
        store_plan::apply_fix(plan, f);
    }
}

/// The viewport's empty state (`fallback` when no explanation arrived yet); `looked`: show where the store was looked
/// for. Returns true when "Re-run with fields stored" was clicked.
pub fn empty_state(ui: &mut egui::Ui, empty: Option<&EmptyState>, fallback: &str, looked: bool) -> bool {
    let Some(st) = empty else {
        ui.weak(fallback);
        return false;
    };
    ui.label(st.text.as_str());
    if let Some(h) = &st.hint {
        ui.weak(h.as_str());
    }
    let mut clicked = false;
    if st.rerun {
        clicked = ui.button(RERUN_LABEL).clicked();
    }
    if let Some(l) = &st.rerun_line {
        ui.weak(l.as_str());
    }
    if looked {
        ui.weak(st.looked.as_str());
    }
    clicked
}
