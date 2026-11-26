# ADR 0809: F-823 registration: where error-walk.spec.ts:177 spends its time under WebGPU in the Linux container

Status: **REGISTERED 2026-11-26, written before any code of F-823 under this ADR and before any run of its checks.** Before
this text no file of the repository was changed and nothing was run but reading: files, and the logs of ADR 0804's runs in
its scratch folder. Results go in amendments and addenda at the end; nothing above them is changed afterwards.

It answers the brief `docs/adr/briefs/f823-error-walk-webgpu.md` (on main, issued 2026-11-26; not on this branch, so not
linked) and follows [docs/process/agent-rules.md](../process/agent-rules.md), version 5, with [the Studio
guardrails](../studio-guardrails.md). It is a further registration of F-823 under rule 13, after
[ADR 0802](0802-f823-registration-the-studio-web-job-builds-the-engine.md) and
[ADR 0804](0804-f823-registration-the-four-linux-failures-and-a-skip-step.md), on the same branch `f823-studio-ci-engine`
after its commit b1e2b376. Scratch folder `w49i-work/f823-error-walk-webgpu-scratch`. Model: Claude Opus 5.5.

## What is asked

The acceptance of F-823 in `docs/roadmap.json` (read by a script): "... the job fails the workflow on any test failure ...
The job builds rcfd-cli, rcfd-runner and rcfd-mcp first so the service e2e tests and the F-824 inventory check do not skip."
ADR 0804 left it partial: with `--enable-unsafe-webgpu` (SwiftShader's WebGPU adapter) in the Linux job,
`error-walk.spec.ts:177` ("1004 LIMIT: a mesh over the preview limit; Build anyway is offered") did not see the refusal
within its own 60 s wait (line 182) in the job's run L, and took 47.3 s in R7l; 28.7 s with WebGL 2 (ADR 0802). The lead
took ADR 0804's Q1 (a): measure where the time goes, the 60 s kept.

## What was read

`AGENTS.md`, `docs/process/agent-rules.md` (version 5) and the brief, each whole; ADR 0804 whole; the studio-web job of
`.github/workflows/ci.yml` (lines 117 to 245); `apps/studio-web/docker/linux-test/Dockerfile`;
`apps/studio-web/tests/e2e/error-walk.spec.ts` whole; `src/renderer/case/MeshPreview.tsx` whole; of
`src/renderer/field/FieldCanvas.tsx` lines 1 to 31, 136 to 160, 425 to 460, 590 to 690 and 900 to 938; of
`crates/rcfd-api/src/mesh_preview.rs` lines 1 to 60, 104 to 111 and 200 to 240; of ADR 0804's scratch folder
`L/step-11.log` lines 895 to 935, `L.log` and `R7l/npm-test.log` (the error-walk lines), its tools (`ctr.sh`,
`tools/common.sh`, `tools/job.sh`, `tools/npmtest.sh`, `tools/muts.sh`, `tools/diag.sh`, the arguments of `run_job.py`).
The item F-823 by a script.

## What the code says the test does (arithmetic before any run)

The case `cases/cavity-re100.toml` of the quick-start project is `cavity_2d` with `nx = 32`, `ny = 32` (the spec's own
replacement pattern, line 118). The test types `mesh.nx` 1500 (Enter), then `mesh.ny` 1500 (Enter). The preview is asked for
500 ms after the last settings change (`PREVIEW_DEBOUNCE_MS`, `MeshPreview.tsx` line 29). The engine estimates the cells from
the counts and refuses over `ESTIMATE_LIMIT` = 2,000,000 (`mesh_preview.rs` lines 26 and 214 to 228): 1500 × 32 = 48,000 is
under it and is **built, loaded and drawn**; 1500 × 1500 = 2,250,000 is over it and is refused at once (0.006 s for a
far larger count, ADR 0138 G4c). So between the two edits the Studio may build the 48,000-cell mesh, read its store and
draw it on the field canvas (`FieldCanvas`, three.js `WebGPURenderer`, `forceWebGL={false}`, `MeshPreview.tsx` line 141),
with its mesh grid; that draw is not what the test checks. With G1 of ADR 0804 the canvas takes the WebGPU back end on
SwiftShader instead of WebGL 2 on llvmpipe (ADR 0804 amendment 1, item 4).

## Hypothesis and the control that tests it

**H.** WebGPU on SwiftShader draws the preview the test walks through (the intermediate 1500 × 32 mesh), and a draw costs
more there than under WebGL 2; the renderer's main thread is held by that work, so the 500 ms timer of the next preview, its
call and the refusal's render come late, past the 60 s wait.

**Control T** (made after this commit, before any code; once): in the container, on a `git archive` copy of b1e2b376, with
only `tests/e2e/error-walk.spec.ts` replaced in the copy by a probe copy (in the scratch folder) in which:

- the 1004 test records the time of each of its steps: s1 `openCase`, s2 `typeInto nx 1500`, s3 `typeInto ny 1500`, s4 the
  wait for `.mp-failed` (in the probe only, waited up to 200 s and the test's timeout 300 s, so the time is measured past
  60 s), s5 the recovery line, s6 Build anyway, s7 the screenshot, s8 `typeInto nx 32`, s9 `typeInto ny 32`, s10 the wait
  for the failure to go;
- before s2 a `PerformanceObserver` of `longtask` is installed in the page; its entries in s2 to s4 are summed;
- from s2's start to s4's end the test samples the page every 250 ms: the round trip of a `page.evaluate`, and of the
  preview `.mp` its `aria-busy`, `data-mesh-hash`, `data-drawn`, the text of `.mp-facts` and whether `.mp-failed` is there;
- a ninth probe test, last in the file: in a fresh `openCase(CASE)`, `mesh.nx` 100000 alone, timed to `.mp-failed` (up to
  200 s), its text and recovery line recorded, then `mesh.nx` 32 and the failure's removal timed.

Arms: **A** `RCFD_TEST_ELECTRON_ARGS="--ignore-gpu-blocklist --enable-unsafe-webgpu"` (G1, the job's), **B**
`--ignore-gpu-blocklist` (WebGL 2). Command per run: the job's Linux Playwright command limited to the file, `xvfb-run
--auto-servernum npx playwright test --reporter=list tests/e2e/error-walk.spec.ts`. **Six runs, in the order A B A B A B**,
each the whole file. The container's `/proc/loadavg` and the host's CPU load are recorded before each run.

**Rule, fixed here.** Let D be, per run, the time from s2's start to s4's end; I, whether a sample of that run shows
`.mp-facts` with "48,000 cells"; F, the long tasks' sum over the same window divided by D.

- **T-H (H holds):** median D of A ≥ 1.5 × median D of B, and in each A run I is true and F ≥ 0.5. Remedy **R1** (below).
- **T-N (not reproduced):** median D of A < 1.5 × median D of B. Then control **T2**, once: the job's Linux Playwright
  command on the whole suite with arm A and the probe copy; the rule above is applied to its one run with T's B median.
  T-H there: R1. Otherwise no code change; the registered runs are made on the code as it is, and the result recorded.
- **T-X (slow, but not as H says):** neither of the above (A slow, but without the intermediate preview, or with F < 0.5,
  the time spent with the page free: the engine or a wait). Stop and amend before any code; no remedy is registered for it.

The outcome is written in amendment 1 before any code.

## Design (registered): R1, a single over-limit edit

Under T-H the time goes to an intermediate preview the test does not check. The property of the test is that a mesh over
the preview limit is refused with the catalogue's line for 1004 and Build anyway, and that the refusal goes when the mesh is
back under the limit. **R1** reaches it with one edit and no intermediate preview, as the walk of
[ADR 0144](0144-wave48j-mesh-run-toggle-and-live-preview.md) (its manual step 5, "Type `100000` into `mesh.nx` ...") does:
`mesh.nx` 100000 with `ny` 32 gives 3,200,000 cells by the estimate, over 2,000,000, so the first preview after the edit is
the refusal; the restore is `mesh.nx` 32, back to the 1,024-cell mesh the test opened with. The assertions, their texts and
their 60 s waits are unchanged; `mesh.ny` is not typed. The control's ninth probe test shows before the code that the single
edit gives the refusal with its line.

Files changed under R1, only these:

- **C1.** `apps/studio-web/tests/e2e/error-walk.spec.ts`, the 1004 test (lines 177 to 189): the two edits of each pair
  become the one edit of `mesh.nx` above; a line of the head comment names ADR 0809 and why.
- **C2.** `apps/studio-web/tests/e2e/error-walk.spec.mutation.patch`, the committed mutation (rule 16), below.

No change of the job, the image or the app: the switch G1 stays for the whole Linux suite (ADR 0804), so what Linux CI
covers is unchanged. The cost of drawing a 48,000-cell mesh on SwiftShader is not a check of this item (below, not covered).
As in ADR 0804, the old lines stay in this ADR (above) and in git history, not as an ignored test (an ignored Playwright
test is a skip, which the job's skip step fails). Taken on the maintainer's standing instruction of 2026-11-11 (take the
recommendation).

## Set-up of the runs (fixed here)

ADR 0804's set-up, its scratch tools copied to this work's scratch folder with its paths: `run_job.py` runs the studio-web
job of the commit under test as written, step by step, as GitHub runs `shell: bash`, in a `git archive` copy; a `uses:` step
is replaced by a check of what it provides. **W**: Windows here, Git Bash, Node 22.19.0, Rust 1.99.0, `CARGO_TARGET_DIR`
this work's `target-win` under the scratch folder. **L**: the container, the image `rcfd-studio-linux-test:ubuntu24.04-f823b`
(ADR 0804's, built from the Dockerfile as it is at b1e2b376), started as ADR 0804's, the steps as the user `studio`, the
volumes `rcfd-f823-target` and `rcfd-f823-cache`. The heavy runs are made one after another, not at the same time, and
the load seen is recorded with each. Counts from the list output's marks and Playwright's summary lines, which must agree.

## Checks

The rows of ADR 0804 that missed (L1, L2, W1, W2, R7l) are made again on the code commit of this ADR; its rows that held and
whose files do not change here (K1w, K1l, the mutations MT, MW2, MS, MC) are carried over by A1 below, not run again.

| Id | Clause | Check | Limit | Source of the limit | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|---|
| E1 | the job fails on any test failure; no test skips (the test passes within its own wait) | L: `error-walk.spec.ts:177` in the job's Playwright step | passed; its waits 60 s and test timeout 120 s unchanged | ADR 0804 Q1 (a) and the brief: the 60 s kept | about 15 s | 4 | code verification |
| E2 | the same | W: the same | as E1 | the same | about 10 s | 6 | code verification |
| L1 | builds the engine first so the service e2e tests and the F-824 check do not skip | L: the Playwright step's counts | 880 passed, 0 failed, 0 skipped or not run | acceptance of F-823, last sentence; ADR 0804 L1 | as the limit | exact | code verification |
| L2 | the job runs | L: every step of the job as written, in order | each step passed, the skip step included; the upload skipped by its `failure()` | ADR 0804 L2 | passes | exact | code verification |
| W1 | the same | W: as L1 | as L1 | ADR 0804 W1 | as the limit | exact | code verification |
| W2 | the same | W: as L2 | each step passed, the Linux steps skipped by their `if:`, the upload by `failure()` | ADR 0804 W2 | passes | exact | code verification |
| R7w | rule 7 | the worktree's `apps/studio-web` on Windows, `CARGO_TARGET_DIR` = `target-win`: `npm run typecheck`, then `npm test` | exit 0; 880 passed, 0 failed, 0 skipped | rule 7; ADR 0804 R7w | passes | exact | code verification |
| R7l | rule 7 | in the container on a copy of the code commit: `npm run typecheck`, then `RCFD_TEST_ELECTRON_ARGS="--ignore-gpu-blocklist --enable-unsafe-webgpu" xvfb-run --auto-servernum npm test` | as R7w | rule 7; ADR 0804 R7l | passes | exact | code verification |
| V1 | the workflow is valid | `actionlint` 1.7.12 on `.github/workflows/ci.yml` | exit 0, no message | ADR 0804 V1 | exit 0 | exact | code verification |
| A1 | only the named files change | `git diff --stat b1e2b376..HEAD` | only C1, C2 (if R1), this ADR and the proposal file | the brief, owned files | as the limit | exact | code verification |
| DS | rule 7 (docs) | `cargo test -p rcfd-cli --release --test docs_site` | passes | rule 7 | passes | exact | code verification |
| P1 | rules 7 and 9 | `python scripts/roadmap_proposals.py check docs/roadmap/proposals/0809-F-823.json`; `python scripts/process_report.py --check` | the proposal valid; the report clean but for a generated file that is only not current | rules 7 and 9 | passes | exact | code verification |

Each registered run is made once. A failed or flaky test is recorded with its output as a miss; nothing is run again to
pass. `cargo fmt`, `clippy` and `baseline_hashes` do not apply: no Rust file changes. If T does not lead to R1, E1 to A1 are
made on the code as it is (A1: this ADR and the proposal file only).

## Mutations (task 3, rule 16)

On scratch copies of the code commit, each once:

| Id | Mutation | Where, how | Caught when |
|---|---|---|---|
| ME | committed (C2): the preview's refusal loses Build anyway (`MeshPreview.tsx` line 70, `tooLarge:` set to `false`) | W, a scratch copy with the patch applied, the engine built: `npx playwright test tests/e2e/error-walk.spec.ts --project=electron` after build and packaging | `error-walk.spec.ts:177` fails at the Build anyway line |
| MR | the remedy taken back: the test's original two-edit lines (b1e2b376) | this is control T's arm A; not made again | T's arm A as recorded |

C2's first line: `# expect-fail: cd apps/studio-web && npm run build && npm run package:dir && npx playwright test
tests/e2e/error-walk.spec.ts --project=electron`, the form of the patches already beside the e2e specs. The test needs
`rcfd` and `rcfd-mcp` in the checkout's `target/release` (it skips without them), as the other service specs' patches do.

## Size

Size (agent): done; 0; 4; 0; 150000

(Heavy runs: L, W, R7l and R7w, whole suites; control T is six runs of one file; T2, if it is needed, a fifth.)

## Decision rule

**Done** for F-823 if every registered row holds on both platforms here (E1, E2, L1, L2, W1, W2, R7w, R7l, V1, A1, DS, P1)
and ME is caught; **partial** otherwise, with the miss recorded. The lead's reading of the first run on GitHub Actions
after the push decides the item.

## Not covered

The runners themselves (`ubuntu-24.04` and `windows-latest`), as in ADR 0804. The time the Studio takes to draw a mesh
preview of tens of thousands of cells on a software WebGPU device: under R1 no test of this item draws one, and the
responsiveness spec's limits are the Studio's checks of the renderer's main thread. The local Docker runner
(`docker/linux-test/run.mjs`) is not changed.

## Open questions for the maintainer

None at registration. Any question of scope the control or the runs raise is listed in an addendum, not decided here.
