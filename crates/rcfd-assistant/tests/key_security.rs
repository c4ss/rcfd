//! ADR 0025 checks K1-K3 (F-149): precedence env > keychain > file, planted-canary redaction scan, file store.
//! The keychain is the in-memory backend; the real OS keychain is never touched.

mod common;

use common::{Mock, Reply};
use rcfd_assistant::config::*;
use rcfd_assistant::keychain::*;
use rcfd_assistant::*;
use serde_json::json;
use std::path::PathBuf;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::Arc;

const CANARY: &str = "rcfd-canary-Qx7Lm2Vp9RtZ4BnK8wYc";

struct Shared(Arc<MemoryBackend>);
impl SecretBackend for Shared {
    fn describe(&self) -> String {
        self.0.describe()
    }
    fn get(&self, s: &str, u: &str) -> Result<Option<String>, String> {
        self.0.get(s, u)
    }
    fn set(&self, s: &str, u: &str, v: &str) -> Result<(), String> {
        self.0.set(s, u, v)
    }
    fn delete(&self, s: &str, u: &str) -> Result<(), String> {
        self.0.delete(s, u)
    }
}

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd-w17c-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn req() -> ChatRequest {
    ChatRequest {
        model: "m".into(),
        system: "s".into(),
        messages: vec![Message::user("hi")],
        tools: vec![],
        max_tokens: 16,
        temperature: None,
    }
}

#[test]
fn k1_precedence_env_then_keychain_then_file_then_nothing() {
    let dir = tmp("k1");
    let be = Arc::new(MemoryBackend::new());
    let file = FileKeyStore::new(dir.join("keys.json"));
    file.set("openai", &Secret::new("file-value")).unwrap();
    let store = LayeredKeyStore::new(
        Box::new(Shared(be.clone())),
        Box::new(FileKeyStore::new(dir.join("keys.json"))),
    );
    be.set(SERVICE, "openai", "keychain-value").unwrap();
    let var = "RCFD_W17C_K1_KEY";
    let mut cfg = ProviderConfig::presets()
        .into_iter()
        .find(|p| p.name == "openai")
        .unwrap();
    cfg.key_env = Some(var.into());

    std::env::set_var(var, "env-value");
    let s1 = resolve_key(&cfg, &store).map(|k| k.expose().to_string());
    std::env::remove_var(var);
    let s2 = resolve_key(&cfg, &store).map(|k| k.expose().to_string());
    be.unavailable.store(true, SeqCst);
    let s3 = resolve_key(&cfg, &store).map(|k| k.expose().to_string());
    file.delete("openai").unwrap();
    let s4 = resolve_key(&cfg, &store).map(|k| k.expose().to_string());
    assert_eq!(s1.as_deref(), Some("env-value"));
    assert_eq!(s2.as_deref(), Some("keychain-value"));
    assert_eq!(s3.as_deref(), Some("file-value"));
    assert_eq!(s4, None);
}

/// Collected outputs: (label, text). A leak is the canary in any of them.
struct Sink(Vec<(String, String)>);
impl Sink {
    fn add(&mut self, label: &str, text: impl Into<String>) {
        self.0.push((label.into(), text.into()));
    }
    fn leaks(&self, canary: &str) -> Vec<String> {
        self.0
            .iter()
            .filter(|(_, t)| t.contains(canary))
            .map(|(l, _)| l.clone())
            .collect()
    }
}

#[test]
fn k2_scanner_self_test_flags_an_unscrubbed_string() {
    let mut s = Sink(vec![]);
    s.add("clean", "nothing here");
    s.add("dirty", format!("error: bad key {CANARY}"));
    assert_eq!(s.leaks(CANARY), ["dirty"]);
}

#[test]
fn k2_planted_canary_never_appears_in_any_output() {
    let dir = tmp("k2");
    let mut sink = Sink(vec![]);
    let secret = Secret::new(CANARY);
    sink.add("secret debug", format!("{secret:?}"));

    // Key in the file store; provider config on disk.
    let keys_path = dir.join("keys.json");
    let be = Arc::new(MemoryBackend::new());
    be.unavailable.store(true, SeqCst);
    be.echo_value_in_errors.store(true, SeqCst);
    let store = LayeredKeyStore::new(Box::new(Shared(be.clone())), Box::new(FileKeyStore::new(&keys_path)));
    store.set("ollama", &secret).unwrap(); // keychain fails with the value echoed; falls back to the file
    sink.add("store location", store.location());

    // Fallback also failing: the file's parent is a regular file.
    std::fs::write(dir.join("blocker"), b"x").unwrap();
    let bad = LayeredKeyStore::new(
        Box::new(Shared(be.clone())),
        Box::new(FileKeyStore::new(dir.join("blocker").join("keys.json"))),
    );
    let e = bad.set("ollama", &secret).unwrap_err();
    sink.add("both stores failed", e);

    let mut cfg = ProviderConfig::presets()
        .into_iter()
        .find(|p| p.name == "ollama")
        .unwrap();
    cfg.model = "m".into();
    let mut ac = AssistantConfig {
        providers: vec![cfg.clone()],
        active: Some("ollama".into()),
    };
    ac.upsert(cfg.clone());
    sink.add("provider config debug", format!("{cfg:?}"));
    sink.add("assistant config debug", format!("{ac:?}"));
    let pj = dir.join("providers.json");
    ac.save(&pj).unwrap();
    sink.add("providers.json", std::fs::read_to_string(&pj).unwrap());
    sink.add("config json", serde_json::to_string(&ac).unwrap());

    // The mock answers every request with an error that echoes the key in different ways.
    let mock = Mock::start();
    let url = format!("http://127.0.0.1:{}/v1", mock.port);
    cfg.base_url = url.clone();
    cfg.timeout_secs = 10;
    for body in [
        format!("{{\"error\":{{\"message\":\"Incorrect API key provided: {CANARY}\"}}}}"),
        format!("rate limited for {CANARY} please retry"),
        format!("{CANARY}"),
        format!("{{\"echo\":\"Bearer {CANARY}\",\"detail\":\"{CANARY}{CANARY}\"}}"),
    ] {
        for _ in 0..4 {
            mock.push(Reply::Json(401, body.clone()));
        }
    }
    let p = build_provider(&cfg, &store, None).unwrap();
    for i in 0..4 {
        let e = p.chat(&req()).unwrap_err();
        sink.add(&format!("chat error {i}"), format!("{e} | {e:?}"));
        let e = p
            .chat_stream(&req(), &mut |ev| {
                sink.0.push(("stream event".into(), format!("{ev:?}")))
            })
            .unwrap_err();
        sink.add(&format!("stream error {i}"), format!("{e} | {e:?}"));
        let oa = rcfd_assistant::openai::OpenAiCompatible::new(
            &url,
            Some(secret.clone()),
            Arc::new(rcfd_assistant::http::UreqTransport::new(5)),
        );
        let e = oa.list_models().unwrap_err();
        sink.add(&format!("list_models error {i}"), format!("{e} | {e:?}"));
        let an = rcfd_assistant::anthropic::Anthropic::new(
            &url,
            secret.clone(),
            Arc::new(rcfd_assistant::http::UreqTransport::new(5)),
        );
        let e = an.chat(&req()).unwrap_err();
        sink.add(&format!("anthropic error {i}"), format!("{e} | {e:?}"));
    }
    let sent = mock.requests();
    assert!(sent.len() >= 16, "requests reached the mock: {}", sent.len());
    for r in &sent {
        sink.add("request url/headers/body (auth excluded)", r.everything_but_auth());
    }

    let leaks = sink.leaks(CANARY);
    assert!(leaks.is_empty(), "canary found in: {leaks:?}");
    // Non-vacuous: the canary was really sent (in the auth header) and really stored (in keys.json only).
    assert!(
        sent.iter()
            .any(|r| r.header("authorization") == Some(format!("Bearer {CANARY}").as_str())),
        "canary never reached the server in an Authorization header"
    );
    assert!(
        sent.iter().any(|r| r.header("x-api-key") == Some(CANARY)),
        "anthropic key header"
    );
    assert!(std::fs::read_to_string(&keys_path).unwrap().contains(CANARY));
    assert!(!std::fs::read_to_string(&pj).unwrap().contains(CANARY));
}

#[test]
fn k3_file_store_round_trip_and_contents() {
    let dir = tmp("k3");
    let f = FileKeyStore::new(dir.join("sub").join("keys.json"));
    assert!(f.get("a").is_none());
    f.set("a", &Secret::new("key-a-1")).unwrap();
    f.set("b", &Secret::new("key-b-2")).unwrap();
    assert_eq!(f.get("a").unwrap().expose(), "key-a-1");
    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("sub").join("keys.json")).unwrap()).unwrap();
    assert_eq!(v, json!({"a": "key-a-1", "b": "key-b-2"}));
    f.delete("a").unwrap();
    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("sub").join("keys.json")).unwrap()).unwrap();
    assert_eq!(v, json!({"b": "key-b-2"}));
    assert!(f.get("a").is_none());
    // Unix owner-only mode is cfg(unix) code that this Windows run cannot exercise.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let m = std::fs::metadata(dir.join("sub").join("keys.json"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(m & 0o777, 0o600);
    }
}
