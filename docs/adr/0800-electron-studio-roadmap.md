# ADR 0800: The Electron Studio roadmap, generated from guardrails, PRODUCT.md and DESIGN.md

Status: ACCEPTED 2026-11-16 (questions answered by the maintainer, section 6). Asked for by the maintainer on 2026-11-16 ("create a new roadmap for the electron studio
that is going to be generated from a set of guardrails and from a PRODUCT.md and DESIGN.md"). The number 0800 and the
items F-800 to F-849 are a block reserved by the maintainer on 2026-11-16, so that this roadmap cannot collide with the
numbers the roadmap loop takes in order (F-704 and up, ADR 0633 and up).

## 1. Context

- ADR 0350 measured an Electron shell over the rcfd APIs (F-447) as fast enough and exact by both bulk routes. F-448
  (parity) waits for the maintainer's go-ahead; F-449 (retiring `rcfd-studio` and `rcfd-gui`) follows it.
- F-448 was one item. It is too large to run, review or measure as one piece, and it had no product or design authority
  beyond the parity list of ADR 0350 §5.

## 2. Decision: three authorities, one roadmap

The Electron Studio is planned from three documents, and every item below names the parts of them it serves:

| Authority | File | Holds | Decided |
|---|---|---|---|
| Guardrails | `docs/studio-guardrails.md` | G1 to G14: what must not happen (clean room, Rust is the engine, bit-exact pictures, additive interfaces, dependencies, honesty, runs outlive the window, accessibility (keyboard, focus, state in words; no contrast requirement since 2026-11-16), performance floor, Electron security, Claude Code, project folder contract, testable without eyes, rcfd-studio frozen); G15 to G21: what the Studio must do (everything the engine can do; no UI-only capability; reproducible with provenance; non-destructive; explain in place; every action a command; never block); G22: implicit by default, explicit on exception | G1 to G14 approved as drafted, 2026-11-16; G15 to G22 asked for by the maintainer the same day |
| Product | `PRODUCT.md` | Users (learners first, engineers next), purpose, positioning, operating context, stack, principles | impeccable `init`, 2026-11-16; stack chosen by the maintainer |
| Design | `DESIGN.md` (+ `.impeccable/design.json`) | The visual system: palette with one job per hue, Archivo with tabular figures, square hairline forms, state pictograms, run bands, field view | Recorded by impeccable's documenter from the shipped prototype `apps/studio-prototype/` |

Decisions of the maintainer on 2026-11-16 that the roadmap rests on:

- Stack: three.js for every 2D and 3D picture on `WebGPURenderer` with TSL (WebGL2 fallback); React 19 + Vite +
  react-three-fiber; React Aria Components as the accessible widget base; the app in `apps/studio-web/`.
- Visual world: a new one (not a port of the egui "technical-report plate"). The direction round (impeccable seed
  `3c0b0ece`) chose the Aicher system: Otl Aicher's Munich 1972 identity program, used only for type, palette, density
  and one signature move (run bands with a linked scrubber).
- The prototype came first, and DESIGN.md was recorded from it. The prototype is a reference, not the product. Its data
  is synthetic.

## 3. How the roadmap is generated

1. Every parity check of ADR 0350 §5 and `docs/ui-restart-plan.md` §6 becomes at least one item.
2. Every guardrail that needs work, not only restraint, becomes an item or an acceptance clause (G3 picture checks, G7
   kill-the-app test, G9 benchmark, G10 hardening, G13 test harness).
3. Every capability in PRODUCT.md (Operating Context, Capabilities) and every component in DESIGN.md that the parity set
   does not already cover becomes an item.
4. Every "Open" line in the three documents becomes an item marked NEEDS THE USER.
5. G15 makes coverage the organising rule of the UX: the engine's own inventory (`settings.schema`, the method list of
   `service.info`, the MCP tool list, the CLI help) is the list of what the Studio must reach, a coverage test keeps it
   honest, and a schema-driven surface lets new engine features appear with no Studio code. The Studio may be refactored
   freely to make a feature fit; engine-side changes stay additive under G4, through an ADR (maintainer, 2026-11-16).
6. Acceptance is written so an agent can check it headlessly (Playwright driving Electron, unit tests, value comparisons
   with the Rust reader or `rcfd-render`), plus a named view-and-report step where only the maintainer can judge (G13).

## 4. The items

F-800 to F-832, category "Studio desktop app". F-448 now depends on the parity subset (F-801 to F-814, F-816, F-817,
F-822) and on the items that make G15 to G21 real (F-824, F-825, F-827 to F-832); its acceptance is unchanged. F-449 is unchanged. The items and their traces are in `docs/ROADMAP.md` and
`docs/roadmap.json`.

| Id | Item | Guardrails | PRODUCT / DESIGN |
|---|---|---|---|
| F-800 | Go-ahead and repository layout for `apps/studio-web/` | G5, G14 | Stack |
| F-801 | Hardened Electron shell and test harness | G10, G13, G9 (P10) | Stack |
| F-802 | Design tokens and accessible primitives from DESIGN.md | G8, G1 | DESIGN Colors, Typography, Components |
| F-803 | Service client: control and events | G2, G4, G7, G9 (P2) | Operating Context |
| F-804 | Bulk field data: R2 reader and R1 route, bit for bit | G3, G9 (P4) | Stack |
| F-805 | Project home and the live project folder | G12 | Operating Context |
| F-806 | Run list and run state | G6, G7, G8 | DESIGN Runs table |
| F-807 | Run control: quick run, stop, queue | G2, G7 | Capabilities |
| F-808 | Field viewport on three.js WebGPU | G3, G9 (P7, P8) | DESIGN Field view |
| F-809 | Derived pictures: iso, streamlines, cut planes, vectors | G3, G6 | Operating Context |
| F-810 | Run bands and linked time scrubbing | G3, G9 (P5, P6) | DESIGN Run bands |
| F-811 | Residual and monitor plots | G3, G9 (P3) | Principles 4 |
| F-812 | Case editor with the engine's schema | G2, G12, G15 | Principle 5 |
| F-813 | A learner's first path | G6 | Users, Principle 5 |
| F-814 | Claude Code panel | G11 | Positioning |
| F-815 | Compare two runs | G3 | Operating Context |
| F-816 | Keyboard and assistive-technology pass | G8 | DESIGN Do's and Don'ts |
| F-817 | Performance regression bench for the product shell | G9 | — |
| F-818 | Dark theme | G8 | DESIGN Open |
| F-819 | Large meshes and polyhedral fan meshes | G9 | Capabilities (unmeasured) |
| F-820 | Installers, signing and updates | G10 | Guardrails Open |
| F-821 | Linux and macOS | G9 | Guardrails Open |
| F-822 | Tutorials that name the GUI | G6 | ui-restart-plan §6 |
| F-823 | Studio npm tests in the gate | G13 | Guardrails G13 |
| F-824 | Capability inventory from the engine and a UI coverage test | G15, G2 | Principle 6 |
| F-825 | Schema-driven case and settings surface | G15, G2, G8 | Principles 5, 6; DESIGN Inputs |
| F-826 | Engine: UI metadata in the case schema (needs its own ADR) | G15, G4 | Principle 6 |
| F-827 | One action layer for every UI action | G16, G20, G18 | Positioning |
| F-828 | Provenance inspector and reproduce-from-CLI | G17, G3 | Principle 1 |
| F-829 | Undo, safe deletes and edit conflicts | G18, G11, G12 | Operating Context |
| F-830 | Explain in place from the engine's docs | G19 | Users, Principle 5 |
| F-831 | Command palette and keyboard shortcuts | G20, G8 | Users (hard to master) |
| F-832 | Responsiveness budget and cancellable work | G21, G9 | Principle 3 |

## 5. Limits

- Agents cannot judge look and feel; every surface item ends with a view-and-report step for the maintainer.
- The prototype's numbers are synthetic; none of its pictures is evidence of anything.
- The design system covers one surface (the run workspace). Project home, the case editor and the learner's path
  extend it through impeccable (`shape`, then build), and DESIGN.md is updated only when the maintainer approves a
  system change.
- The dependency list of the prototype (react, react-dom, three, @react-three/fiber, react-aria-components,
  @fontsource/archivo; dev: vite, @vitejs/plugin-react, typescript, @types/*) is the starting point for F-800's
  justification, not an approval of it.

## 6. Answers of the maintainer (2026-11-16)

- **Q1, go-ahead for F-448:** yes. F-800 to F-802 may start now.
- **Q2, the prototype:** keep `apps/studio-prototype/` as the design reference until F-448 passes, then remove it.
- **Q3, where Studio npm tests run:** in CI, as a job in `.github/workflows/ci.yml` (not a local-only step, not inside
  `scripts/gate_test.py`).

## 7. Change of 2026-11-16: no contrast requirement

The maintainer removed every WCAG AA contrast requirement from the Studio (G8, PRODUCT.md, F-802, F-816, F-818) and chose
to revert the eight contrast-driven token changes of F-802, so the product tokens equal DESIGN.md again. Keyboard
access, visible focus, names and roles, state carried by shape and words as well as colour, colour-blind-safe field
maps and reduced motion stay.

## 8. Change of 2026-11-16: G22, implicit by default

The maintainer added guardrail G22 ("Implicit by default, explicit on exception") and The Quiet Rule in DESIGN.md
(option (a) of three: guardrail plus design principle, rather than a design principle only or a per-surface review).
Cause: features kept adding standing text, as they did in `rcfd-studio`, because G6, G8, G18 and G19 were met most
cheaply with a visible sentence and tests asserted visible text. From now on those words live in accessible names,
tooltips, help and the palette, tests read them by role and name, and visible words are for exceptions. Every F-8xx
acceptance clause that asks for words ("says what is missing", "names what is lost", "state in words") is read under
G22.

The maintainer then chose a visible-word budget per surface (option (a) of three: per surface, rather than one global
budget or no test): each surface's budget of chrome words at rest is set from its screenshots once it is quieted, a
test counts them from the DOM, and raising a budget needs the maintainer's word.

The built surfaces are quieted by one sweep item, F-833 (option chosen by the maintainer, rather than one item per surface or
folding it into F-816); F-448 now also depends on F-833. Surfaces built after it ship under G22 from the start.

## 9. F-817 bench results (2026-11-16)

F-817 adds one entry point over the measurement scripts of F-801, F-803, F-804, F-808 and F-810:
`npm run bench` in `apps/studio-web` (`scripts/bench.mjs`; README, "The performance bench"). It runs P2, P4, P5, P7, P8
and P10 of G9 against the packaged Studio and prints one line per check, `P<n> <what> <value> <unit> limit <limit>
met|MISSED`. Lines marked `(recorded)` are printed for comparison and are not judged. The exit code is 1 on a judged
miss and 2 when a case could not run. The light set (default) runs in a few minutes on an idle machine. `--heavy` takes
P2's full 300-event sample and adds P6 at 4e6 cells, the Follow-live latency and P12 over the 1e6 and 4e6 boxes, all
recorded. The bench is not in `npm test` or CI.

Window mode: the bench starts the Studio in the tests' mode, `RCFD_TEST_HEADLESS=1` (the window shown in place but
transparent, opacity 0, click-through, unfocused, off the taskbar, background throttling off), so a run opens no
visible window. `RCFD_TEST_HEADLESS=0` gives the normal window. The light and heavy runs below (18:36 to 19:42 UTC) were
taken before that mode existed on this branch, with the normal window. The third run (19:48 UTC, after the merge of
`studio-roadmap`) is in the test mode.

Judging follows the per-item records of ADR 0801. P4 and P7 are met by route R1 or R2 (ADR 0350 §3), P4 on the raw
store. P8's frame rate is the better of the vertically synced and the uncapped run, and its p99 frame comes from the
synced run (F-808's reading). P2 also judges its sample size (at least 50 events, as `measure-service.mjs` does).

Machine: AMD Ryzen 7 9800X3D (16 logical), 93.6 GB, Windows 11 (10.0.26300), a 238 Hz display, WebGPU adapter `nvidia`
`blackwell`; launcher Node 22.19.0, Electron 45.3.3 (Chromium 156.0.8078.100, Node 24.22.3), the packaged copy of this
branch. Data: `f447_bench gen` boxes tiny, 1e6 and 4e6 (generated once for the bench in 26 min, not counted).

**Load: every run below was taken on a fully loaded machine.** The processor busy fraction was 1.00 before and after
every run and every case (0.94 once). Other jobs' solver test binaries, `rustc` and `cargo` used 4 to 11 cores
throughout, with 4 to 10 `cargo`/`rustc`/`link` processes and about 33 GB of memory free. Nothing was retried to pass.
The numbers say what the shell does under that load. They are not a baseline for an idle machine.

### Light set (`npm run bench`, normal window, 18:36 to 18:56 UTC)

| Id | Limit | Measured | Met |
|---|---|---|---|
| P10 | median <= 2.5 s | median 1243 ms; first 600, max 4454 (10 starts) | yes |
| P2 | p99 <= 20 ms | 100 events in 93.5 s: p50 0.44, p99 7.70, max 7.85 ms; clock drift 0.44 ms | yes |
| P2, F-807 per run action (recorded) | 20 ms | worst 1.76 ms (`project.cancel_run`, 6 actions, `npm test` of 18:33) | recorded |
| P4 R2 raw | p50 <= 30, p99 <= 60 ms | p50 24.1, p99 52.4, max 77 | yes |
| P4 R1 raw (recorded) | as above | p50 50.4, p99 83.2 | no |
| P4 deflated (recorded) | as above | R1 p50 55.8, p99 86.2; R2 p50 54.3, p99 258 | no |
| P5 | >= 10 per s, no frame > 150 ms | first launch: not run (see below); run again once: 9.80 per s over 200 requests, frame p50 91.6, p99 271, max 300 ms | **no** |
| P7 R2 | median <= 1.5 s | median 74.9 ms; first 853 | yes |
| P7 R1 (recorded) | as above | median 2139 ms | no |
| P8 frame rate | >= 60 per s | 238 per s (vertical sync, 238 Hz display); uncapped 54.1 (recorded) | yes |
| P8 p99 frame | <= 25 ms | 4.40 ms (vertical sync); p50 4.20, max 16.8 | yes |

Verdict of the light run: 7 of 7 judged lines met, with P5 not run (exit code 2). In the first P5 case,
`electron.launch` timed out after 180 s. The Studio's Chromium profile (`%APPDATA%\rcfd Studio`, shared with every
Studio instance on the machine, including other jobs' tests) logged cache-lock errors and a restart of its network
service. P5 was run once more on its own (`--only=P5`, 18:57 UTC, processor busy 1.00) and missed, as the table shows.
F-810 measured 23.6 per s with a longest frame of 91.6 ms on this machine, also under full load.

### Heavy set (`npm run bench -- --heavy`, normal window, once by hand, 19:00 to 19:30 UTC)

| Id | Limit | Measured | Met |
|---|---|---|---|
| P10 | median <= 2.5 s | median 20512 ms; first 20512, max 34450; the ten: 20512, 31965, 34450, 8128, 7940, 1999, 10181, 26990, 29341, 31321 | **no** |
| P2 | p99 <= 20 ms, at least 50 events | 32 events in 120 s (the solver starved): p99 2.56 ms, but p50 -4.51 and clock drift 10.1 ms, so the sample is not usable | **no** (sample) |
| P1, main (recorded) | p99 <= 2 ms | `service.info` 1.01, `run.status` 0.69 | yes |
| P4 R2 raw | p50 <= 30, p99 <= 60 ms | p50 23.8, p99 57.3, max 66.1 | yes |
| P4 R1 raw (recorded) | as above | p50 57.5, p99 105 | no |
| P4 deflated (recorded) | as above | R1 p50 64.0, p99 107; R2 p50 58.5, p99 226 | no |
| P12 (recorded) | 0 differing | 1e6: 80 fields on each route and codec; 4e6: 20 fields of `p` on each; 0 differing | yes |
| P5 | >= 10 per s, no frame > 150 ms | 28.5 per s over 200 requests; frame p50 33.2, p99 43.5, max 457 ms | **no** (one frame) |
| P6 (recorded) | >= 3 per s | 11.2 per s over 100 requests; longest frame 113 ms | yes |
| Follow live (recorded) | none | 493 snapshots committed in 30 s, 115 drawn; commit to drawn p50 47, max 187 ms | recorded |
| P7 R2 | median <= 1.5 s | median 70.9 ms; first 685; cold mount 148 (recorded) | yes |
| P7 R1 (recorded) | as above | median 2071 ms | no |
| P8 frame rate | >= 60 per s | 240 per s (vertical sync); uncapped 34.3 (recorded) | yes |
| P8 p99 frame | <= 25 ms | 4.30 ms (vertical sync); p50 4.20, max 8.20 | yes |

Verdict of the heavy run: 7 of 10 judged lines met. P10, P2 (sample size) and P5 (one 457 ms frame) missed (exit
code 1).

P10 alone (`--only=P10`, normal window, 19:31 UTC, processor busy 1.00), run to check the final script: median
40336 ms, first 40270, max 47063 (**no**).

### Light set in the test mode (`npm run bench`, RCFD_TEST_HEADLESS=1, 19:48 to 20:10 UTC)

| Id | Limit | Measured | Met |
|---|---|---|---|
| P10 | median <= 2.5 s | median 3581 ms; first 20805, max 30231; the ten: 20805, 26454, 27628, 2452, 2449, 12388, 30231, 1362, 2091, 3581 | **no** |
| P2 | p99 <= 20 ms | 57 events in 120 s: p99 1.32 ms, but p50 -1.34 and clock drift 5.35 ms (the join's error grows with the load) | yes |
| P4 R2 raw | p50 <= 30, p99 <= 60 ms | p50 29.1, p99 47.8, max 62.6 | yes |
| P4 R1 raw (recorded) | as above | p50 53.9, p99 92.2 | no |
| P4 deflated (recorded) | as above | R1 p50 66.9, p99 108; R2 p50 44.5, p99 71.8 | no |
| P5 | >= 10 per s, no frame > 150 ms | 22.7 per s over 200 requests; frame p50 41.8, p99 71.0, max 76.4 ms | yes |
| P7 R2 | median <= 1.5 s | median 102 ms; first 1136 | yes |
| P7 R1 (recorded) | as above | median 5421 ms | no |
| P8 frame rate | >= 60 per s | 235 per s (vertical sync); uncapped 54.4 (recorded) | yes |
| P8 p99 frame | <= 25 ms | 8.20 ms (vertical sync); p50 4.20, max 20.7 | yes |

Verdict: 8 of 9 judged lines met, P10 missed (exit code 1). The processor was busy 1.00 before and after every case.
In this mode the window paints and its frames run (P8 at the display's rate, P5 met), so the test mode does not stop
the frame-paced checks. It does not remove the load either, so this run does not establish a baseline.

Reading: P4, P7 and P8 held their earlier records under full load in all three runs (P4 R2 raw within the limit, P7 R2
at 71 to 102 ms, P8 at the display's rate). The misses were P5 (once on the rate, once on a single frame; met in the
third run), P10 in the heavy and third runs, and P2's starved sample. These checks wait on the processor or on Chromium's start-up while other jobs hold every
core, so the misses are recorded as taken under load. A run on an idle machine (Q4 below) settles whether any of them
is a regression. P10's median ranged from 1.2 s to 40.3 s across four runs within 75 minutes, all at the same busy
fraction of 1.00. Its cause was not diagnosed. The shared Chromium profile (Q5) is one candidate.

### Decisions taken in building F-817 (reversible)

- D1, how the bench reuses the scripts: (a) run each `measure-*.mjs` as its own process with `--out`, then read its
  JSON; (b) turn the scripts into modules and import them; (c) one new script that measures everything again. Taken: (a).
  The scripts and their records stay as they are. The only change to them is two additive flags: `--events=<n>` in
  `measure-service.mjs` (the light P2 sample of 100) and `--no-p6` in `measure-scrub.mjs`. Without the flags they
  behave as before.
- D2, the light set: the same data sizes as G9 (1e6 cells, 1e6 triangles, the 1e5 box for P2), with P2's sample cut from
  300 to 100 events. The 4e6 cases and P12 are only in `--heavy`.
- D3, the data: (a) a folder kept between runs (`--data`, `$RCFD_BENCH_DATA` or `<temp>/rcfd-studio-bench-data`) and
  generated once when sizes are missing; (b) `--data` required; (c) generated afresh each run (3 to 26 minutes). Taken:
  (a).
- D4, load: the processor busy fraction, free memory and the busiest processes summed by name over 2 s, before and after
  the run. Each script also takes its own busy fraction. A case at 0.5 or more is marked `LOADED MACHINE` beside each of
  its lines.
- D5, the window: the bench sets `RCFD_TEST_HEADLESS=1` for the scripts unless the environment sets it to `0`, as
  `playwright.config.ts` does for the tests, so a run opens no visible window (asked for on 2026-11-16 while F-817 was
  built). The mode is printed and kept in the results.

### Open questions for the maintainer

- Q1, P8's frame rate: (a) the better of the vertically synced and the uncapped run (taken; F-808 read P8 this way);
  (b) the uncapped run only, as ADR 0350 measured it, which needs the measurement-only switch that F-808 left open;
  (c) the vertically synced run only. Recommendation: (a) until F-808's switch question is answered.
- Q2, P2 under load: (a) judge the sample size as well, so a starved sample cannot pass (taken); (b) judge the p99 alone;
  (c) use a smaller box when the machine is loaded. Recommendation: (a).
- Q3, CI (for F-823): (a) keep the bench out of CI and run it by hand on the maintainer's machine, as G9 says; (b) a
  manually started job on a self-hosted runner with a GPU, running the light set; (c) `--only=P2,P10` on hosted runners,
  recorded and not judged. Recommendation: (a), and (b) if a GPU runner is ever added. Hosted runners have no WebGPU
  adapter for P4, P7 and P8.
- Q4, a baseline on an idle machine: run `npm run bench` once with no other jobs running before F-448's acceptance, and
  record it here. Recommendation: yes. Every number above was taken at full load.
- Q5, the Chromium profile: the scripts launch the packaged app with the default profile `%APPDATA%\rcfd Studio`, which
  every Studio instance on the machine shares. (a) pass a scratch `--user-data-dir` from the measurement scripts (a cold
  GPU cache for each launch, which changes what P7's first open and P10 measure); (b) leave the scripts as they are.
  Recommendation: (b) for P10, which should measure the user's own start, and (a) for the other cases, decided with Q4.
- Q6, the window mode of the bench: (a) the tests' transparent window by default, the normal window with
  `RCFD_TEST_HEADLESS=0` (taken); (b) the normal window by default, since G9's limits describe what a user sees, with the
  test mode by flag. Recommendation: (a), and the idle baseline of Q4 taken once in each mode, which shows whether the
  mode moves any number.

### Answers of the maintainer (2026-11-16)

- **Q4:** yes, one baseline on an idle machine (no other jobs) before F-448's acceptance.
- **Q5:** yes, a scratch `--user-data-dir` for every bench case except P10, which keeps the user's own profile.
- **Q3:** yes, the bench stays out of CI and is run by hand (hosted runners have no WebGPU adapter).

Q5 applied (F-817b, 2026-11-17): `scripts/bench.mjs` makes a fresh folder in the temporary directory for every case but
P10 and passes it to the script as `--profile-root`; each launch of the Studio in that case gets an empty Chromium
profile of its own, `--user-data-dir=<folder>/launch-<n>` (`scripts/profile.mjs`, used by every `measure-*.mjs` that
launches Electron), and the folder is removed when the case ends. P10 (`measure-cold-start.mjs`) keeps the user's own
profile. The bench prints and keeps each case's profile; a script run alone uses the user's profile unless given
`--profile-root`. No bench number above was taken this way; the idle baseline of Q4 will be the first. Details in ADR
0801, Addendum F-817b.

## 10. Retiring the egui apps early (maintainer, 2026-11-16)

The maintainer asked to "kill the old rcfd-studio and clear up some test bandwidth" and chose (a) of three options:
(a) delete `crates/rcfd-studio` and `crates/rcfd-gui` now; (b) drop them from the workspace and keep the code until
F-448; (c) delete only rcfd-studio. G14 now records them as retired. F-449 is done ahead of F-448; F-448 keeps its
meaning (the Electron Studio covers what rcfd-studio did), judged against the capabilities listed when they were
deleted. `rcfd-claude` stays, because `rcfd-mcp` uses it.
