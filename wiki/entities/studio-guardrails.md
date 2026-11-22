---
type: entity
tags: [studio, guardrails, process]
sources: [doc-studio-guardrails, doc-ui-restart-plan, doc-studio-coverage, doc-design, doc-impeccable-surfaces, adr-thread-studio-electron, adr-thread-studio-engine-entry-points, adr-thread-runner-records-and-run-sites]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Studio guardrails (G1 to G22)

**What it is:** the 22 fixed limits of the Electron [[studio]], approved by the maintainer on 2026-11-16 (G1-G14 as drafted, G15-G21 and G22 added the same day). Every Studio roadmap item names the guardrails it touches and its acceptance shows it keeps them; breaking one needs the maintainer's word and an ADR ([[doc-studio-guardrails]]).
**Where:** [docs/studio-guardrails.md](../../docs/studio-guardrails.md); `AGENTS.md` says Studio work also follows it.

## Details

| Id | Name | Rule (as the page states it, shortened) |
|---|---|---|
| G1 | Clean room | No layout, wording, icon, screenshot, workflow diagram or asset from commercial CFD software, manuals or help; nothing decompiled. "Knowing that a feature exists is fine; copying how it looks or what it is called is not." Fonts, icons, images original or OFL/MIT/Apache-2.0-compatible, listed in `THIRD_PARTY_LICENSES.md`. See [[clean-room]]. |
| G2 | Rust is the engine | No solver, mesher, physics, unit conversion or case validation logic in the Studio; numbers come through `rcfd-service/1`, the opt-in `data.*` route (R1), or the TypeScript reader of `store.rts` (R2, ADR 0350 addendum 1). Validation uses the engine's schema. |
| G3 | A picture can be wrong and look right | Every field value the renderer gets equals the Rust reader's value bit for bit (ADR 0350 P12), kept by a test; derived pictures match the engine's reference (`rcfd_post::render`) or a tolerance stated before the code; nothing decorative or made up. |
| G4 | Interfaces change only by addition | New service methods opt-in behind a minor protocol version; no existing method, reply shape, golden, CLI help text, MCP tool list or default changes without the maintainer; existing tests not edited; `baseline_hashes.rs` never re-recorded. New record fields (e.g. `started_by`, `studio_pid`) are additions. |
| G5 | Dependencies | npm deps pinned exactly with lock file, AGPL-compatible licence, justified in an ADR, count kept small; a new Rust crate, native Node addon or `wasm-bindgen` needs the maintainer; JS lives in `apps/studio-web/`, outside the Cargo workspace. |
| G6 | Honest about limits | Never suggest a capability that does not exist ("for example, stress and strain colouring needs a structural solver"); disclaimers stay visible; unverified values, known misses and failed runs stay visible; no invented customers, testimonials, benchmarks or validation claims. |
| G7 | Runs outlive the window | [[rcfd-runner]] owns runs; closing, crashing or reloading never stops, pauses or corrupts a run without the user's choice. Amended 2026-11-17: the Studio owns only a runner it started (`started_by: "studio"` + its `studio_pid`); idle owned runner shut down on close; with active runs the close asks (default Cancel); `--keep-runner` and a setting (off by default) keep the old behaviour. |
| G8 | Accessibility | No WCAG contrast requirement (removed 2026-11-16); colour never the only carrier of state; colour-blind-safe field maps; full keyboard reach, visible focus, reduced motion respected. |
| G9 | Performance does not regress below the spike | P2 solver event to renderer p99 <= 20 ms; P4 1e6-cell scalar to GPU buffer p50 <= 30 ms, p99 <= 60 ms; P5 scrubbing at 1e6 cells >= 10 snapshots per s, no frame > 150 ms; P7 first picture of the 1e6-cell surface <= 1.5 s; P8 1e6 triangles orbit 60 frames per s, p99 frame <= 25 ms; P10 cold start to project view <= 2.5 s (ADR 0350, maintainer's machine). |
| G10 | Electron security | `contextIsolation` on, `nodeIntegration` off, renderer sandboxed, strict CSP, no remote content, IPC allow-list via preload; service on loopback only unless the user says otherwise; remote run sites only as named in the user's sites file and confirmed once (added 2026-11-17, ADR 0739 Q3 (a)). |
| G11 | Claude Code | One writer per session; open sessions mirrored read-only; a closed one may be resumed and typed into; conservative allow-list, no "approve everything" (decisions of 2026-10-26, [[doc-ui-restart-plan]]). |
| G12 | The project folder is the contract | Read and write only as `docs/ui-restart-plan.md` §8 and ADR 0173 define it; edits made elsewhere show up live; no project state only inside the app ([[rcfd-project]]). |
| G13 | Testable without eyes | Each item has automated tests (unit, Playwright driving Electron) or a [[view-and-report]] step named in its acceptance; each wave ends with a view-and-report checklist. |
| G14 | rcfd-studio is retired | `rcfd-studio` and `rcfd-gui` deleted before parity on 2026-11-16 (F-449); no doc may tell a user to run them. |
| G15 | Everything the engine can do, the Studio can do | Every case-schema key, service method, MCP tool, CLI command, post-processing output and solver option reachable from the UI; inventory from the engine itself; a coverage test fails on a capability with no UI path and no dated exemption ([[doc-studio-coverage]]). |
| G16 | No UI-only capability | Every UI action is a named action calling the engine (service method, MCP tool or CLI command) and can be scripted; G15 read the other way. |
| G17 | Reproducible, with provenance one click away | A Studio run can be re-run from the CLI with the same result and the Studio shows the command; any number shows run id, snapshot, file, engine version and unit. |
| G18 | Non-destructive by default | Edits undoable; deletes recoverable or confirmed naming what is lost; never silently overwrite a file another writer changed; show the conflict. |
| G19 | Explain in place | Help on the spot from the engine's own docs, schema and glossary; every error names the problem and the way to recover. |
| G20 | Every action has a command | Every action is findable in a command palette, generated from the same action list as G16. |
| G21 | Never block | Engine work never on the renderer's main thread; long operations show progress and can be cancelled. |
| G22 | Implicit by default, explicit on exception | Normal state carried by pictogram, position and layout; visible words only for exceptions; per-surface budget of visible chrome words at rest, counted from the DOM by a test; raising it needs the maintainer. DESIGN.md calls it the Quiet Rule ([[doc-design]], [[studio-design-system]]). |

- Coverage status at the snapshot: 1070 of 1363 capabilities (78.5 %) have a UI path; 293 have exemptions, 215 of them pointing at an item whose acceptance does not cover them ([[doc-studio-coverage]]).
- Open for the maintainer: the UI framework and component approach; macOS (not asked); certificate type for signing ([[doc-studio-guardrails]]).

### How the guardrails were applied (P3, 2026-11-22)

- ADR 0800 (2026-11-16) generated the Electron roadmap from the guardrails, `PRODUCT.md` and `DESIGN.md`; every guardrail "that needs work, not only restraint, becomes an item or an acceptance clause (G3 picture checks, G7 kill-the-app test, G9 benchmark, G10 hardening, G13 test harness)", and G15 "makes coverage the organising rule of the UX" ([[adr-thread-studio-electron]]).
- G2, G4 and G16 in practice: each Studio gap that needed the engine became an engine ADR with an opt-in switch or a new name and a CLI or MCP twin (ADRs 0730 to 0738), "Defaults and `baseline_hashes.rs` are unchanged" in each ([[adr-thread-studio-engine-entry-points]]).
- G7 amended 2026-11-17 after the maintainer found `rcfd-runner serve` and a paused `rcfd run` still running when the Studio closed: "the runner should close with studio by default if no external (existing) runner env var is specified" (ADR 0801, unnumbered addendum "Runner closes with the Studio") ([[adr-thread-studio-electron]]).
- G9: the idle baseline of 2026-11-17 met every judged line; under load P10 missed in three of four runs (F-817) ([[adr-thread-studio-electron]]).
- G10: remote [[run-site]]s added 2026-11-17 (ADR 0739 Q3 (a)); `rcfd serve` refuses a non-loopback address unless `--allow-unauthenticated-remote` ([[adr-thread-runner-records-and-run-sites]]).
- G18: the project trash with restore and compare-and-set on `settings.set` (ADR 0736) ([[adr-thread-studio-engine-entry-points]]).
- G22: the word budget per surface, chosen by the maintainer as option (a) of three on 2026-11-16, "raising a budget needs the maintainer's word" (ADR 0800 §8); the build ADRs report each budget held ([[adr-thread-studio-electron]], [[adr-thread-studio-engine-entry-points]]).

> [!warning] Contradiction (within the repo docs)
> G11 as the table quotes it ([[doc-studio-guardrails]]) describes Claude Code sessions inside the Studio. ADR 0801 Addendum 23 (2026-11-17) removed the embedded panel; the Studio registers the project server with Claude Code and opens Claude Code in its own terminal, where "Claude Code's own permission prompts govern" ([[adr-thread-studio-electron]]).

## Related

- [[studio]]
- [[studio-design-system]]
- [[work-streams]]
- [[doc-process-agent-rules]]
- [[roadmap-studio-desktop-app]]
- [[adr-thread-studio-electron]]

## Open questions

- Whether `scripts/gate_test.py` also runs the Studio's npm tests is "the maintainer's decision" (G13) ([[doc-studio-guardrails]]); [[doc-process-lead-process]] has `push_main.py` run `npm test` in `apps/studio-web` when a merge touches it, which covers the push, not the full gate.
