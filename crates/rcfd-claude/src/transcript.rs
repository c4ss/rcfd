//! Tolerant tail reader of a session transcript (`~/.claude/projects/<dir>/<session-id>.jsonl`).
//!
//! The documentation declares the line format internal and version-dependent. Only four things are taken from it:
//! role, text, tool names and timestamp, from lines whose top-level `type` is `user` or `assistant` (observed on
//! 2.1.310, ADR 0126). Known bookkeeping lines are skipped; anything else becomes [`TranscriptEntry::Unrecognised`].
//! Nothing here returns an error for content, and nothing panics on it.

use serde_json::Value;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::Path;

/// Who wrote a message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// A person (or the app) typed it.
    User,
    /// The model.
    Assistant,
    /// A tool result fed back to the model (a `user` line whose content is `tool_result` blocks).
    ToolResult,
}

/// One transcript line as the UI shows it.
#[derive(Clone, Debug, PartialEq)]
pub enum TranscriptEntry {
    /// A message.
    Message {
        /// Role.
        role: Role,
        /// Text blocks joined by newlines (thinking blocks are not shown).
        text: String,
        /// Names of the tools called in this message.
        tools: Vec<String>,
        /// The `timestamp` string as written.
        timestamp: Option<String>,
    },
    /// A line of unknown or changed shape; `kind` is its `type` when it has one.
    Unrecognised {
        /// What the line claimed to be.
        kind: Option<String>,
    },
}

/// Bookkeeping line types observed on 2.1.310 that carry nothing for the UI.
const SKIPPED: [&str; 10] = [
    "queue-operation",
    "attachment",
    "atis-latch",
    "api-request-shape",
    "api-request-blob",
    "api-request",
    "last-prompt",
    "cost-state",
    "mode",
    "summary",
];

/// Parses one line (bytes, possibly invalid UTF-8). `None` for blank and bookkeeping lines.
pub fn parse_entry(line: &[u8]) -> Option<TranscriptEntry> {
    let text = String::from_utf8_lossy(line);
    if text.trim().is_empty() {
        return None;
    }
    let unrec = |kind: Option<&str>| {
        Some(TranscriptEntry::Unrecognised {
            kind: kind.map(String::from),
        })
    };
    let Ok(v) = serde_json::from_str::<Value>(&text) else {
        return unrec(None);
    };
    let Some(ty) = v.get("type").and_then(Value::as_str) else {
        return unrec(None);
    };
    let role = match ty {
        "user" => Role::User,
        "assistant" => Role::Assistant,
        t if SKIPPED.contains(&t) => return None,
        t => return unrec(Some(t)),
    };
    let timestamp = v.get("timestamp").and_then(Value::as_str).map(String::from);
    let Some(content) = v.get("message").and_then(|m| m.get("content")) else {
        return unrec(Some(ty));
    };
    let (mut parts, mut tools, mut role) = (Vec::new(), Vec::new(), role);
    match content {
        Value::String(s) => parts.push(s.clone()),
        Value::Array(blocks) => {
            for b in blocks {
                match b.get("type").and_then(Value::as_str) {
                    Some("text") => parts.extend(b.get("text").and_then(Value::as_str).map(String::from)),
                    Some("tool_use") => tools.extend(b.get("name").and_then(Value::as_str).map(String::from)),
                    Some("tool_result") => {
                        role = Role::ToolResult;
                        match b.get("content") {
                            Some(Value::String(s)) => parts.push(s.clone()),
                            Some(Value::Array(inner)) => parts.extend(
                                inner
                                    .iter()
                                    .filter_map(|x| x.get("text").and_then(Value::as_str))
                                    .map(String::from),
                            ),
                            _ => {}
                        }
                    }
                    _ => {} // thinking, images and future block types are not shown
                }
            }
        }
        _ => return unrec(Some(ty)),
    }
    Some(TranscriptEntry::Message {
        role,
        text: parts.join("\n"),
        tools,
        timestamp,
    })
}

/// Complete lines of `bytes`; `skip_first` drops a leading fragment, and an unterminated last line (a writer in
/// the middle of a line) is always dropped.
fn complete_lines(bytes: &[u8], skip_first: bool) -> impl Iterator<Item = &[u8]> {
    let end = bytes.iter().rposition(|&b| b == b'\n').map_or(0, |i| i + 1);
    let mut body = &bytes[..end];
    if skip_first {
        body = body
            .iter()
            .position(|&b| b == b'\n')
            .map_or(&[][..], |i| &body[i + 1..]);
    }
    body.split(|&b| b == b'\n').filter(|l| !l.is_empty())
}

/// The last `max` entries (messages and unrecognised lines) of a transcript. Reads from the end, doubling the window
/// from 256 KiB until enough entries are found or the whole file is read, so a 36 MB transcript is not read whole.
pub fn tail(path: &Path, max: usize) -> io::Result<Vec<TranscriptEntry>> {
    let mut f = File::open(path)?;
    let len = f.metadata()?.len();
    let mut window: u64 = 256 * 1024;
    loop {
        let start = len.saturating_sub(window);
        f.seek(SeekFrom::Start(start))?;
        let mut buf = Vec::new();
        f.by_ref().take(len - start).read_to_end(&mut buf)?;
        let entries: Vec<TranscriptEntry> = complete_lines(&buf, start > 0).filter_map(parse_entry).collect();
        if entries.len() >= max || start == 0 {
            let skip = entries.len().saturating_sub(max);
            return Ok(entries.into_iter().skip(skip).collect());
        }
        window *= 2;
    }
}

/// Entries appended since `offset` (a byte offset returned by the previous call, 0 at first); for read-only mirroring
/// of a session another process is writing. Only complete lines are consumed.
pub fn read_new(path: &Path, offset: &mut u64) -> io::Result<Vec<TranscriptEntry>> {
    let mut f = File::open(path)?;
    if f.metadata()?.len() < *offset {
        *offset = 0; // the file was replaced
    }
    f.seek(SeekFrom::Start(*offset))?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf)?;
    let end = buf.iter().rposition(|&b| b == b'\n').map_or(0, |i| i + 1);
    *offset += end as u64;
    Ok(complete_lines(&buf[..end], false).filter_map(parse_entry).collect())
}

/// The `cwd` written in the first lines of a transcript (to confirm a session belongs to a folder, because the
/// directory naming maps different paths to the same name). Reads at most the first 1 MiB.
pub fn transcript_cwd(path: &Path) -> Option<String> {
    let f = File::open(path).ok()?;
    let mut r = BufReader::new(f.take(1 << 20));
    let mut line = Vec::new();
    while r.read_until(b'\n', &mut line).ok()? > 0 {
        if let Ok(v) = serde_json::from_slice::<Value>(&line) {
            if let Some(c) = v.get("cwd").and_then(Value::as_str) {
                return Some(c.to_string());
            }
        }
        line.clear();
    }
    None
}
