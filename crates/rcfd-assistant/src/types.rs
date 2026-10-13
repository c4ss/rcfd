//! Provider-neutral conversation types.

use serde_json::Value;

/// Key placed in a tool call's `input` when the model produced arguments that
/// were not valid JSON; the value is the raw text. The agent turns this into a
/// tool error so the model can retry.
pub const INVALID_ARGS_KEY: &str = "__invalid_arguments";

/// A secret string (API key). `Debug` is redacted and there is no `Display`.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    /// Wrap a secret.
    pub fn new(s: impl Into<String>) -> Self {
        Secret(s.into())
    }

    /// The secret text; use only when building a request header.
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// Replace every occurrence of the secret in `text` with `***`.
    pub fn scrub(&self, text: &str) -> String {
        if self.0.len() < 4 {
            return text.to_string();
        }
        text.replace(&self.0, "***")
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret(***)")
    }
}

/// Who wrote a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// The human (also carries tool results).
    User,
    /// The model.
    Assistant,
}

/// A piece of message content.
#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    /// Plain text.
    Text(String),
    /// The model asks to call a tool.
    ToolUse {
        /// Call id, echoed in the result.
        id: String,
        /// Tool name.
        name: String,
        /// Arguments.
        input: Value,
    },
    /// The result of a tool call.
    ToolResult {
        /// Id of the call being answered.
        id: String,
        /// Result text.
        content: String,
        /// True when the call failed or was refused.
        is_error: bool,
    },
}

/// One conversation message.
#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    /// Author.
    pub role: Role,
    /// Content blocks.
    pub blocks: Vec<Block>,
}

impl Message {
    /// A user text message.
    pub fn user(text: impl Into<String>) -> Self {
        Message {
            role: Role::User,
            blocks: vec![Block::Text(text.into())],
        }
    }

    /// Concatenated text blocks.
    pub fn text(&self) -> String {
        self.blocks
            .iter()
            .filter_map(|b| match b {
                Block::Text(t) => Some(t.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// A tool offered to the model.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolSpec {
    /// Name.
    pub name: String,
    /// Description.
    pub description: String,
    /// JSON Schema of the arguments (an object schema).
    pub input_schema: Value,
}

/// A chat request.
#[derive(Debug, Clone)]
pub struct ChatRequest {
    /// Model name as the provider knows it.
    pub model: String,
    /// System prompt.
    pub system: String,
    /// Conversation so far.
    pub messages: Vec<Message>,
    /// Tools the model may call.
    pub tools: Vec<ToolSpec>,
    /// Output token cap.
    pub max_tokens: u32,
    /// Sampling temperature, when set.
    pub temperature: Option<f32>,
}

/// Why generation stopped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopReason {
    /// The model finished its turn.
    EndTurn,
    /// The model wants tool results.
    ToolUse,
    /// Output cap reached.
    MaxTokens,
    /// Anything else the provider reported.
    Other(String),
}

/// Token usage when reported.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Usage {
    /// Prompt tokens.
    pub input: u64,
    /// Completion tokens.
    pub output: u64,
}

/// A complete model answer.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatResponse {
    /// Content blocks (text and tool calls).
    pub blocks: Vec<Block>,
    /// Stop reason.
    pub stop: StopReason,
    /// Usage.
    pub usage: Usage,
}

impl ChatResponse {
    /// A text-only end-of-turn response.
    pub fn text(t: impl Into<String>) -> Self {
        ChatResponse {
            blocks: vec![Block::Text(t.into())],
            stop: StopReason::EndTurn,
            usage: Usage::default(),
        }
    }

    /// A response consisting of tool calls (and optional leading text).
    pub fn tool_calls(text: &str, calls: Vec<(&str, &str, Value)>) -> Self {
        let mut blocks = Vec::new();
        if !text.is_empty() {
            blocks.push(Block::Text(text.to_string()));
        }
        for (id, name, input) in calls {
            blocks.push(Block::ToolUse {
                id: id.into(),
                name: name.into(),
                input,
            });
        }
        ChatResponse {
            blocks,
            stop: StopReason::ToolUse,
            usage: Usage::default(),
        }
    }
}

/// Incremental output while a response streams in.
#[derive(Debug, Clone, PartialEq)]
pub enum StreamEvent {
    /// A piece of assistant text.
    Text(String),
    /// The model started a tool call.
    ToolCall {
        /// Tool name.
        name: String,
    },
}

/// Provider failures. Messages never contain the API key.
#[derive(Debug, Clone, PartialEq)]
pub enum ProviderError {
    /// Missing or invalid configuration.
    Config(String),
    /// Connection-level failure.
    Transport(String),
    /// The server answered with an error status.
    Http {
        /// Status code.
        status: u16,
        /// Trimmed, key-scrubbed body.
        message: String,
    },
    /// The answer did not match the documented format.
    Parse(String),
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProviderError::Config(m) => write!(f, "provider configuration: {m}"),
            ProviderError::Transport(m) => write!(f, "network error: {m}"),
            ProviderError::Http { status, message } => write!(f, "provider returned HTTP {status}: {message}"),
            ProviderError::Parse(m) => write!(f, "unexpected provider response: {m}"),
        }
    }
}

impl std::error::Error for ProviderError {}

/// A chat model backend.
pub trait Provider: Send + Sync {
    /// Short display name, e.g. `openai-compatible`.
    fn name(&self) -> String;

    /// Run one chat request to completion.
    fn chat(&self, req: &ChatRequest) -> Result<ChatResponse, ProviderError>;

    /// Like [`Provider::chat`] but reports text as it arrives. The default
    /// implementation is not incremental: it calls `chat` and emits the text at once.
    fn chat_stream(
        &self,
        req: &ChatRequest,
        on_event: &mut dyn FnMut(StreamEvent),
    ) -> Result<ChatResponse, ProviderError> {
        let r = self.chat(req)?;
        emit_events(&r, on_event);
        Ok(r)
    }
}

/// Emit stream events describing a finished response.
pub fn emit_events(r: &ChatResponse, on_event: &mut dyn FnMut(StreamEvent)) {
    for b in &r.blocks {
        match b {
            Block::Text(t) if !t.is_empty() => on_event(StreamEvent::Text(t.clone())),
            Block::ToolUse { name, .. } => on_event(StreamEvent::ToolCall { name: name.clone() }),
            _ => {}
        }
    }
}
