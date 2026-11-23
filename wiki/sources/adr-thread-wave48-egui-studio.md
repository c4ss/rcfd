---
type: source
tags: [adr-thread, studio, egui, rcfd-studio, rcfd-gui, wave-48, claude-code, plugin, viewport, run-control]
sources: [doc-ui-restart-plan, adr-thread-studio-foundation-crates, adr-thread-studio-electron]
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: wave 48, the egui Studio (`rcfd-studio`) from its shell to "first run to first picture" (2026-10-28 to 2026-10-29)

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread: one wave overview with the run-record contract (ADR 0130), five accepted sub-wave scopings (48B to 48F), and the follow-up jobs 48G to 48AA and 49B that the maintainer asked for while using the window; 18 ADRs, 2026-10-28 to 2026-10-29. Most file names carry no item; the items in the index (F-270, F-274, F-287, F-755, F-837) are later items that cite these ADRs.

Wave 48 built the window layer L5 of the Studio plan ([[doc-ui-restart-plan]]) on top of the headless crates of wave 47 ([[adr-thread-studio-foundation-crates]]): a new crate `rcfd-studio` on eframe/egui 0.36.2, with the render core in a viewport, run control through [[rcfd-runner]], a Claude Code sidebar on `rcfd-claude` and a Claude Code plugin with project tools in [[rcfd-mcp]]. The maintainer accepted ADRs 0130 to 0135 on 2026-10-28 "and took the recommended option of every open decision W48-1 to W48-16, scope cut (a)". Then the maintainer opened the window and reported what could not be done, and each report became a job: no picture without a run (48G1 to 48G4), an empty new project (48H), a mesh that could not be reached after a run (48J), follow-ups of the live preview (48M, 48N, 48O), a store key saved at its default (48R) and no obvious Stop (48AA). The jobs share the house rules of ADR 0130: "limits are fixed here before the registered runs; a missed limit becomes an ignored `*_known_miss` test with the limit intact; a defective set-up gets an amendment written before the corrected run" (quoted in ADRs 0149, 0153, 0155, 0161).

## Current state

- **The egui Studio is gone.** `crates/rcfd-studio` and `crates/rcfd-gui` were deleted on 2026-11-16 (ADR 0800 §10, F-449 done ahead of F-448), and later the wgpu render core `rcfd-render` and `rcfd-claude` were removed (F-838) ([[adr-thread-studio-electron]]). The crate list at 2e94e5ef has neither. What this thread built that outlives the window: the run-record contract of 48A (the runner on `rcfd-project` types, `--store-dir`, `--status-monitors`), the opt-in case key `output.store_precision` (48D amendment 1), the `--project` project tools and the plugin in `clients/claude-plugin/` (48F), the store-key rule of 48R and the example projects of 48H ([[doc-example-projects]], [[adr-thread-example-projects-and-linux-values]]).
- **Every ADR after 0135 is "not yet accepted by the maintainer"** in its status line (registered or proposed). Several questions were answered in later ADRs: ADR 0138 Q1 to Q8 (ADR 0144 and its amendment 3), ADR 0144's questions (ADR 0147), ADR 0147 Q2 to Q4 (b) (ADR 0148), ADR 0148's questions (ADR 0149), ADR 0149 Q9 (ADR 0152).
- **Known misses carried by the Studio suite** at the end of the wave: ADR 0131 B9, ADR 0133 R6, ADR 0141 G1, ADR 0144 J3b ("the same four known misses", ADR 0147), plus ADR 0161 AA1 and AA2; under ADR 0169 the aerofoil parts of ADR 0142 G1 and ADR 0148 N9 became ignored known misses because the regenerated runs converge.
- **What never ran in this wave:** "a person at the real window", Linux for most new process tests, the workspace gate, workspace-wide clippy and `cargo deny` are in the not-run list of nearly every addendum.
- **The design lesson recorded for the next Studio:** the egui choice was revisited by ADR 0350 (2026-11-12), whose context says "egui fixes look, layout and motion in Rust code; it has no style sheets, a shallow layout model, hand-made animation, and weaker text, input-method and accessibility support than a browser engine" ([[adr-thread-studio-electron]]).

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0130](../../docs/adr/0130-wave48a-overview-and-run-record-contract.md) | 2026-10-28 | plan (hub) + registration | F-274, F-287 | Wave 48 overview, W48-1 to W48-16; 48A one run-record contract for runner, Studio and plugin | accepted; A1 to A8 passed, no known miss (A4 approximated, A5 touched crates only); defect `serve --project <relative path>` not fixed | |
| [0131](../../docs/adr/0131-wave48b-studio-application-shell.md) | 2026-10-28 | scoping + results | | The application shell `rcfd-studio` (L5), eframe/egui 0.36.2, `rcfd-gui` frozen | accepted; B1 to B9 pass; four B9 parity items ignored known misses; a save turns the 419-byte cavity case into 15,898 bytes | 0149, 0152 (save effects) |
| [0132](../../docs/adr/0132-wave48c-viewport-integration.md) | 2026-10-28 | scoping + results | | Render core in the window: camera, scalar display, time scrubbing | accepted; V1 to V11 pass, no known miss; V7 100 ms registered for `DiscreteGpu` only | 0148 (middle drag) |
| [0133](../../docs/adr/0133-wave48d-run-control-and-live-monitoring.md) | 2026-10-28 | scoping + results | | Several background runs through `rcfd-runner`, live monitoring | accepted; R6 known miss (45.7 ms against 5 ms), R6b met at 0.710 ms; key `output.store_precision` | |
| [0134](../../docs/adr/0134-wave48e-claude-code-sidebar.md) | 2026-10-28 | scoping + results | | Claude Code chat sidebar on `rcfd-claude` | accepted; E1 to E11 pass with `fake-claude`; no real session run; seven open questions | ADR 0801 Addendum 23 (panel removed) |
| [0135](../../docs/adr/0135-wave48f-plugin-and-project-tools.md) | 2026-10-28 | scoping + results | F-755 | Plugin `rcfd`, project tools behind `--project`, edit conflicts by base hash and cooperative lock | accepted; F1 to F5, F8 to F13 pass; F6, F7 need the Studio, not run | [[adr-thread-workspace-layer]] |
| [0138](../../docs/adr/0138-wave48g1-mesh-preview-from-case.md) | 2026-10-28 | registration | | Mesh preview of a case before any run | every G check passed; limits 2,000,000 cells, 256 MiB, 250,000 triangles are proposals; Q8 shrank G3, G5 to 500 x 500 | 0144 |
| [0139](../../docs/adr/0139-wave48g2-case-starter.md) | 2026-10-28 | registration | | "New case from example..." | 25 of 25 after a review fix; 10 examples after amendment 2 | 0147 |
| [0140](../../docs/adr/0140-wave48g3-store-fields-by-default.md) | 2026-10-28 | registration | F-837 | Studio-started runs store fields by default; empty states that say why | every limit met; stores 342 012 / 346 419 / 271 872 bytes against estimates 342 016 / 342 016 / 260 000 | 0152 |
| [0141](../../docs/adr/0141-wave48g4-one-click-run-and-follow.md) | 2026-10-28 | registration | | Run button and F5, follow latest, Pause / Resume / Cancel strip | G1 known miss (Plots not in front); G1b, G2 to G9 pass; first Run waited about 5.3 s | 0143 (start route) |
| [0144](../../docs/adr/0144-wave48j-mesh-run-toggle-and-live-preview.md) | 2026-10-28 | registration | | Mesh / Run toggle; preview follows unsaved edits after 500 ms | J3b known miss (44 frames, limit 3); suite 174 / 0 / 4; editor paths against the case's folder (`base_dir`) | 0147, 0149, 0161 |
| [0147](../../docs/adr/0147-job48m-run-gate-camera-skip-starter-data-files-run-order.md) | 2026-10-28 | registration | F-270 | Follow-ups of the live preview (typed field, camera kept, no rebuild of an identical text, data files, run order) | every check met, no known miss; suite 189 / 0 / 4 | 0148 |
| [0148](../../docs/adr/0148-job48n-selected-case-middle-drag-units-scale-refine.md) | 2026-10-28 | registration | F-270 | The selected case as state; middle drag orbits; Run page units; finer mesh | every check met; suite 212 / 0 / 4; N9 later an ignored known miss (ADR 0169) | 0149, 0153 (Q4) |
| [0149](../../docs/adr/0149-job48o-linked-selection-drafts-per-case-json-cases.md) | 2026-10-28 | registration | | One selection for a case and its runs; unsaved edits per case (`MAX_PARKED` 32); JSON cases | every check met; deviation 4: a saved case stores no fields when Run starts it | 0152 |
| [0151](../../docs/adr/0151-job48q-cpu-render-speed-1m-cells-byte-identical.md) | 2026-10-28 | registration (two parts) | | Faster 1M-cell CPU contour render, byte-identical images | every limit met; 0.73 s to 0.05 s unrestricted, 2.18 s to 0.21 s under S2L | |
| [0152](../../docs/adr/0152-job48r-store-key-at-its-default-is-not-the-case-s-own.md) | 2026-10-28 | registration | | A store key at its schema default is not the case's own | every limit met; rerun 246 / 0 / 5 | |
| [0161](../../docs/adr/0161-job48aa-stop-in-runs-place.md) | 2026-10-29 | registration | | Stop in Run's place on the toolbar (Ctrl+.) | AA1, AA2 known misses (0 `cancel_requested` events), AA1b, AA2b pass | |
| [0165](../../docs/adr/0165-job49b-rcfd-gui-on-egui-036.md) | 2026-10-29 | registration | | The legacy `rcfd-gui` to egui 0.36.2, egui_dock 0.21.1, egui_plot 0.37.0 | L1 to L9 met; `Cargo.lock` 577 to 469 packages; no real window run | ADR 0800 §10 (crate deleted) |

The runner-side jobs of the wave (48I, 48L) are on [[adr-thread-runner-records-and-run-sites]]; the time-step and corrector jobs (48K, 48S, 48V, 48X, 48Z, 48AB) on [[adr-thread-setup-warnings-and-time-step]] and [[adr-thread-piso-stability-consistent-correctors]]; the example-project jobs (48H, 48P, 48U, 48W) on [[adr-thread-example-projects-and-linux-values]].

## Key numbers and limits

- **48A decisions taken** (ADR 0130): D3 (b) `--store-dir`, D5 (a) `--status-monitors`; the Studio applies D4 (a) later. On Linux L1, S4a and S4b fail "the same way on 3492c75 in the same container, so they are not caused by 48A".
- **48B accepted options** (ADR 0131): W48-1 (b) "only `rcfd-studio` needs Rust 1.95"; W48-2 (a) eframe with egui panels at 0.36.2; W48-3 (a) a new crate with `rcfd-gui` frozen; W48-9 (a) copies of the frozen GUI's logic modules. "A relative data file in an edited case resolves against the working folder, not the case's folder" (fixed by ADR 0144 amendment 3).
- **48C** (ADR 0132): W48-7 (a) a shared device rendering into a texture egui shows; W48-8 (a) a scalar-only update while scrubbing. "WARP's median step is 179.3 ms on the 100^3 box and 1414.4 ms on the 1 000 000-triangle surface."
- **48D** (ADR 0133): "The Studio never runs the solver in process; a worker thread per open project reads the run files, and the UI thread reads nothing from disk." R6b median 0.710 ms; 6.700 ms (reported, not limited) with every `status.json` replaced before each tick.
- **48E** (ADR 0134): "a project folder received from someone else, holding a `.mcp.json` or hooks, makes the sidebar's `claude -p` run commands from that folder, and the allow-list cannot prevent this." Decisions W48-10 (a) and (b) (pin MCP servers, confirm the folder), W48-11 (a) (a per-project allow-list file), amendment 2 (a per-user trust record outside the project).
- **48F** (ADR 0135): W48-12 (a) project tools behind `--project`; W48-13 (a) server from the plugin's `.mcp.json` with `rcfd-mcp` on PATH; W48-14 (a) plugin `rcfd` in `clients/claude-plugin/`; W48-15 (a) base hash and cooperative lock. A library-level F6 stand-in "lost 0 updates". The marketplace owner name ("rcfd maintainers") "is still a placeholder".
- **48G1 limits** (ADR 0138): 2,000,000 cells for the generator estimate, 256 MiB for a mesh file, 250,000 surface triangles; over a limit "Build anyway" or "Show anyway".
- **48O** (ADR 0149): at most 32 kept editors (`MAX_PARKED`); closing with unsaved state asks first.
- **48R rule** (ADR 0152): "a zero store key that equals the schema's default" is taken as not set; "An explicit zero cannot be told from the default, so from the Studio a run without stored fields starts only through the Start run dialog, and Run and F5 always store".
- **48Q** (ADR 0151): allocation "about 58 % of the base render"; "no floating-point expression changed"; two threads under S2L slower than one (0.619 s against 0.381 s). The failing CI limits were 2.316 s and 2.086 s against 2 s.
- **48AA** (ADR 0161): Stop is the runner's graceful stop; the run shows "stopped", `status.json` keeps `cancelled`, exit code 7; "a stopped run cannot be continued".

## Open questions

- ADR 0134 addendum 3: seven questions, among them how the E5 rerun is treated and "discovery running `claude agents --json` while the folder is not confirmed"; moot for the egui sidebar since ADR 0801 Addendum 23 removed the panel.
- ADR 0141 question 11: the decision of 2026-10-28 to remove the PowerShell route (done by ADR 0143).
- ADR 0147 questions 1 and 5 "left as they are"; questions 6 and 7 "were not answered in that message".
- ADR 0148 question 1: R5 (ADR 0144's rule) not changed.
- ADR 0152 question 1: the narrower reading of the four store keys.
- ADR 0165 Q1 to Q3: retiring `rcfd-gui`, the CI job's name, Dependabot 7, 8, 10; Q1 settled by the deletion of 2026-11-16 ([[adr-thread-studio-electron]]).

## Related

- Predecessors (added by p3-records-a): the layers L1 to L4 and L6 that wave 48 put into a window are the wave-47 crates of ADRs 0122 to 0126 and the audit of ADR 0128; see [[adr-thread-studio-foundation-crates]].
- [[studio]], [[rcfd-runner]], [[rcfd-project]], [[rcfd-tsstore]], [[rcfd-mcp]], [[claude-plugin]], [[field-store]], [[doc-example-projects]], [[known-miss]], [[registration]]
- Threads: [[adr-thread-studio-foundation-crates]] (wave 47), [[adr-thread-studio-electron]] (the successor), [[adr-thread-workspace-layer]], [[adr-thread-runner-records-and-run-sites]], [[adr-thread-example-projects-and-linux-values]], [[adr-thread-setup-warnings-and-time-step]]
- Areas: [[roadmap-studio-desktop-app]], [[roadmap-claude-and-the-mcp-server]]
