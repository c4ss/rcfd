# Studio: restart of the desktop UI (plan)

Status: plan (2026-10-26), set by the user: restart the UI from scratch, wgpu renderer, project folders holding many runs, background and parallel runs, time scrubbing, a Claude Code chat inside the app, and the same project tools available from Claude Code itself. This is a plan, not a claim of capability.

This page is for the user and for contributors working on the Studio. Read section 5 first for the limits: what agents cannot check, what may not be possible, and what does not exist yet. Section 8 holds the second round of decisions (how the chat sidebar treats open sessions and how tool approvals work) and the project folder contract that the new crates code against. For the computer-fan work that the Studio's later modes serve, see the [computer-fan project breakdown](fan-project-breakdown.md).

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

## 7. Claude Code integration: what the official docs say (researched 2026-10-26)

Source: the Claude Code documentation at code.claude.com/docs/en (sessions, headless, cli-reference, agent-sdk, remote-control, cross-session-messaging, vs-code, jetbrains, plugins, mcp, channels, agent-view), read on that date. Items below marked NOT DOCUMENTED were not found; they must be checked on the installed build before any design depends on them.

| Question | Finding | Consequence for the app |
|---|---|---|
| Transcripts | JSONL at `~/.claude/projects/<working-dir with non-alphanumerics replaced by '-'>/<session-id>.jsonl`. The entry format is declared internal and changing between versions. | List a folder's chats from the directory; do not depend on the line format. Take session id and transcript path from a SessionStart hook or from `-p` JSON output; pin a tested version range and keep a fallback. |
| Headless | `claude -p`, `--output-format stream-json`, `--input-format stream-json`, `--resume <id>`, `--session-id`, `--fork-session`. No Rust SDK; Python and TypeScript Agent SDKs exist; other languages spawn the CLI. | The app spawns the CLI per project; the stream-json event shapes are read from a real run (no schema published). |
| Approvals in headless | Tools needing approval are denied unless allowed by flags or settings, a PermissionRequest hook, or `--permission-prompt-tool` (contract NOT DOCUMENTED). The SDK offers a `canUseTool` callback. | Either a small Agent SDK sidecar owns approvals, or a conservative allow-list is used until the prompt-tool contract is verified. The app never auto-approves everything. |
| Two writers | Resuming one session in two terminals interleaves messages into one transcript; two processes cannot write the same transcript; a session open in a live process cannot be started a second time. | One writer per session. The sidebar mirrors a live terminal session read-only, or offers `claude attach` / Remote Control for control; it types into a session only when no other process has it open. |
| Observing live sessions | `claude agents --json` lists sessions (id, sessionId, name, cwd, state, status, pid); cross-session messaging gives plain-text messages between local sessions (message wire format beyond auth NOT DOCUMENTED). | Session discovery per project folder through `claude agents --json`; messages to a live session are possible but arrive as text, not commands. |
| IDE protocol | Lock file and local websocket exist but no third-party host API is published. | The app does not use the IDE protocol. |
| Plugins | `.claude-plugin/plugin.json`, skills, commands, agents, hooks, `.mcp.json` for a bundled MCP server; local load with `--plugin-dir`; marketplaces for distribution. Mods (panes beside the chat) run inside Claude Code's own process. | The project tools ship as a plugin with an MCP server. A Rust window cannot host a mod; the app builds its own chat panel. |
| MCP | stdio (local) and HTTP; results over about 10 000 tokens warn and 25 000 is the default limit; images shown inline; `list_changed` supported; server push of events only through channels (research preview, flag and auth constraints); resource subscriptions NOT VERIFIED. | A stdio MCP server in front of the run service; large outputs as files; run-finished events by polling a status tool first, channels later if they stabilise. |

Two documents' wording matched the harness's injection scanner (permission-mode names such as bypassPermissions); they describe options only, nothing in this plan uses them.

Decisions this research forces, for the user when wave 47 starts: (1) whether the sidebar may type into a session that is closed (resume with `-p`) while only mirroring open ones (recommended), and (2) whether approvals go through an Agent SDK sidecar (Python or TypeScript dependency) or a conservative allow-list.

## 8. Decisions of 2026-10-26 (second round) and the wave-47 contract

User decisions: (1) the sidebar may type into a Claude Code session that is closed (resume it with `claude -p --resume`) and only mirrors sessions that another process has open; (2) tool approvals use a conservative allow-list first (no sidecar, no auto-approval of everything); a sidecar on the Agent SDK is reconsidered only if the allow-list proves too narrow.

Project folder contract (agents code against it; later changes are additive and go through the lead):

```text
<project>/
  rcfd-project.json        schema_version, name, created, solver_version_created
  cases/                   case files in the existing case format
  meshes/                  mesh files or recipes referenced by cases
  runs/<run-id>/           run-id = r-<UTC yyyymmddThhmmss>-<4 hex>, sortable
    run.json               immutable record: case snapshot hash, mesh hash, solver version, settings, created
    case.snapshot          copy of the case as run
    status.json            state (queued, running, paused, finished, failed, cancelled), step, time, pid, updated; replaced atomically
    events.jsonl           append-only events (state changes, warnings, stop reasons)
    log.txt                solver log
    residuals.jsonl        one line per step: step, time, residuals, monitors; with solver.corrector_control =
                           "adaptive" also correctors and corrector_cap (ADR 0160; absent on every other run)
    fields/                time-series store (index.json, mesh once, one snapshot per saved time)
    metrics.json           final report values
  designs/                 design ledger (ADR 0121)
  notes/                   free text
  .rcfd/                   runner lock and socket, caches (not versioned)
```

Crates (all new, library first): `rcfd-project` (47A), `rcfd-runner` (47B), `rcfd-tsstore` (47C), `rcfd-render` (47D), `rcfd-claude` (47E). Studio application and the plugin follow in wave 48.

## Where to go next

- [Desktop assistant tutorial](tutorials/desktop-assistant.md) for the current, frozen GUI that the Studio replaces.
- [MCP tools](mcp-tools.md) for the existing project tools that the planned plugin (layer L7) extends.
- [Computer-fan project breakdown](fan-project-breakdown.md) for the fan work behind the later computer-fan modes.
