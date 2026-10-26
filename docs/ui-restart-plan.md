# Studio: restart of the desktop UI (plan)

Status: plan (2026-10-26), set by the user: restart the UI from scratch, wgpu renderer, project folders holding many runs, background and parallel runs, time scrubbing, a Claude Code chat inside the app, and the same project tools available from Claude Code itself. This is a plan, not a claim of capability.

## 1. Decisions (user, 2026-10-26)

1. **Renderer: wgpu** (not OpenGL). Reason given: better capability for 3D deforming meshes with flow over their surfaces (compute shaders for streamlines, particles and iso-surfaces, large buffers, offscreen export).
2. **Restart, not extend.** The new application is a new crate (working name `rcfd-studio`). The existing `rcfd-gui` (egui on OpenGL, 2D raster viewport) is frozen: no new features, kept building and tested until the new app reaches parity on the checks listed in section 6, then removed in a separate, explicitly requested step. (Default chosen by the lead; the user can change it.)
3. **Projects are folders** that hold many runs with all their data.
4. **Claude Code is the assistant**: the app shows and drives a Claude Code chat for the project folder (replacing the provider-key sidebar of F-145 as the primary path), and the same project tools are available from Claude Code as a plugin.

## 2. What the user gets (acceptance in plain words)

- Open a project folder; see its cases, meshes and every run, with state (queued, running, paused, finished, failed) and key numbers.
- Start several runs at once and in the background; close the app and the runs continue; reopen and see them.
- Scrub a run through time: fields, residuals and monitors move together; play, pause and step; compare two runs side by side at the same time.
- A 3D view of the mesh and of fields on surfaces, cut planes, iso-surfaces, vectors, streamlines and particles, with deformed shapes and stress/strain colouring once a structural solver exists (not before).
- The inputs of a run (the case) and its outputs (tables, plots, the design ledger) in panels.
- A Claude Code chat in a sidebar. A Claude Code chat opened in the project folder (terminal or elsewhere) appears in the sidebar as viewable and, where the mechanism allows, chattable.
- In Claude Code (outside the app), tools to list projects and runs, edit project files with validation, start and stop runs, read metrics, summarise fields at a time, render images, and compare runs. Edits made there appear live in the app.

## 3. Architecture (layers, each testable without a window)

| Layer | Content | Test route |
|---|---|---|
| L1 Project format | Folder layout and manifest; run records with a case snapshot, mesh reference and hash, solver version, settings, status; atomic writes; schema version and migration | files and unit tests |
| L2 Run service | A detached runner process that owns runs: queue, N concurrent runs under a CPU and memory budget, pause, resume, cancel, crash recovery, progress through append-only status files; GUI and Claude Code talk to it (local socket or files; one design chosen by ADR) | process tests, kill tests |
| L3 Time-series store | Mesh stored once, field snapshots per saved time (chunked, compressed, memory-mappable, indexed), plus residual and monitor series; random access for scrubbing; reader and writer in the solver path | round-trip and bit-exactness tests |
| L4 Render core (wgpu) | Surface mesh, per-vertex and per-cell scalars, colour maps, camera, picking, cut planes, iso-surfaces and streamlines (compute), vectors, particles, deformation, offscreen render to PNG and video | offscreen render against a CPU reference, pixel and statistic checks, frame-time budget |
| L5 App shell | egui on eframe's wgpu back end: docking, project browser, run list, viewports with linked time, plots, input editor (schema driven), output tables, logs | headless frames, golden images with tolerance, plus a human view-and-report step |
| L6 Claude Code bridge | Discover Claude Code sessions for the project folder; show a live transcript; send messages; start a new session; surface approvals | mechanisms researched first, then fixture and process tests |
| L7 Claude Code plugin | A plugin packaging an MCP server of project tools (extending `rcfd-mcp`), skills and slash commands; file watching so edits show in the app and conflicts are detected | MCP protocol tests, a real Claude Code session where possible |

## 4. Wave order

- **Wave 47 (foundations, no window needed):** L1 project format, L2 run service, L3 time-series store, L4 wgpu offscreen render core v0 (mesh, scalar colouring, camera, PNG, checked against a CPU reference), L6 research and library prototype (session discovery, transcript tail, headless message send).
- **Wave 48:** L5 application shell and viewport integration; L7 plugin and the project tools.
- **Wave 49:** compute rendering (iso-surfaces, streamlines, particles), animation, video export, side-by-side compare.
- **Wave 50:** computer-fan modes (tolerance spread, dust field, noise and vibration spectra, design ledger and Pareto view); stress and strain only after a structural solver (decision deferred, see `docs/fan-project-breakdown.md`).

Waves 48 and later start after the user has run the previous result and reported on it.

## 5. Limits stated first

1. Agents cannot see the window. They check logic, pixels and statistics headlessly; whether the app looks and feels right is for the user to judge, and every wave ends with a short view-and-report checklist.
2. A picture can be wrong and look right. Every colour mode has a test that its values match the solver's numbers.
3. wgpu needs a working GPU driver; the Windows path (DX12 or Vulkan) and a software fallback are checked in the first spike. Memory and build size grow.
4. Claude Code integration depends on what Claude Code exposes (session files, headless mode, resuming, plugin interfaces). Two writers on one live session may not be safe; the research decides what is offered. Nothing is promised before it.
5. Stress and strain pictures need a structural solver that does not exist yet.
6. Interactive rendering of very large meshes (millions of cells) is a performance target that has not been measured.

## 6. Parity checks before `rcfd-gui` is removed

Open and save a case; settings editor with schema validation; run with live residual and report monitors and run control; the 2D field view with colour map, legend and hover; assistant access (now through Claude Code); the tutorials that name the GUI still run or are rewritten. Each is a named test in the new crate.
