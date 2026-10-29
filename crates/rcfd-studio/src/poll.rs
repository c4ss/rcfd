//! The run worker (ADR 0133 D2, amendment 1 items 2-4): one thread per open project reads the run records every
//! [`PERIOD`] and sends the UI what changed. [`RunPoller::tick`] is one poll and is called directly by tests (R6).
//!
//! A tick lists `runs/`, reads `run.json` once per new run, `status.json` of every run not yet seen terminal, the
//! runner's lock, and for the shown runs the new bytes of `residuals.jsonl` and `events.jsonl` from kept offsets (after
//! their `status.json`), plus the tail of `log.txt` when its length changed. Bytes after the last newline are held back
//! until the line is complete. Control calls that start processes or wait run on short-lived threads.

use crate::runs::{self, residual_series, RunRow, StartPlan};
use rcfd_project::records::{RunEvent, RunRecord, RunState, RunStatus};
use rcfd_project::runid;
use rcfd_runner::client::Client;
use rcfd_runner::records::RunSettings;
use rcfd_runner::runner::Config;
use rcfd_runner::store::extra;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Poll period of the run worker (ADR 0133 D2: 250 ms).
pub const PERIOD: Duration = Duration::from_millis(250);

/// Lines of `log.txt` kept for the log panel (ADR 0133 D6).
pub const LOG_LINES: usize = 200;

/// Most bytes of one file read in one tick (a long run's first read takes several ticks instead of one long one).
pub const READ_CHUNK: u64 = 32 << 20;

/// Bytes read from the end of `log.txt` to find its last [`LOG_LINES`] lines.
const LOG_WINDOW: u64 = 256 << 10;

/// Bytes read from the end of `residuals.jsonl` to find the last line of a run that is not shown.
const LAST_LINE_WINDOW: u64 = 16 << 10;

/// Test switches for the registered negative controls (ADR 0133 amendment 1); all off in the app.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Switches {
    /// R1 control: read each `status.json` only once.
    pub status_once: bool,
    /// R4 control: move the offset to the end of the file, over a torn tail (the partial line is lost).
    pub drop_torn_tail: bool,
    /// R6 control: re-read every shown file from byte 0 each tick, sending only steps beyond the last sent.
    pub reread_whole: bool,
    /// R3 control: send a pause request for a resume.
    pub resume_as_pause: bool,
    /// R6 known miss (amendment 4): open every file again in every tick and read every `status.json` each tick, as the
    /// registered R6 set-up did.
    pub reopen_files: bool,
    /// The route before ADR 0149: the Start run dialog's plan reads a case as TOML only (a `.json` case is refused with
    /// `not TOML: ...`, a benchmark descriptor with the TOML parser's message).
    pub cases_as_toml: bool,
    /// The route before ADR 0152: the Start run dialog's plan takes a store key in the case text as the case's own
    /// whatever its value ([`crate::runs::PlanRoute::default_store_keys_set`]).
    pub default_store_keys_set: bool,
}

/// New samples of one run's series (column form): name -> (steps, values).
pub type Columns = BTreeMap<String, (Vec<f64>, Vec<f64>)>;

/// What the worker sends to the UI.
#[derive(Debug, Clone)]
pub enum Update {
    /// One tick ended.
    Tick(TickInfo),
    /// New residual and monitor samples of a shown run. `reset`: forget this run's series first (first read, or the file
    /// was replaced).
    Series {
        /// Run id.
        id: String,
        /// Start over.
        reset: bool,
        /// Residuals by series name (`res_u`, `res_p`, `continuity`).
        residuals: Columns,
        /// Monitors by report name.
        monitors: Columns,
    },
    /// New events of a shown run.
    Events {
        /// Run id.
        id: String,
        /// Start over.
        reset: bool,
        /// Parsed lines.
        items: Vec<RunEvent>,
    },
    /// The last lines of a shown run's `log.txt`.
    Log {
        /// Run id.
        id: String,
        /// At most [`LOG_LINES`] lines, oldest first.
        lines: Vec<String>,
    },
    /// `metrics.json` of a shown run that ended.
    Metrics {
        /// Run id.
        id: String,
        /// The document.
        value: Value,
    },
    /// A warning of a run (a non-finite value, a line that does not parse).
    Warning {
        /// Run id.
        id: String,
        /// Text.
        text: String,
    },
    /// The runner budgets (`.rcfd/runner-config.json`, defaults when absent).
    Config(Result<Config, String>),
    /// A start plan for the dialog.
    Plan(Result<StartPlan, String>),
    /// The cell count of a plan's case whose mesh the plan could not count (ADR 0140 D5): cells and mesh bytes.
    PlanCells {
        /// The plan's case (project-relative).
        case: String,
        /// Cells and D4 mesh bytes, or why the mesh could not be built.
        result: Result<(u64, u64), String>,
    },
    /// A run was queued (`Ok(id)`), or the start failed (located message).
    Started(Result<String, String>),
    /// `ensure_runner` returned.
    RunnerStarted(Result<(), String>),
    /// A control call's result to show.
    Message(String),
}

/// One tick's summary.
#[derive(Debug, Clone)]
pub struct TickInfo {
    /// Tick number, from 1.
    pub n: u64,
    /// When the tick started.
    pub started: Instant,
    /// The run rows when any changed since the last tick, sorted by (`created`, id).
    pub rows: Option<Vec<RunRow>>,
    /// The runner's lock is held.
    pub runner_running: bool,
    /// `status.json` reads that failed so far (the last known state was kept).
    pub status_errors: u64,
}

/// Requests from the UI.
#[derive(Debug, Clone)]
pub enum Command {
    /// Read these runs' series, events and log (the selected run).
    Show(Vec<String>),
    /// Hold a run.
    Pause(String),
    /// Release a run.
    Resume(String),
    /// Cancel a run.
    Cancel(String),
    /// Read the runner budgets.
    LoadConfig,
    /// Write the runner budgets.
    SaveConfig(Config),
    /// Make the start plan of a case (project-relative).
    Plan(String),
    /// Queue a run and make sure a runner is alive (`Client::start`).
    Start(StartPlan),
    /// Queue a run without starting a runner (`Client::submit`).
    Queue(StartPlan),
    /// Start the project's runner (`Client::ensure_runner`).
    EnsureRunner,
    /// Queue a new run of a run's unchanged case with the field store on (ADR 0140 D9).
    Rerun(String),
}

#[derive(Debug, Clone)]
struct Known {
    case: String,
    created: String,
    priority: i32,
    threads: usize,
    status: Option<RunStatus>,
    /// The terminal status was read (a terminal run is not read again).
    done: bool,
    /// Length of `residuals.jsonl` when its last line was read (runs not shown).
    res_len: Option<u64>,
    last_res: BTreeMap<String, f64>,
    held: bool,
    /// Length and modification time of `status.json` at its last read (amendment 4: unchanged, not read again).
    status_sig: Option<(u64, Option<std::time::SystemTime>)>,
}

#[derive(Debug, Default)]
struct Shown {
    res: Tail,
    ev: Tail,
    res_reset: bool,
    ev_reset: bool,
    log_len: Option<u64>,
    last_step: Option<u64>,
    warned: BTreeSet<String>,
    metrics_sent: bool,
}

/// The poll state of one project's runs.
pub struct RunPoller {
    root: PathBuf,
    client: Client,
    /// Negative-control switches.
    pub switches: Switches,
    known: BTreeMap<String, Known>,
    shown: BTreeMap<String, Shown>,
    last_rows: Option<Vec<RunRow>>,
    n: u64,
    status_errors: u64,
}

fn read_from(path: &Path, offset: u64, max: u64) -> std::io::Result<(u64, Vec<u8>)> {
    let mut f = std::fs::File::open(path)?;
    let len = f.metadata()?.len();
    let mut buf = Vec::new();
    if offset < len {
        f.seek(SeekFrom::Start(offset))?;
        f.take(max).read_to_end(&mut buf)?;
    }
    Ok((len, buf))
}

fn not_found(e: &std::io::Error) -> bool {
    e.kind() == std::io::ErrorKind::NotFound
}

/// A followed JSONL file: the offset up to the last complete line read, and the read handle kept between ticks (ADR
/// 0133 amendment 4: opening a file just appended to costs milliseconds on the gate host; reading through a kept
/// handle does not).
#[derive(Debug, Default)]
struct Tail {
    offset: u64,
    file: Option<std::fs::File>,
}

impl Tail {
    /// The new complete lines (at most `max` bytes), advancing the offset to just after the last newline read. The
    /// path's length is taken first: equal to the offset, nothing is opened or read; shorter, the file was cut or
    /// replaced and is opened again and read from 0 (`reset` true). With `reopen` (the R6 known-miss switch) the file
    /// is opened every call, as the registered R6 set-up did.
    fn read(&mut self, path: &Path, drop_torn: bool, max: u64, reopen: bool) -> (Vec<u8>, bool) {
        if reopen {
            self.file = None;
            return new_lines(path, &mut self.offset, drop_torn, max);
        }
        let len = match std::fs::metadata(path) {
            Ok(m) => m.len(),
            Err(_) => {
                self.file = None;
                return (Vec::new(), false);
            }
        };
        let mut reset = false;
        if len < self.offset {
            self.offset = 0;
            self.file = None;
            reset = true;
        }
        if len == self.offset {
            return (Vec::new(), reset);
        }
        if self.file.is_none() {
            self.file = std::fs::File::open(path).ok();
        }
        let Some(f) = self.file.as_mut() else {
            return (Vec::new(), reset);
        };
        let mut buf = Vec::new();
        if f.seek(SeekFrom::Start(self.offset)).is_err() || f.take(max).read_to_end(&mut buf).is_err() {
            self.file = None;
            return (Vec::new(), reset);
        }
        (take_complete(buf, &mut self.offset, drop_torn), reset)
    }
}

/// The new complete lines of a JSONL file from `*offset` (at most [`READ_CHUNK`] bytes), advancing the offset to just
/// after the last newline read; a file shorter than the offset was replaced and is read from 0 (`reset` true). Opens
/// the file on every call (the set-up R6 measured; kept for its known-miss test).
fn new_lines(path: &Path, offset: &mut u64, drop_torn: bool, max: u64) -> (Vec<u8>, bool) {
    let (len, buf) = match read_from(path, *offset, max) {
        Ok(v) => v,
        Err(_) => return (Vec::new(), false),
    };
    if len < *offset {
        *offset = 0;
        return match read_from(path, 0, max) {
            Ok((_, b)) => (take_complete(b, offset, drop_torn), true),
            Err(_) => (Vec::new(), true),
        };
    }
    (take_complete(buf, offset, drop_torn), false)
}

fn take_complete(mut buf: Vec<u8>, offset: &mut u64, drop_torn: bool) -> Vec<u8> {
    match buf.iter().rposition(|&b| b == b'\n') {
        Some(end) => {
            *offset += if drop_torn { buf.len() as u64 } else { end as u64 + 1 };
            buf.truncate(end + 1);
            buf
        }
        None => {
            if drop_torn {
                *offset += buf.len() as u64;
            }
            Vec::new()
        }
    }
}

fn lines(bytes: &[u8]) -> impl Iterator<Item = &[u8]> {
    bytes
        .split(|&b| b == b'\n')
        .map(|l| l.strip_suffix(b"\r").unwrap_or(l))
        .filter(|l| !l.iter().all(u8::is_ascii_whitespace))
}

fn last_residuals(line: &rcfd_project::records::ResidualLine) -> BTreeMap<String, f64> {
    line.residuals.iter().map(|(k, v)| (residual_series(k), *v)).collect()
}

/// The last complete line of a file, read from its end.
fn read_last_line(path: &Path) -> Option<(u64, Vec<u8>)> {
    let mut f = std::fs::File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    let start = len.saturating_sub(LAST_LINE_WINDOW);
    f.seek(SeekFrom::Start(start)).ok()?;
    let mut buf = Vec::new();
    f.take(len - start).read_to_end(&mut buf).ok()?;
    let end = buf.iter().rposition(|&b| b == b'\n')?;
    let begin = buf[..end].iter().rposition(|&b| b == b'\n').map_or(0, |i| i + 1);
    Some((len, buf[begin..end].to_vec()))
}

/// The last `n` lines of a text file, read from its end.
pub fn tail_lines(path: &Path, n: usize) -> std::io::Result<(u64, Vec<String>)> {
    let mut f = std::fs::File::open(path)?;
    let len = f.metadata()?.len();
    let start = len.saturating_sub(LOG_WINDOW);
    f.seek(SeekFrom::Start(start))?;
    let mut buf = Vec::new();
    f.take(len - start).read_to_end(&mut buf)?;
    let text = String::from_utf8_lossy(&buf);
    let mut v: Vec<&str> = text.lines().collect();
    if start > 0 && !v.is_empty() {
        v.remove(0); // the first line of the window may be cut
    }
    let from = v.len().saturating_sub(n);
    Ok((len, v[from..].iter().map(|s| s.to_string()).collect()))
}

impl RunPoller {
    /// A poller of the project at `root` (its canonical folder) whose runner client starts `runner_exe`.
    pub fn new(root: &Path, runner_exe: &Path) -> RunPoller {
        RunPoller {
            root: root.to_path_buf(),
            client: Client::new(root, runner_exe),
            switches: Switches::default(),
            known: BTreeMap::new(),
            shown: BTreeMap::new(),
            last_rows: None,
            n: 0,
            status_errors: 0,
        }
    }

    /// The runner client of this project.
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// The kept `residuals.jsonl` offset of a shown run (bytes read up to the last complete line).
    pub fn residual_offset(&self, id: &str) -> Option<u64> {
        self.shown.get(id).map(|s| s.res.offset)
    }

    fn dir(&self, id: &str) -> PathBuf {
        self.root.join("runs").join(id)
    }

    /// Show exactly `ids`: runs newly shown are read from the start of their files.
    pub fn show(&mut self, ids: &[String]) {
        self.shown.retain(|k, _| ids.contains(k));
        for id in ids {
            self.shown.entry(id.clone()).or_insert_with(|| Shown {
                res_reset: true,
                ev_reset: true,
                ..Shown::default()
            });
        }
    }

    fn list(&self) -> Vec<String> {
        let Ok(rd) = std::fs::read_dir(self.root.join("runs")) else {
            return Vec::new();
        };
        rd.filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|n| runid::is_run_id(n))
            .collect()
    }

    fn learn(&self, id: &str) -> Option<Known> {
        let bytes = std::fs::read(self.dir(id).join("run.json")).ok()?;
        let rec: RunRecord = serde_json::from_slice(&bytes).ok()?;
        let s = RunSettings::of(&rec).unwrap_or_default();
        Some(Known {
            case: rec.case,
            created: rec.created,
            priority: s.priority,
            threads: s.threads,
            status: None,
            done: false,
            res_len: None,
            last_res: BTreeMap::new(),
            held: false,
            status_sig: None,
        })
    }

    /// One poll. Returns the tick's summary and every update for the UI, in order.
    pub fn tick(&mut self) -> (TickInfo, Vec<Update>) {
        let started = Instant::now();
        self.n += 1;
        let mut out = Vec::new();
        let ids = self.list();
        self.known.retain(|k, _| ids.contains(k));
        for id in &ids {
            if !self.known.contains_key(id) {
                if let Some(k) = self.learn(id) {
                    self.known.insert(id.clone(), k);
                }
            }
        }
        // status of every run not yet seen terminal
        let ids: Vec<String> = self.known.keys().cloned().collect();
        for id in &ids {
            let dir = self.dir(id);
            let k = self.known.get_mut(id).expect("known");
            let mut read = if self.switches.status_once {
                k.status.is_none()
            } else {
                !k.done
            };
            let path = dir.join("status.json");
            let sig = (!self.switches.reopen_files)
                .then(|| std::fs::metadata(&path).ok().map(|m| (m.len(), m.modified().ok())))
                .flatten();
            if read && sig.is_some() && sig == k.status_sig && k.status.is_some() {
                read = false; // unchanged since its last read (amendment 4)
            }
            if read {
                match std::fs::read(&path)
                    .ok()
                    .and_then(|b| serde_json::from_slice::<RunStatus>(&b).ok())
                {
                    Some(st) => {
                        k.done = st.state.is_terminal();
                        k.status = Some(st);
                        k.status_sig = sig;
                    }
                    None => self.status_errors += 1,
                }
            }
            if !k.done {
                k.held = dir.join(extra::PAUSE).exists();
            } else {
                k.held = false;
            }
        }
        let runner_running = self.client.runner_running();
        // shown runs: series, events, log, metrics (after their status, so lines written before a state change are read)
        let shown: Vec<String> = self
            .shown
            .keys()
            .filter(|id| self.known.contains_key(*id))
            .cloned()
            .collect();
        for id in &shown {
            self.read_shown(id, &mut out);
        }
        // last residuals of the runs not shown
        for id in &ids {
            if self.shown.contains_key(id) {
                continue;
            }
            let path = self.dir(id).join("residuals.jsonl");
            let k = self.known.get_mut(id).expect("known");
            let len = std::fs::metadata(&path).map(|m| m.len()).ok();
            if len.is_some() && len != k.res_len {
                if let Some((l, line)) = read_last_line(&path) {
                    if let Ok(r) = serde_json::from_slice::<rcfd_project::records::ResidualLine>(&line) {
                        k.last_res = last_residuals(&r);
                    }
                    k.res_len = Some(l);
                }
            }
        }
        let mut rows: Vec<RunRow> = self
            .known
            .iter()
            .map(|(id, k)| {
                let st = k.status.as_ref();
                RunRow {
                    id: id.clone(),
                    case: k.case.clone(),
                    created: k.created.clone(),
                    state: st.map_or(RunState::Queued, |s| s.state),
                    step: st.map_or(0, |s| s.step),
                    time: st.map_or(0.0, |s| s.time),
                    pid: st.and_then(|s| s.pid),
                    reason: st.and_then(|s| s.reason.clone()),
                    priority: k.priority,
                    threads: k.threads,
                    held: k.held,
                    queue_pos: None,
                    last_residuals: k.last_res.clone(),
                    exit_code: st.and_then(|s| s.exit_code),
                }
            })
            .collect();
        rows.sort_by(|a, b| (&a.created, &a.id).cmp(&(&b.created, &b.id)));
        runs::set_queue_positions(&mut rows);
        // NaN != NaN: compare the rows through their bits so an unchanged NaN time is no change
        let changed = self.last_rows.as_ref().is_none_or(|old| !same_rows(old, &rows));
        let rows = if changed {
            self.last_rows = Some(rows.clone());
            Some(rows)
        } else {
            None
        };
        let info = TickInfo {
            n: self.n,
            started,
            rows,
            runner_running,
            status_errors: self.status_errors,
        };
        (info, out)
    }

    fn read_shown(&mut self, id: &str, out: &mut Vec<Update>) {
        let dir = self.dir(id);
        let sw = self.switches;
        let done = self.known.get(id).is_some_and(|k| k.done);
        let sh = self.shown.get_mut(id).expect("shown");
        // residuals
        let reread = sw.reread_whole;
        if reread {
            sh.res.offset = 0;
        }
        let max = if reread { u64::MAX } else { READ_CHUNK };
        let (bytes, replaced) = sh
            .res
            .read(&dir.join("residuals.jsonl"), sw.drop_torn_tail, max, sw.reopen_files);
        let reset = (sh.res_reset || replaced) && !reread;
        if replaced && !reread {
            sh.last_step = None;
            sh.warned.clear();
        }
        let (mut residuals, mut monitors) = (Columns::new(), Columns::new());
        let mut last = None;
        let mut bad = 0usize;
        for line in lines(&bytes) {
            let r: rcfd_project::records::ResidualLine = match serde_json::from_slice(line) {
                Ok(r) => r,
                Err(_) => {
                    bad += 1;
                    continue;
                }
            };
            if sh.last_step.is_some_and(|s| r.step <= s) {
                continue;
            }
            sh.last_step = Some(r.step);
            let x = r.step as f64;
            for (k, v) in &r.residuals {
                let name = residual_series(k);
                if !v.is_finite() && sh.warned.insert(name.clone()) {
                    out.push(Update::Warning {
                        id: id.to_string(),
                        text: format!(
                            "run {id}: residual {k} is not finite at step {} (shown as a gap)",
                            r.step
                        ),
                    });
                }
                let c = residuals.entry(name).or_default();
                c.0.push(x);
                c.1.push(*v);
            }
            for (k, v) in &r.monitors {
                if !v.is_finite() && sh.warned.insert(format!("monitor:{k}")) {
                    out.push(Update::Warning {
                        id: id.to_string(),
                        text: format!(
                            "run {id}: monitor {k} is not finite at step {} (shown as a gap)",
                            r.step
                        ),
                    });
                }
                let c = monitors.entry(k.clone()).or_default();
                c.0.push(x);
                c.1.push(*v);
            }
            last = Some(r);
        }
        if bad > 0 {
            out.push(Update::Warning {
                id: id.to_string(),
                text: format!("run {id}: {bad} line(s) of residuals.jsonl do not parse and were skipped"),
            });
        }
        if let (Some(l), Some(k)) = (&last, self.known.get_mut(id)) {
            k.last_res = last_residuals(l);
        }
        if reset || !residuals.is_empty() || !monitors.is_empty() {
            out.push(Update::Series {
                id: id.to_string(),
                reset,
                residuals,
                monitors,
            });
        }
        sh.res_reset = false;
        // events
        let (bytes, replaced) = sh.ev.read(
            &dir.join("events.jsonl"),
            sw.drop_torn_tail,
            READ_CHUNK,
            sw.reopen_files,
        );
        let items: Vec<RunEvent> = lines(&bytes).filter_map(|l| serde_json::from_slice(l).ok()).collect();
        let reset = sh.ev_reset || replaced;
        if reset || !items.is_empty() {
            out.push(Update::Events {
                id: id.to_string(),
                reset,
                items,
            });
        }
        sh.ev_reset = false;
        // log tail when its length changed
        let log = dir.join("log.txt");
        let len = std::fs::metadata(&log).map(|m| m.len()).ok();
        if len.is_some() && len != sh.log_len {
            if let Ok((l, lines)) = tail_lines(&log, LOG_LINES) {
                sh.log_len = Some(l);
                out.push(Update::Log {
                    id: id.to_string(),
                    lines,
                });
            }
        }
        // metrics once the run ended
        if done && !sh.metrics_sent {
            match std::fs::read(dir.join("metrics.json")) {
                Ok(b) => {
                    sh.metrics_sent = true;
                    if let Ok(value) = serde_json::from_slice::<Value>(&b) {
                        out.push(Update::Metrics {
                            id: id.to_string(),
                            value,
                        });
                    }
                }
                Err(e) if not_found(&e) => {}
                Err(_) => {}
            }
        }
    }
}

fn same_rows(a: &[RunRow], b: &[RunRow]) -> bool {
    let key = |r: &RunRow| {
        (
            r.time.to_bits(),
            r.last_residuals
                .iter()
                .map(|(k, v)| (k.clone(), v.to_bits()))
                .collect::<Vec<_>>(),
        )
    };
    a.len() == b.len()
        && a.iter().zip(b).all(|(x, y)| {
            (
                &x.id,
                &x.case,
                &x.created,
                x.state,
                x.step,
                x.pid,
                &x.reason,
                x.priority,
                x.threads,
                x.held,
                x.queue_pos,
            ) == (
                &y.id,
                &y.case,
                &y.created,
                y.state,
                y.step,
                y.pid,
                &y.reason,
                y.priority,
                y.threads,
                y.held,
                y.queue_pos,
            ) && key(x) == key(y)
                && x.exit_code == y.exit_code
        })
}

/// Called after the worker sent something the UI should draw (repaint request).
pub type Wake = std::sync::Arc<dyn Fn() + Send + Sync>;

/// Handle of a running run worker. Dropping it stops and joins the thread (control threads that start processes are
/// not joined; their results are dropped once the worker is gone).
pub struct RunWorker {
    tx: Option<Sender<Command>>,
    rx: Receiver<Update>,
    handle: Option<JoinHandle<()>>,
}

impl RunWorker {
    /// Start the worker of the project at `root` with the given period.
    pub fn start(root: &Path, runner_exe: &Path, period: Duration, switches: Switches, wake: Wake) -> RunWorker {
        let (ctx, crx) = mpsc::channel();
        let (utx, urx) = mpsc::channel();
        let mut poller = RunPoller::new(root, runner_exe);
        poller.switches = switches;
        let handle = std::thread::Builder::new()
            .name("rcfd-studio-runs".into())
            .spawn(move || run(poller, period, crx, utx, wake))
            .expect("spawn the run worker thread");
        RunWorker {
            tx: Some(ctx),
            rx: urx,
            handle: Some(handle),
        }
    }

    /// Send a request (ignored once the worker stopped).
    pub fn send(&self, c: Command) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(c);
        }
    }

    /// Everything the worker sent since the last call; never blocks.
    pub fn drain(&self) -> Vec<Update> {
        self.rx.try_iter().collect()
    }
}

impl Drop for RunWorker {
    fn drop(&mut self) {
        self.tx = None;
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

fn spawn_control(name: &str, f: impl FnOnce() -> Update + Send + 'static, tx: Sender<Update>, wake: Wake) {
    let _ = std::thread::Builder::new().name(name.into()).spawn(move || {
        let u = f();
        if tx.send(u).is_ok() {
            wake();
        }
    });
}

fn run(mut p: RunPoller, period: Duration, rx: Receiver<Command>, tx: Sender<Update>, wake: Wake) {
    let mut next = Instant::now();
    loop {
        let wait = next.saturating_duration_since(Instant::now());
        match rx.recv_timeout(wait) {
            Ok(cmd) => {
                let reply = handle(&mut p, cmd, &tx, &wake);
                if let Some(u) = reply {
                    if tx.send(u).is_err() {
                        return;
                    }
                    wake();
                }
            }
            Err(RecvTimeoutError::Disconnected) => return,
            Err(RecvTimeoutError::Timeout) => {
                next = Instant::now() + period;
                let (info, updates) = p.tick();
                let news = info.rows.is_some() || !updates.is_empty();
                for u in updates {
                    if tx.send(u).is_err() {
                        return;
                    }
                }
                if tx.send(Update::Tick(info)).is_err() {
                    return;
                }
                if news {
                    wake();
                }
            }
        }
    }
}

fn handle(p: &mut RunPoller, cmd: Command, tx: &Sender<Update>, wake: &Wake) -> Option<Update> {
    let msg = |r: Result<(), String>, what: &str, id: &str| {
        Update::Message(match r {
            Ok(()) => format!("{what} requested for run {id}"),
            Err(e) => format!("{what} run {id}: {e}"),
        })
    };
    match cmd {
        Command::Show(ids) => {
            p.show(&ids);
            None
        }
        Command::Pause(id) => Some(msg(p.client.pause(&id), "pause", &id)),
        Command::Resume(id) => Some(if p.switches.resume_as_pause {
            msg(p.client.pause(&id), "resume", &id)
        } else {
            msg(p.client.resume(&id), "resume", &id)
        }),
        Command::Cancel(id) => Some(msg(p.client.cancel(&id), "cancel", &id)),
        Command::LoadConfig => Some(Update::Config(Config::load(&p.root))),
        Command::SaveConfig(c) => {
            if let Err(e) = runs::save_config(&p.root, &c) {
                return Some(Update::Message(format!("runner budgets not saved: {e}")));
            }
            let _ = tx.send(Update::Config(Config::load(&p.root)));
            Some(Update::Message(if p.client.runner_running() {
                "runner budgets saved; the running runner reads them when it starts again (shut it down and start it)"
                    .into()
            } else {
                "runner budgets saved; the runner reads them when it starts".into()
            }))
        }
        Command::Plan(rel) => {
            let mut count = None;
            let r = p
                .root
                .join(&rel)
                .canonicalize()
                .map_err(|e| format!("{rel}: {e}"))
                .and_then(|path| {
                    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
                    let base = path.parent().unwrap_or(&p.root).to_path_buf();
                    // ADR 0149: JSON as `rcfd run` reads it, a benchmark descriptor's one line; ADR 0152: a store key at
                    // its schema default is not the case's own
                    let route = crate::runs::PlanRoute {
                        toml_only: p.switches.cases_as_toml,
                        default_store_keys_set: p.switches.default_store_keys_set,
                    };
                    let mut plan = StartPlan::new_route(&rel, &text, &base, route)?;
                    if plan.facts.cells.is_none() {
                        // ADR 0140 D5: the mesh is built on a control thread, never in the tick loop
                        plan.facts.extra.cells_state = crate::store_plan::CellsState::Counting;
                        count = Some((rel.clone(), text, base));
                    }
                    Ok(plan)
                });
            // the plan first, so that its cell count (sent later on the same channel) finds it
            if tx.send(Update::Plan(r)).is_ok() {
                wake();
            }
            if let Some((case, text, base)) = count {
                spawn_control(
                    "rcfd-studio-cells",
                    move || Update::PlanCells {
                        case,
                        result: crate::store_plan::count_cells(&text, &base),
                    },
                    tx.clone(),
                    wake.clone(),
                );
            }
            None
        }
        Command::Rerun(id) => {
            let (client, root) = (p.client.clone(), p.root.clone());
            spawn_control(
                "rcfd-studio-rerun",
                move || Update::Started(crate::store_plan::rerun(&client, &root, &id)),
                tx.clone(),
                wake.clone(),
            );
            None
        }
        Command::Start(plan) | Command::Queue(plan) if plan.case.is_empty() => {
            let _ = plan;
            Some(Update::Started(Err("no case chosen".into())))
        }
        Command::Start(plan) => {
            let (client, case, settings) = (p.client.clone(), p.root.join(&plan.case), plan.settings());
            spawn_control(
                "rcfd-studio-start",
                move || Update::Started(client.start(&case, &settings)),
                tx.clone(),
                wake.clone(),
            );
            None
        }
        Command::Queue(plan) => {
            let (client, case, settings) = (p.client.clone(), p.root.join(&plan.case), plan.settings());
            spawn_control(
                "rcfd-studio-queue",
                move || Update::Started(client.submit(&case, &settings)),
                tx.clone(),
                wake.clone(),
            );
            None
        }
        Command::EnsureRunner => {
            let client = p.client.clone();
            spawn_control(
                "rcfd-studio-runner",
                move || Update::RunnerStarted(client.ensure_runner()),
                tx.clone(),
                wake.clone(),
            );
            None
        }
    }
}
