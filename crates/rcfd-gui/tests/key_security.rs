//! ADR 0025 K2 (GUI side) and L2/L6 through `AssistantSetup`: the planted canary is typed into the key box and must not
//! appear in the status line, the chat transcript, the saved settings, the saved preferences or anything the server sees
//! except its credential header. Loopback mock server; the OS keychain is not used.

#[path = "../../rcfd-assistant/tests/common/mod.rs"]
mod common;

use common::{Mock, Reply};
use rcfd_assistant::config::{FileKeyStore, ProviderKind};
use rcfd_assistant::keychain::{LayeredKeyStore, MemoryBackend, SecretBackend};
use rcfd_gui::assistant_setup::AssistantSetup;
use rcfd_gui::controller::Controller;
use rcfd_gui::state::{AppState, QuickAction};
use rcfd_gui::theme::Theme;
use rcfd_gui::RcfdApp;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::Arc;
use std::time::{Duration, Instant};

const CANARY: &str = "rcfd-canary-GUI-7Hn3Kd0Wq5Zs1Vb8Xe";

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

fn tmp(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd-w17c-gui-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn frame(app: &mut RcfdApp, ctx: &egui::Context) {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 800.0))),
        ..Default::default()
    };
    let _ = ctx.run(input, |ctx| app.ui(ctx));
}

fn setup_for(dir: &std::path::Path, mock: &Mock, keychain_fails: bool) -> AssistantSetup {
    let be = Arc::new(MemoryBackend::new());
    if keychain_fails {
        be.unavailable.store(true, SeqCst);
        be.echo_value_in_errors.store(true, SeqCst);
    }
    let store = LayeredKeyStore::new(Box::new(Shared(be)), Box::new(FileKeyStore::new(dir.join("keys.json"))));
    let mut s = AssistantSetup::load_with_store(dir.to_path_buf(), Box::new(store));
    s.select("ollama");
    let c = s.active_mut().unwrap();
    assert_eq!(c.kind, ProviderKind::OpenAiCompatible);
    c.base_url = format!("http://127.0.0.1:{}/v1", mock.port);
    c.model = "test-model".into();
    c.timeout_secs = 10;
    s
}

#[test]
fn gui_flow_never_shows_or_saves_the_key_outside_the_key_store() {
    for keychain_fails in [false, true] {
        let dir = tmp(if keychain_fails { "fail" } else { "ok" });
        let mock = Mock::start();
        for _ in 0..3 {
            mock.push(Reply::Json(
                401,
                format!("{{\"error\":{{\"message\":\"Incorrect API key provided: {CANARY}\"}}}}"),
            ));
        }
        let mut setup = setup_for(&dir, &mock, keychain_fails);
        let mut seen: Vec<(String, String)> = vec![];
        setup.key_input = CANARY.into();
        setup.save_key();
        assert!(setup.key_input.is_empty());
        seen.push(("status after save_key".into(), setup.status.clone().unwrap_or_default()));
        seen.push(("key_storage_note".into(), setup.key_storage_note()));
        setup.save();
        seen.push((
            "providers.json".into(),
            std::fs::read_to_string(dir.join("providers.json")).unwrap(),
        ));
        assert!(setup.has_key());
        seen.push(("config debug".into(), format!("{:?}", setup.config)));

        // Model probe: the server answers 401 and echoes the key.
        let e = setup.probe(None).unwrap_err();
        seen.push(("probe error".into(), format!("{e} {e:?}")));

        // A full assistant turn that fails at the provider.
        let prefs = dir.join("prefs.json");
        let cdir = tmp("cases");
        let mut ctrl = Controller::new(None, cdir);
        ctrl.new_from_template().unwrap();
        let mut st = AppState::new(ctrl, setup, Theme::Light);
        st.connect_selected();
        seen.push((
            "status after connect".into(),
            st.setup.status.clone().unwrap_or_default(),
        ));
        st.prefs_path = Some(prefs.clone());
        let mut app = RcfdApp::new(st);
        let ctx = egui::Context::default();
        frame(&mut app, &ctx);
        app.st.chat.input = "hello".into();
        app.st.send_message();
        let deadline = Instant::now() + Duration::from_secs(30);
        while app.st.chat.busy && Instant::now() < deadline {
            frame(&mut app, &ctx);
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(!app.st.chat.busy, "turn ended");
        for _ in 0..3 {
            frame(&mut app, &ctx);
        }
        seen.push(("chat entries".into(), format!("{:?}", app.st.chat.entries)));
        seen.push(("setup status".into(), app.st.setup.status.clone().unwrap_or_default()));
        if prefs.exists() {
            seen.push(("prefs file".into(), std::fs::read_to_string(&prefs).unwrap()));
        }
        let reqs = mock.requests();
        assert!(!reqs.is_empty());
        for r in &reqs {
            seen.push(("request (auth excluded)".into(), r.everything_but_auth()));
        }
        let leaks: Vec<&str> = seen
            .iter()
            .filter(|(_, t)| t.contains(CANARY))
            .map(|(l, _)| l.as_str())
            .collect();
        assert!(leaks.is_empty(), "keychain_fails={keychain_fails}: canary in {leaks:?}");
        assert!(
            reqs.iter()
                .any(|r| r.header("authorization") == Some(format!("Bearer {CANARY}").as_str())),
            "the key was never sent: the scan would be vacuous"
        );
        assert!(
            seen.iter().any(|(l, t)| l == "chat entries" && t.contains("401")),
            "the provider error should be visible in the chat: {:?}",
            seen.iter().find(|(l, _)| l == "chat entries")
        );
    }
}

#[test]
fn quick_actions_open_no_connection_even_when_a_local_endpoint_is_configured() {
    let dir = tmp("quick");
    let mock = Mock::start();
    let setup = setup_for(&dir, &mock, false);
    let cdir = tmp("quick-cases");
    let mut ctrl = Controller::new(None, cdir);
    ctrl.new_from_template().unwrap();
    let mut st = AppState::new(ctrl, setup, Theme::Light);
    let mut done = 0;
    for q in QuickAction::ALL {
        let before = st.chat.entries.len();
        st.quick_action(q);
        assert!(st.chat.entries.len() > before, "{q:?} produced nothing");
        done += 1;
    }
    assert_eq!(done, 4);
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(mock.accepts.load(SeqCst), 0, "a quick action touched the network");
    assert!(mock.requests().is_empty());
}

#[test]
fn probe_through_the_setup_lists_models_from_the_mock() {
    let dir = tmp("probe");
    let mock = Mock::start();
    mock.push(Reply::Json(200, r#"{"data":[{"id":"a"},{"id":"b"}]}"#.into()));
    let setup = setup_for(&dir, &mock, false);
    assert_eq!(setup.probe(None).unwrap(), ["a", "b"]);
    let r = &mock.requests()[0];
    assert_eq!((r.method.as_str(), r.path.as_str()), ("GET", "/v1/models"));
}
