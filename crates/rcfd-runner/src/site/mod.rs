//! Run sites (ADR 0739): where the runner launches a run's solver when it is not this machine.
//!
//! A remote run keeps its folder `runs/<id>/` on this machine, created as every run is, with `site` in `run.json`. The
//! local runner hands such a run to [`remote::RemoteRuns`] instead of launching it: the inputs go to the site as
//! content-addressed objects, the site's own `rcfd-runner` runs it over a mirror of the project, and the local run
//! folder is kept up to date from the site by byte offset. Everything the local runner says to a site goes through the
//! [`Site`] trait; the one implementation, [`link::AgentLink`], speaks `rcfd-site/1` (line JSON-RPC) to the agent
//! `rcfd-runner site --stdio` on the site's standard streams, started through `ssh` (kind `ssh`) or directly (kind
//! `exec`, tests only). The local path (`Runner::launch`) has no `Site`: a run without `site` never reaches this module,
//! and no agent is started unless a run names a site (ADR 0739 P9).
//!
//! Hashes are `fnv64` plus the byte length (Q5 (a)): they catch accidents, not forgery; a site is trusted by the user
//! who named it.

pub mod agent;
pub mod cmd;
pub mod link;
pub mod remote;
pub mod sites;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::Read;
use std::path::{Component, Path};

/// The protocol of the agent (`hello` returns it). A site speaking another major is refused (Q10 (a)).
pub const PROTOCOL: &str = "rcfd-site/1";

/// Most bytes of one `put` or one `read`.
pub const CHUNK: u64 = 1 << 20;

/// What a site says of itself (`hello`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SiteFacts {
    /// `rcfd-site/<major>`.
    pub protocol: String,
    /// The site's engine: `{version, git_rev, exe_hash}` of its `rcfd`.
    pub engine: Value,
    /// `std::env::consts::OS` on the site.
    pub os: String,
    /// `std::env::consts::ARCH` on the site.
    pub arch: String,
    /// The site's host name as it reports it.
    pub host: String,
    /// Cores the site's agent sees.
    pub cores: u64,
    /// The site's clock (RFC 3339 UTC).
    pub time: String,
    /// The site's work root.
    pub work: String,
}

impl SiteFacts {
    /// The major of `protocol` (`rcfd-site/<major>`), `None` when it is not of that form.
    pub fn major(&self) -> Option<u64> {
        protocol_major(&self.protocol)
    }

    /// The engine as one comparable text: `<version> (<git_rev>)`.
    pub fn engine_id(&self) -> String {
        engine_id(&self.engine)
    }
}

/// The major of an `rcfd-site/<major>` identifier.
pub fn protocol_major(p: &str) -> Option<u64> {
    p.strip_prefix("rcfd-site/")?.split('.').next()?.parse().ok()
}

/// `<version> (<git_rev>)` of an engine object `{version, git_rev}`; missing parts are `unknown`.
pub fn engine_id(engine: &Value) -> String {
    format!(
        "{} ({})",
        engine["version"].as_str().unwrap_or("unknown"),
        engine["git_rev"].as_str().unwrap_or("unknown")
    )
}

/// A content-addressed object: its `fnv64` (16 hex) and length. Stored on the site as `objects/<hash>-<bytes>`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct BlobRef {
    /// 16 lowercase hex digits.
    pub hash: String,
    /// Length in bytes.
    pub bytes: u64,
}

impl BlobRef {
    /// The object's file name on the site.
    pub fn name(&self) -> String {
        format!("{}-{}", self.hash, self.bytes)
    }

    /// The blob of an `fnv64:<16 hex>` hash (as `rcfd_project::filehash` writes it) and a length.
    pub fn of_filehash(hash: &str, bytes: u64) -> Option<BlobRef> {
        let h = hash.strip_prefix("fnv64:")?;
        valid_hash(h).then(|| BlobRef {
            hash: h.to_string(),
            bytes,
        })
    }

    /// Checked: 16 lowercase hex digits.
    pub fn is_valid(&self) -> bool {
        valid_hash(&self.hash)
    }
}

fn valid_hash(h: &str) -> bool {
    h.len() == 16 && h.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// An object still to send and the length the site already holds of it (a resumed upload starts there).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Missing {
    /// The object.
    #[serde(flatten)]
    pub blob: BlobRef,
    /// Bytes the site holds of it.
    pub from: u64,
}

/// The project a run belongs to, as a site names its mirror.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectRef {
    /// `<name>-<fnv64 of the local absolute path and created>`: the mirror's folder name.
    pub key: String,
    /// The project's name.
    pub name: String,
    /// The text of `rcfd-project.json`, copied once.
    pub manifest: Option<String>,
}

/// One input of a run: where it goes in the run's case folder on the site, and the object that holds it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputFile {
    /// Relative to the case folder, `/` between components; leading `..` at most [`RemoteRun::up`].
    pub path: String,
    /// The object.
    pub blob: BlobRef,
}

/// The parent of a run that continues from another (`continue_from`), sent when the site does not have it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParentRun {
    /// Its run id.
    pub id: String,
    /// Its `run.json`.
    pub record: Value,
    /// Its `status.json` text.
    pub status: String,
    /// Its `output/restart.rcfd`.
    pub restart: BlobRef,
}

/// A run to create on the site (`submit`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteRun {
    /// The project.
    pub project: ProjectRef,
    /// The run id (the local run's).
    pub id: String,
    /// `run.json` for the site: the local record without `site`, `case_source` and `case_dir` (the site sets its own).
    pub record: Value,
    /// The local `status.json` text.
    pub status: String,
    /// The local `events.jsonl` text: the site's file starts with these bytes, so the copies back are exact.
    pub events: String,
    /// `case.snapshot`.
    pub snapshot: BlobRef,
    /// The files the case reads.
    pub files: Vec<InputFile>,
    /// How many folders above the case folder the inputs reach (`..`).
    pub up: usize,
    /// The parent run for `continue_from`, when it must be sent.
    pub parent: Option<ParentRun>,
    /// The site's budgets for its runner (`.rcfd/runner-config.json` of the mirror).
    pub budgets: Value,
}

/// A run on a site: its project key and run id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteId {
    /// The mirror's key.
    pub project: String,
    /// The run id.
    pub id: String,
}

/// Bytes of a file of a remote run folder, and the file's length then.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Tail {
    /// Whether the file exists.
    pub exists: bool,
    /// Its length on the site.
    pub len: u64,
    /// The bytes read.
    pub data: Vec<u8>,
}

/// A request forwarded to a remote run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Request {
    /// Hold.
    Pause,
    /// Release.
    Resume,
    /// Cancel.
    Cancel,
}

/// A run of a site's mirror and its state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteState {
    /// The run id.
    pub id: String,
    /// `status.json` `state`.
    pub state: String,
}

/// A file of a remote run folder (`files`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteFile {
    /// Relative to the run folder, `/` between components.
    pub path: String,
    /// Length.
    pub bytes: u64,
    /// `fnv64` (16 hex) of the bytes.
    pub hash: String,
}

/// Why a call to a site did not answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SiteError {
    /// The link is gone or did not start (the call can be made again on a new link).
    Link(String),
    /// The site answered with an error (the same call fails again).
    Refused(String),
}

impl std::fmt::Display for SiteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SiteError::Link(m) => write!(f, "link: {m}"),
            SiteError::Refused(m) => f.write_str(m),
        }
    }
}

/// What the local runner says to a site (ADR 0739 P2). `Local` is not a site: the local path keeps its own code.
pub trait Site {
    /// The site's facts.
    fn hello(&mut self) -> Result<SiteFacts, SiteError>;
    /// The objects of `blobs` the site does not hold whole.
    fn missing(&mut self, blobs: &[BlobRef]) -> Result<Vec<Missing>, SiteError>;
    /// Send `bytes` of `blob` from byte `from`; returns the length the site holds now. The last piece is checked
    /// (hash and length) before the object counts as present; a mismatch is refused and the piece dropped.
    fn put(&mut self, blob: &BlobRef, from: u64, bytes: &[u8]) -> Result<u64, SiteError>;
    /// Create the run on the site (complete or not at all) and make sure the site's runner runs; returns the remote
    /// run folder. A run that already exists there is not created again.
    fn submit(&mut self, run: &RemoteRun) -> Result<String, SiteError>;
    /// At most `len` bytes (and at most [`CHUNK`]) of `file` of the run from byte `from`.
    fn read(&mut self, run: &RemoteId, file: &str, from: u64, len: u64) -> Result<Tail, SiteError>;
    /// The new bytes of an append-only file from byte `from`.
    fn tail(&mut self, run: &RemoteId, file: &str, from: u64) -> Result<Tail, SiteError> {
        self.read(run, file, from, CHUNK)
    }
    /// Every file below the folders `dirs` of the run folder, and the files at its top when `top`.
    fn files(&mut self, run: &RemoteId, dirs: &[&str], top: bool) -> Result<Vec<RemoteFile>, SiteError>;
    /// Forward a pause, resume or cancel.
    fn request(&mut self, run: &RemoteId, what: Request) -> Result<(), SiteError>;
    /// The runs of the project's mirror.
    fn list(&mut self, project: &str) -> Result<Vec<RemoteState>, SiteError>;
    /// Make sure the site's runner runs for the project's mirror.
    fn ensure(&mut self, project: &str) -> Result<(), SiteError>;
}

/// FNV-1a 64 of a stream (the value of `rcfd_project::records::fnv64` of the whole), as 16 hex digits, and its length.
pub fn fnv64_reader(mut r: impl Read) -> std::io::Result<(String, u64)> {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut buf = vec![0u8; 1 << 16];
    let mut n_all = 0u64;
    loop {
        let n = r.read(&mut buf)?;
        if n == 0 {
            break;
        }
        n_all += n as u64;
        for &b in &buf[..n] {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    Ok((format!("{h:016x}"), n_all))
}

/// The blob of the file at `path`.
pub fn blob_of_file(path: &Path) -> std::io::Result<BlobRef> {
    let (hash, bytes) = fnv64_reader(std::fs::File::open(path)?)?;
    Ok(BlobRef { hash, bytes })
}

/// The blob of `bytes`.
pub fn blob_of(bytes: &[u8]) -> BlobRef {
    let (hash, n) = fnv64_reader(bytes).unwrap_or_default();
    BlobRef { hash, bytes: n }
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard base64 with padding (the bytes of `put` and `read` in the protocol's JSON).
pub fn b64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n =
            (u32::from(c[0]) << 16) | (u32::from(*c.get(1).unwrap_or(&0)) << 8) | u32::from(*c.get(2).unwrap_or(&0));
        let idx = [(n >> 18) & 63, (n >> 12) & 63, (n >> 6) & 63, n & 63];
        for (i, &k) in idx.iter().enumerate() {
            if i <= c.len() {
                out.push(B64[k as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// The bytes of standard base64 text; `None` when it is not.
pub fn b64_decode(text: &str) -> Option<Vec<u8>> {
    let t = text.as_bytes();
    if t.len() % 4 != 0 {
        return None;
    }
    let val = |b: u8| -> Option<u32> {
        Some(match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        } as u32)
    };
    let mut out = Vec::with_capacity(t.len() / 4 * 3);
    for (q, c) in t.chunks(4).enumerate() {
        let last = q + 1 == t.len() / 4;
        let pad = c.iter().rev().take_while(|&&b| b == b'=').count();
        if pad > 2 || (pad > 0 && !last) {
            return None;
        }
        let mut n = 0u32;
        for &b in &c[..4 - pad] {
            n = (n << 6) | val(b)?;
        }
        n <<= 6 * pad as u32;
        let bytes = [(n >> 16) as u8, (n >> 8) as u8, n as u8];
        out.extend_from_slice(&bytes[..3 - pad]);
    }
    Some(out)
}

/// A relative path of normal components only (no `..`, `.`, root or prefix), `/` or `\` between them: what a site may
/// name in a run folder and what the local side writes. Returns the components.
pub fn plain_rel(p: &str) -> Option<Vec<String>> {
    if p.is_empty() {
        return None;
    }
    let mut out = Vec::new();
    for part in p.split(['/', '\\']) {
        let c = Path::new(part).components().collect::<Vec<_>>();
        match c.as_slice() {
            [Component::Normal(n)] if !part.is_empty() && !part.contains(':') => {
                out.push(n.to_string_lossy().into_owned())
            }
            _ => return None,
        }
    }
    Some(out)
}

/// The lexical form of a relative input path: `(leading .. count, the normal components)`, `None` for an absolute path
/// or one that goes above its own start after a normal component (`a/../../b` is `(1, [b])`, which is fine; a path is
/// refused only when it is absolute or empty).
pub fn lexical_rel(p: &str) -> Option<(usize, Vec<String>)> {
    if p.is_empty() || Path::new(p).is_absolute() || p.starts_with('/') || p.starts_with('\\') {
        return None;
    }
    let mut up = 0usize;
    let mut parts: Vec<String> = Vec::new();
    for part in p.split(['/', '\\']) {
        match part {
            "" | "." => {}
            ".." => {
                if parts.pop().is_none() {
                    up += 1;
                }
            }
            n => {
                if Path::new(n).components().count() != 1 || n.contains(':') {
                    return None;
                }
                parts.push(n.to_string());
            }
        }
    }
    (!parts.is_empty()).then_some((up, parts))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv_matches_the_project_hash_and_base64_round_trips() {
        for data in [&b""[..], b"a", b"hello world", &[0u8, 255, 7, 9, 10][..]] {
            let (h, n) = fnv64_reader(data).unwrap();
            assert_eq!(format!("fnv64:{h}"), rcfd_project::records::hash_bytes(data));
            assert_eq!(n, data.len() as u64);
            let e = b64_encode(data);
            assert_eq!(b64_decode(&e).unwrap(), data, "{e}");
        }
        assert_eq!(b64_encode(b"Man"), "TWFu");
        assert_eq!(b64_encode(b"Ma"), "TWE=");
        assert_eq!(b64_encode(b"M"), "TQ==");
        assert_eq!(b64_decode("TQ=="), Some(b"M".to_vec()));
        assert_eq!(b64_decode("T==="), None);
        assert_eq!(b64_decode("TQ==TQ=="), None);
        assert_eq!(b64_decode("T!=="), None);
        let big: Vec<u8> = (0..10_000u32).map(|i| (i * 7 % 251) as u8).collect();
        assert_eq!(b64_decode(&b64_encode(&big)).unwrap(), big);
    }

    #[test]
    fn paths_a_site_may_name_and_input_paths() {
        assert_eq!(
            plain_rel("output/solution.vtu"),
            Some(vec!["output".into(), "solution.vtu".into()])
        );
        for bad in ["", "../x", "a/../b", "/abs", "a//b", "./a", "C:x", "a/"] {
            assert_eq!(plain_rel(bad), None, "{bad}");
        }
        assert_eq!(
            lexical_rel("../meshes/a.msh"),
            Some((1, vec!["meshes".into(), "a.msh".into()]))
        );
        assert_eq!(lexical_rel("t.csv"), Some((0, vec!["t.csv".into()])));
        assert_eq!(lexical_rel("a/../../b"), Some((1, vec!["b".into()])));
        assert_eq!(lexical_rel("./x/./y"), Some((0, vec!["x".into(), "y".into()])));
        for bad in ["", "/abs/a", "..", "a/..", "C:/x"] {
            assert_eq!(lexical_rel(bad), None, "{bad}");
        }
        assert_eq!(protocol_major("rcfd-site/1"), Some(1));
        assert_eq!(protocol_major("rcfd-site/2.3"), Some(2));
        assert_eq!(protocol_major("other/1"), None);
        assert!(BlobRef::of_filehash("fnv64:0123456789abcdef", 3).is_some());
        assert!(BlobRef::of_filehash("fnv64:0123", 3).is_none());
    }
}
