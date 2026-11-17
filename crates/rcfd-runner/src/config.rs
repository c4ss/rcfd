//! The runner writes its own budgets file (ADR 0733, F-837): [`Config::state`] reads `.rcfd/runner-config.json` with the
//! values the runner last started with, and [`Config::set`] changes keys of it with the runner's own checks, written by
//! `rcfd_project::atomic::replace_if_hash_with` under the file's edit lock, with `fnv64:` hashes and one journal line in
//! `.rcfd/edits.jsonl` per accepted write.
//!
//! The runner reads the file only when `serve` starts, so a change applies at the next runner start (`applies:
//! "next_start"`); nothing here restarts it. `Config`, `Config::default`, `Config::load` and `serve` are unchanged: a
//! file written here is read by the same `load`.

use crate::runner::{config_path, dot_dir, Config};
use rcfd_project::atomic;
use rcfd_project::records::{hash_bytes, hash_matches};
use rcfd_project::ProjectError;
use serde_json::{json, Map, Value};
use std::path::Path;

/// The keys of [`Config`], in its order (the order of the file this module writes).
pub const CONFIG_KEYS: [&str; 8] = [
    "max_runs",
    "cpu_threads",
    "mem_budget_mb",
    "mem_base_mb",
    "mem_kb_per_cell",
    "poll_ms",
    "adopt_probe_ms",
    "cancel_grace_ms",
];

/// The budgets file, project-relative.
pub const CONFIG_FILE: &str = ".rcfd/runner-config.json";

/// When a written file takes effect: the runner reads it when it starts (ADR 0733 Q5 (a)).
pub const APPLIES: &str = "next_start";

/// The base of a [`Config::set`]: what the file must hold for the write to go ahead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Base<'a> {
    /// Whatever is there now (the CLI without `--base`); still compared again under the edit lock.
    Any,
    /// There must be no file yet.
    Absent,
    /// The file must have this hash (`fnv64:`, or the legacy `fnv1a64:` spelling).
    Hash(&'a str),
}

/// How a [`Config::set`] is made and journalled.
#[derive(Debug, Clone, Default)]
pub struct SetOptions {
    /// Write exactly the keys of the changes, whatever the file holds now (ADR 0733 Q4 (a)): every key of [`CONFIG_KEYS`]
    /// must be given, a number or null. Needed when the runner cannot read the file as it is.
    pub replace: bool,
    /// The command named in the journal line (`who`).
    pub who: String,
    /// More fields of the journal line (the actor and the reason, ADR 0188 J1).
    pub extra: Map<String, Value>,
}

/// Why a [`Config::set`] wrote nothing.
#[derive(Debug, Clone, PartialEq)]
pub enum ConfigError {
    /// A key or value the runner would refuse, or a file it cannot read changed without `replace`: the runner's message,
    /// located at the key when there is one.
    Refused {
        /// The key the refusal is about.
        key: Option<String>,
        /// The runner's message.
        message: String,
    },
    /// The file does not have the base: its hash now (`None`: there is no file).
    Conflict {
        /// The file's hash now.
        current_hash: Option<String>,
    },
    /// Another cooperating writer held the file's edit lock.
    LockBusy(String),
    /// The file or the journal could not be read or written.
    Io(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Refused { message, .. } => write!(f, "{CONFIG_FILE}: {message}; nothing was written"),
            ConfigError::Conflict { current_hash } => write!(
                f,
                "{CONFIG_FILE}: changed since it was read (now {}); read it again",
                current_hash.as_deref().unwrap_or("no file")
            ),
            ConfigError::LockBusy(m) | ConfigError::Io(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for ConfigError {}

fn refused(key: Option<&str>, message: impl Into<String>) -> ConfigError {
    ConfigError::Refused {
        key: key.map(String::from),
        message: message.into(),
    }
}

/// The file as read: its bytes (`None`: no file), its object of keys when it is one of known keys, and why the runner
/// cannot read its shape otherwise.
struct Current {
    bytes: Option<Vec<u8>>,
    map: Map<String, Value>,
    shape_problem: Option<String>,
}

fn read_current(project: &Path) -> Result<Current, ConfigError> {
    let p = config_path(project);
    let bytes = match std::fs::read(&p) {
        Ok(b) => Some(b),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(ConfigError::Io(format!("{}: {e}", p.display()))),
    };
    let (map, shape_problem) = match &bytes {
        None => (Map::new(), None),
        Some(b) => match serde_json::from_slice::<Value>(b) {
            Ok(Value::Object(m)) => match m.keys().find(|k| !CONFIG_KEYS.contains(&k.as_str())) {
                Some(k) => (
                    Map::new(),
                    Some(format!(
                        "unknown field `{k}`, expected one of {}",
                        CONFIG_KEYS.map(|k| format!("`{k}`")).join(", ")
                    )),
                ),
                None => (m, None),
            },
            Ok(_) => (Map::new(), Some("not a JSON object".to_string())),
            Err(e) => (Map::new(), Some(format!("not JSON ({e})"))),
        },
    };
    Ok(Current {
        bytes,
        map,
        shape_problem,
    })
}

impl Config {
    /// The checks of [`Config::load`] after the parse: the message without the file's name, or `Ok`.
    pub fn check(&self) -> Result<(), String> {
        if self.max_runs == 0 || self.cpu_threads == 0 || self.poll_ms == 0 {
            return Err("max_runs, cpu_threads and poll_ms must be positive".into());
        }
        Ok(())
    }

    /// The runner's checks of a file's object of keys: the parse of [`Config`] (`deny_unknown_fields`, the types of its
    /// fields) and [`Config::check`]. A refusal is located at the key it is about.
    fn check_map(map: &Map<String, Value>) -> Result<Config, ConfigError> {
        let c: Config = match serde_json::from_value(Value::Object(map.clone())) {
            Ok(c) => c,
            Err(e) => {
                // the key: the first one the parse refuses alone
                let key = map.iter().find_map(|(k, v)| {
                    let one: Map<String, Value> = [(k.clone(), v.clone())].into_iter().collect();
                    serde_json::from_value::<Config>(Value::Object(one))
                        .is_err()
                        .then_some(k.as_str())
                });
                return Err(refused(key, e.to_string()));
            }
        };
        if let Err(m) = c.check() {
            let key = [
                ("max_runs", c.max_runs as u64),
                ("cpu_threads", c.cpu_threads as u64),
                ("poll_ms", c.poll_ms),
            ]
            .into_iter()
            .find(|(_, v)| *v == 0)
            .map(|(k, _)| k);
            return Err(refused(key, m));
        }
        Ok(c)
    }

    /// The budgets of the project as they are: the file (its hash, its values, why the runner would refuse it), the
    /// defaults of this machine and what the runner running now started with (`.rcfd/runner.json`). Writes nothing.
    pub fn state(project: &Path) -> Result<Value, ConfigError> {
        let cur = read_current(project)?;
        Ok(state_of(project, &cur))
    }

    /// Change keys of the project's budgets file: each value a number (set) or null (removed: the runner's default).
    /// The result is checked as [`Config::load`] checks a file; it is written only if the file still has `base`
    /// (atomically, under its edit lock, one journal line with the changed keys' old and new values and `opts.extra`).
    /// A file the runner cannot read (not a JSON object, an unknown key) is refused unless `opts.replace`.
    pub fn set(
        project: &Path,
        changes: &Map<String, Value>,
        base: Base<'_>,
        opts: SetOptions,
    ) -> Result<Value, ConfigError> {
        if changes.is_empty() {
            return Err(refused(None, "give at least one key to set or unset"));
        }
        for (k, v) in changes {
            if !CONFIG_KEYS.contains(&k.as_str()) {
                return Err(refused(
                    Some(k),
                    format!(
                        "unknown field `{k}`, expected one of {}",
                        CONFIG_KEYS.map(|k| format!("`{k}`")).join(", ")
                    ),
                ));
            }
            if !(v.is_number() || v.is_null()) {
                return Err(refused(
                    Some(k),
                    format!("{k} must be a number, or null for the runner's default"),
                ));
            }
        }
        let cur = read_current(project)?;
        let current_hash = cur.bytes.as_deref().map(hash_bytes);
        let base_hash: Option<String> = match base {
            Base::Any => current_hash.clone(),
            Base::Absent => None,
            Base::Hash(h) => Some(h.to_string()),
        };
        let matches = match (&base_hash, &cur.bytes) {
            (None, None) => true,
            (Some(h), Some(b)) => hash_matches(h, b),
            _ => false,
        };
        if !matches {
            return Err(ConfigError::Conflict { current_hash });
        }
        let mut next = if opts.replace {
            if let Some(k) = CONFIG_KEYS.iter().find(|k| !changes.contains_key(**k)) {
                return Err(refused(
                    Some(k),
                    format!("replace gives every key, a number or null; {k} is missing"),
                ));
            }
            Map::new()
        } else {
            if let Some(p) = &cur.shape_problem {
                return Err(refused(
                    None,
                    format!("the runner cannot read the file as it is ({p}); replace it whole: every key, with replace (CLI --replace)"),
                ));
            }
            cur.map.clone()
        };
        for (k, v) in changes {
            if v.is_null() {
                next.remove(k);
            } else {
                next.insert(k.clone(), v.clone());
            }
        }
        Config::check_map(&next)?;
        let bytes = file_text(&next);
        let mut line = opts.extra;
        let list: Vec<Value> = changes
            .keys()
            .map(|k| json!({"path": k, "old": cur.map.get(k).cloned().unwrap_or(Value::Null), "new": next.get(k).cloned().unwrap_or(Value::Null)}))
            .collect();
        line.insert("changes".into(), Value::Array(list));
        let who = if opts.who.is_empty() {
            "rcfd-runner config set"
        } else {
            opts.who.as_str()
        };
        atomic::replace_if_hash_with(project, CONFIG_FILE, base_hash.as_deref(), bytes.as_bytes(), who, line).map_err(
            |e| match e {
                ProjectError::Conflict { current, .. } => ConfigError::Conflict { current_hash: current },
                e @ ProjectError::LockBusy { .. } => ConfigError::LockBusy(e.to_string()),
                e => ConfigError::Io(e.to_string()),
            },
        )?;
        Config::state(project)
    }
}

/// The text of a budgets file: the keys in [`CONFIG_KEYS`] order, two-space indented, a final newline (`{}` when empty).
fn file_text(map: &Map<String, Value>) -> String {
    let lines: Vec<String> = CONFIG_KEYS
        .iter()
        .filter_map(|k| map.get(*k).map(|v| format!("  \"{k}\": {v}")))
        .collect();
    if lines.is_empty() {
        "{}\n".into()
    } else {
        format!("{{\n{}\n}}\n", lines.join(",\n"))
    }
}

/// `{path, exists, hash, values, problem, needs_replace, defaults, in_use, applies}`.
fn state_of(project: &Path, cur: &Current) -> Value {
    let problem = match (&cur.bytes, &cur.shape_problem) {
        (None, _) => None,
        (_, Some(p)) => Some(p.clone()),
        (Some(_), None) => Config::check_map(&cur.map).err().map(|e| match e {
            ConfigError::Refused { message, .. } => message,
            e => e.to_string(),
        }),
    };
    let in_use = std::fs::read(dot_dir(project).join("runner.json"))
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .filter(|r| r.get("config").is_some_and(Value::is_object))
        .map(|r| json!({"values": r["config"], "source": r.get("config_source").cloned().unwrap_or(Value::Null), "started": r.get("started").cloned().unwrap_or(Value::Null)}));
    json!({
        "path": CONFIG_FILE,
        "exists": cur.bytes.is_some(),
        "hash": cur.bytes.as_deref().map(hash_bytes),
        "values": Value::Object(cur.map.clone()),
        "problem": problem,
        "needs_replace": cur.shape_problem.is_some(),
        "defaults": serde_json::to_value(Config::default()).unwrap_or(Value::Null),
        "in_use": in_use,
        "applies": APPLIES,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_the_fields_of_config_in_order() {
        let v = serde_json::to_value(Config::default()).unwrap();
        let keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
        let mut want = CONFIG_KEYS.to_vec();
        want.sort_unstable();
        // serde_json's map is sorted; the struct's order is CONFIG_KEYS (checked against the file text below)
        assert_eq!(keys, want);
        let all: Map<String, Value> = CONFIG_KEYS.iter().map(|k| (k.to_string(), json!(1))).collect();
        let text = file_text(&all);
        let order: Vec<usize> = CONFIG_KEYS
            .iter()
            .map(|k| text.find(&format!("\"{k}\"")).unwrap())
            .collect();
        assert!(order.windows(2).all(|w| w[0] < w[1]));
        assert_eq!(file_text(&Map::new()), "{}\n");
    }

    #[test]
    fn check_map_locates_the_key() {
        let m = |v: Value| v.as_object().unwrap().clone();
        let e = Config::check_map(&m(json!({"max_runs": 0}))).unwrap_err();
        assert!(
            matches!(e, ConfigError::Refused { key: Some(ref k), .. } if k == "max_runs"),
            "{e:?}"
        );
        let e = Config::check_map(&m(json!({"max_runs": 2, "cpu_threads": 1.5}))).unwrap_err();
        assert!(
            matches!(e, ConfigError::Refused { key: Some(ref k), .. } if k == "cpu_threads"),
            "{e:?}"
        );
        let e = Config::check_map(&m(json!({"mem_budget_mb": -1}))).unwrap_err();
        assert!(
            matches!(e, ConfigError::Refused { key: Some(ref k), .. } if k == "mem_budget_mb"),
            "{e:?}"
        );
        assert!(Config::check_map(&m(json!({"mem_base_mb": 12.5}))).is_ok());
    }
}
