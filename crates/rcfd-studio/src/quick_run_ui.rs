//! Drawing of the one-click path (ADR 0141): the Run toolbar under the menu bar (Run button, F5, the start dialog for
//! options, where `rcfd-runner` and `rcfd` were found) and the status strip of the selected run above the status bar
//! (state, step, time, residuals, snapshot, Pause, Resume, Cancel). State and decisions are in [`crate::quick_run`].

use crate::quick_run::{self, Control, TimeView, Tone};
use crate::studio::Studio;
use crate::time_step::{self, Facts, RunTime, StepMode, TimePlan};
use crate::ui::{StartDialog, UiState};
use egui::{Color32, RichText};

const ERR: Color32 = Color32::from_rgb(200, 60, 60);
const WARN: Color32 = Color32::from_rgb(200, 140, 0);
const GO: Color32 = Color32::from_rgb(40, 140, 70);

/// The Run button's hover text (ADR 0141); a transient case's run plan follows it as a second line (ADR 0153).
pub const RUN_HOVER: &str = "start the selected case with the default plan and follow it in the viewport (F5)";
/// The label before the simulated time (ADR 0153).
pub const SIMULATE_FOR: &str = "Simulate for";
/// The button of the time step's popover (ADR 0153).
pub const ADVANCED: &str = "Advanced";
/// The fix button of the Courant warning (ADR 0153).
pub const FIX: &str = "Use the automatic time step";
/// The fix button of the Courant warning when the target is too high (ADR 0153).
pub const FIX_TARGET: &str = "Use the default Courant target";
/// The checkbox of the Advanced popover (ADR 0153).
pub const AUTO_BOX: &str = "automatic time step (Courant controller)";

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
    // ADR 0153: the time row of the selected case, and its plan on Run's hover
    let time = quick_run::time_view(st);
    let plan_hover = match &time {
        TimeView::Transient {
            facts, plan: Ok(tp), ..
        } => Some(time_step::hover(facts, tp)),
        TimeView::Transient { plan: Err(e), .. } => Some(format!("This run is not started: {e}")),
        _ => None,
    };
    let mut go = false;
    ui.horizontal(|ui| {
        let label = RichText::new("  Run  ").strong().color(Color32::WHITE);
        let r = ui
            .add_enabled(blocked.is_none(), egui::Button::new(label).fill(GO))
            .on_hover_ui(|ui| {
                ui.label(RUN_HOVER);
                if let Some(h) = &plan_hover {
                    ui.label(h.as_str());
                }
            })
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
    if let Some(c) = &case {
        time_row(ui, st, c, time);
    }
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

/// The time row of the selected case (ADR 0153): "Simulate for [N] s", the Advanced popover (Courant target, largest
/// step, the case's own step), the step of the run as it will start, the indicator that it overrides the case's own
/// `solver.dt` or `solver.end_time`, and the Courant warning with its fix. A steady case, or a transient one outside the
/// scope, gets one line saying what Run does instead. The settings are the case's run-time settings, never the case's.
fn time_row(ui: &mut egui::Ui, st: &mut Studio, case: &str, view: TimeView) {
    let (facts, rt, plan) = match view {
        TimeView::None => return,
        TimeView::Unknown { text, hover } => {
            let r = ui.weak(text);
            if let Some(h) = hover {
                r.on_hover_text(h);
            }
            return;
        }
        TimeView::Plain { text, hover } => {
            ui.weak(text).on_hover_text(hover);
            return;
        }
        TimeView::Transient { facts, rt, plan } => (facts, rt, plan),
    };
    let mut next = rt.clone();
    ui.horizontal(|ui| {
        ui.label(SIMULATE_FOR);
        let mut n = rt.simulate.unwrap_or(facts.end_time);
        let speed = (n.abs() * 0.01).max(1e-6);
        let r = ui
            .add(
                egui::DragValue::new(&mut n)
                    .range(1e-9..=1e12)
                    .speed(speed)
                    .max_decimals(9)
                    // the Studio's number format (ADR 0148 decision 6): 80, 7.3, no trailing zeros (ADR 0153 amendment 1)
                    .custom_formatter(|v, _| crate::viewport::fmt_num(v)),
            )
            .on_hover_text(format!(
                "Seconds of simulated time this run ends at (solver.end_time of the run; the case says {} s). The run \
                 stops there and stores its final state. A run-time setting: the case file is not changed.",
                crate::viewport::fmt_num(facts.end_time)
            ));
        st.quick.rects.insert(SIMULATE_FOR.into(), r.rect);
        if r.changed() && n.is_finite() && n > 0.0 {
            next.simulate = Some(n);
        }
        ui.label(crate::viewport::TIME_UNIT);
        let adv = ui.button(ADVANCED).on_hover_text(
            "the time step of this case's runs: the Courant target, the largest step, or the case's own solver.dt",
        );
        let q = &mut st.quick;
        q.rects.insert(ADVANCED.into(), adv.rect);
        if adv.clicked() {
            q.advanced_open = !q.advanced_open;
        }
        let (open, rects) = (&mut q.advanced_open, &mut q.rects);
        egui::Popup::from_response(&adv)
            .open_bool(open)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| advanced(ui, &facts, &mut next, rects));
        match &plan {
            Ok(tp) => {
                let hover = time_step::hover(&facts, tp);
                ui.monospace(time_step::label(tp)).on_hover_text(hover.clone());
                if let Some(o) = time_step::overridden(tp) {
                    ui.colored_label(WARN, o).on_hover_text(hover);
                }
            }
            Err(e) => {
                ui.colored_label(ERR, e.as_str());
            }
        }
    });
    if let Some((w, fix)) = plan.as_ref().ok().and_then(|tp| warning_of(case, &facts, tp)) {
        // ADR 0153 amendment 2: the fix first, the warning wrapping after it, so that the button stays in the window
        let last = st.quick.switches.warning_fix_last;
        ui.horizontal(|ui| {
            let clicked = if !last {
                let r = ui.button(fix);
                st.quick.rects.insert(FIX.into(), r.rect);
                ui.add(egui::Label::new(RichText::new(w.as_str()).color(WARN)).wrap());
                r.clicked()
            } else {
                ui.colored_label(WARN, w);
                let r = ui.button(fix);
                st.quick.rects.insert(FIX.into(), r.rect);
                r.clicked()
            };
            if clicked {
                quick_run::fix_warning(st, case);
                next = quick_run::run_time(st, case);
            }
        });
    }
    if next != rt {
        *quick_run::run_time_mut(st, case) = next;
    }
}

/// The Courant warning of a plan and the label of its fix.
fn warning_of(case: &str, f: &Facts, tp: &TimePlan) -> Option<(String, &'static str)> {
    let w = time_step::warning(case, f, tp)?;
    Some((
        w,
        if matches!(tp.step, time_step::Step::CaseDt { .. }) {
            FIX
        } else {
            FIX_TARGET
        },
    ))
}

/// The Advanced popover: the automatic step on or off (the case's own `solver.dt`), its Courant target and its largest
/// step. A case with its own step control keeps it.
fn advanced(
    ui: &mut egui::Ui,
    f: &Facts,
    rt: &mut RunTime,
    rects: &mut std::collections::BTreeMap<String, egui::Rect>,
) {
    ui.strong("Time step of this case's runs");
    ui.weak("run-time settings: the case file keeps its own");
    let own = !f.own_control.is_empty();
    let mut auto = rt.mode != StepMode::Case && !own;
    let r = ui
        .add_enabled(!own, egui::Checkbox::new(&mut auto, AUTO_BOX))
        .on_hover_text(
            "on: Run sets dt from the mesh and the case's speeds and the solver keeps the largest Courant number at the \
             target; off: Run takes the case's own solver.dt",
        )
        .on_disabled_hover_text(format!(
            "the case sets its own step control ({}), which Run keeps",
            f.own_control.join(", ")
        ));
    rects.insert(AUTO_BOX.into(), r.rect);
    if r.changed() {
        rt.mode = if auto { time_step::DEFAULT_MODE } else { StepMode::Case };
    }
    ui.add_enabled_ui(auto, |ui| {
        ui.horizontal(|ui| {
            ui.label("Courant target");
            ui.add(egui::DragValue::new(&mut rt.courant).range(0.01..=50.0).speed(0.01))
                .on_hover_text(format!(
                    "the largest cell Courant number the solver keeps (default {}; above {} the run may become \
                     unstable or noisy)",
                    crate::viewport::fmt_num(time_step::COURANT_TARGET),
                    crate::viewport::fmt_num(time_step::COURANT_WARN)
                ));
        });
        ui.horizontal(|ui| {
            ui.label("Largest step (s)");
            let n = rt.simulate.unwrap_or(f.end_time);
            let mut m = rt.max_dt.unwrap_or(n / time_step::MIN_STEPS);
            let speed = (m.abs() * 0.01).max(1e-9);
            let r = ui.add(
                egui::DragValue::new(&mut m)
                    .range(1e-12..=1e12)
                    .speed(speed)
                    .max_decimals(9)
                    .custom_formatter(|v, _| crate::viewport::fmt_num(v)),
            );
            if r.changed() && m.is_finite() && m > 0.0 {
                rt.max_dt = Some(m);
            }
            if rt.max_dt.is_some() && ui.button("default").clicked() {
                rt.max_dt = None;
            }
        });
    });
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
