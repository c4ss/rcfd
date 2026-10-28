# ADR 0130: Wave 48 overview, and 48A: one run-record contract for the runner, the Studio and the plugin

Status: **ACCEPTED 2026-10-28** (the maintainer accepted the ADR and took the recommended option of every open decision W48-1 to W48-16, scope cut (a)) (scoping only; no code written, nothing run). Written at main 1248a76. Wave 48 starts only after the user
has run the wave-47 result and reported on it (plan section 4); whether that has happened is not recorded in the repository
(UNVERIFIED). House rules of ADRs 0110-0129 apply to every wave-48 sub-wave: limits are fixed before the registered runs; a missed limit
becomes an ignored `*_known_miss` test with the limit intact; a defective set-up is corrected by an amendment registered before the
corrected run; defaults of existing code stay bit-identical; `crates/rcfd-api/tests/baseline_hashes.rs` is not re-recorded. Parent:
[`docs/ui-restart-plan.md`](../ui-restart-plan.md) sections 3, 4, 6 and 8. Roadmap: F-266 (application shell) and F-267 (plugin) cover
wave 48; F-261 to F-265 are its foundations.

This ADR has two parts. Part 1 is the wave-48 overview: sub-waves, order, parallel work, crates touched, the gate plan and the ordered
list of open decisions. Part 2 is sub-wave 48A, a headless prerequisite that the code reading showed is needed before any Studio
panel can list a run that the runner made.

## Part 1: wave 48 overview

### Why the split differs from the proposed 48A-48E

The proposed split was shell, viewport, run control, sidebar, plugin. Reading the wave-47 code showed that the libraries do not yet
agree on the run records they share. A run made by `rcfd-runner` cannot be read by `rcfd-project` (Part 2 Context). Every later
sub-wave reads those records, so a small record-contract sub-wave comes first and the others move one letter down:

| Sub-wave | ADR | Content | Roadmap |
|---|---|---|---|
| 48A | 0130 (this) | one run-record contract: runner on `rcfd-project` types, tolerant readers, store location, per-step monitors (opt-in) | F-261, F-262 notes; new items proposed below |
| 48B | [0131](0131-wave48b-studio-application-shell.md) | Studio application shell: window, project open/new, case/mesh/run browser, case editor, panels, file watching | F-266 |
| 48C | [0132](0132-wave48c-viewport-integration.md) | viewport: render core in the window, camera interaction, scalar display, time scrubbing over `rcfd-tsstore` | F-266, F-264 |
| 48D | [0133](0133-wave48d-run-control-and-live-monitoring.md) | run control UI and live monitoring through `rcfd-runner`: several runs, queue, pause, cancel, residual and monitor plots | F-266, F-262 |
| 48E | [0134](0134-wave48e-claude-code-sidebar.md) | Claude Code sidebar on `rcfd-claude`: sessions, mirror, closed-session resume, allow-list approvals | F-266, F-265 |
| 48F | [0135](0135-wave48f-plugin-and-project-tools.md) | Claude Code plugin: project tools in `rcfd-mcp`, skills, slash commands, edit conflicts | F-267 |

No separate plan file is written: no earlier wave has one (wave 47 starts directly with ADR 0122), so this overview lives here.

### Dependency order and parallel work

```text
48A (records) ──┬──> 48B (shell) ──┬──> 48C integration (viewport in the window)
                │                  ├──> 48D (run control UI)
                │                  └──> 48E integration (sidebar in the window)
                └──> 48F server part (project tools, no window) ──> 48F conflict part (needs the 48B editor)
48C library part (rcfd-render additions) and 48E library part (rcfd-claude additions) can start at once, in parallel with 48A.
```

* 48A first: the run list (48B), run control (48D) and the project tools (48F) all read runner-made run folders.
* After 48A, 48B and the 48F server part run in parallel: the MCP server has no window and does not depend on the Studio crate.
* The library parts of 48C (`Renderer::from_device`, scalar-only update) and 48E (`check_args` additions, allow-list file) touch only
  `rcfd-render` and `rcfd-claude` and can run from the start.
* 48C, 48D and 48E integration each need the 48B shell; they touch different modules of the Studio crate and can run in parallel
  after 48B, with merges in the order C, D, E (the viewport defines the panel layout the others dock into).

### Crates touched (all changes additive)

| Crate | Sub-wave | Change |
|---|---|---|
| `rcfd-project` | 48A, 48F | tolerant readers, open event kind, legacy field aliases; a cooperative edit lock helper (48F) |
| `rcfd-runner` | 48A | uses `rcfd-project` records (ADR 0123 "Switching to rcfd-project"); forwards per-step monitors |
| `rcfd-cli`, `rcfd-api` | 48A | opt-in flags only (`--store-dir`, `--status-monitors`, if decided); defaults byte-identical |
| `rcfd-render` | 48C | `Renderer::from_device`, render into a caller texture, scalar-only update |
| `rcfd-claude` | 48E | allow-list file, folder control-file check, MCP-config flags (if decided) |
| `rcfd-mcp` | 48F | project tools behind `--project`; the default tool list is unchanged |
| `rcfd-studio` (new) | 48B-48E | the application (if decision W48-3 keeps the plan's default) |
| `clients/claude-plugin` (new, not a crate) | 48F | plugin manifest, `.mcp.json`, skills |
| `crates/rcfd-cli/tests/workspace_graph.rs`, `docs/architecture.md` | 48A, 48B | layer numbers for `rcfd-runner`, `rcfd-claude`, `rcfd-studio` |

`rcfd-gui` stays frozen: it is not edited, keeps building on eframe 0.27.2 (glow) and keeps its tests (plan section 1, decision 2).

### Gate and test plan for the wave

* Each sub-wave is its own branch and ADR, gated as in [`docs/test-process.md`](../test-process.md) section 4 (fmt, workspace clippy
  with `-D warnings`, `scripts/gate_test.py` on a `git archive` copy, one slot). The full gate takes over an hour; nobody runs it on
  this machine while other gates hold both slots.
* Every GPU test runs on every adapter that `rcfd_render::adapters()` returns (five on the gate machine, ADR 0125 step 0), and prints
  the located `SKIP` line and passes when there is none. GitHub CI runners have no GPU adapter (UNVERIFIED for WARP on
  `windows-latest`), so CI exercises only the skip branch of GPU tests.
* No registered check needs a window. The Studio is tested as a model (state and decisions in non-UI modules, as `rcfd-gui` does,
  `crates/rcfd-gui/src/lib.rs`), plus headless egui frames, plus offscreen GPU renders compared with CPU references. Checks that need a
  window, a person or a real Claude Code session are listed in each ADR under "Not in CI" and are run by hand with the result recorded
  in the ADR addendum.
* Each sub-wave ends with a short view-and-report checklist for the user (plan section 5, limit 1).
* Build cost is reported, not limited: eframe and its window stack add to the 554 MB release target of `rcfd-render` (ADR 0125).
  The gate machine is memory-constrained when several gates run; the first Studio build is measured with `--timings` and reported.

### Open decisions of the whole wave, in the order the user should take them

Each is written out with options, trade-offs and one recommendation in the ADR named. None is decided here. Suggested order: W48-1,
W48-2, W48-3, then W48-16 (its answer decides whether W48-6 and W48-10 to W48-15 are asked in wave 48 at all), then the rest in
table order.

| # | Decision | ADR | Blocks | Recommendation (short) |
|---|---|---|---|---|
| W48-1 | Toolchain minimum for the Studio | 0131 | any 48B build | exclude `rcfd-studio` from the 1.90 type-check job; the crate declares 1.95 |
| W48-2 | Window and UI toolkit | 0131 | 48B | eframe on its wgpu back end, egui panels, `egui_dock` (the plan's L5 default) |
| W48-3 | New crate or extend `rcfd-gui` | 0131 | 48B | new crate `rcfd-studio` (the plan's default, decision 2 of section 1) |
| W48-4 | Where the fields store of a run lives | 0130 | 48C scrubbing | opt-in `--store-dir` so the store is at `runs/<id>/fields/` as the contract says |
| W48-5 | Do Studio-started runs store fields by default | 0130 | 48C scrubbing | yes, the start dialog shows and sets `output.store_interval` |
| W48-6 | Per-step report monitors | 0130 | 48D parity | opt-in `--status-monitors` on `rcfd run` |
| W48-7 | Viewport route into the window | 0132 | 48C | shared device, render into a texture shown by egui |
| W48-8 | Field update when scrubbing | 0132 | 48C budget | additive scalar-only buffer update |
| W48-9 | Reuse of the frozen GUI's logic modules | 0131 | 48B | copy into the Studio with a source note; leave `rcfd-gui` untouched |
| W48-10 | Project-folder files that change what `claude -p` runs | 0134 | 48E | refuse to start a chat until the user confirms the folder; pin MCP servers with `--strict-mcp-config` |
| W48-11 | Where the allow-list lives and what is on it at first | 0134 | 48E | per project in `.rcfd/claude-allow.json`; read-class project tools listed, nothing else |
| W48-12 | Where the project tools live | 0135 | 48F | `rcfd-mcp` with a `--project` mode, CPU rendering only (no wgpu in the MCP server) |
| W48-13 | How the plugin finds the server binary | 0135 | 48F | `rcfd-mcp` on PATH, project from `${CLAUDE_PROJECT_DIR}` |
| W48-14 | Plugin packaging and distribution | 0135 | 48F | new `clients/claude-plugin` named `rcfd`, in-repo marketplace, no binaries |
| W48-15 | Edit conflict policy | 0135 | 48F, 48B editor | base-hash compare-and-replace under a cooperative lock; the user resolves a conflict |
| W48-16 | Scope cut line of wave 48 | 0130 | planning | below |

**W48-16, scope cut line (Decision needed).**
* (a) Everything in the six ADRs, including the parity checks of plan section 6 that need 48A's monitors.
* (b) As (a), but the parity check "live report monitors" is cut from wave 48 (residuals only) if W48-6 is declined, and parity is
  completed in wave 49.
* (c) A minimum wave: 48A, 48B, 48C and 48F server part; run control, the sidebar and conflicts move to wave 49.

Trade-offs: (a) gives the user the whole loop (start runs, scrub, chat, tools) for the first view-and-report, at the cost of six branches
before the user sees anything; (c) shows something sooner but leaves the Claude Code integration, which the plan names as a goal, for
later and pushes wave 49's compute rendering back. **Recommendation: (a)**, with the user seeing 48B plus 48C first (a window that opens a
project and shows a field) as an interim view-and-report before 48D-48F merge. In every option `rcfd-gui` is not removed in wave 48:
its removal is a separate, explicitly requested step after the parity tests pass (plan section 1).

### What wave 48 leaves for later

* Wave 49: iso-surfaces, streamlines and particles (compute), vectors, filled cut sections (`rcfd_post::surfaces::cut_plane` as a
  surface), animation and video export, side-by-side compare of two runs at one time, a 3D render tool for the plugin.
* Wave 50: computer-fan modes (tolerance spread, dust field, noise and vibration spectra, design ledger and Pareto view); stress and
  strain only after a structural solver exists.
* Not planned in any wave yet: removal of `rcfd-gui`; an Agent SDK sidecar for approvals (only if the allow-list proves too narrow,
  plan section 8); Metal or Linux packaging of the Studio; pause by checkpoint (ADR 0123 limits).

## Part 2: sub-wave 48A, one run-record contract

### Context (read at main 1248a76)

* `rcfd-runner` keeps its own record types (`crates/rcfd-runner/src/records.rs`). Its `RunRecord` (line 107) has `case_source`,
  `case_dir`, an optional `solver_version` and typed `settings`. `rcfd-project`'s `RunRecord` (`crates/rcfd-project/src/records.rs`
  line 45) requires `case` and a `String` `solver_version`.
* `Project::run_summary` (`crates/rcfd-project/src/project.rs` line 328) parses `run.json` into that type, and `Project::runs` (line 360)
  puts any run whose summary fails into `skipped`. Read, not run: a run folder made by the runner has no `case` field, so every
  runner-made run would be listed as skipped, and the Studio run list built on `rcfd-project` would be empty. This is the merge step
  that ADR 0123 section "Switching to rcfd-project" lists and that the F-261 note records as not done.
* The runner writes events as `{"event": "<kind>", ...}` (`crates/rcfd-runner/src/runner.rs` lines 246, 364, 470, 606, 677) with kinds
  `state`, `launched`, `adopted`, `estimate`, `solver`, `cancel_requested`, `killed`, `runner_shutdown`, `skipped_line`.
  `rcfd-project`'s `RunEvent` is `{t, kind}` with a closed `EventKind` of `state`, `warning`, `stop` (`records.rs` line 95), so
  `Project::read_events` fails on a runner-written line.
* `rcfd-project`'s `RunStatus` has no `reason` or `exit_code`; unknown fields are ignored by serde, so a status written by the runner
  parses but loses the failure reason that the Studio must show.
* ADR 0122 final gate: serde_json writes NaN and infinity as `null` and refuses `null` for an `f64`. A diverged run's
  `residuals.jsonl` or `status.json` therefore becomes unreadable. The runner copies the solver's values (`null` when non-finite,
  ADR 0123 note).
* The runner passes `--out runs/<id>/output` (`runner.rs` line 581). The tsstore hook writes `<out_dir>/fields/` (ADR 0124 D8), so a
  runner-started run would put its store at `runs/<id>/output/fields/`. The plan section 8 contract says `runs/<id>/fields/`.
* The hook is opt-in (`output.store_interval`, `output.store_time_interval`, both 0 by default; `crates/rcfd-api/src/fieldstore.rs`),
  so a run started without those keys has nothing to scrub.
* The solver status stream's `iteration` event carries `index`, `time`, `res_u`, `res_p`, `continuity`, `done` only
  ([`docs/run-control.md`](../run-control.md) "Status stream"). The runner writes `monitors: {}` in every residual line (ADR 0123
  decision 9). Report values exist per simulation (`Simulation::report_values`, `crates/rcfd-api/src/sim.rs` line 3876) and are written
  once at the end (`monitors.json`, `runctl.rs` line 518). The plan section 6 parity check "run with live residual and report
  monitors" cannot pass on the runner path without a change.
* `crates/rcfd-cli/tests/workspace_graph.rs` line 83: `layer()` panics on any crate not in its table, and it is called for every
  dependency. `rcfd-runner` and `rcfd-claude` are not in the table today because they have no rcfd dependency.

### Decisions (proposed; the open ones are marked)

1. **D1 runner on `rcfd-project`.** Carry out ADR 0123's five switching steps: `rcfd-runner` re-exports `rcfd-project`'s records,
   `FsStore` is implemented on `rcfd-project`'s helpers (`atomic`, `jsonl`, `runid`), `check_id` becomes `runid::is_run_id`, the
   FNV, civil-date, atomic-write and run-id copies listed in ADR 0128 finding 16b are removed. The runner keeps its file transport,
   budgets and process model unchanged.
2. **D2 additive record changes in `rcfd-project`** (the contract grows; nothing is renamed on disk):
   * `RunRecord`: `case` gains `#[serde(alias = "case_source")]` so runs written by the current runner stay readable; new optional
     `case_dir`; `solver_version` becomes optional on read (a missing or null value reads as "unknown"); `settings` stays free JSON.
     The runner writes `case` from now on (and keeps writing `case_source` for one release so an older Studio can read it).
   * `RunStatus`: optional `reason` and `exit_code`.
   * Snapshot hash spelling: the runner writes `case_snapshot_hash` as `fnv1a64:<hex>`, `rcfd-project` as `fnv64:<hex>`
     (`records.rs` line 133); both are the same FNV-1a 64 (ADR 0128 finding 16b). `validate` (which recomputes the snapshot hash,
     ADR 0122 V1) accepts `fnv1a64:` as a legacy spelling of `fnv64:`; the runner writes `fnv64:` from now on.
   * `EventKind`: a catch-all `Other { kind: String, body: Value }` for kinds it does not model. One serde internally tagged enum
     cannot take two tag names, so the reader is a small custom deserializer that takes the tag from `kind` (current `rcfd-project`)
     or, if absent, from `event` (current runner). Mapping of the runner's lines: `{"event": "state", "state": S, "reason": R}` becomes
     `State { from: None, to: S }` with `reason` kept in a new optional `reason` field of `State`; every other runner kind becomes
     `Other` with its body intact. From now on the runner writes `kind`, with `from` and `to` for state changes and its own kind names
     for the rest.
   * Tolerant numbers: `ResidualLine` values, `RunStatus.time` and `Metrics` read `null` as NaN (the proposed fix of ADR 0122's
     final gate). Writers are unchanged (serde_json still writes `null`).
3. **D3 the store location (Decision needed, W48-4).**
   * (a) Accept `runs/<id>/output/fields/` and amend the contract additively: readers look at `fields/` first, then `output/fields/`.
     No solver change; the contract then has two places.
   * (b) An opt-in `rcfd run --store-dir <dir>` (default: `<out>/fields`, today's behaviour) that the runner sets to `runs/<id>/fields`.
     The contract holds as written; one more CLI flag in [`docs/run-control.md`](../run-control.md).
   * (c) The runner passes `--out runs/<id>` instead of `runs/<id>/output`. The solver's `solution.vtu`, `monitors.json` and
     `report.html` then land next to `run.json`, mixed with the contract files, and S6/S7 of ADR 0123 change their paths.

   Trade-offs: (a) is free now but every reader (Studio, plugin, later tools) must know two places forever; (c) changes registered
   tests' paths and clutters the run folder. **Recommendation: (b)**, it keeps the fixed contract and changes no default.
4. **D4 store on by default for Studio-started runs (Decision needed, W48-5).**
   * (a) The Studio's start dialog proposes `output.store_interval` (steady) or `output.store_time_interval` (transient), shows the
     value and passes it through `RunSettings.set` when the case does not set it; the user can switch it off per run.
   * (b) Only cases that set the keys themselves store fields; the Studio says "no stored fields" for others.
   * (c) Ask on every start.

   Trade-offs: (a) makes scrubbing work for every run the Studio starts but adds disk use (ADR 0124 C7: 674 MB for 1M cells, 20
   fields, 3 snapshots, so about 11 MB per field and snapshot in f64); the solver results are unchanged by the hook (ADR 0124 H1,
   bit-identical on and off). (b) keeps runs lean but most runs cannot be scrubbed. **Recommendation: (a)**, with the four default
   fields (ux, uy, uz, p), at most 100 snapshots per run and the lossy `Precision::F32` mode (ADR 0124 D4, error bound registered there)
   for Studio-started runs. Estimated size at 1M cells: 4 x 100 x about 5.6 MB, about 2.2 GB (f64 would be about 4.5 GB). The numbers are
   a proposal derived from C7, not a measured limit; the estimated size is shown before the start and the user can lower the count or
   pick f64. Whether the run-path hook exposes the precision as a case key is to be checked at implementation (ADR 0124 D8 lists only
   the interval and field keys); if not, an opt-in key `output.store_precision` is part of this decision. `rcfd run`'s own defaults
   are untouched.
5. **D5 per-step monitors (Decision needed, W48-6).**
   * (a) An opt-in `rcfd run --status-monitors` adds `monitors: {name: value}` to each `iteration` event of the status stream; the
     runner always passes it and forwards the values into `residuals.jsonl`. Without the flag the stream is byte-identical except
     `wall_s` (already non-deterministic).
   * (b) No stream change: the Studio shows report values only at the end, from `output/monitors.json`; the parity check is cut
     (W48-16 (b)).
   * (c) The runner re-runs reports from stored snapshots after the run (costly, needs the store).

   Trade-offs: (a) costs one evaluation of every report per step for runs that opt in (cost UNVERIFIED; a force report integrates over
   one patch, so it is small against a pressure solve, but not measured); (b) loses the convergence view of monitors that the frozen
   GUI has today (`crates/rcfd-gui/src/monitors.rs`). **Recommendation: (a)**, with the per-step cost measured as a reported (not
   limited) value in the addendum.
6. **D6 layer table.** `rcfd-runner` moves to layer 2 (it now depends on `rcfd-project`, layer 1) and `rcfd-claude` gets layer 0 (no
   rcfd dependency; ADR 0134 proposes moving it to layer 2 when it gains `rcfd-project`), in `workspace_graph.rs` and in
   [`docs/architecture.md`](../architecture.md). The graph stays acyclic: neither
   crate depends on anything above layer 1.

### Placement

`rcfd-runner -> rcfd-project -> rcfd-core`. No new third-party dependency. `rcfd-cli`/`rcfd-api` get opt-in flags only, if D3 (b) and
D5 (a) are chosen.

### Registered checks (to be fixed in this ADR before any run; numbers below are the proposal)

| Id | Check | Limit | Reason | Negative control (must fail) |
|---|---|---|---|---|
| A1 | A project with 6 runner-made runs (finished, failed by kill, cancelled queued, cancelled running, paused, running) made by `rcfd-runner serve` started directly (as S8) | `Project::runs()`: 6 listed, 0 skipped; `validate()` empty; each summary's state equals `status.json`; failed and cancelled runs carry `reason` | the contract | the same folder read with today's `rcfd-project` reader: skipped = 6 (shown on the pre-change commit, as ADR 0128's fails-before rule) |
| A2 | Every event kind the runner writes (9 kinds above) plus the 3 `rcfd-project` kinds, in both tag forms | each parses: runner `state` lines into `State { from: None, to, reason }`, the 3 `rcfd-project` kinds into their variants, the 8 other runner kinds into `Other` with the body intact; re-serialised lines parse to equal values | additive contract, D2 mapping | a closed reader (today's `EventKind`) fails on at least one runner line |
| A3 | Non-finite values: a residual line, a status and metrics with `null` | read as NaN; the run is listed, not skipped; finite lines bit-exact as before | ADR 0122 found defect | today's reader: the run is skipped |
| A4 | Legacy runs: a run folder written by main 1248a76's runner (fixture copied from a real run) | listed, `case` taken from `case_source`, `solver_version` unknown; `validate()` returns 0 issues (the `fnv1a64:` hash accepted) | old folders stay readable | without the alias: skipped; without the legacy hash spelling: one snapshot issue |
| A5 | ADR 0122 R1-T1, ADR 0123 S1-S11 (S5 stays a known miss), ADR 0128 findings 1-17 tests | pass unchanged | no registered limit moves | (their own controls) |
| A6 | `--store-dir` (only with D3 (b)): H1-H4 of ADR 0124 unchanged; with the flag the store is at the given folder, bit-exact to the store written without it | bit-exact; without the flag byte-identical files | default unchanged | the store written to `output/fields` is not found at `runs/<id>/fields` |
| A7 | `--status-monitors` (only with D5 (a)): cavity case with the `lid_drag` report (`crates/rcfd-mcp/src/lib.rs` template) | the last iteration's `monitors.lid_drag` equals `monitors.json`'s final value bit-exactly; without the flag the stream equals the old one after removing `wall_s`; `baseline_hashes` passes | additive, deterministic solver (ADR 0123 S6) | the stream without the flag has no `monitors` key |
| A8 | `workspace_graph` (3 tests) with the new layer numbers | pass | acyclic graph | a deliberately wrong layer for `rcfd-runner` (layer 1) fails `layering_matches_architecture_doc` |

### What I already knew (rules c, d)

Nothing has been implemented or run. The A1 and A3 failure modes are read from the code, not observed; the rule-d smoke of the
implementer comes first and may change set-ups, which is then disclosed here before registration.

### Out of scope

Socket control of the runner, measured memory enforcement, pause by checkpoint (ADR 0123 limits), residual series inside the
time-series store (F-263 note), the Unix code paths of the runner on a Unix host.

### Risks

* The runner rewrite touches the crate whose kill tests found host faults before (PowerShell hangs, ADR 0123 amendments 2 and 3).
  Mitigation: the new tests start `serve` directly, as ADR 0128 does.
* Writing both `case` and `case_source` for one release is a temporary duplication; its removal needs a later ADR.

### Proposed roadmap items (ids to be assigned by the maintainer)

* Run-record unification of runner and project library (now inside the F-261/F-262 notes; proposed as its own item so it can close).
* Non-finite values in run records read as NaN (ADR 0122 defect).
* Opt-in store directory for `rcfd run` (W48-4 (b)).
* Opt-in per-step report monitors in the status stream (W48-6 (a)).
* Studio toolchain minimum (W48-1, ADR 0131).

## Amendment 1 (2026-10-28, 48A, before any registered run): set-up corrections

Written by the 48A implementer after reading the code and before running A1-A8. What I knew: the code reading below; one real run
made with main 3492c75's `rcfd-runner` for the A4 fixture; the status stream of the A7 case recorded once with main 3492c75's
`rcfd`. `git diff 1248a76 3492c75` touches no file of `rcfd-runner`, `rcfd-project`, `rcfd-cli`, `runctl.rs` or `fieldstore.rs`, so
those binaries are 1248a76's code. No 48A code had run. Limits are unchanged.

1. A5 says the earlier tests pass unchanged; D2 and D1 make that impossible for the lines below, which are edited (pre-declared, as in
   ADR 0128):
   * ADR 0122 R1 negative control "an unknown event kind is refused" (`{"t": "t", "kind": "exploded"}`) contradicts D2's catch-all
     `Other`. Replaced by two controls that the open reader still refuses: a line with neither `kind` nor `event`, and a `state` line
     with an unknown state (`"to": "done"`). R1's other controls are kept.
   * Struct literals of `RunRecord` (R1), `RunStatus` (R1, Q1, `listing_budget.rs` L1, `case_path_wfix.rs` R9) and
     `EventKind::State` (R1, Q1) gain the new optional fields as `None`. No assertion changes.
   * ADR 0123 S10 asserts the `fnv1a64:<hex>` spelling in `run.json`; D2 has the runner write `fnv64:`. S10 asserts
     `fnv64:<the same 16 hex digits>`. `rcfd_runner::records::fnv1a64` stays as a function, now calling `rcfd_core::hash::fnv64`.
   * ADR 0123 unit test `civil_dates_and_id_format` calls the runner's private `civil`, a copy that D1 removes. It calls
     `rcfd_project::time::civil_from_days` with the same three dates.
2. S1-S11, F3 and F4 read runner events by their `event` key (`launched`, `adopted`, `runner_shutdown`, `estimate`, and `state` with
   a `state` field), and F3 writes a `launched` line that has only `event`. So that they stay unchanged, the runner writes both tag
   forms for one release, as D2 does for `case` and `case_source`: every line has `kind` and `event` with the same name, and state
   lines have `from`, `to` and `state`. The reader takes `kind` when present, else `event`. The runner reads both.
3. `#[serde(alias = "case_source")]` fails with "duplicate field" on a record that holds both `case` and `case_source`, which D2 has
   the runner write. The alias is implemented with a wire struct instead: `case` if present, else `case_source`.
4. D2 does not say what the runner writes into `case`. It writes the project-relative path (`cases/a.toml`) when the case file is
   inside the project, else the absolute path; `case_source` keeps the absolute path as before. Reason: `latest_finished` matches
   project-relative paths.
5. A2's "runner `state` lines into `State { from: None, to, reason }`" is the legacy `event` form. The new `kind` form carries
   `from` (the state read before the change). The 3 `rcfd-project` kinds parse into their variants in both tag forms.
6. A4 fixture: 20 steps of the 12 x 12 cavity, submitted by a copy of `rcfd-runner` with no `rcfd` next to it, so `solver_version`
   is null as A4's limit expects ("unknown"), and run by `serve`. Copied: the run folder without `output/` (solver results, which
   no reader of the contract reads).
7. A7 "without the flag the stream equals the old one": compared once on this host with the recorded pre-change stream (same case,
   path and flags) after removing `wall_s`. The CI test compares the streams without and with the flag after removing `wall_s` and
   `monitors`, and checks that the stream without the flag has no `monitors` key.
8. A1's negative control runs on main 3492c75 with only the new test file and a dev-dependency on `rcfd-project` added (the seam).
   The test reads `reason` from `status.json` as JSON so that it compiles there.
9. D5: the solver already evaluates every report after each step and appends it to its monitor store (`sim.rs`, end of `step`),
   which `monitors.json` is written from. `--status-monitors` copies the current step's value from that store, so it adds no report
   evaluation. The per-step cost is still measured and reported.
10. D4 asks to check whether the hook exposes the precision: it does not (`fieldstore.rs` writes `Precision::F64`, no key). An
    `output.store_precision` key is not added in 48A, because the Studio applies W48-5 later; recorded as a question for the
    maintainer.

## Files

This ADR. Implementation (48A): `crates/rcfd-project/src/records.rs`, `crates/rcfd-runner/src/{records,store,runner,client}.rs`,
`crates/rcfd-runner/Cargo.toml`, `crates/rcfd-cli/tests/workspace_graph.rs`, `docs/architecture.md`, and, if decided,
`crates/rcfd-cli/src/main.rs`, `crates/rcfd-api/src/{runctl,fieldstore}.rs`, `docs/run-control.md`. New tests in new files.
