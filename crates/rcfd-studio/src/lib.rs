//! rcfd Studio (plan layer L5, ADR 0131): an eframe/egui application on wgpu that opens project folders, lists their
//! cases, meshes and runs, and edits case files with the schema-driven editor.
//!
//! State and decisions live in the non-UI modules ([`studio`], [`project_view`], [`case_edit`], [`watch`], [`layout`],
//! [`prefs`]); [`ui`] only draws. File work runs on one worker thread per open project ([`watch`]). Tests drive the model
//! and draw headless frames; the look of the window is checked by hand (ADR 0131 "Not in CI").

pub mod case_edit;
pub mod layout;
pub mod monitors;
pub mod prefs;
pub mod project_view;
pub mod schema_model;
pub mod studio;
pub mod theme;
pub mod ui;
pub mod watch;

pub use studio::Studio;
pub use ui::StudioApp;
