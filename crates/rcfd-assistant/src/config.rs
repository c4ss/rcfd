//! Provider configuration and local key storage.
//!
//! Storage design: provider settings (name, kind, base URL, model, the *name*
//! of an environment variable that holds the key) live in `providers.json`
//! in the per-user config directory. Keys live in a separate `keys.json`
//! that is created with owner-only permissions (0600) on unix. On Windows the
//! file is placed in the per-user `%APPDATA%` directory and relies on the
//! default per-user ACL; no ACL is set by this code. There is no OS keychain
//! integration yet (F-149). Keys are never written to `providers.json`, never
//! included in `Debug` output and scrubbed from provider error text.

use crate::anthropic::Anthropic;
use crate::http::{Transport, UreqTransport};
use crate::mock::MockProvider;
use crate::openai::OpenAiCompatible;
use crate::types::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Which wire protocol a provider speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    /// OpenAI Chat Completions compatible server.
    OpenAiCompatible,
    /// Anthropic Messages API.
    Anthropic,
    /// Scripted offline provider.
    Mock,
}

/// A configured model endpoint. Contains no secret.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// Display name and key-store key, e.g. `ollama`.
    pub name: String,
    /// Protocol.
    pub kind: ProviderKind,
    /// Base URL (OpenAI-compatible: up to and including `/v1`).
    pub base_url: String,
    /// Model name.
    pub model: String,
    /// Optional environment variable that supplies the key (takes precedence over the key file).
    #[serde(default)]
    pub key_env: Option<String>,
    /// Output token cap.
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    /// Request timeout in seconds.
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
    /// True for endpoints on this machine (no key needed, works offline).
    #[serde(default)]
    pub local: bool,
}

fn default_max_tokens() -> u32 {
    4096
}
fn default_timeout() -> u64 {
    300
}

impl ProviderConfig {
    /// Built-in presets: local servers and the two hosted protocols. Models are
    /// placeholders the user must change to a model they have.
    pub fn presets() -> Vec<ProviderConfig> {
        let p = |name: &str, kind, url: &str, model: &str, local| ProviderConfig {
            name: name.into(),
            kind,
            base_url: url.into(),
            model: model.into(),
            key_env: None,
            max_tokens: default_max_tokens(),
            timeout_secs: default_timeout(),
            local,
        };
        vec![
            p(
                "ollama",
                ProviderKind::OpenAiCompatible,
                "http://localhost:11434/v1",
                "",
                true,
            ),
            p(
                "lm-studio",
                ProviderKind::OpenAiCompatible,
                "http://localhost:1234/v1",
                "",
                true,
            ),
            p(
                "llama-cpp",
                ProviderKind::OpenAiCompatible,
                "http://localhost:8080/v1",
                "",
                true,
            ),
            p(
                "vllm",
                ProviderKind::OpenAiCompatible,
                "http://localhost:8000/v1",
                "",
                true,
            ),
            p(
                "openai",
                ProviderKind::OpenAiCompatible,
                "https://api.openai.com/v1",
                "",
                false,
            ),
            p(
                "anthropic",
                ProviderKind::Anthropic,
                crate::anthropic::DEFAULT_BASE_URL,
                "",
                false,
            ),
        ]
    }
}

/// Persisted settings.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AssistantConfig {
    /// Configured providers.
    pub providers: Vec<ProviderConfig>,
    /// Name of the selected provider.
    #[serde(default)]
    pub active: Option<String>,
}

impl AssistantConfig {
    /// The selected provider config.
    pub fn active_provider(&self) -> Option<&ProviderConfig> {
        let n = self.active.as_deref()?;
        self.providers.iter().find(|p| p.name == n)
    }

    /// Insert or replace by name.
    pub fn upsert(&mut self, p: ProviderConfig) {
        match self.providers.iter_mut().find(|x| x.name == p.name) {
            Some(x) => *x = p,
            None => self.providers.push(p),
        }
    }

    /// Load from `path`; a missing file gives the presets.
    pub fn load(path: &Path) -> Result<Self, String> {
        match std::fs::read_to_string(path) {
            Ok(t) => serde_json::from_str(&t).map_err(|e| format!("{}: {e}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(AssistantConfig {
                providers: ProviderConfig::presets(),
                active: None,
            }),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    /// Save to `path` (creates parent directories).
    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(d) = path.parent() {
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(path, text).map_err(|e| e.to_string())
    }
}

/// Per-user config directory for rcfd (`%APPDATA%\rcfd`, `$XDG_CONFIG_HOME/rcfd` or `~/.config/rcfd`).
pub fn config_dir() -> Option<PathBuf> {
    let var = |n: &str| std::env::var_os(n).filter(|v| !v.is_empty()).map(PathBuf::from);
    if cfg!(windows) {
        var("APPDATA").map(|d| d.join("rcfd"))
    } else {
        var("XDG_CONFIG_HOME")
            .or_else(|| var("HOME").map(|h| h.join(".config")))
            .map(|d| d.join("rcfd"))
    }
}

/// Where API keys are kept.
pub trait KeyStore: Send + Sync {
    /// Key stored for `provider`.
    fn get(&self, provider: &str) -> Option<Secret>;
    /// Store a key.
    fn set(&self, provider: &str, key: &Secret) -> Result<(), String>;
    /// Remove a key.
    fn delete(&self, provider: &str) -> Result<(), String>;
}

/// In-memory store (tests, or keys that must not touch disk).
#[derive(Default)]
pub struct MemoryKeyStore(Mutex<BTreeMap<String, String>>);

impl MemoryKeyStore {
    /// Empty store.
    pub fn new() -> Self {
        Self::default()
    }
}

impl KeyStore for MemoryKeyStore {
    fn get(&self, provider: &str) -> Option<Secret> {
        self.0.lock().unwrap().get(provider).map(|k| Secret::new(k.clone()))
    }
    fn set(&self, provider: &str, key: &Secret) -> Result<(), String> {
        self.0.lock().unwrap().insert(provider.into(), key.expose().to_string());
        Ok(())
    }
    fn delete(&self, provider: &str) -> Result<(), String> {
        self.0.lock().unwrap().remove(provider);
        Ok(())
    }
}

/// JSON file of keys with owner-only permissions on unix.
pub struct FileKeyStore {
    path: PathBuf,
}

impl FileKeyStore {
    /// Store at `path` (the file is created on first `set`).
    pub fn new(path: impl Into<PathBuf>) -> Self {
        FileKeyStore { path: path.into() }
    }

    fn read(&self) -> BTreeMap<String, String> {
        std::fs::read_to_string(&self.path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    fn write(&self, map: &BTreeMap<String, String>) -> Result<(), String> {
        if let Some(d) = self.path.parent() {
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        let text = serde_json::to_string_pretty(map).map_err(|e| e.to_string())?;
        write_private(&self.path, text.as_bytes()).map_err(|e| format!("{}: {e}", self.path.display()))
    }
}

#[cfg(unix)]
fn write_private(path: &Path, data: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    // The mode above only applies when the file is created; fix an existing file too.
    f.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    f.write_all(data)
}

#[cfg(not(unix))]
fn write_private(path: &Path, data: &[u8]) -> std::io::Result<()> {
    std::fs::write(path, data)
}

impl KeyStore for FileKeyStore {
    fn get(&self, provider: &str) -> Option<Secret> {
        self.read().get(provider).map(|k| Secret::new(k.clone()))
    }
    fn set(&self, provider: &str, key: &Secret) -> Result<(), String> {
        let mut m = self.read();
        m.insert(provider.into(), key.expose().to_string());
        self.write(&m)
    }
    fn delete(&self, provider: &str) -> Result<(), String> {
        let mut m = self.read();
        m.remove(provider);
        self.write(&m)
    }
}

/// The key for `cfg`: its environment variable first, then the store.
pub fn resolve_key(cfg: &ProviderConfig, store: &dyn KeyStore) -> Option<Secret> {
    if let Some(var) = cfg.key_env.as_deref().filter(|v| !v.is_empty()) {
        if let Some(v) = std::env::var(var).ok().filter(|v| !v.is_empty()) {
            return Some(Secret::new(v));
        }
    }
    store.get(&cfg.name)
}

/// Build the provider described by `cfg`, using `transport` (or a real HTTP client when `None`).
pub fn build_provider(
    cfg: &ProviderConfig,
    store: &dyn KeyStore,
    transport: Option<Arc<dyn Transport>>,
) -> Result<Box<dyn Provider>, ProviderError> {
    let transport = transport.unwrap_or_else(|| Arc::new(UreqTransport::new(cfg.timeout_secs)));
    match cfg.kind {
        ProviderKind::Mock => Ok(Box::new(MockProvider::empty())),
        ProviderKind::OpenAiCompatible => {
            if cfg.base_url.trim().is_empty() {
                return Err(ProviderError::Config("base URL is empty".into()));
            }
            Ok(Box::new(OpenAiCompatible::new(
                &cfg.base_url,
                resolve_key(cfg, store),
                transport,
            )))
        }
        ProviderKind::Anthropic => {
            let key = resolve_key(cfg, store).ok_or_else(|| {
                ProviderError::Config(format!(
                    "no API key for '{}'; add one in the assistant settings",
                    cfg.name
                ))
            })?;
            Ok(Box::new(Anthropic::new(&cfg.base_url, key, transport)))
        }
    }
}
