# ADR 0004: Desktop app and assistant crates, and their MSRV

Status: accepted.

Context: the AI helper is a native sidebar inside the desktop app (`rcfd-gui`),
backed by a provider-agnostic library (`rcfd-assistant`). The workspace MSRV is
1.75 (not verified on a 1.75 toolchain; development used rustc 1.99).

Decisions:
- GUI stack: eframe 0.27.2 with egui 0.27.2 (single egui in the tree), egui_dock
  0.12.0 and egui_plot 0.27.2. These are the newest releases whose declared
  rust-version is 1.72, so they fit the workspace MSRV on their own. Newer egui
  releases need a newer compiler and gain nothing the app needs yet.
- HTTP: ureq 3 (blocking, rustls). Its transitive crates (icu_*, home) declare
  rust-version up to 1.88 (checked with `cargo metadata`). Therefore
  `rcfd-assistant` and `rcfd-gui` declare `rust-version = "1.88"`. The solver crates
  keep 1.75 and do not depend on either. This 1.88 figure comes from declared
  metadata; builds on other toolchains were not run.
- No native file dialog (rfd); the app asks for typed paths. This avoids a GTK/portal
  dependency on Linux. Revisit when the viewport work lands.
- eframe `persistence` is not enabled: it serialises app memory to disk, which could
  leak keys. Only theme and layout are saved, by the app itself.
- API keys: environment variable first, then a per-user `keys.json`. On unix the file
  is created with mode 0600 (code is `cfg(unix)` and has not been run on Windows,
  where the per-user profile ACL is the only protection). No OS keychain integration
  yet; keys are plain text in that file. Keys are never logged and are scrubbed from
  error text.
- Tools run in-process: the assistant reuses the `rcfd-mcp` tool definitions and
  permission model; the GUI approval card is the approval hook, and the gate is
  enforced below the UI so a model cannot bypass it.
- Linux builds need the usual X11/Wayland development packages; CI changes for that
  are not made here.
