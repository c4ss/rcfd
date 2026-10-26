//! Tolerant parser of `claude -p --output-format stream-json --verbose --include-partial-messages` lines.
//!
//! No schema is published. The shapes used here were observed on Claude Code 2.1.310 (ADR 0126):
//! `system/init` (session_id, permissionMode, claude_code_version, model), `stream_event` wrapping
//! `content_block_delta` / `text_delta` (text), `assistant` (message.content[] with `tool_use` blocks: id, name, input),
//! `system/permission_denied` (tool_name, tool_use_id, message) and `result` (subtype, is_error, result, session_id,
//! total_cost_usd, permission_denials[]). Text is taken from the deltas only, because the full `assistant` event repeats it.

use crate::allow::AllowRule;
use serde_json::Value;

/// A denied call as listed in the final `result` event.
#[derive(Clone, Debug, PartialEq)]
pub struct Denial {
    /// Tool name.
    pub tool_name: String,
    /// Tool use id.
    pub tool_use_id: String,
    /// The input the model asked for.
    pub tool_input: Value,
}

/// One event of a headless chat, in stream order.
#[derive(Clone, Debug, PartialEq)]
pub enum StreamEvent {
    /// First event: the session id the CLI uses and the permission mode it applied.
    Init {
        /// Session id.
        session_id: String,
        /// Permission mode in effect (the driver aborts unless it is `dontAsk`).
        permission_mode: Option<String>,
        /// `claude_code_version`.
        version: Option<String>,
        /// Model name.
        model: Option<String>,
    },
    /// A piece of assistant text.
    TextDelta(String),
    /// A complete tool call.
    ToolCall {
        /// Tool use id.
        id: String,
        /// Tool name.
        name: String,
        /// Input.
        input: Value,
    },
    /// A tool call was denied. `suggestion` is the exact rule that would allow it (filled in by the driver).
    PermissionDenied {
        /// Tool name.
        tool_name: String,
        /// Tool use id.
        tool_use_id: String,
        /// The CLI's explanation.
        message: String,
        /// "Allow this": a rule for the allow-list, never a mode change.
        suggestion: Option<AllowRule>,
    },
    /// End of the turn.
    Result {
        /// Session id (the same as `Init` for `--resume` and `--session-id`, as observed).
        session_id: String,
        /// `is_error`.
        is_error: bool,
        /// `subtype` (observed: `success`).
        subtype: String,
        /// Final text.
        text: Option<String>,
        /// `total_cost_usd`.
        cost_usd: Option<f64>,
        /// `permission_denials`.
        denials: Vec<Denial>,
    },
    /// The driver stopped the session (e.g. the CLI did not apply `dontAsk`).
    Aborted(String),
    /// The process ended (always the last event from the driver).
    Exited {
        /// Exit code, `None` when killed.
        code: Option<i32>,
        /// Last part of stderr.
        stderr_tail: String,
    },
    /// A line of unknown or changed shape; `kind` is `type` or `type/subtype` when present.
    Unrecognised {
        /// What the line claimed to be.
        kind: Option<String>,
    },
}

fn s(v: &Value, k: &str) -> Option<String> {
    v.get(k).and_then(Value::as_str).map(String::from)
}

/// Parses one stdout line. Empty for blank lines and for known shapes the UI does not need (status, rate limits,
/// non-text stream events, tool results echoed as `user`); one event per `tool_use` block of an `assistant` line.
pub fn parse_line(line: &str) -> Vec<StreamEvent> {
    if line.trim().is_empty() {
        return Vec::new();
    }
    let Ok(v) = serde_json::from_str::<Value>(line) else {
        return vec![StreamEvent::Unrecognised { kind: None }];
    };
    if v.get("type").and_then(Value::as_str) != Some("assistant") {
        return parse_other(&v).into_iter().collect();
    }
    let Some(content) = v
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(Value::as_array)
    else {
        return vec![StreamEvent::Unrecognised {
            kind: Some("assistant".into()),
        }];
    };
    content
        .iter()
        .filter(|b| b.get("type").and_then(Value::as_str) == Some("tool_use"))
        .map(|b| match (s(b, "id"), s(b, "name")) {
            (Some(id), Some(name)) => StreamEvent::ToolCall {
                id,
                name,
                input: b.get("input").cloned().unwrap_or(Value::Null),
            },
            _ => StreamEvent::Unrecognised {
                kind: Some("assistant/tool_use".into()),
            },
        })
        .collect()
}

fn parse_other(v: &Value) -> Option<StreamEvent> {
    let unrec = |kind: Option<String>| Some(StreamEvent::Unrecognised { kind });
    let Some(ty) = s(v, "type") else {
        return unrec(None);
    };
    let sub = s(v, "subtype");
    let kind = || {
        Some(match &sub {
            Some(st) => format!("{ty}/{st}"),
            None => ty.clone(),
        })
    };
    match (ty.as_str(), sub.as_deref()) {
        ("system", Some("init")) => match s(v, "session_id") {
            Some(session_id) => Some(StreamEvent::Init {
                session_id,
                permission_mode: s(v, "permissionMode"),
                version: s(v, "claude_code_version"),
                model: s(v, "model"),
            }),
            None => unrec(kind()),
        },
        ("system", Some("permission_denied")) => match (s(v, "tool_name"), s(v, "tool_use_id")) {
            (Some(tool_name), Some(tool_use_id)) => Some(StreamEvent::PermissionDenied {
                tool_name,
                tool_use_id,
                message: s(v, "message").unwrap_or_default(),
                suggestion: None,
            }),
            _ => unrec(kind()),
        },
        ("system", Some("status" | "thinking_tokens")) | ("rate_limit_event", _) | ("user", _) => None,
        ("stream_event", _) => {
            let Some(ev) = v.get("event") else { return unrec(kind()) };
            match ev.get("type").and_then(Value::as_str) {
                Some("content_block_delta") => {
                    let Some(d) = ev.get("delta") else {
                        return unrec(Some("stream_event/content_block_delta".into()));
                    };
                    match d.get("type").and_then(Value::as_str) {
                        Some("text_delta") => match d.get("text").and_then(Value::as_str) {
                            Some(t) => Some(StreamEvent::TextDelta(t.to_string())),
                            None => unrec(Some("stream_event/text_delta".into())),
                        },
                        _ => None,
                    }
                }
                Some(_) => None,
                None => unrec(kind()),
            }
        }
        ("result", _) => match s(v, "session_id") {
            Some(session_id) => Some(StreamEvent::Result {
                session_id,
                is_error: v.get("is_error").and_then(Value::as_bool).unwrap_or(true),
                subtype: sub.unwrap_or_default(),
                text: s(v, "result"),
                cost_usd: v.get("total_cost_usd").and_then(Value::as_f64),
                denials: v
                    .get("permission_denials")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .filter_map(|d| {
                                Some(Denial {
                                    tool_name: s(d, "tool_name")?,
                                    tool_use_id: s(d, "tool_use_id")?,
                                    tool_input: d.get("tool_input").cloned().unwrap_or(Value::Null),
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
            }),
            None => unrec(kind()),
        },
        _ => unrec(kind()),
    }
}
