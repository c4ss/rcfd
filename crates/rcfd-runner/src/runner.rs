//! The runner process (`rcfd-runner serve`): one per project, holding `<project>/.rcfd/runner.lock`.
//!
//! The project folder is the queue: a client creates `runs/<id>/` with status `queued` and the runner, polling
//! every `poll_ms`, launches queued runs in priority order while the slot, thread and memory budgets allow.
//! Each run is an `rcfd run` child process whose output goes to files (never pipes), so the solver outlives
//! the runner and a later runner adopts it. Pause and cancel are request files in the run folder that the
//! solver itself watches (`--pause-file`, `--stop-file`). ADR 0123.

use crate::records::{self, LaunchSettings, RunRecord, RunSettings, RunState, RunStatus};
use crate::store::{event_kind, extra, state_event, FsStore, RunStore};
use crate::sys;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet};
use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// Most bytes of the solver stream read per drain (ADR 0128 finding 6).
const DRAIN_CHUNK: u64 = 4 << 20;

/// Exit code of `rcfd-runner serve` when another runner holds the project's lock.
pub const EXIT_ALREADY_RUNNING: i32 = 3;

/// Runner settings, from `<project>/.rcfd/runner-config.json` (every key optional).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Runs holding a slot at once (running or paused).
    pub max_runs: usize,
    /// Sum of the `threads` of the runs holding a slot.
    pub cpu_threads: usize,
    /// Sum of the memory of the runs holding a slot, MB.
    pub mem_budget_mb: u64,
    /// Memory estimate of a run without a declared `mem_mb`: this base ...
    pub mem_base_mb: f64,
    /// ... plus this many KB per cell (cells from the pre-launch check).
    pub mem_kb_per_cell: f64,
    /// Poll period of the runner loop, ms.
    pub poll_ms: u64,
    /// Liveness probe period of adopted solver processes (not children of this runner), ms.
    pub adopt_probe_ms: u64,
    /// Time a cancelled solver gets to stop cleanly before it is killed, ms.
    pub cancel_grace_ms: u64,
}

impl Default for Config {
    fn default() -> Self {
        let cores = std::thread::available_parallelism().map_or(2, |n| n.get());
        Config {
            max_runs: 2,
            cpu_threads: (cores / 2).max(1),
            mem_budget_mb: 4096,
            mem_base_mb: 64.0,
            mem_kb_per_cell: 4.0,
            poll_ms: 100,
            adopt_probe_ms: 1000,
            cancel_grace_ms: 10_000,
        }
    }
}

/// `<project>/.rcfd/`.
pub fn dot_dir(project: &Path) -> PathBuf {
    project.join(".rcfd")
}

/// Folder of the solvers' liveness files under [`dot_dir`], one per launched run, named by the run id (ADR 0229 D3).
pub const ALIVE_DIR: &str = "alive";

/// `<project>/.rcfd/runner-config.json`.
pub fn config_path(project: &Path) -> PathBuf {
    dot_dir(project).join("runner-config.json")
}

impl Config {
    /// Read the project's config file (defaults when it does not exist); errors name the file.
    pub fn load(project: &Path) -> Result<Config, String> {
        let p = config_path(project);
        let c: Config = match std::fs::read_to_string(&p) {
            Ok(t) => serde_json::from_str(&t).map_err(|e| format!("{}: {e}", p.display()))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Config::default(),
            Err(e) => return Err(format!("{}: {e}", p.display())),
        };
        if c.max_runs == 0 || c.cpu_threads == 0 || c.poll_ms == 0 {
            return Err(format!(
                "{}: max_runs, cpu_threads and poll_ms must be positive",
                p.display()
            ));
        }
        Ok(c)
    }

    /// The solver binary: `RCFD_BIN`, else `rcfd` next to the runner. Never from the config file: a project folder
    /// must not choose the program the runner executes (ADR 0128 finding 1; an `rcfd_bin` key is an unknown field).
    pub fn solver(&self) -> PathBuf {
        std::env::var_os("RCFD_BIN")
            .map(PathBuf::from)
            .unwrap_or_else(|| sibling_exe("rcfd"))
    }
}

/// `name` (plus the platform suffix) in the directory of the current executable.
pub fn sibling_exe(name: &str) -> PathBuf {
    let exe = std::env::current_exe().unwrap_or_default();
    exe.with_file_name(format!("{name}{}", std::env::consts::EXE_SUFFIX))
}

static LOG: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// Append a line to `.rcfd/runner.log` (the runner has no console).
fn log(msg: &str) {
    if let Some(p) = LOG.get() {
        let _ = records::append_line(p, &format!("{} {msg}", records::now_iso()));
    }
}

fn note<T>(what: &str, r: std::io::Result<T>) {
    if let Err(e) = r {
        log(&format!("{what}: {e}"));
    }
}

struct Active {
    id: String,
    child: Option<Child>,
    pid: u32,
    threads: usize,
    mem_mb: u64,
    offset: u64,
    step: u64,
    time: f64,
    max_seq: u64,
    finish: Option<Value>,
    paused: bool,
    cancel_at: Option<Instant>,
    killed_for_cancel: bool,
    next_probe: Instant,
    /// Consecutive missed liveness probes (adopted solvers).
    misses: u8,
    /// An append of the last drain failed (logged once; the chunk is read again, ADR 0229 D5).
    append_failed: bool,
}

struct Pending {
    id: String,
    rec: RunRecord,
    /// `rec.settings`, typed.
    settings: RunSettings,
    paused: bool,
    mem_mb: Option<u64>,
    check: Option<Child>,
}

struct Runner<S: RunStore = FsStore> {
    store: S,
    cfg: Config,
    cfg_src: String,
    /// The solver binary, fixed at runner start.
    solver: PathBuf,
    /// Liveness probe of adopted solvers (`sys::pid_alive`; tests script it).
    probe: fn(u32, &str) -> bool,
    /// The liveness file's answer (`sys::held`, ADR 0229 D3; tests script it).
    held: fn(&Path) -> Option<bool>,
    /// The forced kill of a solver by its pid (`sys::kill`; tests record it).
    kill: fn(u32) -> bool,
    /// Status writes that failed, the latest per run: written again on every tick until they succeed (ADR 0229 D4).
    unwritten: RefCell<BTreeMap<String, RunStatus>>,
    known: HashSet<String>,
    pending: Vec<Pending>,
    active: Vec<Active>,
    launch_seq: u64,
}

/// Run the runner for `project` until a shutdown request (`<project>/.rcfd/shutdown`). Returns the exit
/// code: 0 after a clean shutdown, [`EXIT_ALREADY_RUNNING`] when another runner holds the lock.
pub fn serve(project: &Path) -> Result<i32, String> {
    let dot = dot_dir(project);
    std::fs::create_dir_all(&dot).map_err(|e| format!("{}: {e}", dot.display()))?;
    let cfg = Config::load(project)?;
    let lock = sys::acquire_lock(&dot.join("runner.lock")).map_err(|e| format!("runner lock: {e}"))?;
    let Some(lock) = lock else {
        return Ok(EXIT_ALREADY_RUNNING);
    };
    let _ = LOG.set(dot.join("runner.log"));
    log(&format!("runner {} started", std::process::id()));
    // a shutdown request older than this runner is void
    let _ = std::fs::remove_file(dot.join("shutdown"));
    let cfg_src = if config_path(project).exists() {
        config_path(project).display().to_string()
    } else {
        "the default (no runner-config.json)".into()
    };
    let info = json!({"pid": std::process::id(), "started": records::now_iso(), "config": cfg,
                      "config_source": cfg_src, "rcfd_bin": cfg.solver()});
    records::atomic_write(&dot.join("runner.json"), info.to_string().as_bytes()).map_err(|e| e.to_string())?;
    let mut r = Runner::new(FsStore::new(project), cfg, cfg_src);
    let poll = Duration::from_millis(r.cfg.poll_ms);
    loop {
        if dot.join("shutdown").exists() {
            break;
        }
        r.scan();
        r.tick_pending();
        r.tick_active();
        r.schedule();
        std::thread::sleep(poll);
    }
    r.shutdown();
    log(&format!("runner {} shut down cleanly", std::process::id()));
    let _ = std::fs::remove_file(dot.join("runner.json"));
    drop(lock);
    // after the lock: while this runner held it, the request told a start to wait for it (ADR 0229 D2)
    let _ = std::fs::remove_file(dot.join("shutdown"));
    Ok(0)
}

/// Missed-probe count after one probe: a hit resets it. Two consecutive misses finalise an adopted run, so one
/// failed probe (a slow or failed process listing) does not end a live solver (ADR 0128 finding 4).
fn next_misses(misses: u8, alive: bool) -> u8 {
    if alive {
        0
    } else {
        misses.saturating_add(1)
    }
}

/// An event line of kind `kind` in both tag forms (see [`crate::store::event`]) with the fields of `fields`.
fn event_line(kind: &str, fields: Value) -> Value {
    let mut v = crate::store::event(kind);
    if let (Some(o), Value::Object(f)) = (v.as_object_mut(), fields) {
        o.extend(f);
    }
    v
}

fn read_last_line(p: &Path) -> Option<String> {
    let text = std::fs::read_to_string(p).ok()?;
    text.lines().rev().find(|l| !l.trim().is_empty()).map(String::from)
}

/// The `residuals.jsonl` line of the solver `iteration` event `v` for step `idx`: step, time, the residuals and the report
/// values (`rcfd run --status-monitors`, ADR 0130 D5; empty from a stream without them), and, only when the event carries
/// them (`solver.corrector_control = "adaptive"`, ADR 0156 D4), the step's `correctors` and `corrector_cap` (ADR 0160 D4):
/// the line of any other run is unchanged.
pub fn residual_line(v: &Value, idx: u64) -> Value {
    let mut line = json!({"step": idx, "time": v["time"],
        "residuals": {"u": v["res_u"], "p": v["res_p"], "continuity": v["continuity"]},
        "monitors": v.get("monitors").filter(|m| m.is_object()).cloned().unwrap_or_else(|| json!({}))});
    for k in ["correctors", "corrector_cap"] {
        if let Some(x) = v.get(k) {
            line[k] = x.clone();
        }
    }
    line
}

/// Final report values for `metrics.json` (ADR 0135 amendment 3): the report names are the `monitors` keys of the last
/// residual line (`rcfd run --status-monitors`); each value is the last `y` of that series in the solver's
/// `monitors.json`, else the residual line's value. Non-finite values are `null` (as serde_json writes them).
pub fn report_values(monitors_json: &Path, last_residual: Option<&Value>) -> Value {
    let mon: Option<Value> = std::fs::read_to_string(monitors_json)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok());
    let mut out = serde_json::Map::new();
    for (name, v) in last_residual
        .and_then(|l| l.get("monitors"))
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
    {
        let fin = mon
            .as_ref()
            .and_then(|m| m["series"][name.as_str()]["y"].as_array())
            .and_then(|y| y.last())
            .cloned();
        out.insert(name.clone(), fin.unwrap_or_else(|| v.clone()));
    }
    Value::Object(out)
}

impl<S: RunStore> Runner<S> {
    fn new(store: S, cfg: Config, cfg_src: String) -> Self {
        Runner {
            store,
            solver: cfg.solver(),
            probe: sys::pid_alive,
            held: sys::held,
            kill: sys::kill,
            unwritten: RefCell::new(BTreeMap::new()),
            cfg,
            cfg_src,
            known: HashSet::new(),
            pending: Vec::new(),
            active: Vec::new(),
            launch_seq: 0,
        }
    }

    fn dir(&self, id: &str) -> PathBuf {
        self.store.run_dir(id)
    }

    fn has(&self, id: &str, name: &str) -> bool {
        self.dir(id).join(name).exists()
    }

    /// The liveness file of run `id` (ADR 0229 D3): under `.rcfd/`, which the project watcher does not read.
    fn alive(&self, id: &str) -> PathBuf {
        dot_dir(self.store.project()).join(ALIVE_DIR).join(id)
    }

    /// Write `status.json` of run `id`; a write that fails is kept and written again on every tick
    /// ([`Runner::retry_status`]), a later write for the run replacing it (ADR 0229 D4). Logged once per failure.
    fn put_status(&self, id: &str, st: &RunStatus) {
        let mut kept = self.unwritten.borrow_mut();
        match self.store.write_status(id, st) {
            Ok(()) => {
                if kept.remove(id).is_some() {
                    log(&format!("status of run {id}: written, after a write that failed"));
                }
            }
            Err(e) => {
                if kept.insert(id.to_string(), st.clone()).is_none() {
                    log(&format!("status of run {id}: {e}; the write is repeated on every tick"));
                }
            }
        }
    }

    /// Write again the statuses whose last write failed.
    fn retry_status(&self) {
        let kept: Vec<(String, RunStatus)> = self
            .unwritten
            .borrow()
            .iter()
            .map(|(id, st)| (id.clone(), st.clone()))
            .collect();
        for (id, st) in kept {
            if self.store.write_status(&id, &st).is_ok() {
                self.unwritten.borrow_mut().remove(&id);
                log(&format!("status of run {id}: written, after a write that failed"));
            }
        }
    }

    fn set_state(&self, id: &str, st: &RunStatus, why: &str) {
        let from = self.store.read_status(id).ok().map(|s| s.state);
        self.put_status(id, st);
        let mut ev = state_event(from, st.state, why);
        if let Some(p) = st.pid {
            ev["pid"] = json!(p);
        }
        note("event", self.store.append_event(id, ev));
    }

    /// End a run that was never launched.
    fn end_pending(&self, id: &str, state: RunState, reason: String) {
        let mut st = RunStatus::new(state);
        st.reason = Some(reason.clone());
        note(
            "metrics",
            self.store.write_metrics(
                id,
                &json!({"state": state, "reason": reason, "steps": 0, "launched": false, "reports": {}}),
            ),
        );
        self.set_state(id, &st, &reason);
        self.clear_requests(id);
    }

    fn fail_pending(&self, id: &str, reason: String) {
        self.end_pending(id, RunState::Failed, reason);
    }

    fn clear_requests(&self, id: &str) {
        for f in [extra::PAUSE, extra::PAUSE_ACK, extra::CANCEL] {
            let _ = std::fs::remove_file(self.dir(id).join(f));
        }
    }

    /// Pick up run folders not seen yet. On the first call this is the crash recovery: runs left `running`
    /// or `paused` with a pid by an earlier runner are adopted and probed.
    fn scan(&mut self) {
        let ids = match self.store.list_runs() {
            Ok(v) => v,
            Err(e) => return log(&format!("list runs: {e}")),
        };
        for id in ids {
            if self.known.contains(&id) {
                continue;
            }
            let (Ok(st), Ok(rec)) = (self.store.read_status(&id), self.store.read_record(&id)) else {
                continue; // retried next tick
            };
            self.known.insert(id.clone());
            let launched = match (st.state, st.pid) {
                (RunState::Queued, _) | (RunState::Paused, None) => self.launched_pid(&id),
                _ => None,
            };
            if let Some(pid) = launched {
                // launched, but the running status never reached the disk: adopt (the probe settles it), never relaunch
                let mut run = st.clone();
                run.state = RunState::Running;
                run.pid = Some(pid);
                self.set_state(&id, &run, "adopted: a launched event without a running status");
                self.adopt(id, &run, pid, &rec);
                continue;
            }
            match (st.state, st.pid) {
                (RunState::Queued, _) | (RunState::Paused, None) => self.pending.push(Pending {
                    paused: st.state == RunState::Paused,
                    mem_mb: None,
                    check: None,
                    // read_record refuses settings that do not parse
                    settings: RunSettings::of(&rec).unwrap_or_default(),
                    id,
                    rec,
                }),
                (RunState::Running | RunState::Paused, Some(pid)) => self.adopt(id, &st, pid, &rec),
                (RunState::Running, None) => self.fail_pending(
                    &id,
                    "status.json says running without a solver pid (inconsistent record found at runner start)".into(),
                ),
                _ => {}
            }
        }
    }

    /// Pid of the last `launched` event of run `id`.
    fn launched_pid(&self, id: &str) -> Option<u32> {
        std::fs::read_to_string(self.dir(id).join("events.jsonl"))
            .unwrap_or_default()
            .lines()
            .rev()
            .filter_map(|l| serde_json::from_str::<Value>(l).ok())
            .find(|v| event_kind(v) == Some("launched"))
            .and_then(|v| v["pid"].as_u64())
            .and_then(|p| u32::try_from(p).ok())
    }

    fn adopt(&mut self, id: String, st: &RunStatus, pid: u32, rec: &RunRecord) {
        let dir = self.dir(&id);
        let settings = RunSettings::of(rec).unwrap_or_default();
        // already forwarded solver events and residual steps are not forwarded twice
        let events: Vec<Value> = std::fs::read_to_string(dir.join("events.jsonl"))
            .unwrap_or_default()
            .lines()
            .filter_map(|l| serde_json::from_str::<Value>(l).ok())
            .collect();
        let max_seq = events
            .iter()
            .filter_map(|v| v["solver_seq"].as_u64())
            .max()
            .unwrap_or(0);
        // the memory the launching runner counted for it
        let mem_mb = events
            .iter()
            .rev()
            .find(|v| event_kind(v) == Some("launched"))
            .and_then(|v| v["mem_mb"].as_u64())
            .unwrap_or(settings.mem_mb.unwrap_or(0));
        let step = read_last_line(&dir.join("residuals.jsonl"))
            .and_then(|l| serde_json::from_str::<Value>(&l).ok())
            .and_then(|v| v["step"].as_u64())
            .unwrap_or(0);
        note(
            "event",
            self.store.append_event(
                &id,
                event_line(
                    "adopted",
                    json!({"pid": pid, "note": "solver found by a new runner; probed for liveness"}),
                ),
            ),
        );
        self.active.push(Active {
            id,
            child: None,
            pid,
            threads: settings.threads,
            mem_mb,
            offset: 0,
            step,
            time: st.time,
            max_seq,
            finish: None,
            paused: st.state == RunState::Paused,
            cancel_at: None,
            killed_for_cancel: false,
            next_probe: Instant::now(),
            misses: 0,
            append_failed: false,
        });
    }

    fn tick_pending(&mut self) {
        let mut keep = Vec::new();
        let pending = std::mem::take(&mut self.pending);
        let mut checking = pending.iter().any(|p| p.check.is_some());
        for mut p in pending {
            if self.has(&p.id, extra::CANCEL) {
                if let Some(mut c) = p.check.take() {
                    let _ = c.kill();
                    let _ = c.wait();
                }
                self.end_pending(&p.id, RunState::Cancelled, "cancelled before launch".into());
                continue;
            }
            let hold = self.has(&p.id, extra::PAUSE);
            if hold != p.paused {
                p.paused = hold;
                let (s, why) = if hold {
                    (RunState::Paused, "held before launch")
                } else {
                    (RunState::Queued, "released")
                };
                let mut st = RunStatus::new(s);
                st.reason = Some(why.into());
                self.set_state(&p.id, &st, why);
            }
            if p.mem_mb.is_none() {
                match self.estimate(&mut p, &mut checking) {
                    Ok(m) => p.mem_mb = m,
                    Err(reason) => {
                        self.fail_pending(&p.id, reason);
                        continue;
                    }
                }
            }
            if let Some(m) = p.mem_mb {
                if let Err(reason) = self.refusal(&p, m) {
                    self.fail_pending(&p.id, reason);
                    continue;
                }
            }
            keep.push(p);
        }
        self.pending = keep;
    }

    /// The memory need of a pending run: declared, or from the cell count of a pre-launch check
    /// (`Ok(None)` while the check runs; one check at a time).
    fn estimate(&self, p: &mut Pending, checking: &mut bool) -> Result<Option<u64>, String> {
        if let Some(m) = p.settings.mem_mb {
            return Ok(Some(m));
        }
        let dir = self.dir(&p.id);
        let Some(c) = p.check.as_mut() else {
            if *checking {
                return Ok(None);
            }
            let out = std::fs::File::create(dir.join(extra::CHECK)).map_err(|e| format!("check.json: {e}"))?;
            let mut cmd = self.solver_cmd(&p.rec, &p.settings, &dir);
            cmd.args(["--check-only", "--status-json"]).stdout(out);
            sys::no_window(&mut cmd);
            let child = cmd
                .spawn()
                .map_err(|e| format!("cannot start the solver {}: {e}", self.solver.display()))?;
            p.check = Some(child);
            *checking = true;
            return Ok(None);
        };
        let Some(code) = c.try_wait().map_err(|e| format!("pre-launch check: {e}"))? else {
            return Ok(None);
        };
        p.check = None;
        *checking = false;
        let check_file = dir.join(extra::CHECK);
        let ev: Value = std::fs::read_to_string(&check_file)
            .ok()
            .and_then(|t| t.lines().find_map(|l| serde_json::from_str(l).ok()))
            .unwrap_or(Value::Null);
        if code.success() && ev["ok"] == true {
            let cells = ev["cells"].as_u64().unwrap_or(0);
            let mb = self.cfg.mem_base_mb + cells as f64 * self.cfg.mem_kb_per_cell / 1024.0;
            note(
                "event",
                self.store.append_event(
                    &p.id,
                    event_line(
                        "estimate",
                        json!({"cells": cells, "mem_mb": mb.ceil(),
                               "rule": format!("{} MB + {} KB per cell", self.cfg.mem_base_mb, self.cfg.mem_kb_per_cell)}),
                    ),
                ),
            );
            return Ok(Some(mb.ceil() as u64));
        }
        let located = ev["diagnostics"]
            .as_array()
            .map(|ds| {
                ds.iter()
                    .map(|d| {
                        format!(
                            "{}: {}",
                            d["path"].as_str().unwrap_or(""),
                            d["message"].as_str().unwrap_or("")
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("; ")
            })
            .filter(|s| !s.is_empty())
            .or_else(|| read_last_line(&dir.join("log.txt")))
            .unwrap_or_default();
        Err(format!(
            "pre-launch check failed (exit {}): {located} (see {} and {})",
            code.code().unwrap_or(-1),
            check_file.display(),
            dir.join("log.txt").display()
        ))
    }

    fn refusal(&self, p: &Pending, mem: u64) -> Result<(), String> {
        let s = &p.settings;
        if s.threads == 0 || s.threads > self.cfg.cpu_threads {
            return Err(format!(
                "CPU budget: the run asks for threads = {} (run.json settings.threads), the whole budget is cpu_threads = {} ({})",
                s.threads, self.cfg.cpu_threads, self.cfg_src
            ));
        }
        if mem > self.cfg.mem_budget_mb {
            let how = match s.mem_mb {
                Some(_) => "declared as settings.mem_mb in run.json".to_string(),
                None => format!(
                    "estimated as mem_base_mb + cells x mem_kb_per_cell, see the estimate event in {}",
                    self.dir(&p.id).join("events.jsonl").display()
                ),
            };
            return Err(format!(
                "memory budget: the run needs {mem} MB ({how}), more than the whole budget mem_budget_mb = {} MB ({})",
                self.cfg.mem_budget_mb, self.cfg_src
            ));
        }
        Ok(())
    }

    /// The directory the case's relative paths resolve against: `case_dir` of `run.json`, else (a run created by
    /// `rcfd-project`, whose `case` is project-relative) the folder of the case in the project.
    fn case_dir(&self, rec: &RunRecord) -> PathBuf {
        match &rec.case_dir {
            Some(d) => PathBuf::from(d),
            None => {
                let case = self.store.project().join(&rec.case);
                case.parent()
                    .map_or_else(|| self.store.project().to_path_buf(), Path::to_path_buf)
            }
        }
    }

    fn solver_cmd(&self, rec: &RunRecord, settings: &RunSettings, dir: &Path) -> Command {
        let mut cmd = Command::new(&self.solver);
        cmd.arg("run")
            .arg(dir.join("case.snapshot"))
            .arg("--case-dir")
            .arg(self.case_dir(rec));
        for s in &settings.set {
            cmd.arg("--set").arg(s);
        }
        let log = OpenOptions::new().create(true).append(true).open(dir.join("log.txt"));
        cmd.stdin(Stdio::null())
            .stderr(log.map_or_else(|_| Stdio::null(), Stdio::from))
            .current_dir(dir);
        cmd
    }

    /// Launch ready runs in (priority desc, submission) order while the head of the queue fits; the queue
    /// is strict (no smaller run overtakes a waiting head).
    fn schedule(&mut self) {
        if self.pending.iter().any(|p| !p.paused && p.mem_mb.is_none()) {
            return; // order among all queued runs needs every estimate
        }
        let mut order: Vec<usize> = (0..self.pending.len()).filter(|&i| !self.pending[i].paused).collect();
        order.sort_by(|&a, &b| {
            let (x, y) = (&self.pending[a], &self.pending[b]);
            (y.settings.priority, &x.rec.created, &x.id).cmp(&(x.settings.priority, &y.rec.created, &y.id))
        });
        let mut launched = Vec::new();
        for i in order {
            let p = &self.pending[i];
            let mem = p.mem_mb.unwrap_or(0);
            let threads = p.settings.threads;
            let used_t: usize = self.active.iter().map(|a| a.threads).sum();
            let used_m: u64 = self.active.iter().map(|a| a.mem_mb).sum();
            if self.active.len() >= self.cfg.max_runs
                || used_t + threads > self.cfg.cpu_threads
                || used_m + mem > self.cfg.mem_budget_mb
            {
                break;
            }
            launched.push(i);
            self.launch(i);
        }
        launched.sort_unstable();
        for i in launched.into_iter().rev() {
            self.pending.remove(i);
        }
    }

    /// The restart file a run with `continue_from` starts from (ADR 0200 S3): `runs/<id>/output/restart.rcfd` of a run of
    /// this project that has ended; the reason when there is none.
    fn continue_file(&self, launch: &LaunchSettings) -> Result<Option<PathBuf>, String> {
        let Some(src) = &launch.continue_from else {
            return Ok(None);
        };
        if !rcfd_project::runid::is_run_id(src) {
            return Err(format!(
                "continue_from: '{src}' is not a run id (run.json settings.continue_from)"
            ));
        }
        let st = self
            .store
            .read_status(src)
            .map_err(|e| format!("continue_from: run {src}: {e}"))?;
        if !st.state.is_terminal() {
            return Err(format!(
                "continue_from: run {src} has not ended (it is {}); a run continues only from the restart file of an ended run",
                crate::client::state_name(st.state)
            ));
        }
        let file = self.dir(src).join(extra::OUTPUT).join("restart.rcfd");
        if !file.is_file() {
            return Err(format!(
                "continue_from: run {src} left no restart file {}",
                file.display()
            ));
        }
        Ok(Some(file))
    }

    fn launch(&mut self, i: usize) {
        let (id, rec, settings, mem_mb) = {
            let p = &self.pending[i];
            (p.id.clone(), p.rec.clone(), p.settings.clone(), p.mem_mb.unwrap_or(0))
        };
        // ADR 0200 S3: the launch settings, checked before anything is started
        let started = LaunchSettings::of(&rec).and_then(|l| Ok((self.continue_file(&l)?, l.max_steps)));
        let (continue_file, max_steps) = match started {
            Ok(s) => s,
            Err(why) => return self.fail_pending(&id, why),
        };
        let dir = self.dir(&id);
        let mut cmd = self.solver_cmd(&rec, &settings, &dir);
        cmd.arg("--out")
            .arg(dir.join(extra::OUTPUT))
            .arg("--status-file")
            .arg(dir.join(extra::SOLVER_STREAM))
            .args(["--status-every", "1", "--quiet", "--threads"])
            .arg(settings.threads.to_string())
            .arg("--stop-file")
            .arg(dir.join(extra::CANCEL))
            .arg("--pause-file")
            .arg(dir.join(extra::PAUSE))
            // ADR 0130 D3, D5: the field store at runs/<id>/fields (used only when the case turns it on), and the
            // report values of every step in the status stream
            .arg("--store-dir")
            .arg(dir.join(extra::FIELDS))
            .arg("--status-monitors");
        if let Some(f) = &continue_file {
            cmd.arg("--continue-from").arg(f);
        }
        if let Some(n) = max_steps {
            cmd.arg("--max-steps").arg(n.to_string());
        }
        let log = OpenOptions::new().create(true).append(true).open(dir.join("log.txt"));
        cmd.stdout(log.map_or_else(|_| Stdio::null(), Stdio::from));
        sys::no_window(&mut cmd);
        // ADR 0229 D3: the solver holds its liveness file as its standard input for as long as it lives (Windows; the
        // null device stays where the file cannot be made)
        if let Ok(f) = sys::alive_file(&self.alive(&id)) {
            cmd.stdin(Stdio::from(f));
        }
        match cmd.spawn() {
            Ok(mut child) => {
                drop(cmd); // the runner keeps no handle on log.txt or on the liveness file
                self.launch_seq += 1;
                let pid = child.id();
                let mut st = RunStatus::new(RunState::Running);
                st.pid = Some(pid);
                // the launched event first: a later runner adopts a run that has one instead of launching it again
                let recorded = self
                    .store
                    .append_event(
                        &id,
                        event_line(
                            "launched",
                            json!({"pid": pid, "launch_seq": self.launch_seq,
                                   "threads": settings.threads, "mem_mb": mem_mb}),
                        ),
                    )
                    .map_err(|e| (dir.join("events.jsonl"), e))
                    .and_then(|()| {
                        self.store
                            .write_status(&id, &st)
                            .map_err(|e| (dir.join("status.json"), e))
                    });
                if let Err((file, e)) = recorded {
                    let _ = child.kill();
                    let _ = child.wait();
                    self.fail_pending(
                        &id,
                        format!(
                            "cannot record the launched solver {pid} in {}: {e}; the solver was stopped so that no second one starts",
                            file.display()
                        ),
                    );
                    return;
                }
                let mut launched = state_event(Some(RunState::Queued), st.state, "launched");
                launched["pid"] = json!(pid);
                note("event", self.store.append_event(&id, launched));
                self.active.push(Active {
                    id,
                    child: Some(child),
                    pid,
                    threads: settings.threads,
                    mem_mb,
                    offset: 0,
                    step: 0,
                    time: 0.0,
                    max_seq: 0,
                    finish: None,
                    paused: false,
                    cancel_at: None,
                    killed_for_cancel: false,
                    next_probe: Instant::now(),
                    misses: 0,
                    append_failed: false,
                });
            }
            Err(e) => {
                let why = format!("cannot start the solver {}: {e}", self.solver.display());
                self.fail_pending(&id, why);
            }
        }
    }

    /// Forward complete new lines of the solver stream (at most [`DRAIN_CHUNK`] bytes per call): iterations to
    /// `residuals.jsonl`, everything else to `events.jsonl`. Returns whether the step moved.
    fn drain(&self, a: &mut Active) -> bool {
        let path = self.dir(&a.id).join(extra::SOLVER_STREAM);
        let Ok(mut f) = std::fs::File::open(&path) else {
            return false;
        };
        let mut buf = Vec::new();
        if f.seek(SeekFrom::Start(a.offset)).is_err() || f.take(DRAIN_CHUNK).read_to_end(&mut buf).is_err() {
            return false;
        }
        let Some(end) = buf.iter().rposition(|&b| b == b'\n') else {
            if buf.len() as u64 == DRAIN_CHUNK {
                // a full chunk without a newline is no status line: skip it, the rest of that line fails to parse
                note(
                    "event",
                    self.store.append_event(
                        &a.id,
                        event_line("skipped_line", json!({"offset": a.offset, "bytes": buf.len()})),
                    ),
                );
                a.offset += DRAIN_CHUNK;
            }
            return false;
        };
        // ADR 0229 D5: a cursor moves only after its append. The step and the time after the chunk's residual lines,
        // `max_seq` after each solver event (the first failed one stops the forwarding of the later ones), the offset
        // only when every append succeeded; a chunk read again skips what was written, by step and by `seq`.
        let next = a.offset + end as u64 + 1;
        let (mut step, mut time) = (a.step, a.time);
        let mut residuals = Vec::new();
        let mut failed: Option<(&str, std::io::Error)> = None;
        for line in String::from_utf8_lossy(&buf[..end]).lines() {
            let Ok(v) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            if v["event"] == "iteration" {
                let idx = v["index"].as_u64().unwrap_or(0);
                if idx > step {
                    residuals.push(residual_line(&v, idx));
                    step = idx;
                    time = v["time"].as_f64().unwrap_or(time);
                }
                continue;
            }
            if v["event"] == "finish" {
                a.finish = Some(v.clone());
            }
            let seq = v["seq"].as_u64().unwrap_or(0);
            if seq > a.max_seq && failed.is_none() {
                match self
                    .store
                    .append_event(&a.id, event_line("solver", json!({"solver_seq": seq, "solver": v})))
                {
                    Ok(()) => a.max_seq = seq,
                    Err(e) => failed = Some(("event", e)),
                }
            }
        }
        let mut moved = false;
        match self.store.append_residuals(&a.id, &residuals) {
            Ok(()) => {
                moved = step > a.step;
                a.step = step;
                a.time = time;
            }
            Err(e) => failed = failed.or(Some(("residuals", e))),
        }
        match failed {
            None => {
                if a.append_failed {
                    log(&format!("run {}: the solver stream is forwarded again", a.id));
                }
                a.append_failed = false;
                a.offset = next;
            }
            Some((what, e)) => {
                if !a.append_failed {
                    log(&format!(
                        "run {}: {what}: {e}; the solver stream is read again from byte {} on every tick",
                        a.id, a.offset
                    ));
                }
                a.append_failed = true;
            }
        }
        moved
    }

    fn status_of(a: &Active) -> RunStatus {
        let mut st = RunStatus::new(if a.paused { RunState::Paused } else { RunState::Running });
        st.step = a.step;
        st.time = a.time;
        st.pid = Some(a.pid);
        st
    }

    fn tick_active(&mut self) {
        self.retry_status();
        let active = std::mem::take(&mut self.active);
        let mut keep = Vec::new();
        for mut a in active {
            // exit first, then drain: every line written before the exit is read
            let exited: Option<Option<i32>> = match a.child.as_mut() {
                Some(c) => match c.try_wait() {
                    Ok(Some(s)) => Some(s.code()),
                    Ok(None) => None,
                    Err(_) => Some(None),
                },
                None if Instant::now() >= a.next_probe => {
                    a.next_probe = Instant::now() + Duration::from_millis(self.cfg.adopt_probe_ms);
                    // ADR 0229 D3: the liveness file answers for the solver itself; a run without one is probed by
                    // its pid and name, as before
                    let alive = (self.held)(&self.alive(&a.id)).unwrap_or_else(|| {
                        let name = self.solver.file_stem().map_or("rcfd".into(), |s| s.to_string_lossy());
                        (self.probe)(a.pid, &name)
                    });
                    a.misses = next_misses(a.misses, alive);
                    (a.misses >= 2).then_some(None)
                }
                None => None,
            };
            let mut dirty = self.drain(&mut a);
            if let Some(code) = exited {
                // everything the solver wrote before it exited, however much that is
                let mut at = None;
                while at != Some(a.offset) {
                    at = Some(a.offset);
                    self.drain(&mut a);
                }
                self.finalize(&a, code);
                continue;
            }
            let (hold, acked) = (self.has(&a.id, extra::PAUSE), self.has(&a.id, extra::PAUSE_ACK));
            if hold && acked && !a.paused {
                // lines written between the drain above and the ack belong before the hold
                self.drain(&mut a);
                a.paused = true;
                self.set_state(&a.id, &Self::status_of(&a), "held by the solver between two steps");
                dirty = false;
            } else if !hold && a.paused {
                a.paused = false;
                self.set_state(&a.id, &Self::status_of(&a), "resumed");
                dirty = false;
            }
            if self.has(&a.id, extra::CANCEL) {
                let grace = Duration::from_millis(self.cfg.cancel_grace_ms);
                match a.cancel_at {
                    None => {
                        a.cancel_at = Some(Instant::now());
                        note(
                            "event",
                            self.store
                                .append_event(&a.id, event_line("cancel_requested", json!({}))),
                        );
                    }
                    // ADR 0229 D3: an adopted solver is killed by its pid only while its liveness file does not say
                    // that it is gone (a pid that the system gave to another process is not killed); the probe ends
                    // such a run
                    Some(t)
                        if t.elapsed() > grace
                            && !a.killed_for_cancel
                            && (a.child.is_some() || (self.held)(&self.alive(&a.id)) != Some(false)) =>
                    {
                        a.killed_for_cancel = (self.kill)(a.pid);
                        note(
                            "event",
                            self.store.append_event(
                                &a.id,
                                event_line("killed", json!({"pid": a.pid, "why": "cancel grace elapsed"})),
                            ),
                        );
                    }
                    _ => {}
                }
            }
            if dirty {
                self.put_status(&a.id, &Self::status_of(&a));
            }
            keep.push(a);
        }
        self.active.extend(keep);
    }

    fn finalize(&self, a: &Active, code: Option<i32>) {
        let dir = self.dir(&a.id);
        let log = dir.join("log.txt");
        let cancel = self.has(&a.id, extra::CANCEL);
        let fin = a.finish.as_ref();
        let fin_code = fin.and_then(|f| f["exit_code"].as_i64()).map(|c| c as i32);
        let (state, reason) = match fin {
            Some(f) if code.is_some() && code != fin_code => (
                RunState::Failed,
                format!(
                    "the solver reported '{}' (exit {}) but the process exited with {}: {} (log: {})",
                    f["state"].as_str().unwrap_or("?"),
                    fin_code.unwrap_or(-1),
                    code.unwrap_or(-1),
                    read_last_line(&log).unwrap_or_default(),
                    log.display()
                ),
            ),
            Some(f) if f["state"] == "interrupted" && cancel => (
                RunState::Cancelled,
                format!("cancelled on request; the solver stopped cleanly after step {}", a.step),
            ),
            Some(f) if matches!(fin_code, Some(0) | Some(5)) => (
                RunState::Finished,
                format!("{}: {}", f["state"].as_str().unwrap_or("?"), f["reason"].as_str().unwrap_or("")),
            ),
            Some(f) => (
                RunState::Failed,
                format!(
                    "the solver ended in state '{}' (exit {}): {} (log: {})",
                    f["state"].as_str().unwrap_or("?"),
                    fin_code.unwrap_or(-1),
                    f["reason"].as_str().unwrap_or(""),
                    log.display()
                ),
            ),
            None if a.killed_for_cancel => (
                RunState::Cancelled,
                format!(
                    "cancelled on request; the solver did not stop within cancel_grace_ms = {} and was killed after step {}",
                    self.cfg.cancel_grace_ms, a.step
                ),
            ),
            None => (
                RunState::Failed,
                format!(
                    "solver process {} ended without a finish event ({}); last step {} at time {}; last log line: '{}' (log: {})",
                    a.pid,
                    code.map_or_else(
                        || "found not running by the liveness probe".to_string(),
                        |c| format!("exit code {c}")
                    ),
                    a.step,
                    a.time,
                    read_last_line(&log).unwrap_or_default(),
                    log.display()
                ),
            ),
        };
        let mut st = RunStatus::new(state);
        st.step = a.step;
        st.time = a.time;
        st.reason = Some(reason.clone());
        st.exit_code = fin_code.or(code);
        let last = read_last_line(&dir.join("residuals.jsonl")).and_then(|l| serde_json::from_str::<Value>(&l).ok());
        // `reports` is additive (ADR 0135 amendment 3); the other keys are as before
        let reports = report_values(&dir.join(extra::OUTPUT).join("monitors.json"), last.as_ref());
        let metrics = json!({
            "state": state, "reason": reason, "exit_code": st.exit_code, "steps": a.step, "time": a.time,
            "solver_finish": fin, "last_residuals": last, "reports": reports,
        });
        note("metrics", self.store.write_metrics(&a.id, &metrics));
        self.set_state(&a.id, &st, &reason);
        self.clear_requests(&a.id);
        // the solver is gone: its liveness file is free (best effort; an empty file is left when it is not yet)
        let _ = std::fs::remove_file(self.alive(&a.id));
    }

    fn shutdown(&mut self) {
        self.retry_status();
        for p in &mut self.pending {
            if let Some(mut c) = p.check.take() {
                let _ = c.kill();
                let _ = c.wait();
            }
        }
        for a in &self.active {
            note(
                "event",
                self.store.append_event(
                    &a.id,
                    event_line(
                        "runner_shutdown",
                        json!({"pid": a.pid, "note": "the solver keeps running; the next runner adopts it"}),
                    ),
                ),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::records::RunSettings;
    use std::io;

    const CASE: &str = "title = \"runner unit\"\n[mesh]\ngenerator = \"cavity_2d\"\nnx = 12\nny = 12\n[physics]\nnu = 0.01\n\
        [boundaries.lid]\nvelocity = \"fixed\"\nvelocity_value = [1.0, 0.0, 0.0]\n[boundaries.walls]\nvelocity = \"fixed\"\n\
        [solver]\nalgorithm = \"simplec\"\nconvection = \"upwind\"\nmax_iterations = 400\ntolerance_u = 1e-14\ntolerance_p = 1e-14\n";

    /// ADR 0160 R1: a fixed run's line is byte-identical to the construction before ADR 0160 (copied here); an adaptive
    /// event adds exactly `correctors` and `corrector_cap`.
    #[test]
    fn r1_residual_line_adds_the_corrector_count_only_when_the_event_has_it() {
        let before = |v: &Value, idx: u64| -> Value {
            json!({"step": idx, "time": v["time"],
                "residuals": {"u": v["res_u"], "p": v["res_p"], "continuity": v["continuity"]},
                "monitors": v.get("monitors").filter(|m| m.is_object()).cloned().unwrap_or_else(|| json!({}))})
        };
        let events = [
            json!({"event": "iteration", "index": 3, "time": 0.03, "res_u": 1e-3, "res_p": 2e-4, "continuity": 1e-12,
                   "done": false, "monitors": {"lid_drag": 0.31}}),
            json!({"event": "iteration", "index": 4, "time": 0.04, "res_u": 1e-3, "res_p": 2e-4, "continuity": 1e-12,
                   "done": true}),
            json!({"event": "iteration", "index": 5, "time": 0.05, "res_u": null, "res_p": null, "continuity": null,
                   "done": true}),
        ];
        for e in &events {
            let idx = e["index"].as_u64().unwrap();
            assert_eq!(residual_line(e, idx).to_string(), before(e, idx).to_string());
        }
        let mut a = events[0].clone();
        a["correctors"] = json!(5);
        a["corrector_cap"] = json!(0);
        let line = residual_line(&a, 3);
        let mut want = before(&events[0], 3);
        want["correctors"] = json!(5);
        want["corrector_cap"] = json!(0);
        assert_eq!(line.to_string(), want.to_string());
        assert_eq!(line.as_object().unwrap().len(), 6);
    }

    /// `rcfd` of the same target directory (`target/<profile>/deps/<test>` -> `target/<profile>/rcfd`).
    fn rcfd() -> PathBuf {
        let exe = std::env::current_exe().unwrap();
        let p = exe
            .parent()
            .and_then(Path::parent)
            .unwrap()
            .join(format!("rcfd{}", std::env::consts::EXE_SUFFIX));
        assert!(p.exists(), "{} missing (cargo build -p rcfd-cli)", p.display());
        p
    }

    fn project(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("rcfd_runner_unit_{}_{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("cases")).unwrap();
        std::fs::write(d.join("cases/cavity.toml"), CASE).unwrap();
        d
    }

    fn runner<S: RunStore>(store: S) -> Runner<S> {
        let cfg = Config {
            max_runs: 1,
            cpu_threads: 8,
            mem_budget_mb: 100_000,
            poll_ms: 50,
            adopt_probe_ms: 0,
            ..Config::default()
        };
        let mut r = Runner::new(store, cfg, "test".into());
        r.solver = rcfd();
        r
    }

    fn tick<S: RunStore>(r: &mut Runner<S>) {
        r.scan();
        r.tick_pending();
        r.tick_active();
        r.schedule();
    }

    fn launched_pids(store: &FsStore, id: &str) -> Vec<u32> {
        std::fs::read_to_string(store.run_dir(id).join("events.jsonl"))
            .unwrap_or_default()
            .lines()
            .filter_map(|l| serde_json::from_str::<Value>(l).ok())
            .filter(|v| v["event"] == "launched")
            .filter_map(|v| v["pid"].as_u64().map(|p| p as u32))
            .collect()
    }

    /// A store whose write of a `running` status fails (ADR 0128 finding 3).
    struct FailRunning(FsStore);

    impl RunStore for FailRunning {
        fn project(&self) -> &Path {
            self.0.project()
        }
        fn list_runs(&self) -> io::Result<Vec<String>> {
            self.0.list_runs()
        }
        fn create_run(&self, case: &Path, settings: &RunSettings, v: Option<String>) -> io::Result<String> {
            self.0.create_run(case, settings, v)
        }
        fn read_record(&self, id: &str) -> io::Result<RunRecord> {
            self.0.read_record(id)
        }
        fn read_status(&self, id: &str) -> io::Result<RunStatus> {
            self.0.read_status(id)
        }
        fn write_status(&self, id: &str, status: &RunStatus) -> io::Result<()> {
            if status.state == RunState::Running {
                return Err(io::Error::other("injected write failure"));
            }
            self.0.write_status(id, status)
        }
        fn append_event(&self, id: &str, event: Value) -> io::Result<()> {
            self.0.append_event(id, event)
        }
        fn append_residual(&self, id: &str, line: &Value) -> io::Result<()> {
            self.0.append_residual(id, line)
        }
        fn write_metrics(&self, id: &str, metrics: &Value) -> io::Result<()> {
            self.0.write_metrics(id, metrics)
        }
    }

    thread_local! {
        /// Scripted probe answers of this test thread (alive when empty).
        static PROBE: std::cell::RefCell<std::collections::VecDeque<bool>> = Default::default();
    }

    fn scripted(_: u32, _: &str) -> bool {
        PROBE.with(|q| q.borrow_mut().pop_front().unwrap_or(true))
    }

    /// F4: an adopted run is finalised after two consecutive missed probes, not after one.
    #[test]
    fn f04_one_missed_probe_does_not_finalise_two_do() {
        let settings = RunSettings {
            mem_mb: Some(10),
            ..RunSettings::default()
        };
        let state_after = |answers: &[bool]| {
            // one project per script: a runner adopts every running run of its project
            let dir = project(&format!("f04-{answers:?}"));
            let fs = FsStore::new(&dir);
            let id = fs.create_run(&dir.join("cases/cavity.toml"), &settings, None).unwrap();
            let mut st = RunStatus::new(RunState::Running);
            st.pid = Some(u32::MAX - 7); // never probed for real: the probe is scripted
            fs.write_status(&id, &st).unwrap();
            let mut r = runner(fs.clone());
            r.probe = scripted;
            r.scan();
            for &alive in answers {
                PROBE.with(|q| q.borrow_mut().push_back(alive));
                r.tick_active();
            }
            let state = fs.read_status(&id).unwrap().state;
            let _ = std::fs::remove_dir_all(&dir);
            state
        };
        let cases = [
            (vec![false], RunState::Running),
            (vec![false, true, false], RunState::Running),
            (vec![false, false], RunState::Failed),
            (vec![true, false, false], RunState::Failed),
        ];
        let got: Vec<RunState> = cases.iter().map(|(a, _)| state_after(a)).collect();
        eprintln!(
            "F4 probe scripts {:?} -> {got:?}",
            cases.iter().map(|c| &c.0).collect::<Vec<_>>()
        );
        assert_eq!(got, cases.iter().map(|c| c.1).collect::<Vec<_>>());
    }

    /// F6: a 100 MB solver stream of an adopted run is drained in chunks of at most 4 MiB, every line forwarded.
    #[test]
    fn f06_large_status_file_is_drained_in_bounded_chunks() {
        use std::io::Write;
        let dir = project("f06");
        let fs = FsStore::new(&dir);
        let settings = RunSettings {
            mem_mb: Some(10),
            ..RunSettings::default()
        };
        let id = fs.create_run(&dir.join("cases/cavity.toml"), &settings, None).unwrap();
        let mut st = RunStatus::new(RunState::Running);
        st.pid = Some(u32::MAX - 7);
        fs.write_status(&id, &st).unwrap();
        let pad = "x".repeat(900);
        let mut w = std::io::BufWriter::new(std::fs::File::create(fs.run_dir(&id).join(extra::SOLVER_STREAM)).unwrap());
        let (mut bytes, mut lines) = (0u64, 0u64);
        while bytes < 100_000_000 {
            lines += 1;
            let l = format!(
                "{{\"event\":\"iteration\",\"index\":{lines},\"time\":{lines}.0,\"res_u\":1e-3,\"res_p\":1e-3,\"continuity\":1e-3,\"pad\":\"{pad}\"}}
"
            );
            w.write_all(l.as_bytes()).unwrap();
            bytes += l.len() as u64;
        }
        drop(w);
        let mut r = runner(fs.clone());
        r.probe = scripted; // always alive
        r.scan();
        let (mut ticks, mut max_step) = (0, 0u64);
        while r.active[0].offset < bytes && ticks < 1000 {
            let before = r.active[0].offset;
            r.tick_active();
            max_step = max_step.max(r.active[0].offset - before);
            ticks += 1;
        }
        // counted streaming, so the count adds no memory to the measured peak
        let residuals = std::io::BufRead::lines(std::io::BufReader::new(
            std::fs::File::open(fs.run_dir(&id).join("residuals.jsonl")).unwrap(),
        ))
        .count() as u64;
        eprintln!("F6: {bytes} bytes, {lines} lines drained in {ticks} ticks, largest tick {max_step} bytes, {residuals} residual lines");
        assert!(ticks >= 24, "drained in {ticks} ticks");
        assert!(max_step <= 4 << 20, "a tick read {max_step} bytes");
        assert_eq!(residuals, lines);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// F3: a failed `running` write kills the solver and fails the run; a later runner launches nothing.
    #[test]
    fn f03_failed_running_write_kills_the_solver_and_fails_the_run() {
        let dir = project("f03");
        let fs = FsStore::new(&dir);
        let settings = RunSettings {
            mem_mb: Some(10),
            set: vec!["solver.max_iterations=100000".into()],
            ..RunSettings::default()
        };
        let id = fs.create_run(&dir.join("cases/cavity.toml"), &settings, None).unwrap();
        let mut r1 = runner(FailRunning(fs.clone()));
        for _ in 0..3 {
            tick(&mut r1);
        }
        let first = launched_pids(&fs, &id);
        let gone = first.first().is_some_and(|&p| {
            let t0 = Instant::now();
            while sys::pid_alive(p, "rcfd") && t0.elapsed() < Duration::from_secs(5) {
                std::thread::sleep(Duration::from_millis(50));
            }
            !sys::pid_alive(p, "rcfd")
        });
        let st1 = fs.read_status(&id).unwrap();
        let mut r2 = runner(fs.clone());
        for _ in 0..3 {
            tick(&mut r2);
        }
        let all = launched_pids(&fs, &id);
        let spawns = r1.launch_seq + r2.launch_seq;
        for &p in &all {
            if sys::pid_alive(p, "rcfd") {
                sys::kill(p);
            }
        }
        eprintln!(
            "F3: spawns {spawns}, launched events {all:?}, first solver gone {gone}, status after the first runner {:?}: {:?}",
            st1.state, st1.reason
        );
        assert_eq!(spawns, 1, "solver processes started");
        assert_eq!(all.len(), 1, "launched events");
        assert!(gone, "the solver of the failed write still runs");
        assert_eq!(st1.state, RunState::Failed);
        assert!(st1.reason.unwrap_or_default().contains("status.json"));
        std::thread::sleep(Duration::from_millis(200));
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ---------------------------------------------------------------- ADR 0229

    /// A store whose writes fail a set number of times (ADR 0229 W1-W3): terminal status writes, residual appends and
    /// appends of forwarded solver events.
    struct Flaky {
        fs: FsStore,
        terminal_status: std::cell::Cell<u32>,
        residuals: std::cell::Cell<u32>,
        solver_events: std::cell::Cell<u32>,
    }

    impl Flaky {
        fn new(fs: &FsStore) -> Flaky {
            Flaky {
                fs: fs.clone(),
                terminal_status: Default::default(),
                residuals: Default::default(),
                solver_events: Default::default(),
            }
        }

        fn fail(left: &std::cell::Cell<u32>) -> io::Result<()> {
            if left.get() > 0 {
                left.set(left.get() - 1);
                return Err(io::Error::other("injected write failure"));
            }
            Ok(())
        }
    }

    impl RunStore for Flaky {
        fn project(&self) -> &Path {
            self.fs.project()
        }
        fn list_runs(&self) -> io::Result<Vec<String>> {
            self.fs.list_runs()
        }
        fn create_run(&self, case: &Path, settings: &RunSettings, v: Option<String>) -> io::Result<String> {
            self.fs.create_run(case, settings, v)
        }
        fn read_record(&self, id: &str) -> io::Result<RunRecord> {
            self.fs.read_record(id)
        }
        fn read_status(&self, id: &str) -> io::Result<RunStatus> {
            self.fs.read_status(id)
        }
        fn write_status(&self, id: &str, status: &RunStatus) -> io::Result<()> {
            if status.state.is_terminal() {
                Flaky::fail(&self.terminal_status)?;
            }
            self.fs.write_status(id, status)
        }
        fn append_event(&self, id: &str, event: Value) -> io::Result<()> {
            if event_kind(&event) == Some("solver") {
                Flaky::fail(&self.solver_events)?;
            }
            self.fs.append_event(id, event)
        }
        fn append_residual(&self, id: &str, line: &Value) -> io::Result<()> {
            self.append_residuals(id, std::slice::from_ref(line))
        }
        fn append_residuals(&self, id: &str, lines: &[Value]) -> io::Result<()> {
            if !lines.is_empty() {
                Flaky::fail(&self.residuals)?;
            }
            self.fs.append_residuals(id, lines)
        }
        fn write_metrics(&self, id: &str, metrics: &Value) -> io::Result<()> {
            self.fs.write_metrics(id, metrics)
        }
    }

    thread_local! {
        /// The liveness file's answer where the platform has none (`sys::held` is `None` off Windows).
        static HELD: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
        /// The pids the kill function was called with on this test thread.
        static KILLED: std::cell::RefCell<Vec<u32>> = const { std::cell::RefCell::new(Vec::new()) };
    }

    /// The real liveness file's answer; where the platform gives none, the scripted one.
    fn held_or_scripted(p: &Path) -> Option<bool> {
        sys::held(p).or_else(|| HELD.with(|h| h.get()).filter(|_| p.exists()))
    }

    fn recorded_kill(pid: u32) -> bool {
        KILLED.with(|k| k.borrow_mut().push(pid));
        true
    }

    const ADOPTED_PID: u32 = u32::MAX - 7; // never probed or killed for real: both are scripted

    /// A project with one run left `running` with [`ADOPTED_PID`] by an earlier runner.
    fn adopted(tag: &str) -> (PathBuf, FsStore, String) {
        let dir = project(tag);
        let fs = FsStore::new(&dir);
        let settings = RunSettings {
            mem_mb: Some(10),
            ..RunSettings::default()
        };
        let id = fs.create_run(&dir.join("cases/cavity.toml"), &settings, None).unwrap();
        let mut st = RunStatus::new(RunState::Running);
        st.pid = Some(ADOPTED_PID);
        fs.write_status(&id, &st).unwrap();
        (dir, fs, id)
    }

    fn events_of(fs: &FsStore, id: &str) -> Vec<Value> {
        std::fs::read_to_string(fs.run_dir(id).join("events.jsonl"))
            .unwrap_or_default()
            .lines()
            .filter_map(|l| serde_json::from_str::<Value>(l).ok())
            .collect()
    }

    fn residual_steps(fs: &FsStore, id: &str) -> Vec<u64> {
        std::fs::read_to_string(fs.run_dir(id).join("residuals.jsonl"))
            .unwrap_or_default()
            .lines()
            .filter_map(|l| serde_json::from_str::<Value>(l).ok())
            .filter_map(|v| v["step"].as_u64())
            .collect()
    }

    /// ADR 0229 I2: an adopted run whose liveness file is not held is not killed by its pid when the cancel grace has
    /// passed, although the pid probe says alive (a recycled pid); it ends as found not running. Control: the same run
    /// without a liveness file is killed once, by its pid.
    #[test]
    fn i2_a_cancel_does_not_kill_a_pid_whose_liveness_file_is_free() {
        let outcome = |with_file: bool| {
            let (dir, fs, id) = adopted(if with_file { "i2-file" } else { "i2-legacy" });
            std::fs::write(fs.run_dir(&id).join(extra::CANCEL), b"").unwrap();
            if with_file {
                let alive = dot_dir(&dir).join(ALIVE_DIR).join(&id);
                std::fs::create_dir_all(alive.parent().unwrap()).unwrap();
                std::fs::write(&alive, b"").unwrap(); // present, held by nobody
            }
            HELD.with(|h| h.set(Some(false)));
            KILLED.with(|k| k.borrow_mut().clear());
            let mut r = runner(fs.clone());
            r.cfg.cancel_grace_ms = 0;
            r.cfg.adopt_probe_ms = 60_000; // one probe at adoption, the next when the test says
            r.probe = scripted; // alive: the pid belongs to some process named like the solver
            r.held = held_or_scripted;
            r.kill = recorded_kill;
            r.scan();
            r.tick_active(); // the first probe; the cancel request is seen
            std::thread::sleep(Duration::from_millis(5));
            r.tick_active(); // the grace (0) has passed
            let killed = KILLED.with(|k| k.borrow().clone());
            for _ in 0..2 {
                if let Some(a) = r.active.first_mut() {
                    a.next_probe = Instant::now();
                }
                r.tick_active();
            }
            let st = fs.read_status(&id).unwrap();
            let kill_events = events_of(&fs, &id)
                .iter()
                .filter(|v| event_kind(v) == Some("killed"))
                .count();
            let _ = std::fs::remove_dir_all(&dir);
            (killed, st.state, st.reason.unwrap_or_default(), kill_events)
        };
        let (killed, state, reason, kill_events) = outcome(true);
        let (c_killed, c_state, _, c_kill_events) = outcome(false);
        eprintln!(
            "I2: kill calls {killed:?}, state {state:?}, killed events {kill_events}, reason: {reason}; control (no liveness \
             file): kill calls {c_killed:?}, state {c_state:?}, killed events {c_kill_events}"
        );
        assert!(
            killed.is_empty(),
            "a pid whose liveness file is free was killed: {killed:?}"
        );
        assert_eq!(state, RunState::Failed);
        assert!(reason.contains("found not running"), "{reason}");
        assert_eq!(kill_events, 0);
        assert_eq!(c_killed, [ADOPTED_PID], "control: the run without a liveness file");
    }

    /// ADR 0229 I3: while the liveness file is held, the pid probe is not consulted, so a process listing that fails
    /// does not end the run.
    #[test]
    fn i3_a_held_liveness_file_keeps_the_run_whatever_the_pid_probe_says() {
        let (dir, fs, id) = adopted("i3");
        let alive = dot_dir(&dir).join(ALIVE_DIR).join(&id);
        // Windows: held for real, as a solver holds it; elsewhere the answer is scripted for the file that is there
        let holder = sys::alive_file(&alive);
        if holder.is_err() {
            std::fs::create_dir_all(alive.parent().unwrap()).unwrap();
            std::fs::write(&alive, b"").unwrap();
        }
        HELD.with(|h| h.set(Some(true)));
        let mut r = runner(fs.clone());
        r.probe = scripted;
        r.held = held_or_scripted;
        r.scan();
        PROBE.with(|q| q.borrow_mut().extend([false; 4]));
        for _ in 0..4 {
            r.tick_active();
        }
        let left = PROBE.with(|q| q.borrow().len());
        let state = fs.read_status(&id).unwrap().state;
        PROBE.with(|q| q.borrow_mut().clear());
        eprintln!(
            "I3: state after 4 ticks {state:?}, scripted pid-probe answers left {left} of 4 (file held for real: {})",
            holder.is_ok()
        );
        assert_eq!(state, RunState::Running);
        assert_eq!(left, 4, "the pid probe was consulted");
        assert_eq!(r.active.len(), 1);
        drop(holder);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// ADR 0229 W1: a terminal status write that fails is written again on the following ticks; the state event is
    /// written once.
    #[test]
    fn w1_a_failed_terminal_status_write_is_repeated_until_it_succeeds() {
        let (dir, fs, id) = adopted("w1");
        let store = Flaky::new(&fs);
        store.terminal_status.set(2);
        let mut r = runner(store);
        r.probe = scripted;
        PROBE.with(|q| q.borrow_mut().extend([false, false]));
        r.scan();
        r.tick_active();
        r.tick_active(); // the second missed probe: finalized, the status write fails
        let at_end = fs.read_status(&id).unwrap().state;
        r.tick_active(); // repeated, fails again
        let after_1 = fs.read_status(&id).unwrap().state;
        r.tick_active(); // repeated, written
        let after_2 = fs.read_status(&id).unwrap().state;
        let kept = r.unwritten.borrow().len();
        let terminal_events = events_of(&fs, &id)
            .iter()
            .filter(|v| event_kind(v) == Some("state") && v["to"] == "failed")
            .count();
        eprintln!(
            "W1: status at the finalizing tick {at_end:?}, one tick later {after_1:?}, two ticks later {after_2:?}; \
             kept writes left {kept}; terminal state events {terminal_events}"
        );
        assert_eq!(
            at_end,
            RunState::Running,
            "set-up: the injected failure did not take effect"
        );
        assert_eq!(after_1, RunState::Running);
        assert_eq!(after_2, RunState::Failed);
        assert_eq!(kept, 0);
        assert_eq!(terminal_events, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The stream of W2 and W3: a solver event, 5 iterations, a solver event.
    fn write_stream(fs: &FsStore, id: &str) {
        let mut text = String::from("{\"event\":\"warning\",\"seq\":1,\"message\":\"first\"}\n");
        for i in 1..=5 {
            text.push_str(&format!(
                "{{\"event\":\"iteration\",\"index\":{i},\"time\":{i}.0,\"res_u\":1e-3,\"res_p\":1e-3,\"continuity\":1e-3}}\n"
            ));
        }
        text.push_str("{\"event\":\"warning\",\"seq\":2,\"message\":\"second\"}\n");
        std::fs::write(fs.run_dir(id).join(extra::SOLVER_STREAM), text).unwrap();
    }

    fn solver_seqs(fs: &FsStore, id: &str) -> Vec<u64> {
        events_of(fs, id)
            .iter()
            .filter(|v| event_kind(v) == Some("solver"))
            .filter_map(|v| v["solver_seq"].as_u64())
            .collect()
    }

    /// ADR 0229 W2: a failed residual append leaves the offset where it was; the chunk is read again and every
    /// residual line and solver event is in its file exactly once.
    #[test]
    fn w2_a_failed_residual_append_loses_no_line_and_doubles_none() {
        let (dir, fs, id) = adopted("w2");
        write_stream(&fs, &id);
        let store = Flaky::new(&fs);
        store.residuals.set(1);
        let mut r = runner(store);
        r.probe = scripted; // alive
        r.scan();
        r.tick_active();
        let (steps_1, offset_1) = (residual_steps(&fs, &id), r.active[0].offset);
        r.tick_active();
        let (steps_2, offset_2, seqs) = (residual_steps(&fs, &id), r.active[0].offset, solver_seqs(&fs, &id));
        eprintln!(
            "W2: after tick 1 residual steps {steps_1:?}, offset {offset_1}; after tick 2 steps {steps_2:?}, offset \
             {offset_2}, solver events {seqs:?}"
        );
        assert!(steps_1.is_empty(), "{steps_1:?}");
        assert_eq!(offset_1, 0);
        assert_eq!(steps_2, [1, 2, 3, 4, 5]);
        assert_eq!(seqs, [1, 2]);
        assert!(offset_2 > 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// ADR 0229 W3: a failed solver-event append leaves the offset where it was; the residual lines written in that
    /// tick are not written again, and the events arrive in stream order.
    #[test]
    fn w3_a_failed_event_append_loses_no_event_and_doubles_no_residual() {
        let (dir, fs, id) = adopted("w3");
        write_stream(&fs, &id);
        let store = Flaky::new(&fs);
        store.solver_events.set(1);
        let mut r = runner(store);
        r.probe = scripted; // alive
        r.scan();
        r.tick_active();
        let (steps_1, offset_1, seqs_1) = (residual_steps(&fs, &id), r.active[0].offset, solver_seqs(&fs, &id));
        r.tick_active();
        let (steps_2, seqs_2) = (residual_steps(&fs, &id), solver_seqs(&fs, &id));
        eprintln!(
            "W3: after tick 1 residual steps {steps_1:?}, offset {offset_1}, solver events {seqs_1:?}; after tick 2 steps \
             {steps_2:?}, solver events {seqs_2:?}"
        );
        assert_eq!(steps_1, [1, 2, 3, 4, 5]);
        assert_eq!(offset_1, 0);
        assert_eq!(steps_2, [1, 2, 3, 4, 5]);
        assert_eq!(seqs_2, [1, 2]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
