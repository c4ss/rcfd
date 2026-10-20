//! Tutorial extraction and execution (ADR 0089), shared by `rcfd-cli/tests/tutorials.rs` and `rcfd-mcp/tests/tutorials.rs`.
//!
//! Every fenced block of a tutorial carries one tag: `file=<name>`, `run` (with optional `exit=<n>`), `run-slow`, `output` or
//! `not-executed`, after an optional language word. Untagged blocks are errors, so nothing in a tutorial is shown without
//! being either executed or marked as not executed.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

/// What a block is for.
#[derive(Debug, Clone, PartialEq)]
pub enum Tag {
    /// Written to this path in the working directory.
    File(String),
    /// Executed; expected exit code, and whether only the slow (ignored) test runs it.
    Run { exit: i32, slow: bool },
    /// Lines that must occur in the output of the preceding executed block.
    Output,
    /// Shown only.
    NotExecuted,
}

/// One fenced block.
#[derive(Debug, Clone)]
pub struct Block {
    /// 1-based line of the opening fence.
    pub line: usize,
    /// Language word (may be empty).
    pub lang: String,
    pub tag: Tag,
    pub text: String,
}

/// Fenced blocks of a tutorial, or a located error for an untagged or malformed block.
pub fn parse(md: &str) -> Result<Vec<Block>, String> {
    let mut out = Vec::new();
    let mut lines = md.lines().enumerate();
    while let Some((i, l)) = lines.next() {
        let Some(info) = l.trim_start().strip_prefix("```") else {
            continue;
        };
        let (mut lang, mut tag, mut exit) = (String::new(), None, None);
        for w in info.split_whitespace() {
            let t = match w {
                "run" => Some(Tag::Run { exit: 0, slow: false }),
                "run-slow" => Some(Tag::Run { exit: 0, slow: true }),
                "output" => Some(Tag::Output),
                "not-executed" => Some(Tag::NotExecuted),
                _ if w.starts_with("file=") => Some(Tag::File(w[5..].to_string())),
                _ if w.starts_with("exit=") => {
                    exit = Some(
                        w[5..]
                            .parse::<i32>()
                            .map_err(|_| format!("line {}: bad '{w}'", i + 1))?,
                    );
                    None
                }
                _ if lang.is_empty() && tag.is_none() => {
                    lang = w.to_string();
                    None
                }
                _ => return Err(format!("line {}: unknown word '{w}' in the fence info", i + 1)),
            };
            if let Some(t) = t {
                if tag.is_some() {
                    return Err(format!("line {}: more than one tag", i + 1));
                }
                tag = Some(t);
            }
        }
        let mut tag = tag.ok_or_else(|| {
            format!(
                "line {}: untagged code block (use file=, run, run-slow, output or not-executed)",
                i + 1
            )
        })?;
        match (&mut tag, exit) {
            (Tag::Run { exit: e, .. }, Some(x)) => *e = x,
            (_, Some(_)) => return Err(format!("line {}: exit= only applies to run blocks", i + 1)),
            _ => {}
        }
        let mut text = String::new();
        let mut closed = false;
        for (_, l) in lines.by_ref() {
            if l.trim_start().starts_with("```") {
                closed = true;
                break;
            }
            text.push_str(l);
            text.push('\n');
        }
        if !closed {
            return Err(format!("line {}: unclosed code block", i + 1));
        }
        out.push(Block {
            line: i + 1,
            lang,
            tag,
            text,
        });
    }
    Ok(out)
}

/// Split a command line on whitespace; single or double quotes group (no escapes, no shell).
pub fn split_args(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let (mut quote, mut any) = (None, false);
    for c in line.chars() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => cur.push(c),
            None if c == '\'' || c == '"' => {
                quote = Some(c);
                any = true;
            }
            None if c.is_whitespace() => {
                if any || !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
                any = false;
            }
            None => cur.push(c),
        }
    }
    if any || !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Repository root.
pub fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

/// All tutorials (`docs/tutorials/*.md`), sorted.
pub fn tutorials() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(repo().join("docs").join("tutorials"))
        .expect("docs/tutorials exists")
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "md"))
        .collect();
    v.sort();
    v
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let p = e.unwrap().path();
        let t = to.join(p.file_name().unwrap());
        if p.is_dir() {
            copy_dir(&p, &t);
        } else {
            std::fs::copy(&p, &t).unwrap();
        }
    }
}

/// A fresh working directory holding a copy of the repository's `cases/` (commands are written as if run from the repo root).
pub fn workdir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_tutorial_{}_{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    copy_dir(&repo().join("cases"), &d.join("cases"));
    d
}

/// Counts of one tutorial run.
#[derive(Debug, Default)]
pub struct Report {
    pub executed: usize,
    pub skipped_slow: usize,
    pub not_executed: usize,
    pub outputs_checked: usize,
}

/// Run the blocks of `md` in `dir`. `exec` runs one `run`/`run-slow` block and returns its combined output or an error;
/// `slow` also runs the `run-slow` blocks. An `output` block after a skipped block is skipped too.
pub fn run(
    md: &str,
    dir: &Path,
    slow: bool,
    exec: &mut dyn FnMut(&Block, &Path) -> Result<String, String>,
) -> Result<Report, String> {
    let mut r = Report::default();
    let mut last: Option<String> = None;
    for b in parse(md)? {
        match &b.tag {
            Tag::File(name) => {
                let p = dir.join(name);
                std::fs::create_dir_all(p.parent().unwrap()).map_err(|e| e.to_string())?;
                std::fs::write(&p, &b.text).map_err(|e| format!("line {}: {e}", b.line))?;
            }
            Tag::Run { slow: s, .. } if *s && !slow => {
                r.skipped_slow += 1;
                last = None;
            }
            Tag::Run { .. } => {
                last = Some(exec(&b, dir).map_err(|e| format!("line {}: {e}", b.line))?);
                r.executed += 1;
            }
            Tag::Output => {
                let Some(out) = &last else { continue };
                for l in b.text.lines().map(str::trim).filter(|l| !l.is_empty()) {
                    if !out.contains(l) {
                        return Err(format!("line {}: output lacks '{l}'; it was:\n{out}", b.line));
                    }
                }
                r.outputs_checked += 1;
            }
            Tag::NotExecuted => r.not_executed += 1,
        }
    }
    Ok(r)
}

/// Whether a tutorial is an MCP tutorial (its executed blocks are `jsonl` JSON-RPC sessions).
pub fn is_mcp(md: &str) -> bool {
    parse(md)
        .unwrap_or_default()
        .iter()
        .any(|b| matches!(b.tag, Tag::Run { .. }) && b.lang == "jsonl")
}
