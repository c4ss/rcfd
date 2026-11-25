---
type: entity
tags: [roadmap, area]
sources: [doc-roadmap-json, doc-roadmap-overview, adr-thread-studio-electron, adr-thread-studio-engine-entry-points, adr-thread-runner-records-and-run-sites, adr-thread-workspace-layer, adr-thread-mcp-tools, adr-thread-wave48-egui-studio]
created: 2026-11-22
updated: 2026-11-25
status: draft
---

# Roadmap area: Studio desktop app

**What it is:** the 'Studio desktop app' line of work (Use family) of the [[roadmap]], with every [[f-item]] in it ([[doc-roadmap-json]]).
**Where:** `docs/roadmap.json` (source of truth), categories in `scripts/roadmap_progress.py`.

## Synthesis

- **Two Studios, one left.** The egui Studio (`rcfd-studio`, layers L1 to L7 of the Studio plan) was built in waves 47 and 48; on 2026-11-12 the maintainer asked for an Electron spike over the rcfd APIs (ADR 0350, F-447), on 2026-11-16 ADR 0800 split F-448 into F-800 to F-832 and the maintainer said yes, and the same day `rcfd-studio` and `rcfd-gui` were deleted before parity (F-449 done) ([[adr-thread-studio-electron]]). F-140 to F-144, F-261 to F-266 and F-293 belong to that earlier line; F-293 is retired (2026-11-18) ([[adr-thread-workspace-layer]]).
- **The Electron Studio is mostly built.** ~~Of the 58 items, 40 are done~~ Of the 62 items, 44 are done, 13 partial and 5 not started at 709e302e ([[doc-roadmap-overview]]); the F-80x / F-81x / F-83x items each recorded their build in an addendum of ADR 0801 (25 addenda, to 2026-11-17). What keeps F-448 open: F-813 and F-830 wait for a learner's [[view-and-report]] step, F-817 for a diagnosis of P10 under load (the idle baseline of 2026-11-17 met every limit), F-821 for Linux GPU runs and the WebGL 2 bench lines, F-823 for a first green run on GitHub Actions, F-828 for Studio numbers that still have no provenance ([[adr-thread-studio-electron]], [[adr-thread-studio-engine-entry-points]]).
- **Engine first.** Every Studio item that needed a number, a write or a check got an engine entry point through its own ADR, accepted and built on 2026-11-17: the opt-in `rcfd serve --ui` family (schema metadata, help, error catalogue, `mesh.preview`, `case.facts`, `mesh.refine`, `case.save`), `rcfd project init`, launch records and rerun, session journals on disk, a project trash, compare-and-set, derived pictures, and `set_runner_config` ([[adr-thread-studio-engine-entry-points]], [[adr-thread-runner-records-and-run-sites]]). The rule behind it is G2 and G16 of the [[studio-guardrails]].
- **Claude Code outside the Studio.** The embedded Claude Code panel was removed on 2026-11-17 (ADR 0801 Addendum 23): the Studio registers the engine's project server with Claude Code for the folder, opens Claude Code in a terminal, and follows its changes through the [[project-journal]]'s feed; F-814 is still partial for a test with a real Claude Code session and the maintainer's view-and-report ([[adr-thread-studio-electron]]).
- **Next lines.** F-848 (run sites; phase 1 built by ADR 0739, the roadmap still says `todo`) ([[run-site]]), ~~F-852 (an end-to-end test that times out on Windows in CI)~~ (done, see the next bullet), F-151 (3D viewport of the old plan), F-266 (egui shell, todo), F-262 (the runner: socket control and runs started from the application; S5 an ignored known miss; the order of its final writes fixed by ADR 0742) ([[adr-thread-runner-records-and-run-sites]]).
- **Tests that failed on the lead's Windows machine (2026-11-23 and 2026-11-24).** Four items, each a failing or leaking end-to-end test traced to a cause by a test that fails without the fix: F-852 (ADR 0781, done: an event lost when the settings page replaced its listener), F-890 (ADR 0787, partial: a synchronous wait on the GPU process at the first 2D canvas; the task of 400 to 440 ms itself was not made again), F-891 (ADR 0788, done: the runner-budgets panel took a feed entry of its own write for another writer's change), F-892 (ADR 0789, done: a Studio that quit before its new `rcfd serve` was connected left it running). F-890 to F-892 are the follow-ups of F-888 (ADR 0786), entered and issued on the maintainer's "Yes, issue all three". After them the whole `npm test` (873 tests) passed in the first run at the pushes of F-861 and of F-852 with F-451. Follow-ups proposed and not entered: `undo.spec.ts:139`, a second Write after a refused one, `data.open`'s length unit, `run-control.spec.ts:320` ([[adr-thread-studio-electron]]).
- **The headless layers came first (added by p3-records-a).** Wave 47 (2026-10-26) built L1 `rcfd-project`, L2 `rcfd-runner`, L3 `rcfd-tsstore`, L4 `rcfd-render` and L6 `rcfd-claude` as separate crates before any UI (ADRs 0122 to 0126), and ADR 0128 fixed sixteen audit findings across them; the first three remain, the last two were removed later. Before that, the egui GUI and assistant of ADR 0004 had only headless checks (ADR 0025) ([[adr-thread-studio-foundation-crates]], [[adr-thread-founding-decisions]]).

## ADR threads

- [[adr-thread-studio-electron]] — ADR 0350 spike, ADR 0353 (F-456), ADR 0800 roadmap, ADR 0801 dependencies and its 25 addenda; the end-to-end test items F-888 (ADR 0786), F-852 (ADR 0781), F-890 to F-892 (ADRs 0787 to 0789).
- [[adr-thread-studio-engine-entry-points]] — ADRs 0377, 0730, 0731, 0732, 0735, 0736, 0737, 0738: the engine entry points of the Studio items.
- [[adr-thread-runner-records-and-run-sites]] — ADRs 0733, 0734, 0739, 0742: runner budgets, launch records, run sites, final writes (F-837, F-828, F-848, F-262).
- [[adr-thread-workspace-layer]] — ADR 0173 and its amendments (F-270 to F-275), which the Studio and the MCP server share.
- [[adr-thread-mcp-tools]] — the tool list and allow-list changes these ADRs made.
- [[adr-thread-studio-foundation-crates]] — ADRs 0025, 0122 to 0126, 0128 (F-143, F-144, F-261 to F-265, F-866)
- [[adr-thread-founding-decisions]] — ADR 0004 (F-140, the egui app and assistant crates and their MSRV)
- [[adr-thread-wave48-egui-studio]] — ADRs 0130 to 0135, 0138 to 0141, 0144, 0147 to 0149, 0151, 0152, 0161, 0165: wave 48, the egui Studio from its shell to "first run to first picture" (records without an item; added by p3-records-b)
- [[adr-thread-example-projects-and-linux-values]] — ADRs 0142, 0150, 0155, 0157, 0163, 0176: the shipped example projects, the Linux-built tree and the rotated aerofoil cases (p3-records-b)
- Runner records of wave 48 and ADR 0229 (0130, 0143, 0146, 0229) added to [[adr-thread-runner-records-and-run-sites]]; the Studio's time step (ADR 0153) on [[adr-thread-setup-warnings-and-time-step]] (p3-records-b)

## Items

<!-- generated by tools/gen_roadmap_pages.py: begin (do not edit inside) -->

Generated from `docs/roadmap.json` and `scripts/roadmap_progress.py` at 82985b47 on 2026-11-25.

**63 items:** 45 done, 13 partial, 5 not started.

| Item | Title | Status | ADRs | Milestones | Gap (what is missing now) |
|---|---|---|---|---|---|
| F-140 | Desktop app shell (rcfd-gui, eframe/egui): window, menu bar, light/dark theme, case open/save, in-process solver service | done |  |  |  |
| F-141 | Dockable panel layout (egui_dock): dock, split, float and hide/show panels; reset layout; layout restored on restart | done |  |  |  |
| F-142 | Settings-tree editor driven by the case JSON schema: typed widgets, enum combos, ranges, units, named-object add/remove, inline server validation errors | done |  |  |  |
| F-143 | Mesh and field viewport: 2D slice field raster with colour map and legend, cell-centre view, field selector (3D is F-151) | done |  |  |  |
| F-144 | Monitors panel: residual and report plots (log scale), run/pause/stop controls, run status and log tail, polled without blocking the UI | done |  |  |  |
| F-151 | 3D viewport (wgpu): surfaces, slices, iso-surfaces, vectors and picking; selection feeds assistant context | todo |  |  |  |
| F-261 | Studio project folder library (rcfd-project) | partial |  |  | The runner still keeps its own copies instead of using these types, and the other additive contract items listed in the ADR addendum are not done. |
| F-262 | Studio run service (rcfd-runner) | partial | 0742 |  | Socket control (files only) and runs started from the application are not done. Registered check S5 is an ignored known miss; the note does not give its number. |
| F-263 | Studio time-series field store (rcfd-tsstore) | partial |  |  | Residual and monitor series are not in the store yet, and the solver does not write to it. |
| F-264 | Studio wgpu render core v0 (rcfd-render) | partial |  |  | Registered check P2 (picking returns the scalar value) is an ignored known miss. Not done: cut planes, iso-surfaces, streamlines, particles, video. Open decision: the egui front end (eframe 0.36.2) needs Rust 1.95 while the workspace builds on an older toolchain. |
| F-266 | Studio application shell (egui on wgpu) | todo |  |  |  |
| F-293 | Studio follow-ups that wait on the maintainer's Studio worktree | todo |  |  |  |
| F-447 | Electron over the rcfd APIs: performance spike (control, live events, field transfer, scrubbing, WebGPU draw) | partial |  |  | Viability under ADR 0350 section 4 is not decided; it is the maintainer's call (questions QA1 to QA5 of addendum 1). The R3 WebAssembly reader was not run. The note says P3 is met after F-456 (82.6 ms against 100). |
| F-448 | Electron Studio to parity with rcfd-studio | todo |  |  |  |
| F-449 | Retire rcfd-studio and rcfd-gui | done |  |  |  |
| F-456 | monitors.get at 1e5 samples: build only what is asked for | done | 0353 |  |  |
| F-475 | Studio preview of a case with [mesh.adapt] shows the adapted mesh | done | 0377 |  |  |
| F-800 | Electron Studio: go-ahead and the apps/studio-web layout | done |  |  |  |
| F-801 | Electron Studio: hardened shell and test harness | done |  |  |  |
| F-802 | Electron Studio: design tokens and accessible primitives from DESIGN.md | done |  |  |  |
| F-803 | Electron Studio: service client for control and events | done |  |  |  |
| F-804 | Electron Studio: bulk field data by the R2 reader and the R1 route, bit for bit | done |  |  |  |
| F-805 | Electron Studio: project home and the live project folder | done |  |  |  |
| F-806 | Electron Studio: run list and run state | done |  |  |  |
| F-807 | Electron Studio: run control (quick run, stop, pause, queue) | done |  |  |  |
| F-808 | Electron Studio: field viewport on three.js WebGPU | done |  |  |  |
| F-809 | Electron Studio: derived pictures (iso-surfaces, streamlines, cut planes, vectors) | done | 0738 |  |  |
| F-810 | Electron Studio: run bands and linked time scrubbing | done |  |  |  |
| F-811 | Electron Studio: residual and monitor plots | done |  |  |  |
| F-812 | Electron Studio: case editor validated by the engine's schema | done |  |  |  |
| F-813 | Electron Studio: a learner's first path | partial |  |  | The maintainer's first-time view-and-report step is not done. |
| F-814 | Electron Studio: Claude Code, registered with Claude Code and followed live | partial |  | fan | No test runs a real Claude Code session against a registered folder, and the maintainer's view-and-report of the new registration flow is pending. |
| F-815 | Electron Studio: compare two runs | done |  |  |  |
| F-816 | Electron Studio: keyboard and assistive-technology pass | done |  |  |  |
| F-817 | Electron Studio: performance regression bench for the product shell | partial |  |  | An idle-machine baseline (Q4) is missing. P10 missed in three of four loaded runs (3.6 to 40.3 s against 2.5 s, cause not diagnosed) and P5 missed twice. |
| F-818 | Electron Studio: dark theme | done |  |  |  |
| F-819 | Electron Studio: large meshes and polyhedral fan meshes | done |  |  |  |
| F-820 | Electron Studio: installers, signing and updates | done |  |  |  |
| F-821 | Electron Studio: Linux and macOS | partial |  |  | The bench lines P5, P7 and P8 with the WebGL 2 fallback are not run (the bench scripts do not pass the GPU blocklist switch), P4 and WebGPU need a Linux machine with a GPU, and the weekly workflow has not run on GitHub yet. macOS was not asked for. |
| F-822 | Electron Studio: tutorials that name the GUI | done |  |  |  |
| F-823 | Electron Studio: npm tests in CI | partial |  |  | The job has not yet run green on GitHub Actions for the first time. |
| F-824 | Electron Studio: capability inventory from the engine and a UI coverage test | done |  |  |  |
| F-825 | Electron Studio: schema-driven case and settings surface | done |  |  |  |
| F-826 | Engine: UI metadata in the case schema (labels, units, ranges, advanced flag) | done | 0730 |  |  |
| F-827 | Electron Studio: one action layer for every UI action | done |  |  |  |
| F-828 | Electron Studio: provenance inspector and reproduce-from-CLI | partial | 0734, 0735 |  | Studio numbers without provenance remain: the Runs residual columns, the compared run's residuals, legend ticks, domain size, scale bar, runner budgets and home counts. |
| F-829 | Electron Studio: undo, safe deletes and edit conflicts | done | 0736 |  |  |
| F-830 | Electron Studio: explain in place from the engine's docs | partial |  |  | A learner's view-and-report step (G19) is not done. |
| F-831 | Electron Studio: command palette and keyboard shortcuts | done |  |  |  |
| F-832 | Electron Studio: responsiveness budget and cancellable work | done |  |  |  |
| F-833 | Electron Studio: quiet the built surfaces and lock a word budget per surface (G22) | done |  |  |  |
| F-834 | Engine tests that went with rcfd-studio, moved to rcfd-workspace | done |  |  |  |
| F-835 | Electron Studio: new project | done | 0731 |  |  |
| F-836 | Electron Studio: mesh preview before a run | done | 0732 |  |  |
| F-837 | Electron Studio: run setup, logs and field-view extras from rcfd-studio | done | 0733, 0737 |  |  |
| F-838 | Local-model assistant: keep, port or remove | done |  |  |  |
| F-848 | Runs on a remote site: a remote simulation service for the Studio, the CLI and the MCP server | todo | 0739 |  |  |
| F-852 | Studio: the end-to-end test "Close session cannot be undone" times out on Windows in CI | done | 0781 |  |  |
| F-888 | Studio: the end-to-end test of the quick run by keyboard fails where the Studio window lacks the system's focus | done | 0786 |  |  |
| F-890 | Studio: responsiveness.spec.ts:260 fails on the lead's machine with one task of 400 to 440 ms when the workspace tab opens | partial | 0787 |  | The cause is shown as a mechanism and fixed, and responsiveness.spec.ts:260 passed 10 of 10 runs alone and 3 of 3 whole runs. Missing: a record that this mechanism was the task of 400 to 440 ms. It needs the machine under 0.75 busy: the unchanged test on the tree before ADR 0787's fix must show the task again, and on the tree with it must pass. In the whole runs undo.spec.ts:268 and run-control.spec.ts:320 each failed once in three, outside this item. |
| F-891 | Studio: f837.spec.ts:198 fails in 2 of 6 whole runs, at two different lines | done | 0788 |  |  |
| F-892 | Studio: name the test that leaves an rcfd serve process running after npm test on Windows | done | 0789 |  |  |
| F-893 | Studio: run-control.spec.ts:320 does not find the pause request within 5 s in some whole runs, and a run-control test leaves its rcfd run solver running | done | 0790 |  |  |

<!-- generated by tools/gen_roadmap_pages.py: end -->
