//! F-146 / F-149: adapters against fixtures written from the published request
//! and response formats (no live network), plus key handling.

use rcfd_assistant::anthropic::Anthropic;
use rcfd_assistant::config::*;
use rcfd_assistant::http::{FakeTransport, Transport};
use rcfd_assistant::openai::OpenAiCompatible;
use rcfd_assistant::*;
use serde_json::{json, Value};
use std::sync::Arc;

fn request() -> ChatRequest {
    ChatRequest {
        model: "m1".into(),
        system: "be brief".into(),
        messages: vec![
            Message::user("set nu to 0.02"),
            Message {
                role: Role::Assistant,
                blocks: vec![
                    Block::Text("ok".into()),
                    Block::ToolUse {
                        id: "call_1".into(),
                        name: "set_setting".into(),
                        input: json!({"path": "physics.nu", "value": 0.02}),
                    },
                ],
            },
            Message {
                role: Role::User,
                blocks: vec![Block::ToolResult {
                    id: "call_1".into(),
                    content: "{\"ok\":true}".into(),
                    is_error: false,
                }],
            },
        ],
        tools: vec![ToolSpec {
            name: "set_setting".into(),
            description: "Set a setting".into(),
            input_schema: json!({"type": "object", "properties": {"path": {"type": "string"}}}),
        }],
        max_tokens: 256,
        temperature: Some(0.0),
    }
}

fn body(t: &FakeTransport) -> Value {
    serde_json::from_str(t.last_request().unwrap().body.as_deref().unwrap()).unwrap()
}

#[test]
fn openai_request_shape() {
    let t = Arc::new(FakeTransport::new());
    t.push(200, r#"{"choices":[{"message":{"role":"assistant","content":"hi"},"finish_reason":"stop"}],"usage":{"prompt_tokens":5,"completion_tokens":2}}"#);
    let p = OpenAiCompatible::new(
        "http://localhost:11434/v1/",
        Some(Secret::new("sk-test-123456")),
        t.clone(),
    );
    let r = p.chat(&request()).unwrap();
    assert_eq!(r.blocks, vec![Block::Text("hi".into())]);
    assert_eq!(r.usage, Usage { input: 5, output: 2 });
    let req = t.last_request().unwrap();
    assert_eq!(req.url, "http://localhost:11434/v1/chat/completions");
    assert_eq!(req.header("authorization"), Some("Bearer sk-test-123456"));
    let b = body(&t);
    assert_eq!(b["model"], "m1");
    assert_eq!(b["messages"][0], json!({"role": "system", "content": "be brief"}));
    assert_eq!(b["messages"][1]["role"], "user");
    // Assistant tool call: arguments are a JSON *string*.
    let call = &b["messages"][2]["tool_calls"][0];
    assert_eq!(call["type"], "function");
    assert_eq!(call["id"], "call_1");
    assert_eq!(call["function"]["name"], "set_setting");
    assert_eq!(
        call["function"]["arguments"],
        "{\"path\":\"physics.nu\",\"value\":0.02}"
    );
    // Tool result: role tool with tool_call_id.
    assert_eq!(
        b["messages"][3],
        json!({"role": "tool", "tool_call_id": "call_1", "content": "{\"ok\":true}"})
    );
    assert_eq!(b["tools"][0]["type"], "function");
    assert_eq!(b["tools"][0]["function"]["parameters"]["type"], "object");
    assert_eq!(b["max_tokens"], 256);
    assert!(b.get("stream").is_none());
}

#[test]
fn openai_keyless_local_endpoint_sends_no_auth_header() {
    let t = Arc::new(FakeTransport::new());
    t.push(
        200,
        r#"{"choices":[{"message":{"content":"x"},"finish_reason":"stop"}]}"#,
    );
    let p = OpenAiCompatible::new("http://localhost:1234/v1", None, t.clone());
    p.chat(&request()).unwrap();
    assert!(t.last_request().unwrap().header("authorization").is_none());
}

#[test]
fn openai_parses_tool_calls_and_survives_malformed_arguments() {
    let t = Arc::new(FakeTransport::new());
    t.push(
        200,
        json!({"choices": [{"finish_reason": "tool_calls", "message": {"content": null, "tool_calls": [
            {"id": "a", "type": "function", "function": {"name": "check_mesh", "arguments": "{}"}},
            {"id": "b", "type": "function", "function": {"name": "set_setting", "arguments": "{\"path\": \"x\", "}}
        ]}}]})
        .to_string(),
    );
    let p = OpenAiCompatible::new("http://h/v1", None, t);
    let r = p.chat(&request()).unwrap();
    assert_eq!(r.stop, StopReason::ToolUse);
    assert_eq!(
        r.blocks[0],
        Block::ToolUse {
            id: "a".into(),
            name: "check_mesh".into(),
            input: json!({})
        }
    );
    match &r.blocks[1] {
        Block::ToolUse { input, .. } => assert!(input.get(INVALID_ARGS_KEY).is_some(), "{input}"),
        b => panic!("{b:?}"),
    }
}

#[test]
fn openai_streaming_accumulates_text_and_fragmented_tool_calls() {
    let t = Arc::new(FakeTransport::new());
    let sse = [
        r#"data: {"choices":[{"index":0,"delta":{"role":"assistant","content":"Hel"},"finish_reason":null}]}"#,
        "",
        r#"data: {"choices":[{"index":0,"delta":{"content":"lo"},"finish_reason":null}]}"#,
        r#"data: {"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call_9","type":"function","function":{"name":"set_setting","arguments":""}}]},"finish_reason":null}]}"#,
        r#"data: {"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"path\":\"mesh"}}]},"finish_reason":null}]}"#,
        r#"data: {"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":".nx\",\"value\":8}"}}]},"finish_reason":null}]}"#,
        r#"data: {"choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}"#,
        "data: [DONE]",
    ]
    .join("\n");
    t.push(200, sse);
    let p = OpenAiCompatible::new("http://h/v1", None, t.clone());
    let mut events = vec![];
    let r = p.chat_stream(&request(), &mut |e| events.push(e)).unwrap();
    assert_eq!(body(&t)["stream"], true);
    assert_eq!(
        events,
        vec![
            StreamEvent::Text("Hel".into()),
            StreamEvent::Text("lo".into()),
            StreamEvent::ToolCall {
                name: "set_setting".into()
            }
        ]
    );
    assert_eq!(r.blocks[0], Block::Text("Hello".into()));
    assert_eq!(
        r.blocks[1],
        Block::ToolUse {
            id: "call_9".into(),
            name: "set_setting".into(),
            input: json!({"path": "mesh.nx", "value": 8})
        }
    );
    assert_eq!(r.stop, StopReason::ToolUse);
}

#[test]
fn truncated_streams_are_errors() {
    let t = Arc::new(FakeTransport::new());
    t.push(200, r#"data: {"choices":[{"index":0,"delta":{"content":"Hel"}}]}"#);
    let p = OpenAiCompatible::new("http://h/v1", None, t);
    assert!(matches!(
        p.chat_stream(&request(), &mut |_| {}),
        Err(ProviderError::Parse(_))
    ));
}

#[test]
fn http_errors_are_reported_without_the_key() {
    let key = "sk-super-secret-key-9999";
    let t = Arc::new(FakeTransport::new());
    // A server that echoes the credential back in its error body.
    t.push(
        401,
        format!(r#"{{"error":{{"message":"Incorrect API key provided: {key}"}}}}"#),
    );
    let p = OpenAiCompatible::new("http://h/v1", Some(Secret::new(key)), t);
    let e = p.chat(&request()).unwrap_err();
    let shown = format!("{e} / {e:?}");
    assert!(shown.contains("401"));
    assert!(!shown.contains(key), "{shown}");

    let t = Arc::new(FakeTransport::new());
    t.push(429, format!("rate limited for {key}"));
    let a = Anthropic::new("", Secret::new(key), t);
    let e = a.chat(&request()).unwrap_err();
    assert!(!format!("{e} {e:?}").contains(key));
}

#[test]
fn anthropic_request_shape() {
    let t = Arc::new(FakeTransport::new());
    t.push(
        200,
        json!({"id": "msg_1", "type": "message", "role": "assistant", "stop_reason": "tool_use",
               "content": [{"type": "text", "text": "Setting it."},
                           {"type": "tool_use", "id": "toolu_1", "name": "set_setting", "input": {"path": "physics.nu", "value": 0.02}}],
               "usage": {"input_tokens": 11, "output_tokens": 7}})
        .to_string(),
    );
    let p = Anthropic::new("", Secret::new("ak-test-0001"), t.clone());
    let r = p.chat(&request()).unwrap();
    assert_eq!(r.stop, StopReason::ToolUse);
    assert_eq!(r.usage, Usage { input: 11, output: 7 });
    assert_eq!(
        r.blocks[1],
        Block::ToolUse {
            id: "toolu_1".into(),
            name: "set_setting".into(),
            input: json!({"path": "physics.nu", "value": 0.02})
        }
    );
    let req = t.last_request().unwrap();
    assert_eq!(req.url, "https://api.anthropic.com/v1/messages");
    assert_eq!(req.header("x-api-key"), Some("ak-test-0001"));
    assert_eq!(req.header("anthropic-version"), Some("2023-06-01"));
    assert_eq!(req.header("content-type"), Some("application/json"));
    let b = body(&t);
    assert_eq!(b["system"], "be brief");
    assert_eq!(b["max_tokens"], 256);
    assert_eq!(b["tools"][0]["input_schema"]["type"], "object");
    assert!(b["tools"][0].get("parameters").is_none());
    assert_eq!(b["messages"][1]["content"][1]["type"], "tool_use");
    assert_eq!(b["messages"][1]["content"][1]["input"]["path"], "physics.nu");
    // Tool results are carried in a user turn with tool_use_id.
    assert_eq!(b["messages"][2]["role"], "user");
    assert_eq!(
        b["messages"][2]["content"][0],
        json!({"type": "tool_result", "tool_use_id": "call_1", "content": "{\"ok\":true}", "is_error": false})
    );
}

#[test]
fn anthropic_streaming_events() {
    let t = Arc::new(FakeTransport::new());
    let sse = [
        r#"event: message_start"#,
        r#"data: {"type":"message_start","message":{"id":"m","type":"message","role":"assistant","content":[],"usage":{"input_tokens":25,"output_tokens":1}}}"#,
        "",
        r#"event: content_block_start"#,
        r#"data: {"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}"#,
        r#"event: ping"#,
        r#"data: {"type":"ping"}"#,
        r#"data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Run"}}"#,
        r#"data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"ning."}}"#,
        r#"data: {"type":"content_block_stop","index":0}"#,
        r#"data: {"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"toolu_7","name":"run","input":{}}}"#,
        r#"data: {"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":""}}"#,
        r#"data: {"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"max_steps\": 2"}}"#,
        r#"data: {"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"0}"}}"#,
        r#"data: {"type":"content_block_stop","index":1}"#,
        r#"data: {"type":"message_delta","delta":{"stop_reason":"tool_use","stop_sequence":null},"usage":{"output_tokens":42}}"#,
        r#"data: {"type":"message_stop"}"#,
    ]
    .join("\n");
    t.push(200, sse);
    let p = Anthropic::new("https://example.invalid/", Secret::new("ak-test-0002"), t.clone());
    let mut events = vec![];
    let r = p.chat_stream(&request(), &mut |e| events.push(e)).unwrap();
    assert_eq!(t.last_request().unwrap().url, "https://example.invalid/v1/messages");
    assert_eq!(body(&t)["stream"], true);
    assert_eq!(events.len(), 3);
    assert_eq!(r.blocks[0], Block::Text("Running.".into()));
    assert_eq!(
        r.blocks[1],
        Block::ToolUse {
            id: "toolu_7".into(),
            name: "run".into(),
            input: json!({"max_steps": 20})
        }
    );
    assert_eq!(r.stop, StopReason::ToolUse);
    assert_eq!(r.usage, Usage { input: 25, output: 42 });
}

// ---- keys and config ----

#[test]
fn secret_debug_is_redacted_and_config_has_no_key() {
    let s = Secret::new("sk-should-never-print");
    assert!(!format!("{s:?}").contains("sk-should"));
    let cfg = ProviderConfig {
        name: "openai".into(),
        kind: ProviderKind::OpenAiCompatible,
        base_url: "https://api.openai.com/v1".into(),
        model: "m".into(),
        key_env: Some("RCFD_TEST_KEY_VAR".into()),
        max_tokens: 100,
        timeout_secs: 10,
        local: false,
    };
    let store = MemoryKeyStore::new();
    store.set("openai", &s).unwrap();
    let text = serde_json::to_string(&AssistantConfig {
        providers: vec![cfg.clone()],
        active: Some("openai".into()),
    })
    .unwrap();
    assert!(!text.contains("sk-should"));
    // Providers built with a key never show it in Debug-able config.
    assert!(!format!("{cfg:?} {:?}", store.get("openai")).contains("sk-should"));
    assert_eq!(resolve_key(&cfg, &store).unwrap().expose(), "sk-should-never-print");
}

#[test]
fn env_var_takes_precedence_over_the_store() {
    let mut cfg = ProviderConfig::presets()
        .into_iter()
        .find(|p| p.name == "openai")
        .unwrap();
    cfg.key_env = Some("RCFD_TEST_PRECEDENCE_KEY".into());
    let store = MemoryKeyStore::new();
    store.set("openai", &Secret::new("from-store")).unwrap();
    assert_eq!(resolve_key(&cfg, &store).unwrap().expose(), "from-store");
    std::env::set_var("RCFD_TEST_PRECEDENCE_KEY", "from-env");
    assert_eq!(resolve_key(&cfg, &store).unwrap().expose(), "from-env");
    std::env::remove_var("RCFD_TEST_PRECEDENCE_KEY");
}

#[test]
fn file_key_store_and_config_round_trip_in_a_temp_dir() {
    let dir = std::env::temp_dir().join(format!("rcfd-assistant-keys-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let store = FileKeyStore::new(dir.join("keys.json"));
    assert!(store.get("anthropic").is_none());
    store.set("anthropic", &Secret::new("ak-file-key")).unwrap();
    store.set("openai", &Secret::new("sk-other")).unwrap();
    assert_eq!(store.get("anthropic").unwrap().expose(), "ak-file-key");
    store.delete("openai").unwrap();
    assert!(store.get("openai").is_none());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(dir.join("keys.json")).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
    let mut cfg = AssistantConfig::load(&dir.join("providers.json")).unwrap();
    assert!(cfg.providers.iter().any(|p| p.name == "ollama" && p.local));
    cfg.active = Some("ollama".into());
    cfg.save(&dir.join("providers.json")).unwrap();
    let back = AssistantConfig::load(&dir.join("providers.json")).unwrap();
    assert_eq!(back, cfg);
    assert_eq!(back.active_provider().unwrap().base_url, "http://localhost:11434/v1");
    let on_disk = std::fs::read_to_string(dir.join("providers.json")).unwrap();
    assert!(!on_disk.contains("ak-file-key"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn build_provider_requires_a_key_for_anthropic_only() {
    let store = MemoryKeyStore::new();
    let presets = ProviderConfig::presets();
    let get = |n: &str| presets.iter().find(|p| p.name == n).unwrap().clone();
    assert!(matches!(
        build_provider(&get("anthropic"), &store, None),
        Err(ProviderError::Config(_))
    ));
    assert!(build_provider(&get("ollama"), &store, None).is_ok());
    store.set("anthropic", &Secret::new("ak-1234")).unwrap();
    assert_eq!(
        build_provider(&get("anthropic"), &store, None).unwrap().name(),
        "anthropic"
    );
}

#[test]
fn local_preset_builds_a_working_request_and_lists_models() {
    let store = MemoryKeyStore::new();
    let mut cfg = ProviderConfig::presets()
        .into_iter()
        .find(|p| p.name == "ollama")
        .unwrap();
    cfg.model = "llama3.2".into();
    let t = Arc::new(FakeTransport::new());
    t.push(
        200,
        r#"{"choices":[{"message":{"content":"ok"},"finish_reason":"stop"}]}"#,
    );
    let p = build_provider(&cfg, &store, Some(t.clone() as Arc<dyn Transport>)).unwrap();
    let mut req = request();
    req.model = cfg.model.clone();
    p.chat(&req).unwrap();
    assert_eq!(
        t.last_request().unwrap().url,
        "http://localhost:11434/v1/chat/completions"
    );
    let t = Arc::new(FakeTransport::new());
    t.push(200, r#"{"object":"list","data":[{"id":"llama3.2"},{"id":"qwen2.5"}]}"#);
    let oa = OpenAiCompatible::new(&cfg.base_url, None, t.clone());
    assert_eq!(oa.list_models().unwrap(), ["llama3.2", "qwen2.5"]);
    assert_eq!(t.last_request().unwrap().url, "http://localhost:11434/v1/models");
}
