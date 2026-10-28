//! rcfd Studio (plan layer L5, ADR 0131): an eframe/egui application on wgpu that opens project folders, lists their
//! cases, meshes and runs, and edits case files with the schema-driven editor.
//!
//! State and decisions live in the non-UI modules ([`studio`], [`project_view`], [`case_edit`], [`watch`], [`layout`],
//! [`prefs`], [`runs`], [`poll`], [`viewport`]); [`ui`] and [`viewport_ui`] only draw. The viewport (ADR 0132) renders a
//! run's stored fields with `rcfd-render` on eframe's own wgpu device into a texture that egui shows. File work runs on worker threads per open project: the project
//! worker ([`watch`]) and the run worker ([`poll`], ADR 0133), which talks to the project's `rcfd-runner`. Tests drive
//! the model and draw headless frames; the look of the window is checked by hand (ADR 0131 "Not in CI"). The Claude Code
//! sidebar ([`chat`], ADR 0134) has its own worker and turn threads.

pub mod bins;
pub mod case_edit;
pub mod chat;
pub mod chat_ui;
pub mod layout;
pub mod monitors;
pub mod poll;
pub mod prefs;
pub mod project_view;
pub mod quick_run;
pub mod quick_run_ui;
pub mod runs;
pub mod schema_model;
pub mod studio;
pub mod theme;
pub mod ui;
pub mod viewport;
pub mod viewport_ui;
pub mod watch;

pub use studio::Studio;
pub use ui::StudioApp;
