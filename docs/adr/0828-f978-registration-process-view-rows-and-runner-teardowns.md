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
