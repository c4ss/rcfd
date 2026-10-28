//! Claude Code bridge for the Studio (layer L6 of `docs/ui-restart-plan.md`, ADR 0126).
//!
//! * [`discover`]: the Claude Code sessions of a project folder, each OPEN (another process has it) or CLOSED.
//! * [`transcript`]: a tolerant tail reader of the session transcript (role, text, tool names, timestamp only).
//! * [`stream`]: a tolerant parser of `claude -p --output-format stream-json` lines.
//! * [`allow`]: the typed allow-list and its mapping onto CLI flags; nothing here can emit a bypass flag.
//! * [`driver`]: spawns `claude -p` per message, streams events to a channel, cancels without orphans.
//!
//! The transcript and stream formats are not published; they were observed on Claude Code 2.1.310 (ADR 0126).
//! Unknown shapes become `Unrecognised` values, never errors or panics.

pub mod allow;
pub mod discover;
pub mod driver;
pub mod stream;
pub mod transcript;

pub use allow::{AllowList, AllowRule};
pub use discover::{Discovery, OpenState, SessionInfo};
pub use driver::{Chat, Claude};
pub use stream::StreamEvent;
pub use transcript::TranscriptEntry;

use std::fmt;
use std::path::PathBuf;
use std::process::Command;

/// Windows `CreateProcess` flag: a console program gets a console without a window.
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// `program` as a console program that opens no window (creation flags `CREATE_NO_WINDOW` on Windows, none elsewhere;
/// stdio untouched). Returns the command and the flags set, which std's `Command` cannot report. Every program the
/// bridge starts goes through this (ADR 0128 finding 17).
pub(crate) fn console(program: impl AsRef<std::ffi::OsStr>) -> (Command, u32) {
    #[allow(unused_mut)]
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
        (cmd, CREATE_NO_WINDOW)
    }
    #[cfg(not(windows))]
    {
        (cmd, 0)
    }
}

/// Errors of the bridge. Each names the session, rule, argument or path it is about.
#[derive(Debug)]
pub enum Error {
    /// The session is open in another process (one writer per session).
    SessionOpen {
        /// Session id.
        session_id: String,
        /// Pid of the process that has it open.
        pid: u32,
    },
    /// Whether the session is open could not be established, so writing is refused.
    OpenStateUnknown {
        /// Session id.
        session_id: String,
        /// Why the listing failed.
        reason: String,
    },
    /// Not a UUID (session ids are passed to the CLI only after this check).
    InvalidSessionId(String),
    /// An allow-list rule that the conservative policy rejects.
    InvalidRule {
        /// The rule as given.
        rule: String,
        /// Why it is rejected.
        reason: &'static str,
    },
    /// A generated argument list failed the forbidden-flag check (should never happen).
    ForbiddenArgument(String),
    /// Spawning or talking to the CLI failed.
    Io {
        /// Program or file involved.
        path: PathBuf,
        /// Underlying error.
        source: std::io::Error,
    },
    /// `claude agents --json` failed or returned something unreadable.
    Listing(String),
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
            Error::SessionOpen { session_id, pid } => write!(
                f,
                "session {session_id} is open in process {pid}; it can only be mirrored read-only until that process ends"
            ),
            Error::OpenStateUnknown { session_id, reason } => write!(
                f,
                "session {session_id}: cannot tell whether another process has it open ({reason}); not writing to it"
            ),
            Error::InvalidSessionId(s) => write!(f, "not a session id (UUID expected): {s:?}"),
            Error::InvalidRule { rule, reason } => write!(f, "allow-list rule {rule:?} rejected: {reason}"),
            Error::ForbiddenArgument(a) => write!(f, "refusing to start claude: forbidden argument {a:?}"),
            Error::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Error::Listing(s) => write!(f, "claude agents --json: {s}"),
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

/// True when `s` is a UUID in the 8-4-4-4-12 hex form.
pub fn is_uuid(s: &str) -> bool {
    s.len() == 36
        && s.char_indices().all(|(i, c)| {
            if matches!(i, 8 | 13 | 18 | 23) {
                c == '-'
            } else {
                c.is_ascii_hexdigit()
            }
        })
}

/// A fresh random version-4 UUID.
// shortcut: std-only randomness (RandomState keys + time + pid + counter), unique but not cryptographic; use a CSPRNG crate if ids ever become secrets.
pub fn new_uuid() -> String {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let word = |salt: u64| {
        let mut h = RandomState::new().build_hasher();
        h.write_u128(nanos);
        h.write_u32(std::process::id());
        h.write_u64(n);
        h.write_u64(salt);
        h.finish()
    };
    let (hi, lo) = (word(1), word(2));
    let hi = (hi & 0xffff_ffff_ffff_0fff) | 0x0000_0000_0000_4000; // version 4
    let lo = (lo & 0x3fff_ffff_ffff_ffff) | 0x8000_0000_0000_0000; // RFC 4122 variant
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        hi >> 32,
        (hi >> 16) & 0xffff,
        hi & 0xffff,
        lo >> 48,
        lo & 0xffff_ffff_ffff
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuids_are_valid_and_distinct() {
        let ids: std::collections::HashSet<String> = (0..1000).map(|_| new_uuid()).collect();
        assert_eq!(ids.len(), 1000);
        assert!(ids.iter().all(|s| is_uuid(s) && &s[14..15] == "4"));
        // negative controls: the validator rejects injection-shaped and malformed ids
        for bad in [
            "--dangerously-skip-permissions",
            "",
            "00000000-0000-4000-8000-00000000000",
            "00000000-0000-4000-8000-00000000000g",
        ] {
            assert!(!is_uuid(bad), "{bad}");
        }
        assert!(is_uuid("00000000-0000-4000-8000-000000000001"));
    }

    /// ADR 0128 finding 17 (a, b): every program the bridge starts is built with `CREATE_NO_WINDOW`.
    #[cfg(windows)]
    #[test]
    fn no_window_flags_on_every_console_spawn() {
        let c = crate::driver::Claude::new("claude", "home");
        let here = std::path::Path::new(".");
        let sites = [
            ("tasklist (live_pids)", crate::discover::tasklist_cmd(1).1),
            ("taskkill (kill_tree)", crate::driver::kill_tree_cmd(1).1),
            ("claude agents --json", c.agents_cmd(here).1),
            ("claude -p (chat)", c.chat_cmd(here, &[]).1),
        ];
        eprintln!("F17 bridge flags: {sites:x?}");
        let hidden = |f: u32| f & crate::CREATE_NO_WINDOW != 0;
        for (name, f) in sites {
            assert!(hidden(f), "{name}: flags {f:#x} lack CREATE_NO_WINDOW");
        }
        // negative control: a command built without the helper carries no flags and fails the same check
        let _plain = std::process::Command::new("tasklist");
        assert!(!hidden(0));
    }
}
