//! Case editor model (ADR 0131 B5, B6): a case file of the project opened in an in-process `rcfd_api` session, edited through
//! the schema tree, saved with the base-hash rule.
//!
//! The editor never touches files itself: it is built from the bytes a worker loaded, gives the worker the bytes to write, and
//! is told the outcome. [`save_if_base`] is the compare-then-write the worker runs (ADR 0131 amendment 1 item 2; the
//! cooperative lock of ADR 0135 replaces it when 48F merges). Messages that refuse an edit name the setting's path.

use crate::schema_model::{build_tree, parse_input, Node};
use rcfd_api::case::Case;
use rcfd_api::service::{RpcError, Service};
use rcfd_project::records::hash_bytes;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The editor's relation to the file on disk.
#[derive(Debug, Clone, PartialEq)]
pub enum EditorState {
    /// Settings equal the file as loaded or last saved.
    Clean,
    /// Edited and not saved.
    Dirty,
    /// The file changed on disk while the editor was dirty (or a save found another hash). The edited settings are kept;
    /// `disk` holds the file's current bytes (`None`: the file is gone).
    Conflict {
        /// Current bytes on disk.
        disk: Option<Vec<u8>>,
    },
}

/// What [`CaseEditor::external_change`] did.
#[derive(Debug, Clone, PartialEq)]
pub enum External {
    /// The bytes are the ones the editor already has (for example its own save).
    Same,
    /// The editor was clean and reloaded the new content.
    Reloaded,
    /// The editor was dirty (or the new content does not open): it is now in the conflict state.
    Conflict,
}

/// Outcome of [`save_if_base`].
#[derive(Debug, Clone, PartialEq)]
pub enum SaveOutcome {
    /// Written; the new base hash.
    Saved(String),
    /// The file's hash differs from the base: nothing written. The current bytes (`None`: no file).
    Conflict(Option<Vec<u8>>),
}

/// Replace `path` with `bytes` only if its current content hashes to `base` (`hash_bytes` form), or, with an empty `base`,
/// only if there is no file. Written atomically.
pub fn save_if_base(path: &Path, base: &str, bytes: &[u8]) -> Result<SaveOutcome, String> {
    let disk = match std::fs::read(path) {
        Ok(b) => Some(b),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    match &disk {
        Some(d) if hash_bytes(d) == base => {}
        None if base.is_empty() => {}
        _ => return Ok(SaveOutcome::Conflict(disk)),
    }
    rcfd_project::atomic::atomic_write(path, bytes).map_err(|e| e.to_string())?;
    Ok(SaveOutcome::Saved(hash_bytes(bytes)))
}

/// The settings schema tree of the linked solver (`settings.schema`).
pub fn schema_tree(svc: &Arc<Service>) -> Node {
    let doc = svc.call("settings.schema", &json!({})).unwrap_or(json!({}));
    build_tree(&doc["case"])
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

/// One open case file.
pub struct CaseEditor {
    svc: Arc<Service>,
    session: String,
    /// Project-relative path of the file (`cases/a.toml`).
    pub rel: String,
    /// Current settings tree (from the service).
    pub settings: Value,
    saved: Value,
    /// Hash of the bytes the settings were loaded from or last saved as.
    pub base_hash: String,
    conflict: Option<Option<Vec<u8>>>,
    /// Per-setting refusal text, `<path>: <reason>`.
    pub field_errors: BTreeMap<String, String>,
    /// Problems from the last validation.
    pub diagnostics: Vec<String>,
    /// A save was handed to the worker and its answer has not come back.
    pub saving: bool,
    /// The field being typed in and not committed yet, `(path, text)` (ADR 0144): the case preview builds the settings
    /// with this text in place. A commit of that field ends it; a refused commit keeps it (the field keeps the text).
    pub draft: Option<(String, String)>,
    revision: u64,
    /// The case's folder: relative data paths (mesh file, lookup tables) resolve against it in the session, its
    /// validation and the bytes a save writes (ADR 0144, the maintainer's answer to ADR 0131 question 3). `None`: the
    /// working folder, as before ([`CaseEditor::from_bytes`]).
    pub dir: Option<PathBuf>,
}

impl Drop for CaseEditor {
    fn drop(&mut self) {
        let _ = self.svc.call("session.close", &json!({"session": self.session}));
    }
}

fn is_json(rel: &str) -> bool {
    Path::new(rel)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("json"))
}

/// A session of the case text; relative data paths resolve against `dir` (the case's folder, ADR 0144), else against
/// the working folder (the service's rule for text).
fn create_session(svc: &Arc<Service>, rel: &str, dir: Option<&Path>, bytes: &[u8]) -> Result<String, String> {
    let text = std::str::from_utf8(bytes).map_err(|e| format!("{rel}: not UTF-8 text: {e}"))?;
    let mut p = if is_json(rel) {
        let v: Value = serde_json::from_str(text).map_err(|e| format!("{rel}: {e}"))?;
        json!({ "settings": v })
    } else {
        json!({ "toml": text })
    };
    if let Some(d) = dir {
        p["base_dir"] = json!(d.to_string_lossy());
    }
    let v = svc
        .call("session.create", &p)
        .map_err(|e| format!("{rel}: {}", rpc_msg(&e)))?;
    Ok(v["session"].as_str().unwrap_or_default().to_string())
}

impl CaseEditor {
    /// Open `rel` from the bytes a worker read, relative data paths against the working folder (the route before
    /// ADR 0144; the Studio uses [`CaseEditor::from_bytes_in`]). Fails with a located message when the case does not
    /// parse or validate.
    pub fn from_bytes(svc: Arc<Service>, rel: &str, bytes: &[u8]) -> Result<CaseEditor, String> {
        Self::open(svc, rel, None, bytes)
    }

    /// Open `rel` from the bytes a worker read, relative data paths against the case's folder `dir` (as `rcfd run` and
    /// the runner resolve them; ADR 0144).
    pub fn from_bytes_in(svc: Arc<Service>, rel: &str, dir: &Path, bytes: &[u8]) -> Result<CaseEditor, String> {
        Self::open(svc, rel, Some(dir.to_path_buf()), bytes)
    }

    fn open(svc: Arc<Service>, rel: &str, dir: Option<PathBuf>, bytes: &[u8]) -> Result<CaseEditor, String> {
        let session = create_session(&svc, rel, dir.as_deref(), bytes)?;
        let mut ed = CaseEditor {
            svc,
            session,
            rel: rel.to_string(),
            settings: Value::Null,
            saved: Value::Null,
            base_hash: hash_bytes(bytes),
            conflict: None,
            field_errors: BTreeMap::new(),
            diagnostics: Vec::new(),
            saving: false,
            draft: None,
            revision: 0,
            dir,
        };
        ed.refresh();
        ed.saved = ed.settings.clone();
        Ok(ed)
    }

    /// A number that changes whenever the settings or the draft change (ADR 0144: the preview builds again after it).
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// The text of field `path` as it is being typed (its whole text, on every change; not committed, the settings are
    /// unchanged). The case preview builds the settings with this text in place (ADR 0144).
    pub fn type_text(&mut self, path: &str, text: &str) {
        let d = (path.to_string(), text.to_string());
        if self.draft.as_ref() != Some(&d) {
            self.draft = Some(d);
            self.revision += 1;
        }
    }

    /// Typing in `path` ended without a change to commit (the field holds the stored value's text again).
    pub fn end_draft(&mut self, path: &str) {
        if self.draft.as_ref().is_some_and(|(p, _)| p == path) {
            self.draft = None;
            self.revision += 1;
        }
    }

    fn call(&self, method: &str, mut p: Value) -> Result<Value, RpcError> {
        p["session"] = json!(self.session);
        self.svc.call(method, &p)
    }

    fn refresh(&mut self) {
        if let Ok(v) = self.call("settings.get", json!({})) {
            let v = v["value"].clone();
            if v != self.settings {
                self.settings = v;
                self.revision += 1;
            }
        }
    }

    /// Clean, dirty or in conflict.
    pub fn state(&self) -> EditorState {
        match &self.conflict {
            Some(disk) => EditorState::Conflict { disk: disk.clone() },
            None if self.settings != self.saved => EditorState::Dirty,
            None => EditorState::Clean,
        }
    }

    /// Settings differ from the last load or save.
    pub fn is_dirty(&self) -> bool {
        self.settings != self.saved
    }

    /// In the conflict state.
    pub fn in_conflict(&self) -> bool {
        self.conflict.is_some()
    }

    /// Edit a setting from user text: parsed and range-checked locally against the schema, then set through the service.
    /// A refusal is returned and kept in [`CaseEditor::field_errors`] as `<path>: <reason>`; the settings are unchanged.
    pub fn edit_text(&mut self, schema: &Node, path: &str, text: &str) -> Result<(), String> {
        let Some(node) = schema.find(path) else {
            return Err(self.refuse(path, "unknown setting".into()));
        };
        match parse_input(&node.kind, text) {
            Ok(v) => {
                self.set_value(path, v)?;
                self.end_draft(path); // committed: the field's text is the setting now (ADR 0144)
                Ok(())
            }
            Err(m) => Err(self.refuse(path, m)),
        }
    }

    fn refuse(&mut self, path: &str, reason: String) -> String {
        let m = format!("{path}: {reason}");
        self.field_errors.insert(path.to_string(), m.clone());
        m
    }

    /// Set a setting to a JSON value through the service (`settings.set`).
    pub fn set_value(&mut self, path: &str, value: Value) -> Result<(), String> {
        match self.call("settings.set", json!({"path": path, "value": value})) {
            Ok(_) => {
                self.field_errors.remove(path);
                self.refresh();
                Ok(())
            }
            Err(e) => {
                let m = rpc_msg(&e);
                // The service usually names the path itself; keep one copy of it.
                let reason = m.strip_prefix(&format!("{path}: ")).unwrap_or(&m).to_string();
                Err(self.refuse(path, reason))
            }
        }
    }

    /// Add a named entry (a patch, a report) with the default of its first field.
    pub fn add_named(&mut self, schema: &Node, collection: &str, name: &str) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() || name.contains('.') || name.contains(char::is_whitespace) {
            return Err(self.refuse(
                collection,
                "names must be non-empty and contain no dots or spaces".into(),
            ));
        }
        let first = schema
            .find(collection)
            .and_then(|c| c.children.first())
            .and_then(|item| item.children.first())
            .cloned();
        let Some(first) = first else {
            return Err(self.refuse(collection, "not a named collection".into()));
        };
        self.set_value(&format!("{collection}.{name}.{}", first.name), first.default.clone())
    }

    /// Remove a named entry.
    pub fn remove_named(&mut self, path: &str) -> Result<(), String> {
        match self.call("settings.remove", json!({ "path": path })) {
            Ok(_) => {
                self.refresh();
                Ok(())
            }
            Err(e) => Err(self.refuse(path, rpc_msg(&e))),
        }
    }

    /// Validate with the service; the diagnostics are kept. True when valid.
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

    /// The bytes a save writes: the edited case as TOML (or JSON for a `.json` file).
    pub fn to_bytes(&mut self) -> Result<Vec<u8>, String> {
        self.refresh();
        let dir = self.dir.clone().unwrap_or_else(|| PathBuf::from("."));
        let case = Case::from_json_in(&self.settings, &dir)
            .map_err(|d| d.iter().map(|x| x.to_string()).collect::<Vec<_>>().join("; "))?;
        Ok(if is_json(&self.rel) {
            case.to_json_string()
        } else {
            case.to_toml()
        }
        .into_bytes())
    }

    /// The worker wrote the file; `hash` is the new base.
    pub fn saved(&mut self, hash: String) {
        self.saving = false;
        self.saved = self.settings.clone();
        self.base_hash = hash;
        self.conflict = None;
    }

    /// The worker refused the save: the file has another hash.
    pub fn save_conflict(&mut self, disk: Option<Vec<u8>>) {
        self.saving = false;
        self.conflict = Some(disk);
    }

    /// The file changed on disk (`None`: removed). Same bytes are ignored; a clean editor reloads; a dirty one, or content
    /// that does not open, enters the conflict state with the edited settings kept.
    pub fn external_change(&mut self, disk: Option<Vec<u8>>) -> External {
        if disk.as_deref().map(hash_bytes).as_deref() == Some(self.base_hash.as_str()) {
            return External::Same;
        }
        if self.conflict.is_none() && !self.is_dirty() {
            if let Some(bytes) = &disk {
                if self.reload(bytes).is_ok() {
                    return External::Reloaded;
                }
            }
        }
        self.conflict = Some(disk);
        External::Conflict
    }

    fn reload(&mut self, bytes: &[u8]) -> Result<(), String> {
        let session = create_session(&self.svc, &self.rel, self.dir.as_deref(), bytes)?;
        let old = std::mem::replace(&mut self.session, session);
        let _ = self.svc.call("session.close", &json!({ "session": old }));
        self.base_hash = hash_bytes(bytes);
        self.conflict = None;
        self.field_errors.clear();
        self.diagnostics.clear();
        self.draft = None;
        self.revision += 1;
        self.refresh();
        self.saved = self.settings.clone();
        Ok(())
    }

    /// Resolve a conflict by taking the version on disk (the edits are dropped).
    pub fn take_disk(&mut self) -> Result<(), String> {
        match self.conflict.clone() {
            Some(Some(bytes)) => self.reload(&bytes),
            Some(None) => Err(format!("{}: the file was removed; save to write it again", self.rel)),
            None => Ok(()),
        }
    }

    /// Resolve a conflict by keeping the edits: the disk version becomes the base, so the next save replaces it.
    pub fn keep_mine(&mut self) {
        if let Some(disk) = self.conflict.take() {
            self.base_hash = disk.as_deref().map(hash_bytes).unwrap_or_default();
            // Dirty against the disk version, whatever the last save was.
            self.saved = Value::Null;
        }
    }
}
