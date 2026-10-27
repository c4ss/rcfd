//! The runner process (`rcfd-runner serve`): one per project, holding `<project>/.rcfd/runner.lock`.
//!
//! The project folder is the queue: a client creates `runs/<id>/` with status `queued` and the runner, polling
//! every `poll_ms`, launches queued runs in priority order while the slot, thread and memory budgets allow.
//! Each run is an `rcfd run` child process whose output goes to files (never pipes), so the solver outlives
//! the runner and a later runner adopts it. Pause and cancel are request files in the run folder that the
//! solver itself watches (`--pause-file`, `--stop-file`). ADR 0123.

use crate::records::{self, RunRecord, RunState, RunStatus};
use crate::store::{extra, FsStore, RunStore};
use crate::sys;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

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
}

struct Pending {
    id: String,
    rec: RunRecord,
    paused: bool,
    mem_mb: Option<u64>,
    check: Option<Child>,
}

struct Runner {
    store: FsStore,
    cfg: Config,
    cfg_src: String,
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
    let mut r = Runner {
        store: FsStore::new(project),
        cfg,
        cfg_src,
        known: HashSet::new(),
        pending: Vec::new(),
        active: Vec::new(),
        launch_seq: 0,
    };
    let poll = Duration::from_millis(r.cfg.poll_ms);
    loop {
        if dot.join("shutdown").exists() {
            let _ = std::fs::remove_file(dot.join("shutdown"));
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
    Ok(0)
}

fn read_last_line(p: &Path) -> Option<String> {
    let text = std::fs::read_to_string(p).ok()?;
    text.lines().rev().find(|l| !l.trim().is_empty()).map(String::from)
}

impl Runner {
    fn dir(&self, id: &str) -> PathBuf {
        self.store.run_dir(id)
    }

    fn has(&self, id: &str, name: &str) -> bool {
        self.dir(id).join(name).exists()
    }

    fn set_state(&self, id: &str, st: &RunStatus, why: &str) {
        note("status", self.store.write_status(id, st));
        let mut ev = json!({"event": "state", "state": st.state, "reason": why});
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
                &json!({"state": state, "reason": reason, "steps": 0, "launched": false}),
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
            match (st.state, st.pid) {
                (RunState::Queued, _) | (RunState::Paused, None) => self.pending.push(Pending {
                    paused: st.state == RunState::Paused,
                    mem_mb: None,
                    check: None,
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

    fn adopt(&mut self, id: String, st: &RunStatus, pid: u32, rec: &RunRecord) {
        let dir = self.dir(&id);
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
            .find(|v| v["event"] == "launched")
            .and_then(|v| v["mem_mb"].as_u64())
            .unwrap_or(rec.settings.mem_mb.unwrap_or(0));
        let step = read_last_line(&dir.join("residuals.jsonl"))
            .and_then(|l| serde_json::from_str::<Value>(&l).ok())
            .and_then(|v| v["step"].as_u64())
            .unwrap_or(0);
        note(
            "event",
            self.store.append_event(
                &id,
                json!({"event": "adopted", "pid": pid, "note": "solver found by a new runner; probed for liveness"}),
            ),
        );
        self.active.push(Active {
            id,
            child: None,
            pid,
            threads: rec.settings.threads,
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
        if let Some(m) = p.rec.settings.mem_mb {
            return Ok(Some(m));
        }
        let dir = self.dir(&p.id);
        let Some(c) = p.check.as_mut() else {
            if *checking {
                return Ok(None);
            }
            let out = std::fs::File::create(dir.join(extra::CHECK)).map_err(|e| format!("check.json: {e}"))?;
            let mut cmd = self.solver_cmd(&p.rec, &dir);
            cmd.args(["--check-only", "--status-json"]).stdout(out);
            sys::no_window(&mut cmd);
            let child = cmd
                .spawn()
                .map_err(|e| format!("cannot start the solver {}: {e}", self.cfg.solver().display()))?;
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
                    json!({"event": "estimate", "cells": cells, "mem_mb": mb.ceil(),
                           "rule": format!("{} MB + {} KB per cell", self.cfg.mem_base_mb, self.cfg.mem_kb_per_cell)}),
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
        let s = &p.rec.settings;
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

    fn solver_cmd(&self, rec: &RunRecord, dir: &Path) -> Command {
        let mut cmd = Command::new(self.cfg.solver());
        cmd.arg("run")
            .arg(dir.join("case.snapshot"))
            .arg("--case-dir")
            .arg(&rec.case_dir);
        for s in &rec.settings.set {
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
            let (x, y) = (&self.pending[a].rec, &self.pending[b].rec);
            (y.settings.priority, &x.created, &x.id).cmp(&(x.settings.priority, &y.created, &y.id))
        });
        let mut launched = Vec::new();
        for i in order {
            let p = &self.pending[i];
            let mem = p.mem_mb.unwrap_or(0);
            let threads = p.rec.settings.threads;
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

    fn launch(&mut self, i: usize) {
        let (id, rec, mem_mb) = {
            let p = &self.pending[i];
            (p.id.clone(), p.rec.clone(), p.mem_mb.unwrap_or(0))
        };
        let dir = self.dir(&id);
        let mut cmd = self.solver_cmd(&rec, &dir);
        cmd.arg("--out")
            .arg(dir.join(extra::OUTPUT))
            .arg("--status-file")
            .arg(dir.join(extra::SOLVER_STREAM))
            .args(["--status-every", "1", "--quiet", "--threads"])
            .arg(rec.settings.threads.to_string())
            .arg("--stop-file")
            .arg(dir.join(extra::CANCEL))
            .arg("--pause-file")
            .arg(dir.join(extra::PAUSE));
        let log = OpenOptions::new().create(true).append(true).open(dir.join("log.txt"));
        cmd.stdout(log.map_or_else(|_| Stdio::null(), Stdio::from));
        sys::no_window(&mut cmd);
        match cmd.spawn() {
            Ok(child) => {
                drop(cmd); // the runner keeps no handle on log.txt
                self.launch_seq += 1;
                let pid = child.id();
                let mut st = RunStatus::new(RunState::Running);
                st.pid = Some(pid);
                self.set_state(&id, &st, "launched");
                note(
                    "event",
                    self.store.append_event(
                        &id,
                        json!({"event": "launched", "pid": pid, "launch_seq": self.launch_seq,
                               "threads": rec.settings.threads, "mem_mb": mem_mb}),
                    ),
                );
                self.active.push(Active {
                    id,
                    child: Some(child),
                    pid,
                    threads: rec.settings.threads,
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
                });
            }
            Err(e) => {
                let why = format!("cannot start the solver {}: {e}", self.cfg.solver().display());
                self.fail_pending(&id, why);
            }
        }
    }

    /// Forward complete new lines of the solver stream: iterations to `residuals.jsonl`, everything else
    /// to `events.jsonl`. Returns whether the step moved.
    fn drain(&self, a: &mut Active) -> bool {
        let path = self.dir(&a.id).join(extra::SOLVER_STREAM);
        let Ok(mut f) = std::fs::File::open(&path) else {
            return false;
        };
        let mut buf = Vec::new();
        if f.seek(SeekFrom::Start(a.offset)).is_err() || f.read_to_end(&mut buf).is_err() {
            return false;
        }
        drop(f);
        let Some(end) = buf.iter().rposition(|&b| b == b'\n') else {
            return false;
        };
        a.offset += end as u64 + 1;
        let mut moved = false;
        for line in String::from_utf8_lossy(&buf[..end]).lines() {
            let Ok(v) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            if v["event"] == "iteration" {
                let idx = v["index"].as_u64().unwrap_or(0);
                if idx > a.step {
                    let r = json!({"step": idx, "time": v["time"],
                        "residuals": {"u": v["res_u"], "p": v["res_p"], "continuity": v["continuity"]},
                        "monitors": {}});
                    note("residuals", self.store.append_residual(&a.id, &r));
                    a.step = idx;
                    a.time = v["time"].as_f64().unwrap_or(a.time);
                    moved = true;
                }
                continue;
            }
            if v["event"] == "finish" {
                a.finish = Some(v.clone());
            }
            let seq = v["seq"].as_u64().unwrap_or(0);
            if seq > a.max_seq {
                a.max_seq = seq;
                note(
                    "event",
                    self.store
                        .append_event(&a.id, json!({"event": "solver", "solver_seq": seq, "solver": v})),
                );
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
                    (!sys::pid_alive(a.pid, "rcfd")).then_some(None)
                }
                None => None,
            };
            let mut dirty = self.drain(&mut a);
            if let Some(code) = exited {
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
                            self.store.append_event(&a.id, json!({"event": "cancel_requested"})),
                        );
                    }
                    Some(t) if t.elapsed() > grace && !a.killed_for_cancel => {
                        a.killed_for_cancel = sys::kill(a.pid);
                        note(
                            "event",
                            self.store.append_event(
                                &a.id,
                                json!({"event": "killed", "pid": a.pid, "why": "cancel grace elapsed"}),
                            ),
                        );
                    }
                    _ => {}
                }
            }
            if dirty {
                note("status", self.store.write_status(&a.id, &Self::status_of(&a)));
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
        let metrics = json!({
            "state": state, "reason": reason, "exit_code": st.exit_code, "steps": a.step, "time": a.time,
            "solver_finish": fin, "last_residuals": read_last_line(&dir.join("residuals.jsonl"))
                .and_then(|l| serde_json::from_str::<Value>(&l).ok()),
        });
        note("metrics", self.store.write_metrics(&a.id, &metrics));
        self.set_state(&a.id, &st, &reason);
        self.clear_requests(&a.id);
    }

    fn shutdown(&mut self) {
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
                    json!({"event": "runner_shutdown", "pid": a.pid,
                           "note": "the solver keeps running; the next runner adopts it"}),
                ),
            );
        }
    }
}
