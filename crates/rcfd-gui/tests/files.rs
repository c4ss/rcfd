//! Headless tests of the file-dialog logic (a scripted chooser stands in for the native dialog) and of
//! key storage as seen from the sidebar setup. The native dialog itself is not exercised here.

use rcfd_assistant::config::{FileKeyStore, KeyStore, ProviderConfig};
use rcfd_assistant::keychain::{LayeredKeyStore, MemoryBackend};
use rcfd_gui::assistant_setup::AssistantSetup;
use rcfd_gui::controller::Controller;
use rcfd_gui::files::*;
use std::cell::RefCell;
use std::path::{Path, PathBuf};

/// Answers dialogs from a script and records what it was asked.
struct Scripted {
    open: Option<PathBuf>,
    save: Option<PathBuf>,
    asked: RefCell<Vec<(Option<PathBuf>, String)>>,
}

impl Scripted {
    fn new(open: Option<PathBuf>, save: Option<PathBuf>) -> Self {
        Scripted {
            open,
            save,
            asked: RefCell::new(vec![]),
        }
    }
}

impl FileChooser for Scripted {
    fn pick_open(&self, start_dir: Option<&Path>) -> Option<PathBuf> {
        self.asked
            .borrow_mut()
            .push((start_dir.map(Path::to_path_buf), "open".into()));
        self.open.clone()
    }
    fn pick_save(&self, start_dir: Option<&Path>, suggested_name: &str) -> Option<PathBuf> {
        self.asked
            .borrow_mut()
            .push((start_dir.map(Path::to_path_buf), format!("save:{suggested_name}")));
        self.save.clone()
    }
}

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd-gui-files-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn ctrl() -> Controller {
    let mut c = Controller::new(None, std::env::temp_dir().join("rcfd-gui-files-cases"));
    c.new_from_template().unwrap();
    c
}

#[test]
fn extension_rules() {
    assert_eq!(with_case_extension("a/case".into()), PathBuf::from("a/case.toml"));
    assert_eq!(with_case_extension("a/case.toml".into()), PathBuf::from("a/case.toml"));
    assert_eq!(with_case_extension("a/case.JSON".into()), PathBuf::from("a/case.JSON"));
    assert_eq!(
        with_case_extension("a/case.txt".into()),
        PathBuf::from("a/case.txt.toml")
    );
    assert_eq!(with_case_extension("a/v1.2".into()), PathBuf::from("a/v1.2.toml"));
}

#[test]
fn start_folder_and_suggested_name() {
    let d = tmp("start");
    let f = d.join("mycase.toml");
    assert_eq!(start_dir(Some(&f), None), Some(d.clone()));
    // A file in a folder that no longer exists falls back.
    assert_eq!(
        start_dir(Some(&d.join("gone").join("x.toml")), Some(&d)),
        Some(d.clone())
    );
    assert_eq!(start_dir(None, Some(&d.join("missing"))), None);
    assert_eq!(start_dir(Some(Path::new("x.toml")), None), None);
    assert_eq!(suggested_name(Some(&f)), "mycase.toml");
    assert_eq!(suggested_name(None), "case.toml");
}

#[test]
fn save_adds_an_extension_and_open_reads_it_back() {
    let d = tmp("roundtrip");
    let mut c = ctrl();
    assert!(c.edit_text("physics.nu", "0.0025"));
    let chooser = Scripted::new(None, Some(d.join("re400")));
    let out = save_as_with_dialog(&mut c, &chooser, Some(&d));
    let saved = d.join("re400.toml");
    assert_eq!(out, Outcome::Done(saved.clone()));
    assert!(saved.is_file());
    assert_eq!(c.file.as_deref(), Some(saved.as_path()));
    assert_eq!(
        chooser.asked.borrow()[0],
        (Some(d.clone()), "save:case.toml".to_string())
    );

    let mut fresh = ctrl();
    let chooser = Scripted::new(Some(saved.clone()), None);
    assert_eq!(open_with_dialog(&mut fresh, &chooser, Some(&d)), Outcome::Done(saved));
    assert_eq!(fresh.settings["physics"]["nu"], 0.0025);
    assert!(!fresh.dirty);
}

#[test]
fn cancel_changes_nothing() {
    let d = tmp("cancel");
    let mut c = ctrl();
    let before = (c.session.clone(), c.file.clone(), c.settings.clone());
    let chooser = Scripted::new(None, None);
    assert_eq!(open_with_dialog(&mut c, &chooser, Some(&d)), Outcome::Cancelled);
    assert_eq!(save_as_with_dialog(&mut c, &chooser, Some(&d)), Outcome::Cancelled);
    assert_eq!(before, (c.session.clone(), c.file.clone(), c.settings.clone()));
    assert_eq!(std::fs::read_dir(&d).unwrap().count(), 0, "nothing written");
}

#[test]
fn a_bad_file_is_reported_and_keeps_the_open_case() {
    let d = tmp("bad");
    let bad = d.join("broken.toml");
    std::fs::write(&bad, "this is = = not toml").unwrap();
    let mut c = ctrl();
    let session = c.session.clone();
    let chooser = Scripted::new(Some(bad.clone()), Some(d.join("no-such-dir").join("x.toml")));
    match open_with_dialog(&mut c, &chooser, None) {
        Outcome::Failed(m) => assert!(!m.is_empty()),
        other => panic!("{other:?}"),
    }
    assert_eq!(c.session, session, "the previous case stays open");
    match save_as_with_dialog(&mut c, &chooser, None) {
        Outcome::Failed(m) => assert!(m.contains("x.toml"), "{m}"),
        other => panic!("{other:?}"),
    }
    // A missing file is an error, not a crash.
    let chooser = Scripted::new(Some(d.join("absent.toml")), None);
    assert!(matches!(open_with_dialog(&mut c, &chooser, None), Outcome::Failed(_)));
}

#[test]
fn sidebar_stores_keys_in_the_keychain_backend_and_reports_where() {
    let d = tmp("keys");
    let be = std::sync::Arc::new(MemoryBackend::new());
    struct Shared(std::sync::Arc<MemoryBackend>);
    impl rcfd_assistant::keychain::SecretBackend for Shared {
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
    let store = LayeredKeyStore::new(
        Box::new(Shared(be.clone())),
        Box::new(FileKeyStore::new(d.join("keys.json"))),
    );
    let mut s = AssistantSetup::load_with_store(d.clone(), Box::new(store));
    let name = ProviderConfig::presets()
        .into_iter()
        .find(|p| p.name == "anthropic")
        .unwrap()
        .name;
    s.select(&name);
    assert!(!s.has_key());
    s.key_input = "ak-gui-test-7777".into();
    s.save_key();
    assert!(s.key_input.is_empty(), "the input box is cleared");
    let status = s.status.clone().unwrap();
    assert!(status.contains("test keychain"), "{status}");
    assert!(!status.contains("ak-gui-test-7777"));
    assert!(s.has_key());
    assert_eq!(be.len(), 1);
    assert!(!d.join("keys.json").exists(), "no plaintext copy");
    assert!(s.key_storage_note().contains("test keychain"));
    s.delete_key();
    assert!(!s.has_key());
    assert!(be.is_empty());
}

#[test]
fn load_from_never_uses_the_os_keychain() {
    let d = tmp("filekeys");
    let mut s = AssistantSetup::load_from(d.clone());
    s.select("anthropic");
    s.key_input = "ak-file-only-8888".into();
    s.save_key();
    assert!(std::fs::read_to_string(d.join("keys.json"))
        .unwrap()
        .contains("ak-file-only-8888"));
    let _ = FileKeyStore::new(d.join("keys.json")).delete("anthropic");
}
