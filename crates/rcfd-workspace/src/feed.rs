//! The change feed of a project folder (ADR 0173 R3 and D3 (a), registered for F-273 by ADR 0188 J6): one ordered list of
//! what changed, merged from the project journal (`.rcfd/edits.jsonl`, [`crate::journal`]), the runs' event files and the
//! watcher's changes. The files stay the truth; a reader polls them.
//!
//! One [`Feed::poll`]:
//!
//! 1. the watcher's poll (`rcfd_project::Watcher`; `.rcfd/` is not watched);
//! 2. the journal's new lines, each an entry with its actor, command, changes and reason;
//! 3. the new lines of each changed `runs/<id>/events.jsonl`: `state`, `warning` and `stop` events enter, other kinds
//!    (`solver`, `launched`, ...) and progress do not;
//! 4. every other changed path outside `runs/`: explained when a journal entry names it with the hash the file has now
//!    (kept [`CLAIM_KEEP`]); otherwise an `external` entry with its path only, once it is still unexplained
//!    [`EXTERNAL_WAIT`] after its modification time (after it was first seen, for a removed file). The wait covers a
//!    write whose journal line comes after the watcher saw the file.
//!
//! Entries are numbered from 1 in the order the reader finds them (journal lines in file order, run events by run and
//! file order, external paths in path order); the last [`KEEP`] are kept. The feed writes nothing.

use crate::journal::{read_lines, Actor, Reader};
use crate::queries::state_name;
use rcfd_project::records::{hash_bytes, EventKind, RunEvent};
use rcfd_project::{ProjectError, Watcher};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

/// How long a changed path may stay unexplained before it is reported as `external` (ADR 0188 J6).
pub const EXTERNAL_WAIT: Duration = Duration::from_millis(500);
/// How long a journal entry can explain a change the watcher has not seen yet.
pub const CLAIM_KEEP: Duration = Duration::from_secs(60);
/// Entries a feed keeps.
pub const KEEP: usize = 1000;

/// Where an entry comes from.
pub const JOURNAL: &str = "journal";
/// A run's events.
pub const RUN: &str = "run";
/// A change the journal does not explain.
pub const EXTERNAL: &str = "external";

/// One entry of the feed.
#[derive(Debug, Clone, PartialEq)]
pub struct Change {
    /// Its number in this feed, from 1.
    pub n: u64,
    /// [`JOURNAL`], [`RUN`] or [`EXTERNAL`].
    pub source: String,
    /// The journal line's `kind` (`edit`, `case_created`, `mesh_imported`, `run_submitted`, `run_control`,
    /// `stale_lock_broken`, ...), `run_state`, `run_warning`, `run_stop`, or `external`.
    pub kind: String,
    /// Time written (RFC 3339), when the line has one.
    pub t: Option<String>,
    /// The journal line's `seq` (lines written before ADR 0188 have none).
    pub seq: Option<u64>,
    /// The project-relative file it is about.
    pub path: Option<String>,
    /// The run it is about.
    pub run: Option<String>,
    /// The command.
    pub who: Option<String>,
    /// The client and process that made it (the journal's `actor`).
    pub actor: Option<Value>,
    /// Why, as the caller said.
    pub reason: Option<String>,
    /// The changed settings (`[{path, old, new}]`).
    pub changes: Option<Value>,
    /// The line's other fields (hashes, files, counts, the run's case, ...).
    pub other: Map<String, Value>,
    /// One line for people: who changed what and why.
    pub text: String,
}

impl Change {
    /// The entry as one JSON object (absent fields left out).
    pub fn to_json(&self) -> Value {
        let mut m = self.other.clone();
        m.insert("n".into(), json!(self.n));
        m.insert("source".into(), json!(self.source));
        m.insert("kind".into(), json!(self.kind));
        let mut opt = |k: &str, v: Option<Value>| {
            if let Some(v) = v {
                m.insert(k.into(), v);
            }
        };
        opt("t", self.t.clone().map(Value::from));
        opt("seq", self.seq.map(Value::from));
        opt("path", self.path.clone().map(Value::from));
        opt("run", self.run.clone().map(Value::from));
        opt("who", self.who.clone().map(Value::from));
        opt("actor", self.actor.clone());
        opt("reason", self.reason.clone().map(Value::from));
        opt("changes", self.changes.clone());
        m.insert("text".into(), json!(self.text));
        Value::Object(m)
    }

    /// The actor, when the entry has one of the journal's form.
    pub fn actor(&self) -> Option<Actor> {
        self.actor.as_ref().and_then(Actor::from_json)
    }
}

/// What one [`Feed::poll`] found.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Polled {
    /// Paths the watcher found changed in this poll (project-relative, sorted), as `Watcher::changes_since` gives them.
    pub paths: Vec<String>,
    /// The new entries, in order.
    pub changes: Vec<Change>,
}

/// A change of a file named by a journal entry, waiting for the watcher.
#[derive(Debug, Clone)]
struct Claim {
    hash: String,
    at: Instant,
}

/// The change feed of one project folder.
#[derive(Debug)]
pub struct Feed {
    root: PathBuf,
    watcher: Watcher,
    journal: Reader,
    events: BTreeMap<String, u64>,
    cases: BTreeMap<String, Option<String>>,
    run_skipped: u64,
    claims: BTreeMap<String, Vec<Claim>>,
    pending: BTreeMap<String, Instant>,
    kept: VecDeque<Change>,
    next: u64,
}

/// The run id of `runs/<id>/events.jsonl`.
fn events_run(path: &str) -> Option<&str> {
    path.strip_prefix("runs/")?
        .strip_suffix("/events.jsonl")
        .filter(|id| !id.contains('/'))
}

fn s(v: &Value, k: &str) -> Option<String> {
    v.get(k).and_then(Value::as_str).map(String::from)
}

/// A value for a one-line text: compact JSON, at most 60 characters.
fn short(v: &Value) -> String {
    let t = v.to_string();
    if t.chars().count() > 60 {
        format!("{}...", t.chars().take(57).collect::<String>())
    } else {
        t
    }
}

/// The one-line text of a journal line.
fn journal_text(kind: &str, line: &Value, actor: Option<&Actor>) -> String {
    let who = s(line, "who").unwrap_or_else(|| kind.to_string());
    let by = actor.map(|a| format!(" by {}", a.label())).unwrap_or_default();
    let path = s(line, "path").unwrap_or_default();
    let run = s(line, "run").unwrap_or_default();
    let what = match kind {
        "edit" => {
            let list = line["changes"].as_array().cloned().unwrap_or_default();
            let mut t = path.clone();
            if !list.is_empty() {
                let parts: Vec<String> = list
                    .iter()
                    .take(3)
                    .map(|c| {
                        format!(
                            "{} {} -> {}",
                            c["path"].as_str().unwrap_or("?"),
                            short(&c["old"]),
                            short(&c["new"])
                        )
                    })
                    .collect();
                t = format!("{t} {}", parts.join(", "));
                let total = line["changes_total"].as_u64().unwrap_or(list.len() as u64);
                if total > 3 {
                    t.push_str(&format!(" (and {} more)", total - 3));
                }
            }
            t
        }
        "case_created" => match s(line, "from") {
            Some(f) => format!("created {path} ({f})"),
            None => format!("created {path}"),
        },
        "mesh_imported" => format!(
            "imported {path} ({}, {} cells) from {}",
            s(line, "format").unwrap_or_default(),
            line["cells"],
            s(line, "source").unwrap_or_default()
        ),
        "run_submitted" => format!("run {run} of {} queued", s(line, "case").unwrap_or_default()),
        "run_control" => format!("{} run {run}", s(line, "requested").unwrap_or_default()),
        "stale_lock_broken" => format!("removed a stale edit lock of {path} ({} s old)", line["age_s"]),
        _ if !path.is_empty() => path.clone(),
        _ => run.clone(),
    };
    let why = s(line, "reason").map(|r| format!("; why: {r}")).unwrap_or_default();
    format!("{who}{by}: {what}{why}")
}

impl Feed {
    /// The feed of the project at `root` (canonical), from now: what the journal and the runs hold already is not
    /// reported.
    pub fn open(root: &Path) -> Result<Feed, ProjectError> {
        let watcher = Watcher::new(root)?;
        let mut events = BTreeMap::new();
        for e in std::fs::read_dir(root.join("runs")).into_iter().flatten().flatten() {
            let id = e.file_name().to_string_lossy().into_owned();
            if let Ok(m) = std::fs::metadata(e.path().join("events.jsonl")) {
                events.insert(id, m.len());
            }
        }
        Ok(Feed {
            root: root.to_path_buf(),
            watcher,
            journal: Reader::at_end(root),
            events,
            cases: BTreeMap::new(),
            run_skipped: 0,
            claims: BTreeMap::new(),
            pending: BTreeMap::new(),
            kept: VecDeque::new(),
            next: 1,
        })
    }

    /// The number of the latest entry (0 before the first).
    pub fn cursor(&self) -> u64 {
        self.next - 1
    }

    /// Lines of the journal and of the runs' event files skipped because they are not JSON objects.
    pub fn skipped_lines(&self) -> u64 {
        self.journal.skipped() + self.run_skipped
    }

    /// The kept entries numbered after `since`, oldest first, and how many entries after `since` are no longer kept.
    pub fn since(&self, since: u64) -> (Vec<Change>, u64) {
        let first = self.kept.front().map_or(self.next, |c| c.n);
        let dropped = first.saturating_sub(since + 1);
        (self.kept.iter().filter(|c| c.n > since).cloned().collect(), dropped)
    }

    fn claim(&mut self, path: &str, hash: Option<String>, at: Instant) {
        if let Some(hash) = hash {
            self.claims
                .entry(path.to_string())
                .or_default()
                .push(Claim { hash, at });
        }
    }

    /// The files a journal line says it wrote, with their hashes.
    fn claims_of(line: &Value) -> Vec<(String, Option<String>)> {
        let mut out = Vec::new();
        let kind = line["kind"].as_str().unwrap_or_default();
        if let Some(p) = s(line, "path") {
            let h = match kind {
                "edit" => s(line, "new_hash"),
                "case_created" => s(line, "hash"),
                _ => None,
            };
            out.push((p, h));
        }
        for key in ["data_files", "files"] {
            for f in line[key].as_array().into_iter().flatten() {
                if let Some(p) = s(f, "path") {
                    out.push((p, s(f, "hash")));
                }
            }
        }
        out
    }

    fn run_case(&mut self, id: &str) -> Option<String> {
        let root = &self.root;
        self.cases
            .entry(id.to_string())
            .or_insert_with(|| {
                rcfd_project::atomic::read_json::<Value>(&root.join("runs").join(id).join("run.json"))
                    .ok()
                    .and_then(|v| s(&v, "case"))
            })
            .clone()
    }

    fn run_entries(&mut self, id: &str, out: &mut Vec<Change>) {
        let path = self.root.join("runs").join(id).join("events.jsonl");
        let mut pos = self.events.get(id).copied().unwrap_or(0);
        let lines = read_lines(&path, &mut pos, &mut self.run_skipped);
        self.events.insert(id.to_string(), pos);
        for v in lines {
            let Ok(ev) = serde_json::from_value::<RunEvent>(v.clone()) else {
                self.run_skipped += 1;
                continue;
            };
            let case = self.run_case(id);
            let mut other = Map::new();
            if let Some(c) = &case {
                other.insert("case".into(), json!(c));
            }
            let at = case.map(|c| format!(" ({c})")).unwrap_or_default();
            let (kind, text, reason) = match ev.kind {
                EventKind::State { from, to, reason } => {
                    other.insert("from".into(), json!(from.map(state_name)));
                    other.insert("to".into(), json!(state_name(to)));
                    let from = from.map_or("new".to_string(), state_name);
                    (
                        "run_state",
                        format!("run {id}{at}: {from} -> {}", state_name(to)),
                        reason,
                    )
                }
                EventKind::Warning { message } => {
                    other.insert("message".into(), json!(message));
                    ("run_warning", format!("run {id}{at}: warning: {message}"), None)
                }
                EventKind::Stop { reason } => ("run_stop", format!("run {id}{at}: stopped: {reason}"), Some(reason)),
                EventKind::Other { .. } => continue,
            };
            let text = match &reason {
                Some(r) if kind == "run_state" => format!("{text} ({r})"),
                _ => text,
            };
            out.push(Change {
                n: 0,
                source: RUN.into(),
                kind: kind.into(),
                t: Some(ev.t),
                seq: None,
                path: None,
                run: Some(id.to_string()),
                who: None,
                actor: None,
                reason,
                changes: None,
                other,
                text,
            });
        }
    }

    /// One pass over the project (see the module text): the paths the watcher found changed and the new entries.
    pub fn poll(&mut self) -> Result<Polled, ProjectError> {
        let before = self.watcher.token();
        let token = self.watcher.poll()?;
        let paths = if token != before {
            self.watcher.changes_since(before)
        } else {
            Vec::new()
        };
        let now = Instant::now();
        let mut out = Vec::new();
        // the journal, read after the watcher's scan: a write the scan saw has its line here unless the line is late
        for line in self.journal.read() {
            for (p, h) in Self::claims_of(&line) {
                self.claim(&p, h, now);
            }
            let kind = s(&line, "kind").unwrap_or_default();
            let actor = line.get("actor").and_then(Actor::from_json);
            let text = journal_text(&kind, &line, actor.as_ref());
            let mut other = line.as_object().cloned().unwrap_or_default();
            for k in ["kind", "t", "seq", "path", "run", "who", "actor", "reason", "changes"] {
                other.remove(k);
            }
            out.push(Change {
                n: 0,
                source: JOURNAL.into(),
                kind,
                t: s(&line, "t"),
                seq: line.get("seq").and_then(Value::as_u64),
                path: s(&line, "path"),
                run: s(&line, "run"),
                who: s(&line, "who"),
                actor: line.get("actor").cloned(),
                reason: s(&line, "reason"),
                changes: line.get("changes").cloned(),
                other,
                text,
            });
        }
        // the runs' events
        let runs: Vec<String> = paths.iter().filter_map(|p| events_run(p)).map(String::from).collect();
        for id in runs {
            self.run_entries(&id, &mut out);
        }
        // the other changed paths: explained by a journal entry, or external once they have waited
        for p in paths.iter().filter(|p| !p.starts_with("runs/")) {
            self.pending.entry(p.clone()).or_insert(now);
        }
        self.claims.retain(|_, cs| {
            cs.retain(|c| now.duration_since(c.at) < CLAIM_KEEP);
            !cs.is_empty()
        });
        let sys_now = SystemTime::now();
        let pending: Vec<(String, Instant)> = self.pending.iter().map(|(p, t)| (p.clone(), *t)).collect();
        for (p, first) in pending {
            let file = self.root.join(&p);
            let meta = std::fs::metadata(&file).ok().filter(|m| m.is_file());
            if let (Some(_), Some(cs)) = (&meta, self.claims.get_mut(&p)) {
                let hash = std::fs::read(&file).ok().map(|b| hash_bytes(&b));
                if let Some(i) = cs.iter().position(|c| Some(&c.hash) == hash.as_ref()) {
                    cs.remove(i);
                    self.pending.remove(&p);
                    continue;
                }
            }
            let age = match meta.as_ref().and_then(|m| m.modified().ok()) {
                Some(m) => sys_now.duration_since(m).unwrap_or(Duration::ZERO),
                None => now.duration_since(first),
            };
            if age < EXTERNAL_WAIT {
                continue;
            }
            self.pending.remove(&p);
            let what = if meta.is_some() { "changed" } else { "removed" };
            out.push(Change {
                n: 0,
                source: EXTERNAL.into(),
                kind: EXTERNAL.into(),
                t: None,
                seq: None,
                path: Some(p.clone()),
                run: None,
                who: None,
                actor: None,
                reason: None,
                changes: None,
                other: Map::new(),
                text: format!("{p} {what} outside the workspace"),
            });
        }
        for c in &mut out {
            c.n = self.next;
            self.next += 1;
            self.kept.push_back(c.clone());
        }
        while self.kept.len() > KEEP {
            self.kept.pop_front();
        }
        Ok(Polled { paths, changes: out })
    }
}
