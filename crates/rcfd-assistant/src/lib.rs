//! Provider-agnostic AI assistant core for the rcfd desktop app (F-145..F-150).
//!
//! * [`types`]: conversation types and the [`types::Provider`] trait.
//! * [`openai`], [`anthropic`], [`mock`]: provider adapters. The HTTP ones are
//!   tested against fixtures written from the published request/response
//!   formats; they have not been run against live services.
//! * [`tools`]: the MCP tool set executed in-process plus workflow tools, behind
//!   an approval gate; [`workflow`] holds the analysis logic.
//! * [`agent`]: the resumable assistant loop.
//! * [`config`]: provider configuration and local key storage.

pub mod agent;
pub mod anthropic;
pub mod config;
pub mod http;
pub mod mock;
pub mod openai;
pub mod tools;
pub mod types;
pub mod workflow;

pub use agent::{Agent, AgentContext, AgentError, AgentEvent, Decision, PendingCall, Step};
pub use tools::{ApprovalMode, Toolbox};
pub use types::*;
