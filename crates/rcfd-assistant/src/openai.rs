//! OpenAI-compatible Chat Completions adapter (`POST {base_url}/chat/completions`).
//!
//! Works with any server that implements that endpoint with function calling:
//! OpenAI itself, Ollama (`http://localhost:11434/v1`), LM Studio
//! (`http://localhost:1234/v1`), vLLM, llama.cpp's server and others. Tested
//! only against fixtures written from the published format; not exercised
//! against a live server.

use crate::http::{trim_body, HttpRequest, HttpResponse, Transport};
use crate::types::*;
use serde_json::{json, Map, Value};
use std::sync::Arc;

/// OpenAI-compatible provider.
pub struct OpenAiCompatible {
    base_url: String,
    key: Option<Secret>,
    transport: Arc<dyn Transport>,
}

impl OpenAiCompatible {
    /// Provider for `base_url` (without the `/chat/completions` suffix). The key is optional.
    pub fn new(base_url: &str, key: Option<Secret>, transport: Arc<dyn Transport>) -> Self {
        OpenAiCompatible {
            base_url: base_url.trim_end_matches('/').to_string(),
            key,
            transport,
        }
    }

    /// Build the HTTP request for `req`.
    pub fn build_request(&self, req: &ChatRequest, stream: bool) -> HttpRequest {
        let mut headers = vec![("Content-Type".to_string(), "application/json".to_string())];
        if let Some(k) = &self.key {
            headers.push(("Authorization".into(), format!("Bearer {}", k.expose())));
        }
        HttpRequest {
            url: format!("{}/chat/completions", self.base_url),
            headers,
            body: Some(request_body(req, stream).to_string()),
        }
    }

    /// Ask the server which models it offers (`GET {base_url}/models`).
    pub fn list_models(&self) -> Result<Vec<String>, ProviderError> {
        let mut headers = vec![];
        if let Some(k) = &self.key {
            headers.push(("Authorization".to_string(), format!("Bearer {}", k.expose())));
        }
        let r = self.transport.send(&HttpRequest {
            url: format!("{}/models", self.base_url),
            headers,
            body: None,
        })?;
        let r = self.check(r)?;
        let v: Value = serde_json::from_str(&r.body).map_err(|e| ProviderError::Parse(e.to_string()))?;
        Ok(v["data"]
            .as_array()
            .map(|a| a.iter().filter_map(|m| m["id"].as_str().map(String::from)).collect())
            .unwrap_or_default())
    }

    fn scrub(&self, s: &str) -> String {
        match &self.key {
            Some(k) => k.scrub(s),
            None => s.to_string(),
        }
    }

    fn check(&self, r: HttpResponse) -> Result<HttpResponse, ProviderError> {
        if (200..300).contains(&r.status) {
            Ok(r)
        } else {
            Err(ProviderError::Http {
                status: r.status,
                message: self.scrub(&trim_body(&r.body)),
            })
        }
    }
}

/// The JSON request body for a chat request.
pub fn request_body(req: &ChatRequest, stream: bool) -> Value {
    let mut messages = vec![];
    if !req.system.is_empty() {
        messages.push(json!({"role": "system", "content": req.system}));
    }
    for m in &req.messages {
        match m.role {
            Role::User => {
                for b in &m.blocks {
                    if let Block::ToolResult { id, content, .. } = b {
                        messages.push(json!({"role": "tool", "tool_call_id": id, "content": content}));
                    }
                }
                let text = m.text();
                if !text.is_empty() {
                    messages.push(json!({"role": "user", "content": text}));
                }
            }
            Role::Assistant => {
                let text = m.text();
                let calls: Vec<Value> = m
                    .blocks
                    .iter()
                    .filter_map(|b| match b {
                        Block::ToolUse { id, name, input } => {
                            let args = match input.get(INVALID_ARGS_KEY).and_then(Value::as_str) {
                                Some(raw) => raw.to_string(),
                                None => input.to_string(),
                            };
                            Some(json!({"id": id, "type": "function", "function": {"name": name, "arguments": args}}))
                        }
                        _ => None,
                    })
                    .collect();
                let mut msg = Map::new();
                msg.insert("role".into(), json!("assistant"));
                msg.insert(
                    "content".into(),
                    if text.is_empty() { Value::Null } else { json!(text) },
                );
                if !calls.is_empty() {
                    msg.insert("tool_calls".into(), Value::Array(calls));
                }
                messages.push(Value::Object(msg));
            }
        }
    }
    let mut body = Map::new();
    body.insert("model".into(), json!(req.model));
    body.insert("messages".into(), Value::Array(messages));
    if !req.tools.is_empty() {
        let tools: Vec<Value> = req
            .tools
            .iter()
            .map(|t| {
                json!({"type": "function", "function": {
                    "name": t.name, "description": t.description, "parameters": t.input_schema}})
            })
            .collect();
        body.insert("tools".into(), Value::Array(tools));
    }
    body.insert("max_tokens".into(), json!(req.max_tokens));
    if let Some(t) = req.temperature {
        body.insert("temperature".into(), json!(t));
    }
    if stream {
        body.insert("stream".into(), json!(true));
    }
    Value::Object(body)
}

fn parse_args(name: &str, raw: &str) -> Value {
    let _ = name;
    if raw.trim().is_empty() {
        return json!({});
    }
    match serde_json::from_str::<Value>(raw) {
        Ok(v) if v.is_object() => v,
        _ => json!({ INVALID_ARGS_KEY: raw }),
    }
}

fn stop_from(finish: Option<&str>, has_calls: bool) -> StopReason {
    if has_calls {
        return StopReason::ToolUse;
    }
    match finish {
        Some("stop") | None => StopReason::EndTurn,
        Some("tool_calls") | Some("function_call") => StopReason::ToolUse,
        Some("length") => StopReason::MaxTokens,
        Some(o) => StopReason::Other(o.into()),
    }
}

/// Parse a non-streaming response body.
pub fn parse_response(body: &str) -> Result<ChatResponse, ProviderError> {
    let v: Value = serde_json::from_str(body).map_err(|e| ProviderError::Parse(format!("not JSON: {e}")))?;
    let choice = v["choices"]
        .get(0)
        .ok_or_else(|| ProviderError::Parse("no choices in response".into()))?;
    let msg = &choice["message"];
    let mut blocks = vec![];
    if let Some(t) = msg["content"].as_str() {
        if !t.is_empty() {
            blocks.push(Block::Text(t.to_string()));
        }
    }
    let mut has_calls = false;
    for c in msg["tool_calls"].as_array().into_iter().flatten() {
        let name = c["function"]["name"].as_str().unwrap_or_default();
        let args = c["function"]["arguments"].as_str().unwrap_or("");
        has_calls = true;
        blocks.push(Block::ToolUse {
            id: c["id"].as_str().unwrap_or("call_0").to_string(),
            name: name.to_string(),
            input: parse_args(name, args),
        });
    }
    Ok(ChatResponse {
        blocks,
        stop: stop_from(choice["finish_reason"].as_str(), has_calls),
        usage: Usage {
            input: v["usage"]["prompt_tokens"].as_u64().unwrap_or(0),
            output: v["usage"]["completion_tokens"].as_u64().unwrap_or(0),
        },
    })
}

/// Accumulates server-sent-event lines of a streamed completion.
#[derive(Default)]
pub struct StreamAccumulator {
    text: String,
    calls: Vec<(String, String, String)>,
    finish: Option<String>,
    usage: Usage,
    done: bool,
}

impl StreamAccumulator {
    /// Feed one line of the SSE stream.
    pub fn feed(&mut self, line: &str, on_event: &mut dyn FnMut(StreamEvent)) -> Result<(), ProviderError> {
        let Some(data) = line.strip_prefix("data:") else {
            return Ok(());
        };
        let data = data.trim();
        if data == "[DONE]" {
            self.done = true;
            return Ok(());
        }
        let v: Value =
            serde_json::from_str(data).map_err(|e| ProviderError::Parse(format!("bad stream chunk: {e}")))?;
        if let Some(u) = v.get("usage").filter(|u| u.is_object()) {
            self.usage = Usage {
                input: u["prompt_tokens"].as_u64().unwrap_or(0),
                output: u["completion_tokens"].as_u64().unwrap_or(0),
            };
        }
        let Some(choice) = v["choices"].get(0) else {
            return Ok(());
        };
        if let Some(f) = choice["finish_reason"].as_str() {
            self.finish = Some(f.to_string());
        }
        let delta = &choice["delta"];
        if let Some(t) = delta["content"].as_str() {
            if !t.is_empty() {
                self.text.push_str(t);
                on_event(StreamEvent::Text(t.to_string()));
            }
        }
        for c in delta["tool_calls"].as_array().into_iter().flatten() {
            let idx = c["index"].as_u64().unwrap_or(0) as usize;
            if idx > 64 {
                return Err(ProviderError::Parse("tool call index out of range".into()));
            }
            while self.calls.len() <= idx {
                self.calls.push(Default::default());
            }
            let slot = &mut self.calls[idx];
            if let Some(id) = c["id"].as_str() {
                slot.0 = id.to_string();
            }
            if let Some(n) = c["function"]["name"].as_str() {
                if slot.1.is_empty() {
                    on_event(StreamEvent::ToolCall { name: n.to_string() });
                }
                slot.1.push_str(n);
            }
            if let Some(a) = c["function"]["arguments"].as_str() {
                slot.2.push_str(a);
            }
        }
        Ok(())
    }

    /// Finish and build the response.
    pub fn finish(self) -> Result<ChatResponse, ProviderError> {
        if !self.done && self.finish.is_none() {
            return Err(ProviderError::Parse(
                "stream ended before a finish reason or [DONE]".into(),
            ));
        }
        let mut blocks = vec![];
        if !self.text.is_empty() {
            blocks.push(Block::Text(self.text));
        }
        let has_calls = self.calls.iter().any(|c| !c.1.is_empty());
        for (i, (id, name, args)) in self.calls.into_iter().enumerate() {
            if name.is_empty() {
                continue;
            }
            blocks.push(Block::ToolUse {
                id: if id.is_empty() { format!("call_{i}") } else { id },
                input: parse_args(&name, &args),
                name,
            });
        }
        Ok(ChatResponse {
            blocks,
            stop: stop_from(self.finish.as_deref(), has_calls),
            usage: self.usage,
        })
    }
}

impl Provider for OpenAiCompatible {
    fn name(&self) -> String {
        "openai-compatible".into()
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
