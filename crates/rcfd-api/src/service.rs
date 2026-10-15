//! Solver service: JSON-RPC 2.0 over newline-delimited streams (TCP or
//! stdio) with sessions, run control, monitors, field access, rendering and
//! an event log (F-085). Protocol version 1; see `docs/service-protocol.md`.
//!
//! The same [`Service`] object can be driven in-process through
//! [`Service::call`], which is how the MCP server uses it.

use crate::case::{Case, Diagnostic};
use crate::runctl;
use crate::schema::case_schema;
use crate::sim::Simulation;
use crate::workflow::{self, Task, TaskState, Workflow};
use rcfd_core::vec3::Vec3;
use rcfd_mesh::quality::check_mesh;
use rcfd_post::monitors::monitor_json_schema;
use rcfd_post::render::{overlay_vectors, render_scalar, RenderOptions};
use serde_json::{json, Map, Value};
use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// Protocol identifier reported by `service.info`.
pub const PROTOCOL: &str = "rcfd-service/1";

/// JSON-RPC error codes used by the service.
pub mod code {
    /// Invalid JSON.
    pub const PARSE: i64 = -32700;
    /// Not a valid request object.
    pub const INVALID_REQUEST: i64 = -32600;
    /// Unknown method.
    pub const METHOD_NOT_FOUND: i64 = -32601;
    /// Bad or missing parameters.
    pub const INVALID_PARAMS: i64 = -32602;
    /// Unknown session id.
    pub const NO_SESSION: i64 = 1001;
    /// Operation not allowed in the current run state.
    pub const BAD_STATE: i64 = 1002;
    /// Settings failed validation; `data.diagnostics` lists the problems.
    pub const VALIDATION: i64 = 1003;
    /// A configured limit was exceeded or a path left the sandbox.
    pub const LIMIT: i64 = 1004;
    /// Anything else.
    pub const INTERNAL: i64 = -32000;
}

/// Error returned by service calls.
#[derive(Debug, Clone, PartialEq)]
pub struct RpcError {
    /// Numeric code (see [`code`]).
    pub code: i64,
    /// Message.
    pub message: String,
    /// Optional structured data.
    pub data: Option<Value>,
}

impl RpcError {
    fn new(code: i64, message: impl Into<String>) -> Self {
        RpcError {
            code,
            message: message.into(),
            data: None,
        }
    }

    fn params(message: impl Into<String>) -> Self {
        Self::new(code::INVALID_PARAMS, message)
    }

    fn validation(diags: Vec<Diagnostic>) -> Self {
        let text = diags.iter().map(|d| d.to_string()).collect::<Vec<_>>().join("; ");
        RpcError {
            code: code::VALIDATION,
            message: format!("validation failed: {text}"),
            data: Some(json!({"diagnostics": diags})),
        }
    }

    fn to_json(&self) -> Value {
        let mut e = json!({"code": self.code, "message": self.message});
        if let Some(d) = &self.data {
            e["data"] = d.clone();
        }
        e
    }
}

impl std::fmt::Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

type Rpc<T = Value> = std::result::Result<T, RpcError>;

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Limits and sandbox of a service instance.
#[derive(Debug, Clone)]
pub struct ServiceConfig {
    /// Directory all file paths must stay inside (`None` = unrestricted).
    pub root: Option<PathBuf>,
    /// Refuse to build simulations with more cells than this.
    pub max_cells: usize,
    /// Largest `max_steps` / `count` accepted in one run or step request.
    pub max_steps_per_call: usize,
    /// Event log capacity.
    pub event_capacity: usize,
    /// Wall-clock cap for one `run.start` (seconds); the run is stopped when exceeded.
    pub max_run_seconds: Option<f64>,
}

impl Default for ServiceConfig {
    fn default() -> Self {
        ServiceConfig {
            root: None,
            max_cells: 2_000_000,
            max_steps_per_call: 10_000_000,
            event_capacity: 20_000,
            max_run_seconds: None,
        }
    }
}

/// Run state of a session.
#[derive(Debug, Clone, PartialEq)]
pub enum RunState {
    /// Case defined, no simulation built.
    Defined,
    /// Simulation built, ready to run.
    Ready,
    /// Worker thread is stepping.
    Running,
    /// Worker paused.
    Paused,
    /// Run completed (`converged` tells whether the targets were met).
    Finished {
        /// Residual targets met (steady) or end time reached (transient).
        converged: bool,
    },
    /// Run stopped by the client.
    Stopped,
    /// Worker failed.
    Failed(String),
}

impl RunState {
    fn name(&self) -> &'static str {
        match self {
            RunState::Defined => "defined",
            RunState::Ready => "ready",
            RunState::Running => "running",
            RunState::Paused => "paused",
            RunState::Finished { .. } => "finished",
            RunState::Stopped => "stopped",
            RunState::Failed(_) => "failed",
        }
    }
}

struct Event {
    seq: u64,
    value: Value,
}

struct EventLog {
    cap: usize,
    inner: Mutex<(u64, VecDeque<Event>)>,
    cv: Condvar,
}

impl EventLog {
    fn emit(&self, session: &str, kind: &str, data: Value) {
        let mut g = lock(&self.inner);
        g.0 += 1;
        let seq = g.0;
        let value = json!({"seq": seq, "session": session, "kind": kind, "data": data});
        g.1.push_back(Event { seq, value });
        while g.1.len() > self.cap {
            g.1.pop_front();
        }
        self.cv.notify_all();
    }

    fn poll(&self, after: u64, session: Option<&str>, timeout: Duration, limit: usize) -> (Vec<Value>, u64) {
        let deadline = Instant::now() + timeout;
        let mut g = lock(&self.inner);
        loop {
            let found: Vec<Value> =
                g.1.iter()
                    .filter(|e| e.seq > after && session.map_or(true, |s| e.value["session"] == s))
                    .take(limit)
                    .map(|e| e.value.clone())
                    .collect();
            if !found.is_empty() || Instant::now() >= deadline {
                let next = found
                    .last()
                    .map_or(g.0.max(after), |v| v["seq"].as_u64().unwrap_or(after));
                return (found, next);
            }
            let left = deadline.saturating_duration_since(Instant::now());
            let (ng, _) = self.cv.wait_timeout(g, left).unwrap_or_else(|e| e.into_inner());
            g = ng;
        }
    }
}

struct RunCtl {
    state: Mutex<RunState>,
    cv: Condvar,
    stop: AtomicBool,
    pause: AtomicBool,
}

struct SessionInner {
    case: Case,
    sim: Option<Arc<Mutex<Simulation>>>,
    log: VecDeque<String>,
    journal: Vec<Value>,
    workflow: Option<Workflow>,
}

struct Session {
    id: String,
    inner: Mutex<SessionInner>,
    ctl: Arc<RunCtl>,
    worker: Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl Session {
    fn state(&self) -> RunState {
        lock(&self.ctl.state).clone()
    }

    fn set_state(&self, s: RunState) {
        *lock(&self.ctl.state) = s;
        self.ctl.cv.notify_all();
    }
}

/// The solver service.
pub struct Service {
    cfg: ServiceConfig,
    sessions: Mutex<HashMap<String, Arc<Session>>>,
    next: AtomicU64,
    events: Arc<EventLog>,
}

fn s_param<'a>(p: &'a Value, key: &str) -> Rpc<&'a str> {
    p.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| RpcError::params(format!("missing string parameter '{key}'")))
}

fn u_param(p: &Value, key: &str, default: u64) -> Rpc<u64> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(default),
        Some(v) => v
            .as_u64()
            .ok_or_else(|| RpcError::params(format!("parameter '{key}' must be a non-negative integer"))),
    }
}

/// Base64 (standard alphabet, padded) used for image payloads.
pub fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

impl Service {
    /// New service with the given configuration.
    pub fn new(cfg: ServiceConfig) -> Arc<Service> {
        let cap = cfg.event_capacity;
        Arc::new(Service {
            cfg,
            sessions: Mutex::new(HashMap::new()),
            next: AtomicU64::new(1),
            events: Arc::new(EventLog {
                cap,
                inner: Mutex::new((0, VecDeque::new())),
                cv: Condvar::new(),
            }),
        })
    }

    /// The configuration.
    pub fn config(&self) -> &ServiceConfig {
        &self.cfg
    }

    fn session(&self, p: &Value) -> Rpc<Arc<Session>> {
        let id = s_param(p, "session")?;
        lock(&self.sessions)
            .get(id)
            .cloned()
            .ok_or_else(|| RpcError::new(code::NO_SESSION, format!("no session '{id}'")))
    }

    /// Resolve a client path inside the sandbox root.
    pub fn sandbox_path(&self, p: &str) -> Rpc<PathBuf> {
        let path = Path::new(p);
        let Some(root) = &self.cfg.root else {
            return Ok(path.to_path_buf());
        };
        if path.components().any(|c| matches!(c, Component::ParentDir)) {
            return Err(RpcError::new(code::LIMIT, format!("path '{p}' leaves the sandbox")));
        }
        let full = if path.is_absolute() {
            path.to_path_buf()
        } else {
            root.join(path)
        };
        let canon_root = root.canonicalize().unwrap_or_else(|_| root.clone());
        let mut probe = full.clone();
        while !probe.exists() {
            if !probe.pop() {
                break;
            }
        }
        let canon = probe.canonicalize().unwrap_or(probe);
        if !canon.starts_with(&canon_root) {
            return Err(RpcError::new(code::LIMIT, format!("path '{p}' is outside the sandbox")));
        }
        Ok(full)
    }

    /// Refuse cases whose mesh file or output directory leave the sandbox.
    fn check_case_paths(&self, case: &Case) -> Rpc<()> {
        if self.cfg.root.is_none() {
            return Ok(());
        }
        if let Some(f) = case.get("mesh.file").ok().and_then(|v| v.as_str().map(String::from)) {
            if !f.is_empty() {
                let full = if Path::new(&f).is_absolute() {
                    f
                } else {
                    case.base_dir.join(&f).display().to_string()
                };
                self.sandbox_path(&full)?;
            }
        }
        if let Some(d) = case
            .get("output.directory")
            .ok()
            .and_then(|v| v.as_str().map(String::from))
        {
            if !d.is_empty() {
                self.sandbox_path(&d)?;
            }
        }
        Ok(())
    }

    /// Call a method in-process.
    pub fn call(self: &Arc<Self>, method: &str, params: &Value) -> Rpc {
        let null = Value::Null;
        let p = if params.is_null() { &null } else { params };
        match method {
            "service.info" => Ok(json!({
                "name": "rcfd",
                "version": env!("CARGO_PKG_VERSION"),
                "protocol": PROTOCOL,
                "methods": METHODS,
                "limits": {"max_cells": self.cfg.max_cells, "max_steps_per_call": self.cfg.max_steps_per_call, "max_run_seconds": self.cfg.max_run_seconds},
                "sandbox_root": self.cfg.root.as_ref().map(|r| r.display().to_string()),
            })),
            "settings.schema" => Ok(json!({
                "case": case_schema().to_json_schema(),
                "monitors": monitor_json_schema(),
            })),
            "session.create" => self.session_create(p),
            "session.list" => {
                let s = lock(&self.sessions);
                let mut ids: Vec<&String> = s.keys().collect();
                ids.sort();
                Ok(
                    json!({"sessions": ids.iter().map(|id| json!({"session": id, "state": s[*id].state().name()})).collect::<Vec<_>>()}),
                )
            }
            "session.close" => {
                let s = self.session(p)?;
                self.stop_worker(&s, true);
                lock(&self.sessions).remove(&s.id);
                self.events.emit(&s.id, "closed", json!({}));
                Ok(json!({"closed": s.id}))
            }
            "settings.get" => {
                let s = self.session(p)?;
                let inner = lock(&s.inner);
                let v = match p.get("path").and_then(Value::as_str) {
                    Some(path) if !path.is_empty() => {
                        inner.case.get(path).map_err(|e| RpcError::params(e.to_string()))?
                    }
                    _ => inner.case.tree.to_json(),
                };
                Ok(json!({"value": v}))
            }
            "settings.set" => {
                let s = self.session(p)?;
                let path = s_param(p, "path")?;
                let value = p
                    .get("value")
                    .cloned()
                    .ok_or_else(|| RpcError::params("missing 'value'"))?;
                // turbulence settings of a built simulation that is Ready or Paused are applied live (F-044): the flow
                // field, the iteration counter and the monitors are kept and only the turbulence state is rebuilt
                let live_turb = path.starts_with("turbulence.")
                    && matches!(s.state(), RunState::Ready | RunState::Paused)
                    && lock(&s.inner).sim.is_some();
                if matches!(s.state(), RunState::Running) || (matches!(s.state(), RunState::Paused) && !live_turb) {
                    return Err(RpcError::new(code::BAD_STATE, "stop the run before changing settings"));
                }
                let mut inner = lock(&s.inner);
                let old = inner.case.get(path).unwrap_or(Value::Null);
                inner.case.set(path, value).map_err(|e| {
                    RpcError::validation(vec![Diagnostic {
                        path: path.into(),
                        line: None,
                        message: e.to_string(),
                    }])
                })?;
                if let Err(e) = self.check_case_paths(&inner.case) {
                    let _ = inner.case.set(path, old);
                    return Err(e);
                }
                if live_turb {
                    if let Some(sim) = inner.sim.clone() {
                        let new_case = inner.case.clone();
                        let res = lock(&sim).reconfigure_turbulence(&new_case);
                        if let Err(diags) = res {
                            let _ = inner.case.set(path, old);
                            return Err(RpcError::validation(diags));
                        }
                    }
                } else {
                    inner.sim = None;
                }
                if p.get("_workflow") != Some(&Value::Bool(true)) {
                    if let Some(w) = inner.workflow.as_mut() {
                        w.invalidate_all();
                    }
                }
                let stored = inner.case.get(path).unwrap_or(Value::Null);
                inner.journal.push(json!({"op": "set", "path": path, "value": stored}));
                drop(inner);
                if !live_turb {
                    s.set_state(RunState::Defined);
                }
                self.events
                    .emit(&s.id, "setting_changed", json!({"path": path, "live": live_turb}));
                Ok(json!({"path": path, "value": lock(&s.inner).case.get(path).ok()}))
            }
            "settings.remove" => {
                let s = self.session(p)?;
                let path = s_param(p, "path")?;
                if matches!(s.state(), RunState::Running | RunState::Paused) {
                    return Err(RpcError::new(code::BAD_STATE, "stop the run before changing settings"));
                }
                let (coll, name) = path
                    .rsplit_once('.')
                    .ok_or_else(|| RpcError::params("path must be '<collection>.<name>'"))?;
                let mut inner = lock(&s.inner);
                inner.case.tree.remove_named(coll, name).map_err(|e| {
                    RpcError::validation(vec![Diagnostic {
                        path: path.into(),
                        line: None,
                        message: e.to_string(),
                    }])
                })?;
                inner.sim = None;
                if let Some(w) = inner.workflow.as_mut() {
                    w.invalidate_all();
                }
                inner.journal.push(json!({"op": "remove", "path": path}));
                drop(inner);
                s.set_state(RunState::Defined);
                self.events.emit(&s.id, "setting_changed", json!({"path": path}));
                Ok(json!({"removed": path}))
            }
            "settings.validate" => {
                let s = self.session(p)?;
                let inner = lock(&s.inner);
                let mut diags = inner.case.semantic_errors();
                let mesh_err = Simulation::new(&inner.case).err();
                diags.extend(mesh_err.unwrap_or_default());
                Ok(json!({"ok": diags.is_empty(), "diagnostics": diags}))
            }
            "mesh.check" => {
                let s = self.session(p)?;
                let case = lock(&s.inner).case.clone();
                self.check_case_paths(&case)?;
                let mesh = crate::sim::build_mesh(&case).map_err(RpcError::validation)?;
                let q = check_mesh(&mesh);
                Ok(json!({
                    "ok": q.is_ok(),
                    "report": serde_json::to_value(&q).unwrap_or(Value::Null),
                    "patches": mesh.patches().iter().map(|p| json!({"name": p.name, "faces": p.size, "kind": format!("{:?}", p.kind)})).collect::<Vec<_>>(),
                }))
            }
            "mesh.partition" => {
                let s = self.session(p)?;
                let case = lock(&s.inner).case.clone();
                self.check_case_paths(&case)?;
                let over: Vec<(String, Value)> = crate::decomp::OVERRIDE_KEYS
                    .iter()
                    .filter_map(|k| p.get(*k).map(|v| (k.to_string(), v.clone())))
                    .collect();
                let case = crate::decomp::with_overrides(&case, &over).map_err(RpcError::validation)?;
                let mesh = crate::sim::build_mesh(&case).map_err(RpcError::validation)?;
                let report = crate::decomp::run(&case, &mesh).map_err(RpcError::validation)?;
                let with_parts = p.get("with_parts").and_then(Value::as_bool).unwrap_or(false);
                Ok(crate::decomp::report_json(&report, with_parts))
            }
            "mesh.inflate" => {
                let s = self.session(p)?;
                let case = lock(&s.inner).case.clone();
                self.check_case_paths(&case)?;
                let mesh = crate::sim::build_mesh(&case).map_err(RpcError::validation)?;
                let req = crate::inflate::InflateRequest::from_json(p).map_err(|e| RpcError::params(e.to_string()))?;
                crate::inflate::inflate(&mesh, &req)
                    .map(|(_, doc)| doc)
                    .map_err(|e| RpcError::params(e.to_string()))
            }
            "field.sample" => {
                let s = self.session(p)?;
                let sim = self.sim(&s)?;
                let sim = lock(&sim);
                let spec = crate::postproc::spec_from_json(p).map_err(RpcError::params)?;
                let set = crate::postproc::sample(&sim, &spec).map_err(RpcError::params)?;
                Ok(json!({
                    "names": set.names, "n_samples": set.len(), "n_inside": set.n_inside(),
                    "inside": set.cells.iter().map(Option::is_some).collect::<Vec<_>>(),
                    "csv": set.to_csv(),
                }))
            }
            "run.initialize" => {
                let s = self.session(p)?;
                self.require_idle(&s)?;
                self.initialize(&s)?;
                Ok(self.status(&s))
            }
            "run.start" => self.run_start(p),
            "run.pause" => {
                let s = self.session(p)?;
                if s.state() != RunState::Running {
                    return Err(RpcError::new(code::BAD_STATE, "session is not running"));
                }
                s.ctl.pause.store(true, Ordering::SeqCst);
                self.wait_state(&s, |st| *st == RunState::Paused, Duration::from_secs(5));
                Ok(self.status(&s))
            }
            "run.resume" => {
                let s = self.session(p)?;
                if s.state() != RunState::Paused {
                    return Err(RpcError::new(code::BAD_STATE, "session is not paused"));
                }
                s.ctl.pause.store(false, Ordering::SeqCst);
                s.set_state(RunState::Running);
                self.events.emit(&s.id, "resumed", json!({}));
                Ok(self.status(&s))
            }
            "run.stop" => {
                let s = self.session(p)?;
                self.stop_worker(&s, false);
                Ok(self.status(&s))
            }
            "run.step" => self.run_step(p),
            "run.status" => {
                let s = self.session(p)?;
                Ok(self.status(&s))
            }
            "run.wait" => {
                let s = self.session(p)?;
                let ms = u_param(p, "timeout_ms", 10_000)?;
                self.wait_state(&s, |st| !matches!(st, RunState::Running), Duration::from_millis(ms));
                Ok(self.status(&s))
            }
            "monitors.get" => {
                let s = self.session(p)?;
                let sim = self.sim(&s)?;
                let sim = lock(&sim);
                let since = p.get("since").and_then(Value::as_f64);
                let names: Option<Vec<&str>> = p
                    .get("names")
                    .and_then(Value::as_array)
                    .map(|a| a.iter().filter_map(Value::as_str).collect());
                let mut doc = sim.monitors.to_json();
                if let (Some(series), true) = (doc["series"].as_object_mut(), names.is_some() || since.is_some()) {
                    series.retain(|k, _| names.as_ref().map_or(true, |n| n.contains(&k.as_str())));
                    if let Some(after) = since {
                        for (k, v) in series.iter_mut() {
                            if let Some(sl) = sim.monitors.since(k, after) {
                                *v = json!({"unit": sl.unit, "x": sl.x, "y": sl.y});
                            }
                        }
                    }
                }
                Ok(doc)
            }
            "reports.get" => {
                let s = self.session(p)?;
                let sim = self.sim(&s)?;
                let sim = lock(&sim);
                let vals: Vec<Value> = sim
                    .report_values()
                    .into_iter()
                    .map(|(n, r)| match r {
                        Ok(v) => json!({"name": n, "value": v.value, "unit": v.unit}),
                        Err(e) => json!({"name": n, "error": e}),
                    })
                    .collect();
                Ok(json!({"reports": vals}))
            }
            "report.export" => {
                let s = self.session(p)?;
                let sim = self.sim(&s)?;
                let sim = lock(&sim);
                let fmt = p.get("format").and_then(Value::as_str).unwrap_or("html");
                let (mime, body) = match fmt {
                    "html" => ("text/html", sim.html_report()),
                    "csv" => ("text/csv", sim.monitors.to_csv()),
                    "json" => ("application/json", sim.monitors.to_json().to_string()),
                    other => return Err(RpcError::params(format!("unknown format '{other}' (html, csv, json)"))),
                };
                if let Some(path) = p.get("path").and_then(Value::as_str) {
                    let path = self.sandbox_path(path)?;
                    if let Some(dir) = path.parent() {
                        std::fs::create_dir_all(dir).map_err(|e| RpcError::new(code::INTERNAL, e.to_string()))?;
                    }
                    std::fs::write(&path, &body).map_err(|e| RpcError::new(code::INTERNAL, e.to_string()))?;
                    return Ok(json!({"mime": mime, "path": path.display().to_string(), "bytes": body.len()}));
                }
                Ok(json!({"mime": mime, "content": body}))
            }
            "field.stats" => {
                let s = self.session(p)?;
                let sim = self.sim(&s)?;
                let r = lock(&sim).field_stats(s_param(p, "name")?);
                r.map_err(|e| RpcError::params(e.to_string()))
            }
            "field.slice" => {
                let s = self.session(p)?;
                let sim = self.sim(&s)?;
                let sim = lock(&sim);
                let name = s_param(p, "name")?;
                let vals = sim.field(name).map_err(|e| RpcError::params(e.to_string()))?;
                let maxp = u_param(p, "max_points", 2000)?.max(1) as usize;
                let stride = vals.len().div_ceil(maxp).max(1);
                let cc = sim.geom.mesh.cell_centres();
                let idx: Vec<usize> = (0..vals.len()).step_by(stride).collect();
                Ok(json!({
                    "name": name, "n_cells": vals.len(), "stride": stride,
                    "x": idx.iter().map(|&i| cc[i][0]).collect::<Vec<_>>(),
                    "y": idx.iter().map(|&i| cc[i][1]).collect::<Vec<_>>(),
                    "z": idx.iter().map(|&i| cc[i][2]).collect::<Vec<_>>(),
                    "values": idx.iter().map(|&i| vals[i]).collect::<Vec<_>>(),
                }))
            }
            "field.export_vtk" => {
                let s = self.session(p)?;
                let dir = p.get("dir").and_then(Value::as_str).unwrap_or("results");
                let dir = self.sandbox_path(dir)?;
                let sim = self.sim(&s)?;
                let sim = lock(&sim);
                let name = p.get("name").and_then(Value::as_str).unwrap_or("solution");
                let path = sim
                    .export_vtu(&dir, name)
                    .map_err(|e| RpcError::new(code::INTERNAL, e.to_string()))?;
                Ok(json!({"path": path.display().to_string()}))
            }
            "render.png" => {
                let s = self.session(p)?;
                let sim = self.sim(&s)?;
                let sim = lock(&sim);
                let field = p.get("field").and_then(Value::as_str).unwrap_or("speed");
                let vals = sim.field(field).map_err(|e| RpcError::params(e.to_string()))?;
                let opt = RenderOptions {
                    width: u_param(p, "width", 640)?.clamp(64, 4096) as usize,
                    ..Default::default()
                };
                let mut r = render_scalar(&sim.geom.mesh, &vals, &opt).map_err(|e| RpcError::params(e.to_string()))?;
                if p.get("vectors").and_then(Value::as_bool).unwrap_or(false) {
                    let u: Vec<Vec3> = sim.velocity().values.clone();
                    overlay_vectors(&sim.geom.mesh, &u, &opt, 24, &mut r.image)
                        .map_err(|e| RpcError::params(e.to_string()))?;
                }
                let png = r
                    .image
                    .to_png()
                    .map_err(|e| RpcError::new(code::INTERNAL, e.to_string()))?;
                if let Some(path) = p.get("path").and_then(Value::as_str) {
                    let path = self.sandbox_path(path)?;
                    if let Some(dir) = path.parent() {
                        std::fs::create_dir_all(dir).map_err(|e| RpcError::new(code::INTERNAL, e.to_string()))?;
                    }
                    std::fs::write(&path, &png).map_err(|e| RpcError::new(code::INTERNAL, e.to_string()))?;
                }
                Ok(json!({
                    "mime": "image/png", "width": r.image.width, "height": r.image.height,
                    "field": field, "min": r.vmin, "max": r.vmax, "data_base64": base64(&png),
                }))
            }
            "render.glb" => {
                let s = self.session(p)?;
                let sim = self.sim(&s)?;
                let sim = lock(&sim);
                let field = p.get("field").and_then(Value::as_str);
                let vals = match field {
                    Some(f) => Some(sim.field(f).map_err(|e| RpcError::params(e.to_string()))?),
                    None => None,
                };
                let mesh = &sim.geom.mesh;
                let names: Vec<String> = match p.get("patches") {
                    Some(v) => v
                        .as_array()
                        .ok_or_else(|| RpcError::params("patches must be an array of names"))?
                        .iter()
                        .map(|x| {
                            x.as_str()
                                .map(str::to_string)
                                .ok_or_else(|| RpcError::params("patches must be an array of names"))
                        })
                        .collect::<Rpc<_>>()?,
                    None => mesh.patches().iter().map(|q| q.name.clone()).collect(),
                };
                let refs: Vec<&str> = names.iter().map(String::as_str).collect();
                let range = match (
                    p.get("min").and_then(Value::as_f64),
                    p.get("max").and_then(Value::as_f64),
                ) {
                    (Some(a), Some(b)) => Some((a, b)),
                    (None, None) => None,
                    _ => return Err(RpcError::params("give both min and max or neither")),
                };
                let opt = rcfd_post::gltf::GltfOptions {
                    range,
                    scalar_name: field.unwrap_or("").to_string(),
                };
                let (glb, st) = rcfd_post::gltf::glb_patches(
                    mesh,
                    &refs,
                    vals.as_deref().map(rcfd_post::gltf::GltfScalar::Cell),
                    &opt,
                )
                .map_err(|e| RpcError::params(e.to_string()))?;
                if let Some(path) = p.get("path").and_then(Value::as_str) {
                    let path = self.sandbox_path(path)?;
                    if let Some(dir) = path.parent() {
                        std::fs::create_dir_all(dir).map_err(|e| RpcError::new(code::INTERNAL, e.to_string()))?;
                    }
                    std::fs::write(&path, &glb).map_err(|e| RpcError::new(code::INTERNAL, e.to_string()))?;
                }
                let inline = p.get("inline").and_then(Value::as_bool).unwrap_or(false);
                let mut out = json!({
                    "mime": "model/gltf-binary", "bytes": glb.len(), "field": field,
                    "faces": st.faces, "triangles": st.triangles, "vertices": st.vertices,
                    "min": st.range.0, "max": st.range.1, "patches": names,
                });
                if inline {
                    if glb.len() > 4 << 20 {
                        return Err(RpcError::params("GLB larger than 4 MiB; write it with 'path' instead"));
                    }
                    out["data_base64"] = json!(base64(&glb));
                }
                Ok(out)
            }
            "field.grid" => {
                let s = self.session(p)?;
                let sim = self.sim(&s)?;
                let sim = lock(&sim);
                let name = s_param(p, "name")?;
                let vals = sim.field(name).map_err(|e| RpcError::params(e.to_string()))?;
                let cols = u_param(p, "cols", 40)?.clamp(2, 200) as usize;
                let rows = u_param(p, "rows", 20)?.clamp(2, 100) as usize;
                let cc = sim.geom.mesh.cell_centres();
                let ext = |a: usize| {
                    let lo = cc.iter().map(|c| c[a]).fold(f64::INFINITY, f64::min);
                    let hi = cc.iter().map(|c| c[a]).fold(f64::NEG_INFINITY, f64::max);
                    (lo, hi)
                };
                let mut axes = [0usize, 1, 2];
                axes.sort_by(|&a, &b| (ext(b).1 - ext(b).0).total_cmp(&(ext(a).1 - ext(a).0)));
                let (ax, ay) = (axes[0], axes[1]);
                let ((x0, x1), (y0, y1)) = (ext(ax), ext(ay));
                let mut sum = vec![0.0; cols * rows];
                let mut cnt = vec![0u32; cols * rows];
                for (c, v) in cc.iter().zip(vals.iter()) {
                    let fx = if x1 > x0 { (c[ax] - x0) / (x1 - x0) } else { 0.5 };
                    let fy = if y1 > y0 { (c[ay] - y0) / (y1 - y0) } else { 0.5 };
                    let i = ((fx * cols as f64) as usize).min(cols - 1);
                    let j = ((fy * rows as f64) as usize).min(rows - 1);
                    // first row is the top (largest coordinate)
                    let k = (rows - 1 - j) * cols + i;
                    sum[k] += v;
                    cnt[k] += 1;
                }
                let mut grid = Vec::with_capacity(sum.len());
                let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
                for (s, &n) in sum.iter().zip(&cnt) {
                    if n > 0 {
                        let v = s / n as f64;
                        lo = lo.min(v);
                        hi = hi.max(v);
                        grid.push(json!(v));
                    } else {
                        grid.push(Value::Null);
                    }
                }
                let names = ["x", "y", "z"];
                Ok(json!({
                    "name": name, "cols": cols, "rows": rows, "values": grid,
                    "min": lo, "max": hi, "axes": format!("{}{}", names[ax], names[ay]),
                }))
            }
            "panel.snapshot" => {
                let s = self.session(p)?;
                let since = p.get("since").and_then(Value::as_f64);
                let mut out = json!({"status": self.status(&s)});
                let lines = u_param(p, "log_lines", 8)? as usize;
                {
                    let inner = lock(&s.inner);
                    let skip = inner.log.len().saturating_sub(lines);
                    out["log"] = json!(inner.log.iter().skip(skip).cloned().collect::<Vec<_>>());
                }
                let has_sim = lock(&s.inner).sim.is_some();
                if has_sim {
                    let mut m = json!({"session": s.id});
                    if let Some(x) = since {
                        m["since"] = json!(x);
                    }
                    out["monitors"] = self.call("monitors.get", &m)?;
                    if let Ok(r) = self.call("reports.get", &json!({"session": s.id})) {
                        out["reports"] = r["reports"].clone();
                    }
                    if let Some(f) = p.get("field").and_then(Value::as_str) {
                        let mut g = json!({"session": s.id, "name": f});
                        for k in ["cols", "rows"] {
                            if let Some(v) = p.get(k) {
                                g[k] = v.clone();
                            }
                        }
                        out["grid"] = self.call("field.grid", &g)?;
                    }
                }
                Ok(out)
            }
            "journal.get" => {
                let s = self.session(p)?;
                let inner = lock(&s.inner);
                Ok(json!({
                    "format": "rcfd-journal", "version": 1,
                    "entries": inner.journal,
                    "case_hash": format!("{:016x}", inner.case.tree.content_hash()),
                }))
            }
            "journal.replay" => {
                let j = p.get("journal").ok_or_else(|| RpcError::params("missing 'journal'"))?;
                if j["format"] != "rcfd-journal" || j["version"] != 1 {
                    return Err(RpcError::params("not an rcfd-journal version 1 document"));
                }
                let entries = j["entries"]
                    .as_array()
                    .ok_or_else(|| RpcError::params("journal has no entries"))?;
                let first = entries.first().filter(|e| e["op"] == "create");
                let src = first.map_or(json!({}), |e| e["source"].clone());
                let mut cp = Map::new();
                for k in ["toml", "settings", "path"] {
                    if !src[k].is_null() {
                        cp.insert(k.into(), src[k].clone());
                    }
                }
                let id = self.call("session.create", &Value::Object(cp))?["session"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                for e in entries.iter().skip(usize::from(first.is_some())) {
                    if e["op"] == "set" {
                        self.call(
                            "settings.set",
                            &json!({"session": id, "path": e["path"], "value": e["value"]}),
                        )?;
                    } else if e["op"] == "remove" {
                        self.call("settings.remove", &json!({"session": id, "path": e["path"]}))?;
                    }
                }
                let hash = self.call("journal.get", &json!({"session": id}))?["case_hash"].clone();
                let matches = j.get("case_hash").map_or(true, |h| *h == hash);
                Ok(json!({"session": id, "case_hash": hash, "matches": matches}))
            }
            "log.tail" => {
                let s = self.session(p)?;
                let n = u_param(p, "lines", 50)? as usize;
                let inner = lock(&s.inner);
                let skip = inner.log.len().saturating_sub(n);
                Ok(json!({"lines": inner.log.iter().skip(skip).cloned().collect::<Vec<_>>()}))
            }
            "events.poll" => {
                let after = u_param(p, "after", 0)?;
                let timeout = Duration::from_millis(u_param(p, "timeout_ms", 0)?.min(60_000));
                let limit = u_param(p, "limit", 500)?.clamp(1, 5000) as usize;
                let sess = p.get("session").and_then(Value::as_str);
                let (events, next) = self.events.poll(after, sess, timeout, limit);
                Ok(json!({"events": events, "next": next}))
            }
            "workflow.create" => {
                let s = self.session(p)?;
                let mut w = if let Some(doc) = p.get("workflow") {
                    Workflow::from_json(doc).map_err(RpcError::params)?
                } else {
                    let t = p.get("template").and_then(Value::as_str).unwrap_or("mesh_to_results");
                    Workflow::template(t).ok_or_else(|| {
                        RpcError::params(format!(
                            "unknown template '{t}' (known: {})",
                            workflow::TEMPLATES.iter().map(|x| x.0).collect::<Vec<_>>().join(", ")
                        ))
                    })?
                };
                if let Some(n) = p.get("name").and_then(Value::as_str) {
                    w.name = n.into();
                }
                let sum = w.summary();
                lock(&s.inner).workflow = Some(w);
                self.events.emit(
                    &s.id,
                    "workflow_created",
                    json!({"tasks": sum["tasks"].as_array().map_or(0, Vec::len)}),
                );
                Ok(sum)
            }
            "workflow.list" => {
                let s = self.session(p)?;
                let mut v = self.with_workflow(&s, |w| Ok(w.summary()))?;
                v["kinds"] = json!(workflow::KINDS
                    .iter()
                    .map(|k| json!({"kind": k.0, "description": k.1}))
                    .collect::<Vec<_>>());
                v["templates"] = json!(workflow::TEMPLATES
                    .iter()
                    .map(|k| json!({"name": k.0, "description": k.1}))
                    .collect::<Vec<_>>());
                Ok(v)
            }
            "workflow.get" => {
                let s = self.session(p)?;
                self.with_workflow(&s, |w| Ok(w.to_json()))
            }
            "workflow.run" => self.workflow_run(p),
            "workflow.insert" => {
                let s = self.session(p)?;
                let t = p.get("task").ok_or_else(|| RpcError::params("missing 'task'"))?;
                let task = workflow::task_from_json(t).map_err(RpcError::params)?;
                let after = p.get("after").and_then(Value::as_str);
                self.with_workflow(&s, |w| w.insert(task, after).map(|_| w.summary()))
            }
            "workflow.delete" => {
                let s = self.session(p)?;
                let id = s_param(p, "id")?;
                self.with_workflow(&s, |w| w.delete(id).map(|_| w.summary()))
            }
            "workflow.save" => {
                let s = self.session(p)?;
                let path = self.sandbox_path(s_param(p, "path")?)?;
                let doc = self.with_workflow(&s, |w| Ok(w.to_json()))?;
                if let Some(dir) = path.parent() {
                    std::fs::create_dir_all(dir).map_err(|e| RpcError::new(code::INTERNAL, e.to_string()))?;
                }
                std::fs::write(&path, serde_json::to_string_pretty(&doc).unwrap_or_default())
                    .map_err(|e| RpcError::new(code::INTERNAL, e.to_string()))?;
                Ok(json!({"path": path.display().to_string()}))
            }
            "workflow.load" => {
                let s = self.session(p)?;
                let path = self.sandbox_path(s_param(p, "path")?)?;
                let text = std::fs::read_to_string(&path)
                    .map_err(|e| RpcError::params(format!("cannot read {}: {e}", path.display())))?;
                let doc: Value =
                    serde_json::from_str(&text).map_err(|e| RpcError::params(format!("workflow file: {e}")))?;
                let w = Workflow::from_json(&doc).map_err(RpcError::params)?;
                let sum = w.summary();
                lock(&s.inner).workflow = Some(w);
                Ok(sum)
            }
            "run.check" => {
                let case = if p.get("session").is_some() {
                    let s = self.session(p)?;
                    let c = lock(&s.inner).case.clone();
                    c
                } else {
                    let path = self.sandbox_path(s_param(p, "path")?)?;
                    Case::load_with(&path, self.cfg.root.as_deref()).map_err(RpcError::validation)?
                };
                self.check_case_paths(&case)?;
                let (code, ev) = runctl::check_only(&case);
                Ok(json!({"ok": code == runctl::exit::OK, "exit_code": code, "event": ev}))
            }
            "run.headless" => {
                let s = self.session(p)?;
                let (out, events) = self.headless(&s, p)?;
                Ok(
                    json!({"outcome": out.to_json(), "ok": out.exit_code == runctl::exit::OK, "events": events, "status": self.status(&s)}),
                )
            }
            "status.read" => {
                let path = self.sandbox_path(s_param(p, "path")?)?;
                let text = std::fs::read_to_string(&path)
                    .map_err(|e| RpcError::params(format!("cannot read {}: {e}", path.display())))?;
                runctl::summarize_stream(&text).map_err(RpcError::params)
            }
            other => Err(RpcError::new(
                code::METHOD_NOT_FOUND,
                format!("unknown method '{other}'"),
            )),
        }
    }

    fn with_workflow<R>(&self, s: &Session, f: impl FnOnce(&mut Workflow) -> Result<R, String>) -> Rpc<R> {
        let mut inner = lock(&s.inner);
        let w = inner
            .workflow
            .as_mut()
            .ok_or_else(|| RpcError::new(code::BAD_STATE, "no workflow; call workflow.create first"))?;
        f(w).map_err(RpcError::params)
    }

    /// Run a session's simulation under the run-control driver (F-121): limits, stop file,
    /// continue-from, checkpoints, status stream. Blocks until the run ends.
    fn headless(&self, s: &Arc<Session>, p: &Value) -> Rpc<(runctl::RunOutcome, Vec<Value>)> {
        self.require_idle(s)?;
        let opt_path = |key: &str| -> Rpc<Option<PathBuf>> {
            p.get(key)
                .and_then(Value::as_str)
                .map(|x| self.sandbox_path(x))
                .transpose()
        };
        let cont = opt_path("continue_from")?;
        let stop_file = opt_path("stop_file")?;
        let checkpoint = opt_path("checkpoint")?;
        let status_path = opt_path("status_path")?;
        let out_dir = opt_path("out_dir")?;
        if lock(&s.inner).sim.is_none()
            || cont.is_some()
            || matches!(
                s.state(),
                RunState::Finished { .. } | RunState::Stopped | RunState::Failed(_)
            )
        {
            self.initialize(s)?;
        }
        let sim = self.sim(s)?;
        if let Some(c) = &cont {
            lock(&sim)
                .load_checkpoint(c)
                .map_err(|e| RpcError::params(format!("continue_from: {e}")))?;
        }
        if let Some(src) = opt_path("interpolate_case")? {
            let ck = opt_path("interpolate_checkpoint")?
                .ok_or_else(|| RpcError::params("interpolate_case needs interpolate_checkpoint"))?;
            if cont.is_some() {
                return Err(RpcError::params("interpolate_* cannot be combined with continue_from"));
            }
            let mode = rcfd_post::interpolate::InterpMode::parse(
                p.get("interpolate_mode")
                    .and_then(Value::as_str)
                    .unwrap_or("nonconservative"),
            )
            .ok_or_else(|| RpcError::params("interpolate_mode must be conservative or nonconservative"))?;
            let src_case = Case::load_with(&src, self.cfg.root.as_deref()).map_err(RpcError::validation)?;
            self.check_case_paths(&src_case)?;
            lock(&sim)
                .interpolate_from(&src_case, &ck, mode)
                .map_err(|e| RpcError::params(format!("interpolate: {e}")))?;
        }
        let cap = self.cfg.max_steps_per_call;
        let max_steps = u_param(p, "max_steps", cap as u64)?.min(cap as u64) as usize;
        let max_elapsed = p
            .get("max_elapsed_s")
            .and_then(Value::as_f64)
            .into_iter()
            .chain(self.cfg.max_run_seconds)
            .fold(None, |a: Option<f64>, x| Some(a.map_or(x, |y| y.min(x))));
        let opts = runctl::RunOptions {
            max_steps: Some(max_steps),
            max_elapsed,
            stop_file,
            status_every: u_param(p, "status_every", 0)? as usize,
            checkpoint,
            checkpoint_every: u_param(p, "checkpoint_every", 0)? as usize,
            out_dir: out_dir.clone(),
            info: json!({"driver": "service", "continue_from": cont.as_ref().map(|c| c.display().to_string())}),
        };
        let mut file = match &status_path {
            Some(f) => {
                if let Some(dir) = f.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                Some(std::fs::File::create(f).map_err(|e| RpcError::new(code::INTERNAL, format!("status_path: {e}")))?)
            }
            None => None,
        };
        s.ctl.stop.store(false, Ordering::SeqCst);
        s.ctl.pause.store(false, Ordering::SeqCst);
        s.set_state(RunState::Running);
        self.events
            .emit(&s.id, "started", json!({"max_steps": max_steps, "headless": true}));
        let mut events: Vec<Value> = Vec::new();
        let outcome = runctl::drive(&sim, &opts, &|| s.ctl.stop.load(Ordering::SeqCst), &mut |ev: &Value| {
            if let Some(f) = file.as_mut() {
                let _ = writeln!(f, "{ev}");
            }
            if ev["event"] == "iteration" {
                let mut inner = lock(&s.inner);
                inner.log.push_back(format!(
                    "iter {:>6}  res_u {:.3e}  res_p {:.3e}",
                    ev["index"],
                    ev["res_u"].as_f64().unwrap_or(f64::NAN),
                    ev["res_p"].as_f64().unwrap_or(f64::NAN)
                ));
                while inner.log.len() > 2000 {
                    inner.log.pop_front();
                }
                drop(inner);
                self.events.emit(&s.id, "iteration", ev.clone());
            }
            events.push(ev.clone());
        });
        if events.len() > 60 {
            let skip = events.len() - 60;
            events.drain(1..=skip.min(events.len() - 1));
        }
        match outcome.state {
            "converged" | "finished" => {
                s.set_state(RunState::Finished { converged: true });
                self.events.emit(&s.id, "finished", json!({"converged": true}));
            }
            "not_converged" => {
                s.set_state(RunState::Finished { converged: false });
                self.events.emit(&s.id, "finished", json!({"converged": false}));
            }
            "step_limit" => {
                s.set_state(RunState::Ready);
                self.events
                    .emit(&s.id, "chunk_done", json!({"steps": outcome.iterations}));
            }
            "interrupted" | "time_limit" => {
                s.set_state(RunState::Stopped);
                self.events.emit(&s.id, "stopped", json!({"reason": outcome.reason}));
            }
            _ => {
                s.set_state(RunState::Failed(outcome.reason.clone()));
                self.events.emit(&s.id, "error", json!({"message": outcome.reason}));
            }
        }
        if let Some(dir) = out_dir {
            let g = lock(&sim);
            runctl::write_results(&g, &dir).map_err(|e| RpcError::new(code::INTERNAL, e.to_string()))?;
        }
        Ok((outcome, events))
    }

    /// Execute the planned tasks of `workflow.run`.
    fn workflow_run(self: &Arc<Self>, p: &Value) -> Rpc {
        let s = self.session(p)?;
        self.require_idle(&s)?;
        let with_pre = p.get("with_prerequisites").and_then(Value::as_bool).unwrap_or(true);
        let plan: Vec<String> = {
            let inner = lock(&s.inner);
            let w = inner
                .workflow
                .as_ref()
                .ok_or_else(|| RpcError::new(code::BAD_STATE, "no workflow; call workflow.create first"))?;
            match p.get("task").and_then(Value::as_str) {
                Some(t) => w.plan(t, with_pre).map_err(RpcError::params)?,
                None => {
                    let mut all: Vec<String> = Vec::new();
                    for t in w.tasks.iter().filter(|t| t.state != TaskState::Done) {
                        for id in w.plan(&t.id, true).map_err(RpcError::params)? {
                            if !all.contains(&id) {
                                all.push(id);
                            }
                        }
                    }
                    all
                }
            }
        };
        let mut results = Vec::new();
        let mut ok = true;
        for id in plan {
            let task = lock(&s.inner)
                .workflow
                .as_ref()
                .and_then(|w| w.task(&id).cloned())
                .ok_or_else(|| RpcError::params(format!("task '{id}' vanished")))?;
            self.events
                .emit(&s.id, "workflow_task", json!({"id": id, "state": "running"}));
            let r = self.execute_task(&s, &task, p);
            let (good, message, result) = match r {
                Ok((m, v)) => (true, m, v),
                Err((m, v)) => (false, m, v),
            };
            if let Some(w) = lock(&s.inner).workflow.as_mut() {
                // a set task changes the case under every other result
                w.finish(&id, good, message.clone(), result.clone());
                if task.kind == "set" && good {
                    w.invalidate_all();
                    if let Some(i) = w.index(&id) {
                        w.tasks[i].state = TaskState::Done;
                    }
                }
            }
            self.events.emit(
                &s.id,
                "workflow_task",
                json!({"id": id, "state": if good { "done" } else { "failed" }, "message": message}),
            );
            results.push(json!({"id": id, "ok": good, "message": message, "result": result}));
            if !good {
                ok = false;
                break;
            }
        }
        let summary = self.with_workflow(&s, |w| Ok(w.summary()))?;
        Ok(json!({"ok": ok, "ran": results, "workflow": summary}))
    }

    fn execute_task(
        self: &Arc<Self>,
        s: &Arc<Session>,
        task: &Task,
        call: &Value,
    ) -> Result<(String, Value), (String, Value)> {
        let sid = json!(s.id);
        let fail = |e: RpcError| (e.message.clone(), e.data.clone().unwrap_or(Value::Null));
        let tp = &task.params;
        match task.kind.as_str() {
            "check_mesh" => {
                let r = self.call("mesh.check", &json!({"session": sid})).map_err(fail)?;
                let cells = r["report"]["n_cells"].clone();
                if r["ok"] == true {
                    Ok((format!("mesh ok, {cells} cells"), json!({"report": r["report"]})))
                } else {
                    Err(("mesh failed its quality check".into(), json!({"report": r["report"]})))
                }
            }
            "validate" => {
                let r = self.call("settings.validate", &json!({"session": sid})).map_err(fail)?;
                if r["ok"] == true {
                    Ok(("settings valid".into(), Value::Null))
                } else {
                    let msgs: Vec<String> = r["diagnostics"]
                        .as_array()
                        .map(|a| {
                            a.iter()
                                .take(3)
                                .map(|d| {
                                    format!(
                                        "{}: {}",
                                        d["path"].as_str().unwrap_or(""),
                                        d["message"].as_str().unwrap_or("")
                                    )
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    Err((
                        format!("invalid set-up: {}", msgs.join("; ")),
                        json!({"diagnostics": r["diagnostics"]}),
                    ))
                }
            }
            "set" => {
                let r = self
                    .call(
                        "settings.set",
                        &json!({"session": sid, "path": tp["path"], "value": tp["value"], "_workflow": true}),
                    )
                    .map_err(fail)?;
                Ok((format!("{} = {}", tp["path"].as_str().unwrap_or(""), r["value"]), r))
            }
            "initialize" => {
                let r = self.call("run.initialize", &json!({"session": sid})).map_err(fail)?;
                Ok((format!("initialised, {} cells", r["simulation"]["cells"]), r))
            }
            "solve" => {
                let mut hp = json!({"max_steps": tp.get("max_steps").or_else(|| call.get("max_steps")).cloned().unwrap_or(Value::Null)});
                for k in [
                    "max_elapsed_s",
                    "stop_file",
                    "status_path",
                    "checkpoint",
                    "status_every",
                ] {
                    if let Some(v) = tp.get(k).or_else(|| call.get(k)) {
                        hp[k] = v.clone();
                    }
                }
                hp["session"] = sid.clone();
                if hp["max_steps"].is_null() {
                    hp.as_object_mut().map(|o| o.remove("max_steps"));
                }
                // the sim built by an earlier initialize task is continued, not rebuilt
                let (out, events) = self.headless(s, &hp).map_err(fail)?;
                let body =
                    json!({"outcome": out.to_json(), "last_events": events.iter().rev().take(3).collect::<Vec<_>>()});
                let msg = format!("{} after {} iterations ({})", out.state, out.iterations, out.reason);
                if matches!(out.state, "converged" | "finished") {
                    Ok((msg, body))
                } else {
                    Err((msg, body))
                }
            }
            "report" => {
                let r = self.call("reports.get", &json!({"session": sid})).map_err(fail)?;
                let mut body = json!({"reports": r["reports"]});
                if let Some(path) = tp.get("path").and_then(Value::as_str) {
                    let mut ep = json!({"session": sid, "path": path});
                    if let Some(f) = tp.get("format") {
                        ep["format"] = f.clone();
                    }
                    let e = self.call("report.export", &ep).map_err(fail)?;
                    body["file"] = e["path"].clone();
                }
                let n = r["reports"].as_array().map_or(0, Vec::len);
                Ok((format!("{n} report values"), body))
            }
            "export_vtk" => {
                let mut ep = json!({"session": sid});
                for k in ["dir", "name"] {
                    if let Some(v) = tp.get(k) {
                        ep[k] = v.clone();
                    }
                }
                let r = self.call("field.export_vtk", &ep).map_err(fail)?;
                Ok((format!("wrote {}", r["path"].as_str().unwrap_or("")), r))
            }
            other => Err((format!("unknown task kind '{other}'"), Value::Null)),
        }
    }

    fn sim(&self, s: &Session) -> Rpc<Arc<Mutex<Simulation>>> {
        lock(&s.inner)
            .sim
            .clone()
            .ok_or_else(|| RpcError::new(code::BAD_STATE, "no simulation yet; call run.initialize or run.start"))
    }

    fn require_idle(&self, s: &Session) -> Rpc<()> {
        if matches!(s.state(), RunState::Running | RunState::Paused) {
            Err(RpcError::new(code::BAD_STATE, "a run is in progress; stop it first"))
        } else {
            Ok(())
        }
    }

    fn session_create(&self, p: &Value) -> Rpc {
        let create_entry = json!({"op": "create", "source": {
            "toml": p.get("toml"), "settings": p.get("settings"), "path": p.get("path"),
        }});
        let mut case = if let Some(path) = p.get("path").and_then(Value::as_str) {
            let path = self.sandbox_path(path)?;
            Case::load_with(&path, self.cfg.root.as_deref()).map_err(RpcError::validation)?
        } else if let Some(text) = p.get("toml").and_then(Value::as_str) {
            let base = self.cfg.root.clone().unwrap_or_else(|| PathBuf::from("."));
            Case::parse_toml_with(text, &base, self.cfg.root.as_deref()).map_err(RpcError::validation)?
        } else if let Some(obj) = p.get("settings") {
            let base = self.cfg.root.clone().unwrap_or_else(|| PathBuf::from("."));
            Case::from_json_with(obj, &base, self.cfg.root.as_deref()).map_err(RpcError::validation)?
        } else {
            Case::new()
        };
        if p.get("path").is_none() {
            if let Some(root) = &self.cfg.root {
                case.base_dir = root.clone();
            }
        }
        self.check_case_paths(&case)?;
        let id = format!("s{}", self.next.fetch_add(1, Ordering::SeqCst));
        let session = Arc::new(Session {
            id: id.clone(),
            inner: Mutex::new(SessionInner {
                case,
                sim: None,
                log: VecDeque::new(),
                journal: vec![create_entry],
                workflow: None,
            }),
            ctl: Arc::new(RunCtl {
                state: Mutex::new(RunState::Defined),
                cv: Condvar::new(),
                stop: AtomicBool::new(false),
                pause: AtomicBool::new(false),
            }),
            worker: Mutex::new(None),
        });
        lock(&self.sessions).insert(id.clone(), session);
        self.events.emit(&id, "created", json!({}));
        Ok(json!({"session": id}))
    }

    fn initialize(&self, s: &Arc<Session>) -> Rpc<()> {
        let case = lock(&s.inner).case.clone();
        self.check_case_paths(&case)?;
        let sim = Simulation::new(&case).map_err(RpcError::validation)?;
        if sim.geom.n_cells() > self.cfg.max_cells {
            return Err(RpcError::new(
                code::LIMIT,
                format!("mesh has {} cells, limit is {}", sim.geom.n_cells(), self.cfg.max_cells),
            ));
        }
        let n = sim.geom.n_cells();
        let mut inner = lock(&s.inner);
        inner.sim = Some(Arc::new(Mutex::new(sim)));
        inner.log.push_back(format!("initialised simulation with {n} cells"));
        drop(inner);
        s.ctl.stop.store(false, Ordering::SeqCst);
        s.ctl.pause.store(false, Ordering::SeqCst);
        s.set_state(RunState::Ready);
        self.events.emit(&s.id, "initialized", json!({"cells": n}));
        Ok(())
    }

    fn status(&self, s: &Session) -> Value {
        let state = s.state();
        let mut v = json!({"session": s.id, "state": state.name()});
        if let RunState::Failed(m) = &state {
            v["error"] = json!(m);
        }
        if let RunState::Finished { converged } = &state {
            v["converged"] = json!(converged);
        }
        if let Some(sim) = &lock(&s.inner).sim {
            v["simulation"] = lock(sim).status_json();
        }
        v
    }

    fn wait_state(&self, s: &Session, pred: impl Fn(&RunState) -> bool, timeout: Duration) {
        let deadline = Instant::now() + timeout;
        let mut g = lock(&s.ctl.state);
        while !pred(&g) {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                break;
            }
            let (ng, _) = s.ctl.cv.wait_timeout(g, left).unwrap_or_else(|e| e.into_inner());
            g = ng;
        }
    }

    fn stop_worker(&self, s: &Arc<Session>, silent: bool) {
        s.ctl.stop.store(true, Ordering::SeqCst);
        s.ctl.pause.store(false, Ordering::SeqCst);
        let handle = lock(&s.worker).take();
        if let Some(h) = handle {
            let _ = h.join();
        } else if matches!(s.state(), RunState::Ready | RunState::Paused) && !silent {
            s.set_state(RunState::Stopped);
            self.events.emit(&s.id, "stopped", json!({}));
        }
    }

    fn run_start(self: &Arc<Self>, p: &Value) -> Rpc {
        let s = self.session(p)?;
        self.require_idle(&s)?;
        let max_steps = u_param(p, "max_steps", u64::MAX)?;
        if max_steps != u64::MAX && max_steps > self.cfg.max_steps_per_call as u64 {
            return Err(RpcError::new(
                code::LIMIT,
                format!(
                    "max_steps {max_steps} exceeds the limit {}",
                    self.cfg.max_steps_per_call
                ),
            ));
        }
        let limit = max_steps.min(self.cfg.max_steps_per_call as u64);
        if lock(&s.inner).sim.is_none()
            || matches!(
                s.state(),
                RunState::Finished { .. } | RunState::Stopped | RunState::Failed(_)
            )
        {
            self.initialize(&s)?;
        }
        let sim = self.sim(&s)?;
        s.ctl.stop.store(false, Ordering::SeqCst);
        s.ctl.pause.store(false, Ordering::SeqCst);
        s.set_state(RunState::Running);
        self.events.emit(&s.id, "started", json!({"max_steps": limit}));
        let (svc, sess) = (self.clone(), s.clone());
        let h = std::thread::spawn(move || svc.worker(&sess, &sim, limit));
        *lock(&s.worker) = Some(h);
        Ok(self.status(&s))
    }

    fn worker(&self, s: &Arc<Session>, sim: &Arc<Mutex<Simulation>>, limit: u64) {
        let mut taken = 0u64;
        let started = Instant::now();
        loop {
            if self
                .cfg
                .max_run_seconds
                .is_some_and(|m| started.elapsed().as_secs_f64() > m)
            {
                s.set_state(RunState::Stopped);
                self.events
                    .emit(&s.id, "limit", json!({"reason": "max_run_seconds exceeded"}));
                self.events.emit(&s.id, "stopped", json!({}));
                return;
            }
            if s.ctl.stop.load(Ordering::SeqCst) {
                s.set_state(RunState::Stopped);
                self.events.emit(&s.id, "stopped", json!({}));
                return;
            }
            if s.ctl.pause.load(Ordering::SeqCst) {
                if s.state() == RunState::Running {
                    s.set_state(RunState::Paused);
                    self.events.emit(&s.id, "paused", json!({}));
                }
                std::thread::sleep(Duration::from_millis(5));
                continue;
            }
            if s.state() == RunState::Paused {
                s.set_state(RunState::Running);
            }
            if taken >= limit {
                s.set_state(RunState::Ready);
                self.events.emit(&s.id, "chunk_done", json!({"steps": taken}));
                return;
            }
            let step = {
                let mut g = lock(sim);
                if g.is_done() {
                    let converged = g.is_converged() || !g.is_steady();
                    drop(g);
                    s.set_state(RunState::Finished { converged });
                    self.events.emit(&s.id, "finished", json!({"converged": converged}));
                    return;
                }
                match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| g.step())) {
                    Ok(st) => st,
                    Err(_) => {
                        drop(g);
                        s.set_state(RunState::Failed("solver panicked".into()));
                        self.events.emit(&s.id, "error", json!({"message": "solver panicked"}));
                        return;
                    }
                }
            };
            taken += 1;
            self.record(s, &step);
            if !step.res_u.is_finite() || !step.res_p.is_finite() {
                s.set_state(RunState::Failed("residual is not finite (diverged)".into()));
                self.events.emit(&s.id, "error", json!({"message": "diverged"}));
                return;
            }
        }
    }

    fn record(&self, s: &Session, step: &crate::sim::StepSummary) {
        let line = format!(
            "{} {:>6}  res_u {:.3e}  res_p {:.3e}  continuity {:.2e}",
            if step.time > 0.0 { "step" } else { "iter" },
            step.index,
            step.res_u,
            step.res_p,
            step.continuity
        );
        let mut inner = lock(&s.inner);
        inner.log.push_back(line);
        while inner.log.len() > 2000 {
            inner.log.pop_front();
        }
        drop(inner);
        self.events.emit(
            &s.id,
            "iteration",
            json!({"index": step.index, "time": step.time, "res_u": step.res_u, "res_p": step.res_p, "continuity": step.continuity}),
        );
    }

    fn run_step(&self, p: &Value) -> Rpc {
        let s = self.session(p)?;
        self.require_idle(&s)?;
        let count = u_param(p, "count", 1)?;
        if count > self.cfg.max_steps_per_call as u64 {
            return Err(RpcError::new(
                code::LIMIT,
                format!("count {count} exceeds the limit {}", self.cfg.max_steps_per_call),
            ));
        }
        if lock(&s.inner).sim.is_none()
            || matches!(
                s.state(),
                RunState::Finished { .. } | RunState::Stopped | RunState::Failed(_)
            )
        {
            self.initialize(&s)?;
        }
        let sim = self.sim(&s)?;
        for _ in 0..count {
            let step = {
                let mut g = lock(&sim);
                if g.is_done() {
                    break;
                }
                g.step()
            };
            self.record(&s, &step);
        }
        let done = lock(&sim).is_done();
        if done {
            let converged = {
                let g = lock(&sim);
                g.is_converged() || !g.is_steady()
            };
            s.set_state(RunState::Finished { converged });
            self.events.emit(&s.id, "finished", json!({"converged": converged}));
        }
        Ok(self.status(&s))
    }

    /// Handle one JSON-RPC request object; returns the response (None for notifications).
    pub fn handle_request(self: &Arc<Self>, req: &Value) -> Option<Value> {
        let id = req.get("id").cloned();
        let valid = req.get("jsonrpc") == Some(&json!("2.0")) && req.get("method").and_then(Value::as_str).is_some();
        let resp = if !valid {
            Err(RpcError::new(code::INVALID_REQUEST, "not a JSON-RPC 2.0 request"))
        } else {
            self.call(
                req["method"].as_str().unwrap_or(""),
                req.get("params").unwrap_or(&Value::Null),
            )
        };
        let id = id?;
        Some(match resp {
            Ok(r) => json!({"jsonrpc": "2.0", "id": id, "result": r}),
            Err(e) => json!({"jsonrpc": "2.0", "id": id, "error": e.to_json()}),
        })
    }

    /// Serve one connection: newline-delimited JSON-RPC requests in, responses out.
    pub fn serve_stream(self: &Arc<Self>, reader: impl std::io::Read, mut writer: impl Write) -> std::io::Result<()> {
        for line in BufReader::new(reader).lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let resp = match serde_json::from_str::<Value>(&line) {
                Ok(req) => self.handle_request(&req),
                Err(e) => Some(json!({
                    "jsonrpc": "2.0", "id": null,
                    "error": RpcError::new(code::PARSE, format!("parse error: {e}")).to_json(),
                })),
            };
            if let Some(r) = resp {
                writeln!(writer, "{r}")?;
                writer.flush()?;
            }
        }
        Ok(())
    }

    /// Listen on `addr` (use port 0 for an ephemeral port); one thread per connection.
    pub fn serve_tcp(self: &Arc<Self>, addr: &str) -> std::io::Result<TcpServer> {
        let listener = TcpListener::bind(addr)?;
        let local = listener.local_addr()?;
        let stop = Arc::new(AtomicBool::new(false));
        let (svc, flag) = (self.clone(), stop.clone());
        let accept = std::thread::spawn(move || {
            for conn in listener.incoming() {
                if flag.load(Ordering::SeqCst) {
                    break;
                }
                if let Ok(stream) = conn {
                    let svc = svc.clone();
                    std::thread::spawn(move || {
                        if let Ok(r) = stream.try_clone() {
                            let _ = svc.serve_stream(r, stream);
                        }
                    });
                }
            }
        });
        Ok(TcpServer {
            addr: local,
            stop,
            accept: Some(accept),
        })
    }

    /// Stop every running worker (used on shutdown).
    pub fn shutdown(&self) {
        let sessions: Vec<Arc<Session>> = lock(&self.sessions).values().cloned().collect();
        for s in sessions {
            s.ctl.stop.store(true, Ordering::SeqCst);
            if let Some(h) = lock(&s.worker).take() {
                let _ = h.join();
            }
        }
    }
}

/// Handle to a listening TCP service.
pub struct TcpServer {
    /// Bound address.
    pub addr: SocketAddr,
    stop: Arc<AtomicBool>,
    accept: Option<std::thread::JoinHandle<()>>,
}

impl TcpServer {
    /// Block until the server is stopped (normally never).
    pub fn wait(mut self) {
        if let Some(h) = self.accept.take() {
            let _ = h.join();
        }
    }

    /// Stop accepting connections.
    pub fn stop(mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(self.addr);
        if let Some(h) = self.accept.take() {
            let _ = h.join();
        }
    }
}

/// Methods advertised by `service.info`.
pub const METHODS: &[&str] = &[
    "service.info",
    "settings.schema",
    "session.create",
    "session.list",
    "session.close",
    "settings.get",
    "settings.set",
    "settings.remove",
    "settings.validate",
    "mesh.check",
    "mesh.partition",
    "mesh.inflate",
    "field.sample",
    "run.initialize",
    "run.start",
    "run.pause",
    "run.resume",
    "run.stop",
    "run.step",
    "run.status",
    "run.wait",
    "monitors.get",
    "reports.get",
    "report.export",
    "field.stats",
    "field.slice",
    "field.export_vtk",
    "render.png",
    "render.glb",
    "log.tail",
    "journal.get",
    "journal.replay",
    "field.grid",
    "panel.snapshot",
    "events.poll",
    "workflow.create",
    "workflow.list",
    "workflow.get",
    "workflow.run",
    "workflow.insert",
    "workflow.delete",
    "workflow.save",
    "workflow.load",
    "run.check",
    "run.headless",
    "status.read",
];

/// Minimal blocking client for the line protocol (tests, CLI, scripts).
pub struct Client {
    reader: BufReader<TcpStream>,
    writer: TcpStream,
    next: u64,
}

impl Client {
    /// Connect to a service.
    pub fn connect(addr: SocketAddr) -> std::io::Result<Client> {
        let stream = TcpStream::connect(addr)?;
        stream.set_read_timeout(Some(Duration::from_secs(120)))?;
        Ok(Client {
            reader: BufReader::new(stream.try_clone()?),
            writer: stream,
            next: 1,
        })
    }

    /// Call a method; returns the result or the RPC error.
    pub fn call(&mut self, method: &str, params: Value) -> std::result::Result<Value, RpcError> {
        let id = self.next;
        self.next += 1;
        let req = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        writeln!(self.writer, "{req}").map_err(|e| RpcError::new(code::INTERNAL, e.to_string()))?;
        let mut line = String::new();
        self.reader
            .read_line(&mut line)
            .map_err(|e| RpcError::new(code::INTERNAL, e.to_string()))?;
        let v: Value = serde_json::from_str(&line).map_err(|e| RpcError::new(code::PARSE, e.to_string()))?;
        if let Some(e) = v.get("error") {
            return Err(RpcError {
                code: e["code"].as_i64().unwrap_or(code::INTERNAL),
                message: e["message"].as_str().unwrap_or("").into(),
                data: e.get("data").cloned(),
            });
        }
        Ok(v["result"].clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::tests_support::CAVITY_TOML;

    fn started() -> (Arc<Service>, String) {
        let svc = Service::new(ServiceConfig::default());
        let r = svc.call("session.create", &json!({"toml": CAVITY_TOML})).unwrap();
        (svc, r["session"].as_str().unwrap().to_string())
    }

    #[test]
    fn base64_matches_known_vectors() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn in_process_run_to_convergence_with_monitors_reports_and_render() {
        let (svc, id) = started();
        let r = svc.call("run.start", &json!({"session": id})).unwrap();
        assert_eq!(r["state"], "running");
        let done = svc
            .call("run.wait", &json!({"session": id, "timeout_ms": 60000}))
            .unwrap();
        assert_eq!(done["state"], "finished", "{done}");
        assert_eq!(done["converged"], true);
        let m = svc
            .call("monitors.get", &json!({"session": id, "names": ["res_u"], "since": 5}))
            .unwrap();
        let xs = m["series"]["res_u"]["x"].as_array().unwrap();
        assert!(xs[0].as_f64().unwrap() > 5.0);
        assert!(m["series"].get("res_p").is_none());
        let rep = svc.call("reports.get", &json!({"session": id})).unwrap();
        assert!(rep["reports"][0]["value"].as_f64().unwrap() < 0.0);
        let img = svc
            .call("render.png", &json!({"session": id, "field": "speed", "vectors": true}))
            .unwrap();
        assert_eq!(img["mime"], "image/png");
        assert!(img["data_base64"].as_str().unwrap().starts_with("iVBORw0KGgo"));
        let glb = svc
            .call("render.glb", &json!({"session": id, "field": "speed", "inline": true}))
            .unwrap();
        assert_eq!(glb["mime"], "model/gltf-binary");
        assert!(glb["data_base64"].as_str().unwrap().starts_with("Z2xU"));
        assert!(glb["faces"].as_u64().unwrap() > 0 && glb["max"].as_f64().unwrap() > glb["min"].as_f64().unwrap());
        assert!(svc
            .call("render.glb", &json!({"session": id, "patches": ["nope"]}))
            .is_err());
        let html = svc.call("report.export", &json!({"session": id})).unwrap();
        assert!(html["content"].as_str().unwrap().contains("<polyline"));
        let stats = svc.call("field.stats", &json!({"session": id, "name": "p"})).unwrap();
        assert!(stats["max"].as_f64().unwrap() > stats["min"].as_f64().unwrap());
        let log = svc.call("log.tail", &json!({"session": id, "lines": 3})).unwrap();
        assert_eq!(log["lines"].as_array().unwrap().len(), 3);
        svc.shutdown();
    }

    #[test]
    fn pause_resume_stop_and_state_errors() {
        let (svc, id) = started();
        svc.call(
            "settings.set",
            &json!({"session": id, "path": "solver.max_iterations", "value": 100000}),
        )
        .unwrap();
        svc.call(
            "settings.set",
            &json!({"session": id, "path": "solver.tolerance_u", "value": 1e-14}),
        )
        .unwrap();
        svc.call(
            "settings.set",
            &json!({"session": id, "path": "solver.tolerance_p", "value": 1e-14}),
        )
        .unwrap();
        svc.call("run.start", &json!({"session": id})).unwrap();
        let p = svc.call("run.pause", &json!({"session": id})).unwrap();
        assert_eq!(p["state"], "paused");
        let it1 = p["simulation"]["iteration"].as_u64().unwrap();
        std::thread::sleep(Duration::from_millis(60));
        let it2 = svc.call("run.status", &json!({"session": id})).unwrap()["simulation"]["iteration"]
            .as_u64()
            .unwrap();
        assert_eq!(it1, it2, "no progress while paused");
        let e = svc
            .call("settings.set", &json!({"session": id, "path": "mesh.nx", "value": 8}))
            .unwrap_err();
        assert_eq!(e.code, code::BAD_STATE);
        let e = svc.call("run.start", &json!({"session": id})).unwrap_err();
        assert_eq!(e.code, code::BAD_STATE);
        assert_eq!(
            svc.call("run.resume", &json!({"session": id})).unwrap()["state"],
            "running"
        );
        let s = svc.call("run.stop", &json!({"session": id})).unwrap();
        assert_eq!(s["state"], "stopped");
        let e = svc.call("run.pause", &json!({"session": id})).unwrap_err();
        assert_eq!(e.code, code::BAD_STATE);
        // settings can change again after a stop, which discards the simulation
        svc.call("settings.set", &json!({"session": id, "path": "mesh.nx", "value": 8}))
            .unwrap();
        assert_eq!(
            svc.call("run.status", &json!({"session": id})).unwrap()["state"],
            "defined"
        );
        svc.shutdown();
    }

    #[test]
    fn synchronous_steps_validation_and_limits() {
        let svc = Service::new(ServiceConfig {
            max_cells: 100,
            max_steps_per_call: 50,
            ..Default::default()
        });
        let id = svc.call("session.create", &json!({"toml": CAVITY_TOML})).unwrap()["session"]
            .as_str()
            .unwrap()
            .to_string();
        // 12x12 = 144 cells > 100
        let e = svc.call("run.step", &json!({"session": id})).unwrap_err();
        assert_eq!(e.code, code::LIMIT);
        svc.call("settings.set", &json!({"session": id, "path": "mesh.nx", "value": 8}))
            .unwrap();
        let st = svc.call("run.step", &json!({"session": id, "count": 3})).unwrap();
        assert_eq!(st["simulation"]["iteration"], 3);
        let e = svc.call("run.step", &json!({"session": id, "count": 51})).unwrap_err();
        assert_eq!(e.code, code::LIMIT);
        let e = svc
            .call(
                "settings.set",
                &json!({"session": id, "path": "solver.alpha_u", "value": 9.0}),
            )
            .unwrap_err();
        assert_eq!(e.code, code::VALIDATION);
        assert!(e.data.unwrap()["diagnostics"][0]["path"] == "solver.alpha_u");
        let e = svc
            .call("session.create", &json!({"toml": "[mesh]\nnx = \"x\"\n"}))
            .unwrap_err();
        assert_eq!(e.code, code::VALIDATION);
        assert_eq!(svc.call("nope", &json!({})).unwrap_err().code, code::METHOD_NOT_FOUND);
        assert_eq!(
            svc.call("run.status", &json!({"session": "zzz"})).unwrap_err().code,
            code::NO_SESSION
        );
        assert_eq!(
            svc.call("run.status", &json!({})).unwrap_err().code,
            code::INVALID_PARAMS
        );
    }

    #[test]
    fn sandbox_blocks_paths_outside_the_root() {
        let root = std::env::temp_dir().join(format!("rcfd_sbx_{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let svc = Service::new(ServiceConfig {
            root: Some(root.clone()),
            ..Default::default()
        });
        assert!(svc.sandbox_path("a/b.vtu").is_ok());
        assert_eq!(svc.sandbox_path("../x").unwrap_err().code, code::LIMIT);
        let outside = std::env::temp_dir().join("elsewhere.toml");
        assert_eq!(
            svc.sandbox_path(outside.to_str().unwrap()).unwrap_err().code,
            code::LIMIT
        );
        let e = svc
            .call("session.create", &json!({"path": "../../etc/passwd"}))
            .unwrap_err();
        assert_eq!(e.code, code::LIMIT);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn mesh_file_outside_sandbox_and_runtime_cap_are_enforced() {
        let root = std::env::temp_dir().join(format!("rcfd_sbx2_{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let svc = Service::new(ServiceConfig {
            root: Some(root.clone()),
            max_run_seconds: Some(0.05),
            ..Default::default()
        });
        let id = svc.call("session.create", &json!({"toml": CAVITY_TOML})).unwrap()["session"]
            .as_str()
            .unwrap()
            .to_string();
        let e = svc
            .call(
                "settings.set",
                &json!({"session": id, "path": "mesh.file", "value": "../../secret.msh"}),
            )
            .unwrap_err();
        assert_eq!(e.code, code::LIMIT);
        assert_eq!(
            svc.call("settings.get", &json!({"session": id, "path": "mesh.file"}))
                .unwrap()["value"],
            ""
        );
        svc.call(
            "settings.set",
            &json!({"session": id, "path": "solver.tolerance_u", "value": 1e-14}),
        )
        .unwrap();
        svc.call(
            "settings.set",
            &json!({"session": id, "path": "solver.tolerance_p", "value": 1e-14}),
        )
        .unwrap();
        svc.call(
            "settings.set",
            &json!({"session": id, "path": "solver.max_iterations", "value": 1000000}),
        )
        .unwrap();
        svc.call("settings.set", &json!({"session": id, "path": "mesh.nx", "value": 40}))
            .unwrap();
        svc.call("settings.set", &json!({"session": id, "path": "mesh.ny", "value": 40}))
            .unwrap();
        svc.call("run.start", &json!({"session": id})).unwrap();
        let st = svc
            .call("run.wait", &json!({"session": id, "timeout_ms": 20000}))
            .unwrap();
        assert_eq!(st["state"], "stopped");
        let ev = svc.call("events.poll", &json!({"after": 0, "session": id})).unwrap();
        assert!(ev["events"].as_array().unwrap().iter().any(|e| e["kind"] == "limit"));
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn panel_snapshot_and_field_grid() {
        let (svc, id) = started();
        svc.call("run.step", &json!({"session": id, "count": 4})).unwrap();
        let snap = svc
            .call(
                "panel.snapshot",
                &json!({"session": id, "since": 1, "field": "ux", "cols": 10, "rows": 6, "log_lines": 2}),
            )
            .unwrap();
        assert_eq!(snap["status"]["simulation"]["iteration"], 4);
        assert_eq!(snap["log"].as_array().unwrap().len(), 2);
        assert_eq!(snap["monitors"]["series"]["res_u"]["x"].as_array().unwrap().len(), 3);
        let g = &snap["grid"];
        let vals = g["values"].as_array().unwrap();
        assert_eq!(vals.len(), 60);
        assert!(vals.iter().all(|v| !v.is_null()));
        // the moving lid is the top row and carries the largest ux
        let mean = |r: &[Value]| r.iter().map(|v| v.as_f64().unwrap()).sum::<f64>() / r.len() as f64;
        assert!(mean(&vals[..10]) > mean(&vals[50..]));
        let empty = svc.call("session.create", &json!({})).unwrap()["session"]
            .as_str()
            .unwrap()
            .to_string();
        let s = svc.call("panel.snapshot", &json!({"session": empty})).unwrap();
        assert_eq!(s["status"]["state"], "defined");
        assert!(s.get("monitors").is_none());
    }

    #[test]
    fn named_entries_can_be_removed_and_replay_reproduces_it() {
        let (svc, id) = started();
        svc.call(
            "settings.set",
            &json!({"session": id, "path": "reports.m.type", "value": "volume_average"}),
        )
        .unwrap();
        svc.call(
            "settings.set",
            &json!({"session": id, "path": "reports.m.field", "value": "speed"}),
        )
        .unwrap();
        svc.call("settings.remove", &json!({"session": id, "path": "reports.m"}))
            .unwrap();
        let names = svc
            .call("settings.get", &json!({"session": id, "path": "reports"}))
            .unwrap();
        assert!(names["value"].get("m").is_none());
        assert!(names["value"].get("lid_drag").is_some());
        // removing something absent is a validation error, not a crash
        assert_eq!(
            svc.call("settings.remove", &json!({"session": id, "path": "reports.m"}))
                .unwrap_err()
                .code,
            1003
        );
        let j = svc.call("journal.get", &json!({"session": id})).unwrap();
        let r = svc.call("journal.replay", &json!({"journal": j})).unwrap();
        assert_eq!(r["matches"], true);
    }

    #[test]
    fn recorded_journal_replays_to_the_same_case_hash() {
        let (svc, id) = started();
        for (path, value) in [
            ("mesh.nx", json!(20)),
            ("solver.alpha_u", json!(0.7)),
            ("boundaries.inlet.velocity", json!("zero_gradient")),
            ("reports.m.type", json!("volume_average")),
            ("reports.m.field", json!("speed")),
        ] {
            svc.call("settings.set", &json!({"session": id, "path": path, "value": value}))
                .unwrap();
        }
        // a rejected change is not journaled
        svc.call(
            "settings.set",
            &json!({"session": id, "path": "solver.alpha_u", "value": 9.0}),
        )
        .unwrap_err();
        let j = svc.call("journal.get", &json!({"session": id})).unwrap();
        assert_eq!(j["entries"].as_array().unwrap().len(), 6);
        // through text, as a saved file would be
        let saved: Value = serde_json::from_str(&j.to_string()).unwrap();
        let r = svc.call("journal.replay", &json!({"journal": saved})).unwrap();
        assert_eq!(r["matches"], true);
        assert_eq!(r["case_hash"], j["case_hash"]);
        let other = r["session"].as_str().unwrap();
        assert_eq!(
            svc.call("settings.get", &json!({"session": other, "path": "solver.alpha_u"}))
                .unwrap()["value"],
            0.7
        );
        // a tampered journal no longer matches
        let mut bad = j.clone();
        bad["entries"][1]["value"] = json!(25);
        assert_eq!(
            svc.call("journal.replay", &json!({"journal": bad})).unwrap()["matches"],
            false
        );
    }

    #[test]
    fn mesh_check_and_validate() {
        let (svc, id) = started();
        let m = svc.call("mesh.check", &json!({"session": id})).unwrap();
        assert_eq!(m["ok"], true);
        assert_eq!(m["report"]["n_cells"], 144);
        let v = svc.call("settings.validate", &json!({"session": id})).unwrap();
        assert_eq!(v["ok"], true);
        svc.call(
            "settings.set",
            &json!({"session": id, "path": "reports.lid_drag.patch", "value": "nowhere"}),
        )
        .unwrap();
        let v = svc.call("settings.validate", &json!({"session": id})).unwrap();
        assert_eq!(v["ok"], false);
        assert_eq!(v["diagnostics"][0]["path"], "reports.lid_drag.patch");
    }

    #[test]
    fn handle_request_follows_json_rpc_rules() {
        let svc = Service::new(ServiceConfig::default());
        let r = svc
            .handle_request(&json!({"jsonrpc": "2.0", "id": 7, "method": "service.info"}))
            .unwrap();
        assert_eq!(r["id"], 7);
        assert_eq!(r["result"]["protocol"], PROTOCOL);
        assert!(svc
            .handle_request(&json!({"jsonrpc": "2.0", "method": "service.info"}))
            .is_none());
        let bad = svc.handle_request(&json!({"id": 1, "method": "x"})).unwrap();
        assert_eq!(bad["error"]["code"], code::INVALID_REQUEST);
    }
}
