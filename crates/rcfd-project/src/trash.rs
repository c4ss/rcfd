//! The project trash (ADR 0736 P1, F-829 E-829a): `.rcfd/trash/<token>/` per trashed item.
//!
//! A token is `t-<UTC yyyymmddThhmmss>-<4 hex>`, owned by whoever creates its folder (`create_dir`, as run ids,
//! [`crate::runid`]). Inside it, `item.json` ([`Item`]) is written first, then the item is moved whole into `data/` by
//! one rename per entry (same volume: the trash is inside the project). A run moves as `data/<id>/`, a run's results as
//! `data/fields/` and `data/output/`, a case file as `data/<file name>`. [`restore`] moves the entries back and then
//! removes the token's own `item.json` and its empty folders; [`empty`] removes token folders for good.
//!
//! Every move happens under the edit lock of the original path ([`crate::atomic::lock_edit`]) and of the token
//! (`.rcfd/trash/<token>`), so two cooperating writers never move one item at once. A case is moved only if it still has
//! its base hash ([`ProjectError::Conflict`] otherwise). A restore onto an occupied path is [`ProjectError::Exists`] and
//! moves nothing. [`empty`] removes only tokens whose `item.json` reads, under the token's lock, so a token being filled
//! (its folder made, its `item.json` not yet written) is never removed.
//!
//! This module only moves and removes; which runs may be trashed (terminal, no unfinished run continuing from them) is
//! the caller's rule (`rcfd_workspace`). It writes no journal line; the caller does. `.rcfd/` is not read by
//! [`crate::Project::validate`] and not watched, so a project with a trash validates as before.

use crate::atomic::{self, lock_edit, with_retry, DEFAULT_RETRY};
use crate::{io_err, Issue, ProjectError, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::hash_map::RandomState;
use std::fs;
use std::hash::{BuildHasher, Hasher};
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Project-relative folder of the trash.
pub const DIR: &str = ".rcfd/trash";
/// The record of a trashed item, in its token folder.
pub const ITEM_FILE: &str = "item.json";
/// The folder of a token that holds the moved entries.
pub const DATA: &str = "data";

/// `item.json`: what was trashed, from where, when, by whom and why.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Item {
    /// The token (`t-<yyyymmddThhmmss>-<4 hex>`).
    pub token: String,
    /// `run`, `results` or `case`.
    pub kind: String,
    /// Project-relative original path (`runs/<id>` for a run and for its results, the file for a case).
    pub path: String,
    /// The entries of `path` that moved (`fields`, `output`); empty when `path` itself moved.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<String>,
    /// When it was trashed (RFC 3339, UTC).
    pub trashed: String,
    /// The command that trashed it.
    pub who: String,
    /// The caller's reason.
    #[serde(default)]
    pub reason: Option<String>,
    /// Hash of a trashed file (a case); `None` for folders.
    #[serde(default)]
    pub hash: Option<String>,
    /// Bytes of the moved files.
    pub bytes: u64,
    /// The client and process that trashed it, as the journal writes it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor: Option<Value>,
}

/// What to trash.
#[derive(Debug, Clone)]
pub struct Spec<'a> {
    /// `run`, `results` or `case`.
    pub kind: &'a str,
    /// Project-relative path, normalised (`/` separators).
    pub path: &'a str,
    /// Entries of `path` to move instead of `path` itself.
    pub parts: &'a [&'a str],
    /// The hash the file must still have (a case).
    pub base_hash: Option<&'a str>,
    /// The command.
    pub who: &'a str,
    /// The caller's reason.
    pub reason: Option<&'a str>,
    /// The actor, as the journal writes it.
    pub actor: Option<Value>,
}

/// The items of the trash, newest first, and the token folders whose `item.json` could not be read.
#[derive(Debug, Clone, Default)]
pub struct Listing {
    /// The items.
    pub items: Vec<Item>,
    /// Unreadable token folders.
    pub skipped: Vec<Issue>,
}

/// True if `s` has the token form.
pub fn is_token(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 22
        && s.starts_with("t-")
        && b[2..10].iter().all(u8::is_ascii_digit)
        && b[10] == b'T'
        && b[11..17].iter().all(u8::is_ascii_digit)
        && b[17] == b'-'
        && b[18..].iter().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(c))
}

/// The time a token was made (its stamp, to the second), if it has the token form.
pub fn token_time(token: &str) -> Option<SystemTime> {
    if !is_token(token) {
        return None;
    }
    let n = |a: usize, b: usize| token[a..b].parse::<i64>().ok();
    let (y, mo, d) = (n(2, 6)?, n(6, 8)?, n(8, 10)?);
    let (h, mi, s) = (n(11, 13)?, n(13, 15)?, n(15, 17)?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || s > 60 {
        return None;
    }
    let days = days_from_civil(y, mo, d);
    let secs = days * 86_400 + h * 3600 + mi * 60 + s;
    u64::try_from(secs).ok().map(|s| UNIX_EPOCH + Duration::from_secs(s))
}

/// Days since 1970-01-01 of a proleptic Gregorian date (the inverse of [`crate::time::civil_from_days`], same published
/// algorithm).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn random_u64() -> u64 {
    let mut h = RandomState::new().build_hasher();
    h.write_u128(SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos()));
    h.finish()
}

/// Claim a fresh token folder under the trash at time `now`.
fn claim(root: &Path, now: SystemTime) -> Result<(String, PathBuf)> {
    let dir = root.join(DIR);
    atomic::mkdirs(&dir)?;
    let stamp = crate::time::compact(now);
    let start = (random_u64() & 0xffff) as u32;
    for i in 0..0x1_0000u32 {
        let token = format!("t-{stamp}-{:04x}", (start + i) & 0xffff);
        let p = dir.join(&token);
        match fs::create_dir(&p) {
            Ok(()) => return Ok((token, p)),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => {
                return Err(ProjectError::Io {
                    op: "create trash folder",
                    path: p,
                    source: e,
                })
            }
        }
    }
    Err(ProjectError::Invalid {
        path: dir,
        msg: format!("all trash tokens of second {stamp} are taken"),
    })
}

/// The edit-lock key of a token.
fn token_key(token: &str) -> String {
    format!("{DIR}/{token}")
}

/// Bytes of the files under `p` (links not followed).
pub fn tree_bytes(p: &Path) -> u64 {
    let Ok(m) = fs::symlink_metadata(p) else {
        return 0;
    };
    if !m.is_dir() {
        return m.len();
    }
    fs::read_dir(p)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| tree_bytes(&e.path()))
        .sum()
}

fn exists(p: &Path) -> bool {
    fs::symlink_metadata(p).is_ok()
}

fn not_found(op: &'static str, p: &Path) -> ProjectError {
    ProjectError::Io {
        op,
        path: p.to_path_buf(),
        source: io::Error::from(io::ErrorKind::NotFound),
    }
}

/// One rename, retried on a transient error (a reader holding a file open on Windows).
fn rename(from: &Path, to: &Path) -> Result<()> {
    with_retry("move", from, &DEFAULT_RETRY, || fs::rename(from, to))
}

/// Move each `(from, to)` in order; on a failure the ones moved are moved back and the error is returned.
fn move_all(moves: &[(PathBuf, PathBuf)]) -> Result<()> {
    for (i, (from, to)) in moves.iter().enumerate() {
        if let Err(e) = rename(from, to) {
            for (f, t) in moves[..i].iter().rev() {
                let _ = rename(t, f);
            }
            return Err(e);
        }
    }
    Ok(())
}

/// The token folder's own files: `item.json`, then the empty `data/` and the folder itself (never a file it did not
/// write).
fn remove_token_files(dir: &Path) {
    let _ = fs::remove_file(dir.join(ITEM_FILE));
    let _ = fs::remove_dir(dir.join(DATA));
    let _ = fs::remove_dir(dir);
}

/// The entries of an item: `(original, in the trash)`.
fn entries(root: &Path, dir: &Path, path: &str, parts: &[String]) -> Vec<(PathBuf, PathBuf)> {
    let orig = root.join(path);
    let data = dir.join(DATA);
    if parts.is_empty() {
        let name = path.rsplit('/').next().unwrap_or(path);
        vec![(orig, data.join(name))]
    } else {
        parts.iter().map(|p| (orig.join(p), data.join(p))).collect()
    }
}

/// Move an item into the trash (see the module text). The original path must exist (each part, when parts are given);
/// with `base_hash` it must be a file that still has that hash. Returns its [`Item`].
pub fn trash(root: &Path, spec: &Spec<'_>) -> Result<Item> {
    trash_at(root, spec, SystemTime::now())
}

/// [`trash`] at time `now` (the token's stamp and `trashed`).
pub fn trash_at(root: &Path, spec: &Spec<'_>, now: SystemTime) -> Result<Item> {
    let orig = crate::paths::resolve_in(root, spec.path)?;
    let _path_lock = lock_edit(root, spec.path, spec.who)?;
    let parts: Vec<String> = spec.parts.iter().map(|p| p.to_string()).collect();
    for p in &parts {
        if p.is_empty() || p.contains(['/', '\\']) || p == "." || p == ".." {
            return Err(ProjectError::Invalid {
                path: orig.clone(),
                msg: format!("part {p:?} is not one entry name"),
            });
        }
    }
    let mut hash = None;
    if let Some(base) = spec.base_hash {
        let bytes = match atomic::read(&orig) {
            Ok(b) => b,
            Err(ProjectError::Io { source, .. }) if source.kind() == io::ErrorKind::NotFound => {
                return Err(ProjectError::Conflict {
                    path: orig,
                    current: None,
                })
            }
            Err(e) => return Err(e),
        };
        if !crate::records::hash_matches(base, &bytes) {
            return Err(ProjectError::Conflict {
                path: orig,
                current: Some(crate::records::hash_bytes(&bytes)),
            });
        }
        hash = Some(crate::records::hash_bytes(&bytes));
    }
    // the sources are checked before a token is made
    let probe = entries(root, Path::new(""), spec.path, &parts);
    for (from, _) in &probe {
        if !exists(from) {
            return Err(not_found("trash", from));
        }
    }
    let bytes = probe.iter().map(|(f, _)| tree_bytes(f)).sum();
    let (token, dir) = claim(root, now)?;
    let token_lock = lock_edit(root, &token_key(&token), spec.who);
    let token_lock = match token_lock {
        Ok(l) => l,
        Err(e) => {
            remove_token_files(&dir);
            return Err(e);
        }
    };
    let item = Item {
        token: token.clone(),
        kind: spec.kind.to_string(),
        path: spec.path.to_string(),
        parts: parts.clone(),
        trashed: crate::time::rfc3339_ms(now),
        who: spec.who.to_string(),
        reason: spec.reason.map(String::from),
        hash,
        bytes,
        actor: spec.actor.clone(),
    };
    let res = (|| {
        atomic::write_json_new(&dir.join(ITEM_FILE), &item)?;
        let data = dir.join(DATA);
        fs::create_dir(&data).map_err(io_err("create directory", &data))?;
        move_all(&entries(root, &dir, spec.path, &parts))
    })();
    if let Err(e) = res {
        remove_token_files(&dir);
        drop(token_lock);
        return Err(e);
    }
    drop(token_lock);
    Ok(item)
}

/// Read one token's `item.json`.
pub fn read_item(root: &Path, token: &str) -> Result<Item> {
    if !is_token(token) {
        return Err(ProjectError::PathRefused {
            input: token.to_string(),
            component: token.to_string(),
            reason: "not a trash token (t-<yyyymmddThhmmss>-<4 hex>)",
        });
    }
    atomic::read_json(&root.join(DIR).join(token).join(ITEM_FILE))
}

/// The items of the trash, newest first (by `trashed`, then token). No trash folder is an empty trash.
pub fn list(root: &Path) -> Result<Listing> {
    let dir = root.join(DIR);
    let mut out = Listing::default();
    let rd = match fs::read_dir(&dir) {
        Ok(rd) => rd,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(out),
        Err(e) => {
            return Err(ProjectError::Io {
                op: "list",
                path: dir,
                source: e,
            })
        }
    };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if !is_token(&name) || !e.path().is_dir() {
            continue;
        }
        match read_item(root, &name) {
            Ok(i) => out.items.push(i),
            Err(err) => out.skipped.push(Issue {
                path: e.path(),
                message: err.to_string(),
            }),
        }
    }
    out.items
        .sort_by(|a, b| (&b.trashed, &b.token).cmp(&(&a.trashed, &a.token)));
    Ok(out)
}

/// Move a trashed item back to where it was. Refused with [`ProjectError::Exists`] (naming the path) when something
/// occupies an original path now, and with a not-found error when a run's results go back into a run folder that is
/// gone; then nothing moves. The token's own files are removed afterwards. Returns the item.
pub fn restore(root: &Path, token: &str, who: &str) -> Result<Item> {
    let item = read_item(root, token)?;
    let _token_lock = lock_edit(root, &token_key(token), who)?;
    // read again under the lock: another writer may have restored or emptied it
    let item = match read_item(root, token) {
        Ok(i) => i,
        Err(_) => return Err(not_found("restore", &root.join(DIR).join(&item.token))),
    };
    let orig = crate::paths::resolve_in(root, &item.path)?;
    let _path_lock = lock_edit(root, &item.path, who)?;
    let dir = root.join(DIR).join(token);
    let moves: Vec<(PathBuf, PathBuf)> = entries(root, &dir, &item.path, &item.parts)
        .into_iter()
        .map(|(o, t)| (t, o))
        .collect();
    for (from, _) in &moves {
        if !exists(from) {
            return Err(ProjectError::Invalid {
                path: from.clone(),
                msg: format!("trash item {token} is incomplete: this entry is missing"),
            });
        }
    }
    if !item.parts.is_empty() && !orig.is_dir() {
        return Err(not_found("restore into", &orig));
    }
    for (_, to) in &moves {
        if exists(to) {
            return Err(ProjectError::Exists { path: to.clone() });
        }
    }
    if item.parts.is_empty() {
        if let Some(parent) = orig.parent() {
            atomic::mkdirs(parent)?;
        }
    }
    move_all(&moves)?;
    remove_token_files(&dir);
    Ok(item)
}

/// Remove trashed items for good: the tokens given (each must be in the trash, else a not-found error and nothing is
/// removed), or every item; with `older_than`, only items trashed longer ago than that (by the token's stamp). Returns
/// the items removed.
pub fn empty(root: &Path, tokens: Option<&[String]>, older_than: Option<Duration>, who: &str) -> Result<Vec<Item>> {
    empty_at(root, tokens, older_than, who, SystemTime::now())
}

/// [`empty`] at time `now`.
pub fn empty_at(
    root: &Path,
    tokens: Option<&[String]>,
    older_than: Option<Duration>,
    who: &str,
    now: SystemTime,
) -> Result<Vec<Item>> {
    let candidates: Vec<Item> = match tokens {
        Some(ts) => {
            let mut v = Vec::new();
            for t in ts {
                match read_item(root, t) {
                    Ok(i) => v.push(i),
                    Err(ProjectError::PathRefused { .. }) | Err(ProjectError::Io { .. }) => {
                        return Err(not_found("empty trash", &root.join(DIR).join(t)))
                    }
                    Err(e) => return Err(e),
                }
            }
            v
        }
        None => list(root)?.items,
    };
    let mut removed = Vec::new();
    for item in candidates {
        if let Some(age) = older_than {
            let old = token_time(&item.token)
                .and_then(|t| now.duration_since(t).ok())
                .is_some_and(|a| a > age);
            if !old {
                continue;
            }
        }
        let _lock = lock_edit(root, &token_key(&item.token), who)?;
        // under the lock: only a token whose item.json still reads
        if read_item(root, &item.token).is_err() {
            continue;
        }
        let dir = root.join(DIR).join(&item.token);
        with_retry("remove trash item", &dir, &DEFAULT_RETRY, || fs::remove_dir_all(&dir))?;
        removed.push(item);
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_form_and_time() {
        assert!(is_token("t-20261117T101500-0a1f"));
        assert!(!is_token("r-20261117T101500-0a1f"));
        assert!(!is_token("t-20261117T101500-0A1F"));
        let t = token_time("t-20261117T101500-0a1f").unwrap();
        assert_eq!(crate::time::compact(t), "20261117T101500");
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2000, 3, 1), 11_017);
    }
}
