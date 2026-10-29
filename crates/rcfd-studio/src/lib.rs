//! rcfd Studio (plan layer L5, ADR 0131): an eframe/egui application on wgpu that opens project folders, lists their
//! cases, meshes and runs, and edits case files with the schema-driven editor.
//!
//! State and decisions live in the non-UI modules ([`studio`], [`project_view`], [`case_edit`], [`watch`], [`layout`],
//! [`prefs`], [`runs`], [`poll`], [`viewport`], [`preview`]); [`ui`], [`viewport_ui`] and [`preview_ui`] only draw. The
//! viewport (ADR 0132) renders a run's stored fields with `rcfd-render` on eframe's own wgpu device into a texture that
//! egui shows; its Mesh / Run toggle ([`view_toggle`], ADR 0144) shows instead the mesh of the open case (ADR 0138), which
//! follows the case editor's unsaved edits. File work runs on worker threads per open project: the project
//! worker ([`watch`]) and the run worker ([`poll`], ADR 0133), which talks to the project's `rcfd-runner`. Tests drive
//! the model and draw headless frames; the look of the window is checked by hand (ADR 0131 "Not in CI"). The Claude Code
//! sidebar ([`chat`], ADR 0134) has its own worker and turn threads. Studio-started runs store fields by default, and
//! a run without fields to show says why ([`store_plan`], [`run_fields`], [`fields_ui`], ADR 0140). Run sets the time
//! step of a transient run when it starts, from the mesh and the case's speeds, and simulates the time asked for on the
//! toolbar ([`time_step`], ADR 0153).

pub mod bins;
pub mod case_edit;
pub mod case_starter;
pub mod case_starter_ui;
pub mod chat;
pub mod chat_ui;
pub mod fields_ui;
pub mod layout;
pub mod monitors;
pub mod poll;
pub mod prefs;
pub mod preview;
pub mod preview_ui;
pub mod project_view;
pub mod quick_run;
pub mod quick_run_ui;
pub mod run_fields;
pub mod runs;
pub mod schema_model;
pub mod stop;
pub mod stop_ui;
pub mod store_plan;
pub mod studio;
pub mod theme;
pub mod time_step;
pub mod ui;
pub mod view_toggle;
pub mod viewport;
pub mod viewport_ui;
pub mod watch;

pub use studio::Studio;
pub use ui::StudioApp;
