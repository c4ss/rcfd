//! The project journal's entries (ADR 0173 D4 (c), registered for F-273 by ADR 0188 J1 to J5): who changed what in the
//! project folder and why, as lines of `.rcfd/edits.jsonl`.
//!
//! A line keeps `who` (the command) and adds, when known, the [`Actor`] (the client and its process), the `reason` the
//! caller gave and what changed (`changes`: each setting with its old and new value). A write of an existing file
//! extends the one line `rcfd_project::atomic::replace_if_hash_with` writes; a creation, a mesh import, a run submission
//! and a run control request append a line of their own ([`append`]). Every line is numbered (`seq`) by
//! `rcfd_project::journal::append`. Lines written before ADR 0188 carry none of these fields and are read as they are
//! ([`Reader`]).

use crate::{err, RpcError, PARAMS};
use rcfd_api::case::Case;
use rcfd_project::atomic::EDIT_LOG;
use rcfd_project::records::hash_bytes;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The client name of the MCP server (`rcfd-mcp --project`).
pub const MCP: &str = "mcp";
/// The client name of the Studio.
pub const STUDIO: &str = "studio";
/// Longest `reason`, in characters (ADR 0188 J4).
pub const MAX_REASON: usize = 500;
/// Most `changes` (and `set` entries) a line holds; the rest are counted in `changes_total` (`set_total`).
pub const MAX_CHANGES: usize = 100;
/// A value whose JSON text is longer than this many bytes is written as `{"bytes": n, "hash": "fnv64:..."}`.
pub const MAX_VALUE_BYTES: usize = 1000;

/// Who made a change: a client and its process (ADR 0173 D4 (b)). `started` is the time (RFC 3339, UTC) at which this
/// process first made an actor; the standard library gives no process start time, and with the pid it tells two
/// processes of one pid apart. Whether the MCP server's process can be linked to a Claude Code session is NOT VERIFIED
/// (ADR 0188 Q4); nothing here records such a link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Actor {
    /// `mcp`, `studio`, or another client's name.
    pub client: String,
    /// Process id.
    pub pid: u32,
    /// When this process first made an actor.
    pub started: String,
}

impl Actor {
    /// The actor `client` of this process.
    pub fn this_process(client: &str) -> Actor {
        static STARTED: OnceLock<String> = OnceLock::new();
        Actor {
            client: client.to_string(),
            pid: std::process::id(),
            started: STARTED.get_or_init(rcfd_project::time::now).clone(),
        }
    }

    /// `{client, instance: {pid, started}}`, as the journal writes it.
    pub fn to_json(&self) -> Value {
        json!({"client": self.client, "instance": {"pid": self.pid, "started": self.started}})
    }

    /// The actor of a journal line's `actor` value, if it has that form.
    pub fn from_json(v: &Value) -> Option<Actor> {
        Some(Actor {
            client: v.get("client")?.as_str()?.to_string(),
            pid: u32::try_from(v.pointer("/instance/pid")?.as_u64()?).ok()?,
            started: v.pointer("/instance/started")?.as_str()?.to_string(),
        })
    }

    /// `mcp (pid 4242)`.
    pub fn label(&self) -> String {
        format!("{} (pid {})", self.client, self.pid)
    }
}

/// The origin of one change: the command (`who`), the actor when known and the caller's reason.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Origin {
    /// The command's name.
    pub who: String,
    /// The client and process, when the writer has one.
    pub actor: Option<Actor>,
    /// Why, as the caller said.
    pub reason: Option<String>,
}

impl Origin {
    /// An origin.
    pub fn new(who: &str, actor: Option<Actor>, reason: Option<String>) -> Origin {
        Origin {
            who: who.to_string(),
            actor,
            reason,
        }
    }

    /// The fields `actor` and `reason` (each only when present).
    pub fn fields(&self) -> Map<String, Value> {
        let mut m = Map::new();
        if let Some(a) = &self.actor {
            m.insert("actor".into(), a.to_json());
        }
        if let Some(r) = &self.reason {
            m.insert("reason".into(), json!(r));
        }
        m
    }
}

/// The optional `reason` argument of a write command (ADR 0188 J4): a string of at most [`MAX_REASON`] characters
/// (-32602 otherwise); a blank one is no reason.
pub fn reason_arg(args: &Value) -> Result<Option<String>, RpcError> {
    match args.get("reason") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => {
            let n = s.chars().count();
            if n > MAX_REASON {
                return Err(err(
                    PARAMS,
                    format!("reason: at most {MAX_REASON} characters (this one has {n})"),
                ));
            }
            Ok((!s.trim().is_empty()).then(|| s.clone()))
        }
        Some(other) => Err(err(PARAMS, format!("reason must be a string, not {other}"))),
    }
}

/// `v`, or `{"bytes": n, "hash": "fnv64:..."}` when its JSON text is longer than [`MAX_VALUE_BYTES`].
pub fn bounded_value(v: Value) -> Value {
    let text = v.to_string();
    if text.len() > MAX_VALUE_BYTES {
        json!({"bytes": text.len(), "hash": hash_bytes(text.as_bytes())})
    } else {
        v
    }
}

/// Put the list `items` under `key`, cut to [`MAX_CHANGES`] entries; `<key>_total` holds the full count when it was cut.
pub fn put_list(line: &mut Map<String, Value>, key: &str, mut items: Vec<Value>) {
    let total = items.len();
    if total > MAX_CHANGES {
        items.truncate(MAX_CHANGES);
        line.insert(format!("{key}_total"), json!(total));
    }
    line.insert(key.into(), Value::Array(items));
}

/// One entry of `changes`.
pub fn change(path: &str, old: Option<Value>, new: Option<Value>) -> Value {
    json!({"path": path, "old": bounded_value(old.unwrap_or(Value::Null)),
        "new": bounded_value(new.unwrap_or(Value::Null))})
}

/// The leaf settings of a settings tree by dotted path (objects are opened; arrays and scalars are leaves).
fn leaves(v: &Value, prefix: &str, out: &mut BTreeMap<String, Value>) {
    match v.as_object() {
        Some(m) if !m.is_empty() => {
            for (k, c) in m {
                let p = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                leaves(c, &p, out);
            }
        }
        _ => {
            if !prefix.is_empty() {
                out.insert(prefix.to_string(), v.clone());
            }
        }
    }
}

/// Every setting whose value differs between the cases `old` and `new` (ADR 0188 J5): the leaves of both settings
/// trees, each compared as the case reads it (`Case::get`: defaults included); a setting only one side holds has `null`
/// on the other. In path order.
pub fn setting_changes(old: &Case, new: &Case) -> Vec<Value> {
    let (mut a, mut b) = (BTreeMap::new(), BTreeMap::new());
    leaves(&old.tree.to_json(), "", &mut a);
    leaves(&new.tree.to_json(), "", &mut b);
    let paths: std::collections::BTreeSet<&String> = a.keys().chain(b.keys()).collect();
    paths
        .into_iter()
        .filter_map(|p| {
            let o = old.get(p).ok().or_else(|| a.get(p).cloned());
            let n = new.get(p).ok().or_else(|| b.get(p).cloned());
            (o != n).then(|| change(p, o, n))
        })
        .collect()
}

/// Append a line of `kind` for `origin` to the journal of the project at `root`: `{t, kind, who, ...fields, actor?,
/// reason?}`, numbered (`seq`). Best effort, as the journal is: the `seq` written, or `None`.
pub fn append(root: &Path, kind: &str, origin: &Origin, fields: Map<String, Value>) -> Option<u64> {
    let mut line = fields;
    line.extend(origin.fields());
    line.insert("t".into(), json!(rcfd_project::time::now()));
    line.insert("kind".into(), json!(kind));
    line.insert("who".into(), json!(origin.who));
    rcfd_project::journal::append(root, Value::Object(line))
}

/// The new complete lines of an append-only JSON Lines file from byte `pos` on, as JSON objects. A torn last line is
/// left for the next read; a complete line that is not a JSON object is skipped and counted in `skipped`; a file shorter
/// than `pos` is read again from its start. Never writes.
pub(crate) fn read_lines(path: &Path, pos: &mut u64, skipped: &mut u64) -> Vec<Value> {
    let Ok(mut f) = std::fs::File::open(path) else {
        return Vec::new();
    };
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    if len < *pos {
        *pos = 0;
    }
    let mut buf = Vec::new();
    if f.seek(SeekFrom::Start(*pos)).is_err() || f.read_to_end(&mut buf).is_err() {
        return Vec::new();
    }
    let Some(end) = buf.iter().rposition(|&b| b == b'\n') else {
        return Vec::new();
    };
    *pos += end as u64 + 1;
    let mut out = Vec::new();
    for l in buf[..end].split(|&b| b == b'\n') {
        let l = l.strip_suffix(b"\r").unwrap_or(l);
        if l.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        match serde_json::from_slice::<Value>(l) {
            Ok(v) if v.is_object() => out.push(v),
            _ => *skipped += 1,
        }
    }
    out
}

/// A reader of the journal from a byte position on.
#[derive(Debug, Clone)]
pub struct Reader {
    path: PathBuf,
    pos: u64,
    skipped: u64,
}

impl Reader {
    /// A reader of the journal of the project at `root`, from its start.
    pub fn at_start(root: &Path) -> Reader {
        Reader {
            path: root.join(EDIT_LOG),
            pos: 0,
            skipped: 0,
        }
    }

    /// A reader from the journal's present end (what was written before is not read).
    pub fn at_end(root: &Path) -> Reader {
        let path = root.join(EDIT_LOG);
        let mut pos = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        // a torn last line is read once it is complete: start at the end of the last complete line
        let mut last = [0u8];
        let torn = pos > 0
            && std::fs::File::open(&path)
                .and_then(|mut f| f.seek(SeekFrom::Start(pos - 1)).and_then(|_| f.read_exact(&mut last)))
                .is_ok()
            && last[0] != b'\n';
        if torn {
            if let Ok(b) = std::fs::read(&path) {
                pos = b.iter().rposition(|&c| c == b'\n').map_or(0, |i| i + 1) as u64;
            }
        }
        Reader { path, pos, skipped: 0 }
    }

    /// The lines completed since the last read.
    pub fn read(&mut self) -> Vec<Value> {
        read_lines(&self.path, &mut self.pos, &mut self.skipped)
    }

    /// Complete lines skipped so far because they are not JSON objects.
    pub fn skipped(&self) -> u64 {
        self.skipped
    }
}
