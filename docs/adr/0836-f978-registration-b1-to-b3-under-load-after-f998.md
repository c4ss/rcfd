# ADR 0836: F-978 registration: B1 to B3 under arm H's load on main after F-998, F-978's closing run

Status: **REGISTERED 2026-11-27, written before any code of F-978 in this work and before any run of its checks.** Before
this text nothing was changed, built or run but reading, searches, `git apply --check` of the three patches below (each
applies to 476de79b) and the arithmetic below. Results go in addenda at the end; nothing above them is changed
afterwards. Written by Claude Opus 5.5 (`claude-opus-5-5`), as a background agent of the lead.

It answers the brief `docs/adr/briefs/f978-process-view-after-f998.md` ([the file](briefs/f978-process-view-after-f998.md))
and follows [docs/process/agent-rules.md](../process/agent-rules.md), version 5. Branch `f978-process-view-after-f998`
from 476de79b (equal to origin/main), scratch `w49i-work/f978-process-view-after-f998-scratch`. It is registered under
F-978 (rule 13) and closes the line of [ADR 0826](0826-f978-registration-the-process-view-of-a-case.md),
[ADR 0828](0828-f978-registration-process-view-rows-and-runner-teardowns.md),
[ADR 0830](0830-f978-registration-the-refused-edit-case.md) and
[ADR 0832](0832-f978-registration-the-hold-of-a-refused-file.md), after F-998's remedy
([ADR 0835](0835-f998-registration-the-plain-write-for-new-case-files-only.md): new case files by a plain write on
Windows, 0 of 8900 `edit_case` refused).

## What was read

`AGENTS.md`, the rules (version 5) and the brief, whole; ADRs 0826, 0828 and 0835 whole; of ADRs 0830 and 0832 the load
L, the B1 rows and their results; `crates/rcfd-mcp/tests/canvas_process_f978.rs` whole; of `replace_hold_f978.rs` the
spinning load (lines 84 to 97) and S1; of `common/mod.rs` `scratch` and `binaries`; the three committed patches below;
F-978's acceptance (by a script). Bytes read: about 115 000.

Arithmetic beforehand: F-998 measured 0 refusals in 8900 `edit_case` under H's load; the 95 % upper rate is 3/8900 =
3.4 × 10^-4 per edit. This work makes 12 `edit_case` of B1 (3 per run, 4 runs with L1), so P(any refusal) ≤ 1 - (1 -
3.4 × 10^-4)^12 = 0.4 %.

## The acceptance, clause by clause

F-978's acceptance, read with a script, is ADR 0826's, split there into a1 to a6. Each clause is met in this work if
the assertions of B1 to B3 named for it hold in every registered run (R1, R2, R3, L1):

| Clause | Assertion of `canvas_process_f978.rs` |
|---|---|
| a1 ordered path from creation through mesh, set-up, each run (states, residuals, monitors) to results | first step `created`; steps `mesh`, `setup`, `run`, `state`, `residuals`, `results`; times never decrease; the after-run set-up last |
| a2 read from the journal, the run records and their events | journal steps' `seq` equal the concerned lines'; state steps equal `events.jsonl`'s; residuals' `last` equal `read_residuals`' last points |
| a3 each step opens the view of its state | every step's `opens` shown by `show_view` (`opened` equal to the step count) |
| a4 a test builds a case through the MCP tools from new to a finished run | the build of B1 through an `rcfd-mcp` client, the run's last state step its terminal state |
| a5 every journaled step in order, none missing against the journal | `got == want` (the journal filter of D2 (a) to (c), written in the test) |
| a6 by addition, the kind named, defaults unchanged | B2 (export refused at `panels[0].kind`, nothing written) and B3 (description names `process {case}`; a plain view has no `process`) |

## Design (one test added, no library change)

`canvas_process_f978.rs` gains, by addition only, one ignored test `b1_b2_b3_under_the_load_of_arm_h_adr0836`: it
starts **16** threads spinning in the test process (the loop of `replace_hold_f978.rs` 86 to 95, ADR 0832's arm H, with
the count fixed at 16, this machine's logical processors), waits 500 ms, calls the existing test function
`b1_b2_b3_the_process_of_a_case_built_from_new_to_a_finished_run` unchanged (inside `catch_unwind`), stops and joins the
threads, and passes the panic on. B1's runner is ended by its existing `EndRunner` guard (ADR 0828 D2). It is ignored so
that the file's default run is unchanged.

## Checks (rule 14)

Release profile; `CARGO_TARGET_DIR` under the scratch folder spelled with backslashes; `CARGO_BUILD_JOBS` 8. The CPU
(`LoadPercentage`, the average of the processors) is recorded before and after each run of R1 to R3 and L1. Each run is a
separate `cargo test` process, made once.

| Id | Clause | Check | Limit | Source | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|---|
| R1, R2, R3 | a1 to a6 | the ignored test above, run 3 times | in each run, every assertion of B1, B2 and B3 holds, and the test prints 16 spinning threads | ADR 0826 B1 to B3; the brief (arm H, at least 3 runs, each must hold) | holds (P(refusal) ≤ 0.4 %) | exact | code verification |
| L1 | a1 to a6 | `canvas_process_f978` default run (B1 to B3 without the added load), once | pass | ADR 0826 B1 to B3; rule 7 | pass | exact | code verification |
| T3 | | `rcfd-runner` processes of this work's target alive after each of R1 to R3 and L1 | 0 | ADR 0828 T3 | 0 | exact | code verification |
| M1 | a5 | committed `crates/rcfd-mcp/tests/canvas_process_f978.mutation.patch` (ADR 0826 M1) with its expect-fail command | exits non-zero, failing at "B1: the journal's steps, in order, none missing" | ADR 0826 M1; rule 16 | caught | exact | code verification |
| M2 | a3 | committed `crates/rcfd-workspace/src/view.mutation.patch` (ADR 0828 M4) with its expect-fail command | exits non-zero, failing at "P1: every opens is a valid view" | ADR 0828 M4; rule 16 | caught | exact | code verification |
| M3 | | committed `crates/rcfd-mcp/tests/replace_hold_f978.mutation.patch` (ADR 0833 M1, beside F-978's test file of ADR 0832) with its expect-fail command | exits non-zero, failing at S1 ("HO's project ... under") | ADR 0833 M1; rule 16 | caught | exact | code verification |
| L2 | a6 | `rcfd-mcp --lib` | pass | rule 7 | pass | exact | unchanged default output |
| L3 | a6 | `cargo fmt -p rcfd-mcp --check`; `cargo clippy --release -p rcfd-mcp --all-targets -- -D warnings` | clean | rule 7 | clean | exact | unchanged default output |
| L4 | | `docs_site`; `roadmap_proposals.py check` of `docs/roadmap/proposals/0836-F-978.json`; `process_report.py --check` | pass (the last: only a generated file not current may fail) | rules 7, 9 | pass | exact | unchanged default output |

The mutations (rule 16) run on a `git archive` copy of the final commit in the scratch folder, one patch at a time,
restored between; their expect-fail commands as the patches' first lines give them (`--target-dir target/mutation`,
inside the copy), after `cargo build --release -p rcfd-cli -p rcfd-runner` into that folder, which `common::binaries`
needs. Caught means the command fails at the assertion named. Negative control: L1 and R1 to R3 on the unmutated code.
No new mutation is made: the code is unchanged. An OS refusal in a run is recorded as it comes and not run again.

Order: R1, R2, R3, L1 (with T3 after each), M1 to M3, L2, L3, L4. No shared solver file and no Studio file is touched,
so neither `baseline_hashes` nor `npm test` is run.

## Size

Size (agent): done; 0; 1; 0; 115000

(Expected done; no further registration; one heavy run, the mutation copy's build; no question; about 115 000 bytes read.)

## Decision rule

**done** for F-978 if every registered run of B1 to B3 holds (R1, R2, R3 and L1), the acceptance's clauses a1 to a6 are
met (the table above) and each committed mutation M1 to M3 fails as expected; **partial** otherwise, naming the run or
mutation. T3 and L2 to L4 are reported beside it. A missed limit is recorded with its number and held as a
`*_known_miss` record (rule 2).

## Not covered

- E1 to E3 (the Studio e2e of ADR 0826) are not run again: no Studio file changes; they held in ADR 0828's `npm test`.
  ADR 0826's M5 is a scratch Studio mutation, not committed, and is not run again.
- A load other than the 16 threads in the test process; other agents' load is recorded as CPU only.
- Linux and macOS.

## Open questions for the maintainer

None at registration.

## Addendum 1: results (2026-11-27)

Written after the runs; nothing above is changed. Release profile, `CARGO_TARGET_DIR` under the scratch folder spelled
with backslashes, `CARGO_BUILD_JOBS` 8. Code commit: d4456811 (the ignored test, 32 lines added, nothing else). Each
registered run was made once, in the registered order. Scratch: `runs.sh`, `runs.log`, `run-<R1,R2,R3,L1>.log`,
`mut.sh`, `mut.log`. Another agent's work kept the machine at 99 % to 100 % CPU through every run.

### Checks

| Id | Limit | Measured | Met |
|---|---|---|---|
| R1 | every assertion of B1 to B3; 16 spinning threads | 16 threads; run finished; 10 steps (created, mesh, setup, state, run, state, state, residuals, results, setup); journal seq 1 to 5 against 1 to 5; states 3 against 3, terminal; residuals 4 of 4 equal; results; ordered; after-run set-up last; views shown 10 of 10; B2 refused 1003 at `panels[0].kind`, nothing written; B3 held; 96.7 s; CPU 100 % at start and end | yes |
| R2 | as R1 | the same values in every assertion; 39.7 s; CPU 100 % and 100 % | yes |
| R3 | as R1 | the same values in every assertion; 31.9 s; CPU 100 % and 100 % | yes |
| L1 | pass | B1 to B3 passed with the same values (1 passed, 1 ignored: the test of this work); 7.3 s; CPU 99 % and 100 % | yes |
| T3 | 0 after each | 0, 0, 0, 0; no process of this work's targets alive at the end | yes |
| M1 | fails at "B1: the journal's steps, in order, none missing" | exit 101 at line 238 with that message (journal seq 1, 4 against 1 to 5); CPU 82 % | **caught** |
| M2 | fails at "P1: every opens is a valid view" | exit 101 at `view.rs` 3623 with that message; CPU 100 % | **caught** |
| M3 | fails at S1 ("HO's project ... under") | exit 101 at `replace_hold_f978.rs` 335, "S1: HO's project ...\Temp\rcfd-mcp-w48f-f998-s1-o-40784 under ...\Temp\"; CPU 100 % | **caught** |
| L2 | pass | `rcfd-mcp --lib` 18 passed, 1 ignored (existing) | yes |
| L3 | clean | `cargo fmt -p rcfd-mcp --check` exit 0; clippy `--release -p rcfd-mcp --all-targets -- -D warnings` exit 0 | yes |
| L4 | pass | in addendum 2 | see there |

No `edit_case` was refused in the 12 of this work's runs, nor in the 3 of M1's run.

### The acceptance, clause by clause

a1 to a6 are met: every assertion the table of the clauses names held in R1, R2, R3 and L1.

Deviations: none. Not run: nothing registered was left out. Actual size: done; 0 further registrations; 1 heavy run
(the mutation copy's build); 0 questions; about 115 000 bytes read.

Decision by the registered rule: **done**. B1 to B3 held in every registered run (R1, R2, R3 under the 16 threads, and
L1), the clauses a1 to a6 are met, and each committed mutation M1 to M3 failed as expected.

## Open questions for the maintainer

None.

### For the lead: roadmap entries

> Status: done.

> Note: ADR 0836, 2026-11-27, done: on main after F-998, B1 to B3 held in 3 of 3 registered runs under arm H's 16 spinning threads and in the file's default run (CPU 99 % to 100 %, 10 of 10 steps, journal seq 1 to 5 against 1 to 5, every step's view shown, 0 of 12 edit_case refused), and the three committed mutations beside F-978's tests were each caught.

The same entries are `docs/roadmap/proposals/0836-F-978.json`.
