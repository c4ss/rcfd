# ADR 0830: F-978 registration: why the first edit_case of B1 is refused under load, then B1 to B3 once

Status: **REGISTERED 2026-11-27, written before any code of F-978 in this work and before any run of its checks.** Before
this text nothing was changed, built or run but reading and three queries of the machine (Defender's state, the CPU load,
the processor count). Results go in addenda at the end; nothing above them is changed afterwards. Written by Claude Opus
5.5 (`claude-opus-5-5`), as a background agent of the lead.

It answers the brief `docs/adr/briefs/f978-edit-case-refused.md` ([the file](briefs/f978-edit-case-refused.md)) and
follows [docs/process/agent-rules.md](../process/agent-rules.md), version 5. Branch `f978-edit-case-refused` from
39d689c6, scratch `w49i-work/f978-edit-case-refused-scratch`. It is registered under F-978 (rule 13) and continues
[ADR 0826](0826-f978-registration-the-process-view-of-a-case.md) and
[ADR 0828](0828-f978-registration-process-view-rows-and-runner-teardowns.md) (partial: B1 to B3 not decided, the first
`edit_case` after `new_case` refused with "replace ...process-cavity.toml: failed after 12 attempts: Access is denied.
(os error 5)" at 82 % and 99 % CPU).

## What was read

`AGENTS.md`, the rules (version 5), the brief and ADR 0828, whole; `crates/rcfd-mcp/tests/canvas_process_f978.rs` whole;
F-978's acceptance (by a script); of `crates/rcfd-project/src/atomic.rs` lines 1 to 200 (`Retry`, `DEFAULT_RETRY`,
`is_transient`, `with_retry`, `atomic_write_with`, `write_new`, `read`), 219 to 226 (`EDIT_LOCK_WAIT` 2 s) and 420 to 560
(`replace_if_hash_with`) and the test hooks 640 to 704; `crates/rcfd-workspace/src/cases.rs` 60 to 200 (`create_files`:
`new_case` writes the case by `write_new`), `write.rs` 26 and 160 to 180 (`edit_case` replaces by `replace_if_hash_with`);
the head of `crates/rcfd-project/src/watch.rs`; `crates/rcfd-project/tests/hammer.rs` 135 to 215 (A3, the existing test
of a replace refused while the target is held without delete sharing); `crates/rcfd-mcp/tests/common/mod.rs` (`scratch`
in the system temp folder, `Mcp`); the toolchain's own source of `std::fs::rename` on Windows (rustc 1.99.0,
`library/std/src/sys/fs/windows.rs` lines 1321 to 1390). Bytes read: about 80 000.

## Findings of the reading

1. **The replace.** `edit_case` → `replace_if_hash_with` (atomic.rs 442) → `atomic_write` (DEFAULT_RETRY: 12 attempts,
   sleeps 1 ms doubling to 100 ms, 527 ms of sleep in all) → `fs::rename(tmp, case)`. std's rename (windows.rs 1321) calls
   `MoveFileExW(REPLACE_EXISTING)`; on ERROR_ACCESS_DENIED it tries again with `FileRenameInfoEx` and the flags
   `REPLACE_IF_EXISTS | POSIX_SEMANTICS`; if that fails too it returns **the first error, os error 5** (line 1382). The
   POSIX rename replaces a target that others hold open with delete sharing. So os error 5 means the target (or the temp
   file) was held open **without delete sharing**, for longer than the retries.
2. **Every rcfd reader shares delete.** std's `File::open` asks for read, write and delete sharing; the only
   `share_mode` calls outside tests (S1: `grep -rn share_mode crates --include=*.rs`) are the runner's lock files
   (`rcfd-runner/src/sys.rs`, `site/remote.rs`) and the test hook of atomic.rs 697. The `--canvas` server and the watch
   read case files with std. Predicted from the reading: no rcfd process can cause the refusal.
3. The new case file is created by `write_new` (a temp file, synced, hard-linked to the case's name, the temp name
   removed), then replaced at once by the first `edit_case`. B1's project is under the system temp folder
   (`common::scratch`). Defender's real-time protection is on (`Get-MpComputerStatus`: RealTimeProtectionEnabled True);
   its exclusions cannot be read without administrator rights. CPU 90 % at the time of writing, 16 logical processors.

## Hypotheses and the arms that decide them

| H | Hypothesis | File and line | Predicts | Arm | Remedy |
|---|---|---|---|---|---|
| H1 | a reader in an rcfd process (the `--canvas` server's reads, the watch) holds the new case file | watch.rs, the server's reads via std; atomic.rs 138 | refusals with the canvas server, none without; a held std reader refuses the replace | U1; arms A and B | none in the replace; the reader |
| H2 | a process outside rcfd (an antivirus or indexer scan of the newly written file) holds it without delete sharing | atomic.rs 138, windows.rs 1382 | at each refusal the trace shows a holder whose name is not `rcfd-*` | the trace X in arms A and B | a longer replace budget (F1) |
| H3 | the replace's retry budget (527 ms of sleep) is shorter than such a hold under load | atomic.rs 26 to 31 | the foreign hold at each refusal lasts longer than 0.53 s and less than 2 s | X in arms A and B | F1 |

- **X, the trace.** A scratch PowerShell script `trace.ps1` (C# by `Add-Type`) that every 2 ms opens each
  `cases/*.toml` of every folder `%TEMP%\rcfd-mcp-w48f-f978-*\quick-start` with read-attributes access and full sharing,
  asks the operating system which processes have the file open (`NtQueryInformationFile`, class 47,
  `FileProcessIdsUsingFileInformation`; the class and its layout are cited from memory, since no page of it was read,
  and the control TC checks the tool), closes it, and writes a line `epoch ms, file, pid:name...` whenever the set
  changes, leaving out its own pid. It runs beside arms A, B, A2 and the run of B1, and is ended after each.
- **U1** (code verification of finding 2): a file made by `write_new`, held by `std::fs::File::open`, is replaced by
  `atomic_write_with` with 1 attempt: Ok.
- **Arms A and B**: an ignored test each in `crates/rcfd-mcp/tests/edit_case_refused_f978.rs`, on a copy of
  `example-projects/quick-start` in `common::scratch("f978-arm-<a|b>")`: A with the `--canvas` server of B1 started, B
  without; both with the load L; 30 times `new_case` `cases/arm-<i>.toml` (copy of `cavity-re100.toml`) then two
  `edit_case` as B1's (`mesh.nx`, `mesh.ny` 16; `physics.nu` 0.02), each call's start and end in epoch ms and its result
  printed; 90 replaces in all (30 creations, 60 edits). Then TC.
- **L, the load**: threads equal to `available_parallelism` (16) spinning in the test process from before the first call
  to after the last; the machine's CPU load is recorded before each arm (`Win32_Processor.LoadPercentage`).
- **TC** (control of X): at the end of each arm the test holds the last case file with `File::open` for 500 ms; X must
  show the test process on that file.

**Rules of decision, fixed now.** H1 is refuted if U1 passes and every refusal in A and B has a holder not named `rcfd-*`
in X during it; confirmed if a refusal has only `rcfd-*` holders in X. H2 is confirmed if at least one refusal occurs in
A and B and every refusal has a non-rcfd holder in X during it. H3 is confirmed if H2 is and the longest non-rcfd hold
that overlaps a refusal is under 2 s. **The cause is found** when H2 and H3 are confirmed and H1 is refuted. If A and B
give no refusal, the arms do not decide and the cause is not found.

## The fix F1, taken only if the cause is found

`crates/rcfd-project/src/atomic.rs`: a constant `EDIT_REPLACE_RETRY: Retry` = 27 attempts, 1 ms doubling to 100 ms (1
+ 2 + ... + 64 + 19 × 100 = 2027 ms of sleep, at least `EDIT_LOCK_WAIT`, 2 s, the time a cooperative writer already
waits for the edit lock, atomic.rs 219), used by `replace_if_hash_with` for its replace instead of `DEFAULT_RETRY`
(`atomic_write_with`). Every other user of `DEFAULT_RETRY` (`atomic_write`, `read`, the locks, the trash) is unchanged.
Defaults: a replace that succeeds writes the same bytes and journal line as before; only a refusal comes later (2.03 s
of sleep instead of 0.53 s) and names 27 attempts. No golden, hash table or existing test is changed (hammer.rs A3 tests
`atomic_write`, which keeps `DEFAULT_RETRY`). If the cause is not found, F1 is not made and is reported as an option.

## Checks (rule 14)

Release profile, `CARGO_TARGET_DIR` under the scratch folder, `CARGO_BUILD_JOBS` 8, `RCFD_BIN` from that target. New
tests: `crates/rcfd-project/tests/replace_refused_f978.rs` (U0 to U2, Windows only) and
`crates/rcfd-mcp/tests/edit_case_refused_f978.rs` (arms A, B, A2, ignored: they load the whole machine on purpose and
are run once each by this work).

| Id | Check | Limit | Source | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|
| U1 | finding 2: std reader held, replace with 1 attempt | Ok | the brief (an arm per hypothesis) | Ok | exact | code verification |
| A, B | arms A and B with X | decides H1 to H3 by the rules above | the brief | H2, H3 confirmed, H1 refuted | rule | code verification |
| TC | X sees the test's 500 ms hold, in A and in B | at least 1 line with the test's pid on the file | the brief (a trace of the handles) | 1 | exact | code verification |
| U0 | F1: target held without delete sharing throughout `replace_if_hash` | `ProjectError::Retry`, 27 attempts, os error 5, the file unchanged | finding 1; F1 | as limit | exact | code verification |
| U2 | F1: the same hold released after 1.0 s | Ok, the new bytes, waited at least 0.9 s | F1 (1.0 s between 0.53 s and 2.03 s) | Ok | 2.03 | code verification |
| A2 | arm A again after F1, same load | 0 refusals of 90 | the brief (the replace under the registered load) | 0 | exact | code verification |
| B1 | `canvas_process_f978` once, as ADR 0826 registered it, with X beside it, CPU recorded | every assertion of B1 | ADR 0826 B1 | holds | exact | code verification |
| B2 | the same run | as ADR 0826 B2 | ADR 0826 B2 | holds | exact | code verification |
| B3 | the same run | as ADR 0826 B3 | ADR 0826 B3 | holds | exact | unchanged default output |
| T3 | `rcfd-runner` processes of this work's target alive after the B1 run | 0 | ADR 0828 T3 | 0 | exact | code verification |
| L1 | `rcfd-project --lib`, `rcfd-mcp --lib`; the existing `hammer`, `edit_lock_f755`, `edit_lock_f855` of `rcfd-project` | pass | rule 7 | pass | exact | unchanged default output |
| L2 | `cargo fmt --check` and `cargo clippy --release --all-targets -- -D warnings` of `rcfd-project` and `rcfd-mcp` | clean | rule 7 | clean | exact | unchanged default output |
| L3 | `docs_site`; `roadmap_proposals.py check`; `process_report.py --check` | pass (the last: only a generated file not current may fail) | rules 7, 9 | pass | exact | unchanged default output |

If F1 is not taken, U0, U2 and A2 are not run and B1 runs on the unchanged code. An OS refusal in B1 is recorded as it
comes and not run again.

### Negative controls and mutations (rule 16)

- **N1** is U0: the hold that F1 cannot outlast must still be refused with os error 5 (the observed message).
- **M1** (committed as `crates/rcfd-project/tests/replace_refused_f978.mutation.patch`): `replace_if_hash_with` back to
  `atomic_write` (DEFAULT_RETRY). Must fail: U2 (refused after 12 attempts while the hold lasts 1.0 s). First line `#
  expect-fail: cargo test --release -p rcfd-project --test replace_refused_f978 --target-dir target/mutation`. Run on a
  scratch copy of the final commit with its own target folder.

## Size

Size (agent): done; 0; 4; 0; 120000

(Expected done; no further registration; four heavy runs: arms A, B, A2 under the load, and B1; no question expected;
about 120 000 bytes read.)

## Decision rule

**done** for F-978 if the cause is found (the rules above), B1 to B3 hold in their one run and M1 is caught; **partial**
otherwise, naming what failed. A missed limit is recorded with its number and held as a `*_known_miss` record (rule 2).

## Not covered

- Which product of the antivirus or indexer holds the file beyond its process name, and its share mode beyond what
  finding 1 implies; Defender's settings (administrator rights).
- The other writers of `DEFAULT_RETRY` (`atomic_write` of status files, views, derived pictures), which F1 leaves as
  they are; Linux and macOS, where an open file does not block a rename.

## Amendment 1 (2026-11-27): the trace starved under the load; X corrected before any further run

Written after U1 and arm A and before any other run; nothing above is changed. U1 passed (`U1: Ok(())`). Arm A was run
once (CPU 52 % at the start, 68 % at the end, 3.65 s): **2 refusals of 90**, `arm-19.toml` edit1 (619 ms) and edit2
(618 ms), both "failed after 12 attempts: Access is denied. (os error 5)"; the other 28 creations and 56 edits went
through. A first start of the arm script stopped on a cargo argument error before any test ran; it is not a run. Under the
load L the trace X was starved: it reached each file about once every 60 ms, saw no holder of `arm-19.toml` during
either refusal and one line `4:System` on it 665 ms after the second, and never reached `arm-29.toml` during TC. **TC
missed in arm A** (0 lines with the test's pid), so X does not decide in arm A and the rules of decision cannot use it.

The set-up was defective (the trace ran at the priority of the load it watches). Corrected, by rule 2:

- **X'**: `trace.ps1` sets its own process to the priority class High (`ProcessPriorityClass.High`, which a user may
  set), resolves each pid's name once, and looks only at `arm-*.toml` and `process-cavity.toml`. Everything else of X
  is as registered.
- **Arm A1**: arm A again with X', a new ignored test `arm_a1_with_the_canvas_server_under_load_traced_at_high_priority`
  (the same function, tag `a1`); then **arm B** with X', as registered. The rules of decision use A1 and B in place of A
  and B; arm A's 2 refusals are kept beside them as its record. TC applies to A1 and B with its registered limit.
- Nothing else changes: limits, U0, U2, F1, A2, B1 to B3 and M1 are as registered.

## Amendment 2 (2026-11-27): one look of the trace takes 71 ms; X looks at the newest case only; 100 rounds

Written after arm A1 and before any other run; nothing above is changed. **Arm A1** (CPU 85 % at the start, 82 % at the
end, 3.04 s): **0 refusals of 90**; **TC missed again** (0 lines with the test's pid): X' looked at each file once in
about 2 s. A timing of the tool alone (`tooltime.ps1` in the scratch folder, 20 looks each, not a check) gave 71 ms
per look on a file in the temp folder and 71 ms on one in the scratch folder, against 0.15 ms and 0.05 ms for a plain
`GetAttributes`, and a held file was seen (`61640:powershell`): the class-47 query is slow in itself, not starved. With
30 files a look per file came every 2 s, longer than TC's hold and than a refusal (619 ms).

Corrected, by rule 2:

- **X''**: X' that looks, in each folder, only at the newest `arm-<i>.toml` (the largest i present) and at
  `process-cavity.toml`: one look per 71 ms or so, about 8 looks in a refusal of 0.62 s and 7 in TC's 500 ms.
- **Rounds**: arm A gave 2 refusals in 90 replaces and A1 none, about 1 in 90. At 30 rounds (90 replaces) an arm sees no
  refusal with a chance of about e^-1 = 37 %; at 100 rounds (300 replaces) about e^-3.3 = 4 %. The new arms make **100
  rounds** (300 replaces).
- **Arms A3 and B**: A3 is arm A with X'' and 100 rounds (a new ignored test, tag `a3`); arm B is as registered with X''
  and 100 rounds. The rules of decision use A3 and B; arms A and A1 are kept beside them as their records. TC applies to
  A3 and B. A2 (after F1) also makes 100 rounds, with its limit of 0 refusals.
- Nothing else changes.

## Addendum 1: results (2026-11-27)

Written after the runs; nothing above is changed. Release profile, `CARGO_TARGET_DIR` in the scratch folder,
`CARGO_BUILD_JOBS` 8. Each registered run was made once; the scratch logs are `arm-<tag>.log` and `trace-<tag>.tsv`.

### Checks

| Id | Limit | Measured | Met |
|---|---|---|---|
| U1 | Ok | `U1: Ok(())`, the new bytes, one attempt | yes |
| A (record) | decides H1 to H3 | 2 refusals of 90 (`arm-19.toml` edit1 619 ms, edit2 618 ms), CPU 52 % to 68 %; X starved | not decided (amendment 1) |
| A1 (record) | as A | 0 refusals of 90, CPU 85 % to 82 % | not decided (amendment 2) |
| A3 | as A, X'', 100 rounds | **1 refusal of 300** (`arm-5.toml` edit1, 594 ms), CPU 83 % to 79 %; no holder seen during it, `4:System` seen 175 ms after it | see the rules below |
| B | as A3, without the `--canvas` server | **4 refusals of 300** (`arm-26` edit1 587 ms; `arm-42` edit1 590 ms and edit2 603 ms; `arm-87` edit1 612 ms), CPU 88 % to 88 %; `4:System` seen during `arm-42`'s two and `arm-87`'s; none seen during `arm-26`'s, `4:System` 68 ms after it | see the rules below |
| TC | the test's pid seen, in A, A1, A3 and B | A no, A1 no, A3 **no**, B **yes** (`59076:edit_case_refused_f978-...` on `arm-99.toml`) | **missed** in A3 |
| U0, U2, A2 | F1's | not run: F1 not taken | not run |
| B1 | every assertion | held: 10 steps, journal seq 1 to 5 against 1 to 5, states 3 against 3 and terminal, residuals 4 of 4 equal, results, ordered, after-run set-up last, views shown 10 of 10; CPU 80 % at the start, 68 % at the end, 5.30 s; X saw no holder of `process-cavity.toml` | **yes** |
| B2 | as ADR 0826 | refused at `panels[0].kind` (1003), nothing written | **yes** |
| B3 | as ADR 0826 | the description names `process {case}`; the plain `read_view` has no `process` | **yes** |
| T3 | 0 | 0 runners of this work's target alive after the B1 run | yes |
| L1, L2, L3 | pass, clean | in the last run, reported with the commit | see the report |

**The rules of decision.** Over A3 and B, 5 refusals: 3 with a holder not named `rcfd-*` in X'' during them (all
`4:System`, a handle held in kernel mode), 2 with no holder seen during them (and `4:System` seen at the next look after
them). No rcfd process was seen holding a case file in any arm, and B, without the `--canvas` server, had more refusals
(4) than A3 with it (1). By the registered rules: H1 is **not refuted** (2 refusals without a seen non-rcfd holder) and
**not confirmed** (no refusal with only rcfd holders); H2 is **not confirmed** (not every refusal has one); H3 **does not
decide**: X'' leaves a file when a newer one is made, so no hold has a seen end. **The cause is not found**, and F1 was
not made. What the arms show, without deciding: the refusal does not need the `--canvas` server (B), a reader sharing
delete does not refuse the replace (U1), and in 3 of 5 refusals the System process held the file during the refusal.

Deviations: amendments 1 and 2 (the trace's set-up corrected twice before the runs that use it; arms A and A1 kept as
records). The committed mutation of rule 16 (M1) is not made: it mutates F1, which was not taken; no other mutation was
registered, so this work commits none. Not run: U0, U2, A2.

Actual size: partial; 1 further registration (the follow-up below); 5 heavy runs (arms A, A1, A3, B, and B1); 0
questions taken from the brief's list beyond the one below; about 95 000 bytes read.

Decision by the registered rule: **partial**, because the cause is not found and M1 was not made; B1 to B3 held in their
one run (CPU 80 %).

## Open questions for the maintainer

- **Q1. The remedy, with the cause not decided** (scope; a change of `crates/rcfd-project/src/atomic.rs`, a shared file
  outside the brief's list): (a) F1 as registered (27 attempts, 2.03 s of sleep, for `replace_if_hash_with` only), in
  a follow-up registration that first decides H3 with a trace that keeps looking at a refused file until its holder
  ends; (b) F1 now, on the evidence above (every refusal took 587 to 619 ms, the 12-attempt budget, and no rcfd holder
  was ever seen); (c) nothing until the System holder is named. Recommendation: (a), since the registered rule did not
  find the cause; not taken here, since it is a question of scope.

### For the lead: roadmap entries

> Status: partial.

> Note: ADR 0830, 2026-11-27, partial: B1 to B3 held in their one registered run (CPU 80 %, 10 of 10 steps, journal seq 1 to 5 against 1 to 5); the diagnosis of the refused edit_case reproduced it under a 16-thread load (1 of 300 replaces with the canvas server, 4 of 300 without it, each refused after the 12-attempt budget, 587 to 619 ms) and saw only the System process holding the file, in 3 of the 5 refusals, so by its rule the cause is not found.

> Gap: B1 to B3 now hold (ADR 0830). Missing: the cause of the refused first edit_case under load. The refusal comes without the canvas server (4 of 300 replaces), a reader that shares delete does not cause it, and in 3 of 5 refusals the System process (a kernel-mode handle) held the new case file; its length is not measured. A registration under F-978 that traces a refused file until its holder ends, then F1 (a longer replace budget for edits) if the hold is under 2 s.

> Follow-ups: one under F-978 (rule 13), no new item: "F-978: the System holder of a refused edit_case", a trace that keeps looking at a refused file until its holder ends, then F1 of ADR 0830 with its checks U0, U2, A2 and its mutation M1; its limits are ADR 0830's.

The same entries are `docs/roadmap/proposals/0830-F-978.json`.
