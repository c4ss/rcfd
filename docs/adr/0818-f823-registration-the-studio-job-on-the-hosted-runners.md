# ADR 0818: F-823 registration, the Studio's job on GitHub's hosted runners: the registered skips, a WebGPU adapter on Windows, and the race of Follow live

REGISTERED 2026-11-26, written before any code of F-823 under this registration and before any run of its checks.

Written by Claude Opus 5.5 (`claude-opus-5-5`), as a background agent of the lead. It answers the brief
`docs/adr/briefs/f823-hosted-runners.md` ([briefs/f823-hosted-runners.md](briefs/f823-hosted-runners.md)) and follows
`docs/process/agent-rules.md` ([agent-rules.md](../process/agent-rules.md)), version 5. A further registration of F-823
by rule 13: it adds no acceptance clause of its own; it serves the clause "the job fails the workflow on any test
failure" on GitHub's hosted runners, as the lead's read of ADR 0809's question 2 asks. Branch `f823-hosted-runners`, at
the brief's commit e2908a03 (equal to `origin/main`). Scratch folder `E:/projects/cfd/w49i-work/f823-hosted-runners-scratch`.

## The item

F-823's acceptance (read from `docs/roadmap.json` by a script; status partial): "The Studio's unit and Playwright tests
run as a job in .github/workflows/ci.yml (ADR 0800 Q3, maintainer 2026-11-16), with npm ci from the committed lock file
and Electron run headless; the job fails the workflow on any test failure; docs/test-process.md names the job. A weekly,
grouped Dependabot npm entry for /apps/studio-web lands with the job (ADR 0801 Q2). The job builds rcfd-cli, rcfd-runner
and rcfd-mcp first so the service e2e tests and the F-824 inventory check do not skip."

## What was read

AGENTS.md, the rules (version 5) and the brief, in full. `.github/workflows/ci.yml` in full (the studio-web job: the
Linux switches `--ignore-gpu-blocklist --enable-unsafe-webgpu`, none on Windows; the step "no test skipped but the
registered ones" with `STUDIO_ALLOWED_SKIPS: ""`). [ADR 0812](0812-f823-registration-case-facts-refuses-by-the-counts-estimate.md)
in full (its R7w miss of P2 under load, its question 2). [ADR 0807](0807-f979-registration-live-meshes-settings-and-monitors-on-the-canvas.md)
amendment 1 (the four tests of `canvas-f979.spec.ts` kept as a skipped record by `test.skip(true, ...)` at line 34).
Of the Studio: `tests/support/launch.ts` (`RCFD_TEST_ELECTRON_ARGS`), `tests/support/gpu-hold.ts` (lines 34 to 37: no
adapter throws "no WebGPU adapter: the GPU process cannot be held"), `tests/e2e/workspace-first-open.spec.ts` lines 1 to
140, `tests/e2e/provenance-f828.spec.ts` lines 1 to 40 and 200 to 345 (the scan SC and its exemptions),
`tests/e2e/run-control.spec.ts` lines 1 to 137 and 268 to 330 (`P2_MS` 20 from G9, `SHOWN_MS` 50; both clocks are
`performance.timeOrigin + performance.now()`, `src/main/project/control.ts` line 51), `tests/e2e/workspace.spec.ts`
lines 1 to 120 and 380 to 459, `tests/e2e/a11y.spec.ts` lines 1 to 120 and 403 to 445,
`src/renderer/workspace/RunWorkspace.tsx` lines 205 to 240 (Follow live sets the line by a passive effect after the count
changes), `src/renderer/workspace/model.ts` `counterText`, `src/renderer/field/FieldCanvas.tsx` lines 905 to 940 and
`FieldViewport.tsx` lines 750 to 751 (the label "Drawn by WebGL 2 (fallback: no WebGPU adapter or device)", visible only
on the fallback), `src/shared/work.ts` (`LONG_TASK_LIMIT_MS` 100, `LONG_FRAME_LIMIT_MS` 150, G9's P5). G9 of
[studio-guardrails.md](../studio-guardrails.md) and the [tolerance table](../process/tolerances.md).

The logs of the three runs (`gh run view <id> --log-failed`) and their artifacts (`gh run download`: each job's
`playwright-results.json`, `playwright-report/`, the error contexts; no traces: `retries: 0` and no trace option), read by
a scratch script. About 210,000 bytes.

## What the runs show (from the reports)

One correction of the brief's account: `run-control.spec.ts:268` and `workspace.spec.ts:389` failed on **ubuntu-24.04**
(runs 45886730170 and 45891872417), not on windows-latest, where both passed in all three runs. On ubuntu the third run
(45894509968) passed every test (890 in the report) and failed only the skip step.

| Test | Job | Runs failed | What the report gives |
|---|---|---|---|
| `workspace-first-open.spec.ts:59` | windows-latest | 3 of 3 | `navigator.gpu.requestAdapter()` gave null: "no WebGPU adapter: the GPU process cannot be held" (gpu-hold.ts:36). On ubuntu (with `--enable-unsafe-webgpu`) it passed 3 of 3, the hold 4150 to 5209 ms. |
| `provenance-f828.spec.ts:288` | windows-latest | 3 of 3 | SC found one text in workspace, two in compare, one in difference, each "Drawn by WebGL 2 (fallback: no WebGPU adapter or device) < span.fv-backend"; every other scan empty, KT as registered. On ubuntu SC empty 3 of 3. |
| `responsiveness.spec.ts:260` | windows-latest | 1 of 3 (run 1) | all 3 passes: a long task of 201 to 208 ms and a frame of 226 to 264 ms in `workspace.tab`, at a machine load of 0.03 to 0.05 busy; runs 2 and 3: at most 67 ms. |
| `a11y.spec.ts:427` | windows-latest | 1 of 3 (run 3) | after Enter on "Open project…" with the dialog stubbed to the empty project, the heading "An empty project" did not appear in 30 s; the window still showed the quick start, no dialog or notice; the 2 later tests of the serial file did not run. |
| `run-control.spec.ts:268` | ubuntu-24.04 | 2 of 3 | run 1: `project.cancel_run` commit 56.2 ms against `SHOWN_MS` 50 (received 19.8 ms); run 2: `project.queue_case` received 39.7 ms against P2's 20 ms. In the same processes `project.run_case` was received in 1.3 to 1.4 ms. |
| `workspace.spec.ts:389` | ubuntu-24.04 | 2 of 3 | line 415 `expect(later.snapshot).toBe(n - 1)`: 69 against 76, then 85 against 99. |
| skip step | ubuntu-24.04 | 1 of 1 reached | "890 tests in the report, 4 skipped and not in the registered list": `e2e/canvas-f979.spec.ts:162`, `:175`, `:208`, `:244`. On Windows the step was never reached (the Playwright step failed first). |

## Hypotheses, evidence and the remedy each points to

**H1, no WebGPU adapter on windows-latest (a hosted runner without a GPU).** Evidence: the null adapter of
workspace-first-open (3 of 3), the fallback label's own reason "no WebGPU adapter or device" (so `'gpu' in navigator`
holds and only the adapter is missing), and the Linux job, which passes `--enable-unsafe-webgpu` and has an adapter (the
SwiftShader Electron ships, ADR 0804), passing both tests 3 of 3. It explains provenance-f828:288: the only text SC
reports is the fallback label, whose digit is the API's name "WebGL 2", shown only when there is no adapter
(`FieldViewport.tsx` line 750; with WebGPU the label is `sr-only` and SC skips it). Both tests were registered on a
machine with WebGPU; their premise is an adapter. **Remedy R1:** the Windows Playwright step passes
`RCFD_TEST_ELECTRON_ARGS` with the switch the Linux step already uses for WebGPU, `--enable-unsafe-webgpu` (a switch of
the tests' Electron, `tests/support/launch.ts`, not of the app). The premise is stated and checked by the test itself:
workspace-first-open fails, never skips, when no adapter exists (gpu-hold.ts:36). No test, limit or exemption changes.
Which switches, fixed now: `--enable-unsafe-webgpu`; if the local probe PR2 below gives no adapter and PR3 gives one,
`--enable-unsafe-webgpu --use-webgpu-adapter=swiftshader` (PR3's set). The probe does not decide the runner; the lead's
run does.

**H2, responsiveness:260 on Windows is the WebGL 2 fallback's first draw, not the runner's load.** Evidence: the three
passes of run 1 agree (201 to 208 ms) at a load of 0.05 or less, so it is not load; the task is in `workspace.tab`, where
the field view creates its renderer; it occurred on the runner without an adapter, and not on ubuntu, where WebGPU drew
(at most 76 ms). That it occurred in one run of three fits a runner image whose CPU differs between runs (the fallback
compiles its shaders on the CPU). Weaker than H1: inferred, not measured. **Remedy:** R1 (the field view then draws with
WebGPU, as on ubuntu); `LONG_TASK_LIMIT_MS` and `LONG_FRAME_LIMIT_MS` unchanged.

**H3, Follow live: a race of the test's two reads (a defect of the test).** `settled(page)` reads the line's snapshot,
then line 414 reads the count "of N" in later calls; the run writes a snapshot every 5 iterations meanwhile, so N can
grow between the two reads whatever the Studio does. On ubuntu the renderer is slow (software WebGL; the spec's own
comment at line 404 says so) and the Playwright round trips are long: 7 and 14 snapshots came between. The app also
moves the line to the newest by a passive effect after the count's commit (`RunWorkspace.tsx` lines 224 to 228), so even
one read can land between the two commits. **Remedy R3 (rule 2, a corrected test; the old kept as the record):** a new
test after it in `workspace.spec.ts`, the same as the old line for line but for lines 413 to 415: after `settled` and
`later.snapshot > first.snapshot` as before, the line (`.ws` `data-snapshot`) and the count (`.tp-counter`, "of N") are
read in one `page.evaluate` (one task of the page, one DOM state), polled by the spec's own `until` for 15 s (the wait
the same test uses for "Follow live brings it back", line 433) until the line equals N - 1. The property held is the
one the test names, "the line stays on the running run's newest snapshot"; a line that never reaches the newest still
fails. The old test gets one first line `test.skip(true, 'ADR 0818: ...')` and nothing else changes (as ADR 0807
amendment 1 did); it is the record, at `e2e/workspace.spec.ts:389`, entered in `STUDIO_ALLOWED_SKIPS`.

**H4, run-control:268 on ubuntu: the runner's CPU against a limit measured on another machine.** Evidence: the misses
are single actions (39.7 ms once, 56.2 ms once) while the same processes delivered `run_case` in 1.3 to 1.4 ms, so it is
not a clock offset between the main process and the renderer (both use `timeOrigin + now()`, and an offset would apply
to every action); the 4-processor runner also runs the engine, the runner, rcfd-mcp, Xvfb and llvmpipe. ADR 0812 saw the
same limit missed under load on this host (23.6 ms). Not a race of the test and no defect found. **Remedy:** none in
code. P2 is G9's limit and `SHOWN_MS` the spec's; neither has a source for a hosted runner, so neither changes. A miss
in the lead's run is recorded as a miss; making it a known miss needs an amendment and is a question for the maintainer
(Q1).

**H5, a11y:427 on Windows: not diagnosed.** The report gives no trace; the window shows the quick start, no refusal and
no dialog, so the open of the empty project either never started (the Enter lost) or did not end in 30 s (for example a
first start of rcfd-mcp for a new folder on the runner). One run of three. **Remedy:** none in code; the lead's run
records it. A trace on failure would change every test's timing, so it is not added here (Q2).

**The four skips of ADR 0807.** `STUDIO_ALLOWED_SKIPS` gets `e2e/canvas-f979.spec.ts:162 e2e/canvas-f979.spec.ts:175
e2e/canvas-f979.spec.ts:208 e2e/canvas-f979.spec.ts:244`, with a comment naming ADR 0807 amendment 1, and
`e2e/workspace.spec.ts:389` with this ADR. The keys are `<spec.file>:<spec.line>` as the step builds them; both jobs'
reports write `spec.file` with `/` (read from the Windows reports).

## Changes (all of them)

1. `.github/workflows/ci.yml`, studio-web job only: the Windows branch of the Playwright step runs with
   `RCFD_TEST_ELECTRON_ARGS` set as R1 fixes, its comment saying why; `STUDIO_ALLOWED_SKIPS` as above, the step's comment
   naming ADR 0807 amendment 1 and this ADR. Nothing else of the file.
2. `apps/studio-web/tests/e2e/workspace.spec.ts`: R3's skip line in the old test, R3's corrected test after it.
3. `apps/studio-web/tests/e2e/workspace.spec.mutation.patch` (MU1, rule 16), this ADR,
   `docs/roadmap/proposals/0818-F-823.json`.

No app file changes. Not the canvas files.

## Table of checks

| Id | Clause | Check | Limit | Source of the limit | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|---|
| SK1 | "the job fails the workflow on any test failure" (the skip step) | the step's script (taken from the committed ci.yml by a scratch tool) over run 45894509968's ubuntu report with the new list | exit 0, "0 skipped and not in the registered list" | ADR 0804 (the step), ADR 0807 amendment 1 (the record) | passes | exact | code verification |
| SK2 | negative control of SK1 | the same with main's list `""` | exit 1 naming the 4 tests of canvas-f979 | reproduces the run's step | exit 1 | exact | code verification |
| AL | the job's file | `actionlint .github/workflows/ci.yml` | exit 0, no finding | the goal; rule 7 | passes | exact | code verification |
| PR | H1, a local stand-in (not deciding) | the probe below, PR0 to PR3, once each | recorded: adapter or null, its info, WebGL 2 or not | the brief ("any local reproduction ... with its number of runs") | PR0 adapter; PR1 null; PR2 or PR3 adapter | n/a | code verification |
| SW | R1 changes nothing on a machine with a GPU | `workspace-first-open.spec.ts`, `provenance-f828.spec.ts`, `responsiveness.spec.ts` with R1's switches on this host, once | 0 failed, 0 skipped; their limits unchanged | the specs (F-890 ADR 0787; F-828 ADR 0795; F-832 and G9 P5) | passes | exact | code verification |
| EMU | R1 where no GPU is used (local stand-in for the runner) | the same three specs with `--disable-gpu` and R1's switches, once; made only if PR2 or PR3 gave an adapter, else recorded as not run | 0 failed, 0 skipped | as SW | passes | exact | code verification |
| FL | R3 | in R7w: the corrected Follow live passed; the old one skipped at `e2e/workspace.spec.ts:389` | passed; skipped | F-810's spec (its own assertions) | passes | exact | code verification |
| R7w | rule 7 | `npm run typecheck`, then `npm test` in `apps/studio-web` (Windows, this host, no switches, as the maintainer runs it); then the step's script over its report | both exit 0; 0 failed, 0 did not run, skipped exactly the 5 listed; the script exit 0 | rule 7; ADR 0812 R7w | passes (risk: P2 under load, as ADR 0812) | exact | code verification |
| MU | rule 16 | MU1 to MU4 below | each caught | rule 16 | caught | exact | code verification |
| GHW | the clause, windows-latest | the lead's `workflow_dispatch` run of the branch | the Playwright step 0 failed (workspace-first-open:59, provenance-f828:288, responsiveness:260, a11y:427 passed); the skip step exit 0 | the acceptance; the brief | H1 and H2 hold: passes; H5 unknown | exact | code verification |
| GHU | the clause, ubuntu-24.04 | the same run | 0 failed (run-control:268 and the corrected Follow live included); the skip step exit 0 | the acceptance; the brief | H3 fixed; H4 may miss again | exact | code verification |
| DS | rule 7 (docs) | `cargo test -p rcfd-cli --release --test docs_site` | passes | rule 7 | passes | exact | code verification |
| P1 | rules 7 and 9 | `python scripts/roadmap_proposals.py check docs/roadmap/proposals/0818-F-823.json`; `python scripts/process_report.py --check` | valid; clean but for a generated file only not current | rules 7 and 9 | passes | exact | code verification |
| A1 | owned files | `git diff --stat e2908a03..HEAD` | the files of "Changes" only | the brief | as the limit | exact | code verification |

### Set-up of the runs (fixed here)

- Cargo: `CARGO_TARGET_DIR=E:/projects/cfd/w49i-work/f823-hosted-runners-scratch/target`, `CARGO_BUILD_JOBS=8`, release:
  `cargo build --release -p rcfd-cli -p rcfd-runner -p rcfd-mcp` and `-p rcfd-api --example f447_bench`, so `npm test`
  finds this commit's engine and bench there (the variable the support files read), not a stale one.
- `npm ci` in the worktree's `apps/studio-web` before any Studio run.
- The host CPU load (mean `LoadPercentage` of `Win32_Processor`) is recorded before each timed run and every 30 s during
  the long ones (a scratch sampler).
- **The probe** (scratch, `tools/probe-gpu.mjs`): Playwright's `_electron` launches the packaged Studio
  (`out/packaged`) with `RCFD_TEST_ELECTRON_ARGS`-like switches, and in the window evaluates
  `navigator.gpu.requestAdapter()` (null or its `info` and `isFallbackAdapter`) and whether a canvas gives a `webgl2`
  context. PR0 no switches; PR1 `--disable-gpu`; PR2 `--disable-gpu --enable-unsafe-webgpu`; PR3 `--disable-gpu
  --enable-unsafe-webgpu --use-webgpu-adapter=swiftshader`. Once each.
- Order: build, `npm ci`, probe, the code commit, SW, EMU, R7w, the mutations, SK1, SK2, AL, DS, P1. Each once. A miss
  is recorded with its number and nothing is run again to pass; a failure on a file the OS refused under load is
  reported, not retried.
- The links of each ADR commit are checked before it by a scratch link checker (every relative link resolves); DS is the
  registered run, once, at the end.

## Negative controls and planned mutations

SK2 is the negative control of the skip list. Mutations, each once, on a scratch copy (`git archive` of the code commit
with a real `node_modules` from `npm ci` and this work's target folder), the e2e ones by
`npm run build && npm run package:dir && npx playwright test tests/e2e/workspace.spec.ts --project=electron`:

| Id | Mutation | Caught when |
|---|---|---|
| MU1 | committed: Follow live does not move the line (`RunWorkspace.tsx` line 227 without `|| (follow && stop.index !== last)`) | the corrected Follow live test fails |
| MU2 | the corrected test's read compares the line with N, not N - 1 | the corrected test fails (its `until` runs out) |
| MU3 | `STUDIO_ALLOWED_SKIPS` without `e2e/workspace.spec.ts:389`, over R7w's report | the step's script exits 1 naming it |
| MU4 | `STUDIO_ALLOWED_SKIPS` without `e2e/canvas-f979.spec.ts:208`, over run 45894509968's ubuntu report | the step's script exits 1 naming it |

MU1 is committed as `apps/studio-web/tests/e2e/workspace.spec.mutation.patch`, its first line `# expect-fail: cd
apps/studio-web && npm run build && npm run package:dir && npx playwright test tests/e2e/workspace.spec.ts
--project=electron`.

## Size

Size (agent): partial; 1; 3; 2; 260000

(Heavy runs: R7w, SW with EMU, and the e2e mutations. Partial expected because H4 has no remedy here and H5 is not
diagnosed; the lead's run decides.)

## Decision rule

**Done** if every row holds locally and in the lead's run on GitHub (GHW, GHU) and each of MU1 to MU4 is caught;
**partial** otherwise, with each miss recorded with its number.

## What is not covered

The Ubuntu 24.04 container run (ADR 0809's R7l): the hosted ubuntu runner is the subject here. A cause for H4 and H5
beyond the hypotheses above. The latent behaviour of provenance-f828's scan on a machine without an adapter (Q3). The
unit-tests job's failure on ubuntu in run 45886730170 (one Rust unit test, not the Studio's job). The egui Studio.

## Open questions for the maintainer

None at registration but those named: Q1 (P2 and `SHOWN_MS` on a hosted runner), Q2 (traces on failure), Q3 (the
fallback label in provenance-f828's scan), each with options in the addendum.

## Amendment 1 (2026-11-26): SW's first command ran no test; the same command from the right folder

Written after the code commit 87ea95d5 and the probe, before the corrected run; nothing above is changed.

The probe gave (once each, host CPU 66 % before): PR0 an adapter (`nvidia`, `blackwell`); PR1 (`--disable-gpu`) no
adapter, WebGL 2 present, the state of the windows-latest runs; PR2 (`--disable-gpu --enable-unsafe-webgpu`) an adapter
(`google`, `swiftshader`); PR3 the same as PR2. So R1's switches are `--enable-unsafe-webgpu`, as fixed above.

SW's command (host CPU 90 % before) ran no test: in the scratch command the CPU sampler was started with `&` at the end
of a `cd ... && ...` chain, so the `cd` ran in the background shell and `npx playwright` ran outside `apps/studio-web`,
fetched another Playwright (1.65.1) and stopped with "Project(s) "electron" not found", exit 1. No test of the three
specs started; SW is not decided by it. Corrected set-up: the same command with the `cd` in the foreground and the
sampler started separately. Run once. SW's limit is unchanged; EMU follows it as registered (PR2 gave an adapter).

## Amendment 2 (2026-11-26): MU1 was not caught; the committed patch is MU2's

Written after the mutation runs and before the addendum; nothing above is changed and no limit or check is affected.
MU1 (Follow live's clause removed from `RunWorkspace.tsx` line 227), run once on its scratch copy (host CPU 39 % before):
`workspace.spec.ts` 6 passed, 1 skipped (the record), exit 0, so the corrected test did **not** catch it; the line still
reached the newest snapshot without that clause, by a path this work did not find. It is recorded as not caught. The
patch committed at 412d1441 is MU1's and would not fail, so it does not meet rule 16. It is replaced, in the commit of
this amendment, by MU2's patch (the corrected test's read compares the line with N, not N - 1), which was caught (run
once, host CPU 80 % before: the corrected test failed, "timed out waiting for the line on the newest snapshot while
following"). The file name and its expect-fail line are unchanged.

## Addendum 1: results (2026-11-26)

Commits: e68e7b52 (this registration), 87ea95d5 (ci.yml and workspace.spec.ts), 3dd7287b (amendment 1), 412d1441 (MU1's
patch), c74bd3c8 (amendment 2, MU2's patch), then this addendum with the proposal. Each registered run was made once;
the host CPU load (mean `LoadPercentage`) is given with each. Logs in the scratch folder (`logs/`). Other agents loaded
the machine throughout.

### Checks

| Id | Limit | Measured | Met |
|---|---|---|---|
| SK1 | exit 0, 0 not registered | run 45894509968's ubuntu report: "890 tests in the report, 0 skipped and not in the registered list", exit 0 | met |
| SK2 | exit 1 naming the 4 tests | main's list: 4 skipped and not registered (`canvas-f979.spec.ts:162`, `:175`, `:208`, `:244`), exit 1 | met |
| AL | exit 0 | `actionlint` 1.7.12: no finding, exit 0 | met |
| PR | recorded | in amendment 1: PR0 adapter (nvidia); PR1 null, WebGL 2 present; PR2 and PR3 adapter (google, swiftshader). R1 = `--enable-unsafe-webgpu` | recorded |
| SW | 0 failed, 0 skipped | first command ran no test (amendment 1). Corrected run (CPU 93 % before, 54 % to 87 %): 8 passed, 0 failed, 0 skipped, 1.6 m; SC empty everywhere; F-832 workspace.tab no task, frames at most 141 ms; F-890 hold 2866 ms, no task over 50 ms | met |
| EMU | 0 failed, 0 skipped | `--disable-gpu --enable-unsafe-webgpu` (CPU 50 % before, 28 % to 100 %): 4 passed, **4 failed**, 4.5 m: `provenance-f828.spec.ts:288` and `:500` "timed out waiting for both views in one range"; `responsiveness.spec.ts:260` and `workspace-first-open.spec.ts:59` "timed out waiting for the small run drawn". The adapter exists (the hold of workspace-first-open was made), but the field views did not draw with SwiftShader's WebGPU in this stand-in | **missed** |
| FL | passed; the record skipped | the corrected test `workspace.spec.ts:450` passed; `:389` skipped | met |
| R7w | typecheck and `npm test` exit 0; 0 failed, 0 did not run, the 5 skipped; the step exit 0 | typecheck exit 0; `npm test` exit 0: 886 passed, 0 failed, 5 skipped (the 4 of canvas-f979 and `workspace.spec.ts:389`), 0 did not run, 13.2 m (CPU 52 % before; 26 samples 27 % to 99 %, median 67 %). `run-control.spec.ts:268` passed; `error-walk.spec.ts:177` passed in 3.2 s. The step's script over its report: 891 tests, 0 not registered, exit 0 | met |
| MU | each caught | MU2, MU3, MU4 caught; MU1 **not caught** (below) | **missed** |
| GHW, GHU | the lead's run | not run by this work (the lead dispatches it) | not run |
| DS, P1, A1 | as registered | in the report | see the report |

### Mutations

| Id | Caught by |
|---|---|
| MU1 (Follow live's clause removed) | **not caught**: `workspace.spec.ts` 6 passed, 1 skipped (CPU 39 % before); amendment 2 |
| MU2 (line compared with N) | the corrected test: "timed out waiting for the line on the newest snapshot while following" (CPU 80 %); committed |
| MU3 (list without `workspace.spec.ts:389`, R7w's report) | the step's script: 1 skipped and not registered, exit 1 |
| MU4 (list without `canvas-f979.spec.ts:208`, run 45894509968's report) | the step's script: 1 skipped and not registered, exit 1 |

### Deviations

SW's first command ran no test (my scratch command's `&`), corrected by amendment 1 before its one corrected run. The
committed patch was MU1's, which was not caught; replaced by MU2's by amendment 2. The brief's account of where
run-control:268 and workspace:389 failed (Windows) is corrected above: both failed on ubuntu.

### What was not run

The lead's GitHub run (GHW, GHU), which decides H1, H2 and H4 on the runners. The container run (not registered).

### Size

Size (agent, actual): partial; 1; 4; 3; 270000

### Decision by the registered rule

**Partial.** SK1, SK2, AL, SW, FL and R7w hold and MU2 to MU4 are caught. EMU misses: on this host, a window without the
GPU and with SwiftShader's WebGPU adapter did not draw the field views, so R1 may change Windows's failure rather than
remove it (the runner has its own software D3D device, which this stand-in does not reproduce; the lead's run decides).
MU1 is not caught. GHW and GHU are the lead's. H4 (P2 on ubuntu) has no remedy here and H5 (a11y:427) is not diagnosed.

## Open questions for the maintainer

- **Q1 (scope).** `run-control.spec.ts:268` holds G9's P2 (20 ms) and `SHOWN_MS` (50 ms) on the hosted ubuntu runner,
  which missed them in 2 of 3 runs. Options: (a) keep both; a miss fails the job and is recorded; (b) by an amendment, the
  hosted runners record P2 as a measurement (reported, not failing) while the maintainer's machine holds it; (c) a
  self-hosted runner for the timing specs. Recommendation: (a) until the lead's run gives the rate, then (c).
- **Q2 (scope).** No traces on failure, so a11y:427 could not be diagnosed. Options: (a) leave it; (b) Playwright's trace
  on failure in CI only, by a registration that measures its effect on the timing specs. Recommendation: (b).
- **Q3 (scope).** On any machine without an adapter, provenance-f828's scan counts the "2" of "Drawn by WebGL 2" as a
  number without provenance, and EMU shows SwiftShader's WebGPU may not draw where no GPU is used. Options: (a) leave
  both, the premise being an adapter; (b) a registration that corrects the scan (rule 2: a corrected test exempting the
  backend label, the old kept as the record) and, if the lead's run shows Windows does not draw, takes R1 back for Windows
  and runs the WebGPU-only spec where a hardware adapter exists. Recommendation: (b).

## For the lead: roadmap entries

> Status: partial

> Note: ADR 0818, 2026-11-26, partial: the studio-web job's skip step lists ADR 0807's four records and the record of Follow live's race (corrected by reading the line and the count in one task; it passed), and the Windows step gives WebGPU a software adapter (--enable-unsafe-webgpu); npm test on Windows passed 886 with 5 registered skips; a GPU-less local stand-in did not draw with that adapter, and one mutation (Follow live's clause) was not caught; the run on GitHub is to be read.

> Gap: The hosted runners: the Windows WebGPU tests now get a software adapter (--enable-unsafe-webgpu), not yet seen on GitHub, and a GPU-less local stand-in did not draw with it; run-control.spec.ts:268's P2 (20 ms) missed on ubuntu in 2 of 3 runs, no remedy; a11y.spec.ts:427 on Windows undiagnosed (no traces). The skip step and Follow live's race are corrected (ADR 0818). The lead's workflow_dispatch run decides.

No follow-up item is proposed here; Q2 and Q3 name the ones this work would recommend.

## Amendment 3 (2026-11-26): the lead's run on GitHub and the answers; the switch taken back, a corrected provenance scan, and workspace-first-open's premise checked

Written after addendum 1 and before any change or run it covers; nothing above is changed.

**The lead's run.** Run 45906133164 (workflow_dispatch, head 83e232c4), from the lead: ubuntu-24.04 studio-web passed
(every test, the skip step with this ADR's list), so GHU is **met**; windows-latest studio-web failed, 17 failed and 806
passed (19.2 min), the views not drawn with `--enable-unsafe-webgpu` as EMU predicted (a11y:326, cancel:207,
canvas-f979-a1:160, compare:157, f836:85, field-derived:177, field:200, idle-frames:153, plots:177,
provenance-escape-f988:63, provenance-f828:288 and :500, provenance:379, responsiveness:260, theme-surfaces:152,
word-budget:164, workspace:116), so GHW is **missed** and R1 is refuted on the runner. The lead's answers (on the
maintainer's standing instruction of 2026-11-26, go with the recommended option): Q1 the limits stay, a self-hosted
runner becomes a roadmap item (the lead's); Q2 traces on failure are a later item; Q3 the switch is taken back for
Windows and the scan gets a corrected test under rule 2.

**Changes (all of them).**

1. `.github/workflows/ci.yml`: the Windows branch of the Playwright step runs `npx playwright test` with no switch again
   (as on main), its comment saying why (this amendment). `STUDIO_ALLOWED_SKIPS` gains `e2e/provenance-f828.spec.ts:288`
   (the record below). A new variable `STUDIO_PREMISE_SKIPS: "e2e/workspace-first-open.spec.ts:59"`: the skip step
   allows a skipped test listed there only when its skip annotation's description begins `premise not met:`, and prints
   each such test on its own line `skipped, premise not met: <file>:<line> [<project>] <reason>`; any other skip of it
   fails the step as before. So a missing build still fails it (the file's `NO_BENCH` and `NO_MCP` skips do not begin so).
2. `apps/studio-web/tests/e2e/provenance-f828.spec.ts`: the old test at line 288 gets one first line
   `test.skip(true, 'ADR 0818 amendment 3: ...')` and nothing else changes (the record of the defect: on a machine
   without an adapter its scan counts the "2" of the renderer's fallback label as a number of a view). A corrected test
   after it, the same line for line but that each scan's `bare` list drops the entries matching
   `/^Drawn by WebGL 2 \(fallback: [^)]*\) < span\.fv-backend /`, exactly the label of `FieldViewport.tsx` line 751 as
   the scan prints it, and nothing else. Its limits are the old test's.
3. `apps/studio-web/tests/e2e/workspace-first-open.spec.ts`: its premise is a GPU process with a WebGPU adapter (the hold
   of `gpu-hold.ts` needs one; ADR 0787). Checked at the test's start, after the Studio is launched and before anything
   is measured: the window asks `navigator.gpu.requestAdapter()`; without an adapter the test is skipped with
   `premise not met: no WebGPU adapter in this window's GPU process (a machine without a GPU); the GPU process cannot be
   held (ADR 0818 amendment 3, ADR 0787)`, the Studio closed in its `finally`. With an adapter nothing changes. This is
   chosen over a known miss because the test still holds its limit wherever its premise holds (ubuntu with SwiftShader,
   the maintainer's machine), and the skip is reported by the named entry above, never silently.

**Runs (each once, locally, CPU load recorded).** After `npm run build && npm run package:dir`:
- A3a: `provenance-f828.spec.ts` and `workspace-first-open.spec.ts` with no switch: 0 failed; skipped only `:288` (the
  record).
- A3b: the same two with `RCFD_TEST_ELECTRON_ARGS=--disable-gpu` (the runner's state, PR1): 0 failed; skipped `:288`
  and `workspace-first-open.spec.ts:59` with the premise reason; the skip step's script with the committed lists over
  its report exits 0 and prints the premise line.
- A3c: `npm run typecheck`, then `npm test` with no switch: exit 0, 0 failed, 0 did not run, skipped exactly the 6 of
  `STUDIO_ALLOWED_SKIPS`; the step's script over its report exits 0.
- A3d: `actionlint .github/workflows/ci.yml`: exit 0.

**Mutations (each once).** MA1: A3b's report with `STUDIO_PREMISE_SKIPS` empty: the step's script exits 1 naming
`workspace-first-open.spec.ts:59`. MA2: A3c's report with `e2e/canvas-f979.spec.ts:162` moved from
`STUDIO_ALLOWED_SKIPS` to `STUDIO_PREMISE_SKIPS`: exits 1 (its reason is ADR 0807's, not a premise). MA3: the corrected
provenance test with its filter removed, run once with `--disable-gpu` on a scratch copy: it fails on the label. The
committed patch stays MU2's.

**Decision rule** unchanged: done only if every row holds locally and in the lead's next run on GitHub and each mutation
is caught; MU1 (not caught) and EMU (missed) stand, so this registration stays partial whatever these runs give.
