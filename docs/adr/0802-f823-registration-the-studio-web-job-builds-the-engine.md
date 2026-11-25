# ADR 0802: F-823 registration: the studio-web job builds the engine, so the tests that need it run

Status: **REGISTERED 2026-11-25, written before any code of F-823 and before any run of its checks.** Before this text no file
of the repository was changed. What was done: the reading below; the logs of the CI run 45705107206 read with `gh` (read
only) and parsed by a scratch script; `playwright` was not run; and, for the times of the build steps, one cold release build
of the unchanged `rcfd`, `rcfd-runner`, `rcfd-mcp` and the `f447_bench` example on Windows into this work's own target folder
and one in the Ubuntu 24.04 container on this work's own volumes. Results go in addenda at the end; nothing above them is
changed afterwards.

It answers the brief `docs/adr/briefs/f823-studio-ci-engine.md` ([the file](briefs/f823-studio-ci-engine.md)) and follows
[docs/process/agent-rules.md](../process/agent-rules.md), version 5, with [the Studio guardrails](../studio-guardrails.md).
Branch `f823-studio-ci-engine` from main dcb4a1df, scratch `w49i-work/f823-studio-ci-engine-scratch`. Model: Claude Opus 5.5.

## What is asked

The acceptance of F-823 in `docs/roadmap.json` (read by a script): "The Studio's unit and Playwright tests run as a job in
.github/workflows/ci.yml (ADR 0800 Q3, maintainer 2026-11-16), with npm ci from the committed lock file and Electron run
headless; the job fails the workflow on any test failure; docs/test-process.md names the job. A weekly, grouped Dependabot
npm entry for /apps/studio-web lands with the job (ADR 0801 Q2). The job builds rcfd-cli, rcfd-runner and rcfd-mcp first so
the service e2e tests and the F-824 inventory check do not skip." The `gap` names the last sentence. The lead decided (the
brief, 2026-11-25): the job builds the three binaries and the `f447_bench` example in release, packages the Studio with the
engine, and every test that still skips is named with a reason the code states, not a missing build; the time limit and any
cache are argued here from measured times; no test is skipped, removed or loosened to pass.

## What was read

`AGENTS.md`, `docs/process/agent-rules.md` (version 5) and the brief, each whole; `docs/studio-guardrails.md` and
`.github/workflows/ci.yml`, whole; section 8 of [docs/test-process.md](../test-process.md) (the job's section) and 7.4; of
[ADR 0801](0801-studio-web-dependencies-and-layout.md) the parts of addendum 19 on Linux (WebGL, the GPU, the container)
and the lines of addenda 13 and 14 that give the times of a whole `npm test`; [ADR 0150](0150-job48p-linux-example-stores.md),
its head and the name of its image; `apps/studio-web/package.json`, `playwright.config.ts`, `scripts/package-dir.mjs`,
`docker/linux-test/{Dockerfile,inside.sh,run.mjs}`, `src/main/engine/locate.ts` (the engine parts and `checkoutRelease`);
`tests/support/{f447,rcfd,project,runner}.ts` (how each test finds a binary); every `test.skip(` of `tests/` (86 calls, with
their blocks); `.github/dependabot.yml`; the items F-823 and F-824 (by a script); the logs of the studio-web jobs and the
step times of the four test jobs of run 45705107206.

## Findings of the reading (before any run)

1. **The run 45705107206 (2d125d55), studio-web**: on `ubuntu-24.04` and on `windows-latest` 596 passed and 284 skipped, 880
   tests; the 284 are the same tests on both platforms (the lists compared, path separators made equal): 45 of the project
   `unit` and 239 of `electron` (the gap says 44 unit; the logs give 45). Nothing in `apps/studio-web` or `.github` changed
   between 2d125d55 and dcb4a1df, so the suite here is that suite.
2. **Every skip is a missing build.** Each of the 284 lies under at least one `test.skip(condition, reason)` of its file, and
   every condition is a binary that was not found: by the parts its conditions name (the reason strings are the constants
   `NO_RCFD`, `NO_MCP`, `NO_RUNNER`, `NO_BENCH` of `tests/support/` or a literal of the same form):

   | Needs | Tests (unit, electron) | Files |
   |---|---|---|
   | rcfd, rcfd-mcp | 65 (0, 65) | a11y 12, cancel 3, case-editor 8, close-guard 5, error-walk 8, f836 5, f891 4, theme-surfaces 7, undo-case 3, word-budget 10 |
   | f447_bench | 62 (27, 35) | e2e: data 8, field-derived 10, field 17; unit: compare 1, data-routes 23, field-derived 3 |
   | rcfd-mcp, rcfd-runner | 52 (0, 52) | claude-code 4, compare 7, provenance 10, run-control 8, runner-lifetime 8, runs 3, sites 6, workspace 6 |
   | rcfd | 47 (12, 35) | e2e: actions-rcfd 3, case-file-adr0737 5, plots 5, service 9, settings-rcfd 6, settings-ui-f826 3, undo-rcfd 4; unit: capabilities 1 (the F-824 inventory, `capabilities.spec.ts:210`), derive-bridge 1, field-derived 2, help-engine 2, leftover-sweep 1, mesh-preview 2, session-journal-adr0735 1, settings-schema 2 |
   | rcfd-mcp | 28 (2, 26) | e2e: commands 10, idle-frames 2, project 10, run-bar-keyboard 1, trash 3; unit: compare 1, field-derived 1 |
   | rcfd, rcfd-mcp, rcfd-runner | 13 (0, 13) | f837 13 |
   | rcfd-mcp, rcfd-runner, rcfd beside rcfd-mcp | 6 (0, 6) | first-path 6 |
   | f447_bench, rcfd-mcp | 5 (0, 5) | cancel 3, responsiveness 1, workspace-first-open 1 |
   | rcfd-runner | 2 (2, 0) | run-leftovers 2 |
   | f447_bench, rcfd | 1 (1, 0) | data-routes 1 |
   | rcfd and rcfd-runner beside rcfd-mcp | 1 (1, 0) | sites 1 |
   | the engine packaged in `out/packaged/resources/bin`, rcfd-mcp | 1 (0, 1) | project 1 (`project.spec.ts:302`) |
   | the engine packaged in `out/packaged/resources/bin`, rcfd | 1 (0, 1) | service 1 (`service.spec.ts:229`) |

   No skip call of the suite has a condition of the platform, a display, a GPU or anything else. So the change must make all
   284 run, and **the registered list of the tests that stay skipped is empty, on both platforms**. The service e2e tests
   (`service.spec.ts`, 10) and the F-824 inventory check are among them.
3. **Where the tests and the packaging look.** The tests find each binary by its variable, then `checkoutRelease`
   (`$CARGO_TARGET_DIR/release`, else `<repo>/target/release`), then `target/release` of every folder above the app; the bench
   in `RCFD_F447_BENCH`, `$CARGO_TARGET_DIR/release/examples`, then `target/release/examples` above. `package-dir.mjs` without
   `--no-engine` copies `rcfd`, `rcfd-mcp` and `rcfd-runner` from `checkoutRelease` into `out/packaged/resources/bin` and fails,
   naming the file and the cargo command, when one is missing. On the runner the default `target/release` of the checkout is
   found with no variable. Here, a worktree inside the main checkout would also find the main checkout's `target/release`
   (it holds all four), which would hide a missing binary: the job runs and the mutations below are therefore made in copies
   (`git archive`) under the scratch folder, where no folder above has a `target`.
4. **Linux without a GPU** (ADR 0801 addendum 19, "Linux bugs found and fixed" and the list of specs that cannot pass): with
   no GPU Chromium turns off WebGL as well as WebGPU, the field views never report a first picture and the plots fail (13
   failed, 52 did not run in its run 2); with `--ignore-gpu-blocklist`, passed to the tests' Electron through
   `RCFD_TEST_ELECTRON_ARGS` (`tests/support/launch.ts`), WebGL 2 runs on Mesa's llvmpipe and every one of them passed. A
   GitHub runner has no GPU. Those specs ran in CI so far only as skips (they need the bench or the engine). The switch is a
   setting of the test environment, not of the app; the Studio's own launch is unchanged.
5. **The container.** ADR 0150's image `rcfd-linux:ubuntu24.04-rust1.99.0` holds Rust only (no Node, no Electron libraries,
   no Xvfb). The Studio's Ubuntu 24.04 image is ADR 0801 addendum 19's `rcfd-studio-linux-test:ubuntu24.04` (Ubuntu 24.04.5,
   glibc 2.39, Rust 1.99.0, Node 22.19.0, Xvfb, Mesa): that image is used, the same distribution as ADR 0150's. Taken on the
   maintainer's standing instruction of 2026-11-11 (take the recommendation).
6. **The section.** The brief names "the sentence of `docs/test-process.md` section 7 that names the job"; the job is named in
   section 8, "The Electron Studio's npm tests (CI job `studio-web`)" (section 7 is the daily gate; 7.4 names `npm test` only).
   The sentence changed is the lead-in of section 8's list of steps ("The job, in `apps/studio-web/` with Node 22.19.0 ...").
   Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).

## Times measured (for the time limit and the cache)

| Where | What | Time |
|---|---|---|
| CI run 45705107206, unit-tests, ubuntu-24.04 | `cargo build --release --locked -p rcfd-cli`, dependencies restored by rust-cache (cache hit) | 129 s |
| the same, windows-latest | the same | 256 s |
| CI run 45705107206, studio-web, windows-latest / ubuntu-24.04 | the whole job without the engine (Playwright 71 s / 58 s, 596 tests, 239 e2e skipped) | 132 s / 102 s |
| here, Windows 11, 16 logical processors shared with three other agents | cold target folder: `-p rcfd-cli -p rcfd-runner -p rcfd-mcp` / the example | 85 s / 66 s |
| here, the container (16 processors, the same machine) | cold target folder and an empty registry: the same two | 52 s / 46 s |
| ADR 0801 addenda 13 and 14, Windows here | a whole `npm test` with the engine, 658 tests / 638 under load | 8.7 min / 20.7 min |

Worst case on the runner, argued: the build of the three binaries and the example from a cold cache is taken as the local cold
time scaled by the runner's measured slowness on `rcfd-cli` with a warm cache (129/52 = 2.5 Linux, 256/85 = 3.0 Windows), and
doubled for the dependencies that a cold cache compiles too: about 8 min on Linux and 15 min on Windows. Playwright: the
recorded 20.7 min under load scaled to 880 tests is 28 min. npm ci, the build and the packaging: 1 min. Windows worst: about
44 min, Linux about 37 min. **`timeout-minutes: 60`** (the unit-tests job's), margin 60/44 = 1.4 on the worst estimate. No
check row is a wall-clock limit (the brief); the time limit is the job's setting and the lead reads the first run's times.

**Cache**: `Swatinem/rust-cache`, at the commit the workflow already pins for the other Rust jobs (no new action), with its
default key (job, runner OS, toolchain, `Cargo.lock`): it keeps the registry and the compiled dependencies, never the
workspace's own crates, which are built every run from the commit under test; it is saved only after a job that passed. npm's
download cache stays as it is (keyed on the lock file); `node_modules`, builds, the packaged app and test results are never
cached.

## Design (registered)

The studio-web job of `.github/workflows/ci.yml` only; its name, matrix (both systems on every pull request), Node version,
`npm ci`, type checks, build, the Linux sandbox step, the Playwright command and reporters, the upload on failure and the licence
check stay.

- **C1. Toolchain and cache before the build**: `dtolnay/rust-toolchain` with `RUST_GATE_TOOLCHAIN` moves to the head of the
  job, on both systems (it was a Linux-only step before the licence check; that one is removed, the toolchain is there), then
  `Swatinem/rust-cache`.
- **C2. The engine**, a step from the repository root: `cargo build --release --locked -p rcfd-cli -p rcfd-runner -p rcfd-mcp`,
  then `cargo build --release --locked -p rcfd-api --example f447_bench` (two commands: a target selection with `--example`
  would leave the binaries of the other packages out). Into the checkout's `target/release`, where the tests and the packaging
  look with no variable (finding 3).
- **C3. Packaging with the engine**: `npm run package:dir` without `--no-engine`.
- **C4. Linux, the runner**: the sandbox step also installs Mesa's DRI drivers (`libgl1-mesa-dri`, the package the container
  image has) when the runner lacks them, as it installs Xvfb when missing; and the Playwright step's Linux branch passes
  `RCFD_TEST_ELECTRON_ARGS=--ignore-gpu-blocklist` (finding 4). Windows is unchanged.
- **C5.** `timeout-minutes: 60`; the comments of the job say what it builds and why.
- **C6. `docs/test-process.md`**, section 8, the one sentence of finding 6: the job first builds the three binaries and the
  example in release from the repository root, with the dependency cache, and then runs the steps listed.

## Set-up of the checks (fixed here)

"The job's steps as written" is the scratch script `run_job.py`: it reads `.github/workflows/ci.yml` of the commit under test
and runs the studio-web job's steps in their order; each `run:` as GitHub runs `shell: bash` (`bash --noprofile --norc -eo
pipefail`), in its working directory, with the workflow's, job's and step's `env`, `RUNNER_OS` and `CI=true`; an `if:` of the
forms `runner.os == '...'` and `failure()` is evaluated; after a failed step only `failure()` steps run. A `uses:` step is
replaced by a check of what it provides: setup-node by `node --version` equal to the step's version, rust-toolchain by
`rustc --version` of the step's toolchain, setup-python by a Python 3; checkout by the copy itself; rust-cache and
upload-artifact by nothing (no cache is restored here). The copy is `git archive` of the commit under test in the scratch
folder.

- **Windows here** (W): Git Bash, Node 22.19.0, Rust 1.99.0, with `CARGO_TARGET_DIR` set to this work's own folder
  `target-win` in the scratch folder (the brief); the tests and the packaging take that folder first (finding 3). This
  machine has a GPU; the runner has none (not covered).
- **Ubuntu 24.04 in Docker** (L): the image of finding 5, started with `--shm-size 2g --security-opt seccomp=unconfined` as
  `run.mjs` starts it; the steps run as the unprivileged user `studio` (Chromium's sandbox, G10), as the runner's steps run
  as `runner`; no `CARGO_TARGET_DIR`: the checkout's `target` is this work's volume `rcfd-f823-target`, so the runner's
  layout is the one exercised; `CARGO_HOME` and the download caches on the volume `rcfd-f823-cache`. The kernel has no
  `apparmor_restrict_unprivileged_userns` and the image has Xvfb and Mesa, so the sandbox step changes nothing there.
- The counts are read from the Playwright step's list output (marks ✓, ✘, -), by test, with the file paths made equal.

## Checks

| Id | Clause | Check | Limit | Source of the limit | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|---|
| A5w | builds the engine first so the service e2e tests and the F-824 check do not skip | W: the Playwright step's counts and its skipped tests | 880 passed, 0 failed, 0 skipped; the skipped list equal to the registered list (empty); `capabilities.spec.ts:210` and the 10 tests of `service.spec.ts` among the passed | acceptance of F-823, last sentence; the brief, task 1 | as the limit | exact | code verification |
| A5l | the same | L: the same | as A5w | the same | as the limit | exact | code verification |
| S1w | the job runs | W: every step of the job as written, in order | each step passed, or skipped by its `if:` (the Linux steps); the engine step leaves the four files in `target-win/release` | the brief, task 1 | passes | exact | code verification |
| S1l | the same | L: the same | each step passed (the Linux steps run, the upload skipped by `failure()`) | the brief, task 1 | passes | exact | code verification |
| A2w | the job fails the workflow on any test failure | negative control on a copy (not committed): one `expect` of `tests/unit/tokens.spec.ts` made false; the Playwright step's Windows command on that file: `npx playwright test --reporter=list,html tests/unit/tokens.spec.ts` under `bash -eo pipefail` | exit status not 0, one failed test; the same command on the unchanged copy exit 0 | acceptance of F-823 | as the limit | exact | code verification |
| A2l | the same | L: the same file with the Linux command, `RCFD_TEST_ELECTRON_ARGS=--ignore-gpu-blocklist xvfb-run --auto-servernum npx playwright test --reporter=list,html tests/unit/tokens.spec.ts` | as A2w | acceptance of F-823 | as the limit | exact | code verification |
| A1 | a job of ci.yml, npm ci from the lock file, Electron headless | the diff of the workflow | `npm ci`, the Playwright command (Xvfb on Linux, `RCFD_TEST_HEADLESS` of the config) and the job's name and matrix unchanged; no other job changed (`git diff` of the other jobs empty) | acceptance of F-823; the brief, task 2 | as the limit | exact | code verification |
| A3 | docs/test-process.md names the job | the sentence of C6; `cargo test -p rcfd-cli --release --test docs_site` | the sentence names the job and its engine build; `docs_site` passes | acceptance of F-823; rule 7 | passes | exact | code verification |
| A4 | Dependabot npm entry, weekly, grouped | `git diff origin/main -- .github/dependabot.yml` | empty (the entry for `/apps/studio-web`, weekly, five groups, unchanged) | acceptance of F-823 | empty | exact | unchanged default output |
| V1 | the workflow is valid | `actionlint` 1.7.12 on `.github/workflows/ci.yml` (no shellcheck on this machine, so its shell checks do not run); control: the file of main | after the change: exit 0 and no message; the control's messages, if any, stated | the brief, task 1 ("a named check") | exit 0 | exact | code verification |
| R7 | rule 7 | in the worktree's `apps/studio-web`, with `CARGO_TARGET_DIR` set to `target-win`: `npm run typecheck`, then `npm test` | exit 0; counts as A5w | rule 7; the brief, task 4 | passes | exact | code verification |
| P1 | rule 7, rule 9 | `python scripts/roadmap_proposals.py check`; `python scripts/process_report.py --check` | the proposal file valid; the report clean but for a generated file that is only not current | rules 7 and 9 | passes | exact | code verification |

Each registered run is made once. A failed or flaky test is recorded with its output as a miss; nothing is run again to pass.

## Mutations (task 3), each on a scratch copy, none committed

The workflow is not a Rust or Studio test target, so no `*.mutation.patch` is committed (the brief; rule 16's committed
mutation does not apply). Each is a copy of the changed `ci.yml` in a `git archive` copy under the scratch folder, run on
Windows with `run_job.py`, with a fresh `CARGO_TARGET_DIR` of its own in the scratch folder (cold, so no binary of an earlier
build is there):

| Id | Mutation | Seen when |
|---|---|---|
| M1 | `-p rcfd-runner` left out of the engine step | the job fails (expected at the packaging step, which names `rcfd-runner`); and the count row of the unit project with that target (`npx playwright test --project=unit`, the Playwright step's command limited to the unit project) has skipped tests, 3 expected (`run-leftovers` 2, `sites` 1) |
| M2 | the `f447_bench` command left out | the job's steps up to the packaging pass, and the count row of the unit project has skipped tests, 28 expected (`data-routes` 24, `compare` 1, `field-derived` 3) |
| M3 | `--no-engine` kept on `npm run package:dir` (the target of M2's copy is complete for this: M3 adds back the bench command on that copy) | the count row of `tests/e2e/project.spec.ts` and `tests/e2e/service.spec.ts` has skipped tests, 2 expected (`project.spec.ts:302`, `service.spec.ts:229`) |

A mutation is caught if its row shows what the column says (a failed job, or skipped tests where the registered list has none);
the expected numbers are from finding 2 and are not limits.

## Size

Size (agent): done; 0; 2; 2; 180000

(Heavy runs: the two whole suites, W and L; the other runs are a unit project, two spec files, two cold builds of about 2.5
min each and the link check. Questions: the two below.)

## Decision rule

**Done** if every registered row holds on both platforms here and each of M1 to M3 is caught; **partial** otherwise, with the
miss recorded. The lead's reading of the first run on GitHub Actions after the push decides the item; the verdict of this work
is the one the rule gives for the rows here.

## Not covered

The runners themselves: their speed, their missing GPU on Windows (this machine has one, so the Windows field views draw on a
GPU here and on WARP or nothing on `windows-latest`), whether `windows-latest` gives WebGL without a switch, rust-cache's
behaviour and the time limit. They are seen by the lead's first run. WebGPU on Linux (never available in the container).

## Open questions for the maintainer (to be answered with the results)

- **Q1.** Whether a test that still skips should be made to run by a change of the test or the app. Registered: none is
  expected to remain (finding 2); the addendum answers it from the runs.
- **Q2.** If the first run on `windows-latest` shows the field views without WebGL (no GPU on the runner), whether to pass
  `--ignore-gpu-blocklist` on Windows too. Options: (a) pass it on Windows as on Linux, a setting of the test environment
  only; (b) leave Windows as it is and record those tests as known misses of the runner; (c) run the field specs on Windows
  only on a machine with a GPU. Recommendation: (a), the change of C4 on the other branch of the step.
