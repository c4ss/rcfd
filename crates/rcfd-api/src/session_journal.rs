//! Session journals on disk (F-828 E-828c, ADR 0735).
//!
//! - The create entry of a session made from a file records the file it read: `source.resolved` (the absolute path
//!   after the sandbox resolved it) and `source.hash` (`fnv64:` of its bytes, the spelling of
//!   `rcfd_project::records::hash_bytes`). `journal.get` and `session.list` give `source: {kind, path, hash}` (Q1 (a),
//!   always on; additive members).
//! - With `rcfd serve --journal-dir <dir>` (Q5 (a)) the service keeps one file per session,
//!   `<dir>/<session>-<created yyyymmddThhmmss>.journal.json`, replaced by write-then-rename at `session.create` and
//!   after every accepted `settings.set` and `settings.remove`. Its content is the `journal.get` reply. Without the flag
//!   nothing is written.
//! - `rcfd run --journal <file>` refuses a path source whose bytes no longer have the recorded hash (Q3 (a)),
//!   [`check_source`]; `--journal-ignore-hash` runs it anyway. A document without `source.hash` is not checked.

use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// The service method that writes a session's journal once to a path the client names; only on a service with journal
/// files on (`Service::enable_journal_files`), which then lists it at the end of `service.info` `methods`.
pub const JOURNAL_SAVE: &str = "journal.save";

/// `fnv64:<16 hex>` of the bytes: the same string as `rcfd_project::records::hash_bytes` (this crate does not depend on
/// rcfd-project).
pub fn hash_bytes(bytes: &[u8]) -> String {
    format!("fnv64:{:016x}", rcfd_core::hash::fnv64(bytes))
}

/// An absolute spelling of `p` without touching the file system beyond the working folder (no `\\?\` prefix, which
/// `canonicalize` gives on Windows).
pub fn absolute(p: &Path) -> PathBuf {
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::env::current_dir().map_or_else(|_| p.to_path_buf(), |c| c.join(p))
    }
}

/// `yyyymmddThhmmss` (UTC) of a time.
pub fn stamp(t: SystemTime) -> String {
    let secs = t.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs()) as i64;
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}{m:02}{d:02}T{:02}{:02}{:02}",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// Days since 1970-01-01 to (year, month, day) of the proleptic Gregorian calendar.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

/// The file name of a session's journal: `<session>-<created>.journal.json`.
pub fn file_name(session: &str, created: &str) -> String {
    format!("{session}-{created}.journal.json")
}

/// `{kind, path, hash}` of a create entry's source: `kind` is `path`, `toml`, `settings` or `empty`; `path` and `hash`
/// are null unless the session was made from a file.
pub fn source_summary(create_entry: &Value) -> Value {
    let src = &create_entry["source"];
    let kind = if src["path"].is_string() {
        "path"
    } else if src["toml"].is_string() {
        "toml"
    } else if !src["settings"].is_null() {
        "settings"
    } else {
        "empty"
    };
    let (path, hash) = if kind == "path" {
        (src["path"].clone(), src["hash"].clone())
    } else {
        (Value::Null, Value::Null)
    };
    json!({"kind": kind, "path": path, "hash": hash})
}

/// Write a JSON document to `path` by writing `<path>.tmp` and renaming it over `path`.
pub fn write_atomic(path: &Path, doc: &Value) -> std::io::Result<()> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    let mut text = serde_json::to_string_pretty(doc).map_err(std::io::Error::other)?;
    text.push('\n');
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

/// The case file a journal's `path` source names, read relative to `base_dir` (the journal file's folder), as
/// `rcfd run --journal` always did; when that file is not there, the `source.resolved` path the service read.
pub fn source_file(journal: &Value, base_dir: &Path) -> Option<PathBuf> {
    let first = journal["entries"].as_array()?.first().filter(|e| e["op"] == "create")?;
    let src = &first["source"];
    let p = src["path"].as_str()?;
    let joined = base_dir.join(p);
    if joined.exists() {
        return Some(joined);
    }
    match src["resolved"].as_str() {
        Some(r) if Path::new(r).exists() => Some(PathBuf::from(r)),
        _ => Some(joined),
    }
}

/// Is the file of a journal's `path` source the one the service read? `Ok` when the source is not a file, when the
/// document records no hash (journals written before ADR 0735) or when the hashes agree; otherwise the message names
/// the file and both hashes. A file that cannot be read is left to the case loader, which reports it.
pub fn check_source(journal: &Value, base_dir: &Path) -> Result<(), String> {
    let Some(file) = source_file(journal, base_dir) else {
        return Ok(());
    };
    let Some(recorded) = journal["entries"][0]["source"]["hash"].as_str() else {
        return Ok(());
    };
    let Ok(bytes) = std::fs::read(&file) else {
        return Ok(());
    };
    let now = hash_bytes(&bytes);
    if now == recorded {
        Ok(())
    } else {
        Err(format!(
            "the case file {} changed since the journal was recorded: recorded hash {recorded}, the file now {now} \
             (--journal-ignore-hash runs it anyway)",
            file.display()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn stamp_is_utc_calendar_time() {
        assert_eq!(stamp(UNIX_EPOCH), "19700101T000000");
        // 2026-11-17 08:09:10 UTC
        assert_eq!(
            stamp(UNIX_EPOCH + Duration::from_secs(1_794_902_950)),
            "20261117T080910"
        );
        // a leap day
        assert_eq!(stamp(UNIX_EPOCH + Duration::from_secs(951_782_400)), "20000229T000000");
    }

    #[test]
    fn hash_is_the_project_spelling() {
        assert_eq!(hash_bytes(b""), "fnv64:cbf29ce484222325");
        assert_eq!(hash_bytes(b"foobar"), "fnv64:85944171f73967e8");
    }

    #[test]
    fn source_file_beside_the_journal_first_then_the_resolved_path() {
        let d = std::env::temp_dir().join(format!("rcfd_adr0735_unit_{}", std::process::id()));
        let (beside, read) = (d.join("journals"), d.join("cases"));
        std::fs::create_dir_all(&beside).unwrap();
        std::fs::create_dir_all(&read).unwrap();
        std::fs::write(read.join("case.toml"), "x").unwrap();
        let doc = |resolved: Value| json!({"entries": [{"op": "create", "source": {"path": "case.toml", "resolved": resolved}}]});
        let r = read.join("case.toml").display().to_string();
        // not beside the journal: the file the service read
        assert_eq!(source_file(&doc(json!(r)), &beside), Some(read.join("case.toml")));
        // no resolved path (an old journal): beside the journal, as before
        assert_eq!(source_file(&doc(Value::Null), &beside), Some(beside.join("case.toml")));
        // beside the journal wins
        std::fs::write(beside.join("case.toml"), "y").unwrap();
        assert_eq!(source_file(&doc(json!(r)), &beside), Some(beside.join("case.toml")));
        assert_eq!(
            source_file(
                &json!({"entries": [{"op": "create", "source": {"toml": "x"}}]}),
                &beside
            ),
            None
        );
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn summary_of_each_source_kind() {
        let s = |src: Value| source_summary(&json!({"op": "create", "source": src}));
        assert_eq!(
            s(json!({"path": "a.toml", "hash": "fnv64:1", "toml": null, "settings": null})),
            json!({"kind": "path", "path": "a.toml", "hash": "fnv64:1"})
        );
        assert_eq!(s(json!({"toml": "x", "path": null}))["kind"], "toml");
        assert_eq!(s(json!({"settings": {}}))["kind"], "settings");
        assert_eq!(
            s(json!({"toml": null, "settings": null, "path": null})),
            json!({"kind": "empty", "path": null, "hash": null})
        );
    }
}
