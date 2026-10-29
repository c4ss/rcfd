//! Light/dark theme and persisted user preferences.

use crate::layout::{self, Panel};
use egui_dock::DockState;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Colour theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    /// Light visuals.
    Light,
    /// Dark visuals.
    Dark,
}

impl Theme {
    /// egui visuals for the theme.
    pub fn visuals(self) -> egui::Visuals {
        match self {
            Theme::Light => egui::Visuals::light(),
            Theme::Dark => egui::Visuals::dark(),
        }
    }

    /// The other theme.
    pub fn toggled(self) -> Theme {
        match self {
            Theme::Light => Theme::Dark,
            Theme::Dark => Theme::Light,
        }
    }

    /// egui's theme value.
    pub fn egui(self) -> egui::Theme {
        match self {
            Theme::Light => egui::Theme::Light,
            Theme::Dark => egui::Theme::Dark,
        }
    }

    /// Apply to a context. egui 0.36 keeps a light and a dark style and by default follows the system theme; the theme
    /// is fixed first so that the system theme cannot switch it, then the visuals are set as before (ADR 0165 S3).
    pub fn apply(self, ctx: &egui::Context) {
        ctx.set_theme(self.egui());
        ctx.set_visuals(self.visuals());
    }
}

/// Saved preferences: theme and panel layout. Never contains keys or case data.
#[derive(Serialize, Deserialize)]
pub struct Prefs {
    /// Theme.
    pub theme: Theme,
    /// Layout as JSON text (validated on load).
    pub layout: Option<String>,
}

impl Prefs {
    /// Preferences for the current state.
    pub fn capture(theme: Theme, dock: &DockState<Panel>) -> Prefs {
        Prefs {
            theme,
            layout: layout::to_json(dock).ok(),
        }
    }

    /// Write to `path`.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(d) = path.parent() {
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        std::fs::write(path, serde_json::to_string_pretty(self).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
    }

    /// Read from `path`; `None` when absent or unreadable.
    pub fn load(path: &Path) -> Option<Prefs> {
        serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
    }

    /// The saved layout if present and valid.
    pub fn dock(&self) -> Option<DockState<Panel>> {
        self.layout.as_deref().and_then(|t| layout::from_json(t).ok())
    }
}
