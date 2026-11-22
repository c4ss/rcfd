---
type: source
tags: [adr-thread, studio, electron, performance, dependencies, claude-code]
sources: [doc-studio-guardrails, doc-product, doc-design, doc-ui-restart-plan]
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: the Electron Studio, from the performance spike to the roadmap and its addenda (F-447, F-448, F-449, F-456, F-800 to F-838)

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread: one measurement spike with a decision rule (ADR 0350), its follow-up registration (ADR 0353), the roadmap ADR of the Electron Studio (ADR 0800) and the dependencies ADR that carries the per-item addenda (ADR 0801, 25 addenda); 4 ADRs, 2026-11-12 to 2026-11-17. ADRs 0350, 0800 and 0801 are in the list of job p3-records-b, which may add to this page; ADR 0353 is this job's.

On 2026-11-12 the maintainer asked for "an ADR to test the performance of electron<>rcfd APIs and potentially retiring rcfd-studio", after the lead recommended a web-technology shell for designer freedom. ADR 0350 registered the checks P1 to P12 and a decision rule before any code. On 2026-11-16 ADR 0800 turned the one item F-448 ("Electron Studio to parity") into F-800 to F-832, generated from three authorities (guardrails, `PRODUCT.md`, `DESIGN.md`), and the maintainer gave the go-ahead. ADR 0801 then justified the 14 npm dependencies and the layout of `apps/studio-web/`, and became the place where each Studio item records what it built.

## Current state

- **The spike** (ADR 0350, F-447 partial): both bulk routes are "fast enough and exact": R1 (the opt-in `data.*` methods, `rcfd-data/1`, service protocol 1.2) and R2 (a Node reader of `store.rts`); P3 (`monitors.get` of 4 series of 1e5 samples) missed at 127 ms against 100 ms. The rule "has no outcome for this case; the decision goes to the maintainer"; viability "is the maintainer's call (questions QA1 to QA5 of addendum 1)" per the roadmap gap. P3 was met after F-456 (ADR 0353, median 82.6 ms). Electron was chosen over Tauri (Q5 (a), taken by the lead on the standing instruction of 2026-11-11).
- **The roadmap** (ADR 0800, accepted 2026-11-16): F-800 to F-832 (later F-833 to F-838, F-848, F-852), each tracing to guardrails and to `PRODUCT.md` / `DESIGN.md`. The maintainer said yes to F-448 (Q1), keep `apps/studio-prototype/` until F-448 passes (Q2), Studio npm tests in CI as a job of `.github/workflows/ci.yml` (Q3). Two changes of 2026-11-16: no WCAG contrast requirement; guardrail G22 "Implicit by default, explicit on exception" with a word budget per surface.
- **The egui apps are gone** (ADR 0800 §10): the maintainer chose to "kill the old rcfd-studio and clear up some test bandwidth": `crates/rcfd-studio` and `crates/rcfd-gui` deleted on 2026-11-16 (F-449 done ahead of F-448). F-838 (2026-11-17): the local-model assistant `crates/rcfd-assistant` and the wgpu render core `crates/rcfd-render` removed, then `crates/rcfd-claude`; "Cargo.lock: 216 to 83 packages", then "83 to 82 packages".
- **Dependencies** (ADR 0801, accepted 2026-11-16): fourteen direct packages, six shipped (`react` 19.3.1, `react-dom` 19.3.1, `three` 0.187.0, `@react-three/fiber` 9.8.2, `react-aria-components` 1.22.1, `@fontsource/archivo` 5.3.0) and eight for building (`electron` 45.3.3, `vite` 8.3.8, `@vitejs/plugin-react` 6.3.0, `typescript` 7.0.2, `@types/*`), exact pins, lock file committed (119 packages), every licence AGPL-3.0 compatible.
- **Claude Code** (ADR 0801 Addendum 23, 2026-11-17): the maintainer asked to "completely remove the custom claude code panel and find a way to register the mcp in claude code itself and it should update the project in electron in realtime as it calls the mcp". The embedded panel, its allow-list writing and the Rust allow-list twin were removed; the Studio registers `claude mcp add rcfd-project --scope local -- "<engine rcfd-mcp>" --project "<folder>"`, opens Claude Code in a terminal ("Open Claude Code here"), and shows one activity line from the change feed, telling Claude Code's changes from its own by `rcfd-mcp --client studio`. "This supersedes F-814's panel (the parts of Addenda 18 and 20 about it) and A20's writing of the allow-list file."
- **Engine found without setup** (Addendum 21): every launch finds `rcfd`, `rcfd-mcp` and `rcfd-runner` from one folder with nothing set (`--no-engine`, `--engine-dir`, the `RCFD_STUDIO_*` variables, else `<resources>/bin` or `target/release`); "Nothing is ever looked for on PATH". Attaching to another service is `--service <host:port>` or `RCFD_STUDIO_SERVICE`, loopback only today.
- **Runner closes with the Studio** (unnumbered addendum, 2026-11-17, amends G7): the Studio owns only a runner that came up while it had the project open; an idle owned runner is shut down on close; with active runs the close guard asks (Stop runs and close / Keep them running and close / Cancel, the default).
- **Performance bench** (ADR 0800 §9, F-817 partial): every limit met on the idle baseline of 2026-11-17 (P10 median 320 ms against 2500; P2 p99 0.64 ms against 20; P4 R2 13.8 / 20.6 ms against 30 / 60; P5 63.9 per s; P7 44.7 ms; P8 240 per s). Under full load P10 "ranged from 1.2 s to 40.3 s across four runs within 75 minutes ... Its cause was not diagnosed." The bench stays out of CI (Q3).

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0350](../../docs/adr/0350-electron-studio-performance-spike-and-rcfd-studio-retirement.md) | 2026-11-12 | plan + registration (spike), addendum 1 results | F-447, F-448, F-449 | Can Electron over the rcfd APIs control, follow, scrub and draw as well as `rcfd-studio`? | every check but P3 met by both routes; P3 127 ms against 100; no outcome under the rule, maintainer's call | 0800 (phase 2 plan) |
| [0353](../../docs/adr/0353-f456-registration-monitors-get-builds-only-what-is-asked.md) | 2026-11-12 | registration, amendments 2 to 4 (2026-11-20) | F-456 | `monitors.get {names, since}` builds only what is asked for | done: P3 median 82.6 ms <= 100, I1 0.30 ms <= 2, I2 1.17 ms <= 2; I2 re-defined 2026-11-20 (ADR 0750) | |
| [0800](../../docs/adr/0800-electron-studio-roadmap.md) | 2026-11-16 | plan (hub), answers, changes, bench results | F-448, F-449, F-800 to F-838 | The Electron Studio roadmap from guardrails, `PRODUCT.md` and `DESIGN.md` | accepted 2026-11-16; egui apps deleted; idle baseline met | |
| [0801](../../docs/adr/0801-studio-web-dependencies-and-layout.md) | 2026-11-16 | decision + 25 addenda (per-item records) | F-800 and most F-80x / F-81x / F-83x | Dependencies, layout, pin and update policy of `apps/studio-web` | accepted 2026-11-16; addenda to 2026-11-17 | Addendum 23 supersedes the panel parts of Addenda 18 and 20 |

Addenda of ADR 0801 (headings): 1 F-801 dependencies; 2 F-803 the service client; 3 F-804 bulk field data by R2 and R1; 4 F-811 plots; 5 F-808 field viewport on three.js WebGPU; 6 F-810 run bands and linked scrubbing; 7 F-809 derived pictures; 8 F-828 provenance; 9 F-832 responsiveness budget; 10 F-815 compare two runs; 11 F-833 the Quiet Rule sweep and word budgets; 12 F-816 keyboard and assistive technology; 13 F-837 run logs, runner budgets and field-view extras; 14 F-817b drawing on demand and a scratch profile per bench case; 15 F-819 large meshes and FAN-01; 16 F-819b three fixes at 4e6 cells; 17 F-818 themes; 18 the Studio-only leftovers of F-814, F-813 and F-828; the "Engine follow-ups of the Studio items" table; 19 F-820 and F-821 installers in CI and Linux in Docker; 20 the maintainer's answers of 2026-11-17; 21 engine found without setup; 22 viewport camera and playback; an unnumbered "Runner closes with the Studio"; 24 run sites in the Studio; 23 Claude Code registered with Claude Code itself.

## Key numbers and limits

- **G9 limits** (ADR 0350, carried into the guardrails): P2 p99 <= 20 ms; P4 p50 <= 30, p99 <= 60 ms; P5 >= 10 snapshots per s, no frame > 150 ms; P7 median <= 1.5 s; P8 >= 60 frames per s, p99 frame <= 25 ms; P10 median <= 2.5 s.
- **Spike results** (ADR 0350 addendum 1, Electron 45.1.0, RTX 5080, 60 Hz display, load 0.3 to 0.6): R1 loads a 1e6-cell scalar to a GPU buffer "in p50 25.9 ms, p99 57.7 ms; R2 ... in p50 14.2 ms, p99 22.4 ms; JSON (R0) takes 552 ms"; "0 differing bits in 672e6 values"; cold start median 0.38 s; "R3 (WebAssembly) not run: it needs `unsafe` exports or `wasm-bindgen`".
- **F-456** (ADR 0353): replies equal to the old code byte for byte, "0 differing of 3000 stores (E1) and of 594 requests through the call and through the wire (E2, E3)"; the control on the unchanged server took 151.4 ms. I2 was later judged on "an increment of 100 samples per series of a store that grew by pushes, with the reallocation outside the timer, limit 2 ms unchanged" (maintainer, 2026-11-20); the old I2 is an ignored record.
- **Large meshes** (ADR 0800 §9, F-819 and F-819b): at 4e6 cells the scrub went from "11.3 to 14.8 per s, longest frame 83 to 101 ms" to "20.1 to 20.2 per s, longest frame 51 to 66 ms" after the viewport took surface values (1.22 MiB instead of 15.3 MiB); "No polyhedral FAN-01 mesh exists".
- **Pin policy** (ADR 0801 §4): a new direct dependency or major needs an ADR entry; Electron supports its latest three stable majors, "so 45 stays supported until about the release of 48".
- **Claude Code registration** (Addendum 23): checked against "the real CLI (Claude Code 2.1.326) in a scratch folder"; `get` "takes about 1.6 s because it starts the server to check it"; the project home's feed poll is 400 ms.

## Open questions

- F-447: viability under ADR 0350 §4 and QA1 to QA5 are "the maintainer's call" (roadmap gap). QA1 was answered in effect by F-456.
- F-817: P10 under load not diagnosed; P5 missed twice under load (roadmap gap).
- ADR 0800 §9 Q1, Q2 and Q6 (P8's frame rate, P2 under load, the bench's window mode) were taken on their recommendations; only Q3, Q4 and Q5 are recorded as answered by the maintainer.
- Addendum 23 decisions A23-D1 to A23-D6 are "options, the recommendation taken; each reversible".

> [!warning] Contradiction (within the repo docs)
> Guardrail G11 in `docs/studio-guardrails.md` still reads "One writer per Claude Code session. Open sessions are mirrored read-only; the Studio may resume and type into a closed one" ([[doc-studio-guardrails]]). ADR 0801 Addendum 23 (2026-11-17) removed the embedded Claude Code panel and its session list: Claude Code now runs in its own terminal against the registered project server, and "Claude Code's own permission prompts govern".

## Related

- [[studio]], [[studio-guardrails]], [[studio-design-system]], [[claude-plugin]], [[rcfd-service-protocol]], [[field-store]], [[rcfd-tsstore]], [[view-and-report]]
- Threads: [[adr-thread-studio-engine-entry-points]], [[adr-thread-runner-records-and-run-sites]], [[adr-thread-workspace-layer]], [[adr-thread-mcp-tools]], [[adr-thread-wave48-egui-studio]] (the egui Studio this one replaced)
- Areas: [[roadmap-studio-desktop-app]], [[roadmap-claude-and-the-mcp-server]]
