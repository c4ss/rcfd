//! The error catalogue (F-830, ADR 0730 P3): every error code and exit code the engine's surfaces can give, each with one
//! line that says what the user can do next.
//!
//! The messages the engine returns do not change. A client shows the message (the problem) followed by the `recovery` of
//! its code from this catalogue, so that every error names the problem and the way out (guardrail G19). The JSON-RPC
//! codes of the service, the project tools (`rcfd-workspace`) and the MCP server share one number space: the project tools
//! and the MCP server reuse the service's codes for the same meaning and add their own (1100, 1200 to 1202). Exit codes
//! are per program.
//!
//! `tests/errors_catalogue_adr0730.rs` reads the sources of every surface and fails when a code is missing here, and
//! checks this catalogue against the tables of `docs/service-protocol.md`, `docs/mcp-tools.md` and `rcfd --help`.

use serde_json::{json, Value};

/// Where a code comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    /// A JSON-RPC error of the solver service (`rcfd serve`, `rcfd call`, the MCP session tools).
    Service,
    /// A JSON-RPC error added by the project tools (`rcfd-mcp --project`, crate `rcfd-workspace`).
    Workspace,
    /// A JSON-RPC error added by the MCP server's policy.
    Mcp,
    /// An exit code of `rcfd` (the command line).
    CliExit,
    /// An exit code of `rcfd-runner`.
    RunnerExit,
    /// An exit code of the `rcfd-mcp` server.
    McpExit,
    /// An exit code of `rcfd-mcp --edit-hook` (the Claude Code edit hook).
    McpHookExit,
}

impl Surface {
    /// The name in `help.errors` (`surface`).
    pub fn name(self) -> &'static str {
        match self {
            Surface::Service => "service",
            Surface::Workspace => "workspace",
            Surface::Mcp => "mcp",
            Surface::CliExit => "cli_exit",
            Surface::RunnerExit => "runner_exit",
            Surface::McpExit => "mcp_exit",
            Surface::McpHookExit => "mcp_hook_exit",
        }
    }

    /// True for the surfaces whose codes are JSON-RPC error codes (one shared number space).
    pub fn is_rpc(self) -> bool {
        matches!(self, Surface::Service | Surface::Workspace | Surface::Mcp)
    }
}

/// One code of the catalogue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ErrorEntry {
    /// Where the code comes from.
    pub surface: Surface,
    /// The code.
    pub code: i64,
    /// The name of the constant in the engine's source, or a short name for an exit code without one.
    pub name: &'static str,
    /// What the code means, as the engine's tables say it.
    pub meaning: &'static str,
    /// One line: what the user can do next.
    pub recovery: &'static str,
}

const fn e(
    surface: Surface,
    code: i64,
    name: &'static str,
    meaning: &'static str,
    recovery: &'static str,
) -> ErrorEntry {
    ErrorEntry {
        surface,
        code,
        name,
        meaning,
        recovery,
    }
}

use Surface::*;

/// Every code, grouped by surface.
pub const CATALOGUE: &[ErrorEntry] = &[
    // ---- the service (crate::service::code, docs/service-protocol.md "Errors")
    e(
        Service,
        -32700,
        "PARSE",
        "invalid JSON",
        "Send one complete JSON object per line; look for a typo or a line that was cut off.",
    ),
    e(
        Service,
        -32600,
        "INVALID_REQUEST",
        "not a JSON-RPC 2.0 request",
        "Send an object with \"jsonrpc\": \"2.0\" and a method name (and an id when you want a reply).",
    ),
    e(
        Service,
        -32601,
        "METHOD_NOT_FOUND",
        "unknown method",
        "Check the name against the methods of service.info; help.* needs rcfd serve --ui, data.* the data channel.",
    ),
    e(
        Service,
        -32602,
        "INVALID_PARAMS",
        "missing or invalid parameters",
        "Fix the parameter the message names; docs/service-protocol.md lists the parameters of each method.",
    ),
    e(
        Service,
        1001,
        "NO_SESSION",
        "unknown session",
        "Make a session with session.create, or pick an existing one from session.list.",
    ),
    e(
        Service,
        1002,
        "BAD_STATE",
        "operation not allowed in the current run state",
        "Pause or stop the run, or wait until it ends, then try again; run.status shows the state.",
    ),
    e(
        Service,
        1003,
        "VALIDATION",
        "settings failed validation; data.diagnostics lists the problems",
        "Fix the settings named in data.diagnostics (each has a path and a message), then try again.",
    ),
    e(
        Service,
        1004,
        "LIMIT",
        "limit exceeded or path outside the sandbox root",
        "Keep file paths inside the service's root folder, or ask for less than the limits service.info reports.",
    ),
    e(
        Service,
        1005,
        "PARTICIPANT",
        "a hosted co-simulation participant returned an error",
        "Read the participant's message, fix the co-simulation case or the participant, then call again.",
    ),
    e(
        Service,
        1006,
        "NO_PARTICIPANT",
        "no hosted co-simulation participant of that name",
        "Use a participant name that cosim.list returns.",
    ),
    e(
        Service,
        -32000,
        "INTERNAL",
        "internal error",
        "Something failed inside the engine. Try again; if it happens again, report it with the message.",
    ),
    // ---- the project tools (crates/rcfd-workspace, docs/mcp-tools.md "Errors added by these tools")
    e(
        Workspace,
        1200,
        "CONFLICT",
        "the file changed since its hash was read",
        "Read the file again (data.current_hash is its hash now), make the change on the new text, then save.",
    ),
    e(
        Workspace,
        1201,
        "LOCK_BUSY",
        "another cooperating writer held the file's edit lock",
        "Another program is saving this file. Wait a moment and try again.",
    ),
    e(
        Workspace,
        1202,
        "NOT_FOUND",
        "no such run, file, field or snapshot",
        "Check the name: list_runs, list_meshes and project_info show what the project holds.",
    ),
    // ---- the MCP server (crates/rcfd-mcp/src/policy.rs, docs/mcp-tools.md)
    e(
        Mcp,
        1100,
        "PERMISSION_DENIED",
        "permission denied by the server's policy",
        "The server is read-only, the tool is not allowed, or the call was not approved; approve it or change the policy.",
    ),
    // ---- rcfd exit codes (rcfd --help "EXIT CODES", docs/run-control.md "Exit codes")
    e(
        CliExit,
        1,
        "FAIL",
        "runtime or I/O failure",
        "Read the error line it printed: a file that could not be read or written, or a failure during the run.",
    ),
    e(
        CliExit,
        2,
        "USAGE_ERR",
        "usage error",
        "Check the command and its options with rcfd --help.",
    ),
    e(
        CliExit,
        3,
        "INVALID",
        "invalid case or settings",
        "Fix the keys the diagnostics name; rcfd validate <case> lists them with their line numbers.",
    ),
    e(
        CliExit,
        4,
        "BAD_MESH",
        "the mesh failed its quality check",
        "Look at the worst cells with rcfd check-mesh <case>, then improve the mesh.",
    ),
    e(
        CliExit,
        5,
        "NOT_CONVERGED",
        "steady run stopped at its iteration cap without meeting the tolerances",
        "Allow more iterations (solver.max_iterations), or look at the residuals for a stall.",
    ),
    e(
        CliExit,
        6,
        "DIVERGED",
        "residuals became non-finite, or a divergence guard stopped the run",
        "Lower the time step or the relaxation factors, check the boundary conditions and the mesh, then run again.",
    ),
    e(
        CliExit,
        7,
        "INTERRUPTED",
        "interrupted through the stop file",
        "The run was asked to stop. Run it again, or continue from its checkpoint with rcfd run --continue-from.",
    ),
    e(
        CliExit,
        8,
        "TIME_LIMIT",
        "--max-elapsed reached",
        "Allow more time (--max-elapsed), or continue from the checkpoint with rcfd run --continue-from.",
    ),
    // ---- rcfd-runner exit codes (rcfd-runner --help)
    e(
        RunnerExit,
        1,
        "FAILED",
        "the command failed, or wait ended with a run that did not finish",
        "Read the rcfd-runner: line; for wait, rcfd-runner status <id> shows why the run did not finish.",
    ),
    e(
        RunnerExit,
        3,
        "EXIT_ALREADY_RUNNING",
        "another runner already serves the project",
        "Nothing to fix: a runner already serves this project. Use it, or stop it with rcfd-runner shutdown first.",
    ),
    // ---- rcfd-mcp exit codes
    e(
        McpExit,
        1,
        "FAILURE",
        "the server's input or output failed",
        "Restart the MCP server; check that the client still holds its standard input and output.",
    ),
    e(
        McpExit,
        2,
        "BAD_PROJECT",
        "--project names no folder, or a folder that is not an rcfd project",
        "Give --project the folder that holds rcfd-project.json.",
    ),
    e(
        McpHookExit,
        1,
        "NOT_READ",
        "the hook input could not be read (Claude Code goes on)",
        "Nothing was blocked. If it repeats, check that the hook is called by Claude Code with its JSON input.",
    ),
    e(
        McpHookExit,
        2,
        "REFUSE",
        "an Edit or Write under cases/, runs/ or designs/ of the project was refused",
        "Change cases through the project tools (edit_case, new_case) instead of editing the files directly.",
    ),
];

/// The entry of a code on a surface.
pub fn lookup(surface: Surface, code: i64) -> Option<&'static ErrorEntry> {
    CATALOGUE.iter().find(|x| x.surface == surface && x.code == code)
}

/// The entry of a JSON-RPC error code, whichever of the service, the project tools or the MCP server gave it.
pub fn lookup_rpc(code: i64) -> Option<&'static ErrorEntry> {
    CATALOGUE.iter().find(|x| x.surface.is_rpc() && x.code == code)
}

/// The reply of `help.errors` and the text of `rcfd help errors`.
pub fn catalogue_json() -> Value {
    json!({
        "errors": CATALOGUE
            .iter()
            .map(|x| json!({
                "surface": x.surface.name(),
                "code": x.code,
                "name": x.name,
                "meaning": x.meaning,
                "recovery": x.recovery,
            }))
            .collect::<Vec<_>>()
    })
}
