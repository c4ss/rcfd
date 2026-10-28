//! Light and dark theme.

use serde::{Deserialize, Serialize};

/// Colour theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    /// Light visuals.
    #[default]
    Light,
    /// Dark visuals.
    Dark,
}

impl Theme {
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

    /// Make it the context's theme (cheap when unchanged).
    pub fn apply(self, ctx: &egui::Context) {
        if ctx.theme() != self.egui() {
            ctx.set_theme(self.egui());
        }
    }
}
