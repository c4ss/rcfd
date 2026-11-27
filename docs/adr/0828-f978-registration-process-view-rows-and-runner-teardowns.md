# ADR 0828: F-978 registration: the process view's undecided rows, and the tests that leave a runner behind

Status: **REGISTERED 2026-11-27, written before any code of F-978 in this work and before any run of its checks.** Before
this text no test, support or library file was changed. What was done: reading (below), searches of the source tree
(S1), and, on the unchanged tree, a release build of `rcfd-cli`, `rcfd-mcp`, `rcfd-runner` and the `f447_bench` example
into the scratch target and `npm ci` in `apps/studio-web` (neither is a check). Results go in addenda at the end; nothing
above them is changed afterwards. Written by Claude Opus 5.5 (`claude-opus-5-5`), as a background agent of the lead.

It answers the brief `docs/adr/briefs/f978-process-view-rows.md` ([the file](briefs/f978-process-view-rows.md)) and
follows [docs/process/agent-rules.md](../process/agent-rules.md), version 5. Branch `f978-process-view-rows` from
2cbdfa5f, scratch `w49i-work/f978-process-view-rows-scratch`. It continues
[ADR 0826](0826-f978-registration-the-process-view-of-a-case.md) (partial: B1 to B3 not decided, M4 and M5 did not
build).

## What is asked

F-978's acceptance (read with a script from `docs/roadmap.json`), unchanged, as ADR 0826 quotes it and splits it into
a1 to a6. The brief adds: B1 to B3 run again as ADR 0826 registered them, once; M4 and M5 made again as edits that build;
and the tests that start a run end its runner (the maintainer's ask of 2026-11-27, in the brief).

## What was read

`AGENTS.md`, the rules (version 5) and the brief, whole; ADR 0826 whole; `crates/rcfd-mcp/tests/canvas_process_f978.rs`
and `apps/studio-web/tests/e2e/canvas-f979-a1.spec.ts` whole; the `afterAll` hooks of `canvas-f977`, `canvas-f978` and
`canvas-f979.spec.ts`; `tests/support/project.ts`, `runner.ts` and `global-teardown.ts` whole, `rcfd.ts` (`killPid`,
`alive`); `crates/rcfd-mcp/tests/common/mod.rs` (`binaries`, `Mcp`); `rcfd-runner` `client.rs` (`ensure_runner_pid`) and
the `shutdown` command of `main.rs`; the runner guards of the files of S1; of `view.rs` `opens` and the results step; of
`Canvas.tsx` `ProcessPanel`; of `scripts/push_main.py` how patches are found. Bytes read: about 130 000.

## Findings of the reading

1. **S1, the registered search.** Over `apps/studio-web/tests/e2e/*.spec.ts` and `crates/*/tests/**/*.rs`, a line not a
   comment that matches `"start_run"` or `'start_run'`, `.ensure()`, `ensure_runner`, `"ensure"`, `submit(` or
   `"serve", "--project"` starts a run or a runner; a line that matches `shutdown`, `endAll(`, `cleanUp(` ends one. 45
   files match the first. Each one's ends were read: 32 end the runner in a guard or hook that runs on failure too
   (`Drop`, `afterAll`). Of the 13 without an end, 8 start no runner: `project_list_f270.rs`, `tool_list_f275.rs`,
   `start_run_store_description_f862.rs` (tool lists), `project_w48f.rs` 158 and 257 (refused calls),
   `new_case_f271.rs` 194 to 217 (a runner path that does not exist), `run_tags_f274.rs` 51 (submit only queues),
   `stop_f834.rs` and `store_runs_f834.rs` (the guard of `f834/mod.rs` 262). The rest start a runner and do not shut it
   down: `crates/rcfd-mcp/tests/canvas_process_f978.rs` 84; `apps/studio-web/tests/e2e/canvas-f978.spec.ts` 169,
   `canvas-f979-a1.spec.ts` 245, `canvas-f977.spec.ts` 228 (it kills the pid of `.rcfd/runner.json`, no shutdown) and
   `canvas-f979.spec.ts` 247, which is skipped whole (`test.skip(true, ...)`, ADR 0807 amendment 1): its hooks do not run
   and it starts nothing, so this ignored record is left as it is.
2. `Runner.endAll` of `tests/support/runner.ts` (ADR 0790) already cancels a run not over, runs `rcfd-runner shutdown
   --project <folder>` and kills the runner if it is still alive; the four specs do not use it.
3. **M4 of ADR 0826** did not build because `panels` lost its type once `self.opens(panels)` was gone (E0283). Made
   again: `"opens": (self.opens(panels), Value::Null).1` in the results step of `view.rs`; `panels` keeps its type and
   the step's `opens` is null. **M5** did not typecheck (TS18047). Made again: `{opened && s.opens && s.i < 0 && (` in
   `ProcessPanel` of `Canvas.tsx`; a picked step draws no `cv-opened`.
4. The file `canvas_process_f978.mutation.patch` holds ADR 0826's M1; this work's committed mutation is M4, beside the
   unit test P1 that catches it, as `crates/rcfd-workspace/src/view.mutation.patch`.

## Design (tests only)

- **D1.** `tests/support/runner.ts` gains `endProjectRunner(project)`: `new Runner(findRunner(), project).endAll()`,
  never throwing (a failure is printed). Each of `canvas-f977`, `canvas-f978` and `canvas-f979-a1.spec.ts` calls it in
  its `afterAll`, in a `finally` after the client, the Studio and the MCP processes are ended and before `rm` of the
  folder; `canvas-f977`'s pid kill is replaced by it.
- **D2.** `canvas_process_f978.rs` gains a guard `EndRunner(dir)` made right after the project copy, whose `Drop` runs
  `rcfd_runner::client::Client::shutdown` of the project (10 s), as the guards of `feed_f273.rs` do; it is dropped after
  `drop(studio)`, and during an unwind otherwise. The checks of B1 to B3 are not changed.
- Nothing else: no engine, Studio or library file.

## Checks (rule 14)

Release profile, `CARGO_TARGET_DIR` and `CARGO_BUILD_JOBS` 8 in the scratch folder, `RCFD_BIN`, `RCFD_MCP_BIN`,
`RCFD_F447_BENCH` from that target. T1 to T3 count with `runners.py` of the scratch folder: the `rcfd-runner.exe`
processes whose executable is under the worktree or the scratch folder, alive at the count and started at or after the
run's start; other agents' runners are not counted. CPU load is recorded with each timed run.

| Id | Clause | Check | Limit | Source | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|---|
| B1 | a1 to a5 | `canvas_process_f978` once, as ADR 0826 registered it | every assertion of B1 | ADR 0826 B1 | holds | exact | code verification |
| B2 | a6 | same run | as ADR 0826 B2 | ADR 0826 B2 | holds | exact | code verification |
| B3 | a6 | same run | as ADR 0826 B3 | ADR 0826 B3 | holds | exact | unchanged default output |
| T1 | brief | runners alive before `npm test` | 0 | the brief (the maintainer, 2026-11-27) | 0 | exact | code verification |
| T2 | brief | runners started by `npm test` and alive after it | 0 | the brief | 0 | exact | code verification |
| T3 | brief | runners started by the B1 run and alive after it | 0 | the brief | 0 | exact | code verification |
| L2 | a4, a6 | `npm run typecheck`, then `npm test` once (branch engine, its `f447_bench`) | typecheck exit 0; every test passes but main's 6 registered skips (ADRs 0807, 0818) | rule 7; ADR 0826 L2 | pass | exact | unchanged default output |
| L5 | a6 | `rcfd-mcp --lib`; fmt and clippy of `rcfd-mcp` | pass, clean | rule 7 | pass | exact | unchanged default output |
| L4 | a6 | `docs_site`; `roadmap_proposals.py check`; `process_report.py --check` | pass (the last: only a generated file not current may fail) | rules 7, 9 | pass | exact | unchanged default output |

The other rows of ADR 0826 (V7, V8, P1, E1 to E3, L1, L3) held in its addendum 1 and their code is not changed here;
E1 to E3 and L2 are run again inside `npm test`. An OS refusal in B1 is recorded as not deciding, not run again.

### Negative control and mutations (rule 16)

Each on a scratch copy of the final commit (`git archive`; a real `node_modules` by `npm ci`), with its own target
folder, restored afterwards. Caught means the run fails at the assertion named, not elsewhere.

| Id | Mutation | Must fail |
|---|---|---|
| N1 | the `endProjectRunner` call removed from `canvas-f979-a1.spec.ts`; that spec alone | T: the count after it is 1 or more |
| N2 | the `EndRunner` guard removed from `canvas_process_f978.rs` | T: the count after it is 1 or more |
| M4 | finding 3, `view.rs` | P1 (`every opens is a valid view`) and B1 (`every step's view is shown`) |
| M5 | finding 3, `Canvas.tsx` | E2 of `canvas-f978.spec.ts` |

M4 is committed as `crates/rcfd-workspace/src/view.mutation.patch`, first line `# expect-fail: cargo test --release -p
rcfd-workspace --lib --target-dir target/mutation -- p1_f978`.

## Size

Size (agent): done; 0; 3; 1; 130000

(Expected done; no further registration; three heavy runs: `npm test`, the scratch Studio build with N1 and M5, the
scratch cargo build with N2 and M4; one question; about 130 000 bytes read.)

## Decision rule

**done** for F-978 if B1 to B3 hold, M4 and M5 are each caught, and every other row of ADR 0826 holds (by its addendum
1 and, for E1 to E3 and L2, in this `npm test`); **partial** otherwise, naming the row. T1 to T3, N1 and N2 are reported
beside it and do not change F-978's verdict. A missed limit is recorded with its number and held as a `*_known_miss`
record (rule 2).

## Not covered

- The engine side (a runner started without inherited handles; an exit when its folder is gone): F-997.
- Runners that a test's crash before its hooks leaves (a killed Playwright worker): the global teardown is not changed.
- Linux and macOS.

## Open questions for the maintainer

- **Q1. The skipped record `canvas-f979.spec.ts`** (scope): (a) left as it is, since it starts nothing; (b) given the
  same teardown, so that it is safe if it is ever unskipped. Recommendation: (a), since ignored records are not edited
  (rule 2); not taken here, since it is a question of scope.

## Addendum 1: results (2026-11-27)

Written after the runs; nothing above is changed. Release profile, `CARGO_TARGET_DIR` in the scratch folder,
`CARGO_BUILD_JOBS` 8. Code commits: 22d3dc9e (the teardowns, D1 and D2), 37d3b325 (the committed mutation M4). Each
registered run was made once.

### Checks

| Id | Limit | Measured | Met |
|---|---|---|---|
| B1 | every assertion of B1 | **failed before any step was read**: the first `edit_case` after `new_case` got -32000 "replace ...process-cavity.toml: failed after 12 attempts: Access is denied. (os error 5)", CPU 99 % at the start; not run again | **no (not decided)** |
| B2, B3 | as ADR 0826 | not reached (the same test function as B1) | **no (not run)** |
| T1 | 0 before `npm test` | 0 (13 runners of other agents on the machine, not counted) | yes |
| T2 | 0 after `npm test` | 0 started at or after 16:58:52 UTC and alive at 17:13:43 | yes |
| T3 | 0 after the B1 run | 0; but the run ended before `start_run`, so no runner was started and T3 does not decide | not decided |
| L2 | typecheck 0; all pass but the 6 skips | typecheck exit 0; 903 passed, 6 skipped, 0 failed, 14.6 min; CPU 100 % at the start, 70 % at the end. In it, F-978's E1 692 ms (limit 1600), 10 steps, seq 1 to 5 against 1 to 5, 4 of 4 residuals equal; E2 10 of 10; E3 162 ms; `canvas-f977` E0 to E7 and `canvas-f979-a1` E0 to E3 passed with their new teardowns | yes |
| L5 | pass, clean | `rcfd-mcp --lib` 18 passed, 1 ignored (existing); `cargo fmt -p rcfd-mcp --check` clean; `cargo clippy --release -p rcfd-mcp --all-targets -- -D warnings` clean | yes |
| L4 | pass | in the last run, reported with the commit | see the report |

### Negative controls and mutations

| Id | Caught by |
|---|---|
| N1 (teardown call removed from `canvas-f979-a1`) | the count: 1 runner of the scratch build alive after the spec (pid 51620); the spec itself passed (4 of 4) |
| N2 (the `EndRunner` guard removed) | the count: 1 runner of the mutated build alive after the test (pid 53516); in that run B1 to B3 passed |
| M4 (committed, `crates/rcfd-workspace/src/view.mutation.patch`) | P1 ("every opens is a valid view") and B1 (line 252, "every step's view is shown": 9 of 10, step 8 `results` opens nothing) |
| M5 | E2 of `canvas-f978.spec.ts`: 0 of 10 steps opened their view |

Both M4 and M5 built (the Studio of M5 passed `npm run build`, which typechecks). The two runners that N1 and N2 left
were this work's and were ended by pid after their counts. The count script first failed in N2 on a 7-digit fraction of a
second (Python's `fromisoformat`) and was corrected before N2's first count. In T1 to T3 no runner matched the path
filter, so the time was never parsed there.

The mutated runs of B1 (M4 at CPU 55 %, N2 at CPU 69 %) made the build with no OS refusal, and in N2 every row of B1, B2
and B3 held. Those are not the registered run. The refusal came in both registered runs (ADR 0826 at CPU 82 %, here at
99 %) and in none of the four other runs (all at 70 % or less). Not diagnosed: a cause to look at is a reader of the new
case file (the `--canvas` server reads the project when the feed moves) holding it while `edit_case` replaces it.

Deviations: none in the limits. `canvas-f979.spec.ts` left as it is (finding 1, Q1). Not run: B2 and B3 (not reached).

Actual size: partial; 0 further registrations; 3 heavy runs (`npm test` 14.6 min, the scratch Studio build with N1 and
M5, the scratch cargo build with M4 and N2); 1 question; about 140 000 bytes read.

Decision by the registered rule: **partial**, because B1 to B3 did not decide (an OS refusal in their one run). M4 and
M5 are caught, and the other rows of ADR 0826 hold. The teardown rows held beside it (T1, T2, N1, N2); T3 did not decide.

### For the lead: roadmap entries

> Status: partial.

> Note: ADR 0828, 2026-11-27, partial: M4 and M5 built and were caught (P1 and B1; E2 0 of 10), and the canvas tests now end the runner their start_run started (0 runners left after npm test, 903 passed; 1 left with a teardown removed); the one registered run of B1 failed again at its first edit_case on Windows' Access is denied (CPU 99 %), while the same build passed in both of this work's mutated runs.

> Gap: Every row of ADR 0826 holds but B1 to B3. Missing: a passing registered run of canvas_process_f978. In both registered runs (CPU 82 % and 99 %) the first edit_case after new_case was refused by Windows (Access is denied, os error 5, replacing the new case file); in all four mutated runs (CPU 70 % or less) the same build passed. A diagnosis of the refusal, registered under F-978 (rule 13), and then one run of B1 to B3.

> Follow-ups: one under F-978 (rule 13), no new item: "F-978: the refused first edit_case of B1", the diagnosis of the os error 5 (who holds the new case file when edit_case replaces it), then B1 to B3 run once; its limits are B1 to B3 of ADR 0826.

The same entries are `docs/roadmap/proposals/0828-F-978.json`.

## Answers of the maintainer (2026-11-27), written by the lead

1. `canvas-f979.spec.ts`, a skipped record that starts nothing, keeps its teardown as it is (option a); taken by the lead on the maintainer's standing instruction of 2026-11-26 (go with the recommended option always).
2. The diagnosis of the refused `edit_case`, then one run of B1 to B3, is a follow-up registration under F-978 (ADR 0830); taken by the lead on the maintainer's standing instruction of 2026-11-26 (go with the recommended option always).

Merged at 34017c43 and pushed at 30d79bdb.
