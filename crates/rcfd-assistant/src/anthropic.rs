//! Anthropic Messages API adapter (`POST {base_url}/v1/messages`).
//!
//! Request/response shapes follow the published Messages API reference
//! (headers `x-api-key` and `anthropic-version`, `tools[].input_schema`,
//! `tool_use` / `tool_result` content blocks, server-sent streaming events).
//! Tested only against fixtures; not exercised against the live service.

use crate::http::{trim_body, HttpRequest, HttpResponse, Transport};
use crate::types::*;
use serde_json::{json, Map, Value};
use std::sync::Arc;

/// Default API base URL.
pub const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";
/// Value sent in the `anthropic-version` header.
pub const API_VERSION: &str = "2023-06-01";

/// Anthropic Messages provider.
pub struct Anthropic {
    base_url: String,
    key: Secret,
    transport: Arc<dyn Transport>,
}

impl Anthropic {
    /// Provider for `base_url` (empty = the default) with the user's key.
    pub fn new(base_url: &str, key: Secret, transport: Arc<dyn Transport>) -> Self {
        let base = if base_url.trim().is_empty() {
            DEFAULT_BASE_URL
        } else {
            base_url
        };
        Anthropic {
            base_url: base.trim_end_matches('/').to_string(),
            key,
            transport,
        }
    }

    /// Build the HTTP request for `req`.
    pub fn build_request(&self, req: &ChatRequest, stream: bool) -> HttpRequest {
        HttpRequest {
            url: format!("{}/v1/messages", self.base_url),
            headers: vec![
                ("x-api-key".into(), self.key.expose().to_string()),
                ("anthropic-version".into(), API_VERSION.into()),
                ("content-type".into(), "application/json".into()),
            ],
            body: Some(request_body(req, stream).to_string()),
        }
    }

    fn check(&self, r: HttpResponse) -> Result<HttpResponse, ProviderError> {
        if (200..300).contains(&r.status) {
            Ok(r)
        } else {
            Err(ProviderError::Http {
                status: r.status,
                message: self.key.scrub(&trim_body(&r.body)),
            })
        }
    }
}

/// The JSON request body.
pub fn request_body(req: &ChatRequest, stream: bool) -> Value {
    let messages: Vec<Value> = req
        .messages
        .iter()
        .map(|m| {
            let content: Vec<Value> = m
                .blocks
                .iter()
                .map(|b| match b {
                    Block::Text(t) => json!({"type": "text", "text": t}),
                    Block::ToolUse { id, name, input } => {
                        let input = if input.get(INVALID_ARGS_KEY).is_some() {
                            json!({})
                        } else {
                            input.clone()
                        };
                        json!({"type": "tool_use", "id": id, "name": name, "input": input})
                    }
                    Block::ToolResult { id, content, is_error } => {
                        json!({"type": "tool_result", "tool_use_id": id, "content": content, "is_error": is_error})
                    }
                })
                .collect();
            json!({"role": match m.role { Role::User => "user", Role::Assistant => "assistant" }, "content": content})
        })
        .collect();
    let mut body = Map::new();
    body.insert("model".into(), json!(req.model));
    body.insert("max_tokens".into(), json!(req.max_tokens));
    if !req.system.is_empty() {
        body.insert("system".into(), json!(req.system));
    }
    body.insert("messages".into(), Value::Array(messages));
    if !req.tools.is_empty() {
        let tools: Vec<Value> = req
            .tools
            .iter()
            .map(|t| json!({"name": t.name, "description": t.description, "input_schema": t.input_schema}))
            .collect();
        body.insert("tools".into(), Value::Array(tools));
    }
    if let Some(t) = req.temperature {
        body.insert("temperature".into(), json!(t));
    }
    if stream {
        body.insert("stream".into(), json!(true));
    }
    Value::Object(body)
}

fn stop_from(s: Option<&str>) -> StopReason {
    match s {
        Some("end_turn") | None => StopReason::EndTurn,
        Some("tool_use") => StopReason::ToolUse,
        Some("max_tokens") => StopReason::MaxTokens,
        Some(o) => StopReason::Other(o.into()),
    }
}

/// Parse a non-streaming response body.
pub fn parse_response(body: &str) -> Result<ChatResponse, ProviderError> {
    let v: Value = serde_json::from_str(body).map_err(|e| ProviderError::Parse(format!("not JSON: {e}")))?;
    let content = v["content"]
        .as_array()
        .ok_or_else(|| ProviderError::Parse("response has no content array".into()))?;
    let mut blocks = vec![];
    for c in content {
        match c["type"].as_str() {
            Some("text") => blocks.push(Block::Text(c["text"].as_str().unwrap_or_default().to_string())),
            Some("tool_use") => blocks.push(Block::ToolUse {
                id: c["id"].as_str().unwrap_or_default().to_string(),
                name: c["name"].as_str().unwrap_or_default().to_string(),
                input: c["input"].clone(),
            }),
            _ => {}
        }
    }
    Ok(ChatResponse {
        blocks,
        stop: stop_from(v["stop_reason"].as_str()),
        usage: Usage {
            input: v["usage"]["input_tokens"].as_u64().unwrap_or(0),
            output: v["usage"]["output_tokens"].as_u64().unwrap_or(0),
        },
    })
}

enum Open {
    Text(String),
    Tool { id: String, name: String, json: String },
    Other,
}

/// Accumulates the server-sent events of a streamed message.
#[derive(Default)]
pub struct StreamAccumulator {
    blocks: Vec<Block>,
    open: Option<Open>,
    stop: Option<String>,
    usage: Usage,
    stopped: bool,
}

impl StreamAccumulator {
    /// Feed one SSE line (`event:` lines are ignored; the JSON `type` is authoritative).
    pub fn feed(&mut self, line: &str, on_event: &mut dyn FnMut(StreamEvent)) -> Result<(), ProviderError> {
        let Some(data) = line.strip_prefix("data:") else {
            return Ok(());
        };
        let v: Value =
            serde_json::from_str(data.trim()).map_err(|e| ProviderError::Parse(format!("bad stream event: {e}")))?;
        match v["type"].as_str() {
            Some("message_start") => {
                self.usage.input = v["message"]["usage"]["input_tokens"].as_u64().unwrap_or(0);
            }
            Some("content_block_start") => {
                let cb = &v["content_block"];
                self.open = Some(match cb["type"].as_str() {
                    Some("text") => Open::Text(cb["text"].as_str().unwrap_or_default().to_string()),
                    Some("tool_use") => {
                        let name = cb["name"].as_str().unwrap_or_default().to_string();
                        on_event(StreamEvent::ToolCall { name: name.clone() });
                        Open::Tool {
                            id: cb["id"].as_str().unwrap_or_default().to_string(),
                            name,
                            json: String::new(),
                        }
                    }
                    _ => Open::Other,
                });
            }
            Some("content_block_delta") => {
                let d = &v["delta"];
                match (&mut self.open, d["type"].as_str()) {
                    (Some(Open::Text(t)), Some("text_delta")) => {
                        let s = d["text"].as_str().unwrap_or_default();
                        t.push_str(s);
                        on_event(StreamEvent::Text(s.to_string()));
                    }
                    (Some(Open::Tool { json, .. }), Some("input_json_delta")) => {
                        json.push_str(d["partial_json"].as_str().unwrap_or_default());
                    }
                    _ => {}
                }
            }
            Some("content_block_stop") => match self.open.take() {
                Some(Open::Text(t)) => self.blocks.push(Block::Text(t)),
                Some(Open::Tool { id, name, json }) => {
                    let input = if json.trim().is_empty() {
                        serde_json::json!({})
                    } else {
                        match serde_json::from_str::<Value>(&json) {
                            Ok(v) if v.is_object() => v,
                            _ => serde_json::json!({ INVALID_ARGS_KEY: json }),
                        }
                    };
                    self.blocks.push(Block::ToolUse { id, name, input });
                }
                _ => {}
            },
            Some("message_delta") => {
                if let Some(s) = v["delta"]["stop_reason"].as_str() {
                    self.stop = Some(s.to_string());
                }
                if let Some(o) = v["usage"]["output_tokens"].as_u64() {
                    self.usage.output = o;
                }
            }
            Some("message_stop") => self.stopped = true,
            Some("error") => {
                return Err(ProviderError::Http {
                    status: 200,
                    message: trim_body(&v["error"].to_string()),
                })
            }
            _ => {}
        }
        Ok(())
    }

    /// Finish and build the response.
    pub fn finish(self) -> Result<ChatResponse, ProviderError> {
        if !self.stopped {
            return Err(ProviderError::Parse("stream ended before message_stop".into()));
        }
        Ok(ChatResponse {
            blocks: self.blocks,
            stop: stop_from(self.stop.as_deref()),
            usage: self.usage,
        })
    }
}

impl Provider for Anthropic {
    fn name(&self) -> String {
        "anthropic".into()
    }

    fn chat(&self, req: &ChatRequest) -> Result<ChatResponse, ProviderError> {
        let r = self.transport.send(&self.build_request(req, false))?;
        let r = self.check(r)?;
        parse_response(&r.body)
    }

    fn chat_stream(
        &self,
        req: &ChatRequest,
        on_event: &mut dyn FnMut(StreamEvent),
    ) -> Result<ChatResponse, ProviderError> {
        let mut acc = StreamAccumulator::default();
        let mut err = None;
        let r = self.transport.send_lines(&self.build_request(req, true), &mut |l| {
            if err.is_none() {
                if let Err(e) = acc.feed(l, on_event) {
                    err = Some(e);
                }
            }
        })?;
        self.check(r)?;
        if let Some(e) = err {
            return Err(e);
        }
        acc.finish()
    }
}
