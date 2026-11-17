//! The agent `rcfd-runner site --stdio --work <work>` (ADR 0739 P3 A1): one process per link, alive as long as its
//! standard input (the SSH session). It speaks `rcfd-site/1` (JSON-RPC 2.0, one object per line) and works only below
//! `<work>`:
//!
//! ```text
//! <work>/objects/<16 hex fnv64>-<bytes>        content-addressed inputs (0600), tmp-<name> while they arrive
//! <work>/received.jsonl                        one line per piece received: the agent's byte counter
//! <work>/projects/<key>/                       a project mirror: rcfd-project.json, .rcfd/runner-config.json, runs/
//! <work>/projects/<key>/inputs/<id>/           the inputs of run <id> at the paths its case names
//! ```
//!
//! The runs of a mirror are run by the site's ordinary `rcfd-runner` over that folder; the agent starts it
//! (`Client::ensure_runner`) and reads and writes only the run folders' files. A name that comes from the link is
//! checked before it becomes a path: a project key, a run id, a relative path of plain components.

use super::{
    b64_decode, b64_encode, fnv64_reader, plain_rel, BlobRef, InputFile, Missing, RemoteFile, RemoteRun, CHUNK,
};
use crate::client::Client;
use crate::store::{FsStore, RunStore};
use serde_json::{json, Value};
use std::io::{BufRead, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// Test answers of `hello` (only with `RCFD_TEST_EXEC_SITES=1`): another protocol, another `git_rev`.
pub const ENV_TEST_PROTOCOL: &str = "RCFD_SITE_TEST_PROTOCOL";
/// See [`ENV_TEST_PROTOCOL`].
pub const ENV_TEST_GIT_REV: &str = "RCFD_SITE_TEST_GIT_REV";

struct Agent {
    work: PathBuf,
}

type R = Result<Value, String>;

/// Create `dir` (and its parents) with mode 0700 on Unix.
fn private_dir(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

/// Mode 0600 on Unix.
fn private_file(f: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(f, std::fs::Permissions::from_mode(0o600))?;
    }
    let _ = f;
    Ok(())
}

/// Serve the link on standard input and output until the input ends. Returns the exit code.
pub fn serve_stdio(work: &Path) -> Result<i32, String> {
    let work = if work.is_absolute() {
        work.to_path_buf()
    } else {
        std::env::current_dir().map_err(|e| e.to_string())?.join(work)
    };
    for d in [work.clone(), work.join("objects"), work.join("projects")] {
        private_dir(&d).map_err(|e| format!("{}: {e}", d.display()))?;
    }
    let agent = Agent { work };
    let stdin = std::io::stdin();
    let mut out = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let req: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                let r = json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": e.to_string()}});
                let _ = writeln!(out, "{r}");
                continue;
            }
        };
        let id = req["id"].clone();
        let reply = match agent.dispatch(req["method"].as_str().unwrap_or(""), &req["params"]) {
            Ok(v) => json!({"jsonrpc": "2.0", "id": id, "result": v}),
            Err(m) => json!({"jsonrpc": "2.0", "id": id, "error": {"code": 1, "message": m}}),
        };
        if writeln!(out, "{reply}").and_then(|()| out.flush()).is_err() {
            break;
        }
    }
    Ok(0)
}

fn s<'a>(p: &'a Value, k: &str) -> Result<&'a str, String> {
    p[k].as_str().ok_or_else(|| format!("{k}: a string is needed"))
}

fn blob_arg(p: &Value) -> Result<BlobRef, String> {
    let b: BlobRef = serde_json::from_value(p.clone()).map_err(|e| format!("blob: {e}"))?;
    if !b.is_valid() {
        return Err(format!("'{}' is not an fnv64 hash", b.hash));
    }
    Ok(b)
}

/// A project key: letters, digits, `.`, `_` and `-`, not starting with `.`.
fn valid_key(k: &str) -> bool {
    !k.is_empty()
        && k.len() <= 200
        && !k.starts_with('.')
        && k.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'-')
}

/// This machine's name, as well as it can be told.
fn host_name() -> String {
    for k in ["COMPUTERNAME", "HOSTNAME"] {
        if let Ok(v) = std::env::var(k) {
            if !v.trim().is_empty() {
                return v.trim().to_string();
            }
        }
    }
    std::fs::read_to_string("/etc/hostname")
        .map(|t| t.trim().to_string())
        .ok()
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| "unknown".into())
}

impl Agent {
    fn dispatch(&self, method: &str, p: &Value) -> R {
        match method {
            "hello" => Ok(self.hello()),
            "missing" => self.missing(p),
            "put" => self.put(p),
            "submit" => self.submit(p),
            "read" => self.read(p),
            "files" => self.files(p),
            "request" => self.request(p),
            "list" => self.list(p),
            "ensure" => self.ensure(s(p, "project")?).map(|()| json!({})),
            other => Err(format!("unknown method '{other}'")),
        }
    }

    fn hello(&self) -> Value {
        let solver = crate::runner::Config::default().solver();
        let d = crate::build::describe(&solver);
        let mut engine = json!({"version": d["version"], "git_rev": d["build"]["git_rev"],
                                "exe_hash": d["build"]["exe_hash"]});
        let mut protocol = super::PROTOCOL.to_string();
        if super::sites::exec_allowed() {
            if let Ok(p) = std::env::var(ENV_TEST_PROTOCOL) {
                protocol = p;
            }
            if let Ok(g) = std::env::var(ENV_TEST_GIT_REV) {
                engine["git_rev"] = json!(g);
            }
        }
        json!({
            "protocol": protocol,
            "engine": engine,
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "host": host_name(),
            "cores": std::thread::available_parallelism().map_or(1, |n| n.get()),
            "time": crate::records::now_iso(),
            "work": self.work.display().to_string(),
        })
    }

    fn object(&self, b: &BlobRef) -> PathBuf {
        self.work.join("objects").join(b.name())
    }

    fn tmp(&self, b: &BlobRef) -> PathBuf {
        self.work.join("objects").join(format!("tmp-{}", b.name()))
    }

    fn has(&self, b: &BlobRef) -> bool {
        std::fs::metadata(self.object(b)).is_ok_and(|m| m.len() == b.bytes)
    }

    fn missing(&self, p: &Value) -> R {
        let mut out = Vec::new();
        for b in p["blobs"].as_array().into_iter().flatten() {
            let b = blob_arg(b)?;
            if self.has(&b) {
                continue;
            }
            let from = std::fs::metadata(self.tmp(&b)).map_or(0, |m| m.len());
            out.push(Missing {
                from: if from <= b.bytes { from } else { 0 },
                blob: b,
            });
        }
        Ok(json!({"missing": out}))
    }

    fn put(&self, p: &Value) -> R {
        let b = blob_arg(p)?;
        if self.has(&b) {
            return Ok(json!({"len": b.bytes}));
        }
        let from = p["from"].as_u64().ok_or("from: a number is needed")?;
        let data = b64_decode(s(p, "data")?).ok_or("data: not base64")?;
        let tmp = self.tmp(&b);
        let have = std::fs::metadata(&tmp).map_or(0, |m| m.len());
        if have > b.bytes {
            let _ = std::fs::remove_file(&tmp);
            return Ok(json!({"len": 0}));
        }
        if from != have {
            // not where the object stands: the sender goes on from the length it is told
            return Ok(json!({"len": have}));
        }
        if from + data.len() as u64 > b.bytes {
            return Err(format!("object {}: more bytes than its length", b.name()));
        }
        {
            let mut f = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&tmp)
                .map_err(|e| format!("{}: {e}", tmp.display()))?;
            f.write_all(&data).map_err(|e| format!("{}: {e}", tmp.display()))?;
        }
        let _ = private_file(&tmp);
        let _ = crate::records::append_line(
            &self.work.join("received.jsonl"),
            &json!({"t": crate::records::now_iso(), "object": b.name(), "bytes": data.len()}).to_string(),
        );
        let len = have + data.len() as u64;
        if len == b.bytes {
            let got = std::fs::File::open(&tmp)
                .and_then(fnv64_reader)
                .map_err(|e| format!("{}: {e}", tmp.display()))?;
            if got != (b.hash.clone(), b.bytes) {
                let _ = std::fs::remove_file(&tmp);
                return Err(format!(
                    "object {}: the bytes that arrived hash to {}-{}; the object was dropped",
                    b.name(),
                    got.0,
                    got.1
                ));
            }
            crate::records::retry(|| std::fs::rename(&tmp, self.object(&b)))
                .map_err(|e| format!("{}: {e}", self.object(&b).display()))?;
        }
        Ok(json!({"len": len}))
    }

    fn mirror(&self, key: &str) -> Result<PathBuf, String> {
        if !valid_key(key) {
            return Err(format!("'{key}' is not a project key"));
        }
        Ok(self.work.join("projects").join(key))
    }

    fn run_dir(&self, p: &Value) -> Result<PathBuf, String> {
        let m = self.mirror(s(p, "project")?)?;
        let id = s(p, "run")?;
        if !rcfd_project::runid::is_run_id(id) {
            return Err(format!("'{id}' is not a run id"));
        }
        Ok(m.join("runs").join(id))
    }

    /// Put object `b` at `to`: a hard link where the file system allows, else a copy.
    fn place(&self, b: &BlobRef, to: &Path) -> Result<(), String> {
        if !self.has(b) {
            return Err(format!("object {} is not on the site; nothing was submitted", b.name()));
        }
        if let Some(d) = to.parent() {
            std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
        }
        let _ = std::fs::remove_file(to);
        if std::fs::hard_link(self.object(b), to).is_err() {
            std::fs::copy(self.object(b), to).map_err(|e| format!("{}: {e}", to.display()))?;
        }
        Ok(())
    }

    fn submit(&self, p: &Value) -> R {
        let run: RemoteRun = serde_json::from_value(p.clone()).map_err(|e| format!("submit: {e}"))?;
        let mirror = self.mirror(&run.project.key)?;
        if !rcfd_project::runid::is_run_id(&run.id) {
            return Err(format!("'{}' is not a run id", run.id));
        }
        let runs = mirror.join("runs");
        let dir = runs.join(&run.id);
        if dir.join("run.json").is_file() {
            self.ensure(&run.project.key)?;
            return Ok(json!({"remote": dir.display().to_string(), "created": false}));
        }
        // every object first: a run is created only with all its inputs
        let mut all: Vec<&BlobRef> = vec![&run.snapshot];
        all.extend(run.files.iter().map(|f| &f.blob));
        all.extend(run.parent.iter().map(|p| &p.restart));
        for b in &all {
            if !b.is_valid() || !self.has(b) {
                return Err(format!("object {} is not on the site; nothing was submitted", b.name()));
            }
        }
        private_dir(&mirror.join(".rcfd")).map_err(|e| format!("{}: {e}", mirror.display()))?;
        std::fs::create_dir_all(&runs).map_err(|e| format!("{}: {e}", runs.display()))?;
        if let Some(m) = &run.project.manifest {
            let f = mirror.join("rcfd-project.json");
            if !f.exists() {
                crate::records::atomic_write(&f, m.as_bytes()).map_err(|e| format!("{}: {e}", f.display()))?;
            }
        }
        if run.budgets.as_object().is_some_and(|o| !o.is_empty()) {
            let f = crate::runner::config_path(&mirror);
            let text = serde_json::to_string_pretty(&run.budgets).unwrap_or_default();
            crate::records::atomic_write(&f, text.as_bytes()).map_err(|e| format!("{}: {e}", f.display()))?;
        }
        // the inputs at the paths the case names, below a case folder `up` levels deep
        let inputs = mirror.join("inputs").join(&run.id);
        let mut case_dir = inputs.clone();
        for _ in 0..run.up {
            case_dir.push("_");
        }
        std::fs::create_dir_all(&case_dir).map_err(|e| format!("{}: {e}", case_dir.display()))?;
        for InputFile { path, blob } in &run.files {
            let (up, parts) = super::lexical_rel(path).ok_or_else(|| format!("input path '{path}' is refused"))?;
            if up > run.up {
                return Err(format!("input path '{path}' reaches above its case folder"));
            }
            let mut to = case_dir.clone();
            for _ in 0..up {
                to.pop();
            }
            for c in &parts {
                to.push(c);
            }
            self.place(blob, &to)?;
        }
        // the parent of continue_from, when this mirror does not have it
        if let Some(par) = &run.parent {
            if !rcfd_project::runid::is_run_id(&par.id) {
                return Err(format!("'{}' is not a run id", par.id));
            }
            let pdir = runs.join(&par.id);
            if !pdir.join("run.json").is_file() {
                let tmp = runs.join(format!(".tmp-{}", par.id));
                let _ = std::fs::remove_dir_all(&tmp);
                std::fs::create_dir_all(tmp.join("output")).map_err(|e| format!("{}: {e}", tmp.display()))?;
                self.place(&par.restart, &tmp.join("output").join("restart.rcfd"))?;
                write_new(
                    &tmp.join("run.json"),
                    serde_json::to_string_pretty(&par.record).unwrap_or_default().as_bytes(),
                )?;
                write_new(&tmp.join("status.json"), par.status.as_bytes())?;
                crate::records::retry(|| std::fs::rename(&tmp, &pdir))
                    .map_err(|e| format!("{}: {e}", pdir.display()))?;
            }
        }
        // the run folder, complete or not at all
        let mut record = run.record.clone();
        record["case_dir"] = json!(case_dir.display().to_string());
        if let Some(o) = record.as_object_mut() {
            o.remove("site");
            o.remove("case_source");
        }
        let tmp = runs.join(format!(".tmp-{}", run.id));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).map_err(|e| format!("{}: {e}", tmp.display()))?;
        self.place(&run.snapshot, &tmp.join("case.snapshot"))?;
        let mut rec_text = serde_json::to_vec_pretty(&record).map_err(|e| e.to_string())?;
        rec_text.push(b'\n');
        write_new(&tmp.join("run.json"), &rec_text)?;
        write_new(&tmp.join("status.json"), run.status.as_bytes())?;
        write_new(&tmp.join("events.jsonl"), run.events.as_bytes())?;
        crate::records::retry(|| std::fs::rename(&tmp, &dir)).map_err(|e| format!("{}: {e}", dir.display()))?;
        self.ensure(&run.project.key)?;
        Ok(json!({"remote": dir.display().to_string(), "created": true}))
    }

    /// Start the site's runner over the mirror unless it runs.
    fn ensure(&self, key: &str) -> Result<(), String> {
        let mirror = self.mirror(key)?;
        if !mirror.join("runs").is_dir() {
            return Ok(());
        }
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        Client::new(&mirror, exe)
            .ensure_runner()
            .map_err(|e| format!("the site's runner for {}: {e}", mirror.display()))
    }

    fn read(&self, p: &Value) -> R {
        let dir = self.run_dir(p)?;
        let file = s(p, "file")?;
        let parts = plain_rel(file).ok_or_else(|| format!("'{file}' is not a plain relative path"))?;
        let mut path = dir;
        for c in parts {
            path.push(c);
        }
        let from = p["from"].as_u64().unwrap_or(0);
        let len = p["len"].as_u64().unwrap_or(CHUNK).min(CHUNK);
        let mut f = match std::fs::File::open(&path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(json!({"exists": false, "len": 0, "data": ""}))
            }
            Err(e) => return Err(format!("{}: {e}", path.display())),
        };
        let total = f.metadata().map_err(|e| e.to_string())?.len();
        let mut buf = Vec::new();
        if from < total {
            f.seek(SeekFrom::Start(from)).map_err(|e| e.to_string())?;
            f.take(len).read_to_end(&mut buf).map_err(|e| e.to_string())?;
        }
        Ok(json!({"exists": true, "len": total, "data": b64_encode(&buf)}))
    }

    fn files(&self, p: &Value) -> R {
        let dir = self.run_dir(p)?;
        let mut out = Vec::new();
        let mut walk: Vec<(PathBuf, String)> = Vec::new();
        for d in p["dirs"].as_array().into_iter().flatten().filter_map(Value::as_str) {
            if plain_rel(d).is_some_and(|c| c.len() == 1) {
                walk.push((dir.join(d), d.to_string()));
            }
        }
        let mut entry = |path: &Path, rel: String| -> Result<(), String> {
            let (hash, bytes) = std::fs::File::open(path)
                .and_then(fnv64_reader)
                .map_err(|e| format!("{}: {e}", path.display()))?;
            out.push(RemoteFile { path: rel, bytes, hash });
            Ok(())
        };
        if p["top"].as_bool().unwrap_or(false) {
            for e in std::fs::read_dir(&dir)
                .map_err(|e| format!("{}: {e}", dir.display()))?
                .flatten()
            {
                if e.file_type().is_ok_and(|t| t.is_file()) {
                    entry(&e.path(), e.file_name().to_string_lossy().into_owned())?;
                }
            }
        }
        while let Some((d, rel)) = walk.pop() {
            let Ok(rd) = std::fs::read_dir(&d) else { continue };
            for e in rd.flatten() {
                let r = format!("{rel}/{}", e.file_name().to_string_lossy());
                match e.file_type() {
                    Ok(t) if t.is_dir() => walk.push((e.path(), r)),
                    Ok(t) if t.is_file() => entry(&e.path(), r)?,
                    _ => {}
                }
            }
        }
        out.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(json!({"files": out}))
    }

    fn request(&self, p: &Value) -> R {
        let dir = self.run_dir(p)?;
        if !dir.join("run.json").is_file() {
            return Err(format!("no run {} on the site", dir.display()));
        }
        let (name, on) = match s(p, "what")? {
            "pause" => (crate::store::extra::PAUSE, true),
            "resume" => (crate::store::extra::PAUSE, false),
            "cancel" => (crate::store::extra::CANCEL, true),
            other => return Err(format!("'{other}' is not pause, resume or cancel")),
        };
        let f = dir.join(name);
        if on {
            std::fs::write(&f, b"").map_err(|e| format!("{}: {e}", f.display()))?;
        } else if let Err(e) = std::fs::remove_file(&f) {
            if e.kind() != std::io::ErrorKind::NotFound {
                return Err(format!("{}: {e}", f.display()));
            }
        }
        Ok(json!({}))
    }

    fn list(&self, p: &Value) -> R {
        let m = self.mirror(s(p, "project")?)?;
        let store = FsStore::new(&m);
        let mut runs = Vec::new();
        for id in store.list_runs().map_err(|e| e.to_string())? {
            if let Ok(st) = store.read_status(&id) {
                runs.push(json!({"id": id, "state": crate::client::state_name(st.state)}));
            }
        }
        Ok(json!({"runs": runs}))
    }
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::write(path, bytes).map_err(|e| format!("{}: {e}", path.display()))
}
