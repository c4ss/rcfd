//! Provider selection, endpoint and key management for the sidebar (no UI code).

use rcfd_assistant::config::{
    build_provider, config_dir, resolve_key, AssistantConfig, FileKeyStore, KeyStore, MemoryKeyStore, ProviderConfig,
};
use rcfd_assistant::http::Transport;
use rcfd_assistant::keychain::{LayeredKeyStore, OsKeychain};
use rcfd_assistant::openai::OpenAiCompatible;
use rcfd_assistant::{Provider, ProviderError, Secret};
use std::path::PathBuf;
use std::sync::Arc;

/// Assistant configuration as edited in the sidebar.
pub struct AssistantSetup {
    /// Provider list and selection.
    pub config: AssistantConfig,
    /// Where `providers.json` lives (`None` = do not persist).
    pub config_path: Option<PathBuf>,
    store: Box<dyn KeyStore>,
    /// Key typed by the user; cleared as soon as it is stored.
    pub key_input: String,
    /// Last status or error line.
    pub status: Option<String>,
    /// Models reported by the last probe.
    pub models: Vec<String>,
}

impl AssistantSetup {
    /// Load from the per-user config directory (providers.json). Keys go to the OS keychain when the
    /// platform has one, with keys.json in that directory as the fallback.
    pub fn load_default() -> AssistantSetup {
        match config_dir() {
            Some(d) => {
                let file = FileKeyStore::new(d.join("keys.json"));
                let store: Box<dyn KeyStore> = match OsKeychain::new() {
                    Some(k) => Box::new(LayeredKeyStore::new(Box::new(k), Box::new(file))),
                    None => Box::new(file),
                };
                Self::load_with_store(d, store)
            }
            None => Self::in_memory(),
        }
    }

    /// Load from `dir` with keys in `dir/keys.json` only (never the OS keychain; used by tests).
    pub fn load_from(dir: PathBuf) -> AssistantSetup {
        let store = Box::new(FileKeyStore::new(dir.join("keys.json")));
        Self::load_with_store(dir, store)
    }

    /// Load providers.json from `dir` and keep keys in `store`.
    pub fn load_with_store(dir: PathBuf, store: Box<dyn KeyStore>) -> AssistantSetup {
        let path = dir.join("providers.json");
        let (config, status) = match AssistantConfig::load(&path) {
            Ok(c) => (c, None),
            Err(e) => (
                AssistantConfig {
                    providers: ProviderConfig::presets(),
                    active: None,
                },
                Some(format!("could not read settings, using defaults: {e}")),
            ),
        };
        AssistantSetup {
            config,
            config_path: Some(path),
            store,
            key_input: String::new(),
            status,
            models: vec![],
        }
    }

    /// Nothing is written to disk (tests, or no config directory).
    pub fn in_memory() -> AssistantSetup {
        AssistantSetup {
            config: AssistantConfig {
                providers: ProviderConfig::presets(),
                active: None,
            },
            config_path: None,
            store: Box::new(MemoryKeyStore::new()),
            key_input: String::new(),
            status: None,
            models: vec![],
        }
    }

    /// The selected provider's config.
    pub fn active(&self) -> Option<&ProviderConfig> {
        self.config.active_provider()
    }

    /// Mutable access to the selected provider's config.
    pub fn active_mut(&mut self) -> Option<&mut ProviderConfig> {
        let n = self.config.active.clone()?;
        self.config.providers.iter_mut().find(|p| p.name == n)
    }

    /// Select by name.
    pub fn select(&mut self, name: &str) {
        if self.config.providers.iter().any(|p| p.name == name) {
            self.config.active = Some(name.to_string());
            self.models.clear();
        }
    }

    /// Does the selected provider have a key (environment or stored)?
    pub fn has_key(&self) -> bool {
        self.active()
            .is_some_and(|c| resolve_key(c, self.store.as_ref()).is_some())
    }

    /// Store the typed key for the selected provider and clear the input box.
    pub fn save_key(&mut self) {
        let Some(name) = self.active().map(|c| c.name.clone()) else {
            return;
        };
        if self.key_input.trim().is_empty() {
            return;
        }
        let key = Secret::new(self.key_input.trim());
        self.key_input.clear();
        self.status = Some(match self.store.set(&name, &key) {
            Ok(()) => format!("key for '{name}' stored in {}", self.store.location()),
            Err(e) => format!("could not store key: {e}"),
        });
    }

    /// Forget the stored key of the selected provider.
    pub fn delete_key(&mut self) {
        if let Some(n) = self.active().map(|c| c.name.clone()) {
            let _ = self.store.delete(&n);
            self.status = Some(format!("key for '{n}' removed"));
        }
    }

    /// One-line description of where keys are kept, for the settings panel.
    pub fn key_storage_note(&self) -> String {
        format!(
            "Keys are kept in {} (the environment variable, if set, wins) and never logged.",
            self.store.location()
        )
    }

    /// Persist the provider list.
    pub fn save(&mut self) {
        if let Some(p) = &self.config_path {
            if let Err(e) = self.config.save(p) {
                self.status = Some(format!("could not save settings: {e}"));
            }
        }
    }

    /// Build the selected provider and its model name.
    pub fn build(&self, transport: Option<Arc<dyn Transport>>) -> Result<(Arc<dyn Provider>, String), ProviderError> {
        let cfg = self
            .active()
            .ok_or_else(|| ProviderError::Config("no provider selected".into()))?;
        if cfg.model.trim().is_empty() {
            return Err(ProviderError::Config("enter a model name first".into()));
        }
        let p = build_provider(cfg, self.store.as_ref(), transport)?;
        Ok((Arc::from(p), cfg.model.clone()))
    }

    /// Ask an OpenAI-compatible endpoint for its models (`GET /models`). Blocks; call from a thread.
    pub fn probe(&self, transport: Option<Arc<dyn Transport>>) -> Result<Vec<String>, ProviderError> {
        let cfg = self
            .active()
            .ok_or_else(|| ProviderError::Config("no provider selected".into()))?;
        if cfg.kind != rcfd_assistant::config::ProviderKind::OpenAiCompatible {
            return Err(ProviderError::Config(
                "model listing is only available for OpenAI-compatible endpoints".into(),
            ));
        }
        let t = transport.unwrap_or_else(|| Arc::new(rcfd_assistant::http::UreqTransport::new(10)));
        OpenAiCompatible::new(&cfg.base_url, resolve_key(cfg, self.store.as_ref()), t).list_models()
    }
}
