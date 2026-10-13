//! The assistant loop as a resumable state machine.
//!
//! [`Agent::step`] runs until it either needs the user to approve a mutating
//! tool call ([`Step::NeedApproval`]) or the model finishes its turn
//! ([`Step::Done`]). The caller answers with [`Agent::resolve`] and steps again.
//! Nothing blocks inside the agent apart from the provider call itself, so a
//! UI can drive it from a worker thread and show approval cards.

use crate::tools::{ApprovalMode, ToolOutput, Toolbox};
use crate::types::*;
use rcfd_mcp::policy::Access;
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// What the user is looking at; rendered as context chips and sent to the model.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AgentContext {
    /// Title of the open case.
    pub case_title: Option<String>,
    /// Service session id.
    pub session: Option<String>,
    /// Run state (`ready`, `running`, ...).
    pub run_state: Option<String>,
    /// Selected setting path in the settings editor.
    pub selection: Option<String>,
}

impl AgentContext {
    /// Context rendered for the system prompt.
    pub fn render(&self) -> String {
        let mut s = String::from("Current context:\n");
        let line = |k: &str, v: &Option<String>| format!("- {k}: {}\n", v.as_deref().unwrap_or("none"));
        s.push_str(&line("case", &self.case_title));
        s.push_str(&line("session", &self.session));
        s.push_str(&line("run state", &self.run_state));
        s.push_str(&line("selected setting", &self.selection));
        s
    }
}

/// Base instructions for the model.
pub const SYSTEM_PROMPT: &str = "You are the rcfd assistant, built into the rcfd desktop CFD application. \
You help the user set up, check, run and understand CFD cases by calling the provided tools.\n\
Rules:\n\
- Use tools to inspect the real case state; never guess values you can read.\n\
- Before running, check the mesh (assess_mesh) and validate the case (validate_case).\n\
- Boundary-condition suggestions are heuristics; values marked placeholder need the user's real numbers, so ask.\n\
- Tools that change the case or start a run need the user's approval; explain briefly why you want each change.\n\
- Make small, reviewable changes. After a run, check residuals (explain_residuals) and summarise the outcome honestly, including anything unconverged.\n\
- rcfd solves laminar and k-epsilon RANS incompressible flow (steady, with standard wall functions), Boussinesq heat transfer, and compressible Euler flow (explicit finite volume, no viscosity); say so when a request needs more (for example LES, viscous compressible flow, or turbulence with heat transfer).";

/// A mutating tool call waiting for the user.
#[derive(Debug, Clone, PartialEq)]
pub struct PendingCall {
    /// Call id.
    pub id: String,
    /// Tool name.
    pub name: String,
    /// Arguments.
    pub args: Value,
    /// Access class.
    pub access: Access,
}

/// The user's answer to a pending call.
#[derive(Debug, Clone, PartialEq)]
pub enum Decision {
    /// Run it.
    Approve,
    /// Refuse, with an optional reason shown to the model.
    Deny(String),
}

/// Progress notifications.
#[derive(Debug, Clone)]
pub enum AgentEvent {
    /// Streamed assistant text.
    Text(String),
    /// A tool call needs approval.
    ApprovalRequested(PendingCall),
    /// A tool call was executed (or refused) with this output.
    ToolResult {
        /// Call id.
        id: String,
        /// Tool name.
        name: String,
        /// Arguments used.
        args: Value,
        /// Result.
        output: ToolOutput,
        /// True if the user denied it.
        denied: bool,
    },
}

/// What [`Agent::step`] returns.
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    /// Waiting for [`Agent::resolve`].
    NeedApproval(PendingCall),
    /// The model finished; carries its final text.
    Done(String),
}

/// Agent failures. After an error the history is still valid and `step` can be retried.
#[derive(Debug, Clone, PartialEq)]
pub enum AgentError {
    /// The provider failed.
    Provider(ProviderError),
    /// The model kept calling tools past the round limit.
    TooManyRounds(usize),
    /// Stopped by the user.
    Cancelled,
    /// `step` was called while a decision is pending, or `resolve` without one.
    Protocol(String),
}

impl std::fmt::Display for AgentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentError::Provider(e) => write!(f, "{e}"),
            AgentError::TooManyRounds(n) => write!(f, "stopped after {n} model rounds without a final answer"),
            AgentError::Cancelled => write!(f, "cancelled"),
            AgentError::Protocol(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for AgentError {}

#[derive(PartialEq)]
enum Phase {
    Idle,
    NeedModel,
    Calls,
}

/// A conversation with a model that can use the rcfd tools.
pub struct Agent {
    provider: Arc<dyn Provider>,
    toolbox: Arc<Toolbox>,
    /// Model name sent to the provider.
    pub model: String,
    /// Output cap per response.
    pub max_tokens: u32,
    /// Max model calls per user message.
    pub max_rounds: usize,
    history: Vec<Message>,
    context: AgentContext,
    phase: Phase,
    queue: VecDeque<(String, String, Value)>,
    awaiting: Option<PendingCall>,
    results: Vec<Block>,
    rounds: usize,
    cancel: Arc<AtomicBool>,
}

impl Agent {
    /// New agent.
    pub fn new(provider: Arc<dyn Provider>, toolbox: Arc<Toolbox>, model: impl Into<String>) -> Self {
        Agent {
            provider,
            toolbox,
            model: model.into(),
            max_tokens: 4096,
            max_rounds: 16,
            history: vec![],
            context: AgentContext::default(),
            phase: Phase::Idle,
            queue: VecDeque::new(),
            awaiting: None,
            results: vec![],
            rounds: 0,
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Replace the provider (e.g. the user picked another model); history is kept.
    pub fn set_provider(&mut self, provider: Arc<dyn Provider>, model: impl Into<String>) {
        self.provider = provider;
        self.model = model.into();
    }

    /// Flag that makes the next `step` stop with [`AgentError::Cancelled`].
    pub fn cancel_flag(&self) -> Arc<AtomicBool> {
        self.cancel.clone()
    }

    /// Update the context sent with the next request.
    pub fn set_context(&mut self, c: AgentContext) {
        self.context = c;
    }

    /// The tool box.
    pub fn toolbox(&self) -> &Arc<Toolbox> {
        &self.toolbox
    }

    /// Conversation so far.
    pub fn history(&self) -> &[Message] {
        &self.history
    }

    /// True when a call is waiting for a decision.
    pub fn awaiting(&self) -> Option<&PendingCall> {
        self.awaiting.as_ref()
    }

    /// Start a turn with a user message.
    pub fn submit(&mut self, text: &str) -> Result<(), AgentError> {
        if self.phase != Phase::Idle {
            return Err(AgentError::Protocol("the previous turn is not finished".into()));
        }
        self.cancel.store(false, Ordering::SeqCst);
        self.history.push(Message::user(text));
        self.phase = Phase::NeedModel;
        self.rounds = 0;
        Ok(())
    }

    /// Conversation as JSON (for saving transcripts).
    pub fn transcript(&self) -> Value {
        let blocks = |m: &Message| -> Vec<Value> {
            m.blocks
                .iter()
                .map(|b| match b {
                    Block::Text(t) => json!({"type": "text", "text": t}),
                    Block::ToolUse { id, name, input } => {
                        json!({"type": "tool_use", "id": id, "name": name, "input": input})
                    }
                    Block::ToolResult { id, content, is_error } => {
                        json!({"type": "tool_result", "id": id, "content": content, "is_error": is_error})
                    }
                })
                .collect()
        };
        json!({"format": "rcfd-assistant-transcript", "version": 1, "model": self.model,
               "messages": self.history.iter().map(|m| json!({"role": if m.role == Role::User {"user"} else {"assistant"}, "content": blocks(m)})).collect::<Vec<_>>()})
    }

    fn request(&self) -> ChatRequest {
        let tools = self
            .toolbox
            .available()
            .into_iter()
            .map(|t| ToolSpec {
                name: t.name,
                description: t.description,
                input_schema: t.schema,
            })
            .collect();
        ChatRequest {
            model: self.model.clone(),
            system: format!("{SYSTEM_PROMPT}\n\n{}", self.context.render()),
            messages: self.history.clone(),
            tools,
            max_tokens: self.max_tokens,
            temperature: None,
        }
    }

    fn fail_result(&mut self, id: String, code: i64, msg: String) {
        let body = json!({"error": {"code": code, "message": msg}}).to_string();
        self.results.push(Block::ToolResult {
            id,
            content: body,
            is_error: true,
        });
    }

    /// Run until approval is needed or the turn is done.
    pub fn step(&mut self, sink: &mut dyn FnMut(AgentEvent)) -> Result<Step, AgentError> {
        if self.awaiting.is_some() {
            return Err(AgentError::Protocol("a tool call is waiting for a decision".into()));
        }
        loop {
            if self.cancel.load(Ordering::SeqCst) && self.phase != Phase::Idle {
                self.abort_turn();
                return Err(AgentError::Cancelled);
            }
            match self.phase {
                Phase::Idle => return Err(AgentError::Protocol("no turn in progress; call submit".into())),
                Phase::NeedModel => {
                    if self.rounds >= self.max_rounds {
                        self.phase = Phase::Idle;
                        return Err(AgentError::TooManyRounds(self.max_rounds));
                    }
                    self.rounds += 1;
                    let req = self.request();
                    let resp = self
                        .provider
                        .chat_stream(&req, &mut |e| {
                            if let StreamEvent::Text(t) = e {
                                sink(AgentEvent::Text(t));
                            }
                        })
                        .map_err(AgentError::Provider)?;
                    let text: String = resp
                        .blocks
                        .iter()
                        .filter_map(|b| if let Block::Text(t) = b { Some(t.as_str()) } else { None })
                        .collect::<Vec<_>>()
                        .join("\n");
                    let calls: Vec<(String, String, Value)> = resp
                        .blocks
                        .iter()
                        .filter_map(|b| match b {
                            Block::ToolUse { id, name, input } => Some((id.clone(), name.clone(), input.clone())),
                            _ => None,
                        })
                        .collect();
                    self.history.push(Message {
                        role: Role::Assistant,
                        blocks: resp.blocks,
                    });
                    if calls.is_empty() {
                        self.phase = Phase::Idle;
                        return Ok(Step::Done(text));
                    }
                    self.queue = calls.into();
                    self.phase = Phase::Calls;
                }
                Phase::Calls => {
                    let Some((id, name, args)) = self.queue.pop_front() else {
                        let blocks = std::mem::take(&mut self.results);
                        self.history.push(Message {
                            role: Role::User,
                            blocks,
                        });
                        self.phase = Phase::NeedModel;
                        continue;
                    };
                    if let Some(p) = self.handle_call(id, name, args, sink) {
                        return Ok(Step::NeedApproval(p));
                    }
                }
            }
        }
    }

    /// Process one call; returns the pending call if approval is required.
    fn handle_call(
        &mut self,
        id: String,
        name: String,
        args: Value,
        sink: &mut dyn FnMut(AgentEvent),
    ) -> Option<PendingCall> {
        let refuse = |this: &mut Self, code: i64, msg: String, sink: &mut dyn FnMut(AgentEvent)| {
            this.fail_result(id.clone(), code, msg.clone());
            let out = ToolOutput {
                text: msg.clone(),
                is_error: true,
                image_png_base64: None,
                structured: json!({"error": {"code": code, "message": msg}}),
            };
            sink(AgentEvent::ToolResult {
                id: id.clone(),
                name: name.clone(),
                args: args.clone(),
                output: out,
                denied: false,
            });
        };
        if let Some(raw) = args.get(INVALID_ARGS_KEY).and_then(Value::as_str) {
            let m = format!("the arguments for '{name}' were not a valid JSON object: {raw}. Call the tool again with a JSON object matching its schema.");
            refuse(self, -32602, m, sink);
            return None;
        }
        let Some(info) = self.toolbox.info(&name).cloned() else {
            let m = format!("unknown tool '{name}'");
            refuse(self, -32601, m, sink);
            return None;
        };
        if let Err(m) = self.toolbox.validate(&name, &args) {
            refuse(self, -32602, format!("invalid arguments for {name}: {m}"), sink);
            return None;
        }
        if info.access != Access::Read && self.toolbox.mode() == ApprovalMode::ReadOnly {
            let m = format!("permission denied: '{name}' changes state and the assistant is in read-only mode");
            refuse(self, rcfd_mcp::policy::PERMISSION_DENIED, m, sink);
            return None;
        }
        if info.access == Access::Read {
            self.run_tool(id, name, args, sink);
            return None;
        }
        let p = PendingCall {
            id,
            name,
            args,
            access: info.access,
        };
        self.awaiting = Some(p.clone());
        sink(AgentEvent::ApprovalRequested(p.clone()));
        Some(p)
    }

    fn run_tool(&mut self, id: String, name: String, args: Value, sink: &mut dyn FnMut(AgentEvent)) {
        let output = self.toolbox.execute(&name, &args);
        self.results.push(Block::ToolResult {
            id: id.clone(),
            content: output.text.clone(),
            is_error: output.is_error,
        });
        sink(AgentEvent::ToolResult {
            id,
            name,
            args,
            output,
            denied: false,
        });
    }

    /// Answer the pending approval.
    pub fn resolve(&mut self, d: Decision, sink: &mut dyn FnMut(AgentEvent)) -> Result<(), AgentError> {
        let p = self
            .awaiting
            .take()
            .ok_or_else(|| AgentError::Protocol("no tool call is waiting".into()))?;
        match d {
            Decision::Approve => {
                self.toolbox.grant(&p.name, &p.args);
                self.run_tool(p.id, p.name, p.args, sink);
            }
            Decision::Deny(reason) => {
                let why = if reason.is_empty() {
                    "no reason given".to_string()
                } else {
                    reason
                };
                let msg = format!("permission denied: the user denied '{}' ({why}). Do not retry the same call; ask what they would prefer.", p.name);
                self.fail_result(p.id.clone(), rcfd_mcp::policy::PERMISSION_DENIED, msg.clone());
                sink(AgentEvent::ToolResult {
                    id: p.id,
                    name: p.name,
                    args: p.args,
                    output: ToolOutput {
                        text: msg.clone(),
                        is_error: true,
                        image_png_base64: None,
                        structured: json!({"error": {"code": rcfd_mcp::policy::PERMISSION_DENIED, "message": msg}}),
                    },
                    denied: true,
                });
            }
        }
        Ok(())
    }

    /// Close the current turn after a cancel so the history stays well formed.
    fn abort_turn(&mut self) {
        if let Some(p) = self.awaiting.take() {
            self.fail_result(p.id, 1100, "cancelled by the user".into());
        }
        while let Some((id, _, _)) = self.queue.pop_front() {
            self.fail_result(id, 1100, "cancelled by the user".into());
        }
        if !self.results.is_empty() {
            let blocks = std::mem::take(&mut self.results);
            self.history.push(Message {
                role: Role::User,
                blocks,
            });
        }
        self.phase = Phase::Idle;
    }

    /// Drive a whole turn, asking `decide` for every approval. Convenience for tests and CLIs.
    pub fn run_turn(
        &mut self,
        text: &str,
        decide: &mut dyn FnMut(&PendingCall) -> Decision,
        sink: &mut dyn FnMut(AgentEvent),
    ) -> Result<String, AgentError> {
        self.submit(text)?;
        loop {
            match self.step(sink)? {
                Step::Done(t) => return Ok(t),
                Step::NeedApproval(p) => {
                    let d = decide(&p);
                    self.resolve(d, sink)?;
                }
            }
        }
    }
}
