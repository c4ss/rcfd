//! Who started a project's runner: the additive `started_by` of `.rcfd/runner.json` (ADR 0801, "Addendum: Runner
//! closes with the Studio", Q-RL-2 as answered by the maintainer 2026-11-17; G4, G7).
//!
//! The value travels in one environment variable, [`ENV`] (`RCFD_RUNNER_STARTED_BY`), whose value is `studio:<pid>`,
//! `cli` or `mcp`:
//!
//! - A start ([`crate::client::Client::ensure_runner`]) sets it on the `rcfd-runner serve` it spawns. The value is the
//!   starting process's own [`ENV`] when that parses, else the client's surface ([`crate::client::Client::with_started_by`]:
//!   `cli` for the `rcfd-runner` commands, the journal actor's client for the project tools, so `mcp` in `rcfd-mcp` and
//!   `cli` in `rcfd project call`). With neither, the variable is removed from the child's environment.
//! - The Studio sets `studio:<its pid>` on the processes it starts that may start a runner: `rcfd-runner ensure` and
//!   its own `rcfd-mcp`. A runner they start is then the Studio's, whichever of them starts it.
//! - `serve` reads it once at start and writes `"started_by": "studio" | "cli" | "mcp"` to `runner.json`, with
//!   `"studio_pid": <pid>` for `studio`. A missing or malformed value writes neither field, so `rcfd-runner serve` run
//!   by hand writes the record as before.
//!
//! An environment variable and not a `serve` flag, so that a client never makes an older `rcfd-runner serve` fail on an
//! unknown flag: an older runner ignores the variable, and older readers of `runner.json` ignore the fields (G4).

use serde_json::Value;

/// The environment variable that says who starts the runner: `studio:<pid>`, `cli` or `mcp`.
pub const ENV: &str = "RCFD_RUNNER_STARTED_BY";

/// The surface that started a runner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartedBy {
    /// The Studio whose main process has this pid.
    Studio {
        /// The Studio's process id.
        pid: u32,
    },
    /// The command line (`rcfd-runner ensure`, `start`, `rerun`; `rcfd project call`).
    Cli,
    /// An MCP client, through `rcfd-mcp`'s project tools.
    Mcp,
}

impl StartedBy {
    /// The value of [`ENV`]: `studio:<pid>` (pid not 0), `cli` or `mcp`; `None` for anything else.
    pub fn parse(s: &str) -> Option<StartedBy> {
        match s {
            "cli" => Some(StartedBy::Cli),
            "mcp" => Some(StartedBy::Mcp),
            _ => {
                let pid: u32 = s.strip_prefix("studio:")?.parse().ok()?;
                (pid != 0).then_some(StartedBy::Studio { pid })
            }
        }
    }

    /// This process's [`ENV`], when set and valid.
    pub fn from_env() -> Option<StartedBy> {
        std::env::var(ENV).ok().as_deref().and_then(StartedBy::parse)
    }

    /// The surface of a journal actor's client name (`cli`, `mcp`); `None` for other clients.
    pub fn from_client_name(client: &str) -> Option<StartedBy> {
        match client {
            "cli" => Some(StartedBy::Cli),
            "mcp" => Some(StartedBy::Mcp),
            _ => None,
        }
    }

    /// The value of [`ENV`] for this surface.
    pub fn env_value(&self) -> String {
        match self {
            StartedBy::Studio { pid } => format!("studio:{pid}"),
            StartedBy::Cli => "cli".into(),
            StartedBy::Mcp => "mcp".into(),
        }
    }

    /// The `started_by` value of `runner.json`.
    pub fn surface(&self) -> &'static str {
        match self {
            StartedBy::Studio { .. } => "studio",
            StartedBy::Cli => "cli",
            StartedBy::Mcp => "mcp",
        }
    }

    /// Adds `started_by` (and `studio_pid` for the Studio) to the runner record `info`, an object.
    pub fn record(&self, info: &mut Value) {
        if let Some(o) = info.as_object_mut() {
            o.insert("started_by".into(), self.surface().into());
            if let StartedBy::Studio { pid } = self {
                o.insert("studio_pid".into(), (*pid).into());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_values_parse_and_round_trip() {
        for by in [StartedBy::Studio { pid: 4242 }, StartedBy::Cli, StartedBy::Mcp] {
            assert_eq!(StartedBy::parse(&by.env_value()), Some(by));
        }
        for bad in [
            "",
            "studio",
            "studio:",
            "studio:0",
            "studio:-1",
            "studio:x",
            "Studio:1",
            "CLI",
            "mcp ",
            "other",
        ] {
            assert_eq!(StartedBy::parse(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn the_record_gains_only_the_additive_fields() {
        let base = json!({"pid": 7, "started": "t"});
        let mut v = base.clone();
        StartedBy::Studio { pid: 99 }.record(&mut v);
        assert_eq!(
            v,
            json!({"pid": 7, "started": "t", "started_by": "studio", "studio_pid": 99})
        );
        let mut v = base.clone();
        StartedBy::Mcp.record(&mut v);
        assert_eq!(v, json!({"pid": 7, "started": "t", "started_by": "mcp"}));
        let mut v = base;
        StartedBy::Cli.record(&mut v);
        assert_eq!(v, json!({"pid": 7, "started": "t", "started_by": "cli"}));
    }

    #[test]
    fn journal_clients_map_to_surfaces() {
        assert_eq!(StartedBy::from_client_name("cli"), Some(StartedBy::Cli));
        assert_eq!(StartedBy::from_client_name("mcp"), Some(StartedBy::Mcp));
        assert_eq!(StartedBy::from_client_name("studio"), None);
        assert_eq!(StartedBy::from_client_name("other"), None);
    }
}
