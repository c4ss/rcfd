//! The Claude Code allow-list rules (ADR 0126, ADR 0128 finding 2, ADR 0134).
//!
//! * [`allow`]: the typed allow-list rule, the conservative policy every rule is checked against, the per-project
//!   allow-list file ([`allow::ALLOW_FILE`]) and the first list of a project ([`allow::PROJECT_READ_TOOLS`]).
//!
//! This crate was the Claude Code bridge of the egui Studio (`rcfd-studio`). The Electron Studio has its own port of the
//! bridge (`apps/studio-web/src/main/claude/`) and depends on no crate. When `rcfd-studio` was removed (F-449) and the
//! local-model assistant with it (F-838, ADR 0800 §10), the session discovery, transcript and stream readers, the chat
//! driver, the folder-trust store and the argument checks went too: they had no user left. What stays is what the MCP
//! server's tests check its tool names against (`crates/rcfd-mcp/tests/project_w48f.rs` F2 and
//! `crates/rcfd-mcp/tests/tool_list_f275.rs` C5-A).

pub mod allow;

pub use allow::{AllowList, AllowRule};

use std::fmt;
use std::path::PathBuf;

/// Errors of the allow-list. Each names the rule or path it is about.
#[derive(Debug)]
pub enum Error {
    /// An allow-list rule that the conservative policy rejects.
    InvalidRule {
        /// The rule as given.
        rule: String,
        /// Why it is rejected.
        reason: &'static str,
    },
    /// Reading or writing an allow-list file failed.
    Io {
        /// File or folder involved.
        path: PathBuf,
        /// Underlying error.
        source: std::io::Error,
    },
    /// An allow-list file was refused as a whole (ADR 0134 W48-11 (a)); nothing from it is used.
    AllowFile {
        /// The file.
        path: PathBuf,
        /// 1-based line of the refused rule or syntax error (0 when not known).
        line: usize,
        /// 1-based column.
        column: usize,
        /// Why.
        reason: String,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::InvalidRule { rule, reason } => write!(f, "allow-list rule {rule:?} rejected: {reason}"),
            Error::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Error::AllowFile {
                path,
                line,
                column,
                reason,
            } => write!(
                f,
                "{}: line {line}, column {column}: {reason}; the allow-list file is not used",
                path.display()
            ),
        }
    }
}

impl std::error::Error for Error {}
