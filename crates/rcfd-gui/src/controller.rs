//! Application model: the open case, run control and polled results.
//!
//! Everything goes through the in-process solver service (`rcfd-api`), the same
//! one the assistant's tools use, so GUI edits, assistant edits and the journal
//! stay consistent. No UI code; every method is callable from tests.

use crate::monitors::MonitorStore;
use crate::schema_model::{build_tree, parse_input, Node};
use crate::viewport::{GridData, SliceData, ViewState};
use rcfd_api::case::Case;
use rcfd_api::service::{RpcError, Service, ServiceConfig};
use rcfd_assistant::{ApprovalMode, Toolbox};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Largest generator mesh (nx * ny) that is rebuilt automatically after an edit.
pub const AUTO_INIT_MAX_CELLS: u64 = 40_000;

/// Parsed `run.status`.
#[derive(Debug, Clone, PartialEq)]
pub struct RunStatus {
    /// `defined`, `ready`, `running`, `paused`, `finished`, `stopped` or `failed`.
    pub state: String,
    /// Iteration count.
    pub iteration: u64,
    /// Simulation time.
    pub time: f64,
    /// Momentum residual.
    pub res_u: Option<f64>,
    /// Pressure residual.
    pub res_p: Option<f64>,
    /// Continuity error.
    pub continuity: Option<f64>,
    /// Converged flag.
    pub converged: bool,
    /// Cell count.
    pub cells: u64,
}

impl Default for RunStatus {
    fn default() -> Self {
        RunStatus {
            state: "none".into(),
            iteration: 0,
            time: 0.0,
            res_u: None,
            res_p: None,
            continuity: None,
            converged: false,
            cells: 0,
        }
    }
}

impl RunStatus {
    /// Parse a `run.status` result.
    pub fn from_json(v: &Value) -> RunStatus {
        let s = &v["simulation"];
        RunStatus {
            state: v["state"].as_str().unwrap_or("none").to_string(),
            iteration: s["iteration"].as_u64().unwrap_or(0),
            time: s["time"].as_f64().unwrap_or(0.0),
            res_u: s["res_u"].as_f64(),
            res_p: s["res_p"].as_f64(),
            continuity: s["continuity"].as_f64(),
            converged: s["converged"].as_bool().unwrap_or(false),
            cells: s["cells"].as_u64().unwrap_or(0),
        }
    }

    /// True while the solver thread is working.
    pub fn is_running(&self) -> bool {
        self.state == "running"
    }
}

/// The open case and everything derived from it.
pub struct Controller {
    /// The solver service (shared with the assistant).
    pub svc: Arc<Service>,
    /// Assistant tools over the same service.
    pub toolbox: Arc<Toolbox>,
    /// Current session id.
    pub session: Option<String>,
    /// Current settings tree.
    pub settings: Value,
    /// Schema tree for the editor.
    pub schema: Node,
    /// File the case was opened from / saved to.
    pub file: Option<PathBuf>,
    /// True when the settings differ from the last open/save.
    pub dirty: bool,
    /// Per-setting error text (local or server).
    pub field_errors: BTreeMap<String, String>,
    /// Problems from the last open/validate.
    pub diagnostics: Vec<String>,
    /// Run status.
    pub status: RunStatus,
    /// Monitor histories.
    pub monitors: MonitorStore,
    /// Report values from the last poll.
    pub reports: Vec<Value>,
    /// Solver log tail.
    pub log: Vec<String>,
    /// Latest raster grid.
    pub grid: Option<GridData>,
    /// Cell-centre samples (for the cells view and the aspect ratio).
    pub slice: Option<SliceData>,
    /// Viewport settings.
    pub view: ViewState,
    /// Last notice or error for the status bar.
    pub message: Option<String>,
    /// Selected setting path (assistant context).
    pub selected: Option<String>,
    saved: Value,
    slice_for: Option<(String, String)>,
}

fn rpc_msg(e: &RpcError) -> String {
    match e.data.as_ref().and_then(|d| d["diagnostics"].as_array()) {
        Some(ds) if !ds.is_empty() => ds
            .iter()
            .map(|d| {
                let p = d["path"].as_str().unwrap_or_default();
                let m = d["message"].as_str().unwrap_or_default();
                if p.is_empty() {
                    m.to_string()
                } else {
                    format!("{p}: {m}")
                }
            })
            .collect::<Vec<_>>()
            .join("; "),
        _ => e.message.clone(),
    }
}

impl Controller {
    /// Controller with a fresh service. `root` confines the files the assistant's tools may touch.
    pub fn new(root: Option<PathBuf>, cases_dir: impl Into<PathBuf>) -> Controller {
        let svc = Service::new(ServiceConfig {
            root,
            ..Default::default()
        });
        let schema_doc = svc.call("settings.schema", &json!({})).unwrap_or(json!({}));
        let toolbox = Arc::new(Toolbox::new(svc.clone(), cases_dir, ApprovalMode::ApproveWrites));
        Controller {
            svc,
            toolbox,
            session: None,
            settings: Value::Null,
            schema: build_tree(&schema_doc["case"]),
            file: None,
            dirty: false,
            field_errors: BTreeMap::new(),
            diagnostics: vec![],
            status: RunStatus::default(),
            monitors: MonitorStore::default(),
            reports: vec![],
            log: vec![],
            grid: None,
            slice: None,
            view: ViewState::default(),
            message: None,
            selected: None,
            saved: Value::Null,
            slice_for: None,
        }
    }

    /// Case title for display.
    pub fn title(&self) -> String {
        self.settings["title"]
            .as_str()
            .filter(|t| !t.is_empty())
            .unwrap_or("untitled")
            .to_string()
    }

    fn call(&self, method: &str, extra: Value) -> Result<Value, RpcError> {
        let mut p = extra;
        if let Some(s) = &self.session {
            p["session"] = json!(s);
        }
        self.svc.call(method, &p)
    }

    fn reset_results(&mut self) {
        self.monitors.clear();
        self.reports.clear();
        self.log.clear();
        self.grid = None;
        self.slice = None;
        self.slice_for = None;
        self.status = RunStatus::default();
    }

    fn adopt(&mut self, id: String) {
        if let Some(old) = self.session.replace(id.clone()) {
            if old != id {
                let _ = self.svc.call("session.close", &json!({"session": old}));
            }
        }
        self.toolbox.set_session(Some(id));
        self.reset_results();
        self.field_errors.clear();
        self.refresh_settings();
        self.saved = self.settings.clone();
        self.dirty = false;
        self.auto_init();
        self.poll(true);
    }

    /// Create a case from the built-in lid-driven cavity template.
    pub fn new_from_template(&mut self) -> Result<(), String> {
        self.open_toml(rcfd_mcp::LID_DRIVEN_CAVITY, None)
    }

    /// Open case text (TOML) as a new session.
    pub fn open_toml(&mut self, text: &str, path: Option<PathBuf>) -> Result<(), String> {
        let r = self.svc.call("session.create", &json!({"toml": text}));
        self.finish_open(r, path)
    }

    /// Open a case from a settings JSON object.
    pub fn open_settings(&mut self, settings: &Value, path: Option<PathBuf>) -> Result<(), String> {
        let r = self.svc.call("session.create", &json!({"settings": settings}));
        self.finish_open(r, path)
    }

    fn finish_open(&mut self, r: Result<Value, RpcError>, path: Option<PathBuf>) -> Result<(), String> {
        match r {
            Ok(v) => {
                let id = v["session"].as_str().unwrap_or_default().to_string();
                self.diagnostics.clear();
                self.file = path;
                self.adopt(id);
                Ok(())
            }
            Err(e) => {
                let m = rpc_msg(&e);
                self.diagnostics = m.split("; ").map(String::from).collect();
                self.message = Some(format!("could not open case: {m}"));
                Err(m)
            }
        }
    }

    /// Open a `.toml` or `.json` case file chosen by the user.
    pub fn open_path(&mut self, path: &Path) -> Result<(), String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let is_json = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("json"));
        let r = if is_json {
            let v: Value = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
            self.svc.call("session.create", &json!({"settings": v}))
        } else {
            self.svc.call("session.create", &json!({"toml": text}))
        };
        self.finish_open(r, Some(path.to_path_buf()))
    }

    /// Save the case to `path` (`.json` writes JSON, anything else TOML).
    pub fn save_to(&mut self, path: &Path) -> Result<(), String> {
        self.refresh_settings();
        let case = Case::from_json(&self.settings)
            .map_err(|d| d.iter().map(|x| x.to_string()).collect::<Vec<_>>().join("; "))?;
        let is_json = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("json"));
        let text = if is_json { case.to_json_string() } else { case.to_toml() };
        std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))?;
        self.file = Some(path.to_path_buf());
        self.saved = self.settings.clone();
        self.dirty = false;
        self.message = Some(format!("saved {}", path.display()));
        Ok(())
    }

    /// Re-read the settings tree from the service.
    pub fn refresh_settings(&mut self) {
        if self.session.is_none() {
            return;
        }
        if let Ok(v) = self.call("settings.get", json!({})) {
            self.settings = v["value"].clone();
            self.dirty = self.settings != self.saved;
        }
    }

    /// Edit a setting from user text. Local parse/range errors and server errors are
    /// recorded in `field_errors[path]`; returns true when the edit was applied.
    pub fn edit_text(&mut self, path: &str, text: &str) -> bool {
        let Some(node) = self.schema.find(path) else {
            self.field_errors.insert(path.into(), "unknown setting".into());
            return false;
        };
        match parse_input(&node.kind, text) {
            Ok(v) => self.set_value(path, v),
            Err(m) => {
                self.field_errors.insert(path.into(), m);
                false
            }
        }
    }

    /// Set a setting to a JSON value.
    pub fn set_value(&mut self, path: &str, value: Value) -> bool {
        if self.session.is_none() {
            self.field_errors.insert(path.into(), "no case is open".into());
            return false;
        }
        match self.call("settings.set", json!({"path": path, "value": value})) {
            Ok(_) => {
                self.field_errors.remove(path);
                self.after_change();
                true
            }
            Err(e) => {
                self.field_errors.insert(path.into(), rpc_msg(&e));
                false
            }
        }
    }

    /// Add a named entry (a patch's boundary conditions, a report) with its default `seed_field`.
    pub fn add_named(&mut self, collection: &str, name: &str) -> bool {
        let name = name.trim();
        if name.is_empty() || name.contains('.') || name.contains(char::is_whitespace) {
            self.field_errors.insert(
                collection.into(),
                "names must be non-empty and contain no dots or spaces".into(),
            );
            return false;
        }
        let Some(item) = self.schema.find(collection).and_then(|c| c.children.first()) else {
            return false;
        };
        let Some(first) = item.children.first() else {
            return false;
        };
        let path = format!("{collection}.{name}.{}", first.name);
        self.set_value(&path, first.default.clone())
    }

    /// Remove a named entry.
    pub fn remove_named(&mut self, path: &str) -> bool {
        match self.call("settings.remove", json!({"path": path})) {
            Ok(_) => {
                self.after_change();
                true
            }
            Err(e) => {
                self.message = Some(rpc_msg(&e));
                false
            }
        }
    }

    fn after_change(&mut self) {
        self.refresh_settings();
        // The service dropped the built simulation; results are stale.
        self.monitors.clear();
        self.reports.clear();
        self.grid = None;
        self.slice = None;
        self.slice_for = None;
        self.auto_init();
        self.poll(true);
    }

    /// Build the simulation when the case is small enough, so the viewport shows the initial state.
    pub fn auto_init(&mut self) {
        if self.session.is_none() || self.status.is_running() {
            return;
        }
        let cells =
            self.settings["mesh"]["nx"].as_u64().unwrap_or(0) * self.settings["mesh"]["ny"].as_u64().unwrap_or(0);
        if self.settings["mesh"]["source"] == "generator" && cells > AUTO_INIT_MAX_CELLS {
            return;
        }
        let _ = self.call("run.initialize", json!({}));
    }

    /// Validate with the service and store the diagnostics.
    pub fn validate(&mut self) -> bool {
        match self.call("settings.validate", json!({})) {
            Ok(v) => {
                self.diagnostics = v["diagnostics"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|d| {
                        format!(
                            "{}: {}",
                            d["path"].as_str().unwrap_or_default(),
                            d["message"].as_str().unwrap_or_default()
                        )
                    })
                    .collect();
                v["ok"] == true
            }
            Err(e) => {
                self.diagnostics = vec![rpc_msg(&e)];
                false
            }
        }
    }

    // ---- run control (all non-blocking) ----

    fn run_cmd(&mut self, method: &str, extra: Value) -> bool {
        match self.call(method, extra) {
            Ok(_) => {
                self.poll(false);
                true
            }
            Err(e) => {
                self.message = Some(rpc_msg(&e));
                false
            }
        }
    }

    /// Start (or restart) the run; returns immediately.
    pub fn run_start(&mut self) -> bool {
        // A fresh run starts from the initial state: forget old histories.
        self.monitors.clear();
        self.reports.clear();
        self.run_cmd("run.start", json!({}))
    }

    /// Pause a running solve.
    pub fn run_pause(&mut self) -> bool {
        self.run_cmd("run.pause", json!({}))
    }

    /// Resume a paused solve.
    pub fn run_resume(&mut self) -> bool {
        self.run_cmd("run.resume", json!({}))
    }

    /// Stop the solve.
    pub fn run_stop(&mut self) -> bool {
        self.run_cmd("run.stop", json!({}))
    }

    /// Advance `n` iterations synchronously (small `n` only; used by the Step button).
    pub fn run_step(&mut self, n: u64) -> bool {
        self.run_cmd("run.step", json!({"count": n}))
    }

    /// Follow the assistant: if its tools switched to another session, adopt it, and
    /// refresh settings after any assistant tool call. Returns true when something changed.
    pub fn sync_with_assistant(&mut self) -> bool {
        let theirs = self.toolbox.session();
        if theirs.is_some() && theirs != self.session {
            let id = theirs.unwrap_or_default();
            // `adopt` closes our old session; the assistant's session stays open.
            self.adopt(id);
            return true;
        }
        let before = self.settings.clone();
        self.refresh_settings();
        if self.settings != before {
            self.after_change();
            return true;
        }
        false
    }

    /// Poll the service: status, new monitor samples, log and the field. Cheap; call a few times a second.
    pub fn poll(&mut self, force_field: bool) {
        let Some(sid) = self.session.clone() else { return };
        let since = self.monitors.since();
        let want_field = force_field || self.status.is_running();
        let aspect = self.slice.as_ref().map_or(1.0, SliceData::aspect);
        let mut p = json!({"session": sid, "log_lines": 200});
        if let Some(s) = since {
            p["since"] = json!(s);
        }
        if want_field && self.view.mode == crate::viewport::ViewMode::Raster {
            p["field"] = json!(self.view.field);
            p["cols"] = json!(self.view.cols);
            p["rows"] = json!(self.view.rows_for(aspect));
        }
        let Ok(snap) = self.svc.call("panel.snapshot", &p) else {
            return;
        };
        let status = RunStatus::from_json(&snap["status"]);
        // A restart resets the iteration counter below what we have: refetch from scratch.
        if status.iteration < self.monitors.since().unwrap_or(0.0) as u64 && !snap["monitors"].is_null() {
            self.monitors.clear();
            if let Ok(full) = self.svc.call("monitors.get", &json!({"session": sid})) {
                self.monitors.merge(&full);
            }
        } else if !snap["monitors"].is_null() {
            self.monitors.merge(&snap["monitors"]);
        }
        self.status = status;
        if let Some(r) = snap["reports"].as_array() {
            self.reports = r.clone();
        }
        if let Some(l) = snap["log"].as_array() {
            self.log = l.iter().filter_map(|x| x.as_str().map(String::from)).collect();
        }
        if let Some(g) = GridData::from_json(&snap["grid"]) {
            self.grid = Some(g);
        }
        self.ensure_slice(&sid);
    }

    /// Fetch cell-centre samples when the case or field changed (needed for aspect and the cells view).
    fn ensure_slice(&mut self, sid: &str) {
        if self.status.state == "none" || self.status.state == "defined" {
            return;
        }
        let key = (sid.to_string(), self.view.field.clone());
        let need = self.slice_for.as_ref() != Some(&key)
            || (self.view.mode == crate::viewport::ViewMode::Cells && self.status.is_running());
        if !need {
            return;
        }
        if let Ok(v) = self.svc.call(
            "field.slice",
            &json!({"session": sid, "name": self.view.field, "max_points": 6000}),
        ) {
            if let Some(s) = SliceData::from_json(&v) {
                self.slice = Some(s);
                self.slice_for = Some(key);
            }
        }
    }

    /// Change the displayed field and refresh.
    pub fn set_view_field(&mut self, name: &str) {
        self.view.field = name.to_string();
        self.slice_for = None;
        self.poll(true);
    }

    /// Wait until the solver is no longer running (tests and batch use only; never from the UI thread).
    pub fn wait_idle(&mut self, timeout_ms: u64) {
        let _ = self.call("run.wait", json!({"timeout_ms": timeout_ms}));
        self.poll(true);
    }
}
