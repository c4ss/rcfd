//! Drawing only: menus, status bar, dialogs and the docked panels. State and decisions are in [`crate::studio`] and the
//! modules it uses. `eframe::App::ui` calls [`StudioApp::ui`]; tests call it with a bare `egui::Context`.

use crate::case_edit::EditorState;
use crate::layout::{self, Panel};
use crate::monitors::MAX_PLOT_POINTS;
use crate::schema_model::{format_value, matches_filter, value_at, Kind, Node};
use crate::studio::Studio;
use crate::theme::Theme;
use egui::{Color32, RichText};
use egui_dock::{DockArea, DockState, TabViewer};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

const ERR: Color32 = Color32::from_rgb(200, 60, 60);
const WARN: Color32 = Color32::from_rgb(200, 140, 0);

/// Fields of the "New project" dialog.
#[derive(Debug, Clone, Default)]
pub struct NewDialog {
    /// Folder to create the project in.
    pub path: String,
    /// Project name.
    pub name: String,
    /// Solver version recorded in the manifest.
    pub version: String,
}

/// UI-only state: dialog fields, edit buffers, test hooks.
#[derive(Default)]
pub struct UiState {
    /// Open "New project" dialog.
    pub new_dialog: Option<NewDialog>,
    /// Open "Open project" dialog (typed path).
    pub open_dialog: Option<String>,
    /// Filter text of the case editor.
    pub filter: String,
    buffers: BTreeMap<String, String>,
    new_names: BTreeMap<String, String>,
    /// Busy CPU time added per run-list row built (ADR 0131 B7 negative control; zero in the app).
    pub row_cost: Duration,
    /// Run-list rows built in the last frame (reported by B7).
    pub rows_built: usize,
    /// Open start dialog (ADR 0133).
    pub start_dialog: Option<StartDialog>,
    /// Edit buffer of the runner budgets (`None`: closed).
    pub budgets: Option<rcfd_runner::runner::Config>,
}

/// UI fields of the start dialog; the plan itself is in the model ([`crate::runs::RunsView::plan`]).
#[derive(Debug, Clone, Default)]
pub struct StartDialog {
    /// Case chosen (project-relative).
    pub case: String,
    /// The case whose plan was asked for.
    pub asked: Option<String>,
    /// `path=value` overrides, one per line.
    pub overrides: String,
    /// Memory need in MB as typed (empty: estimated by the runner).
    pub mem: String,
}

/// The application: model plus UI state.
pub struct StudioApp {
    /// Model.
    pub st: Studio,
    /// UI state.
    pub ui_state: UiState,
}

impl StudioApp {
    /// Wrap a model. With `ctx`, the project worker requests a repaint when it has news (no polling loop in the UI).
    pub fn new(mut st: Studio, ctx: Option<&egui::Context>) -> StudioApp {
        if let Some(ctx) = ctx {
            let ctx = ctx.clone();
            st.set_wake(std::sync::Arc::new(move || ctx.request_repaint()));
        }
        StudioApp {
            st,
            ui_state: UiState::default(),
        }
    }

    /// Draw one frame.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.st.drain();
        self.st.theme.apply(&ctx);
        egui::Panel::top("rcfd-studio-menu").show(ui, |ui| self.menu_bar(ui));
        egui::Panel::bottom("rcfd-studio-status").show(ui, |ui| self.status_bar(ui));
        self.dialogs(&ctx);
        let mut dock = std::mem::replace(&mut self.st.dock, DockState::new(Vec::new()));
        let style = egui_dock::Style::from_egui(ui.style());
        self.ui_state.rows_built = 0;
        egui::CentralPanel::no_frame().show(ui, |ui| {
            DockArea::new(&mut dock).style(style).show_inside(
                ui,
                &mut Viewer {
                    st: &mut self.st,
                    us: &mut self.ui_state,
                },
            );
        });
        self.st.dock = dock;
        if ctx.input(|i| i.viewport().close_requested()) {
            if let Err(e) = self.st.save_prefs_now() {
                eprintln!("rcfd-studio: could not save preferences: {e}");
            }
        }
    }

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("Project", |ui| {
                if ui.button("New project...").clicked() {
                    self.ui_state.new_dialog = Some(NewDialog {
                        version: crate::project_view::solver_version(),
                        ..Default::default()
                    });
                    ui.close();
                }
                if ui.button("Open project...").clicked() {
                    ui.close();
                    if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                        self.st.open_project(&dir);
                    }
                }
                if ui.button("Open typed path...").clicked() {
                    self.ui_state.open_dialog = Some(String::new());
                    ui.close();
                }
                let recent = self.st.prefs.recent.clone();
                ui.add_enabled_ui(!recent.is_empty(), |ui| {
                    ui.menu_button("Recent", |ui| {
                        for r in &recent {
                            if ui.button(r).clicked() {
                                self.st.open_project(&PathBuf::from(r));
                                ui.close();
                            }
                        }
                    });
                });
                if ui
                    .add_enabled(self.st.project.is_some(), egui::Button::new("Refresh"))
                    .clicked()
                {
                    self.st.refresh();
                    ui.close();
                }
                if ui
                    .add_enabled(self.st.project.is_some(), egui::Button::new("Close project"))
                    .clicked()
                {
                    self.st.close_project();
                    ui.close();
                }
            });
            ui.menu_button("Case", |ui| {
                let can_save = self
                    .st
                    .editor
                    .as_ref()
                    .is_some_and(|e| e.is_dirty() && !e.in_conflict());
                if ui.add_enabled(can_save, egui::Button::new("Save")).clicked() {
                    if let Err(m) = self.st.save_case() {
                        self.st.say(m);
                    }
                    ui.close();
                }
                if ui
                    .add_enabled(self.st.editor.is_some(), egui::Button::new("Validate"))
                    .clicked()
                {
                    validate(&mut self.st);
                    ui.close();
                }
            });
            ui.menu_button("View", |ui| {
                for p in Panel::ALL {
                    let mut open = layout::is_open(&self.st.dock, p);
                    if ui.checkbox(&mut open, p.title()).changed() {
                        layout::toggle(&mut self.st.dock, p);
                        self.st.save_prefs();
                    }
                }
                ui.separator();
                ui.menu_button("Float panel", |ui| {
                    for p in Panel::ALL {
                        if ui.button(p.title()).clicked() {
                            layout::float(&mut self.st.dock, p);
                            self.st.save_prefs();
                            ui.close();
                        }
                    }
                });
                if ui.button("Reset layout").clicked() {
                    self.st.dock = layout::default_layout();
                    self.st.save_prefs();
                    ui.close();
                }
                ui.separator();
                let label = if self.st.theme == Theme::Dark {
                    "Light theme"
                } else {
                    "Dark theme"
                };
                if ui.button(label).clicked() {
                    self.st.theme = self.st.theme.toggled();
                    self.st.save_prefs();
                    ui.close();
                }
            });
        });
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            match self.st.browser() {
                Some(b) => {
                    ui.label(RichText::new(&b.name).strong());
                    ui.weak(b.root.display().to_string());
                }
                None if self.st.project.is_some() => {
                    ui.label("opening...");
                }
                None => {
                    ui.weak("no project open");
                }
            }
            if let Some(ed) = &self.st.editor {
                ui.separator();
                let (txt, col) = match ed.state() {
                    EditorState::Clean => (format!("{} saved", ed.rel), None),
                    EditorState::Dirty => (format!("{} *", ed.rel), None),
                    EditorState::Conflict { .. } => (format!("{} conflict", ed.rel), Some(WARN)),
                };
                match col {
                    Some(c) => ui.colored_label(c, txt),
                    None => ui.label(txt),
                };
            }
            if let Some(m) = self.st.last_message() {
                ui.separator();
                ui.label(m);
            }
        });
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        if let Some(mut d) = self.ui_state.new_dialog.take() {
            let mut open = true;
            let mut done = false;
            egui::Window::new("New project")
                .open(&mut open)
                .collapsible(false)
                .show(ctx, |ui| {
                    egui::Grid::new("new-project").num_columns(2).show(ui, |ui| {
                        ui.label("Folder");
                        ui.horizontal(|ui| {
                            ui.add(egui::TextEdit::singleline(&mut d.path).desired_width(300.0));
                            if ui.button("Browse...").clicked() {
                                if let Some(p) = rfd::FileDialog::new().pick_folder() {
                                    d.path = p.display().to_string();
                                }
                            }
                        });
                        ui.end_row();
                        ui.label("Name");
                        ui.add(egui::TextEdit::singleline(&mut d.name).desired_width(300.0));
                        ui.end_row();
                        ui.label("Solver version");
                        ui.add(egui::TextEdit::singleline(&mut d.version).desired_width(300.0));
                        ui.end_row();
                    });
                    let ok = !d.path.trim().is_empty() && !d.name.trim().is_empty();
                    if ui.add_enabled(ok, egui::Button::new("Create")).clicked() {
                        self.st
                            .create_project(&PathBuf::from(d.path.trim()), d.name.trim(), d.version.trim());
                        done = true;
                    }
                });
            if open && !done {
                self.ui_state.new_dialog = Some(d);
            }
        }
        start_dialog(ctx, &mut self.st, &mut self.ui_state);
        budgets_dialog(ctx, &mut self.st, &mut self.ui_state);
        if let Some(mut path) = self.ui_state.open_dialog.take() {
            let mut open = true;
            let mut done = false;
            egui::Window::new("Open project")
                .open(&mut open)
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.label("Project folder (holds rcfd-project.json)");
                    let r = ui.add(egui::TextEdit::singleline(&mut path).desired_width(360.0));
                    let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if (ui.button("Open").clicked() || enter) && !path.trim().is_empty() {
                        self.st.open_project(&PathBuf::from(path.trim()));
                        done = true;
                    }
                });
            if open && !done {
                self.ui_state.open_dialog = Some(path);
            }
        }
    }
}

impl eframe::App for StudioApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        StudioApp::ui(self, ui);
    }
}

fn validate(st: &mut Studio) {
    let Some(ed) = &mut st.editor else { return };
    let ok = ed.validate();
    let rel = ed.rel.clone();
    st.say(if ok {
        format!("{rel}: valid")
    } else {
        format!("{rel}: has problems (see the case editor)")
    });
}

struct Viewer<'a> {
    st: &'a mut Studio,
    us: &'a mut UiState,
}

impl TabViewer for Viewer<'_> {
    type Tab = Panel;

    fn id(&mut self, tab: &mut Panel) -> egui::Id {
        egui::Id::new(("rcfd-studio-panel", *tab))
    }

    fn title(&mut self, tab: &mut Panel) -> egui::WidgetText {
        tab.title().into()
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Panel) {
        match tab {
            Panel::Browser => browser_panel(ui, self.st),
            Panel::CaseEditor => editor_panel(ui, self.st, self.us),
            Panel::Runs => runs_panel(ui, self.st, self.us),
            Panel::Viewport => {
                ui.weak("The 3D view of a run arrives with the viewport integration (48C, ADR 0132).");
            }
            Panel::Plots => plots_panel(ui, self.st),
            Panel::Log => log_panel(ui, self.st),
            Panel::Chat => {
                ui.weak("The Claude Code chat for this project arrives with the sidebar (48E, ADR 0134).");
            }
            Panel::Outputs => outputs_panel(ui, self.st),
        }
    }
}

// ---------------------------------------------------------------- browser

fn browser_panel(ui: &mut egui::Ui, st: &mut Studio) {
    let Some(b) = st.browser() else {
        ui.label(if st.project.is_some() {
            "Opening the project..."
        } else {
            "No project open. Use Project > New project or Open project."
        });
        return;
    };
    ui.heading(&b.name);
    ui.weak(b.root.display().to_string());
    ui.weak(format!("created with {}", b.solver_version_created));
    if let Some(n) = b.notice() {
        ui.colored_label(WARN, n);
    }
    for e in &b.errors {
        ui.colored_label(ERR, e);
    }
    let mut open_case: Option<String> = None;
    let current = st.editor.as_ref().map(|e| e.rel.clone()).or(st.pending_case.clone());
    egui::ScrollArea::vertical()
        .id_salt("browser")
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            egui::CollapsingHeader::new(format!("Cases ({})", b.cases.len()))
                .id_salt("cases")
                .default_open(true)
                .show(ui, |ui| {
                    for c in &b.cases {
                        if ui.selectable_label(current.as_deref() == Some(c), c).clicked() {
                            open_case = Some(c.clone());
                        }
                    }
                });
            egui::CollapsingHeader::new(format!("Meshes ({})", b.meshes.len()))
                .id_salt("meshes")
                .show(ui, |ui| {
                    for m in &b.meshes {
                        ui.label(m);
                    }
                });
            egui::CollapsingHeader::new(format!("Designs ({})", b.designs.len()))
                .id_salt("designs")
                .show(ui, |ui| {
                    for d in &b.designs {
                        ui.label(d);
                    }
                });
            egui::CollapsingHeader::new(format!("Runs: {} listed, {} skipped", b.runs.len(), b.skipped.len()))
                .id_salt("skipped")
                .show(ui, |ui| {
                    for s in &b.skipped {
                        let name = s
                            .path
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        ui.colored_label(WARN, format!("{name}: {}", s.message));
                    }
                });
            if let Some(p) = &st.project {
                if !p.changes.is_empty() {
                    egui::CollapsingHeader::new("Changed on disk")
                        .id_salt("changes")
                        .show(ui, |ui| {
                            for (_, c) in p.changes.iter().rev().take(20) {
                                ui.weak(c);
                            }
                        });
                }
            }
        });
    if let Some(c) = open_case {
        st.open_case(&c);
    }
}

// ---------------------------------------------------------------- runs

/// One drawn row of the run list: the browser's summary with the run worker's row laid over it (ADR 0133 amendment 1
/// item 6).
fn row_line(r: &rcfd_project::RunSummary, live: Option<&crate::runs::RunRow>) -> String {
    match live {
        Some(l) => live_line(l),
        None => format!(
            "{:<23} {:<9} {:>8} {:>11.4e}  {}",
            r.id,
            format!("{:?}", r.state).to_lowercase(),
            r.step,
            r.time,
            r.case
        ),
    }
}

fn live_line(l: &crate::runs::RunRow) -> String {
    let res = |k: &str| l.last_residuals.get(k).map_or("-".to_string(), |v| format!("{v:.2e}"));
    let queue = l.queue_pos.map_or(String::new(), |q| format!(" #{q}"));
    format!(
        "{:<23} {:<9} {:>8} {:>11.4e}  {:>9} {:>9}  {}{queue}",
        l.id,
        l.state_label(),
        l.step,
        l.time,
        res("res_u"),
        res("res_p"),
        l.case
    )
}

fn runs_panel(ui: &mut egui::Ui, st: &mut Studio, us: &mut UiState) {
    if st.browser().is_none() {
        ui.weak("No project open.");
        return;
    }
    runs_toolbar(ui, st, us);
    let Some(b) = st.browser() else { return };
    let (n_b, n_x) = (b.runs.len(), st.runs.extra.len());
    ui.label(format!("{} runs; {} running or paused", n_b + n_x, st.runs.active()));
    ui.monospace(format!(
        "{:<23} {:<9} {:>8} {:>11}  {:>9} {:>9}  case",
        "id", "state", "step", "time", "res u", "res p"
    ));
    let row_h = ui.text_style_height(&egui::TextStyle::Monospace);
    let mut select: Option<String> = None;
    let selected = st.selected_run.clone();
    let cost = us.row_cost;
    let mut built = 0usize;
    egui::ScrollArea::vertical()
        .id_salt("runs")
        .auto_shrink([false; 2])
        .show_rows(ui, row_h, n_b + n_x, |ui, range| {
            for i in range {
                built += 1;
                if !cost.is_zero() {
                    let t = Instant::now();
                    while t.elapsed() < cost {
                        std::hint::spin_loop();
                    }
                }
                let (id, line) = if i < n_b {
                    let r = &b.runs[i];
                    (r.id.as_str(), row_line(r, st.runs.row(&r.id)))
                } else {
                    let Some(l) = st.runs.row(&st.runs.extra[i - n_b]) else {
                        continue;
                    };
                    (l.id.as_str(), live_line(l))
                };
                let text = RichText::new(line).monospace();
                if ui.selectable_label(selected.as_deref() == Some(id), text).clicked() {
                    select = Some(id.to_string());
                }
            }
        });
    us.rows_built += built;
    if let Some(id) = select {
        st.select_run(&id);
    }
}

fn runs_toolbar(ui: &mut egui::Ui, st: &mut Studio, us: &mut UiState) {
    let mut act: Option<&str> = None;
    ui.horizontal(|ui| {
        if ui.button("Start run...").clicked() {
            us.start_dialog = Some(StartDialog {
                case: st.editor.as_ref().map(|e| e.rel.clone()).unwrap_or_default(),
                ..Default::default()
            });
        }
        let sel = st.selected_run.clone();
        let row = sel.as_deref().and_then(|id| st.runs.row(id));
        let live = row.is_some_and(|r| !r.state.is_terminal());
        let paused = row.is_some_and(|r| r.held || r.state == rcfd_project::RunState::Paused);
        if ui.add_enabled(live && !paused, egui::Button::new("Pause")).clicked() {
            act = Some("pause");
        }
        if ui.add_enabled(live && paused, egui::Button::new("Resume")).clicked() {
            act = Some("resume");
        }
        if ui.add_enabled(live, egui::Button::new("Cancel")).clicked() {
            act = Some("cancel");
        }
        if ui.button("Budgets...").clicked() {
            let _ = st.load_budgets();
            us.budgets = Some(match &st.runs.config {
                Some(Ok(c)) => c.clone(),
                _ => rcfd_runner::runner::Config::default(),
            });
        }
    });
    ui.horizontal(|ui| match st.runs.runner_running {
        Some(true) => {
            ui.label("Runner running");
        }
        Some(false) => {
            ui.colored_label(WARN, "Runner not running; queued runs wait");
            let busy = st.runs.starting_runner;
            if ui
                .add_enabled(
                    !busy,
                    egui::Button::new(if busy { "Starting..." } else { "Start runner" }),
                )
                .clicked()
            {
                if let Err(m) = st.start_runner() {
                    st.say(m);
                }
            }
        }
        None => {
            ui.weak("reading runs...");
        }
    });
    if let Some(e) = &st.runs.runner_error {
        ui.colored_label(ERR, e);
    }
    if let (Some(a), Some(id)) = (act, st.selected_run.clone()) {
        let r = match a {
            "pause" => st.pause_run(&id),
            "resume" => st.resume_run(&id),
            _ => st.cancel_run(&id),
        };
        if let Err(m) = r {
            st.say(m);
        }
    }
}

fn start_dialog(ctx: &egui::Context, st: &mut Studio, us: &mut UiState) {
    let Some(mut d) = us.start_dialog.take() else { return };
    let mut open = true;
    let mut done = false;
    let cases = st.browser().map(|b| b.cases.clone()).unwrap_or_default();
    egui::Window::new("Start run")
        .open(&mut open)
        .collapsible(false)
        .show(ctx, |ui| {
            egui::ComboBox::from_label("Case")
                .selected_text(if d.case.is_empty() {
                    "choose a case"
                } else {
                    d.case.as_str()
                })
                .show_ui(ui, |ui| {
                    for c in &cases {
                        ui.selectable_value(&mut d.case, c.clone(), c);
                    }
                });
            if !d.case.is_empty() && d.asked.as_deref() != Some(d.case.as_str()) {
                d.asked = Some(d.case.clone());
                if let Err(m) = st.plan_start(&d.case) {
                    st.say(m);
                }
            }
            let mut start = false;
            match st.runs.plan.as_mut() {
                None if !d.case.is_empty() => {
                    ui.weak("reading the case...");
                }
                None => {}
                Some(Err(e)) => {
                    ui.colored_label(ERR, e.as_str());
                }
                Some(Ok(plan)) if plan.case == d.case => {
                    egui::Grid::new("start-plan").num_columns(2).show(ui, |ui| {
                        ui.label("Priority");
                        ui.add(egui::DragValue::new(&mut plan.priority));
                        ui.end_row();
                        ui.label("Threads");
                        ui.add(egui::DragValue::new(&mut plan.threads).range(1..=256));
                        ui.end_row();
                        ui.label("Memory MB");
                        ui.add(
                            egui::TextEdit::singleline(&mut d.mem)
                                .hint_text("estimated")
                                .desired_width(80.0),
                        );
                        ui.end_row();
                    });
                    store_ui(ui, plan);
                    ui.label("Overrides (path=value, one per line)");
                    ui.add(
                        egui::TextEdit::multiline(&mut d.overrides)
                            .desired_rows(3)
                            .desired_width(360.0),
                    );
                    plan.extra_set = d
                        .overrides
                        .lines()
                        .map(str::trim)
                        .filter(|l| !l.is_empty())
                        .map(String::from)
                        .collect();
                    plan.mem_mb = d.mem.trim().parse().ok();
                    ui.weak(format!("rcfd run --set: {}", plan.set().join(" ")));
                    start = ui.button("Start").clicked();
                }
                Some(Ok(_)) => {
                    ui.weak("reading the case...");
                }
            }
            if start {
                if let Some(Ok(plan)) = st.runs.plan.clone() {
                    match st.start_run(plan) {
                        Ok(()) => done = true,
                        Err(m) => st.say(m),
                    }
                }
            }
        });
    if open && !done {
        us.start_dialog = Some(d);
    }
}

fn store_ui(ui: &mut egui::Ui, plan: &mut crate::runs::StartPlan) {
    use crate::runs::StoreEvery;
    if let Some(n) = &plan.note {
        ui.weak(n.as_str());
    }
    let Some(s) = plan.store.as_mut() else { return };
    ui.checkbox(&mut s.on, "Store fields for scrubbing (ux, uy, uz, p)");
    if !s.on {
        return;
    }
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
        ui.radio_value(&mut s.f32, true, "f32 (lossy, half size)");
        ui.radio_value(&mut s.f32, false, "f64 (exact)");
    });
    match (plan.snapshots(), plan.estimated_bytes()) {
        (Some(n), Some(b)) => ui.weak(format!(
            "about {n} snapshots, estimated {:.1} MB (cells x 4 fields x snapshots x {} bytes)",
            b as f64 / 1e6,
            if plan.store.as_ref().is_some_and(|s| s.f32) {
                4
            } else {
                8
            }
        )),
        (Some(n), None) => ui.weak(format!("about {n} snapshots; cell count unknown before the run")),
        _ => ui.weak(""),
    };
}

fn budgets_dialog(ctx: &egui::Context, st: &mut Studio, us: &mut UiState) {
    let Some(mut c) = us.budgets.take() else { return };
    let mut open = true;
    let mut done = false;
    egui::Window::new("Runner budgets")
        .open(&mut open)
        .collapsible(false)
        .show(ctx, |ui| {
            if let Some(Err(e)) = &st.runs.config {
                ui.colored_label(ERR, e.as_str());
            }
            egui::Grid::new("budgets").num_columns(2).show(ui, |ui| {
                ui.label("Runs at once (max_runs)");
                ui.add(egui::DragValue::new(&mut c.max_runs).range(1..=64));
                ui.end_row();
                ui.label("Threads (cpu_threads)");
                ui.add(egui::DragValue::new(&mut c.cpu_threads).range(1..=1024));
                ui.end_row();
                ui.label("Memory MB (mem_budget_mb)");
                ui.add(egui::DragValue::new(&mut c.mem_budget_mb));
                ui.end_row();
            });
            if st.runs.runner_running == Some(true) {
                ui.colored_label(
                    WARN,
                    "The runner reads the budgets when it starts: shut it down and start it again.",
                );
            }
            if ui.button("Save").clicked() {
                match st.save_budgets(c.clone()) {
                    Ok(()) => done = true,
                    Err(m) => st.say(m),
                }
            }
        });
    if open && !done {
        us.budgets = Some(c);
    }
}

fn outputs_panel(ui: &mut egui::Ui, st: &mut Studio) {
    let (Some(b), Some(id)) = (st.browser(), st.selected_run.as_deref()) else {
        ui.weak("Select a run in the Runs panel.");
        return;
    };
    let live = st.runs.row(id);
    let Some(r) = b.runs.iter().find(|r| r.id == id) else {
        match live {
            Some(l) => {
                ui.label(RichText::new(&l.id).strong());
                ui.label(format!("case {}  created {}", l.case, l.created));
                ui.label(format!("{}, step {}, time {}", l.state_label(), l.step, l.time));
            }
            None => {
                ui.weak("The selected run is no longer listed.");
            }
        }
        return;
    };
    ui.label(RichText::new(&r.id).strong());
    ui.label(format!("case {}  created {}", r.case, r.created));
    match live {
        Some(l) => ui.label(format!("{}, step {}, time {}", l.state_label(), l.step, l.time)),
        None => ui.label(format!("{:?}, step {}, time {}", r.state, r.step, r.time).to_lowercase()),
    };
    if let Some(reason) = live.and_then(|l| l.reason.as_deref()) {
        ui.label(format!("reason: {reason}"));
    }
    if r.metrics.is_empty() {
        ui.weak("no numeric metrics.json entries");
    } else {
        egui::Grid::new("metrics").striped(true).show(ui, |ui| {
            for (k, v) in &r.metrics {
                ui.label(k);
                ui.monospace(format!("{v:.6e}"));
                ui.end_row();
            }
        });
    }
    if let Some(Value::Object(m)) = &st.runs.metrics {
        egui::CollapsingHeader::new("metrics.json")
            .id_salt("metrics-json")
            .show(ui, |ui| {
                for (k, v) in m {
                    ui.monospace(format!("{k}: {v}"));
                }
            });
    }
    let report = b.root.join("runs").join(&r.id).join("output").join("report.html");
    ui.weak(format!("report: {}", report.display()));
}

// ---------------------------------------------------------------- plots and log

fn plots_panel(ui: &mut egui::Ui, st: &mut Studio) {
    let (m, mon) = (&st.plots, &st.monitors);
    for w in &st.runs.warnings {
        ui.colored_label(WARN, w);
    }
    if m.series.is_empty() && mon.series.is_empty() {
        ui.weak(match &st.selected_run {
            Some(id) => format!("No residuals of run {id} yet."),
            None => "Select a run in the Runs panel to plot its residuals and monitors.".into(),
        });
        return;
    }
    let marker = st.plot_marker;
    let h = (ui.available_height() / 2.0 - 8.0).max(80.0);
    ui.label("Residuals (log10)");
    egui_plot::Plot::new("residuals").height(h).show(ui, |p| {
        for (i, n) in m.residual_names().into_iter().enumerate() {
            let colour = series_colour(i);
            for seg in m.plot_segments(&n, true, MAX_PLOT_POINTS) {
                p.line(egui_plot::Line::new(n.clone(), seg).color(colour));
            }
        }
        if let Some(x) = marker {
            p.vline(egui_plot::VLine::new("viewport step", x));
        }
    });
    ui.label("Monitors");
    egui_plot::Plot::new("reports").height(h).show(ui, |p| {
        // monitors of the run, then any non-residual series put into the residual store
        let mut k = 0;
        let all: Vec<String> = mon.series.keys().cloned().collect();
        for (store, names) in [(mon, all), (m, m.report_names())] {
            for n in names {
                let colour = series_colour(k);
                k += 1;
                for seg in store.plot_segments(&n, false, MAX_PLOT_POINTS) {
                    p.line(egui_plot::Line::new(n.clone(), seg).color(colour));
                }
            }
        }
        if let Some(x) = marker {
            p.vline(egui_plot::VLine::new("viewport step", x));
        }
    });
}

fn series_colour(i: usize) -> Color32 {
    const C: [Color32; 6] = [
        Color32::from_rgb(31, 119, 180),
        Color32::from_rgb(255, 127, 14),
        Color32::from_rgb(44, 160, 44),
        Color32::from_rgb(214, 39, 40),
        Color32::from_rgb(148, 103, 189),
        Color32::from_rgb(140, 86, 75),
    ];
    C[i % C.len()]
}

fn log_panel(ui: &mut egui::Ui, st: &mut Studio) {
    if let Some(id) = &st.selected_run {
        egui::CollapsingHeader::new(format!("log.txt of {id} (last {} lines)", crate::poll::LOG_LINES))
            .id_salt("solver-log")
            .default_open(true)
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("solver-log-text")
                    .max_height(220.0)
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        for l in &st.runs.log {
                            ui.monospace(l);
                        }
                    });
            });
        egui::CollapsingHeader::new(format!("events ({})", st.runs.events.len()))
            .id_salt("run-events")
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("run-events-table")
                    .max_height(220.0)
                    .show(ui, |ui| {
                        egui::Grid::new("events").striped(true).show(ui, |ui| {
                            for e in &st.runs.events {
                                ui.monospace(&e.t);
                                ui.monospace(event_text(&e.kind));
                                ui.end_row();
                            }
                        });
                    });
            });
    }
    ui.weak("Studio messages");
    egui::ScrollArea::vertical()
        .id_salt("log")
        .stick_to_bottom(true)
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            for m in &st.messages {
                ui.monospace(m);
            }
        });
}

fn event_text(k: &rcfd_project::records::EventKind) -> String {
    use rcfd_project::records::EventKind;
    let name = |s: &rcfd_project::RunState| crate::runs::state_name(*s);
    match k {
        EventKind::State { from, to, reason } => format!(
            "state {} -> {}{}",
            from.as_ref().map_or("-".into(), name),
            name(to),
            reason.as_ref().map_or(String::new(), |r| format!(": {r}"))
        ),
        EventKind::Warning { message } => format!("warning: {message}"),
        EventKind::Stop { reason } => format!("stop: {reason}"),
        EventKind::Other { kind, body } => {
            let mut s = body.to_string();
            if s.len() > 160 {
                let mut cut = 160;
                while !s.is_char_boundary(cut) {
                    cut -= 1;
                }
                s.truncate(cut);
                s.push_str("...");
            }
            format!("{kind} {s}")
        }
    }
}

// ---------------------------------------------------------------- case editor

enum Edit {
    Text(String, String),
    Value(String, Value),
    Add(String, String),
    Remove(String),
}

fn editor_panel(ui: &mut egui::Ui, st: &mut Studio, us: &mut UiState) {
    let Some(ed) = &mut st.editor else {
        ui.weak(match &st.pending_case {
            Some(c) => format!("Loading {c}..."),
            None => "Select a case in the Project panel.".into(),
        });
        return;
    };
    let mut save = false;
    let mut check = false;
    let mut take_disk = false;
    let mut keep_mine = false;
    ui.horizontal(|ui| {
        ui.label(RichText::new(&ed.rel).strong());
        let state = ed.state();
        let can_save = state == EditorState::Dirty && !ed.saving;
        save = ui.add_enabled(can_save, egui::Button::new("Save")).clicked();
        check = ui.button("Validate").clicked();
        ui.label("Filter");
        ui.add(egui::TextEdit::singleline(&mut us.filter).desired_width(140.0));
    });
    if let EditorState::Conflict { disk } = ed.state() {
        ui.colored_label(
            WARN,
            match &disk {
                Some(_) => {
                    "The file changed on disk while you edited it. Keep your edits (the next save replaces the \
                            file) or take the version on disk (your edits are dropped)."
                }
                None => "The file was removed on disk while you edited it. Keep your edits to write it again.",
            },
        );
        ui.horizontal(|ui| {
            keep_mine = ui.button("Keep my edits").clicked();
            take_disk = ui
                .add_enabled(disk.is_some(), egui::Button::new("Take the disk version"))
                .clicked();
        });
        if let Some(d) = &disk {
            egui::CollapsingHeader::new("Version on disk")
                .id_salt("disk")
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("disk-text")
                        .max_height(160.0)
                        .show(ui, |ui| {
                            ui.monospace(String::from_utf8_lossy(d));
                        });
                });
        }
    }
    for d in &ed.diagnostics {
        ui.colored_label(ERR, d);
    }
    let schema = st.schema.clone();
    let values = ed.settings.clone();
    let filter = us.filter.clone();
    let mut edits = Vec::new();
    egui::ScrollArea::vertical()
        .id_salt("settings")
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            for child in &schema.children {
                node_ui(
                    ui,
                    us,
                    &ed.field_errors,
                    child,
                    &child.name,
                    &values,
                    &filter,
                    &mut edits,
                );
            }
        });
    for e in edits {
        let r = match e {
            Edit::Text(p, t) => {
                let r = ed.edit_text(&schema, &p, &t);
                if r.is_err() {
                    us.buffers.insert(p, t);
                }
                r
            }
            Edit::Value(p, v) => ed.set_value(&p, v),
            Edit::Add(coll, name) => {
                let r = ed.add_named(&schema, &coll, &name);
                if r.is_ok() {
                    us.new_names.remove(&coll);
                }
                r
            }
            Edit::Remove(p) => ed.remove_named(&p),
        };
        if let Err(m) = r {
            st.messages.push(m);
        }
    }
    if keep_mine {
        if let Some(ed) = &mut st.editor {
            ed.keep_mine();
        }
    }
    if take_disk {
        if let Some(Err(m)) = st.editor.as_mut().map(|e| e.take_disk()) {
            st.say(m);
        }
    }
    if check {
        validate(st);
    }
    if save {
        if let Err(m) = st.save_case() {
            st.say(m);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn node_ui(
    ui: &mut egui::Ui,
    us: &mut UiState,
    errors: &BTreeMap<String, String>,
    node: &Node,
    path: &str,
    values: &Value,
    filter: &str,
    edits: &mut Vec<Edit>,
) {
    if !matches_filter(node, path, filter) {
        return;
    }
    match &node.kind {
        Kind::Group => {
            let mut h = egui::CollapsingHeader::new(&node.name).id_salt(path);
            if !filter.is_empty() {
                h = h.open(Some(true));
            }
            h.show(ui, |ui| {
                if !node.doc.is_empty() {
                    ui.weak(&node.doc);
                }
                for c in &node.children {
                    node_ui(ui, us, errors, c, &format!("{path}.{}", c.name), values, filter, edits);
                }
            });
        }
        Kind::Named => {
            let entries: Vec<String> = value_at(values, path)
                .and_then(Value::as_object)
                .map(|m| m.keys().cloned().collect())
                .unwrap_or_default();
            let mut h = egui::CollapsingHeader::new(format!("{} ({})", node.name, entries.len())).id_salt(path);
            if !filter.is_empty() {
                h = h.open(Some(true));
            }
            h.show(ui, |ui| {
                if !node.doc.is_empty() {
                    ui.weak(&node.doc);
                }
                let Some(item) = node.children.first() else { return };
                for name in &entries {
                    let epath = format!("{path}.{name}");
                    ui.horizontal(|ui| {
                        egui::CollapsingHeader::new(name).id_salt(&epath).show(ui, |ui| {
                            for c in &item.children {
                                node_ui(ui, us, errors, c, &format!("{epath}.{}", c.name), values, "", edits);
                            }
                        });
                        if ui.small_button("remove").on_hover_text("delete this entry").clicked() {
                            edits.push(Edit::Remove(epath.clone()));
                        }
                    });
                }
                ui.horizontal(|ui| {
                    let buf = us.new_names.entry(path.to_string()).or_default();
                    ui.add(
                        egui::TextEdit::singleline(buf)
                            .hint_text("new name")
                            .desired_width(110.0),
                    );
                    if ui.button("add").clicked() {
                        edits.push(Edit::Add(path.to_string(), buf.clone()));
                    }
                });
                if let Some(e) = errors.get(path) {
                    ui.colored_label(ERR, e);
                }
            });
        }
        _ => leaf_ui(ui, us, errors, node, path, values, edits),
    }
}

fn leaf_ui(
    ui: &mut egui::Ui,
    us: &mut UiState,
    errors: &BTreeMap<String, String>,
    node: &Node,
    path: &str,
    values: &Value,
    edits: &mut Vec<Edit>,
) {
    let current = value_at(values, path).cloned().unwrap_or_else(|| node.default.clone());
    let err = errors.get(path).cloned();
    ui.horizontal(|ui| {
        let label = ui.label(&node.name);
        if !node.doc.is_empty() {
            label.on_hover_text(&node.doc);
        }
        match &node.kind {
            Kind::Bool => {
                let mut b = current.as_bool().unwrap_or(false);
                if ui.checkbox(&mut b, "").changed() {
                    edits.push(Edit::Value(path.to_string(), Value::Bool(b)));
                }
            }
            Kind::Enum(options) => {
                let cur = current.as_str().unwrap_or_default().to_string();
                egui::ComboBox::from_id_salt(("enum", path))
                    .selected_text(cur.clone())
                    .show_ui(ui, |ui| {
                        for o in options {
                            if ui.selectable_label(&cur == o, o).clicked() && &cur != o {
                                edits.push(Edit::Value(path.to_string(), Value::String(o.clone())));
                            }
                        }
                    });
            }
            _ => {
                let buf = us
                    .buffers
                    .entry(path.to_string())
                    .or_insert_with(|| format_value(&current));
                let r = ui.add(egui::TextEdit::singleline(buf).desired_width(130.0));
                if r.lost_focus() {
                    let text = buf.clone();
                    us.buffers.remove(path);
                    if text != format_value(&current) {
                        edits.push(Edit::Text(path.to_string(), text));
                    }
                } else if !r.has_focus() && err.is_none() {
                    // Not editing: follow the stored value.
                    *buf = format_value(&current);
                }
                match &node.kind {
                    Kind::Float { dim: Some(d), .. } => {
                        ui.weak(d);
                    }
                    Kind::Int {
                        min: Some(a),
                        max: Some(b),
                    }
                    | Kind::Float {
                        min: Some(a),
                        max: Some(b),
                        ..
                    } => {
                        ui.weak(format!("[{a}, {b}]"));
                    }
                    _ => {}
                }
            }
        }
    });
    if let Some(e) = err {
        ui.colored_label(ERR, e);
    }
}
