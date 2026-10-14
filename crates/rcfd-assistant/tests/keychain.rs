//! F-149: OS keychain storage with the plaintext file as fallback, tested with an in-memory backend.
//! Nothing here touches the real keychain of the machine running the tests.

use rcfd_assistant::config::*;
use rcfd_assistant::keychain::*;
use rcfd_assistant::Secret;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::Arc;

/// A backend shared between the store under test and the assertions.
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

fn rig(tag: &str) -> (LayeredKeyStore, Arc<MemoryBackend>, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("rcfd-keychain-test-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let file = dir.join("keys.json");
    let be = Arc::new(MemoryBackend::new());
    let store = LayeredKeyStore::new(Box::new(Shared(be.clone())), Box::new(FileKeyStore::new(&file)));
    (store, be, file)
}

#[test]
fn keys_go_to_the_keychain_and_not_to_the_file() {
    let (store, be, file) = rig("primary");
    store.set("openai", &Secret::new("sk-keychain-1111")).unwrap();
    assert_eq!(be.len(), 1);
    assert!(!file.exists(), "no plaintext file when the keychain works");
    assert_eq!(store.get("openai").unwrap().expose(), "sk-keychain-1111");
    assert!(store.location().contains("test keychain"), "{}", store.location());
    store.delete("openai").unwrap();
    assert!(store.get("openai").is_none());
    assert!(be.is_empty());
}

#[test]
fn unavailable_keychain_falls_back_to_the_file() {
    let (store, be, file) = rig("fallback");
    be.unavailable.store(true, SeqCst);
    store.set("anthropic", &Secret::new("ak-fallback-2222")).unwrap();
    assert!(file.exists());
    assert!(std::fs::read_to_string(&file).unwrap().contains("ak-fallback-2222"));
    assert_eq!(store.get("anthropic").unwrap().expose(), "ak-fallback-2222");
    assert!(store.location().contains("plaintext key file"), "{}", store.location());
    assert!(
        store.location().contains("keychain unavailable"),
        "{}",
        store.location()
    );
    store.delete("anthropic").unwrap();
    assert!(store.get("anthropic").is_none());
}

#[test]
fn a_backend_that_forgets_writes_is_detected() {
    let (store, be, file) = rig("forgetful");
    be.forgetful.store(true, SeqCst);
    store.set("p", &Secret::new("sk-forgotten-3333")).unwrap();
    // The write "succeeded" but the read-back did not match, so the file holds the key.
    assert!(std::fs::read_to_string(&file).unwrap().contains("sk-forgotten-3333"));
    assert_eq!(store.get("p").unwrap().expose(), "sk-forgotten-3333");
}

#[test]
fn existing_plaintext_keys_are_still_found_and_a_keychain_write_removes_them() {
    let (store, _be, file) = rig("migrate");
    FileKeyStore::new(&file)
        .set("old", &Secret::new("sk-old-plaintext-4444"))
        .unwrap();
    assert_eq!(store.get("old").unwrap().expose(), "sk-old-plaintext-4444");
    store.set("old", &Secret::new("sk-new-5555")).unwrap();
    assert_eq!(store.get("old").unwrap().expose(), "sk-new-5555");
    assert!(
        !std::fs::read_to_string(&file)
            .unwrap()
            .contains("sk-old-plaintext-4444"),
        "plaintext copy must be removed once the keychain holds the key"
    );
}

#[test]
fn keychain_failure_text_never_contains_the_key() {
    let (store, be, file) = rig("scrub");
    be.unavailable.store(true, SeqCst);
    be.echo_value_in_errors.store(true, SeqCst);
    // Make the fallback fail too: its parent "directory" is a file.
    let dir = file.parent().unwrap().to_path_buf();
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("blocker"), "x").unwrap();
    let bad = LayeredKeyStore::new(
        Box::new(Shared(be.clone())),
        Box::new(FileKeyStore::new(dir.join("blocker").join("keys.json"))),
    );
    let e = bad.set("p", &Secret::new("sk-must-not-leak-6666")).unwrap_err();
    assert!(!e.contains("sk-must-not-leak-6666"), "{e}");
    assert!(e.contains("***") || e.contains("keychain"), "{e}");
    drop(store);
}

#[test]
fn environment_variable_still_wins_over_the_keychain() {
    let (store, _be, _file) = rig("env");
    store.set("envprov", &Secret::new("from-keychain")).unwrap();
    let mut cfg = ProviderConfig::presets().remove(0);
    cfg.name = "envprov".into();
    cfg.key_env = Some("RCFD_KEYCHAIN_TEST_ENV_F094".into());
    assert_eq!(resolve_key(&cfg, &store).unwrap().expose(), "from-keychain");
    std::env::set_var("RCFD_KEYCHAIN_TEST_ENV_F094", "from-env");
    assert_eq!(resolve_key(&cfg, &store).unwrap().expose(), "from-env");
    std::env::remove_var("RCFD_KEYCHAIN_TEST_ENV_F094");
}

#[test]
fn the_real_keychain_type_is_only_offered_where_a_backend_exists() {
    // Constructing it does not touch the keychain.
    assert_eq!(OsKeychain::new().is_some(), cfg!(any(windows, target_os = "macos")));
}
