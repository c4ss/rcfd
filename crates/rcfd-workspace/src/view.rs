//! Claude's canvas (F-976, ADR 0805): the view specification `rcfd-view` version 1, its validation with located errors
//! (D1), the commands `show_view` and `read_view` (D2), and the registry of the open Studios of a project (D3).
//!
//! A specification names what to show (`case`, `run`, `field`, `mesh`, `plot`, `monitor`, `table`), of which case or
//! run, and the layout. `show_view` checks it against the project, counts the project's live canvases, and writes it to
//! `.rcfd/views/current.json` with one journal line `view_shown`; every open Studio reads that line from the change
//! feed (ADR 0188) and the view with `read_view`. Nothing is written for a refused specification or when no Studio is
//! open.

use crate::{err, journal, project_error, RpcError, Workspace, BAD_STATE, INTERNAL, VALIDATION};
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
        extra_keys(o, &["version", "title", "layout", "panels"], "", &mut errors);
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
        match o.get("panels") {
            None => e(&mut errors, "panels", "is required: an array of 1 to 8 panels"),
            Some(Value::Array(p)) if p.is_empty() || p.len() > MAX_PANELS => e(
                &mut errors,
                "panels",
                format!("holds {} panels; a view has 1 to {MAX_PANELS}", p.len()),
            ),
            Some(Value::Array(p)) => {
                for (i, panel) in p.iter().enumerate() {
                    self.panel_errors(panel, &format!("panels[{i}]"), &mut errors);
                }
            }
            Some(_) => e(&mut errors, "panels", "is an array of panels"),
        }
        errors
    }

    fn panel_errors(&self, panel: &Value, at: &str, errors: &mut Vec<ViewError>) {
        let Some(o) = panel.as_object() else {
            e(errors, at, "a panel is a JSON object with a kind");
            return;
        };
        let kind = match o.get("kind").and_then(Value::as_str) {
            Some(k) if KINDS.contains(&k) => k,
            Some(k) => {
                e(
                    errors,
                    format!("{at}.kind"),
                    format!("'{k}' is not a kind (one of {})", KINDS.join(", ")),
                );
                return;
            }
            None => {
                e(
                    errors,
                    format!("{at}.kind"),
                    format!("is required, one of {}", KINDS.join(", ")),
                );
                return;
            }
        };
        let keys: &[&str] = match kind {
            "case" => &["kind", "title", "case"],
            "mesh" => &["kind", "title", "mesh"],
            "run" | "plot" => &["kind", "title", "run"],
            "field" => &["kind", "title", "run", "field", "snapshot"],
            "monitor" => &["kind", "title", "run", "monitor"],
            _ => &["kind", "title", "runs"],
        };
        extra_keys(o, keys, at, errors);
        title(o, at, errors);
        match kind {
            "case" => self.case_ref(o.get("case"), &format!("{at}.case"), errors),
            "mesh" => self.mesh_ref(o.get("mesh"), &format!("{at}.mesh"), errors),
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
        Ok(json!({"shown": true, "id": hash, "path": CURRENT, "hash": hash, "studios": studios, "panels": kinds.len()}))
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
        Ok(
            json!({"exists": true, "path": CURRENT, "id": hash, "hash": hash, "shown": v["shown"],
            "actor": v.get("actor").cloned().unwrap_or(Value::Null), "reason": v.get("reason").cloned().unwrap_or(Value::Null),
            "view": v["view"]}),
        )
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
}
