# ADR 0808: F-981 registration: retire G15's coverage map and the learner's first path

Status: **REGISTERED 2026-11-26, written before any code of F-981 and before any run of its checks.** Before this text no
Studio or test file of F-981 was changed and no check was run. What was done: reading (below), searches of the source
tree, and, on the unchanged tree, a release build of `rcfd`, `rcfd-mcp` and `rcfd-runner` into the scratch target
(`CARGO_TARGET_DIR=E:/projects/cfd/w49i-work/f981-retire-hand-navigation-scratch/target`) and `npm ci` in
`apps/studio-web` (neither is a check). Results go in addenda at the end; nothing above them is changed afterwards.
Written by Claude Opus 5.5 (`claude-opus-5-5`), as a background agent of the lead.

It answers the brief `docs/adr/briefs/f981-retire-hand-navigation.md` ([the file](briefs/f981-retire-hand-navigation.md))
and follows [docs/process/agent-rules.md](../process/agent-rules.md), version 5. Branch `f981-retire-hand-navigation`
from main 5bf77c58, scratch `E:/projects/cfd/w49i-work/f981-retire-hand-navigation-scratch`. The decision it carries out
is the maintainer's answer 1 of 2026-11-26 in [ADR 0803](0803-the-studio-as-claudes-canvas-record.md): the views stay
and hand navigation goes; G15 and its coverage test are retired with the menus, editors and navigation built for it.

## What is asked

Roadmap item F-981 (deps F-976), the binding acceptance, read with a script:

> Every limit not stated here is registered in the ADR before the run. The coverage test of guardrail G15 and the Studio's hand-only surfaces (menus, editors and navigation whose only purpose is to reach a capability by hand), each listed in the registration with its file, are removed or retired; every test retired is named in the ADR with its reason (the maintainer's answer of 2026-11-26, ADR 0803). The views the canvas uses and their tests stay and pass; npm test passes.

Clauses: **c1** every limit registered here before the run; **c2** the coverage test of G15 and the hand-only surfaces,
each listed with its file, removed or retired; **c3** every retired test named with its reason; **c4** the views the
canvas uses and their tests stay and pass; **c5** `npm test` passes. The brief adds the rows: the canvas's e2e test and
the views' tests pass; the inventory test against the engine still passes; the coverage report no longer requires a UI
path and the inventory still lists every capability; the Studio opens a project and shows a view by `show_view`.

## What was read

`AGENTS.md`, `docs/process/agent-rules.md` (version 5) and the brief, whole; [ADR 0803](0803-the-studio-as-claudes-canvas-record.md)
whole; [docs/studio-guardrails.md](../studio-guardrails.md) whole (G15 and G19 retired, G16, G20, G23); the head and the
findings of [ADR 0805](0805-f976-registration-a-view-specification-and-show-view.md) (style, and that the inventory holds
`mcp:show_view` and `mcp:read_view` after the lead's re-record); the F-981 entry of `docs/roadmap.json` (by script). Of
`apps/studio-web`: `tests/unit/capabilities.spec.ts` whole; `tests/support/capabilities.ts` (outline, the coverage and
report part whole); `scripts/write-capabilities.mjs` whole; the head of `capabilities/coverage.json` and of
`docs/studio-coverage.md`; `src/renderer/main.tsx` whole (the routes); of `src/renderer/project/ProjectHome.tsx` the
imports, `NoProject` and the places the first path is drawn; `src/renderer/canvas/Canvas.tsx` (head and imports); of
`src/renderer/first/FirstPath.tsx` the head, the exports, `Explain` and `NotAProjectYet`; of `first/first.css` the rules
of `.fp`, `.fp-title`, `.fp-actions`, `.fp-explain*`, `.fp--folder`, `.fp-missing`, `.fp-next`; `tests/e2e/canvas-f976.spec.ts`
whole; of `tests/e2e/first-path.spec.ts` the head, the helpers and the cases 1, 2 and 6; of `tests/unit/first-path.spec.ts`
the head, the imports and the G16 block; the first-path parts of `tests/e2e/a11y.spec.ts`, `tests/e2e/word-budget.spec.ts`,
`src/shared/word-budget.ts`, `tests/e2e/provenance-f828.spec.ts` (KF), `tests/unit/provenance.spec.ts` and
`tests/unit/help-engine.spec.ts`; of `scripts/process_report.py` its `AREA_PAGES`; `playwright.config.ts`,
`tests/support/rcfd.ts` and `tests/support/project.ts` (where the binaries are found). Bytes read: about 230 000.

## Findings of the reading (before any code)

1. **The coverage test of G15** is the test "every capability has a UI path or a recorded, dated exemption that names a
   roadmap item (G15)" of `tests/unit/capabilities.spec.ts`, with its checker `checkCoverage` in
   `tests/support/capabilities.ts`, the hand-kept map `capabilities/coverage.json` (1 375 capabilities, 1 072 with a UI
   path, 303 exempted) and the UI and exemption columns of the generated page [docs/studio-coverage.md](../studio-coverage.md).
   The inventory itself (`capabilities/inventory.json`, F-824) is read from the engine by `readEngine` and is independent
   of the map; `scripts/process_report.py` names the page for the line "Studio desktop app" (by path only), so the page
   stays, as the inventory's report.
2. **The canvas reuses** (`Canvas.tsx`): the project home's `CasesTable`, `MeshesTable`, `RunsTable` and `changedAt`; the
   run workspace (`RunWorkspace`, `useSeriesMap` of the time panel); `FieldViewport`; `PlotFrame` and the plot styles;
   `runRows` of runs; the data door. Its e2e test opens a project through the empty home's "Open project folder…" and
   checks the canvas against the open project's view (`.ph-open`). None of these is hand-only.
3. **The learner's first path** (`src/renderer/first/`, F-813; ADR 0803 retired F-813 as "a learner's first path through
   the menus") has three parts in one file: `FirstPathIntro` (the four stations on the empty home), `FirstPath` (the
   make, run, watch and result steps on an empty project) and `NotAProjectYet` (the notice for a folder the engine refused
   as no project, with "Make a project here", the engine's `rcfd project init`, F-835). The first two lead a person by
   hand through the menus to a first result; the third is an exception notice (G6, G22) whose button is a named action.
   The path's run action `first.run_example` lives in `src/shared/actions/first.ts` and is dispatched by run control
   (`src/main/project/control.ts`); it is a named action of G16 and stays.
4. **The case editor (`#/case`) and the settings surface (`#/settings`)** are editors, and the settings surface is the
   "schema-driven surface" G15 asked for. But they carry G18's undo, conflict and close-guard behaviour, and the specs
   that test it (`undo.spec.ts`, `undo-case.spec.ts`, `undo-session-events.spec.ts`, `close-guard.spec.ts`, `f837.spec.ts`,
   `case-editor.spec.ts`, `case-file-adr0737.spec.ts`, `settings*.spec.ts`, and parts of `commands.spec.ts`,
   `theme-surfaces.spec.ts`, `word-budget.spec.ts` and `a11y.spec.ts`), and `error-walk.spec.ts` drives the case editor
   while F-823 (ADR 0809) registers that walk under WebGPU at the same time. Whether they are hand-only is a question of
   scope (rule 6): it is left open below, and both stay in this registration.
5. **The engine binaries** of the main checkout's `target/release` are of 17 and 18 November (without `show_view`), and
   the tests find them by walking up from the worktree. Every run of this registration therefore sets
   `CARGO_TARGET_DIR`, `RCFD_BIN` and `RCFD_MCP_BIN` to the scratch build of this branch's engine (no change under
   `crates/`), so the inventory test against the engine and the canvas test run and are not skipped.

## The hand-only surfaces (removed)

| Id | File | What it does | Why it is hand-only |
|---|---|---|---|
| H1 | `apps/studio-web/capabilities/coverage.json` | maps every capability to a UI path or a dated exemption | its only purpose is G15: holding each capability to a path by hand |
| H1 | `apps/studio-web/tests/support/capabilities.ts`: `checkCoverage`, `CoverageRule`, `CoverageFile`, `Exemption`, `CoverageResult`, `RoadmapItem`, and the UI and exemption parts of `coverageReport` | checks the map and reports UI paths and gaps | the G15 check and its report |
| H1 | `apps/studio-web/scripts/write-capabilities.mjs`: the reading of the map and its exit 2 | fails the generator on an unmapped capability | the G15 check |
| H2 | `apps/studio-web/src/renderer/first/FirstPath.tsx` (`FirstPathIntro`, `FirstPath` and their parts), `first/model.ts`, `first/target.ts`, `first/words.ts`, `first/first.css` | the learner's first path through the menus to a first result (F-813) | navigation whose only purpose is to lead a person by hand to `new_case` and `start_run`; F-813 retired by ADR 0803 |
| H2 | `apps/studio-web/src/renderer/project/ProjectHome.tsx`: the two places that draw `FirstPathIntro` and `FirstPath`, and their import | puts the path on the empty home and an empty project | the entry to H2 |
| H2 | `apps/studio-web/src/shared/word-budget.ts`: the entries `first-path-intro` and `first-path` (22 and 22 words) | the word budgets of the two removed surfaces | the surfaces are gone; no other budget is changed or raised |

`NotAProjectYet` and its `Explain` toggle move unchanged in behaviour (same text, classes, roles and names) from
`first/FirstPath.tsx` to a new `src/renderer/project/NotAProject.tsx`, with the CSS rules it uses moved from
`first/first.css` to a new `src/renderer/project/not-a-project.css`.

## What stays, and why

- The canvas (`src/renderer/canvas/`, F-976; F-979's files) and every view it draws (finding 2): `project/ProjectHome.tsx`
  (opens a project, hosts the canvas tab, its tables are canvas panels), `workspace/` (run panel, scrub), `field/` (field
  panel: pan, zoom, pick), `plots/` (plot and monitor panels), `runs/` (run lists as views, run rows; `RunControls` and
  `Sites` are named actions), `data/`.
- The short ways to look and their provenance (G23, G17): `provenance/`, the field view's pick and legend, the time panel.
- The theme and styling: `theme/`, `design/`, `ui/`, `studio-settings/` (the theme setting).
- Every named action of G16 and G20: `actions/`, `commands/` (the palette), `src/shared/actions/` with `first.run_example`.
- G18's undo and trash (`history/`, `project/Trash.tsx`), G21's progress (`work/`), the error lines (`help/`).
- The development galleries `dev/` (G9's benches and G13's view-and-report use them).
- The inventory of F-824 (`capabilities/inventory.json`, `readEngine`, the canonical-form and engine tests) and the page
  `docs/studio-coverage.md`, now the inventory's report.
- `NotAProjectYet` (finding 3, moved), and the case editor and settings surface (finding 4, open question Q1).

## The tests retired or removed (reason: the maintainer's answer 1 of 2026-11-26, ADR 0803, unless said)

Removed, each with its file:

1. `tests/unit/capabilities.spec.ts`: "a full map passes; an id rule wins over a prefix" and "fails on an unmapped
   capability, a malformed or undated exemption, an unknown item, a whole-kind prefix and a rule that maps nothing" (the
   describe "the coverage check": the checker of the G15 map); "every capability has a UI path or a recorded, dated
   exemption that names a roadmap item (G15)" (the coverage test of G15).
2. `tests/unit/capabilities.spec.ts`: "docs/studio-coverage.md is the report generated from the inventory and the map"
   becomes "docs/studio-coverage.md is the report generated from the inventory": the same comparison of the committed page
   with the generator's output, without the map (the generator changes with H1).
3. `tests/e2e/first-path.spec.ts`, the file: "no project open: the four steps of the path, …", "an empty project: one
   action, …", "run it, and the first honest result: …" and "the same walk by keyboard only, …" (the first path, H2). Its
   cases "an empty folder: what is missing and the next step; Make a project here runs the engine's command and opens the
   project" and "New project… from the command palette, …" test surfaces that stay and move with their steps unchanged to
   a new `tests/e2e/project-folder.spec.ts`, with the footer's disclaimer check of case 1 (G6), which no other e2e test
   makes; the Studio is launched in its `beforeAll` instead of in case 1, and the `rcfd serve --ui` that only the path's
   glossary terms used is not started.
4. `tests/unit/first-path.spec.ts`: the describes "the step a project is at (read from the engine's lists only)" (six
   tests) and "the words of the path (G6, G15, G19)" (five tests, among them the tokens check of `first/first.css`) and
   the imports of `src/renderer/first/`; the describes "first.run_example (G16)" and "the home and a folder that is not a
   project (nothing written)" stay unchanged, and the head comment says what is left.
5. `tests/unit/help-engine.spec.ts`: "every precise term of the first path is a term of the engine's glossary" (also
   G19's retirement) and its import of `first/words.ts`.
6. `tests/unit/provenance.spec.ts`: in "the numbers of the time panel, the workspace, Runs and the field readout are
   wrapped in a provenance trigger", the line counting the triggers of `first/FirstPath.tsx` (3) and its comment; the
   file is removed.
7. `tests/e2e/provenance-f828.spec.ts`: "KF: the learner's picture: a tick and the scale bar" (drives the first path),
   and the imports only it used.
8. `tests/e2e/a11y.spec.ts`: "the first path on a project that holds nothing yet", and `first-path` in the list of "every
   surface was walked".
9. `tests/e2e/word-budget.spec.ts`: "the first path on a project that holds nothing yet"; the line counting
   `first-path-intro` in "the project home with no project open, and the first path's four stations"; the two entries of
   `ROOTS` (with the two of `WORD_BUDGETS`, H2).

No test is kept as a skipped record: each removed test's subject is removed with it, and the ADR names it here.

## New tests

- **N1** (`tests/unit/capabilities.spec.ts`) "the G15 map is retired: no capabilities/coverage.json, and the generator,
  the support code and the report name no UI path or exemption".
- **N2** (`tests/unit/capabilities.spec.ts`) "the report counts every capability of the inventory by kind, and names
  every service method, MCP tool, CLI command and case section".
- **N3** (`tests/unit/hand-only-retired.spec.ts`, ADR 0808, F-981) "the first path is retired: its files are gone, no
  source imports or draws it, and the not-a-project notice is the project home's": the five files of H2 do not exist; no
  file under `src/` imports from a `first/` folder of the renderer or names `FirstPathIntro`, `FirstPath`, `fp--intro` or
  `fp--open`; `project/NotAProject.tsx` exports `NotAProjectYet` and `ProjectHome.tsx` imports it from there.
- **N4** (`tests/e2e/project-folder.spec.ts`, ADR 0808, F-981): the two moved cases of item 3 above.

## Checks

| Row | Clause | Check | Limit | Source of the limit | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|---|
| R1 | c5 | `npm test` in `apps/studio-web` (build, package, every unit and e2e spec), against the scratch engine | 0 failed | acceptance of F-981 ("npm test passes") | 0 failed | pass or fail | code verification |
| R2 | c4 | of R1, `canvas-f976.spec.ts` E1, E2, E3 | 3 of 3 pass, not skipped | acceptance of F-981; brief | 3 of 3 | pass or fail | code verification |
| R3 | c4 | of R2's E1, the time from `show_view`'s result to the shown view (the Studio opens a project and shows a view by `show_view`) | <= 1.6 s | ADR 0805 D5 (4 periods of `FEED_POLL_MS`), held by the test unchanged | 0.5 s (ADR 0805 measured 0.481 s) | 3.2 | code verification |
| R4 | c4 | of R1, the views' specs: unit `field-*`, `plots-*`, `workspace`, `compare`, `runs`, `provenance`, `provenance-f828`, `theme`, `mesh-preview`; e2e `field`, `field-derived`, `plots`, `workspace`, `workspace-first-open`, `compare`, `runs`, `provenance`, `provenance-f828`, `theme`, `theme-surfaces`, `idle-frames` | 0 failed | acceptance of F-981 | 0 failed | pass or fail | code verification |
| R5 | brief | of R1, "the committed inventory is what the engine reports now" | passes, not skipped | brief | pass | pass or fail | code verification |
| R6 | c2 | of R1, N1, N2 and "the inventory file is in its canonical form" | 3 of 3 pass | brief (the report requires no UI path; the inventory lists every capability) | 3 of 3 | pass or fail | code verification |
| R7 | c2 | of R1, N3 | passes | acceptance of F-981 (the hand-only surfaces removed) | pass | pass or fail | code verification |
| R8 | c4 | of R1, N4 (the kept surfaces of the moved cases) | 2 of 2 pass | acceptance of F-981 (views and their tests stay and pass) | 2 of 2 | pass or fail | code verification |
| R9 | c5 | `npm run typecheck` in `apps/studio-web` | exit 0 | rule 7 of agent-rules.md version 5 | exit 0 | pass or fail | code verification |
| R10 | c1 | `cargo test -p rcfd-cli --release --test docs_site` | 0 failed | rule 7 (docs touched) | 0 failed | pass or fail | code verification |
| R11 | c3 | no change under `crates/`, to `docs/roadmap.json`, the ledger, a golden or a baseline table: `git diff --stat 5bf77c58..HEAD -- crates docs/roadmap.json docs/process/ledger.tsv` | empty | rule 3; brief ("no change under crates/") | empty | pass or fail | unchanged default output |
| R12 | - | `python scripts/roadmap_proposals.py check` | exit 0 | rule 9 | exit 0 | pass or fail | code verification |
| R13 | - | `python scripts/process_report.py --check` | no failure but a generated file not current | rule 7 | `docs/adr/index.md` not current | pass or fail | code verification |

No crate is touched, so `cargo fmt` and `cargo clippy` have nothing to check (rule 7) and are not run. Each registered
run is made once.

## Negative control and mutations

On scratch copies (`git archive` of a commit into the scratch folder, with junctions to the worktree's `node_modules`
and, for M3, its `out/` from R1's build), never in the worktree:

- **NC1**: N3's file run on the base tree 5bf77c58 (with the new spec file added): it must fail (the first path is there).
- **M1, a retired surface's route left reachable**: a `src/renderer/first/FirstPath.tsx` that exports a `FirstPathIntro`
  drawing `<section className="fp fp--intro">`, drawn again by `NoProject` of `ProjectHome.tsx`. Expected catch: N3.
  **Committed** as `apps/studio-web/tests/unit/hand-only-retired.spec.mutation.patch` (rule 16).
- **M2a, the inventory check dropped**: `readEngine` leaves out the MCP tool lists. Expected catch: "the committed
  inventory is what the engine reports now".
- **M2b, the report drops capabilities**: the report leaves out the case keys. Expected catch: "docs/studio-coverage.md
  is the report generated from the inventory".
- **M3, the canvas test made to depend on a removed surface**: `canvas-f976.spec.ts`'s `beforeAll` waits for the first
  path's region "A first result in four steps" before it opens the project. Expected catch: `canvas-f976.spec.ts`
  (its `beforeAll` fails, E1 to E3 do not pass).

## Size

Size (agent): done; 0; 2; 2; 300000

## Decision rule

Done if every row R1 to R13 holds, NC1 fails as expected, and each of M1, M2a, M2b and M3 is caught; partial otherwise.

## Not covered

- The case editor and the settings surface (Q1) and the help on demand of the surfaces that stay (Q2).
- `apps/studio-web/README.md` describes the map and the first path; it is not among this brief's files, so it is left
  for the lead.
- The main checkout's stale binaries are not rebuilt: they are not this work's.

## Open questions for the maintainer

- **Q1. The case editor (`#/case`, `src/renderer/case/CaseEditorPage.tsx` and its parts) and the settings surface
  (`#/settings`, `src/renderer/settings/`, and the home's Settings button to it).** Options: (a) retire both in a
  follow-up item after F-979 and F-823 (ADR 0809) have landed, with G18's undo, conflict and close-guard checks carried by
  the engine's session and case tools that Claude drives and by a case view on the canvas; (b) keep them as read-only views
  a view kind can show; (c) keep both as they are. Recommended: (a); it is the maintainer's answer 1 carried through, and
  waiting for F-979 and F-823 keeps the concurrent items' tests standing. Not taken here: it changes the scope (rule 6).
- **Q2. The help on demand (Explain toggles, the glossary lines of `help/engine.ts`) in the surfaces that stay, now that
  G19 is retired.** Options: (a) keep it, as G22's way to say words on demand; (b) remove it in a follow-up. Recommended:
  (a). Not taken here, for the same reason.

## Addendum 1: results (2026-11-26)

Commits after the registration (c1d6abb9): 60a2f8cb (the code and tests as registered), 2c8d03d2 (the committed
mutation). Every run used `CARGO_TARGET_DIR`, `RCFD_BIN`, `RCFD_MCP_BIN` and `RCFD_RUNNER_BIN` of the scratch build
(finding 5). While writing, `tsc --noEmit` on the three configurations was run to find unused names (three times, the
last exit 0); that is compiling, not R9, which was run once afterwards.

| Row | Check | Limit | Measured | Met |
|---|---|---|---|---|
| R1 | `npm test` | 0 failed | 841 passed, 3 failed, 35 did not run (12.1 min): `unit/data-routes.spec.ts:94` "P12, tiny raw" (`lengthUnit` `[null, null]` against `[null, "m"]`), `e2e/data.spec.ts:73` "open: both routes describe the same store" (the same), `e2e/a11y.spec.ts:299` "the Runs view, a run selected from the keyboard, …" (three faded rows of the Runs grid the arrow keys do not reach, and the focus not brought back); the 35 are the serial tests after those | **no** |
| R2 | canvas E1, E2, E3 | 3 of 3 | 3 of 3 passed, none skipped | yes |
| R3 | E1's time from `show_view` to the shown view | <= 1.6 s | 0.481 s | yes |
| R4 | the views' specs | 0 failed | 0 failed (none of the three failures is among them) | yes |
| R5 | inventory against the engine | passes, not skipped | passed | yes |
| R6 | N1, N2, canonical form | 3 of 3 | 3 of 3 passed | yes |
| R7 | N3 | passes | passed | yes |
| R8 | N4 (`project-folder.spec.ts`) | 2 of 2 | 2 of 2 passed, and its footer case | yes |
| R9 | `npm run typecheck` | exit 0 | exit 0 | yes |
| R10 | `docs_site` | 0 failed | in Addendum 2 | - |
| R11 | `git diff --stat 5bf77c58..HEAD -- crates docs/roadmap.json docs/process/ledger.tsv` | empty | empty | yes |
| R12 | `roadmap_proposals.py check` | exit 0 | in Addendum 2 | - |
| R13 | `process_report.py --check` | only a generated file not current | in Addendum 2 | - |

Diagnosis of R1. The two data failures are the ones ADR 0795 (row X) and ADR 0805 (row L3) recorded: the tests take an
`f447_bench` from a `target` folder above the worktree, the main checkout's of 2026-11-16, older than `length_unit`
(2026-11-17); this work builds no `f447_bench` and touches no data code. The a11y failure is not diagnosed: no file of
the Runs view or of its grid changed (`git diff --stat 5bf77c58..HEAD -- apps/studio-web/src/renderer/runs` is empty),
the case runs before every case this work removed from that file, and ADR 0805's run of the same spec passed it. A run
of the same spec on the base tree was tried as a diagnosis on the scratch copy, but packaging that copy failed (its
`package:dir` moves Electron out of `node_modules`, which in the copy was a junction to the worktree's; it failed on the
path before moving anything, and the worktree's Electron was checked to be in place), so the base was not measured. The
test is not edited, skipped or run again here.

Negative control and mutations, each on its scratch copy:

| Id | What | Caught by | Result |
|---|---|---|---|
| NC1 | N3 on the base tree 5bf77c58 | N3 | failed, as it must (`first/FirstPath.tsx` exists) |
| M1 | a stub `FirstPathIntro` drawn again on the empty home | N3 | caught (committed as `tests/unit/hand-only-retired.spec.mutation.patch`) |
| M2a | `readEngine` without the MCP tool lists | "the committed inventory is what the engine reports now" | caught (1 failed, 7 passed) |
| M2b | the report without the case keys | "docs/studio-coverage.md is the report generated from the inventory" | caught (1 failed, 7 passed) |
| M3 | the canvas test waits for the first path's region before it opens the project | `canvas-f976.spec.ts` | caught (E1 failed in `beforeAll`, E2 and E3 did not run) |

Deviations: none from the registered changes. `cargo fmt` and `cargo clippy`: no crate touched, not run.
`apps/studio-web/README.md` still describes the map and the first path (not among the brief's files; for the lead).

Actual size: partial; 0; 2 (the release build, `npm test`); 2; about 330 000 bytes read.

Decision by the registered rule: **partial**. R1 missed (3 failed; two diagnosed as the stale `f447_bench` of the main
checkout, one, the a11y Runs grid, not diagnosed); every other row held, NC1 failed as it must and the four mutations
were caught.

### Open questions for the maintainer

Q1 and Q2 of the registration stand as written there, with their recommendations ((a) for each).

### For the lead: roadmap entries

> Status: partial.

> Note: ADR 0808, 2026-11-26, partial: G15's coverage map and check and the learner's first path retired (the not-a-project notice moved to the home, the G16 action first.run_example kept), every retired test named; the canvas test passes (show_view shown 0.481 s after the result, limit 1.6 s), the inventory test against the engine passes, 4 of 4 mutations caught; npm test 841 passed, 3 failed (two the stale f447_bench of ADR 0795, one a11y Runs grid case not diagnosed).

> Gap: npm test did not pass: data-routes P12 tiny raw and data.spec open fail on the main checkout's stale f447_bench (as ADR 0795 and 0805 recorded), and a11y.spec "the Runs view, a run selected from the keyboard" failed with three grid rows the arrow keys do not reach, not diagnosed (no Runs file changed). Open: Q1 (the case editor and the settings surface) and Q2 (help on demand) of ADR 0808.

## Addendum 2: the remaining runs (2026-11-26)

| Row | Check | Limit | Measured | Met |
|---|---|---|---|---|
| R10 | `cargo test -p rcfd-cli --release --test docs_site` | 0 failed | 2 passed, 0 failed | yes |
| R12 | `python scripts/roadmap_proposals.py check docs/roadmap/proposals/0808-F-981.json` | exit 0 | exit 0 ("F-981: todo -> partial; gap set; note + 468 characters"); a first call without the file stopped at the argument parser (exit 2) and checked nothing | yes |

R13 (`python scripts/process_report.py --check`) is run after this addendum is committed, so that it judges the final
tree; its result is in the report to the lead. The decision of Addendum 1 stands: **partial**.

## Addendum 3: amendment 1, registration of the a11y control and the remedy step (2026-11-26)

REGISTERED 2026-11-26, a further registration of F-981 under rule 13 of `docs/process/agent-rules.md` (version 5),
written before any run it covers and committed alone; on the lead's message of that day. Written by Claude Opus 5.5
(`claude-opus-5-5`). What came before it: the lead merged this branch at a99d003e and `push_main` refused it, because
`npm test` failed in each of its 3 runs on `tests/e2e/a11y.spec.ts:299` only (the case R1 of Addendum 1 also failed);
the same case passed in the push of F-976 (main 1e92df2a). The branch then merged `origin/main` at 2327d4f1 (F-979;
no conflict, no file of this work touched). Q1 and Q2 were answered by the lead (Q1 (a), entered as F-985; Q2: keep);
he writes them into this ADR at the merge.

**What the failure is (from R1's trace).** The case's `surface('runs', '.rv')` walks the Runs grid with the arrow keys
from its Tab stop, the first row (`r-20261028T152641-f8c2`). The trace is: ArrowUp to the column header "Case and run",
then ArrowDown through cells (`react-aria-2`, `-7`, `-12`, `-17`), not rows, and back up to the header. So the three
other rows count as not reached, and the focus comes back to the header rather than the row. The rows are faded because
the walk's arrow keys select (the grid's `selectionBehavior="replace"`); no run is selected before the walk.

**Hypotheses, from the code.** No file of the Runs view or of `ui/Table.tsx` changed (`git diff --stat 5bf77c58..HEAD --
apps/studio-web/src/renderer/runs apps/studio-web/src/renderer/ui` is empty). What this work changed that runs before the
case, in the same Studio, is: (HA) the empty home lost the first path's intro (`FirstPathIntro`), so the case "the project
home with no project open, …" walks fewer Tab stops and composites (the spec keeps the composites it has checked in
`checkedComposites`, keyed by name and root, across the whole file); and (HB) the open home lost `FirstPath`, which drew
nothing for the quick-start project, and the handler `openWorkspace`. (HC) The failure does not come from this work: it
also fails on the base tree.

**The control (C).** `a11y.spec.ts` run on its own (`npx playwright test tests/e2e/a11y.spec.ts --project=electron`),
packaged from each tree, against the engine of 2327d4f1 built in the scratch target (with `CARGO_TARGET_DIR`, `RCFD_BIN`,
`RCFD_MCP_BIN`, `RCFD_RUNNER_BIN` set as in Addendum 1):

- **C1**: the base tree 2327d4f1, a scratch copy with its own `npm ci` (no junction), **3 runs**.
- **C2**: this branch's tree, in the worktree (after `npm run build` and `npm run package:dir`), **3 runs**.

The count 3 is the lead's: his push failed 3 of 3. Outcomes and what follows, fixed now:

- C1 3 of 3 pass and C2 3 of 3 fail: the cause is in this work (HA or HB). A diagnosis D on a scratch copy of this tree
  with the base's `ProjectHome.tsx` and `src/renderer/first/` restored, 1 run of the spec: if it passes, the cause is the
  first path's removal; the remedy is then written as Addendum 4 (an amendment) before any code of it, in this work's
  files only (`ProjectHome.tsx`, `project/NotAProject.tsx`, `not-a-project.css`), and its rows are R14 and R15 below.
- C1 fails in any run: HC. No remedy in this work's files; R14 and R15 are run on this tree as they stand.
- Any other split: the case does not decide between the trees; recorded as not deciding, and R14 and R15 are run as
  they stand.

**Set-up corrected (the data failures of R1).** R1's two data failures took the main checkout's `f447_bench` of
2026-11-16. Before R15 the example is built into the scratch target (`cargo build --release -p rcfd-api --example
f447_bench`), where `tests/support/f447.ts` looks first (`$CARGO_TARGET_DIR/release/examples`). No test changes.

| Row | Check | Limit | Source of the limit | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|
| R14 | `a11y.spec.ts` alone on this tree (after the remedy, if one), 3 runs | 3 of 3 pass | the lead's message (the spec passes on this tree); acceptance of F-981 ("npm test passes") | 3 of 3 | pass or fail | code verification |
| R15 | `npm test` in `apps/studio-web`, once, against the scratch engine and `f447_bench` | 0 failed | acceptance of F-981 | 0 failed | pass or fail | code verification |

Then, once each: `npm run typecheck` (R9b), `cargo test -p rcfd-cli --release --test docs_site` (R10b), `python
scripts/roadmap_proposals.py check docs/roadmap/proposals/0808-F-981.json` (R12b; the file is updated with the outcome)
and `python scripts/process_report.py --check` (R13b, last). C1 and C2 are controls, not rows; neither is run again.

Mutations: if the remedy changes a Studio file, Addendum 4 registers one mutation of it and commits it as a patch beside
the test that must fail it (rule 16). No test is edited, skipped or loosened; no limit changes.

Size (agent): done; 0; 3 (the engine build, the six controls, `npm test`); 0; 60000

Decision rule: F-981 is **done** if R14 and R15 hold and every row of Addendum 1 other than R1 still holds (they are
inside R15) and, if a remedy changes a Studio file, its mutation is caught; **partial** otherwise.

## Addendum 4: results of the control and the diagnosis (2026-11-26)

Every run used the engine of 2327d4f1 built in the scratch target, with `f447_bench` built there as Addendum 3 set up.

| Control | Tree | Runs | Result |
|---|---|---|---|
| C1 | base 2327d4f1, scratch copy, own `npm ci` | 3 | 3 of 3 passed (12 passed each: 1.1 min, 57.7 s, 53.6 s) |
| C2 | this branch (440e2f65 + this ADR), worktree | 3 | 3 of 3 failed: runs 1 and 3 on `a11y.spec.ts:299` (the Runs grid, as in R1: from the first row the arrow keys go up to the column header and then down through cells, so three rows are not reached and the focus ends on the header), with 7 not run; run 2 passed that case and failed `a11y.spec.ts:418` "reduced motion: …" (the heading "Quick start" not shown within 30 s after Ctrl+O), with 2 not run |

By the rule of Addendum 3 the cause is in this work, and D was run once: a scratch copy of this tree with the base's
`ProjectHome.tsx` and `src/renderer/first/` restored (and `NotAProject.tsx` removed), this branch's `a11y.spec.ts`. D
failed: 8 passed, 1 failed, 2 not run. The Runs case passed. "reduced motion" failed as in C2 run 2, at line 422.

Diagnosis, as far as these runs go:

1. **"reduced motion" (`a11y.spec.ts:418`).** It depends on the case this work removed from the spec (item 8 of the
   registration, "the first path on a project that holds nothing yet"). That case pressed Ctrl+1 and opened the empty
   project, and "reduced motion" then reopens the quick start with Ctrl+O and waits for its heading. Without it, the case
   starts from the state the decision-panel case leaves, and the heading does not come. It fails with the source restored
   (D), so it is the removal from the spec, not the Studio. The registration's item 8 was defective: it removed a case
   that a later case of the same serial file relies on.
2. **The Runs grid case (`a11y.spec.ts:299`).** It failed in 2 of 3 C2 runs (and in R1 and in the lead's 3 runs), passed
   in 3 of 3 C1 runs and in D's 1 run. That points at the source change (the first path's removal from the home), but one
   run of D does not decide it, and Addendum 3 fixed no rule for a failing D. Not diagnosed further.

Not run: no remedy was written (Addendum 3 asked for one only after a passing D), so R14 and R15 were not run, nor
R9b, R10b, R12b. The work stopped here at the limit of 100 turns that the goal line sets.

Decision: **partial**, as in Addendum 1: R1 is still missed, and R14 and R15 are not run.

A remedy to register next (for the lead to issue): (1) in `a11y.spec.ts`, the step the removed case gave "reduced motion"
(Ctrl+1 and the open of another project before it) is restored as a step of that case, by an amendment; (2) the Runs grid
case gets a control D2 of 3 runs with the source restored and 3 with this tree's source, with the spec of (1), before any
change to `ProjectHome.tsx`; (3) then R14 (3 of 3) and R15 (`npm test` once).

## Addendum 5: amendment 2, registration of the spec's corrected set-up, the control D2 and the remedy step (2026-11-26)

REGISTERED 2026-11-26, a further registration of F-981 under rule 13 of `docs/process/agent-rules.md` (version 5),
written before any change or run it covers and committed alone; it carries out the next step that Addendum 4 proposed.
Written by Claude Opus 5.5 (`claude-opus-5-5`), as a background agent of the lead. The engine is the one built at
2327d4f1 in the scratch target (no file under `crates/` changed since: `git diff --stat 2327d4f1..HEAD -- crates` is
empty), with `f447_bench` built there as Addendum 3 set up; every run sets `CARGO_TARGET_DIR`, `RCFD_BIN`,
`RCFD_MCP_BIN` and `RCFD_RUNNER_BIN` as Addendum 1 did.

**What was read for it (no run).** The control's reports `test-results-visual/f816/report.json` left by the last run of
C1, of D and of C2. On the Runs surface the base (C1) and D walked 33 Tab stops and the grid's Tab stop was its last row
(`r-20261028T152641-3900`), so the arrow walk went up through the four rows and stopped when all were reached; this
branch (C2) walked 32 stops and the grid's Tab stop was its first row (`r-20261028T152641-f8c2`), so the walk's first
ArrowUp left the rows for the column header "Case and run", from which React Aria's ArrowDown moves by cells, not rows.
React Aria (`react-aria/dist/private/selection/useSelectableCollection.mjs`, lines 293 to 299) puts the focus on the
collection's last key when the focus enters it from an element after it and on the first key otherwise, while no key
has been focused yet: which row is the Tab stop depends on the order the focus first came into the grid. One stop of
the window differs between the trees; which one, the reports do not say.

**S1, the corrected set-up of "reduced motion" (`tests/e2e/a11y.spec.ts`).** Addendum 4, finding 1: item 8 of the
registration removed the case that left the Studio on the empty project, and "reduced motion" relied on it. The case
gets that step back as its first lines, as the removed case did it, without the first path's surface: Ctrl+1, the
heading "Quick start" shown, the folder dialog stubbed to the empty project, Tab to "Open project…", Enter, the heading
"An empty project" shown. Nothing else of the file changes; no check, limit or timeout of the case changes. The
defective version was this branch's own and never on main, so no test of main is replaced and no ignored record is
kept; its record is Addendum 4 and the history. S1 is committed alone, after this addendum and before D2.

**D2, the control of the Runs grid case** (`a11y.spec.ts:299`, "the Runs view, a run selected from the keyboard, …"),
`npx playwright test tests/e2e/a11y.spec.ts --project=electron`, each tree built and packaged first, the report
`test-results-visual/f816/report.json` copied to the scratch folder after each run:

- **D2r**: a scratch copy (`git archive` of the S1 commit, its own `npm ci`, no junction) with the base's (2327d4f1)
  `src/renderer/project/ProjectHome.tsx` and `src/renderer/first/` restored and `project/NotAProject.tsx` and
  `project/not-a-project.css` removed, **3 runs**.
- **D2b**: this branch at the S1 commit, in the worktree, **3 runs**.

Rule, fixed now for every outcome (the Runs case's result in each run; another failing case is recorded, not counted
here):

- **O1**: D2r passes the Runs case in 3 of 3 and D2b fails it in 1 or more of 3. The cause is this work's change of the
  home. **D3**, a diagnosis: one run in each of two scratch copies (D2r's, and one of the S1 commit with its own
  `npm ci`), where the copy's spec only gains, before the Runs case's `surface`, a line that prints the window's
  tabbable elements in document order and the grid's focused key; the two lists are compared. **The remedy** is then a
  change to `ProjectHome.tsx`, `project/NotAProject.tsx` or `project/not-a-project.css` only (the files this work owns),
  that gives the branch's window the stop or the focus order D3 shows the base has, without drawing the first path again
  (N3 holds) and without a change to a test, a support file, `runs/`, `ui/` or the canvas files. It is committed with a
  message citing this addendum. If D3 shows no difference, or the difference is outside those files, there is no remedy,
  and that is recorded.
- **O2**: D2r fails the Runs case in 1 or more of 3. The case fails with the base's home too; no remedy in this work's
  files.
- **O3**: D2b passes the Runs case in 3 of 3 (and D2r does too). With S1 the case does not fail on this branch; no
  remedy.

In every outcome R14 and R15 follow, on this branch as it then stands. D2 and D3 are controls, not rows; none is run
again.

| Row | Check | Limit | Source of the limit | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|
| R14 | `a11y.spec.ts` alone on this branch (after the remedy, if one), 3 runs | 3 of 3 pass (12 passed each) | Addendum 3 (R14); acceptance of F-981 ("npm test passes") | 3 of 3 | pass or fail | code verification |
| R15 | `npm test` in `apps/studio-web`, once, against the scratch engine and `f447_bench` | 0 failed | Addendum 3 (R15); acceptance of F-981 | 0 failed | pass or fail | code verification |

Then, once each: `npm run typecheck` (R9b), `cargo test -p rcfd-cli --release --test docs_site` (R10b), `python
scripts/roadmap_proposals.py check docs/roadmap/proposals/0808-F-981.json` (R12b, the file updated to the verdict) and
`python scripts/process_report.py --check` (R13b, last).

**Mutation (rule 16), if the remedy changes a Studio file:** M4, the remedy reverted, on a scratch copy of the remedy's
commit with its own `npm ci`; expected catch: `a11y.spec.ts`, one run. It is committed as
`apps/studio-web/tests/e2e/a11y.spec.mutation.patch` (made with `git diff`), first line
`# expect-fail: npx playwright test tests/e2e/a11y.spec.ts --project=electron`, and recorded as caught or not caught as
the run comes out. No test is edited beyond S1, skipped or loosened; no limit changes.

Size (agent): done; 0; 4 (D2's six runs, D3 if O1, R14, R15); 0; 40000

Decision rule: F-981 is **done** if R14 and R15 hold (every row of Addendum 1 other than R1 is inside R15) and, if a
remedy changes a Studio file, M4 is caught; **partial** otherwise.

## Addendum 6: results of amendment 2 (2026-11-26)

Written by Claude Opus 5.5 (`claude-opus-5-5`). Commits after Addendum 5 (250c495c): 3353ab65 (S1), 57c7f729 (the
remedy), 021e5840 (M4 committed as `apps/studio-web/tests/e2e/a11y.spec.mutation.patch`). Every run as Addendum 5 set
up; logs and reports are in the scratch folder (`d2.out`, `d3.out`, `r14.out` and the logs they name).

**D2.** D2r (the base's home and `first/` restored, own `npm ci`): 3 of 3 runs passed all 11 cases of the file, the Runs
case among them; its Runs surface had 33 Tab stops and the grid's Tab stop was the last row each time. D2b (this branch
at 3353ab65): run 1 failed the Runs case only (1 failed, 3 passed, 7 not run; 32 stops, Tab stop the first row, the walk
going into the column header as before); runs 2 and 3 passed all 11 (33 stops, Tab stop the last row). "Reduced motion"
passed in each of the five runs that reached it: S1 is the set-up it needed. Outcome **O1**.

**D3.** The instrumented copies printed the window's tabbable elements before the Runs walk: D2r's copy 33, the S1
copy 17 (and that run failed the Runs case again). The 16 missing are all the panel of a selected run
(`r-20261028T152641-3900`: Clear selection, Open in workspace, Run again, the log and report toggles, five provenance
numbers). So in the base a run is already selected when the Runs case starts, and React Aria's Tab stop is the selected
row (`firstSelectedKey`, `useSelectableCollection.mjs` line 299). Who selects it, from the code: the run workspace
selects the newest run when nothing is selected (`workspace/RunWorkspace.tsx`, the effect after `autoSelect`), and the
case before, "the project home of an open project", walks the tab list with the arrow keys and so shows the Workspace tab
for a moment. The workspace is loaded on demand (`lazy`); in the base the first path imported `FieldViewport` into the
home, so the workspace's chunk was small and mounted within that moment; without it, the chunk (with the field view)
often comes too late, nothing is selected, and the Runs case starts from the first row.

**The remedy** (57c7f729, `project/ProjectHome.tsx` only): the workspace's chunk starts loading when a project opens
(one shared promise for `lazy` and an effect of the open project), so it is ready by the time its tab is first shown. The
first path is not drawn again (N3 passed in R15); no test, support file, `runs/`, `ui/` or canvas file changed.

| Row | Check | Limit | Measured | Met |
|---|---|---|---|---|
| R14 | `a11y.spec.ts` alone, 3 runs, at 57c7f729 | 3 of 3 pass | 3 of 3 passed, 11 passed each (1.0 min, 1.0 min, 55.5 s) | yes |
| R15 | `npm test` once, at 021e5840 | 0 failed | 884 passed, **1 failed**, 4 skipped (13.7 min): `e2e/provenance-f828.spec.ts:351` "KR KK: the runs table's residuals are run_status's of their own run; …", at its line 396: after Ctrl+I pressed twice on a Runs grid cell, Escape left the provenance popover ("Provenance of residual of u") open | **no** |
| R9b | `npm run typecheck` | exit 0 | exit 0 | yes |

Deviation in R14: Addendum 5 wrote the limit as "3 of 3 pass (12 passed each)". The file has 11 cases since item 8 of
the registration removed one (C1's 12 were counted on the base's file), so "12 passed" was a counting slip in the
registration that no run of this file can meet. Each run passed every case of the file; the limit is not changed here,
and the lead judges the row. The data failures of R1 did not recur (`f447_bench` from the scratch target).

R15's failure is not diagnosed. The same case passed in R1 (Addendum 1, among R4's views) on the tree before S1 and the
remedy. The remedy changes when the workspace's code loads, and the case drives the Runs grid and the popover, so the
remedy is a candidate, but nothing here shows it; it was not run again (rule 2).

| Mutation | What | Caught by | Result |
|---|---|---|---|
| M4 | the remedy reverted (scratch copy of 021e5840 with the patch applied, own `npm ci`), one run of `a11y.spec.ts` | `a11y.spec.ts:299`, the Runs case | caught (1 failed, 3 passed, 7 not run) |

R10b (`docs_site`), R12b (`roadmap_proposals.py check`) and R13b (`process_report.py --check`) run after this addendum is
committed, so they judge the final tree; their results are in the report to the lead.

Actual size: partial; 0; 4 (D2, D3, R14 with M4, R15); 0; about 60 000 bytes read.

Decision by the rule of Addendum 5: **partial**. R14 held and M4 was caught; R15 missed (1 failed, not diagnosed).

### For the lead: roadmap entries

> Status: partial.

> Note: ADR 0808, 2026-11-26, partial (Addenda 5 and 6): the a11y Runs grid failure diagnosed (a run selected by the workspace's lazy chunk in the base; the home now loads that chunk when a project opens) and the reduced-motion set-up restored; a11y.spec passed 3 of 3 and the remedy's mutation was caught; npm test 884 passed, 1 failed (provenance-f828 KR KK: Escape left a popover open), not diagnosed.

> Gap: npm test did not pass: provenance-f828.spec.ts KR KK failed once after ADR 0808's remedy (Escape after Ctrl+I twice on a Runs grid cell left the provenance popover open), not diagnosed; it passed before the remedy.
