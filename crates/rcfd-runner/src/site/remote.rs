//! The local runner's side of remote runs (ADR 0739 P4, P6, P7): one worker thread per site, holding the link to the
//! site's agent while that site has runs to follow, and dropping it when it has none.
//!
//! For each run the worker sends the inputs (`rcfd check --inputs`), submits the run, then keeps the local run folder up
//! to date from the site every `poll_ms`: the new bytes of `events.jsonl`, `residuals.jsonl` and `log.txt` from the
//! local length (append-only files, so the copy is exact and survives a reconnection), `pause.ack` and `status.json`.
//! A local `pause` or `cancel` is forwarded. When the site's run has ended, `output/`, `fields/`, `metrics.json` and the
//! launch records are downloaded (resumable, each file checked by hash and length) and the final `status.json` is
//! written last. What only the link knows goes to `site.json` (the site's facts, the remote folder, last contact) and
//! `site.jsonl` (link events). A lost link is retried with backoff (100 ms doubling to 5 s, then every 30 s); no state is
//! invented locally.

use super::link::AgentLink;
use super::sites::{self, SiteEntry};
use super::{
    blob_of_file, engine_id, lexical_rel, plain_rel, BlobRef, InputFile, ParentRun, ProjectRef, RemoteId, RemoteRun,
    Request, Site, SiteError, SiteFacts, CHUNK,
};
use crate::records::{self, LaunchSettings, RunSettings, RunState, RunStatus};
use crate::store::{extra, state_event, FsStore, RunStore};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// The link's own record of a remote run.
pub const SITE_JSON: &str = "site.json";
/// The link events of a remote run.
pub const SITE_JSONL: &str = "site.jsonl";

/// The remote runs of one local runner.
pub struct RemoteRuns {
    project: PathBuf,
    solver: PathBuf,
    poll: Duration,
    stop: Arc<AtomicBool>,
    workers: HashMap<String, Sender<String>>,
    handles: Vec<JoinHandle<()>>,
}

impl RemoteRuns {
    /// For the project folder `project`, whose local solver is `solver` (for `rcfd check --inputs` and this machine's
    /// engine), polling every `poll`. No thread starts until a run is handed over.
    pub fn new(project: &Path, solver: &Path, poll: Duration) -> Self {
        RemoteRuns {
            project: project.to_path_buf(),
            solver: solver.to_path_buf(),
            poll,
            stop: Arc::new(AtomicBool::new(false)),
            workers: HashMap::new(),
            handles: Vec::new(),
        }
    }

    /// Follow run `id` on site `site` from now on (its worker starts with the first run of the site).
    pub fn take(&mut self, id: &str, site: &str) {
        if let Some(tx) = self.workers.get(site) {
            if tx.send(id.to_string()).is_ok() {
                return;
            }
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = tx.send(id.to_string());
        let w = Worker {
            store: FsStore::new(&self.project),
            solver: self.solver.clone(),
            site: site.to_string(),
            poll: self.poll,
            runs: Vec::new(),
            link: None,
            facts: None,
            next_try: Instant::now(),
            delay: Duration::ZERO,
            local_engine: None,
        };
        let stop = Arc::clone(&self.stop);
        self.handles.push(std::thread::spawn(move || w.run(rx, stop)));
        self.workers.insert(site.to_string(), tx);
    }

    /// Stop following (the remote runs go on; the next runner catches up from `site.json`).
    pub fn shutdown(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        self.workers.clear();
        for h in self.handles.drain(..) {
            let _ = h.join();
        }
    }
}

impl Drop for RemoteRuns {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// One remote run as the worker follows it.
struct Mirror {
    id: String,
    dir: PathBuf,
    remote: RemoteId,
    submitted: bool,
    lost: bool,
    sent_pause: Option<bool>,
    sent_cancel: bool,
    last_status: Vec<u8>,
    last_contact: Option<Instant>,
    said: HashSet<String>,
}

enum Step {
    Go,
    Done,
}

struct Worker {
    store: FsStore,
    solver: PathBuf,
    site: String,
    poll: Duration,
    runs: Vec<Mirror>,
    link: Option<AgentLink>,
    facts: Option<SiteFacts>,
    next_try: Instant,
    delay: Duration,
    local_engine: Option<String>,
}

/// Append one line to `runs/<id>/site.jsonl` (best effort).
fn site_event(dir: &Path, kind: &str, fields: Value) {
    let mut v = json!({"t": records::now_iso(), "kind": kind});
    if let (Some(o), Value::Object(f)) = (v.as_object_mut(), fields) {
        o.extend(f);
    }
    let _ = records::append_line(&dir.join(SITE_JSONL), &v.to_string());
}

/// Update `runs/<id>/site.json` with `fields` (atomically; best effort).
fn site_json(dir: &Path, fields: Value) {
    let p = dir.join(SITE_JSON);
    let mut v: Value = std::fs::read(&p)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_else(|| json!({}));
    if let (Some(o), Value::Object(f)) = (v.as_object_mut(), fields) {
        o.extend(f);
    }
    let mut bytes = serde_json::to_vec_pretty(&v).unwrap_or_default();
    bytes.push(b'\n');
    let _ = records::atomic_write(&p, &bytes);
}

/// The key of a project's mirror on a site: `<name>-<fnv64 of the local absolute path and created>`.
pub fn project_key(project: &Path) -> (String, String, Option<String>) {
    let manifest = std::fs::read_to_string(project.join("rcfd-project.json")).ok();
    let m: Value = manifest
        .as_deref()
        .and_then(|t| serde_json::from_str(t).ok())
        .unwrap_or(Value::Null);
    let abs = rcfd_project::paths::canonicalize(project).unwrap_or_else(|_| project.to_path_buf());
    let name = m["name"]
        .as_str()
        .map(String::from)
        .or_else(|| abs.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "project".into());
    let clean: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .take(40)
        .collect();
    let clean = if clean.is_empty() || clean.starts_with('.') {
        "project".into()
    } else {
        clean
    };
    let seed = format!("{}\n{}", abs.display(), m["created"].as_str().unwrap_or_default());
    let h = super::blob_of(seed.as_bytes()).hash;
    (format!("{clean}-{h}"), name, manifest)
}

/// This machine's engine as `<version> (<git_rev>)`.
pub fn local_engine(solver: &Path) -> String {
    let d = crate::build::describe(solver);
    engine_id(&json!({"version": d["version"], "git_rev": d["build"]["git_rev"]}))
}

/// Whether a site may take a run (ADR 0739 Q10 (a)): its protocol's major is this one's, and its engine is this
/// machine's or one the user said yes to for this site. `Ok(true)` when the engine differs and was accepted.
pub fn admit(name: &str, file: &Path, entry: &SiteEntry, facts: &SiteFacts, local: &str) -> Result<bool, String> {
    let ours = super::protocol_major(super::PROTOCOL);
    if facts.major() != ours {
        return Err(format!(
            "site {name} ({} [site.{name}], host {}) speaks {}, this rcfd-runner speaks {}: install a matching engine \
             on the site (engine = {})",
            file.display(),
            facts.host,
            facts.protocol,
            super::PROTOCOL,
            entry.engine
        ));
    }
    let theirs = facts.engine_id();
    if theirs == local {
        return Ok(false);
    }
    if entry.accept_engines.contains(&theirs) {
        return Ok(true);
    }
    Err(format!(
        "site {name} runs the engine {theirs}, this machine {local}; results are bit-identical only for one build. \
         Run `rcfd site check {name} --confirm` to see it and say yes for this site and version"
    ))
}

impl Worker {
    fn run(mut self, rx: Receiver<String>, stop: Arc<AtomicBool>) {
        while !stop.load(Ordering::SeqCst) {
            while let Ok(id) = rx.try_recv() {
                if !self.runs.iter().any(|m| m.id == id) {
                    let m = self.mirror(&id);
                    self.runs.push(m);
                }
            }
            if self.runs.is_empty() {
                self.link = None;
                std::thread::sleep(self.poll);
                continue;
            }
            if self.link.is_none() && !self.connect() {
                std::thread::sleep(self.poll.min(Duration::from_millis(100)));
                continue;
            }
            let mut keep = Vec::new();
            let runs = std::mem::take(&mut self.runs);
            let mut lost: Option<String> = None;
            for mut m in runs {
                if lost.is_some() {
                    keep.push(m);
                    continue;
                }
                match self.step(&mut m) {
                    Ok(Step::Done) => {}
                    Ok(Step::Go) => keep.push(m),
                    Err(SiteError::Link(e)) => {
                        lost = Some(e);
                        keep.push(m);
                    }
                    Err(SiteError::Refused(e)) => {
                        if m.submitted {
                            if m.said.insert(e.clone()) {
                                site_event(&m.dir, "error", json!({"site": self.site, "message": e}));
                            }
                            keep.push(m);
                        } else {
                            self.fail(&m, RunState::Failed, &e);
                        }
                    }
                }
            }
            self.runs = keep;
            if let Some(e) = lost {
                self.drop_link(&e);
            }
            std::thread::sleep(self.poll);
        }
        self.link = None;
    }

    fn mirror(&self, id: &str) -> Mirror {
        let dir = self.store.run_dir(id);
        let sj: Value = std::fs::read(dir.join(SITE_JSON))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or(Value::Null);
        let (key, _, _) = project_key(self.store.project());
        Mirror {
            id: id.to_string(),
            remote: RemoteId {
                project: sj["project_key"].as_str().map_or(key, String::from),
                id: id.to_string(),
            },
            submitted: sj["submitted"].as_bool().unwrap_or(false),
            dir,
            lost: false,
            sent_pause: None,
            sent_cancel: false,
            last_status: Vec::new(),
            last_contact: None,
            said: HashSet::new(),
        }
    }

    /// Open the link when the backoff allows; `false` when there is none now.
    fn connect(&mut self) -> bool {
        if Instant::now() < self.next_try {
            return false;
        }
        let entry = match sites::site(&self.site) {
            Ok((_, e)) => e,
            Err(m) => {
                for r in &mut self.runs {
                    if r.said.insert(m.clone()) {
                        site_event(&r.dir, "unconfigured", json!({"site": self.site, "message": m}));
                    }
                }
                // runs not yet sent cannot go; sent ones keep their name and data, and nothing is sent
                let unsent: Vec<String> = self
                    .runs
                    .iter()
                    .filter(|r| !r.submitted)
                    .map(|r| r.id.clone())
                    .collect();
                for id in unsent {
                    if let Some(i) = self.runs.iter().position(|r| r.id == id) {
                        let r = self.runs.remove(i);
                        self.fail(&r, RunState::Failed, &m);
                    }
                }
                self.next_try = Instant::now() + Duration::from_secs(30);
                return false;
            }
        };
        let opened = AgentLink::open(&self.site, &entry).and_then(|mut l| l.hello().map(|f| (l, f)));
        match opened {
            Ok((mut link, facts)) => {
                self.delay = Duration::ZERO;
                let keys: HashSet<String> = self
                    .runs
                    .iter()
                    .filter(|r| r.submitted)
                    .map(|r| r.remote.project.clone())
                    .collect();
                for k in keys {
                    if let Err(e) = link.ensure(&k) {
                        for r in &self.runs {
                            site_event(&r.dir, "error", json!({"site": self.site, "message": e.to_string()}));
                        }
                    }
                }
                for r in &mut self.runs {
                    site_event(
                        &r.dir,
                        "connected",
                        json!({"site": self.site, "host": facts.host, "pid": link.pid()}),
                    );
                    r.lost = false;
                    r.last_contact = None;
                }
                self.facts = Some(facts);
                self.link = Some(link);
                true
            }
            Err(e) => {
                self.drop_link(&e.to_string());
                false
            }
        }
    }

    fn drop_link(&mut self, why: &str) {
        self.link = None;
        for r in &mut self.runs {
            if !r.lost {
                r.lost = true;
                site_event(&r.dir, "lost", json!({"site": self.site, "message": why}));
            }
        }
        self.delay = if self.delay.is_zero() {
            Duration::from_millis(100)
        } else if self.delay < Duration::from_secs(5) {
            (self.delay * 2).min(Duration::from_secs(5))
        } else {
            Duration::from_secs(30)
        };
        self.next_try = Instant::now() + self.delay;
    }

    /// End a run that was never sent (or cannot be): `status.json`, the state event and `metrics.json`, as the local
    /// runner ends a run it never launched.
    fn fail(&self, m: &Mirror, state: RunState, reason: &str) {
        let from = self.store.read_status(&m.id).ok().map(|s| s.state);
        let _ = self.store.write_metrics(
            &m.id,
            &json!({"state": state, "reason": reason, "steps": 0, "launched": false, "reports": {}}),
        );
        let mut st = RunStatus::new(state);
        st.reason = Some(reason.to_string());
        let _ = self.store.write_status(&m.id, &st);
        let _ = self.store.append_event(&m.id, state_event(from, state, reason));
        for f in [extra::PAUSE, extra::PAUSE_ACK, extra::CANCEL] {
            let _ = std::fs::remove_file(m.dir.join(f));
        }
        site_event(&m.dir, "refused", json!({"site": self.site, "message": reason}));
    }

    fn step(&mut self, m: &mut Mirror) -> Result<Step, SiteError> {
        if !m.submitted {
            if m.dir.join(extra::CANCEL).exists() {
                self.fail(m, RunState::Cancelled, "cancelled before it was sent to the site");
                return Ok(Step::Done);
            }
            self.send(m)?;
            return Ok(Step::Go);
        }
        let Some(link) = self.link.as_mut() else {
            return Err(SiteError::Link("no link".into()));
        };
        copy_appended(link, m)?;
        let ack = link.read(&m.remote, extra::PAUSE_ACK, 0, 4096)?;
        let local_ack = m.dir.join(extra::PAUSE_ACK);
        if ack.exists {
            if std::fs::read(&local_ack).ok().as_deref() != Some(&ack.data[..]) {
                let _ = records::atomic_write(&local_ack, &ack.data);
            }
        } else {
            let _ = std::fs::remove_file(&local_ack);
        }
        let st = link.read(&m.remote, "status.json", 0, CHUNK)?;
        let remote: Option<RunStatus> = serde_json::from_slice(&st.data).ok();
        let ended = remote.as_ref().is_some_and(|s| s.state.is_terminal());
        if !ended {
            let pause = m.dir.join(extra::PAUSE).exists();
            if m.sent_pause != Some(pause) {
                let what = if pause { Request::Pause } else { Request::Resume };
                link.request(&m.remote, what)?;
                if m.sent_pause.is_some() || pause {
                    site_event(&m.dir, "request_sent", json!({"site": self.site, "what": what}));
                }
                m.sent_pause = Some(pause);
            }
            if !m.sent_cancel && m.dir.join(extra::CANCEL).exists() {
                link.request(&m.remote, Request::Cancel)?;
                site_event(
                    &m.dir,
                    "request_sent",
                    json!({"site": self.site, "what": Request::Cancel}),
                );
                m.sent_cancel = true;
            }
            if st.exists && remote.is_some() && st.data != m.last_status {
                atomic_status(&m.dir, &st.data);
                m.last_status = st.data;
            }
            if m.last_contact.map_or(true, |t| t.elapsed() > Duration::from_secs(10)) {
                site_json(&m.dir, json!({"last_contact": records::now_iso(), "link": "connected"}));
                m.last_contact = Some(Instant::now());
            }
            return Ok(Step::Go);
        }
        // ended on the site: everything appended, then the results, then the final status
        while copy_appended(link, m)? {}
        let (n, bytes) = download(link, m)?;
        atomic_status(&m.dir, &st.data);
        for f in [extra::PAUSE, extra::PAUSE_ACK, extra::CANCEL] {
            let _ = std::fs::remove_file(m.dir.join(f));
        }
        site_event(
            &m.dir,
            "downloaded",
            json!({"site": self.site, "files": n, "bytes": bytes}),
        );
        site_json(
            &m.dir,
            json!({"last_contact": records::now_iso(), "link": "done", "done": true,
                   "state": remote.map(|s| crate::client::state_name(s.state))}),
        );
        Ok(Step::Done)
    }

    /// Send the run's inputs and submit it.
    fn send(&mut self, m: &mut Mirror) -> Result<(), SiteError> {
        let (file, entry) = sites::site(&self.site).map_err(SiteError::Refused)?;
        let local = self
            .local_engine
            .get_or_insert_with(|| local_engine(&self.solver))
            .clone();
        let facts = self.facts.clone().ok_or_else(|| SiteError::Link("no hello".into()))?;
        let accepted = admit(&self.site, &file, &entry, &facts, &local).map_err(SiteError::Refused)?;
        let rec = self
            .store
            .read_record(&m.id)
            .map_err(|e| SiteError::Refused(format!("run {}: {e}", m.id)))?;
        let settings = RunSettings::of(&rec).map_err(SiteError::Refused)?;
        let launch = LaunchSettings::of(&rec).map_err(SiteError::Refused)?;
        // the files the case reads, as the local engine names them
        let case_dir = match &rec.case_dir {
            Some(d) => PathBuf::from(d),
            None => self
                .store
                .project()
                .join(&rec.case)
                .parent()
                .map_or_else(|| self.store.project().to_path_buf(), Path::to_path_buf),
        };
        let listed = list_inputs(&self.solver, &m.dir.join("case.snapshot"), &case_dir, &settings.set)
            .map_err(SiteError::Refused)?;
        let mut files: Vec<InputFile> = Vec::new();
        let mut sources: BTreeMap<BlobRef, PathBuf> = BTreeMap::new();
        let mut up = 0usize;
        for i in listed["inputs"].as_array().into_iter().flatten() {
            let key = i["key"].as_str().unwrap_or_default();
            let given = i["file"].as_str().unwrap_or_default();
            let (u, parts) = lexical_rel(given).ok_or_else(|| {
                SiteError::Refused(format!(
                    "{key} = \"{given}\": an absolute path cannot go to a site; write it relative to the case"
                ))
            })?;
            up = up.max(u);
            let base = PathBuf::from(i["path"].as_str().unwrap_or_default());
            for f in i["files"].as_array().into_iter().flatten() {
                let sub = f["path"].as_str().unwrap_or_default();
                let blob =
                    BlobRef::of_filehash(f["hash"].as_str().unwrap_or_default(), f["bytes"].as_u64().unwrap_or(0))
                        .ok_or_else(|| SiteError::Refused(format!("{key}: no hash for {given}/{sub}")))?;
                let mut rel: Vec<String> = std::iter::repeat("..".to_string()).take(u).collect();
                rel.extend(parts.iter().cloned());
                let src = if sub.is_empty() {
                    base.clone()
                } else {
                    rel.extend(sub.split('/').map(String::from));
                    sub.split('/').fold(base.clone(), |p, c| p.join(c))
                };
                sources.insert(blob.clone(), src);
                files.push(InputFile {
                    path: rel.join("/"),
                    blob,
                });
            }
        }
        let snapshot_path = m.dir.join("case.snapshot");
        let snapshot = blob_of_file(&snapshot_path).map_err(|e| SiteError::Refused(format!("case.snapshot: {e}")))?;
        sources.insert(snapshot.clone(), snapshot_path);
        // continue_from: the parent goes too unless it ran on this site
        let mut parent = None;
        if let Some(src) = &launch.continue_from {
            let prec = self
                .store
                .read_record(src)
                .map_err(|e| SiteError::Refused(format!("continue_from {src}: {e}")))?;
            if prec.site.as_deref() != Some(self.site.as_str()) {
                let restart = self.store.run_dir(src).join(extra::OUTPUT).join("restart.rcfd");
                let blob = blob_of_file(&restart)
                    .map_err(|e| SiteError::Refused(format!("continue_from {src}: {}: {e}", restart.display())))?;
                sources.insert(blob.clone(), restart);
                let status = std::fs::read_to_string(self.store.run_dir(src).join("status.json")).unwrap_or_default();
                let mut record = serde_json::to_value(&prec).unwrap_or_default();
                if let Some(o) = record.as_object_mut() {
                    o.remove("site");
                    o.remove("case_source");
                    o.remove("case_dir");
                }
                parent = Some(ParentRun {
                    id: src.clone(),
                    record,
                    status,
                    restart: blob,
                });
            }
        }
        let link = self.link.as_mut().ok_or_else(|| SiteError::Link("no link".into()))?;
        // upload what the site does not hold, resumable from what it holds
        let all: Vec<BlobRef> = sources.keys().cloned().collect();
        let missing = link.missing(&all)?;
        let (mut sent, mut objects) = (0u64, 0usize);
        for miss in &missing {
            let path = &sources[&miss.blob];
            let mut f =
                std::fs::File::open(path).map_err(|e| SiteError::Refused(format!("{}: {e}", path.display())))?;
            let mut at = miss.from;
            while at < miss.blob.bytes {
                let mut buf = Vec::new();
                f.seek(SeekFrom::Start(at))
                    .and_then(|_| (&mut f).take(CHUNK).read_to_end(&mut buf))
                    .map_err(|e| SiteError::Refused(format!("{}: {e}", path.display())))?;
                if buf.is_empty() {
                    return Err(SiteError::Refused(format!(
                        "{}: shorter than when it was hashed",
                        path.display()
                    )));
                }
                let now = link.put(&miss.blob, at, &buf)?;
                sent += buf.len() as u64;
                at = now;
            }
            objects += 1;
        }
        let total: u64 = all.iter().map(|b| b.bytes).sum();
        site_event(
            &m.dir,
            "uploaded",
            json!({"site": self.site, "objects": objects, "bytes": sent, "already_there_bytes": total.saturating_sub(sent)}),
        );
        let (key, name, manifest) = project_key(self.store.project());
        let mut record = serde_json::to_value(&rec).unwrap_or_default();
        if let Some(o) = record.as_object_mut() {
            o.remove("site");
            o.remove("case_source");
            o.remove("case_dir");
        }
        let run = RemoteRun {
            project: ProjectRef {
                key: key.clone(),
                name,
                manifest,
            },
            id: m.id.clone(),
            record,
            status: std::fs::read_to_string(m.dir.join("status.json")).unwrap_or_default(),
            events: std::fs::read_to_string(m.dir.join("events.jsonl")).unwrap_or_default(),
            snapshot,
            files,
            up,
            parent,
            budgets: entry.budgets(),
        };
        let remote = link.submit(&run)?;
        m.remote.project = key.clone();
        m.submitted = true;
        m.sent_pause = Some(false);
        site_json(
            &m.dir,
            json!({"site": self.site, "project_key": key, "remote": remote, "host": facts.host, "os": facts.os,
                   "arch": facts.arch, "cores": facts.cores, "protocol": facts.protocol, "engine": facts.engine,
                   "local_engine": local, "engine_accepted": accepted, "submitted": true,
                   "submitted_at": records::now_iso(), "clock": {"site": facts.time, "local": records::now_iso()},
                   "last_contact": records::now_iso(), "link": "connected"}),
        );
        site_event(&m.dir, "submitted", json!({"site": self.site, "remote": remote}));
        Ok(())
    }
}

/// `status.json` with exactly the site's bytes, replaced atomically.
fn atomic_status(dir: &Path, bytes: &[u8]) {
    let _ = rcfd_project::atomic::atomic_write_unsynced(&dir.join("status.json"), bytes);
}

/// Copy the new bytes of the append-only files; `true` when anything was copied. A `.jsonl` file is copied to its
/// last complete line.
fn copy_appended(link: &mut AgentLink, m: &mut Mirror) -> Result<bool, SiteError> {
    let mut any = false;
    for (file, lines) in [("events.jsonl", true), ("residuals.jsonl", true), ("log.txt", false)] {
        let path = m.dir.join(file);
        loop {
            let have = std::fs::metadata(&path).map_or(0, |x| x.len());
            let t = link.tail(&m.remote, file, have)?;
            if !t.exists || t.len <= have {
                if t.exists && t.len < have && m.said.insert(format!("diverged {file}")) {
                    site_event(&m.dir, "diverged", json!({"file": file, "local": have, "site": t.len}));
                }
                break;
            }
            let full = t.data.len() as u64 == CHUNK;
            let take = if lines {
                match t.data.iter().rposition(|&b| b == b'\n') {
                    Some(i) => i + 1,
                    None if full => t.data.len(),
                    None => 0,
                }
            } else {
                t.data.len()
            };
            if take == 0 {
                break;
            }
            let mut f = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .map_err(|e| SiteError::Refused(format!("{}: {e}", path.display())))?;
            f.write_all(&t.data[..take])
                .map_err(|e| SiteError::Refused(format!("{}: {e}", path.display())))?;
            any = true;
            if !full {
                break;
            }
        }
    }
    Ok(any)
}

fn is_launch_name(name: &str) -> bool {
    name == crate::store::LAUNCH_FILE
        || name
            .strip_prefix("launch-")
            .and_then(|r| r.strip_suffix(".json"))
            .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

/// Download `output/`, `fields/`, `metrics.json` and the launch records into the local run folder: each file to
/// `<file>.part` from its length, checked by hash and length, then renamed. A file already here with the same hash is
/// not read again. Returns the files and bytes read.
fn download(link: &mut AgentLink, m: &Mirror) -> Result<(usize, u64), SiteError> {
    let list = link.files(&m.remote, &[extra::OUTPUT, extra::FIELDS], true)?;
    let (mut n, mut bytes) = (0usize, 0u64);
    for f in list {
        let Some(parts) = plain_rel(&f.path) else { continue };
        let wanted = match parts.as_slice() {
            [one] => one == "metrics.json" || is_launch_name(one),
            [first, ..] => first == extra::OUTPUT || first == extra::FIELDS,
            [] => false,
        };
        if !wanted {
            continue;
        }
        let local = parts.iter().fold(m.dir.clone(), |p, c| p.join(c));
        let want = BlobRef {
            hash: f.hash.clone(),
            bytes: f.bytes,
        };
        if blob_of_file(&local).is_ok_and(|b| b == want) {
            continue;
        }
        if let Some(d) = local.parent() {
            std::fs::create_dir_all(d).map_err(|e| SiteError::Refused(format!("{}: {e}", d.display())))?;
        }
        let part = PathBuf::from(format!("{}.part", local.display()));
        let mut at = std::fs::metadata(&part).map_or(0, |x| x.len());
        if at > f.bytes {
            let _ = std::fs::remove_file(&part);
            at = 0;
        }
        while at < f.bytes {
            let t = link.read(&m.remote, &f.path, at, CHUNK)?;
            if t.data.is_empty() {
                break;
            }
            let mut w = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&part)
                .map_err(|e| SiteError::Refused(format!("{}: {e}", part.display())))?;
            w.write_all(&t.data)
                .map_err(|e| SiteError::Refused(format!("{}: {e}", part.display())))?;
            at += t.data.len() as u64;
            bytes += t.data.len() as u64;
        }
        if at == 0 && f.bytes == 0 {
            let _ = std::fs::File::create(&part);
        }
        let got = blob_of_file(&part).map_err(|e| SiteError::Refused(format!("{}: {e}", part.display())))?;
        if got != want {
            let _ = std::fs::remove_file(&part);
            // read again on the next poll
            return Err(SiteError::Link(format!(
                "{}: the downloaded bytes hash to {}, the site's file to {}",
                f.path,
                got.name(),
                want.name()
            )));
        }
        records::retry(|| std::fs::rename(&part, &local))
            .map_err(|e| SiteError::Refused(format!("{}: {e}", local.display())))?;
        n += 1;
    }
    Ok((n, bytes))
}

/// `rcfd check <snapshot> --case-dir <dir> --inputs --json [--set ...]` of the local engine.
fn list_inputs(solver: &Path, snapshot: &Path, case_dir: &Path, set: &[String]) -> Result<Value, String> {
    let (mut c, _) = crate::sys::console(solver, 0);
    c.arg("check")
        .arg(snapshot)
        .arg("--case-dir")
        .arg(case_dir)
        .args(["--inputs", "--json"]);
    for s in set {
        c.arg("--set").arg(s);
    }
    c.stdin(std::process::Stdio::null());
    let o = c
        .output()
        .map_err(|e| format!("cannot run {} check --inputs: {e}", solver.display()))?;
    let v: Value = serde_json::from_slice(&o.stdout).map_err(|_| {
        format!(
            "the inputs of the case: {} (rcfd check --inputs, exit {})",
            String::from_utf8_lossy(&o.stderr).trim(),
            o.status.code().unwrap_or(-1)
        )
    })?;
    if v["ok"] != true {
        let errs: Vec<String> = v["inputs"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|i| {
                i["error"]
                    .as_str()
                    .map(|e| format!("{}: {e}", i["key"].as_str().unwrap_or("")))
            })
            .collect();
        return Err(format!("an input of the case cannot be read: {}", errs.join("; ")));
    }
    Ok(v)
}
