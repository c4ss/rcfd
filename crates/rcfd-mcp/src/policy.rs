//! Permission model for MCP tools (F-091).
//!
//! Every tool has an [`Access`] class. `Read` tools never change state;
//! the others are mutating and are flagged (`readOnlyHint: false`,
//! `_meta."rcfd/needsApproval": true`) in `tools/list` so a client can ask the
//! user. The server enforces its own [`Approval`] mode on top: read-only mode
//! refuses every mutating tool, `Hook` asks a callback for each call. File
//! paths and compute limits are enforced by the service
//! (`ServiceConfig::root`, `max_cells`, `max_steps_per_call`,
//! `max_run_seconds`), which every tool goes through.

use rcfd_api::service::RpcError;
use serde_json::{json, Value};
use std::sync::Arc;

/// What a tool does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    /// Inspects state only.
    Read,
    /// Changes the session (settings, mesh).
    Mutate,
    /// Starts computation.
    Execute,
    /// Writes files to disk.
    WriteFiles,
}

impl Access {
    /// Stable name used in `_meta`.
    pub fn name(self) -> &'static str {
        match self {
            Access::Read => "read",
            Access::Mutate => "mutate",
            Access::Execute => "execute",
            Access::WriteFiles => "write_files",
        }
    }
}

/// A tool invocation presented to the approval logic.
#[derive(Debug, Clone, Copy)]
pub struct ToolCall<'a> {
    /// Tool name.
    pub tool: &'a str,
    /// Its access class.
    pub access: Access,
    /// Validated arguments.
    pub args: &'a Value,
}

/// Callback deciding whether a mutating call may proceed.
pub type ApprovalHook = Arc<dyn Fn(&ToolCall<'_>) -> bool + Send + Sync>;

/// How mutating tools are authorised.
#[derive(Clone)]
pub enum Approval {
    /// Every tool may run (the host client is expected to prompt using the annotations).
    Auto,
    /// Only `Read` tools may run.
    ReadOnly,
    /// Mutating tools run only when the hook returns true.
    Hook(ApprovalHook),
}

/// Server-side permission policy.
#[derive(Clone)]
pub struct Policy {
    /// Approval mode for mutating tools.
    pub approval: Approval,
    /// When set, only these tools may be called.
    pub allow: Option<Vec<String>>,
}

impl Default for Policy {
    fn default() -> Self {
        Policy {
            approval: Approval::Auto,
            allow: None,
        }
    }
}

/// Error code for a denied call (reported inside a tool result).
pub const PERMISSION_DENIED: i64 = 1100;

impl Policy {
    /// Read-only policy.
    pub fn read_only() -> Self {
        Policy {
            approval: Approval::ReadOnly,
            allow: None,
        }
    }

    /// Policy asking `hook` before every mutating call.
    pub fn with_hook(hook: impl Fn(&ToolCall<'_>) -> bool + Send + Sync + 'static) -> Self {
        Policy {
            approval: Approval::Hook(Arc::new(hook)),
            allow: None,
        }
    }

    /// Decide whether a call may proceed.
    pub fn authorize(&self, call: &ToolCall<'_>) -> Result<(), RpcError> {
        let deny = |why: String| RpcError {
            code: PERMISSION_DENIED,
            message: format!("permission denied: {why}"),
            data: Some(json!({"tool": call.tool, "access": call.access.name()})),
        };
        if let Some(allow) = &self.allow {
            if !allow.iter().any(|a| a == call.tool) {
                return Err(deny(format!("tool '{}' is not enabled", call.tool)));
            }
        }
        if call.access == Access::Read {
            return Ok(());
        }
        match &self.approval {
            Approval::Auto => Ok(()),
            Approval::ReadOnly => Err(deny(format!(
                "'{}' modifies state and the server is read-only",
                call.tool
            ))),
            Approval::Hook(h) if h(call) => Ok(()),
            Approval::Hook(_) => Err(deny(format!("'{}' was not approved", call.tool))),
        }
    }
}
