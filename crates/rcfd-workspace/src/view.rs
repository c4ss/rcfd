//! Claude's canvas (F-976, ADR 0805): the view specification `rcfd-view` version 1, its validation with located errors
//! (D1), the commands `show_view` and `read_view` (D2), and the registry of the open Studios of a project (D3).
//!
//! A specification names what to show (`case`, `run`, `field`, `mesh`, `plot`, `monitor`, `table`), of which case or
//! run, and the layout. `show_view` checks it against the project, counts the project's live canvases, and writes it to
//! `.rcfd/views/current.json` with one journal line `view_shown`; every open Studio reads that line from the change
//! feed (ADR 0188) and the view with `read_view`. Nothing is written for a refused specification or when no Studio is
//! open.
//!
//! F-979 (ADR 0807 D1, D2) adds three forms, by addition: a `mesh` panel with `case` (the picture of the case's mesh), the
//! kind `setting` (`case`, `setting`: one setting's value, which `read_view` gives as `settings`), and a `monitor` panel
//! with `case` (the monitor of the newest run of the case, which the Studio follows as the runner writes it).
//!
//! F-977 (ADR 0817 D1 to D3) adds, by addition, the view of a run: `{version: 1, run}` with no `panels`, which
//! `show_view` shows as the layout of the category of the run's case ([`flow_category`], [`category_of_text`],
//! [`Workspace::layout_of_run`]), recording the choice as `chosen` in the shown file.

use crate::{err, journal, project_error, RpcError, Workspace, BAD_STATE, INTERNAL, VALIDATION};
use rcfd_api::case::Case;
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};

/// The version of the specification this engine reads.
pub const VERSION: u64 = 1;
/// The kinds of panel, in the order of the acceptance of F-976.
pub const KINDS: [&str; 7] = ["case", "run", "field", "mesh", "plot", "monitor", "table"];
/// Most panels of one view.
pub const MAX_PANELS: usize = 8;
/// Most columns of the layout.
pub const MAX_COLUMNS: u64 = 4;
/// Most runs of a table.
pub const MAX_TABLE_RUNS: usize = 10;
/// Longest title, in characters.
pub const MAX_TITLE: usize = 200;
/// Most errors listed in a refusal (`errors_total` counts all).
pub const MAX_ERRORS: usize = 20;
/// The shown view, project-relative.
pub const CURRENT: &str = ".rcfd/views/current.json";
/// The journal kind of a shown view.
pub const VIEW_SHOWN: &str = "view_shown";
/// Pages of `list_meshes` a mesh reference is looked for in (100 entries each).
const MESH_PAGES: usize = 20;
/// The executable whose processes are canvases (`rcfd-mcp --canvas`).
pub const CANVAS_EXE: &str = "rcfd-mcp";
/// The kinds of panel F-979 adds after the seven of F-976 (ADR 0807 D1).
pub const KINDS_F979: [&str; 1] = ["setting"];
/// Longest setting path of a `setting` panel, in characters.
pub const MAX_SETTING: usize = 200;

/// One located error of a specification: the dotted path of the value and the reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewError {
    /// `version`, `layout.columns`, `panels[2].kind`, ...
    pub key: String,
    /// What is wrong, in a sentence.
    pub reason: String,
}

fn e(errors: &mut Vec<ViewError>, key: impl Into<String>, reason: impl Into<String>) {
    errors.push(ViewError {
        key: key.into(),
        reason: reason.into(),
    });
}

/// Keys of `o` that are not in `allowed`, each an error at `<at>.<key>`.
fn extra_keys(o: &Map<String, Value>, allowed: &[&str], at: &str, errors: &mut Vec<ViewError>) {
    for k in o.keys().filter(|k| !allowed.contains(&k.as_str())) {
        let key = if at.is_empty() { k.clone() } else { format!("{at}.{k}") };
        e(
            errors,
            key,
            format!(
                "is not a key of the view specification here (allowed: {})",
                allowed.join(", ")
            ),
        );
    }
}

fn title(o: &Map<String, Value>, at: &str, errors: &mut Vec<ViewError>) {
    let key = if at.is_empty() {
        "title".to_string()
    } else {
        format!("{at}.title")
    };
    match o.get("title") {
        None => {}
        Some(Value::String(t)) => {
            let n = t.chars().count();
            if n == 0 || n > MAX_TITLE || t.chars().any(char::is_control) {
                e(
                    errors,
                    key,
                    format!("a title is 1 to {MAX_TITLE} characters with no control character"),
                );
            }
        }
        Some(_) => e(errors, key, "a title is a string"),
    }
}

impl Workspace {
    /// The located errors of view specification `v` against this project (empty: valid), in the order of the keys.
    pub fn view_errors(&self, v: &Value) -> Vec<ViewError> {
        let mut errors = Vec::new();
        let Some(o) = v.as_object() else {
            e(&mut errors, "", "a view specification is a JSON object");
            return errors;
        };
        // ADR 0817 D3: the key `run` is checked below; every other key as F-976 checks it, with F-976's reason
        let mut top = o.clone();
        top.remove("run");
        extra_keys(&top, &["version", "title", "layout", "panels"], "", &mut errors);
        match o.get("version") {
            None => e(&mut errors, "version", "is required (1)"),
            Some(x) if x.as_u64() == Some(VERSION) => {}
            Some(x) => e(
                &mut errors,
                "version",
                format!("{x} is not a version this engine reads (1)"),
            ),
        }
        title(o, "", &mut errors);
        match o.get("layout") {
            None => {}
            Some(Value::Object(l)) => {
                extra_keys(l, &["columns"], "layout", &mut errors);
                match l.get("columns") {
                    None => {}
                    Some(c) if c.as_u64().is_some_and(|c| (1..=MAX_COLUMNS).contains(&c)) => {}
                    Some(c) => e(
                        &mut errors,
                        "layout.columns",
                        format!("{c} is not an integer 1 to {MAX_COLUMNS}"),
                    ),
                }
            }
            Some(_) => e(&mut errors, "layout", "a layout is an object {columns}"),
        }
        match (o.get("panels"), o.get("run")) {
            // ADR 0817 D3: a view of a run takes the layout of its case's category
            (Some(_), Some(_)) => e(
                &mut errors,
                "panels",
                "a view with run takes the layout of its case's category: give run or panels, not both",
            ),
            (None, Some(r)) => {
                self.run_ref(Some(r), "run", &mut errors);
            }
            _ => self.panels_errors(o, &mut errors),
        }
        errors
    }

    fn panels_errors(&self, o: &Map<String, Value>, errors: &mut Vec<ViewError>) {
        match o.get("panels") {
            None => e(errors, "panels", "is required: an array of 1 to 8 panels"),
            Some(Value::Array(p)) if p.is_empty() || p.len() > MAX_PANELS => e(
                errors,
                "panels",
                format!("holds {} panels; a view has 1 to {MAX_PANELS}", p.len()),
            ),
            Some(Value::Array(p)) => {
                for (i, panel) in p.iter().enumerate() {
                    self.panel_errors(panel, &format!("panels[{i}]"), errors);
                }
            }
            Some(_) => e(errors, "panels", "is an array of panels"),
        }
    }

    fn panel_errors(&self, panel: &Value, at: &str, errors: &mut Vec<ViewError>) {
        let Some(o) = panel.as_object() else {
            e(errors, at, "a panel is a JSON object with a kind");
            return;
        };
        let all = || format!("{}, {}", KINDS.join(", "), KINDS_F979.join(", "));
        let kind = match o.get("kind").and_then(Value::as_str) {
            Some(k) if KINDS.contains(&k) || KINDS_F979.contains(&k) => k,
            Some(k) => {
                e(
                    errors,
                    format!("{at}.kind"),
                    format!("'{k}' is not a kind (one of {})", all()),
                );
                return;
            }
            None => {
                e(errors, format!("{at}.kind"), format!("is required, one of {}", all()));
                return;
            }
        };
        let keys: &[&str] = match kind {
            "case" => &["kind", "title", "case"],
            // ADR 0807 D1: a mesh panel names a mesh of meshes/ or a case whose mesh is drawn
            "mesh" => &["kind", "title", "mesh", "case"],
            "run" | "plot" => &["kind", "title", "run"],
            "field" => &["kind", "title", "run", "field", "snapshot"],
            // ADR 0807 D1: a monitor panel names a run, or a case whose newest run it follows
            "monitor" => &["kind", "title", "run", "monitor", "case"],
            "setting" => &["kind", "title", "case", "setting"],
            _ => &["kind", "title", "runs"],
        };
        extra_keys(o, keys, at, errors);
        title(o, at, errors);
        match kind {
            "case" => self.case_ref(o.get("case"), &format!("{at}.case"), errors),
            "mesh" if o.contains_key("case") => {
                if o.contains_key("mesh") {
                    e(
                        errors,
                        format!("{at}.case"),
                        "a mesh panel names its mesh or its case, not both",
                    );
                } else {
                    self.case_read(o.get("case"), &format!("{at}.case"), errors);
                }
            }
            "mesh" => self.mesh_ref(o.get("mesh"), &format!("{at}.mesh"), errors),
            "setting" => {
                if let Some(case) = self.case_read(o.get("case"), &format!("{at}.case"), errors) {
                    setting_ref(&case, o.get("setting"), &format!("{at}.setting"), errors);
                }
            }
            "monitor" if o.contains_key("case") => {
                if o.contains_key("run") {
                    e(
                        errors,
                        format!("{at}.case"),
                        "a monitor panel names its run or its case, not both",
                    );
                } else if let Some(case) = self.case_read(o.get("case"), &format!("{at}.case"), errors) {
                    report_ref(&case, o.get("monitor"), &format!("{at}.monitor"), errors);
                }
            }
            "table" => {
                let key = format!("{at}.runs");
                match o.get("runs").and_then(Value::as_array) {
                    None => e(
                        errors,
                        key,
                        format!("is required: an array of 1 to {MAX_TABLE_RUNS} run ids"),
                    ),
                    Some(r) if r.is_empty() || r.len() > MAX_TABLE_RUNS => e(
                        errors,
                        key,
                        format!("holds {} runs; a table has 1 to {MAX_TABLE_RUNS}", r.len()),
                    ),
                    Some(r) => {
                        for (j, id) in r.iter().enumerate() {
                            let k = format!("{key}[{j}]");
                            if r[..j].contains(id) {
                                e(errors, k, format!("{id} is named twice"));
                            } else {
                                self.run_ref(Some(id), &k, errors);
                            }
                        }
                    }
                }
            }
            _ => {
                let key = format!("{at}.run");
                let Some(dir) = self.run_ref(o.get("run"), &key, errors) else {
                    return;
                };
                if kind == "field" {
                    self.field_ref(o, &dir, at, errors);
                } else if kind == "monitor" {
                    self.monitor_ref(o.get("monitor"), &dir, &format!("{at}.monitor"), errors);
                }
            }
        }
    }

    /// A run id whose `runs/<id>/run.json` exists: its folder, or an error at `key`.
    fn run_ref(&self, v: Option<&Value>, key: &str, errors: &mut Vec<ViewError>) -> Option<PathBuf> {
        let Some(id) = v.and_then(Value::as_str) else {
            e(errors, key, "is required: a run id (see list_runs)");
            return None;
        };
        match self.project.run_dir(id) {
            Ok(d) if d.join("run.json").is_file() => Some(d),
            Ok(_) => {
                e(
                    errors,
                    key,
                    format!("{id}: no such run (runs/{id}/run.json is missing)"),
                );
                None
            }
            Err(x) => {
                e(errors, key, format!("{id}: not a run id ({x})"));
                None
            }
        }
    }

    fn case_ref(&self, v: Option<&Value>, key: &str, errors: &mut Vec<ViewError>) {
        let Some(p) = v.and_then(Value::as_str) else {
            e(errors, key, "is required: a project-relative case file under cases/");
            return;
        };
        let norm = p.replace('\\', "/");
        let ext = Path::new(&norm).extension().and_then(|x| x.to_str()).unwrap_or("");
        if !norm.starts_with("cases/") || !(ext == "toml" || ext == "json") {
            e(
                errors,
                key,
                format!("{p}: a case is a .toml or .json file under cases/"),
            );
            return;
        }
        match rcfd_project::paths::resolve_in(self.project.root(), &norm) {
            Ok(f) if f.is_file() => {}
            Ok(_) => e(errors, key, format!("{p}: no such case file")),
            Err(x) => e(errors, key, format!("{p}: {x}")),
        }
    }

    /// ADR 0807 D1: a case as `case_ref` checks it that also reads as a case (`read_case_text`): the case, or errors at `key`.
    fn case_read(&self, v: Option<&Value>, key: &str, errors: &mut Vec<ViewError>) -> Option<Case> {
        let before = errors.len();
        self.case_ref(v, key, errors);
        if errors.len() > before {
            return None;
        }
        let p = v.and_then(Value::as_str).unwrap_or_default().replace('\\', "/");
        let path = rcfd_project::paths::resolve_in(self.project.root(), &p).ok()?;
        match self.read_case_text(&path) {
            Ok((case, _)) => Some(case),
            Err(x) => {
                e(errors, key, format!("{p}: does not read as a case ({})", x.message));
                None
            }
        }
    }

    /// ADR 0807 D2: the value of each `setting` panel of view `v`, read from its case file now.
    fn setting_values(&self, v: &Value) -> Vec<Value> {
        let mut out = Vec::new();
        for (i, p) in v["panels"].as_array().into_iter().flatten().enumerate() {
            if p["kind"] != "setting" {
                continue;
            }
            let case = p["case"].as_str().unwrap_or_default().replace('\\', "/");
            let setting = p["setting"].as_str().unwrap_or_default();
            let read = rcfd_project::paths::resolve_in(self.project.root(), &case)
                .map_err(|x| x.to_string())
                .and_then(|f| self.read_case_text(&f).map_err(|x| x.message))
                .and_then(|(c, _)| c.get(setting).map_err(|x| x.to_string()));
            let mut one = json!({"panel": i, "case": case, "setting": setting});
            match read {
                Ok(value) => one["value"] = value,
                Err(x) => one["error"] = json!(x),
            }
            out.push(one);
        }
        out
    }

    fn mesh_ref(&self, v: Option<&Value>, key: &str, errors: &mut Vec<ViewError>) {
        let Some(p) = v.and_then(Value::as_str) else {
            e(errors, key, "is required: a mesh of meshes/ (see list_meshes)");
            return;
        };
        let want = p.replace('\\', "/");
        let mut offset = 0u64;
        for _ in 0..MESH_PAGES {
            let page = match self.list_meshes(&json!({"offset": offset, "limit": 100})) {
                Ok(r) => r,
                Err(x) => {
                    e(
                        errors,
                        key,
                        format!("{p}: the meshes could not be listed ({})", x.message),
                    );
                    return;
                }
            };
            let listed = page["meshes"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|m| m["path"].as_str() == Some(want.as_str()));
            if listed {
                return;
            }
            match page["next_offset"].as_u64() {
                Some(n) => offset = n,
                None => break,
            }
        }
        e(
            errors,
            key,
            format!("{p}: not a mesh of meshes/ (list_meshes does not list it)"),
        );
    }

    fn field_ref(&self, o: &Map<String, Value>, dir: &Path, at: &str, errors: &mut Vec<ViewError>) {
        use rcfd_tsstore::{Reader, STORE_FILE};
        let key = format!("{at}.field");
        let Some(field) = o.get("field").and_then(Value::as_str) else {
            e(errors, key, "is required: a field stored in the run's field store");
            return;
        };
        let store = dir.join("fields");
        if !store.join(STORE_FILE).is_file() {
            e(
                errors,
                format!("{at}.run"),
                "the run has no stored fields (runs/<id>/fields)",
            );
            return;
        }
        let mut r = match Reader::open(&store) {
            Ok(r) => r,
            Err(x) => {
                e(
                    errors,
                    format!("{at}.run"),
                    format!("the run's field store does not read: {x}"),
                );
                return;
            }
        };
        let n = r.snapshots().len();
        if n == 0 {
            e(
                errors,
                format!("{at}.run"),
                "the run's field store holds no snapshot yet",
            );
            return;
        }
        let i = match o.get("snapshot") {
            None => n - 1,
            Some(s) => match s.as_u64() {
                Some(s) if (s as usize) < n => s as usize,
                _ => {
                    e(
                        errors,
                        format!("{at}.snapshot"),
                        format!("{s} is not a snapshot index (0 to {})", n - 1),
                    );
                    return;
                }
            },
        };
        match r.fields(i) {
            Ok(f) if f.iter().any(|f| f.name == field) => {}
            Ok(f) => {
                let names: Vec<String> = f.iter().map(|f| f.name.clone()).collect();
                e(
                    errors,
                    key,
                    format!("'{field}' is not stored at snapshot {i} (stored: {})", names.join(", ")),
                );
            }
            Err(x) => e(errors, key, format!("snapshot {i} does not read: {x}")),
        }
    }

    fn monitor_ref(&self, v: Option<&Value>, dir: &Path, key: &str, errors: &mut Vec<ViewError>) {
        let Some(name) = v.and_then(Value::as_str) else {
            e(
                errors,
                key,
                "is required: a monitor of the run (monitors.<name> of read_residuals)",
            );
            return;
        };
        let bytes = std::fs::read(dir.join("residuals.jsonl")).unwrap_or_default();
        let (series, _, _) = crate::residual_series(&bytes, 0, Some(2));
        let full = format!("monitors.{name}");
        if !series.contains_key(&full) {
            let have: Vec<&str> = series.keys().filter_map(|k| k.strip_prefix("monitors.")).collect();
            e(
                errors,
                key,
                format!("'{name}' is not a monitor of the run (monitors: {})", have.join(", ")),
            );
        }
    }

    /// `show_view {view, reason?}` (ADR 0805 D2): validate, count the open Studios, write the view and its journal line.
    pub fn show_view(&self, args: &Value) -> Result<Value, RpcError> {
        let reason = journal::reason_arg(args)?;
        let view = args.get("view").cloned().unwrap_or(Value::Null);
        let errors = self.view_errors(&view);
        if let Some(first) = errors.first() {
            let list: Vec<Value> = errors
                .iter()
                .take(MAX_ERRORS)
                .map(|x| json!({"key": x.key, "reason": x.reason}))
                .collect();
            return Err(RpcError {
                code: VALIDATION,
                message: format!("view.{}: {}", first.key, first.reason).replace("view.: ", "view: "),
                data: Some(json!({"errors": list, "errors_total": errors.len()})),
            });
        }
        let studios = live_canvases(self.project.root());
        if studios == 0 {
            return Err(err(
                BAD_STATE,
                "no Studio has this project open, so there is nowhere to show the view: open the project folder in the Studio, then call show_view again (nothing was written)",
            ));
        }
        // ADR 0817 D3: a view of a run is shown as the layout of its case's category
        let (view, chosen) = match view.get("run").and_then(Value::as_str) {
            Some(run) => {
                let (v, c) = self.layout_of_run(run, view.get("title"), view.get("layout"))?;
                let errors = self.view_errors(&v);
                if let Some(first) = errors.first() {
                    return Err(err(
                        INTERNAL,
                        format!(
                            "the layout of {run} is not a valid view: {}: {}",
                            first.key, first.reason
                        ),
                    ));
                }
                (v, Some(c))
            }
            None => (view, None),
        };
        let mut file = Map::new();
        file.insert("format".into(), json!("rcfd-view-shown"));
        file.insert("version".into(), json!(VERSION));
        file.insert("shown".into(), json!(rcfd_project::time::now()));
        if let Some(a) = &self.actor {
            file.insert("actor".into(), a.to_json());
        }
        if let Some(r) = &reason {
            file.insert("reason".into(), json!(r));
        }
        if let Some(c) = &chosen {
            file.insert("chosen".into(), c.clone());
        }
        file.insert("view".into(), view.clone());
        let bytes = serde_json::to_vec_pretty(&Value::Object(file)).map_err(|x| err(INTERNAL, x.to_string()))?;
        let path = self.project.root().join(CURRENT);
        std::fs::create_dir_all(path.parent().expect("has a parent"))
            .map_err(|x| err(INTERNAL, format!("{CURRENT}: {x}")))?;
        rcfd_project::atomic::atomic_write(&path, &bytes).map_err(project_error)?;
        let hash = rcfd_project::records::hash_bytes(&bytes);
        let kinds: Vec<Value> = view["panels"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|p| p["kind"].clone())
            .collect();
        let mut line = Map::new();
        line.insert("path".into(), json!(CURRENT));
        line.insert("hash".into(), json!(hash));
        line.insert("panels".into(), json!(kinds));
        line.insert("studios".into(), json!(studios));
        journal::append(self.project.root(), VIEW_SHOWN, &self.origin("show_view", reason), line);
        let mut out = json!({"shown": true, "id": hash, "path": CURRENT, "hash": hash, "studios": studios, "panels": kinds.len()});
        if let Some(c) = &chosen {
            out["category"] = c["category"].clone();
            out["rule"] = c["rule"].clone();
        }
        Ok(out)
    }

    /// `read_view {}` (ADR 0805 D2): the view shown last, or `exists: false`.
    pub fn read_view(&self) -> Result<Value, RpcError> {
        let path = self.project.root().join(CURRENT);
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(x) if x.kind() == std::io::ErrorKind::NotFound => {
                return Ok(json!({"exists": false, "path": CURRENT, "view": null}))
            }
            Err(x) => return Err(err(INTERNAL, format!("{CURRENT}: {x}"))),
        };
        let v: Value = serde_json::from_slice(&bytes).map_err(|x| err(INTERNAL, format!("{CURRENT}: {x}")))?;
        let hash = rcfd_project::records::hash_bytes(&bytes);
        let mut out = json!({"exists": true, "path": CURRENT, "id": hash, "hash": hash, "shown": v["shown"],
            "actor": v.get("actor").cloned().unwrap_or(Value::Null), "reason": v.get("reason").cloned().unwrap_or(Value::Null),
            "view": v["view"]});
        // ADR 0807 D2: each shown setting's value, only for a view that has a setting panel
        let settings = self.setting_values(&v["view"]);
        if !settings.is_empty() {
            out["settings"] = Value::Array(settings);
        }
        // ADR 0817 D3: how a view of a run was chosen, only for such a view
        if let Some(c) = v.get("chosen") {
            out["chosen"] = c.clone();
        }
        Ok(out)
    }
}

/// ADR 0807 D1: `setting` of a `setting` panel, a path that `Case::get` reads in `case`.
fn setting_ref(case: &Case, v: Option<&Value>, key: &str, errors: &mut Vec<ViewError>) {
    let Some(s) = v.and_then(Value::as_str) else {
        e(
            errors,
            key,
            "is required: a dotted setting path of the case (as edit_case takes it)",
        );
        return;
    };
    if s.is_empty() || s.chars().count() > MAX_SETTING || s.chars().any(char::is_control) {
        e(
            errors,
            key,
            format!("a setting path is 1 to {MAX_SETTING} characters with no control character"),
        );
        return;
    }
    if let Err(x) = case.get(s) {
        e(errors, key, format!("'{s}' is not a setting of the case ({x})"));
    }
}

/// ADR 0807 D1: `monitor` of a `monitor` panel with `case`, a report of the case (`reports.<monitor>`).
fn report_ref(case: &Case, v: Option<&Value>, key: &str, errors: &mut Vec<ViewError>) {
    let Some(name) = v.and_then(Value::as_str) else {
        e(errors, key, "is required: a report of the case (reports.<name>)");
        return;
    };
    if name.is_empty() || name.contains('.') || case.get(&format!("reports.{name}")).is_err() {
        let have: Vec<String> = case
            .get("reports")
            .ok()
            .and_then(|r| r.as_object().map(|m| m.keys().cloned().collect()))
            .unwrap_or_default();
        e(
            errors,
            key,
            format!("'{name}' is not a report of the case (reports: {})", have.join(", ")),
        );
    }
}

/// The categories of a case (ADR 0817 D1): id and label, in the order of the rule.
pub const CATEGORIES: [(&str, &str); 6] = [
    ("multiphase", "Multiphase"),
    ("heat_transfer", "Heat transfer"),
    ("internal_flow_and_turbomachinery", "Internal flow and turbomachinery"),
    ("external_aerodynamics", "External aerodynamics"),
    ("structural", "Structural"),
    ("general", "General"),
];

/// The category of a case and the test of the rule that gave it (ADR 0817 D1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Category {
    /// One of the ids of [`CATEGORIES`].
    pub id: &'static str,
    /// The rule text: the setting that decided, as the reply of `show_view` names it.
    pub rule: String,
}

fn category(id: &'static str, rule: impl Into<String>) -> Category {
    Category { id, rule: rule.into() }
}

/// The label of category `id` (`General` for an id that is not one).
pub fn category_label(id: &str) -> &'static str {
    CATEGORIES.iter().find(|(i, _)| *i == id).map_or("General", |(_, l)| l)
}

fn is_true(case: &Case, path: &str) -> bool {
    case.get(path).ok().and_then(|v| v.as_bool()) == Some(true)
}

fn text_of(case: &Case, path: &str) -> String {
    case.get(path)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

fn names(case: &Case, collection: &str) -> Vec<String> {
    let mut n = case.tree.names(collection).unwrap_or_default();
    n.sort();
    n
}

/// ADR 0817 D1, tests 3 to 12: the category of a flow case.
pub fn flow_category(case: &Case) -> Category {
    for path in ["multiphase.enabled", "particles.enabled"] {
        if is_true(case, path) {
            return category("multiphase", format!("{path} = true"));
        }
    }
    for path in ["energy.enabled", "phase_change.enabled"] {
        if is_true(case, path) {
            return category("heat_transfer", format!("{path} = true"));
        }
    }
    for s in names(case, "sources") {
        let t = text_of(case, &format!("sources.{s}.type"));
        if t == "rotating_frame" || t == "mrf" {
            return category("internal_flow_and_turbomachinery", format!("sources.{s}.type = {t}"));
        }
    }
    if is_true(case, "mixing_plane.enabled") {
        return category("internal_flow_and_turbomachinery", "mixing_plane.enabled = true");
    }
    if text_of(case, "mesh.source") == "generator" && text_of(case, "mesh.generator") == "axial_fan_sector" {
        return category("internal_flow_and_turbomachinery", "mesh.generator = axial_fan_sector");
    }
    let patches = names(case, "boundaries");
    for p in &patches {
        for key in ["velocity", "pressure"] {
            if text_of(case, &format!("boundaries.{p}.{key}")) == "freestream" {
                return category("external_aerodynamics", format!("boundaries.{p}.{key} = freestream"));
            }
        }
    }
    if !patches.is_empty() {
        return category(
            "internal_flow_and_turbomachinery",
            format!("boundaries: {} named, none freestream", patches.len()),
        );
    }
    category("general", "no boundaries are named")
}

/// ADR 0817 D1: the category of case text `text` (relative paths against `dir`, data files inside `sandbox`): read as a
/// flow case, or else as a solid case file. The flow case is returned with it when the text reads as one.
pub fn category_of_text(text: &str, dir: &Path, sandbox: Option<&Path>) -> (Option<Case>, Category) {
    match crate::plan::parse_case(text, dir, sandbox) {
        Ok(case) => {
            let c = flow_category(&case);
            (Some(case), c)
        }
        Err(d) => {
            if rcfd_api::solid_case::SolidCase::parse_toml_in(text, dir).is_ok() {
                return (
                    None,
                    category("structural", "a solid case file (docs/solid-case-file.md)"),
                );
            }
            let first = d
                .first()
                .map_or_else(|| "no diagnostic".to_string(), |x| x.message.clone());
            (
                None,
                category("general", format!("the case of the run does not read ({first})")),
            )
        }
    }
}

/// ADR 0817 D2: the report types of a category's monitors, in their order, and how many.
fn monitor_types(id: &str) -> (&'static [&'static str], usize) {
    match id {
        "external_aerodynamics" => (&["force_coefficient", "force", "moment"], 2),
        "internal_flow_and_turbomachinery" => (&["mass_flow", "area_average", "moment", "force"], 3),
        "heat_transfer" => (&["heat_flow", "expression"], 2),
        "multiphase" => (&["volume_integral", "volume_average"], 2),
        _ => (&[], 0),
    }
}

/// One template of a layout (ADR 0817 D2).
#[derive(Clone, Copy)]
enum Template {
    Case,
    Mesh,
    Field(&'static str),
    Monitors,
    Plot,
    Table,
}

fn templates(id: &str) -> &'static [Template] {
    use Template::*;
    match id {
        "external_aerodynamics" => &[Field("ux"), Field("p"), Monitors, Plot, Table],
        "internal_flow_and_turbomachinery" => &[Mesh, Field("ux"), Field("p"), Monitors, Plot],
        "heat_transfer" => &[Field("T"), Field("ux"), Monitors, Plot, Table],
        "multiphase" => &[Field("alpha"), Field("ux"), Field("p"), Monitors, Plot, Table],
        "structural" => &[Case, Table],
        _ => &[Case, Plot, Table],
    }
}

/// The names of the fields of the last snapshot of the store at `dir/fields` (empty when there is none).
fn last_fields(dir: &Path) -> Vec<String> {
    use rcfd_tsstore::{Reader, STORE_FILE};
    let store = dir.join("fields");
    if !store.join(STORE_FILE).is_file() {
        return Vec::new();
    }
    let Ok(mut r) = Reader::open(&store) else {
        return Vec::new();
    };
    let n = r.snapshots().len();
    if n == 0 {
        return Vec::new();
    }
    r.fields(n - 1)
        .map(|f| f.into_iter().map(|f| f.name).collect())
        .unwrap_or_default()
}

impl Workspace {
    /// ADR 0817 D1: the category of run `run` (its folder `dir`, its `run.json` case `kase`), read from the run's
    /// `case.snapshot`, or the case file where there is none; also the case it read and the record it read it from.
    fn run_category(&self, run: &str, dir: &Path, kase: &str) -> (Option<Case>, Category, String) {
        let root = self.project.root();
        let case_dir = root.join(Path::new(kase).parent().unwrap_or(Path::new("")));
        let snap = dir.join("case.snapshot");
        let (text, from) = match std::fs::read(&snap) {
            Ok(b) => (b, format!("runs/{run}/case.snapshot")),
            Err(_) => match rcfd_project::paths::resolve_in(root, kase).and_then(|f| rcfd_project::atomic::read(&f)) {
                Ok(b) => (b, kase.to_string()),
                Err(x) => {
                    return (
                        None,
                        category("general", format!("the case of the run does not read ({x})")),
                        kase.to_string(),
                    )
                }
            },
        };
        let Ok(text) = String::from_utf8(text) else {
            return (
                None,
                category("general", "the case of the run does not read (not UTF-8 text)"),
                from,
            );
        };
        let (case, c) = category_of_text(&text, &case_dir, Some(root));
        (case, c, from)
    }

    /// ADR 0817 D2, D3: the view of the layout of run `run`'s category, and what `show_view` records of the choice.
    pub fn layout_of_run(
        &self,
        run: &str,
        title: Option<&Value>,
        layout: Option<&Value>,
    ) -> Result<(Value, Value), RpcError> {
        let dir = self
            .project
            .run_dir(run)
            .map_err(|x| err(VALIDATION, format!("{run}: not a run id ({x})")))?;
        let record: Value = std::fs::read(dir.join("run.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .ok_or_else(|| err(VALIDATION, format!("{run}: runs/{run}/run.json does not read")))?;
        let kase = record["case"].as_str().unwrap_or_default().replace('\\', "/");
        let (case, cat, from) = self.run_category(run, &dir, &kase);
        let mut none = Vec::new();
        let case_ok = {
            self.case_ref(Some(&json!(kase)), "case", &mut none);
            none.is_empty()
        };
        let mesh_ok = case_ok && self.case_read(Some(&json!(kase)), "case", &mut none).is_some();
        let fields = last_fields(&dir);
        let residuals = std::fs::read(dir.join("residuals.jsonl")).unwrap_or_default();
        let (series, _, _) = crate::residual_series(&residuals, 0, Some(2));
        let mut panels = Vec::new();
        for t in templates(cat.id) {
            match *t {
                Template::Case if case_ok => panels.push(json!({"kind": "case", "title": "Case", "case": kase})),
                Template::Mesh if mesh_ok => panels.push(json!({"kind": "mesh", "title": "Mesh", "case": kase})),
                Template::Field(f) if fields.iter().any(|n| n == f) => {
                    panels.push(json!({"kind": "field", "title": f, "run": run, "field": f}))
                }
                Template::Monitors => {
                    let (types, k) = monitor_types(cat.id);
                    let Some(case) = &case else { continue };
                    let reports = names(case, "reports");
                    let mut chosen = Vec::new();
                    for ty in types {
                        for r in &reports {
                            if text_of(case, &format!("reports.{r}.type")) == *ty
                                && series.contains_key(&format!("monitors.{r}"))
                                && chosen.len() < k
                            {
                                chosen.push(r.clone());
                            }
                        }
                    }
                    for r in chosen {
                        panels.push(json!({"kind": "monitor", "title": r, "run": run, "monitor": r}));
                    }
                }
                Template::Plot if !residuals.is_empty() => {
                    panels.push(json!({"kind": "plot", "title": "Residuals", "run": run}))
                }
                Template::Table => panels.push(json!({"kind": "table", "title": "Run", "runs": [run]})),
                _ => {}
            }
        }
        let label = category_label(cat.id);
        let title = title
            .cloned()
            .unwrap_or_else(|| json!(format!("{label}: {kase} ({run})")));
        let layout = layout.cloned().unwrap_or_else(|| json!({"columns": 2}));
        let view = json!({"version": VERSION, "title": title, "layout": layout, "panels": panels});
        let chosen = json!({"run": run, "case": kase, "category": cat.id, "rule": cat.rule, "from": from});
        Ok((view, chosen))
    }
}

/// The registry folder of the canvases of the project at canonical `root` (ADR 0805 D3).
pub fn registry_dir(root: &Path) -> PathBuf {
    let h = rcfd_project::records::hash_bytes(root.display().to_string().as_bytes());
    let hex = h.trim_start_matches("fnv64:");
    std::env::temp_dir().join("rcfd-canvas").join(hex)
}

/// The canvas registration of this process: its registry file, removed when it is dropped.
#[derive(Debug)]
pub struct Canvas {
    file: PathBuf,
}

impl Drop for Canvas {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.file);
    }
}

/// Register this process as a canvas of the project at canonical `root` (`rcfd-mcp --canvas`).
pub fn register_canvas(root: &Path) -> std::io::Result<Canvas> {
    let dir = registry_dir(root);
    std::fs::create_dir_all(&dir)?;
    let pid = std::process::id();
    let file = dir.join(format!("{pid}.json"));
    let body = json!({"pid": pid, "root": root.display().to_string(), "started": rcfd_project::time::now()});
    std::fs::write(&file, body.to_string())?;
    Ok(Canvas { file })
}

/// The number of live canvases of the project at canonical `root`; registry files of processes that are gone are removed.
pub fn live_canvases(root: &Path) -> usize {
    let Ok(rd) = std::fs::read_dir(registry_dir(root)) else {
        return 0;
    };
    let mut n = 0;
    for f in rd.flatten() {
        let p = f.path();
        let pid = std::fs::read_to_string(&p)
            .ok()
            .and_then(|s| serde_json::from_str::<Value>(&s).ok())
            .and_then(|v| v["pid"].as_u64());
        match pid {
            Some(pid) if rcfd_runner::sys::pid_alive(pid as u32, CANVAS_EXE) => n += 1,
            _ => {
                let _ = std::fs::remove_file(&p);
            }
        }
    }
    n
}

// ---- ADR 0823 (F-980): a view exported as one self-contained styled HTML page ----

/// The journal kind of an exported view (ADR 0823 D1).
pub const VIEW_EXPORTED: &str = "view_exported";
/// The project folder of exported pages.
pub const EXPORTS: &str = "exports";
/// Samples per series, as the Studio reads them (`SERIES_POINTS` of apps/studio-web/src/shared/run.ts).
pub const SERIES_POINTS: u64 = 800;
/// The page's Content-Security-Policy (ADR 0823 D2): nothing is fetched; styles are inline, pictures `data:` only.
pub const EXPORT_CSP: &str = "default-src 'none'; style-src 'unsafe-inline'; img-src data:";
/// Longest `out` of `export_view`, in characters.
pub const MAX_OUT: usize = 200;
/// The Studio's stylesheets the page carries (ADR 0823 D2), in this order.
pub const EXPORT_STYLES: [(&str, &str); 4] = [
    (
        "design/tokens.css",
        include_str!("../../../apps/studio-web/src/renderer/design/tokens.css"),
    ),
    (
        "base.css",
        include_str!("../../../apps/studio-web/src/renderer/base.css"),
    ),
    (
        "canvas/canvas.css",
        include_str!("../../../apps/studio-web/src/renderer/canvas/canvas.css"),
    ),
    (
        "canvas/export.css",
        include_str!("../../../apps/studio-web/src/renderer/canvas/export.css"),
    ),
];
/// Width of a picture's view box.
const PIC_W: f64 = 480.0;
/// Greatest height of a picture's view box.
const PIC_H: f64 = 360.0;

/// Text for HTML: `&`, `<`, `>`, `"` and `'` escaped.
fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// ADR 0823 amendment 4: a number as the Studio writes it, JavaScript's `String(x)` (ECMA-262 `Number::toString`): the
/// shortest digits that read back as the same `f64`, of two equally near the even one; `null` when not finite.
fn num(x: f64) -> String {
    if !x.is_finite() {
        return "null".into();
    }
    if x == 0.0 {
        return "0".into();
    }
    let e = format!("{:e}", x.abs());
    let (m, p) = e.split_once('e').unwrap_or((&e, "0"));
    let mut digits: Vec<u8> = m.bytes().filter(|b| *b != b'.').collect();
    let mut n = p.parse::<i32>().unwrap_or(0) + 1;
    let k = digits.len();
    // of two shortest texts equally near x, the even one: x lies halfway when its exact expansion has k + 1 digits
    // ending in 5
    let exact = format!("{:.800e}", x.abs());
    let (em, _) = exact.split_once('e').unwrap_or((&exact, "0"));
    let all: Vec<u8> = em.bytes().filter(|b| *b != b'.').collect();
    let sig = all.iter().rposition(|b| *b != b'0').map_or(0, |i| i + 1);
    if sig == k + 1 && all[k] == b'5' {
        let mut down = all[..k].to_vec();
        if (down[k - 1] - b'0') % 2 == 1 {
            // the even neighbour is the one above: add one in the last place
            let mut i = k;
            loop {
                if i == 0 {
                    down.insert(0, b'1');
                    n += 1;
                    break;
                }
                i -= 1;
                if down[i] == b'9' {
                    down[i] = b'0';
                } else {
                    down[i] += 1;
                    break;
                }
            }
        }
        while down.len() > 1 && down.last() == Some(&b'0') {
            down.pop();
        }
        digits = down;
    }
    let k = digits.len() as i32;
    let d = String::from_utf8(digits).unwrap_or_default();
    let body = if k <= n && n <= 21 {
        format!("{d}{}", "0".repeat((n - k) as usize))
    } else if 0 < n && n <= 21 {
        format!("{}.{}", &d[..n as usize], &d[n as usize..])
    } else if -6 < n && n <= 0 {
        format!("0.{}{d}", "0".repeat((-n) as usize))
    } else {
        let s = if n - 1 < 0 { '-' } else { '+' };
        let mant = if k == 1 {
            d.clone()
        } else {
            format!("{}.{}", &d[..1], &d[1..])
        };
        format!("{mant}e{s}{}", (n - 1).abs())
    };
    format!("{}{body}", if x < 0.0 { "-" } else { "" })
}

/// ADR 0823 amendment 4: a value as the Studio's `JSON.stringify` writes it: JSON, each `f64` by [`num`].
fn js_json(v: &Value) -> String {
    match v {
        Value::Number(x) if x.is_f64() => num(x.as_f64().unwrap_or(f64::NAN)),
        Value::Array(a) => format!("[{}]", a.iter().map(js_json).collect::<Vec<_>>().join(",")),
        Value::Object(o) => format!(
            "{{{}}}",
            o.iter()
                .map(|(k, x)| format!("{}:{}", Value::String(k.clone()), js_json(x)))
                .collect::<Vec<_>>()
                .join(",")
        ),
        other => other.to_string(),
    }
}

/// One located refusal of `export_view` (error 1003, as `show_view`'s).
fn refusal(errors: &[ViewError]) -> RpcError {
    let first = &errors[0];
    let list: Vec<Value> = errors
        .iter()
        .take(MAX_ERRORS)
        .map(|x| json!({"key": x.key, "reason": x.reason}))
        .collect();
    RpcError {
        code: VALIDATION,
        message: format!("view.{}: {}", first.key, first.reason)
            .replace("view.out: ", "out: ")
            .replace("view.: ", "view: "),
        data: Some(json!({"errors": list, "errors_total": errors.len()})),
    }
}

/// ADR 0823 D1: the errors of `out`, a project-relative `exports/<name>.html`.
fn out_errors(out: &Value) -> Vec<ViewError> {
    let mut errors = Vec::new();
    let reason = format!(
        "is a project-relative path exports/<name>.html, 1 to {MAX_OUT} characters, the name of A-Z a-z 0-9 . _ - only"
    );
    let ok = out.as_str().is_some_and(|o| {
        let name = o.strip_prefix("exports/").unwrap_or("");
        o.chars().count() <= MAX_OUT
            && name.len() > ".html".len()
            && name.ends_with(".html")
            && !name.starts_with('.')
            && !name.contains("..")
            && name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'-')
    });
    if !ok {
        e(&mut errors, "out", reason);
    }
    errors
}

/// The colour of table entry `k` of 256 (`colormap.ts`: entry k is `colormap(k / 255)`), as `#rrggbb`.
fn lut_hex(k: usize) -> String {
    let [r, g, b] = rcfd_post::render::colormap(k as f64 / 255.0);
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// The table entry of value `v` in the range `lo..hi`, as the Studio's shader finds it (`colormap.ts` lines 10 to 13).
fn lut_index(v: f32, lo: f32, hi: f32) -> usize {
    let inv = if hi > lo {
        (1.0 / (f64::from(hi) - f64::from(lo))) as f32
    } else {
        1.0
    };
    let t = ((v - lo) * inv).clamp(0.0, 1.0);
    let t = if t.is_nan() { 0.0 } else { t };
    ((t * 255.0 + 0.5).floor() as usize).min(255)
}

/// The surface triangles of `s` that face the viewer, far to near, projected: the view axis is the thin axis of a
/// one-layer store and z otherwise, from the side `FRONT_SIGN` names. Returns the triangles' indices, their screen
/// corners and the view box size.
fn projected(s: &rcfd_post::picture::StoreSurface, n_cells: usize) -> (Vec<usize>, Vec<[[f64; 2]; 3]>, f64, f64) {
    let layers = rcfd_post::picture::mesh_layers(s, n_cells);
    let w = if layers.two_d { layers.thin } else { 2 };
    let sign = rcfd_post::picture::FRONT_SIGN[w];
    let (mut a, mut b) = ((w + 1) % 3, (w + 2) % 3);
    if sign < 0.0 {
        std::mem::swap(&mut a, &mut b);
    }
    let mut keep: Vec<(f64, usize)> = Vec::new();
    for t in 0..s.n_triangles() {
        let p: Vec<[f64; 3]> = (0..3).map(|k| s.corner(t, k).map(f64::from)).collect();
        let u = [p[1][0] - p[0][0], p[1][1] - p[0][1], p[1][2] - p[0][2]];
        let v = [p[2][0] - p[0][0], p[2][1] - p[0][1], p[2][2] - p[0][2]];
        let n = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        if n[w] * sign > 0.0 {
            keep.push((sign * (p[0][w] + p[1][w] + p[2][w]) / 3.0, t));
        }
    }
    keep.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)));
    let (mut amin, mut amax, mut bmin, mut bmax) = (f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY);
    for &(_, t) in &keep {
        for k in 0..3 {
            let c = s.corner(t, k);
            amin = amin.min(f64::from(c[a]));
            amax = amax.max(f64::from(c[a]));
            bmin = bmin.min(f64::from(c[b]));
            bmax = bmax.max(f64::from(c[b]));
        }
    }
    let (ra, rb) = ((amax - amin).max(1e-30), (bmax - bmin).max(1e-30));
    let scale = (PIC_W / ra).min(PIC_H / rb);
    let (width, height) = (ra * scale, rb * scale);
    let tris = keep
        .iter()
        .map(|&(_, t)| {
            [0, 1, 2].map(|k| {
                let c = s.corner(t, k);
                [(f64::from(c[a]) - amin) * scale, (bmax - f64::from(c[b])) * scale]
            })
        })
        .collect();
    (keep.iter().map(|k| k.1).collect(), tris, width, height)
}

fn points(c: &[[f64; 2]; 3]) -> String {
    format!(
        "{:.1},{:.1} {:.1},{:.1} {:.1},{:.1}",
        c[0][0], c[0][1], c[1][0], c[1][1], c[2][0], c[2][1]
    )
}

/// One series as the Studio reads it (`readRunSeries`): its source name and samples.
struct Series {
    name: String,
    x: Vec<f64>,
    y: Vec<f64>,
}

/// Drawn and skipped samples of `s` by `Extent.add` of the Studio's plots (`plots/range.ts`).
fn counts(s: &Series, log: bool) -> (usize, usize) {
    let mut drawn = 0;
    for (x, y) in s.x.iter().zip(&s.y) {
        let py = if log {
            if *y > 0.0 {
                y.log10()
            } else {
                f64::NAN
            }
        } else {
            *y
        };
        if x.is_finite() && py.is_finite() {
            drawn += 1;
        }
    }
    (drawn, s.x.len() - drawn)
}

/// An SVG of lines (the drawable samples joined, a sample that cannot be drawn breaking the line), `log` on y.
fn lines_svg(series: &[&Series], log: bool, height: f64) -> String {
    let pick = |s: &Series| -> Vec<Option<[f64; 2]>> {
        s.x.iter()
            .zip(&s.y)
            .map(|(x, y)| {
                let py = if log {
                    if *y > 0.0 {
                        y.log10()
                    } else {
                        f64::NAN
                    }
                } else {
                    *y
                };
                (x.is_finite() && py.is_finite()).then_some([*x, py])
            })
            .collect()
    };
    let all: Vec<Vec<Option<[f64; 2]>>> = series.iter().map(|s| pick(s)).collect();
    let (mut xlo, mut xhi, mut ylo, mut yhi) = (f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY);
    for p in all.iter().flatten().flatten() {
        xlo = xlo.min(p[0]);
        xhi = xhi.max(p[0]);
        ylo = ylo.min(p[1]);
        yhi = yhi.max(p[1]);
    }
    let pad = 4.0;
    let sx = (PIC_W - 2.0 * pad) / (xhi - xlo).max(1e-300);
    let sy = (height - 2.0 * pad) / (yhi - ylo).max(1e-300);
    let mut out = format!(
        "<svg class=\"xv-plot\" viewBox=\"0 0 {PIC_W} {height}\" role=\"img\" aria-label=\"{}\">",
        if log { "lines on a log scale" } else { "a line" }
    );
    for (i, s) in all.iter().enumerate() {
        let mut run: Vec<String> = Vec::new();
        let flush = |run: &mut Vec<String>, out: &mut String| {
            if !run.is_empty() {
                out.push_str(&format!(
                    "<polyline class=\"xv-line xv-line-{}\" points=\"{}\"/>",
                    i % 6,
                    run.join(" ")
                ));
                run.clear();
            }
        };
        for p in s {
            match p {
                Some([x, y]) => run.push(format!(
                    "{:.2},{:.2}",
                    pad + (x - xlo) * sx,
                    height - pad - (y - ylo) * sy
                )),
                None => flush(&mut run, &mut out),
            }
        }
        flush(&mut run, &mut out);
    }
    out.push_str("</svg>");
    out
}

impl Workspace {
    /// A project tool's answer as a value (the commands of [`crate::COMMANDS`]).
    fn answer(&self, name: &str, args: &Value) -> Result<Value, RpcError> {
        match self.call(name, args) {
            Some(Ok(crate::Answer::Value(v))) => Ok(v),
            Some(Ok(_)) => Err(err(INTERNAL, format!("{name} answered with a picture"))),
            Some(Err(x)) => Err(x),
            None => Err(err(INTERNAL, format!("{name} is not a command"))),
        }
    }

    /// Every series of run `run` as the Studio reads it (`readRunSeries`): the names (2 samples), then each with
    /// [`SERIES_POINTS`].
    fn studio_series(&self, run: &str) -> Result<Vec<Series>, RpcError> {
        let names = self.answer("read_residuals", &json!({"run": run, "max_points": 2}))?;
        let mut out = Vec::new();
        for name in names["series"]
            .as_object()
            .into_iter()
            .flatten()
            .map(|(k, _)| k.clone())
        {
            let r = self.answer(
                "read_residuals",
                &json!({"run": run, "names": [name], "max_points": SERIES_POINTS}),
            )?;
            let s = &r["series"][&name];
            let arr = |k: &str| -> Vec<f64> {
                s[k].as_array()
                    .into_iter()
                    .flatten()
                    .map(|v| v.as_f64().unwrap_or(f64::NAN))
                    .collect()
            };
            if s.is_object() {
                out.push(Series {
                    name,
                    x: arr("x"),
                    y: arr("y"),
                });
            }
        }
        Ok(out)
    }

    /// The runs of the project as `list_runs` lists them.
    fn run_entries(&self) -> Result<Vec<Value>, RpcError> {
        let v = self.answer("list_runs", &json!({"limit": 1000}))?;
        Ok(v["runs"].as_array().cloned().unwrap_or_default())
    }

    /// ADR 0823 D1: `export_view {view?, out?, reason?}`: check the view (or take the view shown last), build the page
    /// (D2) and write it under `exports/` with one journal line `view_exported`.
    pub fn export_view(&self, args: &Value) -> Result<Value, RpcError> {
        let reason = journal::reason_arg(args)?;
        if let Some(o) = args.get("out") {
            let errors = out_errors(o);
            if !errors.is_empty() {
                return Err(refusal(&errors));
            }
        }
        let (view, id, shown_actor) = match args.get("view") {
            Some(v) => {
                let bytes = serde_json::to_vec(v).map_err(|x| err(INTERNAL, x.to_string()))?;
                (v.clone(), rcfd_project::records::hash_bytes(&bytes), None)
            }
            None => {
                let shown = self.read_view()?;
                if shown["exists"] != json!(true) {
                    return Err(err(
                        crate::NOT_FOUND,
                        "no view has been shown in this project (.rcfd/views/current.json does not exist): give view, or show one with show_view first (nothing was written)",
                    ));
                }
                let id = shown["id"].as_str().unwrap_or_default().to_string();
                (shown["view"].clone(), id, Some(shown["actor"].clone()))
            }
        };
        let errors = self.view_errors(&view);
        if !errors.is_empty() {
            return Err(refusal(&errors));
        }
        let view = match view.get("run").and_then(Value::as_str) {
            Some(run) => {
                let (v, _) = self.layout_of_run(run, view.get("title"), view.get("layout"))?;
                let errors = self.view_errors(&v);
                if let Some(first) = errors.first() {
                    return Err(err(
                        INTERNAL,
                        format!(
                            "the layout of {run} is not a valid view: {}: {}",
                            first.key, first.reason
                        ),
                    ));
                }
                v
            }
            None => view,
        };
        let page = self.page_of(&view, &id, shown_actor.as_ref())?;
        let rel = match args.get("out").and_then(Value::as_str) {
            Some(o) => o.to_string(),
            None => format!("{EXPORTS}/view-{}.html", id.trim_start_matches("fnv64:")),
        };
        let bytes = page.into_bytes();
        let path = self.project.root().join(&rel);
        std::fs::create_dir_all(path.parent().expect("has a parent"))
            .map_err(|x| err(INTERNAL, format!("{EXPORTS}: {x}")))?;
        rcfd_project::atomic::atomic_write(&path, &bytes).map_err(project_error)?;
        let hash = rcfd_project::records::hash_bytes(&bytes);
        let kinds: Vec<Value> = view["panels"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|p| p["kind"].clone())
            .collect();
        let mut line = Map::new();
        line.insert("path".into(), json!(rel));
        line.insert("hash".into(), json!(hash));
        line.insert("bytes".into(), json!(bytes.len()));
        line.insert("panels".into(), json!(kinds));
        journal::append(
            self.project.root(),
            VIEW_EXPORTED,
            &self.origin("export_view", reason),
            line,
        );
        Ok(json!({"exported": true, "id": id, "path": rel, "hash": hash, "bytes": bytes.len(), "panels": kinds.len()}))
    }

    /// ADR 0823 D2 + G22 four-layer quiet architecture: the page of valid view `view` (with panels), its id `id`.
    ///
    /// Visible pixels carry only the glance layer (short titles, graphics, compact 3–4 sig-fig readouts/ticks with
    /// units, and tables). Provenance, exact hashes, actor/PID, export timestamp, per-panel facts and the JSON spec
    /// live in `data-*` attributes and collapsed `<details class="xv-audit">` blocks (G17, G22 Layers 1–3).
    fn page_of(&self, view: &Value, id: &str, shown_actor: Option<&Value>) -> Result<String, RpcError> {
        let info = self.answer("project_info", &json!({}))?;
        let project = info["name"].as_str().unwrap_or("project").to_string();
        let panels = view["panels"].as_array().cloned().unwrap_or_default();
        let columns = view["layout"]["columns"]
            .as_u64()
            .unwrap_or_else(|| panels.len().min(2) as u64)
            .clamp(1, MAX_COLUMNS);
        let title = view["title"].as_str();
        let runs = self.run_entries()?;
        let settings = self.setting_values(view);
        let version = env!("CARGO_PKG_VERSION");
        let mut actor = self
            .actor
            .as_ref()
            .map(|a| a.to_json().to_string())
            .unwrap_or_else(|| "none".into());
        if let Some(a) = shown_actor.filter(|a| !a.is_null()) {
            actor.push_str(&format!("; the view was shown by {a}"));
        }
        let mut h = String::with_capacity(1 << 20);
        h.push_str("<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n");
        h.push_str(&format!(
            "<meta http-equiv=\"Content-Security-Policy\" content=\"{EXPORT_CSP}\">\n"
        ));
        h.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
        h.push_str(&format!(
            "<title>{}</title>\n<style>\n",
            esc(&format!("{} - {project}", title.unwrap_or("View")))
        ));
        for (name, css) in EXPORT_STYLES {
            h.push_str(&format!("/* apps/studio-web/src/renderer/{name} */\n"));
            h.push_str(css);
            h.push('\n');
        }
        h.push_str("</style>\n</head>\n<body>\n");
        h.push_str(&format!(
            "<main class=\"xv\" data-export-view=\"{}\" data-engine=\"{version}\">\n",
            esc(id)
        ));
        let spec = serde_json::to_string_pretty(view).unwrap_or_default();
        let heading = title.unwrap_or(&project);
        h.push_str(&format!(
            "<header class=\"xv-head\"><h1 class=\"xv-project\" data-project=\"{}\" data-words=\"data\">{}</h1><details class=\"xv-audit xv-head-audit\"><summary>Provenance</summary><p class=\"xv-note\">Project {}; exported {} by export_view of rcfd {version} (actor {}); view {}.</p><pre>{}</pre></details></header>\n",
            esc(&project),
            esc(heading),
            esc(&project),
            esc(&rcfd_project::time::now()),
            esc(&actor),
            esc(id),
            esc(&spec)
        ));
        h.push_str(&format!(
            "<section class=\"cv\" aria-label=\"{}\" data-panels=\"{}\">\n",
            esc(heading),
            panels.len()
        ));
        h.push_str(&format!("<div class=\"cv-grid\" data-columns=\"{columns}\">\n"));
        for (i, p) in panels.iter().enumerate() {
            let panel = self.panel_html(p, i, &runs, &settings)?;
            let mut hashes = Vec::new();
            for s in &panel.sources {
                let b = std::fs::read(self.project.root().join(s)).unwrap_or_default();
                hashes.push(rcfd_project::records::hash_bytes(&b));
            }
            h.push_str(&format!(
                "<div class=\"cv-panel\" data-canvas-panel=\"{i}\" data-kind=\"{}\" data-sources=\"{}\" data-hashes=\"{}\">\n",
                esc(p["kind"].as_str().unwrap_or_default()),
                esc(&panel.sources.join(" ")),
                hashes.join(" ")
            ));
            if let Some(t) = p["title"].as_str() {
                h.push_str(&format!("<h3 class=\"cv-panel-title\">{}</h3>\n", esc(t)));
            }
            h.push_str(&panel.glance);
            let listed: Vec<String> = panel
                .sources
                .iter()
                .zip(&hashes)
                .map(|(s, x)| format!("{} ({x})", esc(s)))
                .collect();
            let facts_html = panel
                .audit_facts
                .map(|f| format!("<p class=\"xv-facts\">{f}</p>"))
                .unwrap_or_default();
            h.push_str(&format!(
                "<details class=\"xv-audit xv-panel-audit\"><summary aria-label=\"Panel provenance\">ⓘ</summary>{facts_html}<p class=\"xv-sources\">From {}; rcfd {version}.</p></details>\n</div>\n",
                listed.join(", ")
            ));
        }
        h.push_str("</div>\n</section>\n</main>\n</body>\n</html>\n");
        Ok(h)
    }

    /// The typed glance/audit markup of panel `p` (index `i`) and the project-relative files it read (G22 Layer 3).
    fn panel_html(&self, p: &Value, i: usize, runs: &[Value], settings: &[Value]) -> Result<PanelOutput, RpcError> {
        let s = |k: &str| p[k].as_str().unwrap_or_default().replace('\\', "/");
        let run_files = |run: &str| {
            let mut v = vec![format!("runs/{run}/run.json")];
            if self.project.root().join(format!("runs/{run}/status.json")).exists() {
                v.push(format!("runs/{run}/status.json"));
            }
            v
        };
        let entry = |run: &str| {
            runs.iter()
                .find(|r| r["id"] == json!(run))
                .cloned()
                .unwrap_or(Value::Null)
        };
        Ok(match p["kind"].as_str().unwrap_or_default() {
            "case" => {
                let kase = s("case");
                let text = std::fs::read_to_string(self.project.root().join(&kase)).unwrap_or_default();
                PanelOutput {
                    glance: format!(
                        "<div class=\"xv-case\" data-path=\"{}\"><pre class=\"xv-text\">{}</pre></div>\n",
                        esc(&kase),
                        esc(&text)
                    ),
                    audit_facts: Some(esc(&kase)),
                    sources: vec![kase],
                }
            }
            "mesh" if p.get("case").is_some() => {
                let kase = s("case");
                let mut none = Vec::new();
                let case = self
                    .case_read(Some(&json!(kase)), "case", &mut none)
                    .ok_or_else(|| err(INTERNAL, format!("{kase}: does not read as a case")))?;
                let (mesh, _) = self.build_case_mesh(&kase, &case)?;
                let surf = rcfd_post::picture::store_surface(&mesh).map_err(|x| err(INTERNAL, x.to_string()))?;
                let (_, tris, w, hgt) = projected(&surf, mesh.n_cells());
                let mut svg = format!(
                    "<svg class=\"xv-picture\" viewBox=\"0 0 {w:.1} {hgt:.1}\" role=\"img\" aria-label=\"the mesh of {}\">",
                    esc(&kase)
                );
                for c in &tris {
                    svg.push_str(&format!("<polygon class=\"xv-mesh-tri\" points=\"{}\"/>", points(c)));
                }
                svg.push_str("</svg>");
                PanelOutput {
                    glance: format!(
                        "<div class=\"xv-mesh\" data-case=\"{}\" data-triangles=\"{}\">{svg}<div class=\"xv-keys\"><span class=\"xv-num\">{} cells</span></div></div>\n",
                        esc(&kase),
                        surf.n_triangles(),
                        mesh.n_cells()
                    ),
                    audit_facts: Some(format!(
                        "The mesh of {}: {} cells, {} surface triangles ({} facing the viewer drawn).",
                        esc(&kase),
                        mesh.n_cells(),
                        surf.n_triangles(),
                        tris.len()
                    )),
                    sources: vec![kase],
                }
            }
            "mesh" => {
                let m = s("mesh");
                PanelOutput {
                    glance: format!(
                        "<div class=\"xv-meshentry\" data-path=\"{}\"><span class=\"xv-keys\">{}</span></div>\n",
                        esc(&m),
                        esc(&m)
                    ),
                    audit_facts: Some(format!("Mesh file {}", esc(&m))),
                    sources: vec![m],
                }
            }
            "setting" => {
                let got = settings.iter().find(|x| x["panel"] == json!(i));
                let value = got.and_then(|g| g.get("value")).map(js_json);
                let error = got.and_then(|g| g.get("error")).and_then(Value::as_str);
                let mut out = format!(
                    "<div class=\"cv-setting\" data-setting=\"{}\"{}><div class=\"cv-setting-value\"><span class=\"cv-setting-path\">{}</span> = <output>{}</output></div>",
                    esc(&s("setting")),
                    value.as_ref().map(|v| format!(" data-value=\"{}\"", esc(v))).unwrap_or_default(),
                    esc(&s("setting")),
                    esc(value.as_deref().unwrap_or(""))
                );
                if let Some(x) = error {
                    out.push_str(&format!("<p class=\"cv-change\" role=\"alert\">{}</p>", esc(x)));
                }
                out.push_str("</div>\n");
                PanelOutput {
                    glance: out,
                    audit_facts: None,
                    sources: vec![s("case")],
                }
            }
            "table" => {
                let want: Vec<String> = p["runs"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|r| r.as_str().map(String::from))
                    .collect();
                let mut out = String::from(
                    "<table class=\"xv-table\"><thead><tr><th scope=\"col\">Case</th><th scope=\"col\">State</th><th scope=\"col\">Step</th><th scope=\"col\">Time</th></tr></thead><tbody>",
                );
                let mut files = Vec::new();
                for r in runs.iter().filter(|r| want.iter().any(|w| r["id"] == json!(w))) {
                    let rid = r["id"].as_str().unwrap_or_default();
                    let case_rel = r["case"].as_str().unwrap_or_default();
                    let case_short = case_rel
                        .strip_prefix("cases/")
                        .unwrap_or(case_rel)
                        .strip_suffix(".toml")
                        .or_else(|| {
                            case_rel
                                .strip_prefix("cases/")
                                .unwrap_or(case_rel)
                                .strip_suffix(".json")
                        })
                        .unwrap_or(case_rel);
                    let time_disp = r["time"]
                        .as_f64()
                        .map(fmt_glance)
                        .unwrap_or_else(|| js_json(&r["time"]));
                    out.push_str(&format!(
                        "<tr data-run=\"{}\" data-step=\"{}\"><th scope=\"row\">{}</th><td>{}</td><td class=\"xv-num\">{}</td><td class=\"xv-num\">{}</td></tr>",
                        esc(rid),
                        js_json(&r["step"]),
                        esc(case_short),
                        esc(r["state"].as_str().unwrap_or_default()),
                        js_json(&r["step"]),
                        esc(&time_disp)
                    ));
                    files.extend(run_files(rid));
                }
                out.push_str("</tbody></table>\n");
                PanelOutput {
                    glance: out,
                    audit_facts: None,
                    sources: files,
                }
            }
            "run" => {
                let run = s("run");
                let r = entry(&run);
                let snaps = rcfd_tsstore::Reader::open(&self.project.root().join(format!("runs/{run}/fields")))
                    .map(|x| x.snapshots().len().to_string())
                    .unwrap_or_else(|_| "none".into());
                let case_rel = r["case"].as_str().unwrap_or_default();
                let case_short = case_rel.strip_prefix("cases/").unwrap_or(case_rel);
                PanelOutput {
                    glance: format!(
                        "<div class=\"xv-run\" data-run=\"{}\" data-step=\"{}\" data-snapshots=\"{}\"><dl class=\"xv-grid\"><dt>Case</dt><dd>{}</dd><dt>State</dt><dd>{}</dd><dt>Steps</dt><dd class=\"xv-num\">{}</dd><dt>Snapshots</dt><dd class=\"xv-num\">{}</dd></dl></div>\n",
                        esc(&run),
                        js_json(&r["step"]),
                        snaps,
                        esc(case_short),
                        esc(r["state"].as_str().unwrap_or_default()),
                        js_json(&r["step"]),
                        snaps
                    ),
                    audit_facts: Some(format!("Run {} ({})", esc(&run), esc(case_rel))),
                    sources: run_files(&run),
                }
            }
            "field" => {
                let run = s("run");
                let field = s("field");
                let rel = format!("runs/{run}/fields");
                let tserr = |x: &dyn std::fmt::Display| err(INTERNAL, format!("{rel}: {x}"));
                let mut r = rcfd_tsstore::Reader::open(&self.project.root().join(&rel)).map_err(|x| tserr(&x))?;
                let n = r.snapshots().len();
                let snap = p["snapshot"]
                    .as_u64()
                    .map(|x| x as usize)
                    .unwrap_or(n.saturating_sub(1));
                let (step, time) = (r.snapshots()[snap].step, r.snapshots()[snap].time);
                let unit = r
                    .fields(snap)
                    .map_err(|x| tserr(&x))?
                    .into_iter()
                    .find(|f| f.name == field)
                    .map(|f| f.unit)
                    .filter(|u| u != "unknown");
                let values: Vec<f32> = r
                    .read_field(snap, &field)
                    .map_err(|x| tserr(&x))?
                    .iter()
                    .map(|&v| v as f32)
                    .collect();
                let mesh = r.mesh().map_err(|x| tserr(&x))?;
                let surf = rcfd_post::picture::store_surface(&mesh).map_err(|x| err(INTERNAL, x.to_string()))?;
                let at = |c: u32| values.get(c as usize).copied().unwrap_or(f32::NAN);
                let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
                for &c in &surf.cells {
                    let v = at(c);
                    if v < lo {
                        lo = v;
                    }
                    if v > hi {
                        hi = v;
                    }
                }
                let (order, tris, w, hgt) = projected(&surf, mesh.n_cells());
                let mut svg = format!(
                    "<svg class=\"xv-picture\" viewBox=\"0 0 {w:.1} {hgt:.1}\" role=\"img\" aria-label=\"{} of {} at snapshot {snap}\">",
                    esc(&field),
                    esc(&run)
                );
                for (t, c) in order.iter().zip(&tris) {
                    let k = lut_index(at(surf.cells[*t]), lo, hi);
                    svg.push_str(&format!("<polygon points=\"{}\" fill=\"{}\"/>", points(c), lut_hex(k)));
                }
                svg.push_str("</svg>");
                let mut legend =
                    String::from("<svg viewBox=\"0 0 256 1\" preserveAspectRatio=\"none\" aria-hidden=\"true\">");
                for k in 0..256 {
                    legend.push_str(&format!(
                        "<rect x=\"{k}\" width=\"1\" height=\"1\" fill=\"{}\"/>",
                        lut_hex(k)
                    ));
                }
                legend.push_str("</svg>");
                let (lo64, hi64) = (f64::from(lo), f64::from(hi));
                let unit_span = unit
                    .as_deref()
                    .map(|u| format!("<span class=\"xv-unit\">{}</span>", esc(u)))
                    .unwrap_or_default();
                let unit_text = unit.map(|u| format!(", {u}")).unwrap_or_default();
                PanelOutput {
                    glance: format!(
                        "<div class=\"xv-field\" data-field=\"{}\" data-lo=\"{}\" data-hi=\"{}\" data-snapshot=\"{snap}\" data-step=\"{}\">{svg}<div class=\"xv-legend\"><span class=\"xv-num\">{}</span>{legend}<span class=\"xv-num\">{}</span>{unit_span}</div></div>\n",
                        esc(&field),
                        num(lo64),
                        num(hi64),
                        step,
                        esc(&fmt_glance(lo64)),
                        esc(&fmt_glance(hi64))
                    ),
                    audit_facts: Some(format!(
                        "{}{} of {} at snapshot {snap} (step {}, time {}); range {} to {}.",
                        esc(&field),
                        esc(&unit_text),
                        esc(&run),
                        step,
                        num(time),
                        num(lo64),
                        num(hi64)
                    )),
                    sources: vec![format!("{rel}/store.rts")],
                }
            }
            kind @ ("plot" | "monitor") => {
                let run = match p.get("run").and_then(Value::as_str) {
                    Some(r) => r.to_string(),
                    None => followed_run(runs, &s("case")).unwrap_or_default(),
                };
                let series = if run.is_empty() {
                    Vec::new()
                } else {
                    self.studio_series(&run)?
                };
                let files = if run.is_empty() {
                    Vec::new()
                } else {
                    vec![format!("runs/{run}/residuals.jsonl")]
                };
                if kind == "plot" {
                    let res: Vec<&Series> = series.iter().filter(|x| x.name.starts_with("residuals.")).collect();
                    let (mut d, mut k) = (0, 0);
                    for x in &res {
                        let (a, b) = counts(x, true);
                        d += a;
                        k += b;
                    }
                    let chips: Vec<String> = res
                        .iter()
                        .enumerate()
                        .map(|(idx, x)| {
                            let last = x.y.last().copied().unwrap_or(f64::NAN);
                            let short = esc(x.name.trim_start_matches("residuals."));
                            format!(
                                "<span class=\"xv-key\"><span class=\"xv-swatch xv-swatch-{}\" aria-hidden=\"true\"></span>{short} <span class=\"xv-num\">{}</span></span>",
                                idx % 6,
                                esc(&fmt_glance(last))
                            )
                        })
                        .collect();
                    let names: Vec<String> = res
                        .iter()
                        .map(|x| {
                            let last = x.y.last().copied().unwrap_or(f64::NAN);
                            format!("{} {}", esc(x.name.trim_start_matches("residuals.")), num(last))
                        })
                        .collect();
                    PanelOutput {
                        glance: format!(
                            "<div class=\"xv-series\" data-run=\"{}\" data-drawn=\"{d}\" data-skipped=\"{k}\">{}<div class=\"xv-keys\">{}</div></div>\n",
                            esc(&run),
                            lines_svg(&res, true, 120.0),
                            chips.join("")
                        ),
                        audit_facts: Some(format!(
                            "Residuals of {} (last values): {}; {d} samples drawn, {k} not drawn.",
                            esc(&run),
                            names.join(", ")
                        )),
                        sources: files,
                    }
                } else {
                    let m = s("monitor");
                    let one = series.iter().find(|x| x.name == format!("monitors.{m}"));
                    let last = one.and_then(|x| Some((*x.x.last()?, *x.y.last()?)));
                    let (d, k) = one.map(|x| counts(x, false)).unwrap_or((0, 0));
                    let attrs = last
                        .map(|(x, y)| format!(" data-last-step=\"{}\" data-last-value=\"{}\"", num(x), num(y)))
                        .unwrap_or_default();
                    let kpi = last
                        .map(|(_, y)| {
                            format!(
                                "<div class=\"xv-kpi\"><span class=\"xv-num\">{}</span></div>",
                                esc(&fmt_glance(y))
                            )
                        })
                        .unwrap_or_else(|| "<div class=\"xv-keys\">no samples</div>".into());
                    let text = last
                        .map(|(x, y)| format!("{} at step {}: {}", esc(&m), num(x), num(y)))
                        .unwrap_or_else(|| format!("{}: no samples", esc(&m)));
                    PanelOutput {
                        glance: format!(
                            "<div class=\"xv-series\" data-run=\"{}\"{attrs} data-drawn=\"{d}\" data-skipped=\"{k}\">{}{kpi}</div>\n",
                            esc(&run),
                            lines_svg(&one.into_iter().collect::<Vec<_>>(), false, 80.0)
                        ),
                        audit_facts: Some(format!("{text} ({} of {}).", esc(&m), esc(&run))),
                        sources: files,
                    }
                }
            }
            other => return Err(err(INTERNAL, format!("'{other}' is not a kind the page draws"))),
        })
    }
}

/// Typed panel output separating the visible glance layer from collapsed audit/provenance facts (G22 Layer 3).
struct PanelOutput {
    glance: String,
    audit_facts: Option<String>,
    sources: Vec<String>,
}

/// One shared compact number formatter for visible glance-layer labels and ticks (G22 Layers 1 & 3).
/// Exact round-trip values stay in `data-*` attributes and collapsed `<details>` blocks.
pub fn fmt_glance(v: f64) -> String {
    if !v.is_finite() {
        return "—".into();
    }
    if v == 0.0 {
        return "0".into();
    }
    let a = v.abs();
    if (1e-3..1e4).contains(&a) {
        let raw = if a >= 100.0 {
            format!("{v:.1}")
        } else if a >= 10.0 {
            format!("{v:.2}")
        } else if a >= 0.1 {
            format!("{v:.3}")
        } else if a >= 0.01 {
            format!("{v:.4}")
        } else {
            format!("{v:.5}")
        };
        if raw.contains('.') {
            let trimmed = raw.trim_end_matches('0').trim_end_matches('.');
            if trimmed == "-0" {
                "0".into()
            } else {
                trimmed.into()
            }
        } else {
            raw
        }
    } else {
        format!("{v:.2e}")
    }
}

/// The run a panel of case `kase` follows: the newest `created`, then the greatest id (`followedRun` of
/// apps/studio-web/src/renderer/canvas/model.ts).
fn followed_run(runs: &[Value], kase: &str) -> Option<String> {
    let norm = |s: &str| s.replace('\\', "/");
    let want = norm(kase);
    let mut best: Option<(&str, &str)> = None;
    for r in runs {
        if norm(r["case"].as_str().unwrap_or_default()) != want {
            continue;
        }
        let (c, id) = (
            r["created"].as_str().unwrap_or_default(),
            r["id"].as_str().unwrap_or_default(),
        );
        if best.is_none_or(|(bc, bid)| c > bc || (c == bc && id > bid)) {
            best = Some((c, id));
        }
    }
    best.map(|b| b.1.to_string())
}

/// ADR 0823 R3: what the page must not hold (a reference to anything outside it); empty when it holds none.
pub fn page_references(page: &str) -> Vec<String> {
    let mut found = Vec::new();
    let low = page.to_ascii_lowercase();
    for bad in [
        "<script", "<link", "<iframe", "<object", "<embed", "<img", "@import", "http:", "https:", "src=", "href=",
    ] {
        if low.contains(bad) {
            found.push(bad.to_string());
        }
    }
    let mut rest = low.as_str();
    while let Some(at) = rest.find("url(") {
        let after = rest[at + 4..].trim_start_matches(['"', '\'', ' ']);
        if !(after.starts_with('#') || after.starts_with("data:")) {
            found.push(format!("url({}", &after[..after.len().min(20)]));
        }
        rest = &rest[at + 4..];
    }
    found
}

/// G22 Layer 2: universal no-prose, no-machine-strings check on the visible glance layer of an exported HTML page.
///
/// Strips `<head>`, `<style>`, collapsed `<details>...</details>` (the audit/provenance layer) and `<pre>...</pre>`
/// (user-authored case text), then reports any remaining:
/// - visible `<p>` paragraph outside `.cv-change` / `.xv-exception` / `.xv-disclaimer`,
/// - raw machine string (`fnv64:`, `runs/r-`, `{"client":`, `"pid":`, ISO timestamp, or float with >= 6 decimal digits),
/// - multi-clause sentence (`; ` or sentence-ending `.`) in visible text.
pub fn page_quiet_violations(page: &str) -> Vec<String> {
    fn strip_block(s: &str, open: &str, close: &str) -> String {
        let mut out = String::with_capacity(s.len());
        let mut rest = s;
        while let Some(i) = rest.find(open) {
            out.push_str(&rest[..i]);
            let after = &rest[i + open.len()..];
            if let Some(j) = after.find(close) {
                rest = &after[j + close.len()..];
            } else {
                rest = "";
                break;
            }
        }
        out.push_str(rest);
        out
    }
    let mut glance = strip_block(page, "<head", "</head>");
    glance = strip_block(&glance, "<style", "</style>");
    glance = strip_block(&glance, "<details", "</details>");
    glance = strip_block(&glance, "<pre", "</pre>");

    let mut bad = Vec::new();
    // 1. No visible <p> paragraphs outside exception/disclaimer alerts
    let mut scan = glance.as_str();
    while let Some(i) = scan.find("<p") {
        let after = &scan[i + 2..];
        let is_p = after
            .as_bytes()
            .first()
            .is_some_and(|b| matches!(b, b'>' | b' ' | b'\t' | b'\n' | b'\r'));
        if is_p {
            let tag_end = after.find('>').unwrap_or(after.len());
            let tag = &after[..tag_end];
            let allowed = tag.contains("cv-change")
                || tag.contains("xv-exception")
                || tag.contains("xv-disclaimer")
                || tag.contains("role=\"alert\"");
            if !allowed {
                bad.push(format!("visible <p> paragraph: <p{}>", &tag[..tag.len().min(48)]));
            }
        }
        scan = after;
    }

    // Extract visible text segments between tags
    let mut visible = Vec::new();
    let mut in_tag = false;
    let mut cur = String::new();
    for ch in glance.chars() {
        match ch {
            '<' => {
                let t = cur.trim();
                if !t.is_empty() {
                    visible.push(t.to_string());
                }
                cur.clear();
                in_tag = true;
            }
            '>' => in_tag = false,
            _ if !in_tag => cur.push(ch),
            _ => {}
        }
    }
    let tail = cur.trim();
    if !tail.is_empty() {
        visible.push(tail.to_string());
    }

    for text in &visible {
        for pat in ["fnv64:", "runs/r-", "{\"client\":", "\"pid\":"] {
            if text.contains(pat) {
                bad.push(format!("machine string '{pat}' in visible text: {text}"));
            }
        }
        // ISO timestamp pattern Tdd:dd:dd
        let bytes = text.as_bytes();
        for w in bytes.windows(9) {
            if w[0] == b'T'
                && w[1].is_ascii_digit()
                && w[2].is_ascii_digit()
                && w[3] == b':'
                && w[4].is_ascii_digit()
                && w[5].is_ascii_digit()
                && w[6] == b':'
                && w[7].is_ascii_digit()
                && w[8].is_ascii_digit()
            {
                bad.push(format!("ISO timestamp in visible text: {text}"));
                break;
            }
        }
        // Over-precise float: '.' followed by 6+ ASCII digits
        for (idx, b) in bytes.iter().enumerate() {
            if *b == b'.' && idx + 6 < bytes.len() && bytes[idx + 1..=idx + 6].iter().all(u8::is_ascii_digit) {
                bad.push(format!("over-precise float (>=6 decimals) in visible text: {text}"));
                break;
            }
        }
        // Multi-clause prose or sentence punctuation
        if text.contains("; ") || text.ends_with('.') {
            bad.push(format!("sentence punctuation in visible text: {text}"));
        }
    }
    bad
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// A copy of example-projects/quick-start in a fresh temporary folder, with one mesh entry under meshes/.
    fn quick_start() -> (PathBuf, Workspace) {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dir = std::env::temp_dir().join(format!("rcfd-f976-unit-{}-{}", std::process::id(), rand_tag()));
        copy_dir(&repo.join("example-projects/quick-start"), &dir);
        std::fs::write(dir.join("meshes/box.msh"), "$MeshFormat\n2.2 0 8\n$EndMeshFormat\n").unwrap();
        let ws = Workspace::open(&dir).unwrap();
        (dir, ws)
    }

    fn rand_tag() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    }

    fn copy_dir(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();
        for e in std::fs::read_dir(from).unwrap().flatten() {
            let t = to.join(e.file_name());
            if e.file_type().unwrap().is_dir() {
                copy_dir(&e.path(), &t);
            } else {
                std::fs::copy(e.path(), t).unwrap();
            }
        }
    }

    const RUN: &str = "r-20261028T152641-3900";
    const OTHER: &str = "r-20261028T152641-5cc1";

    /// The valid specification of V2: one panel of each kind and a layout.
    fn valid() -> Value {
        json!({"version": 1, "title": "Cavity", "layout": {"columns": 2}, "panels": [
            {"kind": "case", "case": "cases/cavity-re100.toml"},
            {"kind": "run", "run": RUN},
            {"kind": "field", "run": RUN, "field": "p"},
            {"kind": "mesh", "mesh": "meshes/box.msh"},
            {"kind": "plot", "run": RUN},
            {"kind": "monitor", "run": RUN, "monitor": "nu_hot"},
            {"kind": "table", "runs": [RUN, OTHER]}
        ]})
    }

    /// ADR 0805 V2: the valid specification of the seven kinds is accepted.
    #[test]
    fn v2_a_view_of_the_seven_kinds_is_valid() {
        let (dir, ws) = quick_start();
        let errors = ws.view_errors(&valid());
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(errors, vec![], "the valid view is refused");
    }

    /// ADR 0805 V1: the 16 invalid specifications, each refused at its key.
    #[test]
    fn v1_invalid_views_are_refused_at_their_key() {
        let (dir, ws) = quick_start();
        let with = |f: &dyn Fn(&mut Value)| {
            let mut v = valid();
            f(&mut v);
            v
        };
        let cases: Vec<(&str, Value, &str, &str)> = vec![
            (
                "version 2",
                with(&|v| v["version"] = json!(2)),
                "version",
                "not a version",
            ),
            (
                "version missing",
                with(&|v| {
                    v.as_object_mut().unwrap().remove("version");
                }),
                "version",
                "required",
            ),
            (
                "panels missing",
                with(&|v| {
                    v.as_object_mut().unwrap().remove("panels");
                }),
                "panels",
                "required",
            ),
            (
                "panels empty",
                with(&|v| v["panels"] = json!([])),
                "panels",
                "holds 0 panels",
            ),
            (
                "9 panels",
                with(&|v| v["panels"] = json!(vec![json!({"kind": "plot", "run": RUN}); 9])),
                "panels",
                "holds 9 panels",
            ),
            (
                "columns 5",
                with(&|v| v["layout"]["columns"] = json!(5)),
                "layout.columns",
                "not an integer 1 to 4",
            ),
            (
                "key colour",
                with(&|v| v["panels"][1]["colour"] = json!("red")),
                "panels[1].colour",
                "not a key",
            ),
            (
                "kind chart",
                with(&|v| v["panels"][0] = json!({"kind": "chart"})),
                "panels[0].kind",
                "not a kind",
            ),
            (
                "no such run",
                with(&|v| v["panels"][1]["run"] = json!("r-20991231T000000-0000")),
                "panels[1].run",
                "no such run",
            ),
            (
                "case outside cases/",
                with(&|v| v["panels"][0]["case"] = json!("notes/README.md")),
                "panels[0].case",
                "under cases/",
            ),
            (
                "no such case",
                with(&|v| v["panels"][0]["case"] = json!("cases/nope.toml")),
                "panels[0].case",
                "no such case",
            ),
            (
                "mesh not listed",
                with(&|v| v["panels"][3]["mesh"] = json!("meshes/none.msh")),
                "panels[3].mesh",
                "not a mesh",
            ),
            (
                "field not stored",
                with(&|v| v["panels"][2]["field"] = json!("vorticity")),
                "panels[2].field",
                "not stored",
            ),
            (
                "snapshot past the last",
                with(&|v| v["panels"][2]["snapshot"] = json!(100000)),
                "panels[2].snapshot",
                "not a snapshot index",
            ),
            (
                "monitor not a series",
                with(&|v| v["panels"][5]["monitor"] = json!("drag")),
                "panels[5].monitor",
                "not a monitor",
            ),
            (
                "table repeats a run",
                with(&|v| v["panels"][6]["runs"] = json!([RUN, RUN])),
                "panels[6].runs[1]",
                "named twice",
            ),
        ];
        let mut failed = Vec::new();
        for (what, v, key, word) in &cases {
            let errors = ws.view_errors(v);
            let ok = errors.len() == 1 && errors[0].key == *key && errors[0].reason.contains(word);
            eprintln!(
                "V1 {what}: {errors:?} -> {}",
                if ok { "refused at its key" } else { "WRONG" }
            );
            if !ok {
                failed.push(*what);
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(cases.len(), 16);
        assert!(failed.is_empty(), "not refused at the registered key: {failed:?}");
    }

    const RE100: &str = "cases/cavity-re100.toml";

    /// ADR 0807 V3's view: F-976's seven kinds, then the three forms of F-979 (D1): 8 panels.
    fn live() -> Value {
        let mut v = valid();
        let p = v["panels"].as_array_mut().unwrap();
        p.remove(6);
        p.remove(4);
        p.remove(1);
        p.push(json!({"kind": "mesh", "case": RE100}));
        p.push(json!({"kind": "setting", "case": RE100, "setting": "physics.nu"}));
        p.push(json!({"kind": "monitor", "case": RE100, "monitor": "lid_drag"}));
        p.push(json!({"kind": "table", "runs": [RUN, OTHER]}));
        v
    }

    /// ADR 0807 V3: the three new forms are accepted beside the kinds of F-976.
    #[test]
    fn v3_f979_the_live_forms_are_valid() {
        let (dir, ws) = quick_start();
        let v = live();
        let kinds: Vec<&str> = v["panels"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["kind"].as_str().unwrap())
            .collect();
        let errors = ws.view_errors(&v);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(
            kinds,
            ["case", "field", "mesh", "monitor", "mesh", "setting", "monitor", "table"]
        );
        assert_eq!(errors, vec![], "the live forms are refused");
    }

    /// ADR 0807 V4: the 8 invalid views of the new forms, each refused at its key.
    #[test]
    fn v4_f979_invalid_live_forms_are_refused_at_their_key() {
        let (dir, ws) = quick_start();
        let one = |p: Value| json!({"version": 1, "panels": [p]});
        let cases: Vec<(&str, Value, &str, &str)> = vec![
            (
                "mesh and case",
                one(json!({"kind": "mesh", "mesh": "meshes/box.msh", "case": RE100})),
                "panels[0].case",
                "not both",
            ),
            (
                "mesh of a case outside cases/",
                one(json!({"kind": "mesh", "case": "notes/README.md"})),
                "panels[0].case",
                "under cases/",
            ),
            (
                "setting missing",
                one(json!({"kind": "setting", "case": RE100})),
                "panels[0].setting",
                "required",
            ),
            (
                "not a setting",
                one(json!({"kind": "setting", "case": RE100, "setting": "physics.nope"})),
                "panels[0].setting",
                "not a setting",
            ),
            (
                "setting with run",
                one(json!({"kind": "setting", "case": RE100, "setting": "physics.nu", "run": RUN})),
                "panels[0].run",
                "not a key",
            ),
            (
                "monitor with run and case",
                one(json!({"kind": "monitor", "run": RUN, "case": RE100, "monitor": "nu_hot"})),
                "panels[0].case",
                "not both",
            ),
            (
                "monitor not a report",
                one(json!({"kind": "monitor", "case": RE100, "monitor": "drag"})),
                "panels[0].monitor",
                "not a report",
            ),
            (
                "setting of no such case",
                one(json!({"kind": "setting", "case": "cases/nope.toml", "setting": "physics.nu"})),
                "panels[0].case",
                "no such case",
            ),
        ];
        let mut failed = Vec::new();
        for (what, v, key, word) in &cases {
            let errors = ws.view_errors(v);
            let ok = errors.len() == 1 && errors[0].key == *key && errors[0].reason.contains(word);
            eprintln!(
                "V4 {what}: {errors:?} -> {}",
                if ok { "refused at its key" } else { "WRONG" }
            );
            if !ok {
                failed.push(*what);
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(cases.len(), 8);
        assert!(failed.is_empty(), "not refused at the registered key: {failed:?}");
    }

    // ---- F-977 (ADR 0817): the category of a case and the layout of a run ----

    const D5CB: &str = "r-20261028T152128-d5cb";
    const FAN: &str = "r-20261030T223519-0489";
    const F8C2: &str = "r-20261028T152641-f8c2";
    const STRUCT: &str = "r-20261126T000000-5701";

    fn repo() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    /// ADR 0817 (Checks): the still pool, `pool_text("", "")` of `crates/rcfd-api/tests/vof_tree.rs` with `end_time` 0.05
    /// and an `[output]` block.
    const STILL_POOL: &str = r#"
[mesh]
generator = "box_2d"
nx = 10
ny = 20
lx = 0.5
ly = 1.0
[physics]
gravity = [0.0, -9.81, 0.0]
[multiphase]
enabled = true
density_1 = 1000.0
density_2 = 1.0
viscosity_1 = 1.0e-3
viscosity_2 = 1.8e-5

[multiphase.initial]
shape = "half_space"
normal = [0.0, 1.0, 0.0]
point = [0.0, 0.5, 0.0]
[solver]
algorithm = "piso"
convection = "upwind"
n_correctors = 3
dt = 1.0e-3
end_time = 0.05

[boundaries.xmin]
velocity = "fixed"
[boundaries.xmax]
velocity = "fixed"
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[output]
store_interval = 10
store_fields = ["alpha", "ux", "uy", "p"]
"#;

    /// The f977 project of ADR 0817: quick-start with the cylinder's case and run and the fan's case and run.
    fn f977_project() -> (PathBuf, Workspace) {
        let repo = repo();
        let dir = std::env::temp_dir().join(format!("rcfd-f977-unit-{}-{}", std::process::id(), rand_tag()));
        copy_dir(&repo.join("example-projects/quick-start"), &dir);
        for (p, case, run) in [
            ("wind-tunnel", "cylinder-re100.toml", D5CB),
            ("axial-fan", "fan01-passage-upwind.toml", FAN),
        ] {
            let from = repo.join("example-projects").join(p);
            std::fs::copy(from.join("cases").join(case), dir.join("cases").join(case)).unwrap();
            copy_dir(&from.join("runs").join(run), &dir.join("runs").join(run));
        }
        let ws = Workspace::open(&dir).unwrap();
        (dir, ws)
    }

    fn cat_of_file(f: &Path, sandbox: Option<&Path>) -> Category {
        let text = std::fs::read_to_string(f).unwrap();
        category_of_text(&text, f.parent().unwrap(), sandbox).1
    }

    /// ADR 0817 R1: the category of the 14 example cases, the cantilever and the still pool.
    #[test]
    fn r1_f977_the_example_cases_take_their_category() {
        let ex = repo().join("example-projects");
        let int = "internal_flow_and_turbomachinery";
        let ext = "external_aerodynamics";
        let want: Vec<(&str, &str, &str)> = vec![
            ("axial-fan", "fan01-passage-upwind.toml", int),
            ("quick-start", "cavity-re100.toml", int),
            ("quick-start", "cavity-re400.toml", int),
            ("quick-start", "heated-cavity-ra1e3.toml", "heat_transfer"),
            ("quick-start", "heated-cavity-ra1e4.toml", "heat_transfer"),
            ("wind-tunnel", "cylinder-re100.toml", ext),
            ("wind-tunnel", "cylinder-re200.toml", ext),
            ("wind-tunnel", "naca0012-aoa2-rotated.toml", ext),
            ("wind-tunnel", "naca0012-aoa2.toml", ext),
            ("wind-tunnel", "naca0012-aoa4-rotated.toml", ext),
            ("wind-tunnel", "naca0012-aoa4.toml", ext),
            ("wind-tunnel", "step-re400.toml", int),
            ("wind-tunnel", "step-re800.toml", int),
        ];
        let mut failed = Vec::new();
        for (p, f, id) in &want {
            let c = cat_of_file(&ex.join(p).join("cases").join(f), Some(&ex.join(p)));
            eprintln!("R1 {p}/{f}: {} ({})", c.id, c.rule);
            if c.id != *id {
                failed.push(format!("{p}/{f}: {} for {id}", c.id));
            }
        }
        let n_example: usize = ["axial-fan", "quick-start", "wind-tunnel"]
            .iter()
            .map(|p| std::fs::read_dir(ex.join(p).join("cases")).unwrap().count())
            .sum();
        let cant = cat_of_file(
            &repo().join("crates/rcfd-api/tests/solid_cases/cantilever_hex8.toml"),
            None,
        );
        eprintln!("R1 cantilever_hex8: {} ({})", cant.id, cant.rule);
        if cant.id != "structural" {
            failed.push(format!("cantilever: {}", cant.id));
        }
        let pool = category_of_text(STILL_POOL, &std::env::temp_dir(), None).1;
        eprintln!("R1 still pool: {} ({})", pool.id, pool.rule);
        if pool.id != "multiphase" {
            failed.push(format!("still pool: {}", pool.id));
        }
        assert_eq!(
            n_example, 13,
            "the example case files are not the 13 of ADR 0817 amendment 1"
        );
        assert!(failed.is_empty(), "R1: {failed:?}");
    }

    /// ADR 0817 R2: the order of the rule and its texts, on 5 texts.
    #[test]
    fn r2_f977_the_order_and_the_texts_of_the_rule() {
        let ex = repo().join("example-projects");
        let cyl = std::fs::read_to_string(ex.join("wind-tunnel/cases/cylinder-re100.toml")).unwrap();
        let cav = std::fs::read_to_string(ex.join("quick-start/cases/cavity-re100.toml")).unwrap();
        let free = "velocity = \"freestream\"";
        let i = cyl.find("[boundaries.farfield]").unwrap();
        let j = i + cyl[i..].find(free).unwrap();
        let fixed_far = format!("{}velocity = \"fixed\"{}", &cyl[..j], &cyl[j + free.len()..]);
        let int = "internal_flow_and_turbomachinery";
        let cases: Vec<(&str, String, &str, &str)> = vec![
            (
                "cylinder with energy",
                format!("{cyl}\n[energy]\nenabled = true\n"),
                "heat_transfer",
                "energy.enabled = true",
            ),
            (
                "cavity with mrf",
                format!("{cav}\n[sources.spin]\ntype = \"mrf\"\n"),
                int,
                "sources.spin.type = mrf",
            ),
            (
                "no boundaries",
                "[mesh]\ngenerator = \"cavity_2d\"\n".into(),
                "general",
                "no boundaries are named",
            ),
            (
                "not a case",
                "not a case = [".into(),
                "general",
                "the case of the run does not read",
            ),
            (
                "far-field pressure",
                fixed_far,
                "external_aerodynamics",
                "boundaries.farfield.pressure = freestream",
            ),
        ];
        let mut failed = Vec::new();
        for (what, text, id, rule) in &cases {
            let c = category_of_text(text, &ex.join("wind-tunnel/cases"), Some(&ex.join("wind-tunnel"))).1;
            let ok = c.id == *id && c.rule.starts_with(rule);
            eprintln!(
                "R2 {what}: {} ({}) -> {}",
                c.id,
                c.rule,
                if ok { "as registered" } else { "WRONG" }
            );
            if !ok {
                failed.push(*what);
            }
        }
        assert_eq!(cases.len(), 5);
        assert!(failed.is_empty(), "R2: {failed:?}");
    }

    fn field(run: &str, f: &str) -> Value {
        json!({"kind": "field", "title": f, "run": run, "field": f})
    }
    fn monitor(run: &str, m: &str) -> Value {
        json!({"kind": "monitor", "title": m, "run": run, "monitor": m})
    }
    fn plot(run: &str) -> Value {
        json!({"kind": "plot", "title": "Residuals", "run": run})
    }
    fn table(run: &str) -> Value {
        json!({"kind": "table", "title": "Run", "runs": [run]})
    }
    fn mesh(case: &str) -> Value {
        json!({"kind": "mesh", "title": "Mesh", "case": case})
    }
    fn case_panel(case: &str) -> Value {
        json!({"kind": "case", "title": "Case", "case": case})
    }

    fn short(v: &Value) -> Vec<String> {
        v["panels"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|p| {
                let what = p
                    .get("field")
                    .or(p.get("monitor"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                format!("{}{{{what}}}", p["kind"].as_str().unwrap_or("?"))
            })
            .collect()
    }

    /// The layout of `run` in `ws` against the registered panels and title, with no error of the view.
    fn check_layout(ws: &Workspace, what: &str, run: &str, title: &str, panels: Vec<Value>) -> bool {
        let (v, c) = ws.layout_of_run(run, None, None).unwrap();
        let errors = ws.view_errors(&v);
        let want = json!({"version": 1, "title": title, "layout": {"columns": 2}, "panels": panels});
        let ok = v == want && errors.is_empty();
        eprintln!(
            "{what} {run}: {} ({}), {:?}, errors {errors:?} -> {}",
            c["category"],
            c["rule"],
            short(&v),
            if ok { "as registered" } else { "WRONG" }
        );
        if !ok {
            eprintln!("  got  {v}\n  want {want}");
        }
        ok
    }

    /// ADR 0817 L1: the layouts of the four committed example runs.
    #[test]
    fn l1_f977_the_layouts_of_the_example_runs() {
        let (dir, ws) = f977_project();
        let fan = "cases/fan01-passage-upwind.toml";
        let cav = "cases/cavity-re100.toml";
        let rows = [
            (
                "L1 external",
                D5CB,
                format!("External aerodynamics: cases/cylinder-re100.toml ({D5CB})"),
                vec![
                    field(D5CB, "ux"),
                    field(D5CB, "p"),
                    monitor(D5CB, "cd"),
                    monitor(D5CB, "cl"),
                    plot(D5CB),
                    table(D5CB),
                ],
            ),
            (
                "L1 turbomachinery",
                FAN,
                format!("Internal flow and turbomachinery: {fan} ({FAN})"),
                vec![
                    mesh(fan),
                    field(FAN, "ux"),
                    field(FAN, "p"),
                    monitor(FAN, "m_in"),
                    monitor(FAN, "m_out"),
                    monitor(FAN, "p_in"),
                    plot(FAN),
                ],
            ),
            (
                "L1 internal",
                F8C2,
                format!("Internal flow and turbomachinery: {cav} ({F8C2})"),
                vec![
                    mesh(cav),
                    field(F8C2, "ux"),
                    field(F8C2, "p"),
                    monitor(F8C2, "lid_drag"),
                    plot(F8C2),
                ],
            ),
            (
                "L1 heat",
                RUN,
                format!("Heat transfer: cases/heated-cavity-ra1e4.toml ({RUN})"),
                vec![
                    field(RUN, "T"),
                    field(RUN, "ux"),
                    monitor(RUN, "q_hot"),
                    monitor(RUN, "nu_hot"),
                    plot(RUN),
                    table(RUN),
                ],
            ),
        ];
        let mut failed = Vec::new();
        for (what, run, title, panels) in rows {
            if !check_layout(&ws, what, run, &title, panels) {
                failed.push(what);
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
        assert!(failed.is_empty(), "L1: {failed:?}");
    }

    /// ADR 0817 L2: the negative controls: a run without the record a panel needs, structural and general.
    #[test]
    fn l2_f977_a_missing_record_leaves_its_panel_out() {
        let (dir, ws) = f977_project();
        let runs = dir.join("runs");
        let no_fields = "r-20261126T000000-a001";
        let no_res = "r-20261126T000000-a002";
        let general = "r-20261126T000000-a003";
        copy_dir(&runs.join(RUN), &runs.join(no_fields));
        std::fs::remove_dir_all(runs.join(no_fields).join("fields")).unwrap();
        copy_dir(&runs.join(RUN), &runs.join(no_res));
        std::fs::remove_file(runs.join(no_res).join("residuals.jsonl")).unwrap();
        copy_dir(&runs.join(F8C2), &runs.join(general));
        std::fs::write(
            runs.join(general).join("case.snapshot"),
            "[mesh]\ngenerator = \"cavity_2d\"\n",
        )
        .unwrap();
        let cant = repo().join("crates/rcfd-api/tests/solid_cases/cantilever_hex8.toml");
        std::fs::copy(&cant, dir.join("cases/cantilever.toml")).unwrap();
        std::fs::create_dir_all(runs.join(STRUCT)).unwrap();
        std::fs::copy(&cant, runs.join(STRUCT).join("case.snapshot")).unwrap();
        let record = json!({"id": STRUCT, "case": "cases/cantilever.toml", "created": "2026-11-26T00:00:00Z"});
        std::fs::write(runs.join(STRUCT).join("run.json"), record.to_string()).unwrap();
        let heat = "cases/heated-cavity-ra1e4.toml";
        let cav = "cases/cavity-re100.toml";
        let mut failed = Vec::new();
        let mut row = |what: &'static str, run: &str, title: String, panels: Vec<Value>| {
            if !check_layout(&ws, what, run, &title, panels) {
                failed.push(what);
            }
        };
        row(
            "L2a no fields",
            no_fields,
            format!("Heat transfer: {heat} ({no_fields})"),
            vec![
                monitor(no_fields, "q_hot"),
                monitor(no_fields, "nu_hot"),
                plot(no_fields),
                table(no_fields),
            ],
        );
        row(
            "L2b no residuals",
            no_res,
            format!("Heat transfer: {heat} ({no_res})"),
            vec![field(no_res, "T"), field(no_res, "ux"), table(no_res)],
        );
        row(
            "L2d structural",
            STRUCT,
            format!("Structural: cases/cantilever.toml ({STRUCT})"),
            vec![case_panel("cases/cantilever.toml"), table(STRUCT)],
        );
        row(
            "L2e general",
            general,
            format!("General: {cav} ({general})"),
            vec![case_panel(cav), plot(general), table(general)],
        );
        std::fs::remove_file(dir.join(cav)).unwrap();
        row(
            "L2c no case file",
            F8C2,
            format!("Internal flow and turbomachinery: {cav} ({F8C2})"),
            vec![
                field(F8C2, "ux"),
                field(F8C2, "p"),
                monitor(F8C2, "lid_drag"),
                plot(F8C2),
            ],
        );
        let _ = std::fs::remove_dir_all(&dir);
        assert!(failed.is_empty(), "L2: {failed:?}");
    }

    /// ADR 0817 V5: the view of a run is valid, with or without a title and a layout.
    #[test]
    fn v5_f977_the_view_of_a_run_is_valid() {
        let (dir, ws) = quick_start();
        let a = ws.view_errors(&json!({"version": 1, "run": RUN}));
        let b = ws.view_errors(&json!({"version": 1, "run": RUN, "title": "x", "layout": {"columns": 3}}));
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(a, vec![], "the view of a run is refused");
        assert_eq!(b, vec![], "the view of a run with a title and a layout is refused");
    }

    /// ADR 0817 V6: 4 invalid views of a run, each refused at its key.
    #[test]
    fn v6_f977_invalid_views_of_a_run_are_refused_at_their_key() {
        let (dir, ws) = quick_start();
        let cases: Vec<(&str, Value, &str, &str)> = vec![
            (
                "run and panels",
                json!({"version": 1, "run": RUN, "panels": [{"kind": "plot", "run": RUN}]}),
                "panels",
                "not both",
            ),
            ("run 42", json!({"version": 1, "run": 42}), "run", "required"),
            (
                "no such run",
                json!({"version": 1, "run": "r-20991231T000000-0000"}),
                "run",
                "no such run",
            ),
            (
                "key colour",
                json!({"version": 1, "run": RUN, "colour": "red"}),
                "colour",
                "not a key",
            ),
        ];
        let mut failed = Vec::new();
        for (what, v, key, word) in &cases {
            let errors = ws.view_errors(v);
            let ok = errors.len() == 1 && errors[0].key == *key && errors[0].reason.contains(word);
            eprintln!(
                "V6 {what}: {errors:?} -> {}",
                if ok { "refused at its key" } else { "WRONG" }
            );
            if !ok {
                failed.push(*what);
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(cases.len(), 4);
        assert!(failed.is_empty(), "not refused at the registered key: {failed:?}");
    }

    /// ADR 0823 R3 (unit): the page of X1 (ADR 0805 V2's view) refers to nothing outside it, carries the CSP and the
    /// Studio's stylesheets.
    #[test]
    fn r3_f980_the_page_of_x1_is_self_contained() {
        let (dir, ws) = quick_start();
        let r = ws.export_view(&json!({"view": valid()})).unwrap();
        let page = std::fs::read_to_string(dir.join(r["path"].as_str().unwrap())).unwrap();
        let refs = page_references(&page);
        eprintln!("R3 unit: {} bytes, references {refs:?}", page.len());
        let _ = std::fs::remove_dir_all(&dir);
        assert!(refs.is_empty(), "{refs:?}");
        assert!(page.contains(&format!(
            "<meta http-equiv=\"Content-Security-Policy\" content=\"{EXPORT_CSP}\">"
        )));
        for (name, css) in EXPORT_STYLES {
            assert!(page.contains(css), "{name} is not in the page");
        }
        // the control: a page with a reference is found out
        assert_eq!(page_references("<link rel=\"stylesheet\" href=\"https://x\">").len(), 3);
    }

    /// ADR 0823 amendment 4 U1: `num` writes 20 numbers as `String(x)` does in Node v22.19.0 (the Studio's runtime),
    /// whose texts were printed once after the amendment's commit and before this code.
    #[test]
    fn u1_f980_num_writes_a_number_as_the_studio_does() {
        let want: [(f64, &str); 20] = [
            (278.0, "278"),
            (0.01, "0.01"),
            (-152.87271118164063, "-152.87271118164062"),
            (1078.4739990234375, "1078.4739990234375"),
            (1.0, "1"),
            (0.0, "0"),
            (-0.0, "0"),
            (2400.0, "2400"),
            (1e-7, "1e-7"),
            (1.5e-7, "1.5e-7"),
            (1e-6, "0.000001"),
            (2.5e-6, "0.0000025"),
            (1e21, "1e+21"),
            (1.2345678901234568e20, "123456789012345680000"),
            (123.456, "123.456"),
            (-0.5, "-0.5"),
            (5e-324, "5e-324"),
            (1.7976931348623157e308, "1.7976931348623157e+308"),
            (100.0, "100"),
            (0.1 + 0.2, "0.30000000000000004"),
        ];
        let mut equal = 0;
        for (x, s) in want {
            let got = num(x);
            eprintln!("U1 {x:e}: num {got}, String {s}{}", if got == s { "" } else { " MISS" });
            equal += usize::from(got == s);
        }
        eprintln!("U1: {equal} of 20 equal");
        assert_eq!(equal, 20);
        // js_json: JSON, each f64 by num
        assert_eq!(
            js_json(&json!({"a": [1.0, 2u64, "x"], "b": 0.01})),
            "{\"a\":[1,2,\"x\"],\"b\":0.01}"
        );
    }

    /// G22 Layers 1-3: exported page has zero standing prose or machine strings in the visible glance layer, and the
    /// linter catches each banned class in negative controls.
    #[test]
    fn g22_f980_exported_page_obeys_quiet_invariants() {
        let (dir, ws) = quick_start();
        let r = ws.export_view(&json!({"view": valid()})).unwrap();
        let page = std::fs::read_to_string(dir.join(r["path"].as_str().unwrap())).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        let bad = page_quiet_violations(&page);
        assert!(bad.is_empty(), "G22 quiet violations on exported page: {bad:?}");
        // Negative controls: each banned pattern must be caught when visible outside <details>
        assert!(!page_quiet_violations("<main><p>A picture of the view.</p></main>").is_empty());
        assert!(!page_quiet_violations("<main><span>fnv64:c0ec92f5</span></main>").is_empty());
        assert!(!page_quiet_violations("<main><span>-0.22311171889305115</span></main>").is_empty());
        assert!(!page_quiet_violations("<main><span>2026-11-27T11:51:50Z</span></main>").is_empty());
        // Collapsed <details> is the audit layer and is allowed
        assert!(page_quiet_violations(
            "<main><details><summary>Provenance</summary><p>fnv64:abc -0.223111718893.</p></details></main>"
        )
        .is_empty());
    }
}
