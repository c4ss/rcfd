# ADR 0773: F-876 registration: the replace of run.json refused with access denied in adopt_f288

Status: **REGISTERED 2026-11-22, written before any code of F-876 and before any run of its checks.** Before this text
no code of this work was written, nothing was built and no test was run. What was done: reading (below), the arithmetic
below, and read-only queries of the machine's state (the scanner's status and the indexer's service, both below).
Results go in addenda at the end; nothing above them is changed after a run.

It answers the brief `docs/adr/briefs/f876-adopt-f288-access-denied.md` and follows `docs/process/agent-rules.md`,
version 5. Branch `f876-adopt-f288-access-denied` from main d6e44b27, scratch
`E:/projects/cfd/w49i-work/f876-adopt-f288-access-denied-scratch`. Parent records, none of them edited:
[ADR 0201](0201-f288-registration-adopt-matching-runs-after-an-mcp-server-restart.md) (the test A-N and its decoys),
[ADR 0122](0122-wave47a-project-folder-library.md) (the atomic replace) and
[ADR 0758](0758-f855-registration-a-stall-of-the-waiter-or-of-the-holder-separated-from-a-fault-of-the-edit-lock.md)
(how a fault of the edit lock was separated from load on this machine, the model of the loads and series here).

Size (agent): done; 0; 10; 1; 200000

## What is asked

The acceptance of F-876 (`docs/roadmap.json`, read by a script), in clauses: **a1** every limit not stated there is
registered here before the run; **a2** for `adopt_f288::a_n_a_run_that_differs_in_one_item_is_not_adopted`, which
failed once with `Retry { op: replace, attempts: 12, PermissionDenied }` on the run.json of a run, the cause is shown by
a registered control (another handle of the test or of the server on the file, a scanner of the machine, or the load
beside it); **a3** with a registered number of repeated runs with and without a registered load; **a4** the code or the
test is then changed under this registration so that the registered repeats pass; **a5** a mutation shows the test
still catches an adopted run that differs; **a6** the test is not ignored, skipped or retried to make it pass, and the
number of attempts is not raised without the cause. The brief adds the decision rule: done if the cause is shown and the
repeats pass after the change; partial if the failure cannot be reproduced in the registered repeats, recorded with the
numbers.

## What was read

`AGENTS.md`; `docs/process/agent-rules.md` (version 5, whole); the brief; `crates/rcfd-mcp/tests/adopt_f288.rs` and
`crates/rcfd-mcp/tests/common/mod.rs` (the helpers `scratch` and `binaries`); `crates/rcfd-project/src/atomic.rs`
(whole); ADR 0758 (the parts on candidates, loads, series, checks, mutations and the decision rule); the acceptance and
note of F-876 in `docs/roadmap.json`; the ledger line of the left-out run of d08569e7 (`docs/process/ledger.tsv` line 45)
and its log `E:/projects/cfd/w49i-work/left-out-d08569e7.log` (the failure, lines 1666 to 1686);
`crates/rcfd-runner/src/store.rs` (`write_run`), `crates/rcfd-runner/src/client.rs` (`submit_with`, `solver_version`,
`ensure_runner_pid`), `crates/rcfd-runner/src/sys.rs` (every open with a share mode), `crates/rcfd-workspace/src/runs.rs`
(`RunService::new`, `submit_with`), `crates/rcfd-workspace/src/design.rs` (`RunKey::matches`); the implementation of
`std::fs::rename` on Windows in the toolchain's own library source (rustc 1.99.0,
`library/std/src/sys/fs/windows.rs`, function `rename`); a search of `crates/` for `share_mode`.

## What the record says (before any run)

* The panic is at `adopt_f288.rs:508:100`, the `atomic_write` by which the test changes `solver_version` in the run.json
  of decoy (h) right after `submit_with` made it. That is the only replace of a run.json in the test, and no code of the
  product replaces a run.json: the store creates it once (`write_json_new`, a temporary file hard-linked to the name and
  then removed, inside `runs/.tmp-<id>/`, which is then renamed to `runs/<id>/`). The project was in the default
  temporary folder on C: (`C:\Users\...\AppData\Local\Temp\rcfd-mcp-w48f-f288-an-55588`). The error is `Os { code: 5 }`.
* `DEFAULT_RETRY` is 12 attempts with sleeps of 1, 2, 4, 8, 16, 32, 64, 100, 100, 100 and 100 ms between them: 527 ms in
  all. The failure therefore means that every one of 12 renames over at least 0.527 s was refused.
* `std::fs::rename` of this toolchain calls `MoveFileExW` with `MOVEFILE_REPLACE_EXISTING`; when that fails with access
  denied it opens the source with `DELETE` access and renames it by `SetFileInformationByHandle` with
  `FILE_RENAME_FLAG_REPLACE_IF_EXISTS | FILE_RENAME_FLAG_POSIX_SEMANTICS`; when that also fails it returns the first
  error (code 5). So code 5 after the fallback is what a handle on the target or on the source that the second step
  cannot get past gives; the control PC below measures which handles do that on this machine, rather than taking it
  from any description of the system calls.
* At the moment of (h) no runner of the A-N project runs (the test starts one later by `ensure_runner`), and the
  in-process server and the test's own code close every handle of run.json they open (`fs::read`, `write_new`). The
  only processes the test starts before (h) are `rcfd --version` (one per `submit_with`), which open no run file. The
  other test of the binary (A-R) runs at the same time on another project folder with its own runners. No code in
  `crates/` opens a run file with a share mode of its own (the share modes in `rcfd-runner/src/sys.rs` are of the
  runner's lock and the solver's liveness file; the one in `atomic.rs` is a test hook).
* The machine, read now and not changed: the scanner of the operating system reports real-time and on-access
  protection on; its exclusions cannot be read without administrator rights (so whether the temporary folder is
  excluded is not known); the indexing service runs. Sixteen logical processors.

## Candidate causes, and the control of each

* **H1, a handle of the test or of a process of the test** (the in-process server, a runner, `rcfd --version`) on
  run.json or on the replace's temporary file. By the reading above none holds one at (h); the control shows it by
  naming the holder at each refused attempt.
* **H2, a handle of another process of the machine**: the scanner (acts on a file when it is written and closed), the
  indexer, or anything else. Control: the same naming; in addition the age control (a file replaced 3 s or more after
  it was made, against one replaced at once), since a scanner or an indexer acts on new files.
* **H3, the load beside it**: load holds no handle, so it is not a cause alone; it can lengthen the hold of H1 or H2
  beyond 0.527 s. Control: the same series with and without the registered loads, the longest refusal compared.

## Design (fixed now)

### The probe (scratch only, not committed)

A small program in the scratch folder (`probe/`, a crate with its own `[workspace]`, path dependencies on the worktree's
`rcfd-project`, `rcfd-runner`, `rcfd-workspace` and `rcfd-mcp`, no other crate; built with the scratch target folder). It
repeats the test's sequence at decoy (h) and replaces run.json as `atomic_write` does, but watches every attempt:

1. A project made as A-N makes it (`Project::create`, `cases/cavity.toml` the test's `cavity_case(32)` text,
   `.rcfd/runner-config.json` with `max_runs` 2), in `std::env::temp_dir()` as the test does. A `RunService` with the
   `rcfd-runner` of the scratch build (its `rcfd` beside it). No runner is ever started by the probe; at the end it
   records whether `.rcfd/runner.lock` exists.
2. One iteration: `submit_with` of the run of decoy (h) (case `cases/cavity.toml`, `set = ["physics.nu=0.015"]`, no
   launch settings, the full tags of A-N); `atomic::read_json` of its run.json, `solver_version` set to
   `rcfd 0.0.0-decoy`, `serde_json::to_string_pretty`; a temporary file `.run.json.<pid>.<n>.tmp` beside it made as
   `write_temp` makes it (`create_new`, `write_all`, `sync_all`, closed); then `std::fs::rename` attempts with the
   backoff of `DEFAULT_RETRY` (1 ms doubling to 100 ms) not stopped at 12 but at 10 s of sleeping (an observation
   window, not a limit). After every refused attempt, before the sleep, the probe asks the operating system for the
   process ids that hold the target and the source open (`NtQueryInformationFile` with the class
   `FileProcessIdsUsingFileInformation`, number 47, on a handle opened with only `FILE_READ_ATTRIBUTES` and every
   sharing mode, closed at once; `unsafe` code in the scratch program only). It records per iteration: the run id,
   attempts, the time to the replace, the error codes, the pids on the target and on the source at each refused
   attempt. An iteration **would have failed the test** when its rename needed more than 12 attempts.
3. Pids are named by `tasklist /FO CSV /NH` run at the start and at the end of each series, and by the probe's own pid.
4. Mode `fresh`: the replace right after the submit (as the test). Mode `aged`: all N runs are submitted first, then the
   probe waits until the newest run.json is 3 s old and replaces each (3 s is more than five times the 0.527 s that the
   failure's refusal lasted at least).
5. Mode `pc`, the positive controls of the probe and of H1: **PC1**: a thread of the probe opens run.json for reading
   with sharing read and write and not delete and holds it 1.0 s; meanwhile (a) the watched rename and (b)
   `rcfd_project::atomic::atomic_write` itself are tried. **PC2**: the same hold with every sharing mode (as `fs::read`
   holds a file). **PC3**: the hold of PC1 on the temporary source file instead of the target. Each 10 times.

The probe is verified by PC: it must name its own pid as the holder in PC1 and PC3.

### Loads (`loads.py` in the scratch folder, Python standard library)

* **L0**: no load started by this work (other agents share the machine; that is not controlled).
* **LC, three solves of CPU**: 12 processes of endless arithmetic (three solves of four threads, the `cpu_threads` of the
  test's runner configuration).
* **LF, files**: 4 processes that each, in a loop, create a new file in their own folder under the temporary folder,
  write 256 KiB, close it and remove the previous one (new closed files, which a scanner and an indexer act on; a
  solve's field store writes in this way).
* **L1** = LC and LF together (the model of the three heavy solves beside the failure).

### Series, each made once, in this order

* Probe: **PCs** (mode pc, L0); **P0** (fresh, L0, N = 300); **P0a** (aged, L0, N = 100); **PLC** (fresh, LC, N = 300);
  **PLF** (fresh, LF, N = 300); **PLFa** (aged, LF, N = 100).
* The test before the change: **R0**, 20 runs of `cargo test -p rcfd-mcp --release --test adopt_f288` (the whole binary,
  both tests at once, as the left-out run ran it) under L0; **R1**, 20 runs under L1.
* The test after the change: **A0**, 20 runs under L0; **A1**, 20 runs under L1.

The counts: with N = 300 a refusal of any length that happens in 1 % of fresh replaces is seen at least once with
probability 1 - 0.99^300 = 0.95; the aged series of 100 is to compare a rate, not to find a rare event. 20 runs of the
test (as ADR 0758) see a failure of 1 run in 10 with probability 0.88 and of 1 in 100 with 0.18: a reproduction of the
whole failure in R0 or R1 is not expected, and the probe series are the measure that can see the refusals below the
527 ms that the test needs to fail.

### Change T (the test), made after R0 and R1

Decoy (h) of A-N is made so that it differs only in `solver_version` without any replace of a file: it is submitted by
a second `RunService` of the same project whose runner path names a file with no `rcfd` beside it, so that
`Client::solver_version` gives `None` and the store writes `solver_version: "unknown"` (`UNKNOWN_SOLVER_VERSION`) in
run.json once, while the key of the in-process server has the version of the built `rcfd`. Everything else of A-N
(the other decoys, the pauses, the cancel, the failed run, the positive control (k), every assertion) is unchanged.
The body is shared: a parameter says how (h) is made. The test keeps its name and runs in the gate; the old making of
(h) (a replace of run.json) stays as an ignored record `a_n_record_before_adr0773` (rule 2: a defective set-up gets a
corrected test, the old one stays as an ignored record). The header of the file names ADR 0773 and F-876. Nothing of
the product and nothing of `atomic.rs` is changed (`DEFAULT_RETRY` stays 12 attempts).

Why the test and not the code: run.json is immutable in the product (made once, never replaced), so the replace that
failed is a step of the test's set-up, not a behaviour the test checks; the decoy needs a record that differs in one
item, not a replace. If the controls name a holder of the test's own processes (H1), the code that holds it is changed
instead, unless it is `crates/rcfd-mcp/src/project.rs` (then this work stops and reports).

## Table of checks

Release builds, `CARGO_TARGET_DIR=<scratch>/target`. "The acceptance" is that of F-876; "the brief" is
`docs/adr/briefs/f876-adopt-f288-access-denied.md`.

| Id | Check | Limit | Source of the limit | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|
| a1 | every limit of this table registered before its run | holds | the acceptance, clause 1 | holds | n/a | code verification |
| PC1 | a handle without delete sharing held 1.0 s on run.json | (a) refused for the hold, the probe names its own pid; (b) `atomic_write` returns `Retry` with 12 attempts and code 5 | the acceptance ("another handle of the test ... on the file") | as limited | n/a | code verification (positive control) |
| PC2 | a handle with every sharing mode held 1.0 s on run.json | replaced, no `Retry` | the same | replaced at attempt 1 | n/a | code verification (negative control) |
| PC3 | a handle without delete sharing held 1.0 s on the source | refused for the hold, own pid named; the error codes reported | the same | refused, code 32 or 5 | n/a | code verification (positive control) |
| a2 | P0, P0a, PLC, PLF, PLFa: refused iterations, would-fail iterations, longest refusal, the holders named at refused attempts | reported; the cause is named as the class of the holders named (H1 own or started process, H2 a named other process) when at least one refused attempt is seen and every refused attempt with a named holder names one class | the acceptance, clause 2; the brief ("name the cause only as far as a control shows it") | refusals in P0 and more in PLF, holder the scanner, fewer in the aged series | n/a | code verification (a diagnosis without a limit) |
| H3 | the longest refusal under LC and LF against L0 | the load is named as lengthening the hold when a would-fail iteration is seen only under a load, or the longest refusal under a load is at least twice that under L0 | the acceptance ("or the load beside it") | LF lengthens, LC less | n/a | code verification |
| a3-R | R0 and R1: failed runs of the 20 each, the failure printed | reported | the acceptance, clause 3; the brief (partial if not reproduced, with the numbers) | 0 of 20 and 0 or 1 of 20 | n/a | code verification |
| a4 | A0 and A1 after change T | 20 of 20 and 20 of 20 pass | the acceptance, clause 4 | as limited | n/a | code verification |
| a5 | M1, M2 below | each fails A-N | the acceptance, clause 5; rule 16 | caught | n/a | code verification |
| a6 | A-N not ignored or skipped (it runs in the default build); no retry of a test; `atomic.rs` unchanged | holds; `git diff main -- crates/rcfd-project/src/atomic.rs` empty | the acceptance, clause 6 | holds | n/a | code verification |
| E | `cargo test -p rcfd-mcp --release --lib` | every test passes | rule 7 | passes | n/a | unchanged default output |
| P | `cargo fmt -p rcfd-mcp --check`; `cargo clippy -p rcfd-mcp --all-targets -- -D warnings`; `cargo test -p rcfd-cli --release --test docs_site`; `python scripts/roadmap_proposals.py check`; `python scripts/process_report.py --check` | clean; 0 failures (a generated page only not current is left to the lead) | rules 7 and 9 | clean | n/a | code verification |

## Mutations (on a scratch copy of the tree at the change, its own target folder)

* **M1** (committed as `crates/rcfd-mcp/tests/adopt_f288.mutation.patch`, first line `# expect-fail: cargo test
  --release -p rcfd-mcp --test adopt_f288`): `RunKey::matches` in `crates/rcfd-workspace/src/design.rs` does not
  compare `solver_version`. Expected: A-N fails (decoy (h) is adopted).
* **M2**: decoy (h) is submitted by the test's own `RunService`, so its version equals the key's. Expected: A-N fails
  (the decoy made by change T is the one that carries the check).

## Decision rule

* **done**: PC1 and PC2 are met, a2 names a cause (at least one refused attempt seen in the probe series, its holders
  of one class), change T (or, for H1, the change of the holding code) is made, A0 and A1 pass 20 of 20 each, M1 and M2
  are caught, and a6, E and P hold. The cause is stated only as far as a2 and H3 show it; that the left-out run met the
  same holder is not shown by any of this and is said so.
* **partial**: no refused attempt is seen in any probe series and R0 and R1 do not fail (the failure not reproduced,
  recorded with the numbers), or refused attempts are seen with no holder named or with holders of more than one class;
  in these cases change T is still made and A0 and A1 are run, since it removes the only replace of a fresh file from the
  test; or a run of A0 or A1 fails; or a mutation survives; or a registered series was not made.
* **stopped and reported**: the holder is code of `crates/rcfd-mcp/src/project.rs`, or a control would need a change of
  the machine's settings (the scanner's or the indexer's), or a run of A0 or A1 fails on a file refused by the operating
  system; nothing is retried or loosened then.

## Open questions (for the lead; none of scope or priority)

1. The old making of (h) is kept as the ignored record `a_n_record_before_adr0773`. Options: (a) as registered, rule 2;
   (b) change the test in place without the record. Recommendation: (a). Taken on the maintainer's standing
   instruction of 2026-11-11 (take the recommendation).

## What is not covered

* Whether the left-out run of d08569e7 met the holder the probe names: its project folder was removed with the test's
  scratch and nothing of the moment was recorded.
* The product's own replaces of a new file (the runner's first `status.json` after the store made it): if the probe
  names a holder of new files outside the test's processes, the product meets it too; that is reported in the addendum
  as a follow-up, not changed here.
* Linux and macOS: an open file there does not refuse a rename; only Windows 11 is measured.

## Amendment 1 (2026-11-23, after PCs, P0, P0a, PLC, PLF, PLFa, R0 and the first run of R1; before change T and before any run of A0 or A1)

The first run of R1 (the whole binary under L1, beside three solves of another agent on the machine) took 1249.78 s
against 9.89 to 37.38 s for the twenty runs of R0; in it A-N passed and A-R failed at `adopt_f288.rs:215` ("A-R (i)
record 1 against the uninterrupted study": `ok` false against true), a failure of another test and another kind than
F-876, reported in the addendum and not diagnosed here. At that rate R1 and A1 would hold sixteen processes of load on
the shared machine for about fourteen hours. R1 was stopped after its first run (its processes ended by this work; none
of another agent was touched), and the set-up is corrected as follows, before any further run:

* **R1** is recorded as made with 1 run of 20. **R1b**: 20 runs of the binary before change T under **LF** (the load of
  new files, the kind of file the probe's holder acts on), made before change T.
* **A1** is made under LF instead of L1, 20 runs; A0 is unchanged (20 runs under L0).
* The rows a3-R and a4 and the decision rule read R1b for R1 and LF for L1 in A1. Nothing else changes: no limit, no
  count, no other series.

Why LF and not LC: the probe series did not name LC as lengthening the hold (H3: PLC's longest refusal 1842.5 ms against
P0's 1323.0 ms, a factor 1.39, under the factor 2 of the rule, and no would-fail iteration), and LC is the part of L1
that slowed the test eighty times beside the other agent's solves. The choice is taken after these results and is
recorded as such; it does not change what is counted or what passes.

## Amendment 2 (2026-11-23, after R1b, change T and the run of M1; before M2, M3, A0 and A1)

M1 (no comparison of `solver_version` in `RunKey::matches`) does not fail A-N: the server adopts decoy (h), which A-N
holds paused, and waits for its end without a limit (`wait_end`, "no time limit, as in process"), so the test does not
end. Observed: no new run folder (the ten decoys only), (h) with `solver_version` `unknown` paused, the test running
for 81 minutes; then stopped by this work. That is a test that does not pass, not a failing test, and the lead's push
script cannot run it as a committed mutation. M2 would meet the same wait (its decoy (h) is paused as well) and is not
run. The set-up of the mutations is corrected as follows:

* **M1** is recorded as run: not passing, by a wait without end (above), not as a failure.
* **M3**, the committed mutation instead of M1 (`crates/rcfd-mcp/tests/adopt_f288.mutation.patch`, the same first
  line): `choose` in `crates/rcfd-workspace/src/design.rs` no longer leaves out `failed` and `cancelled` runs. Decoys
  (i) and (j) carry the full key, and (j), the newest, has ended, so the server adopts it and A-N must fail at its
  assertion `runs_adopted == 0`. Expected: A-N fails.
* Row a5 and the decision rule read M3 for M1 and M2. That A-N catches an adopted paused decoy only by not ending is
  reported under "Open questions for the maintainer"; it is not changed here.

## Addendum 1: results (2026-11-23)

Commits: 44827995 (this registration, alone), 6fe84518 (amendment 1), 2b82e89c (change T), 9f3e97da (amendment 2),
e8d15386 (the committed mutation M3), then this addendum and the proposal file. Release builds, `CARGO_TARGET_DIR` in
the scratch folder. The full records (`PCs.tsv`, `P0.tsv`, `P0a.tsv`, `PLC.tsv`, `PLF.tsv`, `PLFa.tsv`, `series-*.log`,
`M1.log`, `M3.log`) and the scripts (`probe/`, `loads.py`, `series.sh`, `summary.py`, `make_change_t.py`) are in the
scratch folder; the tables below are their summary.

### The checks

| Id | Measured | Limit met |
|---|---|---|
| PC1 | 10 of 10: the rename refused with code 5 for the whole hold (5 to 7 attempts, 1018 to 1185 ms), the probe's own pid named at every refused attempt; `atomic_write` under the same hold returned `Retry` with 12 attempts and `Os { code: 5, PermissionDenied }` after 540 to 550 ms, 10 of 10 | yes |
| PC2 | 10 of 10 replaced at the first attempt (0.7 to 1.3 ms); `atomic_write` 10 of 10 `Ok` | yes |
| PC3 | 10 of 10 refused for the hold with code 32 (not 5), own pid named on the source | yes |
| a2 | the table below: refused attempts in all three fresh series, none in the two aged ones; every refused attempt (40) names pid 4, the System process, on the target and nothing on the source | yes: one class, H2 |
| H3 | longest refusal PLC 1842.5 ms, PLF 1246.2 ms, against P0 1323.0 ms (factors 1.39 and 0.94); no would-fail iteration in any series | the load is not named as lengthening the hold |
| a3-R | R0 20 of 20 pass (9.89 to 37.38 s); R1 1 run made (amendment 1): A-N passed, A-R failed (another failure, below); R1b 18 of 20 pass, 2 failed (another failure, below); no `Retry` in any run | reported; the F-876 failure not reproduced in 41 runs |
| a4 | A0 20 of 20 pass (6.06 to 9.29 s); A1 (LF) 20 of 20 pass (10.37 to 53.75 s) | yes |
| a5 | M3 caught: A-N failed at `runs_adopted` (0 expected, 1 found: the failed decoy (j) adopted), 1.17 s. M1: A-N does not end (amendment 2), stopped after 81 minutes. M2 not run (amendment 2) | yes, by M3 |
| a6 | A-N runs in the default build under its old name; the record `a_n_record_before_adr0773` is ignored; no retry of a test; `git diff main -- crates/rcfd-project/src/atomic.rs` empty | yes |
| E | `cargo test -p rcfd-mcp --release --lib`: 18 passed, 1 ignored (an existing ignore) | yes |
| P | `cargo fmt -p rcfd-mcp --check` and `cargo clippy -p rcfd-mcp --all-targets -- -D warnings` clean; `docs_site`, `roadmap_proposals.py check` and `process_report.py --check` as the report of this work states | see the report |

The probe series:

| Series | Load | Mode | n | Refused iterations | Would-fail | Longest refusal | Holders at refused attempts |
|---|---|---|---|---|---|---|---|
| P0 | L0 | fresh | 300 | 6 | 0 | 1323.0 ms | System (pid 4), 19 of 19 attempts, code 5 |
| P0a | L0 | aged | 100 | 0 | 0 | none | none |
| PLC | LC | fresh | 300 | 6 | 0 | 1842.5 ms | System (pid 4), 12 of 12, code 5 |
| PLF | LF | fresh | 300 | 9 | 0 | 1246.2 ms | System (pid 4), 9 of 9, code 5 |
| PLFa | LF | aged | 100 | 0 | 0 | none | none |

### What the measurements say

* The signature of the failure (`Retry`, 12 attempts, code 5 on the target) is what a handle on run.json without
  delete sharing gives (PC1), and not what a handle with every sharing mode gives (PC2, the way the test, the server
  and the runner read the file); a handle on the source gives code 32 (PC3), which the failure did not show.
* In the test's own sequence, without any runner or server (`runner.lock` absent at the end of every series), 21 of 900
  fresh replaces of a run.json made a moment earlier were refused with code 5, and at every one of the 40 refused
  attempts the only holder of run.json was pid 4, the System process: a handle opened in kernel mode, not by a process
  of the test. None of 200 replaces of a run.json 3 s or more old was refused. The cause shown is therefore H2: **a
  kernel-mode handle on a newly written file, held for up to 1.84 s, which refuses its replace.** Which driver opens it
  is not shown: pid 4 is where a file system filter's open is counted, and the machine's real-time scanner (on) and
  indexer (running) both act on new files; the probe cannot tell them apart without a change of the machine's
  settings, which this work does not make. H1 is excluded by the same naming. H3 (the load) is not shown to lengthen
  the hold.
* A refused attempt is itself slow (about 190 ms each in P0, up to 1.2 s in PLF), so 12 refused attempts take about
  2.9 s or more; no iteration needed more than 7 attempts, so the full failure was not reproduced; the holds observed
  (up to 1.84 s) are of the same kind. That the left-out run of d08569e7 met this holder is not shown by any record of
  that run.
* Change T removes the only replace of a new file from A-N; A0 and A1 pass, and the decoy still differs in
  `solver_version` alone (`unknown` against the key's `rcfd 0.1.0`, asserted in the test).

### Other failures seen (not of F-876, not diagnosed here)

* R1 run 1 (L1, 1249.78 s): A-R failed at `adopt_f288.rs:215`, "A-R (i) record 1 against the uninterrupted study",
  `ok` false.
* R1b run 4 (LF): A-N failed at line 215, its new run's record `ok` false with the message that the read of the run's
  `status.json` found no file (os error 2), and A-R failed the same way at record 0; run 6: A-R failed at record 1, an
  adopted run reported `queued` and counted failed. These are reads and waits of the product (rcfd-workspace,
  rcfd-runner) under a load of new files, of the binary before change T; change T does not touch them, and A0 and A1
  did not meet them in 40 runs. They are proposed as a follow-up below.

### Deviations

* Amendment 1: R1 stopped after 1 run; R1b and A1 under LF instead of L1.
* Amendment 2: M1 does not fail but waits without end; M2 not run; M3 committed instead.
* The probe depends on `rcfd-workspace` for the case text (`templates::LID_DRIVEN_CAVITY`, the text `rcfd-mcp`
  re-exports), not on `rcfd-mcp`. The `tasklist` calls inside the series failed (an argument converted by the shell),
  so pid 4 was named by `tasklist` and `Get-Process` afterwards (pid 4 is the System process by its fixed id).
* M1 and M3 were run on the worktree itself and reverted with `git checkout`, not on a scratch copy (a second copy
  would have rebuilt the whole tree on the shared machine); the tree was clean after each.
* The load LF and the probe left their folders under the temporary folder (`rcfd-f876-*`), and the stopped runs left
  their test folders; nothing outside the scratch folder was deleted by this work, except the files LF itself wrote.

### What was not run

* M2 (amendment 2). R1 beyond its first run (amendment 1). The full workspace suite (rule 7).

### Actual size

Size (agent): done; 0; 11; 1; 230000

### Decision by the registered rule

**done.** PC1 and PC2 are met; a2 names one class of holder (H2, a kernel-mode handle of the System process on a new
file) at every refused attempt, absent on aged files; change T is made; A0 and A1 pass 20 of 20 each; the committed
mutation M3 is caught (M1 and M2 replaced by amendment 2); a6 and E hold. Not shown: the driver behind pid 4, and that
the left-out run met it.

## Open questions for the maintainer

1. A-N catches an adopted decoy that is paused only by not ending (amendment 2: `wait_end` has no time limit), so a
   mutation of the key cannot be a committed mutation, and a regression of the key would hang the gate rather than fail
   it. Options: (a) a follow-up item that lets A-N end on such an adoption (for example decoys left queued with the
   runner stopped, so that an adopted one runs, ends and fails the assertion), registered with its own mutation of the
   key; (b) leave A-N as it is. Recommendation: (a), proposed below.

## For the lead: roadmap entries

> Status: done.

> Note: ADR 0773, 2026-11-23, done: the refused replace in adopt_f288 A-N is shown to be a kernel-mode handle (the System process, pid 4) that holds a newly written run.json for up to 1.84 s (21 of 900 fresh replaces refused with code 5, 0 of 200 replaces of files 3 s old, no process of the test holding it; a handle without delete sharing reproduces the exact error), so decoy (h) is now written once with solver_version unknown instead of by a replace of a new run.json; A-N passes 20 of 20 without load and 20 of 20 under a load of new files, the committed mutation (failed and cancelled runs adopted) is caught, atomic.rs and its 12 attempts unchanged.

> Follow-up: "adopt_f288 under a load of new files: a run's status.json read as missing and an adopted run counted failed while queued"; deps: F-876; acceptance: "Every limit not stated here is registered in the ADR before the run. For the failures of ADR 0773 addendum 1 (R1b run 4: a read of status.json failed with os error 2; R1b run 6 and R1 run 1: an adopted run reported queued and counted failed, records ok false in adopt_f288 A-R and A-N), the cause is shown by a registered control with a registered number of runs under ADR 0773's load LF, and the code or the test is changed under the registration so that the registered runs pass."

> Follow-up: "adopt_f288 A-N ends when it adopts a paused decoy"; deps: F-876; acceptance: "Every limit not stated here is registered in the ADR before the run. A mutation that drops one item of RunKey::matches (ADR 0773 M1) makes adopt_f288 A-N fail at an assertion instead of waiting without end, shown by a committed mutation patch, and A-N passes a registered number of runs."

## The lead's reading (2026-11-23), at the merge

Merged at ecdfe05f and pushed at c967573b (the report read at 02:34 UTC, 9 minutes of reading, proposed done, written
done). Checked by the lead before the merge: the registration is the first commit and alone, and amendments 1 and 2 are
each committed before what they allow; the branch changes only `crates/rcfd-mcp/tests/adopt_f288.rs` and adds its
mutation patch, the ADR and the proposal; `atomic.rs` and `project.rs` are untouched. The earlier making of decoy (h)
is kept as the ignored record `a_n_record_before_adr0773`, with the test the gate runs named in its reason; it is the
record of the cause, not a test switched off to make a run pass. The push script ran the committed mutation and it
failed as expected; no binary was left out.

Status: **done**, as proposed. The cause is a kernel-mode handle on a newly written file; which driver holds it is not
shown, since that would need a change of the machine's settings, which the brief excluded. The agent stopped mutation
M1 after 81 minutes because it waits without end; the lead notes that the agent ran M1 and M3 in its worktree and
reverted them. The agent of this item ran on Claude Opus 5.5, not on the model its brief names; the lead records it
here. The two follow-ups are scope and are put to the maintainer with the agent's recommendation (a), which the lead
shares.
