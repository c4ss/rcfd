---
type: source
tags: [adr-thread, studio, rcfd-project, rcfd-runner, rcfd-tsstore, rcfd-render, rcfd-claude, gui, wave-47]
sources: [doc-ui-restart-plan, doc-architecture]
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: the Studio foundation crates of wave 47 (layers L1 to L6) and the earlier headless GUI checks

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread (registrations with result addenda, one audit ADR), 7 ADRs: wave 17C (headless GUI verification) and wave 47A to 47E plus the audit of 2026-10-27.

Wave 47 (2026-10-26) built the headless layers of the Studio plan of `docs/ui-restart-plan.md` ([[doc-ui-restart-plan]]) as separate crates, each before any UI: the project folder (L1, `rcfd-project`), the run service (L2, `rcfd-runner`), the time-series field store (L3, `rcfd-tsstore`), an offscreen wgpu render core (L4, `rcfd-render`) and the Claude Code bridge (L6, `rcfd-claude`). ADR 0128 then fixed sixteen audit findings across them and `rcfd-api`. The first three crates are still in the workspace; `rcfd-render` and `rcfd-claude` were removed later, when the Electron [[studio]] replaced the egui track (F-838; `rcfd-claude` on 2026-11-17) ([[doc-architecture]]). Wave 17C (ADR 0025) is the earlier headless check of the egui GUI and assistant.

## Current state

- **`rcfd-project` (F-261):** typed records, atomic replace, JSONL with torn-tail handling, path safety, a poll-based watcher, a migration hook and a query API; "Every later layer reads and writes that folder, so the crate is built first, headless, with no UI and no solver dependency" (ADR 0122). Recorded defect, not fixed there: a diverged run whose `residuals.jsonl` or `status.json` holds NaN "would make the whole residual file unreadable and drop the run into `skipped`". See [[rcfd-project]].
- **`rcfd-runner` (F-262):** one `rcfd-runner serve` per project launches each run as an `rcfd run` child and "uses plain files in the project folder as queue and transport, with no socket and no network port"; pause through the opt-in `rcfd run --pause-file f`; "a new runner adopts running solvers and never relaunches them" (ADR 0123). S5 stays an ignored known miss: "on Windows the runner inherits the client's inheritable handles, and ADR 0146 found no safe way around it on stable Rust"; ADR 0143 replaced the Windows start route. See [[rcfd-runner]].
- **`rcfd-tsstore` (F-263):** "one append-only file per run, `fields/store.rts`, plus an advisory `fields/index.json`"; the run-path hook is opt-in: with `output.store_interval` and `output.store_time_interval` both 0, `Hook::open` returns `None` (ADR 0124). See [[rcfd-tsstore]], [[field-store]].
- **`rcfd-render` (F-264, removed later):** offscreen wgpu core (surface meshes, colour maps, orbit camera, cut plane, picking), wgpu =30.0.1, all five adapters of the machine work including the WARP fallback (ADR 0125).
- **`rcfd-claude` (F-265, removed later):** spawns the Claude Code CLI; every chat process gets `--permission-mode dontAsk --permission-prompts none` plus a typed allow-list, and `check_args` refuses any argument containing `dangerously` or `bypassPermissions`; "The allow-list is a floor that the app controls, not the whole gate"; readers return `Unrecognised { kind }` for unknown lines (ADR 0126). Its allow-list rules moved into `crates/rcfd-mcp/tests/claude_allow/` ([[crate-graph]]). See [[claude-plugin]].
- **Audit (ADR 0128, F-261, F-262, F-265, F-866):** every finding reproduced (each new test failed on the pre-fix code and passes after the fix); amendment 1 withdrew the registered "stop forwarding after an abort" of finding 5 because D3 failed with it.

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0025](../../docs/adr/0025-wave17c-gui.md) | | registration + results | F-073, F-143, F-144, F-149, F-150 | Headless verification of the egui viewport, monitors, key storage and endpoints | one miss (V1 luminance, largest decrease 0.2126 luma, 8-bit quantisation); items partial | GUI removed (F-449) |
| [0122](../../docs/adr/0122-wave47a-project-folder-library.md) | 2026-10-26 | registration + results | F-261 | The project folder library (L1) | met; A2 control and A1 overlap minimum corrected by amendments 1 to 3 (ignored known misses kept) | |
| [0123](../../docs/adr/0123-wave47b-run-service.md) | 2026-10-26 | registration + results | F-262, F-786 | The run service (L2) | met; S5 ignored known miss (inherited handles on Windows) | start route by ADR 0143 |
| [0124](../../docs/adr/0124-wave47c-time-series-field-store.md) | 2026-10-26 | registration + results | F-263 | The time-series field store (L3) and its opt-in hook | met; one C7 row failed in the first CI gate (0.1262 s against 0.05 s), corrected set-up passes locally | |
| [0125](../../docs/adr/0125-wave47d-wgpu-render-core-v0.md) | 2026-10-26 | registration + results | F-264 | wgpu render core v0 (L4) | met except P1 (defective probe, P1b) and P2 value check (1.47e-5 against 1e-5) | crate removed (F-838) |
| [0126](../../docs/adr/0126-wave47e-claude-code-bridge-library.md) | 2026-10-26 | registration + results | F-265 | Claude Code bridge library (L6) | met, no known miss | crate removed 2026-11-17 |
| [0128](../../docs/adr/0128-audit-fixes-runner-claude-project-tsstore-render-api.md) | 2026-10-27 | audit fixes | F-261, F-262, F-265, F-866 | Sixteen audit findings across the wave-47 crates and `rcfd-api` | every finding reproduced and fixed; 16(b) listed only, finding 17 added | |

## Key numbers and limits

- Final gates: 0122 "2137 passed, 0 failed and 662 ignored"; 0123 "2126 passed, 0 failed and 662 ignored"; 0124 "2158 passed, 0 failed, 663 ignored"; 0125 "2153 passed / 0 failed / 664 ignored"; 0126 "2093 passed, 0 failed, 659 ignored"; 0128 run 3 on b8cf6eb "2216 passed, 0 failed and 666 ignored". See [[gate]].
- ADR 0123: gate run 1 failed 12 process tests because `powershell.exe` hung on the gate host; amendment 2 "calls the dependence on PowerShell a defect of the product and adds a direct-start fallback".
- ADR 0124 amendment 2: the CI miss explanation "is a hypothesis, not confirmed on this machine".
- ADR 0125: `Cargo.lock` gains 37 new packages, "no existing locked version moved"; MSRV: "eframe 0.36 needs 1.95, so the Studio track will need 1.95 in wave 48".
- ADR 0126: fields taken only as "observed on Claude Code 2.1.310"; "a session opened by another process between the open check and the spawn is not detected".
- ADR 0025: R2 rendered 1 M cells in 1.033 s (limit 2 s), "not generalised to other hardware".
- ADR 0122: "The kill checks test atomicity, not durability across power loss."

## Open questions

- ADR 0125: the MSRV 1.95 step "is for the lead or the user to decide" (the egui track was later dropped).
- ADR 0123: items "Not done here, for the lead" (section 8 contract files `pause`, `pause.ack`, `cancel` are additive, for the lead).
- ADR 0126: "The lead must test by hand what was not observed, including the non-Windows paths."

## Related

- [[studio]], [[rcfd-project]], [[rcfd-runner]], [[rcfd-tsstore]], [[field-store]], [[claude-plugin]], [[crate-graph]]
- [[adr-thread-founding-decisions]] (crate-creation table)
- [[roadmap-studio-desktop-app]], [[roadmap-claude-and-the-mcp-server]]
