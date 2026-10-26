//! Append-only JSON Lines (`events.jsonl`, `residuals.jsonl`).
//!
//! A line is committed when its `\n` is on disk; each line is written with one `write_all` of the
//! serialized record plus `\n`. A process killed mid-append can leave a last line without `\n`
//! (a torn line): [`read`] ignores it and reports it, and [`Appender::open`] truncates it before the
//! next append so it never ends up glued to a later line. Limit: one writer per file at a time.

use crate::{io_err, ProjectError, Result};
use serde::{de::DeserializeOwned, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// A torn (uncommitted) last line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TornTail {
    /// Byte offset where the torn line starts.
    pub offset: u64,
    /// Its length in bytes.
    pub len: u64,
}

/// The committed records of a JSONL file and, if present, the torn tail that was ignored.
#[derive(Debug, Clone)]
pub struct Lines<T> {
    /// Records of complete lines, in file order.
    pub items: Vec<T>,
    /// The ignored torn last line.
    pub torn: Option<TornTail>,
}

/// Read every complete line. A missing file reads as empty. A complete line that does not parse is
/// a located error (file, 1-based line, column); blank complete lines are skipped.
pub fn read<T: DeserializeOwned>(path: &Path) -> Result<Lines<T>> {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => {
            return Err(ProjectError::Io {
                op: "read",
                path: path.to_path_buf(),
                source: e,
            })
        }
    };
    parse(path, &bytes)
}

/// [`read`] on bytes already in memory.
pub fn parse<T: DeserializeOwned>(path: &Path, bytes: &[u8]) -> Result<Lines<T>> {
    let committed = bytes.iter().rposition(|&b| b == b'\n').map_or(0, |i| i + 1);
    let torn = (committed < bytes.len()).then(|| TornTail {
        offset: committed as u64,
        len: (bytes.len() - committed) as u64,
    });
    let mut items = Vec::new();
    for (i, line) in bytes[..committed].split(|&b| b == b'\n').enumerate() {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let v = serde_json::from_slice(line).map_err(|source| ProjectError::Json {
            path: path.to_path_buf(),
            line: Some(i + 1),
            source,
        })?;
        items.push(v);
    }
    Ok(Lines { items, torn })
}

/// An open append handle.
#[derive(Debug)]
pub struct Appender {
    file: File,
    path: PathBuf,
}

impl Appender {
    /// Open (creating) for append. Returns the handle and the number of torn-tail bytes removed.
    pub fn open(path: &Path) -> Result<(Self, u64)> {
        let mut file = OpenOptions::new()
            .read(true)
            .append(true)
            .create(true)
            .open(path)
            .map_err(io_err("open for append", path))?;
        let len = file.metadata().map_err(io_err("stat", path))?.len();
        let mut removed = 0;
        if len > 0 {
            let mut last = [0u8];
            file.seek(SeekFrom::End(-1))
                .and_then(|_| file.read_exact(&mut last))
                .map_err(io_err("read tail", path))?;
            if last[0] != b'\n' {
                let bytes = std::fs::read(path).map_err(io_err("read", path))?;
                let keep = bytes.iter().rposition(|&b| b == b'\n').map_or(0, |i| i + 1) as u64;
                // Truncation needs a handle without append mode on some platforms.
                OpenOptions::new()
                    .write(true)
                    .open(path)
                    .and_then(|f| f.set_len(keep))
                    .map_err(io_err("truncate torn tail", path))?;
                removed = len - keep;
            }
        }
        Ok((
            Appender {
                file,
                path: path.to_path_buf(),
            },
            removed,
        ))
    }

    /// Append one record as one line. Does not fsync; call [`Appender::sync`] where durability matters.
    pub fn append<T: Serialize>(&mut self, value: &T) -> Result<()> {
        let mut line = serde_json::to_vec(value).map_err(|source| ProjectError::Json {
            path: self.path.clone(),
            line: None,
            source,
        })?;
        line.push(b'\n');
        self.file.write_all(&line).map_err(io_err("append", &self.path))
    }

    /// fsync the file data.
    pub fn sync(&self) -> Result<()> {
        self.file.sync_data().map_err(io_err("sync", &self.path))
    }
}

/// Open, append one record, fsync, close. Returns the torn-tail bytes removed on open.
pub fn append_one<T: Serialize>(path: &Path, value: &T) -> Result<u64> {
    let (mut a, removed) = Appender::open(path)?;
    a.append(value)?;
    a.sync()?;
    Ok(removed)
}
