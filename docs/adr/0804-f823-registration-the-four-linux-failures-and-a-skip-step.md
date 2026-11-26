# ADR 0804: F-823 registration: the four Linux failures of the studio-web job, and a step that fails on a skip

Status: **REGISTERED 2026-11-26, written before any code of F-823 under this ADR and before any run of its checks.** Before
this text no file of the repository was changed and nothing was run but reading: files, the logs of ADR 0802's runs in its
scratch folder, and the sources of two installed npm packages (below). Results go in addenda at the end; nothing above them
is changed afterwards.

It answers the brief `docs/adr/briefs/f823-linux-e2e.md` (on main, issued 2026-11-26; not on this branch, so not linked)
and follows [docs/process/agent-rules.md](../process/agent-rules.md), version 5, with [the Studio
guardrails](../studio-guardrails.md). It is a further registration of F-823 under rule 13, after
[ADR 0802](0802-f823-registration-the-studio-web-job-builds-the-engine.md), on the same branch `f823-studio-ci-engine`
after its commit d5ec1145. Scratch folder `w49i-work/f823-linux-e2e-scratch`. Model: Claude Opus 5.5.

## What is asked

The acceptance of F-823 in `docs/roadmap.json` (read by a script): "... the job fails the workflow on any test failure ...
The job builds rcfd-cli, rcfd-runner and rcfd-mcp first so the service e2e tests and the F-824 inventory check do not skip."
ADR 0802 left it partial: on Windows 880 of 880 passed, in the Ubuntu 24.04 container 860 passed, 4 failed and 16 did not
run. The lead decided (the brief, 2026-11-26): Q1, make the four run and pass on Linux by a change of the job's environment
(the image's packages: a terminal emulator, a software WebGPU adapter) or, where the diagnosis shows a defect of the test or
the app, by a registered change of it, with no skip and no Windows-only e2e; Q2, `--ignore-gpu-blocklist` on Windows too if
`windows-latest` needs it; Q3, a step of the job that fails on any skip not in the registered list.

## What was read

`AGENTS.md`, `docs/process/agent-rules.md` (version 5) and the brief, each whole; ADR 0802 whole; `.github/workflows/ci.yml`
whole; of ADR 0801 addendum 19 the parts on WebGL, WebGPU and the container (its lines 2670 to 2765); the four failing specs
(`claude-code.spec.ts` lines 1 to 60 and 170 to 215, `commands.spec.ts` 1 to 116, `runner-lifetime.spec.ts` 1 to 225,
`workspace-first-open.spec.ts` 1 to 120); `tests/support/{gpu-hold,launch,rcfd,runner}.ts` (the parts named below);
`src/main/claudecode/terminal.ts`; of `src/main/index.ts` its `isFile` (lines 99 to 101); of
`src/renderer/commands/CommandPalette.tsx` lines 80 to 140 and 275 to 292; of `crates/rcfd-runner/src` the shutdown
(`client.rs` 495 to 510), `serve` (`runner.rs` 178 to 222) and `keep_started`/`reap_started` (`sys.rs` 172 to 188);
`docker/linux-test/Dockerfile`; `package.json` (scripts), `playwright.config.ts`; the logs `linux-job/steps.tsv` and
`linux-job/step-11.log` of ADR 0802's scratch folder; two installed npm sources: React Aria's
`node_modules/react-aria/dist/private/focus/FocusScope.mjs` (lines 520 to 550) and Playwright 1.65.1's
`node_modules/playwright/lib/runner/index.js` (`resolveOutputFile`). The item F-823 by a script.

## Diagnosis (hypotheses, each with the control that tests it)

The four failures of ADR 0802's run L, and the 16 tests that did not run:

1. **`claude-code.spec.ts:177`, "Open Claude Code here"** (log: `Expected: 1, Received: 0`, the poll for one recorded
   launch timed out). `launchPlan` of `src/main/claudecode/terminal.ts` (lines 64 to 67) gives a launch on Linux only when
   `isFile('/usr/bin/x-terminal-emulator')` (`statSync`, which follows a link); without it the plan has no command, the
   test mode records nothing. *Hypothesis H1:* the image has no `/usr/bin/x-terminal-emulator`; the Debian alternative of
   that name is provided by a terminal package, and Ubuntu's `xterm` registers it (taken from memory: no source could be
   read here; control D1 shows it). The app is right (it says so to the user); the environment lacks the terminal.
   *Change:* the image and the runner get `xterm` (in test mode it is never opened).
2. **`commands.spec.ts:90`, "Escape gives the focus back"** (log: line 113, `Expected: "button:Open project folder…",
   Received: "body"`). The palette is a React Aria `ModalOverlay`/`Dialog`; on its unmount React Aria's `FocusScope` puts
   the focus back on the element that had it in a `requestAnimationFrame` callback (`FocusScope.mjs` lines 530 to 541),
   not synchronously, so between the dialog's removal and the next frame the focus is on `body`. Line 113 reads the focus
   once, without a retry, right after `toBeHidden()`; the next line (`toBeFocused()`) would retry. ADR 0801 recorded the
   same failure on Windows under load. *Hypothesis H2:* a defect of the test: a one-shot read of a state that the library
   sets one frame later; under Xvfb with software drawing the frame comes later than the read. *Change:* line 113 becomes a
   polled assertion of the same value (`await expect.poll(focusedName).toBe(before)`, Playwright's default 5 s), the
   property unchanged.
3. **`runner-lifetime.spec.ts:192`, "Keep them running and close"** (log: line 216, `expect(alive(runnerPid)).toBe(false)`
   received `true`, right after `runner.cleanUp(id)`). `cleanUp` (`tests/support/runner.ts` 94 to 126) asks the runner to
   shut down (`rcfd-runner shutdown` returns once the lock is released, `client.rs` 498 to 509; the process exits after
   that, `runner.rs` 216 to 222), then, if `alive(runnerPid)`, sends it a signal and returns at once. `alive`
   (`tests/support/rcfd.ts` 94 to 101) is `process.kill(pid, 0)`. On Linux that answers for a process that has ended and
   is not yet reaped (a zombie), and the process that started the runner here is `rcfd-mcp`, which reaps its started
   runners only when it next calls `lock_held` or `pid_alive` (`sys.rs` 180 to 188); and a signal is asynchronous, so a
   look right after it can see the process still running. *Hypothesis H3:* the runner had ended or was ending, and the
   test's look saw a zombie (H3a) or a process a few milliseconds from its exit (H3b); either is a defect of the test's
   support, not of the runner. *Changes (both, since both are true of the code whichever caused this failure):* C7,
   `alive` on Linux counts a process whose `/proc/<pid>/stat` state is `Z` as ended; C8, `cleanUp` waits, at most 10 s
   (the bound the same spec gives "the runner to end", line 361), for each process it signalled to end.
4. **`workspace-first-open.spec.ts:59`** (log: `no WebGPU adapter: the GPU process cannot be held`, from
   `tests/support/gpu-hold.ts` line 40). The test needs a WebGPU device of its own and has no skip for its absence. ADR
   0801 addendum 19: with `--ignore-gpu-blocklist` Chromium gives WebGL 2 on llvmpipe but no WebGPU adapter, although the
   image has Mesa's Vulkan drivers (`mesa-vulkan-drivers`, lavapipe) and `libvulkan1`. *Hypothesis H4:* Chromium on Linux
   exposes WebGPU only with `--enable-unsafe-webgpu`, and its Dawn backend on Linux is Vulkan, which may also need the GPU
   process's Vulkan (`--enable-features=Vulkan`); a software Vulkan device (Mesa's lavapipe, or the SwiftShader that
   Electron ships) then gives an adapter. These switch names are from memory (no source could be read here; control D4
   tests them). *Change:* the Linux branch of the Playwright step passes the set of switches that control D4 selects by the
   rule below, through `RCFD_TEST_ELECTRON_ARGS` (a setting of the tests' Electron, `tests/support/launch.ts`, not of the
   app); the runner gets `mesa-vulkan-drivers` when it lacks it.
5. **The 16 that did not run** (`commands` 9, `runner-lifetime` 6, `claude-code` 1): each file is `mode: 'serial'`, and
   Playwright does not run the rest of a serial group after a failure. No change of their own: they run when the test
   before them passes, and the rows below require them to pass.

With these, every skip the suite has on Windows and Linux stays as ADR 0802 found it: none. **The registered list of tests
allowed to skip is empty, on both platforms.**

### Controls of the hypotheses (made after this commit, before any code; on scratch copies of d5ec1145)

In the container (ADR 0802's set-up: the image `rcfd-studio-linux-test:ubuntu24.04`, this item's volumes
`rcfd-f823-target` and `rcfd-f823-cache`, the user `studio`, `xvfb-run --auto-servernum`, `RCFD_TEST_ELECTRON_ARGS` with
`--ignore-gpu-blocklist` as ADR 0802's job). Each is made once; its output is recorded in the addendum.

| Id | Tests | Control | Expected if the hypothesis holds | If not |
|---|---|---|---|---|
| D1 | H1 | `test -e /usr/bin/x-terminal-emulator` in the image; then, as root in the same container, `apt-get install -y --no-install-recommends xterm` and again, with `readlink -f` | absent before; present after, a link to an xterm program | stop and amend before the code |
| D2 | H2 | `commands.spec.ts` with line 113 replaced, in the copy only, by records of `focusedName()` at once, after two animation frames and after 1 s, printed; `-g "Ctrl\+K opens"` | at once `body` or the button (a race); after two frames and after 1 s the button | if after 1 s it is still `body`: H2 is wrong, stop and amend |
| D3 | H3 | `runner-lifetime.spec.ts` with, in the copy only, before line 216, a print of `alive(runnerPid)`, of `/proc/<pid>/stat`'s state and parent, and of the parent's `comm`; `-g "Keep them running"` | state `Z` (H3a) or `R`/`S` turning gone within 10 s (H3b), or the process already gone (not reproduced) | a process in another state after 10 s: stop and amend |
| D4 | H4 | a probe spec in the copy only: the packaged Studio launched by `launchStudio`, then `navigator.gpu.requestAdapter()` and its `info`, `app.getGPUFeatureStatus()` and one `page.screenshot` (10 s), for each set below in its order | an adapter, WebGL 2 still `enabled`, the screenshot made | if no set gives all three: stop and amend |

D4's sets, in order, each with `--ignore-gpu-blocklist` first: **G1** `--enable-unsafe-webgpu`; **G2** G1 and
`--enable-features=Vulkan`; **G3** G2 and `--use-vulkan=swiftshader --use-webgpu-adapter=swiftshader`. **Rule, fixed here:**
the first set in this order that gives all three is the one the job uses; the probe runs all three regardless, and all are
recorded. The order is the least change first: G1 a switch that only unlocks the API, G2 the system's Vulkan
(lavapipe, a package of the image), G3 Electron's own SwiftShader (no package).

## Design (registered)

Files changed, only these (the brief's owned files; the test files are named here because the diagnosis shows their defect):

- **C1.** `.github/workflows/ci.yml`, the studio-web job, the Linux preparation step: also installs `xterm` when
  `/usr/bin/x-terminal-emulator` is missing and, when D4 selects G1 or G2, `mesa-vulkan-drivers` when it is not
  installed (as it installs Xvfb and Mesa's DRI drivers when missing).
- **C2.** The Playwright step: the Linux branch's `RCFD_TEST_ELECTRON_ARGS` becomes the set D4 selects; both branches add
  Playwright's `json` reporter (`--reporter=list,html,json`), written to `apps/studio-web/playwright-results.json` by the
  step's `env` `PLAYWRIGHT_JSON_OUTPUT_FILE` (Playwright 1.65.1 reads `PLAYWRIGHT_<NAME>_OUTPUT_FILE`,
  `resolveOutputFile`), so the list output is unchanged.
- **C3.** A new step after the Playwright step, on both systems: "no test skipped but the registered ones" (Q3). A Node
  script in the step reads `playwright-results.json`, walks every spec's tests, and fails (exit 1) when the file has no
  test, or when any test has the status `skipped` (a `test.skip` and a test that did not run after a serial failure both
  have it) and its `<file>:<line>` is not in the step's `env` `STUDIO_ALLOWED_SKIPS` (space-separated; empty, as
  registered); it prints each such test and the counts. It runs after a passed Playwright step (a failed one has already
  failed the job).
- **C4.** The upload on failure also takes `playwright-results.json`.
- **C5.** `apps/studio-web/docker/linux-test/Dockerfile`: `xterm` added to the packages; a comment line says why.
- **C6.** `apps/studio-web/tests/e2e/commands.spec.ts`: line 113 as in H2; a line of the head comment names ADR 0804.
- **C7.** `apps/studio-web/tests/support/rcfd.ts`, `alive`: as in H3; Windows and macOS unchanged.
- **C8.** `apps/studio-web/tests/support/runner.ts`, `cleanUp`: as in H3.
- **C9.** `apps/studio-web/tests/e2e/commands.spec.mutation.patch`, the committed mutation (rule 16), below.

No other file. Windows stays as it is but for C2's reporter and C3 (Q2: nothing here shows that `windows-latest` needs the
switch; this machine has a GPU, so it cannot show it; the lead's reading of the first run decides it, as decided).

Rule 2 says existing tests are not edited and a corrected test keeps the old one as an ignored record. The brief allows a
registered change of a test whose defect the diagnosis shows, and forbids skips; an ignored record in a Playwright spec is
a skipped test, which C3 would fail. So the old line 113 is kept in this ADR (above, with its file and line) and in git
history, not as an ignored test. Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).

The image is built from the changed Dockerfile under this work's own tag `rcfd-studio-linux-test:ubuntu24.04-f823b`, so
the tag other work uses is not replaced.

## Set-up of the runs (fixed here)

ADR 0802's set-up, its scratch tools copied to this work's scratch folder: `run_job.py` runs the studio-web job of the
commit under test as written, step by step, as GitHub runs `shell: bash`, in a `git archive` copy; a `uses:` step is
replaced by a check of what it provides. **W**: Windows here, Git Bash, Node 22.19.0, Rust 1.99.0, `CARGO_TARGET_DIR` this
work's `target-win` under the scratch folder. **L**: the container, the image of C5 (tag above), started as ADR 0802's
(`--init --shm-size 2g --security-opt seccomp=unconfined`), the steps as the user `studio`, no `CARGO_TARGET_DIR` (the
checkout's `target` is the volume `rcfd-f823-target`), `CARGO_HOME` and the download caches on `rcfd-f823-cache`. Counts
from the list output's marks and Playwright's summary lines, which must agree (ADR 0802 amendment 1, item 2); retries are 0.

## Checks

| Id | Clause | Check | Limit | Source of the limit | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|---|
| L1 | builds the engine first so the service e2e tests and the F-824 check do not skip; the four pass on Linux | L: the Playwright step's counts | 880 passed, 0 failed, 0 skipped or not run; the four tests of the diagnosis and the 16 among the passed; the skipped list equal to the registered list (empty) | acceptance of F-823, last sentence; the brief, task 1 | as the limit | exact | code verification |
| L2 | the job runs | L: every step of the job as written, in order | each step passed, the new skip step included; the upload skipped by its `failure()` | the brief, task 1 | passes | exact | code verification |
| W1 | the same | W: as L1 | as L1 | the same | as the limit | exact | code verification |
| W2 | the same | W: as L2 | each step passed, the Linux steps skipped by their `if:`, the upload by `failure()` | the brief, task 1 | passes | exact | code verification |
| K1w | the skip step fails the job on an added skip (Q3) | negative control on a scratch copy: in `tests/unit/tokens.spec.ts` one test begins with `test.skip(true, 'ADR 0804 negative control')`; the Playwright step's Windows command and env limited to that file, then the skip step's script; the same on the unchanged copy | mutated: Playwright exit 0 and the skip step exit 1 naming that test; unchanged: both exit 0 | the brief, task 1 | as the limit | exact | code verification |
| K1l | the same | L: the same with the Linux command | as K1w | the brief, task 1 | as the limit | exact | code verification |
| V1 | the workflow is valid | `actionlint` 1.7.12 on `.github/workflows/ci.yml` (no shellcheck here); control: d5ec1145's file | exit 0 and no message; the control's messages stated | the brief, task 1 | exit 0 | exact | code verification |
| A1 | only the named files change | `git diff --stat d5ec1145..HEAD` and `git diff d5ec1145 -- .github/workflows/ci.yml` | only C1 to C9, this ADR and the proposal file; in ci.yml only the studio-web job | the brief, owned files | as the limit | exact | code verification |
| R7w | rule 7 | in the worktree's `apps/studio-web` on Windows, `CARGO_TARGET_DIR` = `target-win`: `npm run typecheck`, then `npm test` | exit 0; 880 passed, 0 failed, 0 skipped | rule 7; the brief, task 4 | passes | exact | code verification |
| R7l | rule 7 | in the container on a copy of the code commit: `npm run typecheck`, then `RCFD_TEST_ELECTRON_ARGS="<the D4 set>" xvfb-run --auto-servernum npm test` | as R7w | rule 7; the brief, task 4 | passes | exact | code verification |
| DS | rule 7 (docs) | `cargo test -p rcfd-cli --release --test docs_site` | passes | rule 7 | passes | exact | code verification |
| P1 | rules 7 and 9 | `python scripts/roadmap_proposals.py check docs/roadmap/proposals/0804-F-823.json`; `python scripts/process_report.py --check` | the proposal valid; the report clean but for a generated file that is only not current | rules 7 and 9 | passes | exact | code verification |

Each registered run is made once. A failed or flaky test is recorded with its output as a miss; nothing is run again to
pass. `cargo fmt`, `clippy` and `baseline_hashes` do not apply: no Rust file changes.

## Mutations (task 3, rule 16)

On scratch copies of the code commit, each once:

| Id | Mutation | Where, how | Caught when |
|---|---|---|---|
| MT | the terminal package removed | L, the image of C5: as root before the run, `dpkg --purge xterm`; then `tests/e2e/claude-code.spec.ts` with the Playwright step's Linux command | `claude-code.spec.ts:177` fails |
| MW | the WebGPU adapter removed | L: for G1 or G2, as root `dpkg --purge --force-depends mesa-vulkan-drivers`; for G3, the two SwiftShader switches left out of `RCFD_TEST_ELECTRON_ARGS`; then `tests/e2e/workspace-first-open.spec.ts` | `workspace-first-open.spec.ts:59` fails with "no WebGPU adapter" |
| MS | a skip added to a spec | the mutated half of K1w and K1l (not made again) | the skip step exits 1 naming the test |
| MC | committed (C9): the palette blurs the focus after it has gone back (in `CommandPalette.tsx`'s `onOpenChange`, `afterFocusReturns(() => (document.activeElement as HTMLElement | null)?.blur())`) | W, a scratch copy with the patch applied: `npx playwright test tests/e2e/commands.spec.ts --project=electron` after build and packaging | `commands.spec.ts:90` fails at the corrected line |

C9's first line: `# expect-fail: cd apps/studio-web && npm run build && npm run package:dir && npx playwright test tests/e2e/commands.spec.ts --project=electron`, the form of the patches already beside the e2e specs.

## Size

Size (agent): done; 0; 4; 0; 250000

(Heavy runs: L, W, R7l and R7w, whole suites; the controls, K1, the mutations and the link check are a spec or a file each.)

## Decision rule

**Done** for F-823 if every registered row holds on both platforms here and each of MT, MW, MS and MC is caught;
**partial** otherwise, with the miss recorded. The lead's reading of the first run on GitHub Actions after the push decides
the item.

## Not covered

The runners themselves: whether `ubuntu-24.04` has a terminal and Mesa's Vulkan drivers (C1 installs them when missing),
whether a software WebGPU adapter behaves there as in the container, `windows-latest` without a GPU (Q2), the job's time.
`docker/linux-test/run.mjs` (the local Docker runner of ADR 0801) keeps passing only `--ignore-gpu-blocklist`; it is not
among the owned files. The WebGPU drawing on Linux is a software device: it shows the code runs, not its speed on a GPU.

## Open questions for the maintainer

None at registration. Any question of scope the runs raise is listed in the addendum, not decided here.

## Amendment 1 (2026-11-26): the controls' results, and the WebGPU adapter is SwiftShader, not lavapipe

Written and committed after the controls D1 to D4 and before any code and any registered run. Nothing above is changed.
The controls were made once, in the container on a `git archive` copy of d5ec1145 with the overlay of the scratch folder
(`diag/`, `diag-out/`, the log `diag-out.log`):

| Id | Result | Hypothesis |
|---|---|---|
| D1 | before: `test -e /usr/bin/x-terminal-emulator` exit 1; `apt-get install -y --no-install-recommends xterm` exit 0; after: exit 0, `readlink -f` gives `/usr/bin/lxterm` (a program of the xterm package) | H1 holds |
| D2 | `before`, at once, after two frames and after 1 s: all `button:Open project folder…`; the test passed (2.9 s) | not reproduced; not contradicted (the stop condition, `body` after 1 s, did not occur) |
| D3 | at the look of line 216: `alive` true, state `D`, parent 1 (`docker-init`); 500 ms later and to 10 s: gone | H3b (a process still ending, reaped by init), not H3a (no zombie) |
| D4 | G1: adapter, vendor `google`, architecture `swiftshader`, `isFallbackAdapter` true, `webgl` and `webgpu` `enabled`, screenshot made (345 ms); G2: the same (198 ms); G3: the same adapter, `webgpu` `unavailable_software` (105 ms) | H4 holds for G1 |

1. **The switch set.** By the registered rule D4 selects **G1**: `--ignore-gpu-blocklist --enable-unsafe-webgpu`.
2. **The adapter is not the image's package.** Under G1 the adapter is SwiftShader (vendor `google`, architecture
   `swiftshader`, a fallback adapter), which Electron ships with itself; the registration expected G1 and G2 to use Mesa's
   lavapipe. So: C1 does **not** install `mesa-vulkan-drivers` (the adapter does not come from it); C1 installs `xterm`
   only, beside what it installed before. And the registered mutation MW for G1 (`dpkg --purge --force-depends
   mesa-vulkan-drivers`) removes a package the adapter does not use: it is made as registered and recorded, and a corrected
   mutation **MW2** is added: `--enable-unsafe-webgpu` left out of `RCFD_TEST_ELECTRON_ARGS` (the switch that gives the
   SwiftShader adapter; with only `--ignore-gpu-blocklist` ADR 0802's run L had no adapter), then
   `tests/e2e/workspace-first-open.spec.ts`; caught when `workspace-first-open.spec.ts:59` fails with "no WebGPU adapter".
   The decision rule's "each mutation is caught" names MW2 in place of MW; MW's result is recorded either way.
3. **C7 and C8 stay as registered** (C8 is the change D3 shows is needed; C7 is kept as registered, since a zombie has
   also ended). **C6 stays as registered** (D2 did not reproduce the race the log shows; the change removes it whether or
   not a given run meets it).
4. **Consequence for the whole Linux suite, stated before the runs:** with G1 every WebGPU user on Linux, three.js's
   `WebGPURenderer` among them, gets the SwiftShader adapter instead of the WebGL 2 fallback, so the field views and plots
   draw by WebGPU on a software device in L and R7l. Any failure that follows is recorded as a miss.

## Addendum 1: results (2026-11-26)

The code is 14442f02 (C1 to C9, amendment 1). Each run was made once on a `git archive` copy of 14442f02 in the scratch
folder (R7w in the worktree), by the set-up registered; the logs are in the scratch folder (`L/`, `W/`, `R7l/`, `R7w.log`,
`muts/`, `k1w/`, `MC/`). L ran alone; R7l overlapped the end of W, MC and R7w on the shared machine (a deviation, below).

### Checks

| Id | Limit | Measured | Met |
|---|---|---|---|
| L1 | 880 passed, 0 failed, 0 skipped or not run; the four and the 16 passed | 875 passed, 1 failed, 4 did not run (summary lines and marks agree); the four passed (`claude-code.spec.ts:177`, `commands.spec.ts:91`, the old line 90, `runner-lifetime.spec.ts:192`, `workspace-first-open.spec.ts:59`), and every test of their files (claude-code 4, commands 10, runner-lifetime 8); `capabilities.spec.ts:210` and the 10 of `service.spec.ts` passed. Failed: `error-walk.spec.ts:177` (`.mp-failed` not shown within its 60 s); the 4 after it in its serial file did not run | **missed** |
| L2 | each step passed, the skip step included; the upload skipped by `failure()` | steps 1 to 10 passed (engine 162 s); 11, Playwright, failed (exit 1, 1034 s); 12, the skip step, skipped after the failure; 13, the upload, ran (`failure()`); 14 and 15 skipped | **missed** |
| W1 | as L1 | 879 passed, 1 failed, 0 skipped; failed: `claude-code.spec.ts:193`, rcfd-mcp's `edit_case`: "replace ...from-claude.toml: failed after 12 attempts: Access is denied. (os error 5)", a file refused by the operating system on the shared machine (the brief: reported, not retried) | **missed** |
| W2 | each step passed, the Linux steps skipped by their `if:` | steps 1 to 8 and 10 passed (engine 338 s cold, `npm ci` 48 s); 9 skipped by its `if:`; 11 failed (exit 1, 833 s); 12 skipped after the failure; 13 ran | **missed** |
| K1w | mutated: Playwright exit 0, the skip step exit 1 naming the test; unchanged: both exit 0 | unchanged: 19 passed, exit 0; the skip step "19 tests in the report, 0 skipped and not in the registered list", exit 0. Mutated: 18 passed, 1 skipped, exit 0; the skip step exit 1, "skipped: unit/tokens.spec.ts:87 [unit] tokens.css is exactly the output of tokens.ts" | met |
| K1l | as K1w | the same numbers and messages in the container | met |
| V1 | exit 0, no message; the control stated | 14442f02's file: exit 0, no message (the call was made on the file as committed, before the commit); control, d5ec1145's file: exit 0, no message | met |
| A1 | only C1 to C9, the ADR and the proposal; in ci.yml only studio-web | `git diff --stat d5ec1145..14442f02`: the six files of C1 to C9 and this ADR; every top-level key and every job but studio-web equal | met |
| R7w | typecheck and `npm test` exit 0; 880 passed | typecheck exit 0; `npm test` exit 0, "880 passed (13.5m)" | met |
| R7l | as R7w | typecheck exit 0; `npm test` exit 1: 872 passed, 3 failed, 5 did not run. Failed, each a time limit: `responsiveness.spec.ts:260` (tasks over 100 ms or frames over 150 ms in every pass), `run-control.spec.ts:268` (`project.queue_case` answered in 37.8 ms, limit 20 ms), `workspace-first-open.spec.ts:59` (tasks over 100 ms with the GPU process held; the adapter was there). `error-walk.spec.ts:177` passed (47.3 s) | **missed** |
| DS | passes | run on this addendum before its commit; its output is in the report | see the report |
| P1 | proposal valid; the report clean but for a generated file | run after the commit; in the report | see the report |

### Controls and mutations

D1 to D4: amendment 1. Mutations, each once:

| Id | Seen | Caught |
|---|---|---|
| MT, `dpkg --purge xterm` | `claude-code.spec.ts:177` failed, 2 passed, 1 did not run, exit 1 | caught |
| MW2, `--enable-unsafe-webgpu` left out | `workspace-first-open.spec.ts:59` failed, "no WebGPU adapter: the GPU process cannot be held", exit 1 | caught |
| MW, `dpkg --purge --force-depends mesa-vulkan-drivers` (as registered) | the test passed (10.8 s): the adapter is SwiftShader, as amendment 1 said | not caught (expected) |
| MS, a skip added | the mutated half of K1w and K1l: the skip step exit 1 naming `unit/tokens.spec.ts:87`, Playwright itself exit 0 | caught |
| MC, committed (C9), Windows | `commands.spec.ts:91` failed, "Expected: button:Open project folder…, Received: body", 9 did not run, exit 1 | caught |

### Diagnosis of the new misses (from the logs and the code; nothing was run again)

1. `error-walk.spec.ts:177` (L only). It waits 60 s for the engine's refusal of a 1500 by 1500 preview. Its times: 28.7 s
   on Linux with WebGL 2 and 34.0 s on Windows (ADR 0802's runs), 47.3 s in R7l, over 60 s in L. With G1 the previews
   drawn before the refusal are drawn by WebGPU on SwiftShader; that drawing, near a bound the test sets itself, is the
   likely cause. Not shown: the copy's `test-results` went with the container.
2. The three of R7l are time limits of the responsiveness and run-control specs, on a software WebGPU device and on a
   machine that ran W, MC and R7w at the same time; in L, run alone, all three passed. Load is the likely cause; not shown.
3. `claude-code.spec.ts:193` (W only): a file the operating system refused (os error 5) while the engine replaced it; it
   passed in R7w and in ADR 0802's runs.

### Deviations

R7l overlapped W, MC and R7w (to stay within this work's turn limit); its time-limit failures are read with that. K1w's
first call stopped before running anything (a path form that Windows Python could not open) and was then made whole once.
A removal of scratch folders that did not yet exist was refused by a safety check and left out. Nothing was run again to
pass.

### Not run

The runners themselves (as registered). `cargo fmt`, `clippy` and `baseline_hashes` do not apply (no Rust file changed).

### Size

Size (agent, actual): partial; 1; 4; 1; 230000

### Decision

By the registered rule: **partial**. The four Linux failures of ADR 0802 and the 16 after them pass in L; every mutation
is caught but MW, which amendment 1 replaced by MW2 (caught); K1w, K1l, V1, A1 and R7w hold; L1, L2, W1, W2 and R7l
missed: one new Linux failure that follows the WebGPU switch, one file refused by Windows, three time limits under load.

## Open questions for the maintainer

- **Q1.** `error-walk.spec.ts:177` with WebGPU on SwiftShader. Options: (a) a follow-up under F-823 (rule 13) that
  measures the test's time under G1 and finds where it goes, the 60 s kept; (b) WebGPU only for the spec that needs it
  (`workspace-first-open.spec.ts`), the WebGL 2 fallback for the rest of the Linux suite; (c) a known miss of Linux.
  Recommendation: (a); (b) changes what Linux CI covers, a question of scope.

## For the lead: roadmap entries

> Status: partial.

> Note: ADR 0804, 2026-11-26, partial: the Linux studio-web job gets xterm and --enable-unsafe-webgpu (SwiftShader's adapter) and a step that fails on any skip; the four Linux failures of ADR 0802 and the 16 after them pass in the container, the skip step fails on an added skip on both systems, the mutations are seen (the package one replaced); but the Linux job failed error-walk.spec.ts:177 (over its 60 s), Windows met one file refused by the OS, and npm test in the container missed three time limits under load.

> Gap: In the Ubuntu 24.04 container the job's Playwright step failed error-walk.spec.ts:177 (the preview refusal not shown within 60 s, with WebGPU on SwiftShader; 28.7 s before the switch) and 4 after it did not run; npm test there missed three time limits under load; on Windows one test met a file refused by the OS. The first run on GitHub Actions is to be read.

> Follow-ups: none as new items; Q1 (a) is proposed as a follow-up registered under F-823 (rule 13).

The same entries are in `docs/roadmap/proposals/0804-F-823.json`.

## Answers of the maintainer (2026-11-26), written by the lead

1. Q1 (a): a follow-up under F-823 that measures error-walk.spec.ts:177 under WebGPU with the 60 s kept, carried out as ADR 0809; taken by the lead on the maintainer's standing instruction of 2026-11-26 (go with the recommended option always).
