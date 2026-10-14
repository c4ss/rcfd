//! rcfd desktop app: an eframe/egui shell with dockable panels, a schema-driven
//! settings editor, run controls, residual monitors, a field viewport and the
//! assistant sidebar.
//!
//! State and decisions live in the non-UI modules (all unit-tested). `ui` only
//! draws. The headless frame test in `tests/smoke.rs` runs the UI without a
//! window; the look of the app has not been inspected visually.

pub mod assistant_setup;
pub mod chat;
pub mod controller;
pub mod files;
pub mod layout;
pub mod monitors;
pub mod schema_model;
pub mod state;
pub mod theme;
pub mod ui;
pub mod viewport;

pub use ui::RcfdApp;
