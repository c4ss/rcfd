//! Drawing code: menus, docked panels and the eframe application.
//!
//! All state and decisions live in the non-UI modules; this file only draws and
//! forwards user actions. It has been exercised headlessly (frames run without
//! a window) but not inspected visually by the author.

use crate::chat::{CardState, Entry};
use crate::files;
use crate::layout::{self, Panel};
use crate::schema_model::{format_value, matches_filter, value_at, Kind, Node};
use crate::state::{AppState, Prompt, QuickAction};
use crate::theme::{Prefs, Theme};
use crate::viewport::{cell_square_size, colormap, legend_ticks, Range, ViewMode};
use egui::{Align2, Color32, FontId, Pos2, Rect, RichText, Sense, Vec2};
use egui_dock::{DockArea, DockState, TabViewer};
use rcfd_api::schema::FIELD_NAMES;
use rcfd_assistant::config::ProviderKind;
use rcfd_assistant::ApprovalMode;
use serde_json::Value;

/// The eframe application: docked panels around shared state.
pub struct RcfdApp {
    /// Panel layout.
    pub dock: DockState<Panel>,
    /// Everything the panels use.
    pub st: AppState,
    theme_applied: Option<Theme>,
    last_prefs: Option<String>,
}

impl RcfdApp {
    /// Build the app. Saved preferences (theme, layout) are restored from `st.prefs_path`.
    pub fn new(mut st: AppState) -> RcfdApp {
        let mut dock = layout::default_layout();
        if let Some(p) = st.prefs_path.as_deref().and_then(Prefs::load) {
            st.theme = p.theme;
            if let Some(d) = p.dock() {
                dock = d;
            }
        }
        RcfdApp {
            dock,
            st,
            theme_applied: None,
            last_prefs: None,
        }
    }

    fn save_prefs_if_changed(&mut self) {
        let Some(path) = self.st.prefs_path.clone() else { return };
        let prefs = Prefs::capture(self.st.theme, &self.dock);
        let key = format!("{:?}{}", prefs.theme, prefs.layout.as_deref().unwrap_or_default());
        if self.last_prefs.as_deref() != Some(&key) {
            // Only a failed write is worth telling the user about.
            if let Err(e) = prefs.save(&path) {
                self.st.ctrl.message = Some(format!("could not save preferences: {e}"));
            }
            self.last_prefs = Some(key);
        }
    }

    /// Draw one frame. `eframe::App::update` calls this; tests call it directly with a bare context.
    pub fn ui(&mut self, ctx: &egui::Context) {
        if self.theme_applied != Some(self.st.theme) {
            self.st.theme.apply(ctx);
            self.theme_applied = Some(self.st.theme);
        }
        let active = self.st.tick();
        self.menu_bar(ctx);
        self.status_bar(ctx);
        self.file_prompt(ctx);
        let style = egui_dock::Style::from_egui(ctx.style().as_ref());
        DockArea::new(&mut self.dock)
            .style(style)
            .show(ctx, &mut Viewer { st: &mut self.st });
        self.save_prefs_if_changed();
        ctx.request_repaint_after(if active {
            std::time::Duration::from_millis(100)
        } else {
            std::time::Duration::from_millis(500)
        });
    }

    fn menu_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("New lid-driven cavity").clicked() {
                        if let Err(e) = self.st.ctrl.new_from_template() {
                            self.st.ctrl.message = Some(e);
                        }
                        ui.close_menu();
                    }
                    if ui.button("Open...").clicked() {
                        ui.close_menu();
                        let st = &mut self.st;
                        match files::open_with_dialog(&mut st.ctrl, st.chooser.as_ref(), st.dialog_dir.as_deref()) {
                            files::Outcome::Failed(e) => st.ctrl.message = Some(e),
                            files::Outcome::Done(_) | files::Outcome::Cancelled => {}
                        }
                    }
                    if ui.button("Open from typed path...").clicked() {
                        self.st.prompt = Some(Prompt::Open);
                        ui.close_menu();
                    }
                    let can_save = self.st.ctrl.session.is_some();
                    if ui
                        .add_enabled(can_save && self.st.ctrl.file.is_some(), egui::Button::new("Save"))
                        .clicked()
                    {
                        if let Some(f) = self.st.ctrl.file.clone() {
                            if let Err(e) = self.st.ctrl.save_to(&f) {
                                self.st.ctrl.message = Some(e);
                            }
                        }
                        ui.close_menu();
                    }
                    if ui.add_enabled(can_save, egui::Button::new("Save as...")).clicked() {
                        ui.close_menu();
                        let st = &mut self.st;
                        match files::save_as_with_dialog(&mut st.ctrl, st.chooser.as_ref(), st.dialog_dir.as_deref()) {
                            files::Outcome::Failed(e) => st.ctrl.message = Some(e),
                            files::Outcome::Done(_) | files::Outcome::Cancelled => {}
                        }
                    }
                    if ui
                        .add_enabled(can_save, egui::Button::new("Save as typed path..."))
                        .clicked()
                    {
                        self.st.prompt = Some(Prompt::SaveAs);
                        self.st.prompt_path = self
                            .st
                            .ctrl
                            .file
                            .as_ref()
                            .map(|p| p.display().to_string())
                            .unwrap_or_else(|| "case.toml".into());
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Quit").clicked() {
                        self.st.quit = true;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.menu_button("Run", |ui| {
                    let s = self.st.ctrl.status.state.clone();
                    let has = self.st.ctrl.session.is_some();
                    if ui
                        .add_enabled(has && s != "running" && s != "paused", egui::Button::new("Run"))
                        .clicked()
                    {
                        self.st.ctrl.run_start();
                        ui.close_menu();
                    }
                    if ui.add_enabled(s == "running", egui::Button::new("Pause")).clicked() {
                        self.st.ctrl.run_pause();
                        ui.close_menu();
                    }
                    if ui.add_enabled(s == "paused", egui::Button::new("Resume")).clicked() {
                        self.st.ctrl.run_resume();
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(s == "running" || s == "paused", egui::Button::new("Stop"))
                        .clicked()
                    {
                        self.st.ctrl.run_stop();
                        ui.close_menu();
                    }
                });
                ui.menu_button("View", |ui| {
                    for p in Panel::ALL {
                        let mut open = layout::is_open(&self.dock, p);
                        if ui.checkbox(&mut open, p.title()).changed() {
                            layout::toggle(&mut self.dock, p);
                        }
                    }
                    ui.separator();
                    ui.menu_button("Float panel", |ui| {
                        for p in Panel::ALL {
                            if ui.button(p.title()).clicked() {
                                layout::float(&mut self.dock, p);
                                ui.close_menu();
                            }
                        }
                    });
                    if ui.button("Reset layout").clicked() {
                        self.dock = layout::default_layout();
                        ui.close_menu();
                    }
                    ui.separator();
                    let label = if self.st.theme == Theme::Dark {
                        "Light theme"
                    } else {
                        "Dark theme"
                    };
                    if ui.button(label).clicked() {
                        self.st.theme = self.st.theme.toggled();
                        ui.close_menu();
                    }
                });
            });
        });
    }

    fn status_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let c = &self.st.ctrl;
                let dirty = if c.dirty { " *" } else { "" };
                ui.label(RichText::new(format!("{}{dirty}", c.title())).strong());
                ui.separator();
                ui.label(format!(
                    "{} | iter {} | cells {}",
                    c.status.state, c.status.iteration, c.status.cells
                ));
                if let Some(m) = &c.message {
                    ui.separator();
                    ui.label(m);
                }
            });
        });
    }

    fn file_prompt(&mut self, ctx: &egui::Context) {
        let Some(kind) = self.st.prompt else { return };
        let mut open = true;
        let mut done = false;
        let title = if kind == Prompt::Open {
            "Open case"
        } else {
            "Save case as"
        };
        egui::Window::new(title)
            .open(&mut open)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.label("Path of a .toml or .json case file");
                let r = ui.add(egui::TextEdit::singleline(&mut self.st.prompt_path).desired_width(360.0));
                let go = ui.button("OK").clicked() || (r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));
                if go {
                    let path = std::path::PathBuf::from(self.st.prompt_path.trim());
                    let res = match kind {
                        Prompt::Open => self.st.ctrl.open_path(&path),
                        Prompt::SaveAs => self.st.ctrl.save_to(&path),
                    };
                    if let Err(e) = res {
                        self.st.ctrl.message = Some(e);
                    }
                    done = true;
                }
            });
        if !open || done {
            self.st.prompt = None;
        }
    }
}

impl eframe::App for RcfdApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.ui(ctx);
    }
}

struct Viewer<'a> {
    st: &'a mut AppState,
}

impl TabViewer for Viewer<'_> {
    type Tab = Panel;

    fn title(&mut self, tab: &mut Panel) -> egui::WidgetText {
        tab.title().into()
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Panel) {
        match tab {
            Panel::Settings => settings_panel(ui, self.st),
            Panel::Viewport => viewport_panel(ui, self.st),
            Panel::Monitors => monitors_panel(ui, self.st),
            Panel::Log => log_panel(ui, self.st),
            Panel::Assistant => assistant_panel(ui, self.st),
        }
    }
}

// ---------------------------------------------------------------- settings

enum Edit {
    Text(String, String),
    Value(String, Value),
    Add(String, String),
    Remove(String),
    Select(String),
}

fn settings_panel(ui: &mut egui::Ui, st: &mut AppState) {
    if st.ctrl.session.is_none() {
        ui.label("No case open. Use File > New or Open.");
        return;
    }
    ui.horizontal(|ui| {
        ui.label("Filter");
        ui.add(egui::TextEdit::singleline(&mut st.settings_filter).desired_width(160.0));
        if ui.button("Validate").clicked() {
            let ok = st.ctrl.validate();
            st.ctrl.message = Some(if ok {
                "case is valid".into()
            } else {
                "case has problems (see below)".into()
            });
        }
    });
    if !st.ctrl.diagnostics.is_empty() {
        for d in &st.ctrl.diagnostics {
            ui.colored_label(Color32::from_rgb(200, 60, 60), d);
        }
    }
    let locked = matches!(st.ctrl.status.state.as_str(), "running" | "paused");
    if locked {
        ui.colored_label(
            Color32::from_rgb(200, 140, 0),
            "Settings are locked while the solver runs.",
        );
    }
    let schema = st.ctrl.schema.clone();
    let values = st.ctrl.settings.clone();
    let filter = st.settings_filter.clone();
    let mut edits = vec![];
    egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
        ui.add_enabled_ui(!locked, |ui| {
            for child in &schema.children {
                node_ui(ui, st, child, &child.name, &values, &filter, &mut edits);
            }
        });
    });
    for e in edits {
        match e {
            Edit::Text(p, t) => {
                if !st.ctrl.edit_text(&p, &t) {
                    st.buffers.insert(p, t);
                }
            }
            Edit::Value(p, v) => {
                st.ctrl.set_value(&p, v);
            }
            Edit::Add(coll, name) => {
                if st.ctrl.add_named(&coll, &name) {
                    st.new_names.remove(&coll);
                }
            }
            Edit::Remove(p) => {
                st.ctrl.remove_named(&p);
            }
            Edit::Select(p) => st.ctrl.selected = Some(p),
        }
    }
}

fn node_ui(
    ui: &mut egui::Ui,
    st: &mut AppState,
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
            let open = !filter.is_empty();
            let mut h = egui::CollapsingHeader::new(&node.name).id_source(path);
            if open {
                h = h.open(Some(true));
            }
            h.show(ui, |ui| {
                if !node.doc.is_empty() {
                    ui.weak(&node.doc);
                }
                for c in &node.children {
                    node_ui(ui, st, c, &format!("{path}.{}", c.name), values, filter, edits);
                }
            });
        }
        Kind::Named => {
            let entries: Vec<String> = value_at(values, path)
                .and_then(Value::as_object)
                .map(|m| m.keys().cloned().collect())
                .unwrap_or_default();
            let mut h = egui::CollapsingHeader::new(format!("{} ({})", node.name, entries.len())).id_source(path);
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
                        egui::CollapsingHeader::new(name).id_source(&epath).show(ui, |ui| {
                            for c in &item.children {
                                node_ui(ui, st, c, &format!("{epath}.{}", c.name), values, "", edits);
                            }
                        });
                        if ui.small_button("remove").on_hover_text("delete this entry").clicked() {
                            edits.push(Edit::Remove(epath.clone()));
                        }
                    });
                }
                ui.horizontal(|ui| {
                    let buf = st.new_names.entry(path.to_string()).or_default();
                    ui.add(
                        egui::TextEdit::singleline(buf)
                            .hint_text("new name")
                            .desired_width(110.0),
                    );
                    if ui.button("add").clicked() {
                        edits.push(Edit::Add(path.to_string(), buf.clone()));
                    }
                });
                if let Some(e) = st.ctrl.field_errors.get(path) {
                    ui.colored_label(Color32::from_rgb(200, 60, 60), e);
                }
            });
        }
        _ => leaf_ui(ui, st, node, path, values, edits),
    }
}

fn leaf_ui(ui: &mut egui::Ui, st: &mut AppState, node: &Node, path: &str, values: &Value, edits: &mut Vec<Edit>) {
    let current = value_at(values, path).cloned().unwrap_or_else(|| node.default.clone());
    let err = st.ctrl.field_errors.get(path).cloned();
    ui.horizontal(|ui| {
        let selected = st.ctrl.selected.as_deref() == Some(path);
        let label = ui.selectable_label(selected, &node.name);
        let label = if node.doc.is_empty() {
            label
        } else {
            label.on_hover_text(&node.doc)
        };
        if label.clicked() {
            edits.push(Edit::Select(path.to_string()));
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
                egui::ComboBox::from_id_source(("enum", path))
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
                let buf = st
                    .buffers
                    .entry(path.to_string())
                    .or_insert_with(|| format_value(&current));
                let r = ui.add(egui::TextEdit::singleline(buf).desired_width(130.0));
                if r.lost_focus() {
                    let text = buf.clone();
                    st.buffers.remove(path);
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
        ui.colored_label(Color32::from_rgb(200, 60, 60), e);
    }
}

// ---------------------------------------------------------------- viewport

fn viewport_panel(ui: &mut egui::Ui, st: &mut AppState) {
    ui.horizontal(|ui| {
        let cur = st.ctrl.view.field.clone();
        egui::ComboBox::from_id_source("vp-field")
            .selected_text(cur.clone())
            .show_ui(ui, |ui| {
                for f in FIELD_NAMES {
                    if ui.selectable_label(cur == *f, *f).clicked() {
                        st.ctrl.set_view_field(f);
                    }
                }
            });
        let mut mode = st.ctrl.view.mode;
        ui.selectable_value(&mut mode, ViewMode::Raster, "Raster");
        ui.selectable_value(&mut mode, ViewMode::Cells, "Cells");
        if mode != st.ctrl.view.mode {
            st.ctrl.view.mode = mode;
            st.ctrl.poll(true);
        }
        let mut fixed = matches!(st.ctrl.view.range, Range::Fixed(..));
        if ui.checkbox(&mut fixed, "fixed range").changed() {
            st.ctrl.view.range = if fixed {
                let (lo, hi) = st.ctrl.grid.as_ref().map_or((0.0, 1.0), |g| (g.min, g.max));
                Range::Fixed(lo, if hi > lo { hi } else { lo + 1.0 })
            } else {
                Range::Auto
            };
        }
        if let Range::Fixed(lo, hi) = &mut st.ctrl.view.range {
            ui.add(egui::DragValue::new(lo).speed(0.01));
            ui.add(egui::DragValue::new(hi).speed(0.01));
        }
    });
    ui.separator();
    let have_raster = st.ctrl.view.mode == ViewMode::Raster && st.ctrl.grid.is_some();
    let have_cells = st.ctrl.view.mode == ViewMode::Cells && st.ctrl.slice.is_some();
    if !have_raster && !have_cells {
        ui.label("Nothing to show yet: open a case (the mesh is built automatically for small cases).");
        return;
    }
    let avail = ui.available_size();
    let legend_w = 70.0;
    let (resp, painter) = ui.allocate_painter(avail, Sense::hover());
    let area = Rect::from_min_size(resp.rect.min, Vec2::new((avail.x - legend_w).max(10.0), avail.y));
    let text_col = ui.visuals().text_color();
    let (lo, hi, hover);
    if have_raster {
        let g = st.ctrl.grid.as_ref().unwrap();
        let (a, b) = st.ctrl.view.effective_range(g.min, g.max);
        lo = a;
        hi = b;
        let aspect = st
            .ctrl
            .slice
            .as_ref()
            .map_or(g.cols as f32 / g.rows as f32, |s| s.aspect() as f32);
        let fit = fit_rect(area, aspect);
        let (cw, ch) = (fit.width() / g.cols as f32, fit.height() / g.rows as f32);
        for r in 0..g.rows {
            for c in 0..g.cols {
                if let Some(v) = g.values[r * g.cols + c] {
                    let [rr, gg, bb] = colormap(v, lo, hi);
                    let min = Pos2::new(fit.min.x + c as f32 * cw, fit.min.y + r as f32 * ch);
                    painter.rect_filled(
                        Rect::from_min_size(min, Vec2::new(cw + 0.5, ch + 0.5)),
                        0.0,
                        Color32::from_rgb(rr, gg, bb),
                    );
                }
            }
        }
        hover = resp.hover_pos().filter(|p| fit.contains(*p)).and_then(|p| {
            let c = (((p.x - fit.min.x) / cw) as usize).min(g.cols - 1);
            let r = (((p.y - fit.min.y) / ch) as usize).min(g.rows - 1);
            g.values[r * g.cols + c].map(|v| format!("{} = {v:.5e}", g.name))
        });
    } else {
        let s = st.ctrl.slice.as_ref().unwrap();
        let ext = s.extent().unwrap_or((0.0, 1.0, 0.0, 1.0));
        let (mut a, mut b) = (f64::INFINITY, f64::NEG_INFINITY);
        for v in s.values.iter().filter(|v| v.is_finite()) {
            a = a.min(*v);
            b = b.max(*v);
        }
        let (l, h) = st
            .ctrl
            .view
            .effective_range(if a.is_finite() { a } else { 0.0 }, if b.is_finite() { b } else { 1.0 });
        lo = l;
        hi = h;
        let fit = fit_rect(area, s.aspect() as f32);
        let size = cell_square_size(ext, s.x.len()) as f32;
        let sx = fit.width() / (ext.1 - ext.0).max(1e-12) as f32;
        let sy = fit.height() / (ext.3 - ext.2).max(1e-12) as f32;
        for i in 0..s.x.len() {
            let px = fit.min.x + (s.x[i] - ext.0) as f32 * sx;
            let py = fit.max.y - (s.y[i] - ext.2) as f32 * sy;
            let [rr, gg, bb] = colormap(s.values[i], lo, hi);
            painter.rect_filled(
                Rect::from_center_size(Pos2::new(px, py), Vec2::new(size * sx + 0.5, size * sy + 0.5)),
                0.0,
                Color32::from_rgb(rr, gg, bb),
            );
        }
        hover = None;
    }
    // Legend
    let lg = Rect::from_min_size(
        Pos2::new(area.max.x + 10.0, area.min.y + 10.0),
        Vec2::new(14.0, (avail.y - 20.0).max(20.0)),
    );
    let steps = 48;
    for i in 0..steps {
        let t = 1.0 - (i as f64 + 0.5) / steps as f64;
        let [r, g, b] = colormap(lo + t * (hi - lo), lo, hi);
        let y0 = lg.min.y + lg.height() * i as f32 / steps as f32;
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(lg.min.x, y0),
                Pos2::new(lg.max.x, y0 + lg.height() / steps as f32 + 0.5),
            ),
            0.0,
            Color32::from_rgb(r, g, b),
        );
    }
    for t in legend_ticks(lo, hi, 5) {
        let y = lg.max.y - lg.height() * ((t - lo) / (hi - lo).max(1e-300)) as f32;
        painter.text(
            Pos2::new(lg.max.x + 4.0, y),
            Align2::LEFT_CENTER,
            format!("{t:.3}"),
            FontId::proportional(11.0),
            text_col,
        );
    }
    if let Some(h) = hover {
        resp.on_hover_text(h);
    }
}

fn fit_rect(area: Rect, aspect: f32) -> Rect {
    let aspect = aspect.clamp(0.05, 20.0);
    let (w, h) = if area.width() / area.height().max(1.0) > aspect {
        (area.height() * aspect, area.height())
    } else {
        (area.width(), area.width() / aspect)
    };
    Rect::from_center_size(area.center(), Vec2::new(w.max(1.0), h.max(1.0)))
}

// ---------------------------------------------------------------- monitors

fn monitors_panel(ui: &mut egui::Ui, st: &mut AppState) {
    let state = st.ctrl.status.state.clone();
    let has = st.ctrl.session.is_some();
    ui.horizontal(|ui| {
        if ui
            .add_enabled(has && state != "running" && state != "paused", egui::Button::new("Run"))
            .clicked()
        {
            st.ctrl.run_start();
        }
        if ui.add_enabled(state == "running", egui::Button::new("Pause")).clicked() {
            st.ctrl.run_pause();
        }
        if ui.add_enabled(state == "paused", egui::Button::new("Resume")).clicked() {
            st.ctrl.run_resume();
        }
        if ui
            .add_enabled(state == "running" || state == "paused", egui::Button::new("Stop"))
            .clicked()
        {
            st.ctrl.run_stop();
        }
        if ui
            .add_enabled(
                has && state != "running" && state != "paused",
                egui::Button::new("Step"),
            )
            .clicked()
        {
            st.ctrl.run_step(1);
        }
        ui.checkbox(&mut st.log_plot, "log y");
        let s = &st.ctrl.status;
        ui.label(format!(
            "{} | iter {} | res_u {} | res_p {} | continuity {}",
            s.state,
            s.iteration,
            fmt_opt(s.res_u),
            fmt_opt(s.res_p),
            fmt_opt(s.continuity)
        ));
    });
    let log = st.log_plot;
    let names = st.ctrl.monitors.residual_names();
    let reports = st.ctrl.monitors.report_names();
    let h = if reports.is_empty() {
        ui.available_height()
    } else {
        ui.available_height() * 0.6
    };
    egui_plot::Plot::new("residuals")
        .height(h.max(80.0))
        .legend(egui_plot::Legend::default())
        .x_axis_label("iteration")
        .y_axis_label(if log { "log10 residual" } else { "residual" })
        .show(ui, |plot| {
            for n in &names {
                let pts = st.ctrl.monitors.plot_points(n, log);
                plot.line(egui_plot::Line::new(egui_plot::PlotPoints::new(pts)).name(n));
            }
        });
    if !reports.is_empty() {
        egui_plot::Plot::new("reports")
            .legend(egui_plot::Legend::default())
            .show(ui, |plot| {
                for n in &reports {
                    let pts = st.ctrl.monitors.plot_points(n, false);
                    plot.line(egui_plot::Line::new(egui_plot::PlotPoints::new(pts)).name(n));
                }
            });
    }
}

fn fmt_opt(v: Option<f64>) -> String {
    v.map_or("-".into(), |x| format!("{x:.2e}"))
}

fn log_panel(ui: &mut egui::Ui, st: &mut AppState) {
    egui::ScrollArea::vertical()
        .stick_to_bottom(true)
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            for l in &st.ctrl.log {
                ui.label(RichText::new(l).monospace());
            }
            if st.ctrl.log.is_empty() {
                ui.weak("The solver log appears here.");
            }
        });
}

// ---------------------------------------------------------------- assistant

fn chip(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.label(
        RichText::new(format!(" {label}: {value} "))
            .small()
            .background_color(ui.visuals().faint_bg_color),
    );
}

fn assistant_panel(ui: &mut egui::Ui, st: &mut AppState) {
    // Header: provider, model, mode.
    ui.horizontal_wrapped(|ui| {
        let names: Vec<String> = st.setup.config.providers.iter().map(|p| p.name.clone()).collect();
        let cur = st
            .setup
            .config
            .active
            .clone()
            .unwrap_or_else(|| "choose provider".into());
        egui::ComboBox::from_id_source("provider")
            .selected_text(cur)
            .show_ui(ui, |ui| {
                for n in &names {
                    if ui
                        .selectable_label(st.setup.config.active.as_deref() == Some(n), n)
                        .clicked()
                    {
                        st.setup.select(n);
                    }
                }
            });
        let mut model = st.setup.active().map(|c| c.model.clone());
        if let Some(m) = &mut model {
            if ui
                .add(egui::TextEdit::singleline(m).hint_text("model").desired_width(120.0))
                .changed()
            {
                if let Some(c) = st.setup.active_mut() {
                    c.model = m.clone();
                }
            }
        }
        if ui.button("Connect").clicked() {
            st.connect_selected();
            st.setup.save();
        }
    });
    egui::CollapsingHeader::new("Provider settings")
        .id_source("prov-settings")
        .show(ui, |ui| {
            let kind = st.setup.active().map(|c| c.kind);
            if let Some(c) = st.setup.active_mut() {
                ui.horizontal(|ui| {
                    ui.label("Base URL");
                    ui.add(egui::TextEdit::singleline(&mut c.base_url).desired_width(220.0));
                });
                ui.horizontal(|ui| {
                    ui.label("Key env var");
                    let mut e = c.key_env.clone().unwrap_or_default();
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut e)
                                .hint_text("optional")
                                .desired_width(140.0),
                        )
                        .changed()
                    {
                        c.key_env = if e.is_empty() { None } else { Some(e) };
                    }
                });
                ui.label(if c.local {
                    "Local endpoint: no key needed, works offline."
                } else {
                    "Hosted endpoint: requests leave this machine."
                });
            }
            if kind.is_some() {
                ui.horizontal(|ui| {
                    ui.label("API key");
                    ui.add(
                        egui::TextEdit::singleline(&mut st.setup.key_input)
                            .password(true)
                            .desired_width(180.0),
                    );
                    if ui.button("Store").clicked() {
                        st.setup.save_key();
                    }
                    if ui.button("Forget").clicked() {
                        st.setup.delete_key();
                    }
                });
                ui.weak(if st.setup.has_key() {
                    "a key is available"
                } else {
                    "no key stored"
                });
                ui.weak(st.setup.key_storage_note());
                if kind == Some(ProviderKind::OpenAiCompatible) && ui.button("List models").clicked() {
                    match st.setup.probe(None) {
                        Ok(m) => {
                            st.setup.status = Some(format!("{} model(s) found", m.len()));
                            st.setup.models = m;
                        }
                        Err(e) => st.setup.status = Some(e.to_string()),
                    }
                }
                if !st.setup.models.is_empty() {
                    let models = st.setup.models.clone();
                    ui.horizontal_wrapped(|ui| {
                        for m in models {
                            if ui.small_button(&m).clicked() {
                                if let Some(c) = st.setup.active_mut() {
                                    c.model = m;
                                }
                            }
                        }
                    });
                }
            }
            if let Some(s) = &st.setup.status {
                ui.label(s);
            }
        });
    // Mode and context chips.
    ui.horizontal(|ui| {
        let ro = st.mode() == ApprovalMode::ReadOnly;
        if ui.selectable_label(ro, "Read-only").clicked() {
            st.set_mode(ApprovalMode::ReadOnly);
        }
        if ui.selectable_label(!ro, "Approve writes").clicked() {
            st.set_mode(ApprovalMode::ApproveWrites);
        }
    });
    ui.horizontal_wrapped(|ui| {
        let c = st.context();
        chip(ui, "case", c.case_title.as_deref().unwrap_or("none"));
        chip(ui, "run", c.run_state.as_deref().unwrap_or("none"));
        if let Some(s) = &c.selection {
            chip(ui, "selected", s);
        }
        if let Some(m) = &st.active_model {
            chip(ui, "model", m);
        }
    });
    ui.horizontal_wrapped(|ui| {
        ui.weak("No model needed:");
        for a in QuickAction::ALL {
            if ui
                .add_enabled(st.ctrl.session.is_some(), egui::Button::new(a.label()).small())
                .clicked()
            {
                st.quick_action(a);
            }
        }
    });
    ui.separator();
    // Transcript.
    let input_h = 70.0;
    let avail = (ui.available_height() - input_h).max(60.0);
    let mut approve = false;
    let mut deny = false;
    egui::ScrollArea::vertical()
        .max_height(avail)
        .stick_to_bottom(true)
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            if st.chat.entries.is_empty() {
                ui.weak(if st.worker.is_some() {
                    "Ask the assistant to set up, check or run a case."
                } else {
                    "Choose a provider and model, then Connect. Local endpoints (Ollama, LM Studio, ...) need no key."
                });
            }
            for e in &st.chat.entries {
                match e {
                    Entry::User(t) => {
                        ui.label(RichText::new("You").strong());
                        ui.label(t);
                    }
                    Entry::Assistant(t) => {
                        ui.label(RichText::new("Assistant").strong());
                        ui.label(t);
                    }
                    Entry::Notice(t) => {
                        ui.colored_label(Color32::from_rgb(200, 140, 0), t);
                    }
                    Entry::Tool {
                        name,
                        args,
                        state,
                        result,
                        mutating,
                        ..
                    } => {
                        egui::Frame::group(ui.style()).show(ui, |ui| {
                            let (tag, col) = match state {
                                CardState::Pending => ("needs approval", Color32::from_rgb(200, 140, 0)),
                                CardState::Done => ("done", Color32::from_rgb(60, 160, 80)),
                                CardState::Failed => ("failed", Color32::from_rgb(200, 60, 60)),
                                CardState::Denied => ("denied", Color32::from_rgb(200, 60, 60)),
                            };
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(name).monospace().strong());
                                ui.colored_label(col, tag);
                                if *mutating {
                                    ui.weak("changes state");
                                }
                            });
                            ui.label(RichText::new(args.to_string()).monospace().small());
                            if *state == CardState::Pending {
                                ui.horizontal(|ui| {
                                    approve |= ui.button("Approve").clicked();
                                    deny |= ui.button("Deny").clicked();
                                });
                            }
                            if let Some(r) = result {
                                egui::CollapsingHeader::new("result")
                                    .id_source(("res", e as *const Entry as usize))
                                    .show(ui, |ui| {
                                        ui.label(RichText::new(r).monospace().small());
                                    });
                            }
                        });
                    }
                }
            }
            if st.chat.busy && st.chat.pending.is_none() {
                ui.weak("thinking...");
            }
        });
    if approve {
        st.approve();
    }
    if deny {
        st.deny();
    }
    // Input.
    ui.horizontal(|ui| {
        let r = ui.add(
            egui::TextEdit::multiline(&mut st.chat.input)
                .desired_rows(2)
                .desired_width(ui.available_width() - 70.0)
                .hint_text("Ask the assistant (Ctrl+Enter to send)"),
        );
        let ctrl_enter = r.has_focus() && ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Enter));
        ui.vertical(|ui| {
            if ui.add_enabled(st.can_send(), egui::Button::new("Send")).clicked() || (ctrl_enter && st.can_send()) {
                st.send_message();
            }
            if st.chat.busy && ui.button("Stop").clicked() {
                st.cancel_turn();
            }
        });
    });
}
