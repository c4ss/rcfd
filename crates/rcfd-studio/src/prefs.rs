//! User preferences: theme, panel layout and recent projects (ADR 0131 B8).
//!
//! Kept in the user's configuration directory ([`config_dir`]), never inside a project. Holds no case data and no keys.
//! A missing file gives the defaults; an unreadable or invalid one gives the defaults and a message, never a panic.

use crate::layout::{self, Panel};
use crate::theme::Theme;
use egui_dock::DockState;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// File name inside the configuration directory.
pub const FILE: &str = "studio-prefs.json";
/// Length of the recent-projects list.
pub const MAX_RECENT: usize = 10;

/// The Studio's configuration directory: `%APPDATA%\rcfd\studio` on Windows, else `$XDG_CONFIG_HOME/rcfd/studio` or
/// `$HOME/.config/rcfd/studio`. `None` when none of those variables is set. Standard library only.
pub fn config_dir() -> Option<PathBuf> {
    let var = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    let base = if cfg!(windows) {
        var("APPDATA")
    } else {
        var("XDG_CONFIG_HOME").or_else(|| var("HOME").map(|h| h.join(".config")))
    };
    base.map(|b| b.join("rcfd").join("studio"))
}

/// Saved preferences.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Prefs {
    /// Theme.
    #[serde(default)]
    pub theme: Theme,
    /// Layout as `egui_dock` JSON text (validated on load).
    #[serde(default)]
    pub layout: Option<String>,
    /// Recently opened project folders, most recent first.
    #[serde(default)]
    pub recent: Vec<String>,
}

/// Result of [`Prefs::load`].
pub struct Loaded {
    /// The preferences (defaults where the file was missing or invalid).
    pub prefs: Prefs,
    /// The layout to use: the saved one if valid, else [`layout::default_layout`].
    pub dock: DockState<Panel>,
    /// What was wrong with the file, located; `None` when it was fine or absent.
    pub message: Option<String>,
}

impl Prefs {
    /// Preferences for the current state.
    pub fn capture(theme: Theme, dock: &DockState<Panel>, recent: &[String]) -> Prefs {
        Prefs {
            theme,
            layout: layout::to_json(dock).ok(),
            recent: recent.to_vec(),
        }
    }

    /// Put `root` first in the recent list (no duplicates, at most [`MAX_RECENT`]).
    pub fn remember(&mut self, root: &Path) {
        let s = root.display().to_string();
        self.recent.retain(|r| *r != s);
        self.recent.insert(0, s);
        self.recent.truncate(MAX_RECENT);
    }

    /// Write to `dir/`[`FILE`] atomically (the directory is created).
    pub fn save(&self, dir: &Path) -> Result<(), String> {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        rcfd_project::atomic::atomic_write(&dir.join(FILE), text.as_bytes()).map_err(|e| e.to_string())
    }

    /// Read `dir/`[`FILE`].
    pub fn load(dir: &Path) -> Loaded {
        let path = dir.join(FILE);
        let defaults = |message: Option<String>| Loaded {
            prefs: Prefs::default(),
            dock: layout::default_layout(),
            message,
        };
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return defaults(None),
            Err(e) => return defaults(Some(format!("{}: {e}; default preferences used", path.display()))),
        };
        let prefs: Prefs = match serde_json::from_str(&text) {
            Ok(p) => p,
            Err(e) => return defaults(Some(format!("{}: {e}; default preferences used", path.display()))),
        };
        match prefs.layout.as_deref().map(layout::from_json) {
            None => Loaded {
                prefs,
                dock: layout::default_layout(),
                message: None,
            },
            Some(Ok(dock)) => Loaded {
                prefs,
                dock,
                message: None,
            },
            Some(Err(e)) => Loaded {
                prefs: Prefs { layout: None, ..prefs },
                dock: layout::default_layout(),
                message: Some(format!("{}: layout: {e}; default layout used", path.display())),
            },
        }
    }
}
