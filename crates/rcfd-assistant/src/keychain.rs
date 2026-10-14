//! OS keychain storage for API keys (F-149).
//!
//! [`LayeredKeyStore`] keeps keys in a [`SecretBackend`] (the OS keychain) and falls back to another
//! [`KeyStore`] (the plaintext `keys.json`) when the backend is missing or fails. Writes are verified by
//! reading the key back, so a backend that accepts a write but does not persist it (for example a
//! `keyring` build without a platform feature) is detected and the fallback is used instead. Keys are
//! scrubbed from every error text produced here.
//!
//! The environment variable still takes precedence over any store; that is done by
//! [`crate::config::resolve_key`], which asks the store only after the variable.

use crate::config::KeyStore;
use crate::types::Secret;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering::SeqCst};
use std::sync::Mutex;

/// Service name under which keys are filed in the OS keychain.
pub const SERVICE: &str = "rcfd-assistant";

/// A secret store addressed by (service, user), such as the OS keychain.
pub trait SecretBackend: Send + Sync {
    /// Short name for status lines, e.g. `the Windows Credential Manager`.
    fn describe(&self) -> String;
    /// The stored value; `Ok(None)` when there is none.
    fn get(&self, service: &str, user: &str) -> Result<Option<String>, String>;
    /// Store a value.
    fn set(&self, service: &str, user: &str, value: &str) -> Result<(), String>;
    /// Remove a value; removing a missing one is not an error.
    fn delete(&self, service: &str, user: &str) -> Result<(), String>;
}

/// The operating system's keychain via the `keyring` crate, where one is built in
/// (Windows Credential Manager, macOS Keychain).
pub struct OsKeychain(());

impl OsKeychain {
    /// The OS keychain, or `None` on platforms without a built-in backend (the caller then uses the file).
    pub fn new() -> Option<OsKeychain> {
        if cfg!(any(windows, target_os = "macos")) {
            Some(OsKeychain(()))
        } else {
            None
        }
    }
}

#[cfg(any(windows, target_os = "macos"))]
impl SecretBackend for OsKeychain {
    fn describe(&self) -> String {
        if cfg!(windows) {
            "the Windows Credential Manager".into()
        } else {
            "the macOS Keychain".into()
        }
    }

    fn get(&self, service: &str, user: &str) -> Result<Option<String>, String> {
        let e = keyring::Entry::new(service, user).map_err(|e| e.to_string())?;
        match e.get_password() {
            Ok(p) => Ok(Some(p)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    fn set(&self, service: &str, user: &str, value: &str) -> Result<(), String> {
        keyring::Entry::new(service, user)
            .and_then(|e| e.set_password(value))
            .map_err(|e| e.to_string())
    }

    fn delete(&self, service: &str, user: &str) -> Result<(), String> {
        match keyring::Entry::new(service, user).and_then(|e| e.delete_credential()) {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
impl SecretBackend for OsKeychain {
    fn describe(&self) -> String {
        "no keychain".into()
    }
    fn get(&self, _: &str, _: &str) -> Result<Option<String>, String> {
        Err("no OS keychain backend on this platform".into())
    }
    fn set(&self, _: &str, _: &str, _: &str) -> Result<(), String> {
        Err("no OS keychain backend on this platform".into())
    }
    fn delete(&self, _: &str, _: &str) -> Result<(), String> {
        Err("no OS keychain backend on this platform".into())
    }
}

/// In-memory [`SecretBackend`] for tests; never touches the real keychain. It can be told to fail or to
/// accept writes without keeping them.
#[derive(Default)]
pub struct MemoryBackend {
    items: Mutex<BTreeMap<(String, String), String>>,
    /// When true every call fails (a locked or unavailable keychain).
    pub unavailable: AtomicBool,
    /// When true writes succeed but nothing is stored (a no-op backend).
    pub forgetful: AtomicBool,
    /// When true a failing write reports the value in its error text, to check that callers scrub it.
    pub echo_value_in_errors: AtomicBool,
}

impl MemoryBackend {
    /// Empty backend.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of stored items.
    pub fn len(&self) -> usize {
        self.items.lock().unwrap().len()
    }

    /// True when nothing is stored.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl SecretBackend for MemoryBackend {
    fn describe(&self) -> String {
        "the test keychain".into()
    }

    fn get(&self, service: &str, user: &str) -> Result<Option<String>, String> {
        if self.unavailable.load(SeqCst) {
            return Err("keychain locked".into());
        }
        Ok(self.items.lock().unwrap().get(&(service.into(), user.into())).cloned())
    }

    fn set(&self, service: &str, user: &str, value: &str) -> Result<(), String> {
        if self.unavailable.load(SeqCst) {
            return Err(if self.echo_value_in_errors.load(SeqCst) {
                format!("keychain refused to store '{value}'")
            } else {
                "keychain locked".into()
            });
        }
        if !self.forgetful.load(SeqCst) {
            self.items
                .lock()
                .unwrap()
                .insert((service.into(), user.into()), value.into());
        }
        Ok(())
    }

    fn delete(&self, service: &str, user: &str) -> Result<(), String> {
        if self.unavailable.load(SeqCst) {
            return Err("keychain locked".into());
        }
        self.items.lock().unwrap().remove(&(service.into(), user.into()));
        Ok(())
    }
}

/// Keychain first, plaintext fallback second.
pub struct LayeredKeyStore {
    primary: Box<dyn SecretBackend>,
    fallback: Box<dyn KeyStore>,
    last: Mutex<String>,
}

impl LayeredKeyStore {
    /// Store keys in `primary`, falling back to `fallback` when `primary` does not work.
    pub fn new(primary: Box<dyn SecretBackend>, fallback: Box<dyn KeyStore>) -> Self {
        let last = Mutex::new(primary.describe());
        LayeredKeyStore {
            primary,
            fallback,
            last,
        }
    }
}

impl KeyStore for LayeredKeyStore {
    fn get(&self, provider: &str) -> Option<Secret> {
        match self.primary.get(SERVICE, provider) {
            Ok(Some(v)) if !v.is_empty() => Some(Secret::new(v)),
            _ => self.fallback.get(provider),
        }
    }

    fn set(&self, provider: &str, key: &Secret) -> Result<(), String> {
        let stored = self
            .primary
            .set(SERVICE, provider, key.expose())
            .map_err(|e| key.scrub(&e))
            .and_then(|()| match self.primary.get(SERVICE, provider) {
                Ok(Some(v)) if v == key.expose() => Ok(()),
                Ok(_) => Err("the keychain accepted the key but did not keep it".to_string()),
                Err(e) => Err(key.scrub(&e)),
            });
        match stored {
            Ok(()) => {
                // Do not leave an older plaintext copy behind.
                if self.fallback.get(provider).is_some() {
                    let _ = self.fallback.delete(provider);
                }
                *self.last.lock().unwrap() = self.primary.describe();
                Ok(())
            }
            Err(why) => {
                self.fallback
                    .set(provider, key)
                    .map_err(|e| key.scrub(&format!("{e} (keychain: {why})")))?;
                *self.last.lock().unwrap() = format!("{} (keychain unavailable: {why})", self.fallback.location());
                Ok(())
            }
        }
    }

    fn delete(&self, provider: &str) -> Result<(), String> {
        // A key that was only ever in one place is gone either way; report a failure only if
        // the fallback (always reachable) failed.
        let _ = self.primary.delete(SERVICE, provider);
        if self.fallback.get(provider).is_some() {
            self.fallback.delete(provider)
        } else {
            Ok(())
        }
    }

    fn location(&self) -> String {
        self.last.lock().unwrap().clone()
    }
}
