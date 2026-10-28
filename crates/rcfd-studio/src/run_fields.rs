//! Where a run's stored fields are and, when there are none to show, why (ADR 0140 D7, D8). The viewport worker calls
//! [`locate`] and [`Explainer::explain`]; the texts are the registered ones of ADR 0140 (SF7).
//!
//! No UI and no threads. [`Explainer`] reads `run.json` and `case.snapshot` once per run, `status.json` on every call,
//! and the `solver` warning lines of `events.jsonl` again only when that file's length changed (at most its last
//! [`EVENTS_WINDOW`] bytes).

use crate::store_plan::{case_unchanged, parse_entry};
use rcfd_project::records::{RunRecord, RunState, RunStatus};
use rcfd_runner::records::RunSettings;
use rcfd_tsstore::STORE_FILE;
use serde_json::Value;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// Bytes read from the end of `events.jsonl` to find the solver's field-store warning.
pub const EVENTS_WINDOW: u64 = 256 << 10;

/// The store folder of the contract (`runs/<id>/fields`).
pub fn contract_dir(run_dir: &Path) -> PathBuf {
    run_dir.join("fields")
}

/// The store folder of runs made before wave 48A (`runs/<id>/output/fields`).
pub fn legacy_dir(run_dir: &Path) -> PathBuf {
    run_dir.join("output").join("fields")
}

/// The store to open: `runs/<id>/fields/` when its store file exists, else `runs/<id>/output/fields/` (D7); `true` for
/// the second, the pre-48A location.
pub fn locate(run_dir: &Path) -> Option<(PathBuf, bool)> {
    let a = contract_dir(run_dir);
    if a.join(STORE_FILE).is_file() {
        return Some((a, false));
    }
    let b = legacy_dir(run_dir);
    b.join(STORE_FILE).is_file().then_some((b, true))
}

/// The note shown above a store read from the pre-48A location.
pub fn legacy_note(run_dir: &Path) -> String {
    format!(
        "Fields read from {}: runs made before wave 48A keep their store there.",
        legacy_dir(run_dir).display()
    )
}

/// Why a run has nothing to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmptyKind {
    /// Waiting for the runner.
    Queued,
    /// Held or paused before its first snapshot.
    Paused,
    /// Running, its first snapshot not stored yet.
    Waiting,
    /// Started without store keys.
    NotStored,
    /// Started with the store keys at 0.
    TurnedOff,
    /// Failed before its first snapshot.
    Failed,
    /// Cancelled before its first snapshot.
    Cancelled,
    /// The solver refused or dropped the store.
    Disabled,
    /// A moving mesh.
    MovingMesh,
    /// A chain run (stores per stage elsewhere).
    Chain,
    /// Finished, store asked for, none found.
    Missing,
    /// Ended with a store that holds no snapshot.
    EmptyStore,
    /// `run.json` could not be read.
    Unreadable,
}

/// What the viewport shows for a run without fields to show.
#[derive(Debug, Clone, PartialEq)]
pub struct EmptyState {
    /// Run id.
    pub run: String,
    /// Why.
    pub kind: EmptyKind,
    /// The main text.
    pub text: String,
    /// A hint under it (runs not started by the Studio).
    pub hint: Option<String>,
    /// Where the store was looked for (`Looked for <a> and <b>.`).
    pub looked: String,
    /// The re-run line: what the button does, or why there is no button.
    pub rerun_line: Option<String>,
    /// "Re-run with fields stored" is offered.
    pub rerun: bool,
}

impl EmptyState {
    /// The text of `Status::NoStore`: the main text, a newline and the "Looked for" line.
    pub fn message(&self) -> String {
        format!("{}\n{}", self.text, self.looked)
    }
}

/// The hint under "not stored" (registered text).
pub const NOT_STORED_HINT: &str = "Runs started from Claude Code or the command line store fields only when the case or an \
                                   override sets output.store_interval or output.store_time_interval.";

/// The re-run button's label.
pub const RERUN_LABEL: &str = "Re-run with fields stored";

/// How a run stores fields, from its case text and `set` overrides (last wins).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StoreAsked {
    /// `output.store_interval`.
    pub steps: u64,
    /// `output.store_time_interval`.
    pub time: f64,
    /// The last interval key set explicitly to 0.
    pub zero_key: Option<String>,
}

impl StoreAsked {
    /// A store is asked for.
    pub fn on(&self) -> bool {
        self.steps > 0 || self.time > 0.0
    }

    /// `every 4 iterations`, `every iteration` or `every 0.005 s of simulated time`.
    pub fn cadence(&self) -> String {
        match self.steps {
            0 => format!("every {} s of simulated time", self.time),
            1 => "every iteration".into(),
            k => format!("every {k} iterations"),
        }
    }
}

fn toml_num(v: &toml::Value) -> f64 {
    v.as_float().or_else(|| v.as_integer().map(|i| i as f64)).unwrap_or(0.0)
}

/// The store a run asks for: the case text's interval keys, then its `set` entries in order.
pub fn store_asked(case_text: &str, set: &[String]) -> StoreAsked {
    let mut a = StoreAsked::default();
    let raw: Option<toml::Value> = toml::from_str(case_text).ok();
    let out = raw.as_ref().and_then(|r| r.get("output"));
    let take = |a: &mut StoreAsked, key: &str, x: f64| {
        if key == "output.store_interval" {
            a.steps = x.max(0.0) as u64;
        } else {
            a.time = x.max(0.0);
        }
        if x <= 0.0 {
            a.zero_key = Some(key.to_string());
        }
    };
    for k in ["store_interval", "store_time_interval"] {
        if let Some(v) = out.and_then(|o| o.get(k)) {
            take(&mut a, &format!("output.{k}"), toml_num(v));
        }
    }
    for e in set {
        if let Some((p, v)) = parse_entry(e) {
            if p == "output.store_interval" || p == "output.store_time_interval" {
                take(&mut a, &p, v.as_f64().unwrap_or(0.0));
            }
        }
    }
    a
}

/// The facts of a run read once (its record and snapshot do not change).
#[derive(Debug, Clone)]
struct Record {
    case: String,
    asked: StoreAsked,
    moving: bool,
    chain: bool,
    rerun: Result<(), String>,
}

fn read_record(root: &Path, run_dir: &Path) -> Result<Record, String> {
    let rec: RunRecord = std::fs::read(run_dir.join("run.json"))
        .map_err(|e| e.to_string())
        .and_then(|b| serde_json::from_slice(&b).map_err(|e| e.to_string()))?;
    let set = RunSettings::of(&rec).map(|s| s.set).unwrap_or_default();
    let text = std::fs::read_to_string(run_dir.join("case.snapshot")).unwrap_or_default();
    let raw: Option<toml::Value> = toml::from_str(&text).ok();
    let path = |p: &[&str]| {
        let mut v = raw.as_ref()?;
        for k in p {
            v = v.get(k)?;
        }
        Some(v.clone())
    };
    let moving = path(&["dynamic_mesh", "enabled"]).and_then(|v| v.as_bool()) == Some(true)
        || set
            .iter()
            .filter_map(|e| parse_entry(e))
            .any(|(p, v)| p == "dynamic_mesh.enabled" && v == Value::Bool(true));
    let chain = path(&["chain", "sequence"])
        .and_then(|v| v.as_array().map(|a| !a.is_empty()))
        .unwrap_or(false);
    Ok(Record {
        asked: store_asked(&text, &set),
        moving,
        chain,
        rerun: case_unchanged(root, &rec).map(|_| ()),
        case: rec.case,
    })
}

/// The message of the solver's last warning about the field store in `events.jsonl` (its last [`EVENTS_WINDOW`] bytes).
fn store_warning(path: &Path) -> Option<String> {
    let mut f = std::fs::File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    let start = len.saturating_sub(EVENTS_WINDOW);
    f.seek(SeekFrom::Start(start)).ok()?;
    let mut buf = Vec::new();
    f.take(len - start).read_to_end(&mut buf).ok()?;
    let text = String::from_utf8_lossy(&buf);
    text.lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|v| v["kind"] == "solver" || v["event"] == "solver")
        .filter(|v| v["solver"]["event"] == "warning")
        .filter_map(|v| v["solver"]["message"].as_str().map(String::from))
        .filter(|m| m.contains("field store"))
        .last()
}

/// Explains empty viewports of one run at a time, caching what does not change.
#[derive(Debug, Default)]
pub struct Explainer {
    run: Option<String>,
    record: Option<Result<Record, String>>,
    events: Option<(u64, Option<String>)>,
}

impl Explainer {
    /// Why run `id` of the project at `root` has nothing to show: no store at either location (`store_exists`
    /// false), or a store without a snapshot (`true`).
    pub fn explain(&mut self, root: &Path, id: &str, store_exists: bool) -> EmptyState {
        if self.run.as_deref() != Some(id) {
            self.run = Some(id.to_string());
            self.record = None;
            self.events = None;
        }
        let run_dir = root.join("runs").join(id);
        if self.record.as_ref().is_none_or(|r| r.is_err()) {
            // an unreadable record is read again next time (a run folder still being written)
            self.record = Some(read_record(root, &run_dir));
        }
        let record = self.record.clone().expect("read above");
        let looked = format!(
            "Looked for {} and {}.",
            contract_dir(&run_dir).join(STORE_FILE).display(),
            legacy_dir(&run_dir).join(STORE_FILE).display()
        );
        let mut st = EmptyState {
            run: id.to_string(),
            kind: EmptyKind::Unreadable,
            text: String::new(),
            hint: None,
            looked,
            rerun_line: None,
            rerun: false,
        };
        let rec = match record {
            Ok(r) => r,
            Err(e) => {
                st.text = format!("Run {id}: run.json could not be read ({e}).");
                return st;
            }
        };
        let set = |st: &mut EmptyState, kind: EmptyKind, text: String| {
            st.kind = kind;
            st.text = text;
        };
        if rec.moving {
            set(
                &mut st,
                EmptyKind::MovingMesh,
                format!(
                    "Run {id} uses a moving mesh (dynamic_mesh.enabled), which the field store does not support, so \
                     there is nothing to show."
                ),
            );
            return st;
        }
        if rec.chain {
            set(
                &mut st,
                EmptyKind::Chain,
                format!(
                    "Run {id} is a chain run: each stage stores its fields in runs/{id}/output/<stage>/fields/, which \
                     the Viewport does not read yet."
                ),
            );
            return st;
        }
        if !rec.asked.on() {
            match &rec.asked.zero_key {
                Some(k) => set(
                    &mut st,
                    EmptyKind::TurnedOff,
                    format!("Run {id} was started with the field store turned off ({k} = 0), so there is nothing to show."),
                ),
                None => {
                    set(
                        &mut st,
                        EmptyKind::NotStored,
                        format!("Run {id} was started without storing fields, so there is nothing to show."),
                    );
                    st.hint = Some(NOT_STORED_HINT.into());
                }
            }
            match &rec.rerun {
                Ok(()) => {
                    st.rerun = true;
                    st.rerun_line = Some(format!(
                        "Starts a new run of {} (unchanged since this run) with the field store on.",
                        rec.case
                    ));
                }
                Err(why) => st.rerun_line = Some(why.clone()),
            }
            return st;
        }
        let events = run_dir.join("events.jsonl");
        let len = std::fs::metadata(&events).map(|m| m.len()).unwrap_or(0);
        if self.events.as_ref().is_none_or(|(l, _)| *l != len) {
            self.events = Some((len, store_warning(&events)));
        }
        if let Some(w) = self.events.as_ref().and_then(|(_, w)| w.clone()) {
            set(
                &mut st,
                EmptyKind::Disabled,
                format!("The solver did not store fields for run {id}: {w}"),
            );
            return st;
        }
        let status: Option<RunStatus> = std::fs::read(run_dir.join("status.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok());
        let state = status.as_ref().map_or(RunState::Queued, |s| s.state);
        match state {
            RunState::Queued => set(
                &mut st,
                EmptyKind::Queued,
                format!("Run {id} has not started yet. Its fields appear here once it stores its first snapshot."),
            ),
            RunState::Paused => set(
                &mut st,
                EmptyKind::Paused,
                format!(
                    "Run {id} is paused and has not stored a snapshot yet. Its fields appear here once it stores one."
                ),
            ),
            RunState::Running => set(
                &mut st,
                EmptyKind::Waiting,
                format!(
                    "Run {id} is running and stores its fields {}. Waiting for the first snapshot.",
                    rec.asked.cadence()
                ),
            ),
            RunState::Failed => {
                let text = match status.as_ref().and_then(|s| s.reason.as_deref()) {
                    Some(r) => format!("Run {id} failed before it stored its first snapshot: {r}"),
                    None => format!("Run {id} failed before it stored its first snapshot."),
                };
                set(&mut st, EmptyKind::Failed, text);
            }
            RunState::Cancelled => set(
                &mut st,
                EmptyKind::Cancelled,
                format!("Run {id} was cancelled before it stored its first snapshot."),
            ),
            RunState::Finished if store_exists => set(
                &mut st,
                EmptyKind::EmptyStore,
                format!("Run {id} ended, but its field store holds no snapshot."),
            ),
            RunState::Finished => set(
                &mut st,
                EmptyKind::Missing,
                format!("Run {id} asked for stored fields, but no store was found."),
            ),
        }
        st
    }
}
