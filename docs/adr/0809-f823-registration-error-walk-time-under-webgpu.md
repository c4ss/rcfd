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

## Amendment 1 (2026-11-26): the controls' results; H does not hold, the time is the engine's; no code change

Written and committed after the controls T and T2 and before any registered run. Nothing above is changed. Both were made
once, in the container on a `git archive` copy of b1e2b376 with the probe copy (scratch folder: `T-overlay/`, `T/`, `T2/`,
`T.log`, `T2.log`; host CPU load sampled every 30 s in `T-hostload.log` and `T2-hostload.log`).

### Control T (the file alone, six runs)

| Run | Arm | loadavg before | D (ms) | I | F | s4 end (ms) | s10 end (ms) | other failures |
|---|---|---|---|---|---|---|---|---|
| 1 | A | 8.93 | 1,053 | false | 0.000 | 1,283 | 37,265 | probe 9 |
| 2 | B | 5.43 | 49,485 | false | 0.000 | 49,691 | 52,244 | 1201 LOCK_BUSY ("Failed to save" not within 30 s); probe 9 not run |
| 3 | A | 3.49 | 45,536 | true | 0.004 | 45,851 | 47,613 | probe 9 |
| 4 | B | 2.91 | 31,200 | true | 0.002 | 31,922 | 33,334 | probe 9 |
| 5 | A | 3.94 | 43,571 | true | 0.002 | 43,888 | 46,412 | probe 9 |
| 6 | B | 5.13 | 39,489 | true | 0.003 | 39,819 | 41,517 | probe 9 |

Host CPU over T: 57 % to 100 % (the lead's runs; median about 90 %). Median D of A 43,571 ms, of B 39,489 ms; ratio 1.10, under
1.5: **T-N** by the rule, so T2 was made. The ninth probe test failed in every run in its own `openCase` ("Edit
cases/cavity-re100.toml" not visible: the test before it leaves the Runs tab open), a defect of the probe; it gave no number.

### Control T2 (the whole Linux suite, arm A, once)

The 1004 test: D 40,518 ms, I true, F 0.002, the longest round trip 81 ms; the test passed in 42.1 s. By the rule: not T-H.
**So no code change; the registered runs are made on the code as it is.** R1, C1 and C2 are not made.

### What the samples and the code show (H is wrong)

In every run but 1 the samples show the page free (F at most 0.004; round trips at most 81 ms) and the preview's call
pending (`aria-busy` true) from about 1 s to the refusal; where the intermediate 1500 × 32 mesh is shown (runs 3 to 6, T2)
it is drawn within 0.3 s of its reply on either back end. The time is not the renderer's. From the code: the case editor's
estimate line asks `case.facts` again after every settings change (`src/renderer/case/estimate.tsx` lines 16 to 27);
`case.facts` builds the whole mesh before its `max_cells` check (`crates/rcfd-api/src/facts.rs` lines 384 to 386), with
`ny` 1500 a mesh of 2,250,000 cells; and the service answers one connection's requests one after another
(`serve_stream`, `crates/rcfd-api/src/service.rs` line 1916), the Studio's dispatcher being one connection. So the
preview's refusal waits behind `case.facts` building a mesh of 2.25 million cells, on WebGL 2 as on WebGPU. In run 1 the
preview's call went first (the refusal at 0.84 s) and the restore's preview waited instead (s10, 35.5 s); in run 2 the save
of the next test waited past its 30 s. ADR 0804's reading (WebGPU on SwiftShader) is not borne out: the 28.7 s, 47.3 s and
over 60 s it saw are the time of that build on a shared, loaded machine. R1 would not remove it: `mesh.nx` 100000 gives
`case.facts` 3,200,000 cells to build. This is the case T-X describes, met inside T-N; the registered rule is followed as
written.

### Consequences, stated before the runs

- The registered runs (L, R7l, W, R7w, V1, A1, DS, P1) are made on the commit of this amendment. A1's limit is this ADR and
  the proposal file only. E1 and E2 are read from L and W as registered; a miss is recorded as a miss.
- ME is made as registered on a Windows scratch copy (the patch applied to a copy, not committed: no Studio file changes,
  so rule 16 asks for no committed patch); it shows whether the unchanged test catches the mutation.
- The remedy is in the engine or in the Studio's estimate line, not in the files this brief owns: an open question of scope
  (addendum).

## Addendum 1: results (2026-11-26)

No code changed (amendment 1). Each registered run was made once, on a `git archive` copy of a5b7d0b8 (R7w in the
worktree), one after another and beside no other run of this work: L 15:10 to 15:35, R7l to 15:48, W to 16:05, R7w to
16:18, then ME. The machine was not quiet. The host CPU, sampled every 30 s (`heavy-hostload.log`), was at 90 % to 100 %
through L and most of R7l, and between 17 % and 100 % (mostly 55 % to 100 %) through W and R7w, from processes not of this
work. The logs are in the scratch folder (`L/`, `R7l/`, `W/`, `R7w.log`, `R7w-npm-test.log`, `ME/`, `ME.log`).

### Checks

| Id | Limit | Measured | Met |
|---|---|---|---|
| E1 | L: `error-walk.spec.ts:177` passed, its waits unchanged | passed (1.0 m in the list output) | met |
| E2 | W: the same | passed (29.4 s); in R7w 36.8 s, in R7l 26.0 s | met |
| L1 | 880 passed, 0 failed, 0 skipped or not run | 865 passed, 4 failed, 11 did not run (the summary lines and the marks agree). Failed: `commands.spec.ts:156` (the palette's first action `claude_code.register`, not `run.status`, within 5 s), `responsiveness.spec.ts:260` (tasks over 100 ms or frames over 150 ms in every pass), `run-control.spec.ts:268` (`project.cancel_run` answered in 51.4 ms, limit 20 ms), `workspace-first-open.spec.ts:59` (tasks over 100 ms with the GPU process held) | **missed** |
| L2 | each step passed, the skip step included | steps 1 to 10 passed (engine 143 s); 11 failed (exit 1, 1297 s); 12 skipped after it; 13, the upload, ran (`failure()`) | **missed** |
| W1 | as L1 | 879 passed, 1 failed, 0 skipped. Failed: `claude-code.spec.ts:193`, rcfd-mcp's `edit_case`: "replace ...from-claude.toml: failed after 12 attempts: Access is denied. (os error 5)", a file refused by the operating system (the brief: reported, not retried), as in ADR 0804's W | **missed** |
| W2 | each step passed, the Linux steps skipped by their `if:` | steps 1 to 8 and 10 passed (engine 114 s, `npm ci` 39 s); 9 skipped by its `if:`; 11 failed (exit 1, 814 s); 12 skipped; 13 ran | **missed** |
| R7w | typecheck and `npm test` exit 0; 880 passed | typecheck exit 0; `npm test` exit 0, "880 passed (13.2m)" | met |
| R7l | as R7w | typecheck exit 0; `npm test` exit 0, "880 passed (13.0m)" | met |
| V1 | exit 0, no message | actionlint 1.7.12 (`-shellcheck=`): exit 0, no message; the file is b1e2b376's, unchanged | met |
| A1 | only this ADR and the proposal file | `git diff --stat b1e2b376..HEAD`: this ADR and `docs/roadmap/proposals/0809-F-823.json` (in the report) | met |
| DS | passes | run on this addendum before its commit; in the report | see the report |
| P1 | proposal valid; the report clean but for a generated file | in the report | see the report |

### Controls and mutations

T and T2: amendment 1 (H does not hold). ME, the preview's refusal without Build anyway (`MeshPreview.tsx` line 70,
`tooLarge: false`), on a Windows scratch copy of a5b7d0b8 after build and packaging: `error-walk.spec.ts:177` failed at
line 184 (`Build anyway` not visible), 3 passed, 4 did not run, exit 1: **caught**. MR is control T's arm A (amendment 1).
No patch is committed (no Studio file changed).

### Reading of the misses (from the logs; nothing was run again)

1. L's four are limits on time and on order. The same commit passed all 880 in R7l, in the same container, right after L.
   Three of the four (responsiveness, run-control, workspace-first-open) are ADR 0804's R7l misses under load, and
   `commands.spec.ts:156` waits 5 s for the palette's list. L ran while the host CPU was at 90 % to 100 %. Load is the
   likely cause; this is not shown.
2. W's failure is the file refused by Windows that ADR 0804's W met, in the same test; R7w passed it.
3. `error-walk.spec.ts:177` passed in all four runs. Its time is the engine's `case.facts` building the 2,250,000-cell
   mesh on the Studio's one connection (amendment 1): 26.0 s to about 60 s here, with the load. With the 60 s wait at
   line 182 kept, it can miss again on a loaded runner, on either back end.

### Deviations

The registered remedy R1 was not made, by the registered rule (T-N, then T2 not T-H). The probe's ninth test was defective
(its `openCase` from the Runs tab) and gave no number. Others loaded the machine through every run; this work ran
nothing else meanwhile.

### Not run

The runners themselves (as registered). `cargo fmt`, `clippy` and `baseline_hashes` do not apply (no Rust file changed).

### Size

Size (agent, actual): partial; 1; 5; 1; 160000

### Decision

By the registered rule: **partial**. E1, E2, R7w, R7l, V1, A1 and ME hold; L1, L2, W1 and W2 missed (four limits on time
and order in L under a loaded host, one file refused by Windows in W). The controls refute ADR 0804's reading (WebGPU on
SwiftShader): `error-walk.spec.ts:177` waits for the engine's `case.facts` of an over-limit mesh.

## Open questions for the maintainer

- **Q1.** `case.facts` builds a whole mesh before its `max_cells` check, and the Studio asks for it after every settings
  change on the connection its mesh preview uses. So any edit to an over-limit count holds the Studio's engine calls for
  tens of seconds, and a real user meets it too. Options: (a) a new registration (an engine item, or one under F-823 by
  rule 13) in which `case.facts` refuses by the counts' estimate before building, as `mesh.preview` does
  (`mesh_preview.rs`, `estimate`), so that its reply for an over-limit case gives the estimate instead of the built count;
  (b) the Studio does not ask for the estimate line while the preview's estimate is over the limit, or asks for it on a
  second connection (the engine still builds the mesh); (c) leave it, and keep the test's 60 s as it is.
  Recommendation: (a). It changes an engine reply, so it is a question of scope and is not taken here.
- **Q2.** The Linux job's time limits under load (L's four misses, and ADR 0804's three in R7l). Options: (a) the lead reads
  the first run on GitHub Actions, whose runners are not shared with other work, before anything is changed; (b) a
  registration that measures them, as amendment 1 did. Recommendation: (a).

## For the lead: roadmap entries

> Status: partial.

> Note: ADR 0809, 2026-11-26, partial: timed controls in the Linux container show that error-walk.spec.ts:177 waits for the engine's case.facts building the 2.25-million-cell mesh on the Studio's one connection, on WebGL 2 as on WebGPU (the renderer is free), not for a WebGPU draw; no code changed; the test passed in all four runs, and R7w and R7l passed 880 of 880, but the job's Linux run missed four time limits under a loaded host and Windows met a file refused by the OS.

> Gap: The job's runs here missed: in the Ubuntu 24.04 container 865 passed, 4 failed (time and order limits under a loaded host) and 11 did not run; on Windows one test met a file refused by the OS. error-walk.spec.ts:177 passes, but its time is case.facts building an over-limit mesh before its max_cells check (Q1 of ADR 0809). The first run on GitHub Actions is to be read.

> Follow-ups: none as new items here; Q1 (a) is proposed as an engine registration, a question of scope for the maintainer.

The same entries are in `docs/roadmap/proposals/0809-F-823.json`.

## Answers of the maintainer (2026-11-26), written by the lead

1. Q1 (a): an engine registration in which case.facts refuses by the counts' estimate before it builds, ADR 0812; taken by the lead on the maintainer's standing instruction of 2026-11-26 (go with the recommended option always).
2. Q2 (a): the lead reads the first GitHub Actions run of the Linux job before any registration of the time limits under load; taken by the lead on the maintainer's standing instruction of 2026-11-26 (go with the recommended option always).
