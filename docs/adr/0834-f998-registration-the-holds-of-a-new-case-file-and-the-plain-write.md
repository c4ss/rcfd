# ADR 0834: F-998 registration: the System process's holds of a new case file as the measure, and the plain write that removes them

Status: **REGISTERED 2026-11-27, written before any code of F-998 and before any run of its checks.** Before this text
nothing was changed, built or run but reading, counting ADR 0833's traces by a script and Poisson arithmetic. Results go
in addenda at the end; nothing above them is changed afterwards. Written by Claude Opus 5.5 (`claude-opus-5-5`), as a
background agent of the lead.

It answers the brief `docs/adr/briefs/f998-plain-write.md` ([the file](briefs/f998-plain-write.md)) and follows
[docs/process/agent-rules.md](../process/agent-rules.md), version 5. Branch `f998-plain-write` from e0c235ec (=
origin/main), scratch `w49i-work/f998-plain-write-scratch`. It answers Q1 (b) of
[ADR 0833](0833-f998-registration-the-refused-replace-outside-temp-and-plainly-written.md) as the lead took it.

## What was read

`AGENTS.md`, the rules (version 5), the brief, ADR 0833, `crates/rcfd-project/src/atomic.rs` and
`crates/rcfd-mcp/tests/replace_hold_f978.rs`, whole; `crates/rcfd-mcp/tests/common/mod.rs` (`scratch`, `binaries`,
`Mcp` and its `Drop`, which ends the server: ADR 0828); every caller of `write_new` and `write_json_new`
(`rcfd-project` `project.rs` 129, 275, 293 and `trash.rs` 307; `rcfd-runner` `store.rs` 64, 239, 240;
`rcfd-workspace` `cases.rs` 142, 180, 187 and `meshes.rs` 676, 707; `rcfd-runner/src/site/agent.rs` has its own
`write_new` and is not a caller); the heads of `hammer.rs`, `records.rs` (`i1_run_json_is_never_replaced`) and
`replace_refused_f978.rs`; ADR 0833's `trace4.ps1`, `run_x4.sh`, `runs.sh`, `analyse4.py`; F-998's acceptance (by a
script). Bytes read: about 75 000.

**ADR 0833's holds, recounted** from `trace-<arm>.tsv` (a SEEN line is a look that found a non-`rcfd` holder; FREE the
first look after it that found none): H 18 files, all `4:System`, SEEN to FREE 141 to 237 ms, of 129 distinct files
the trace looked at; HO 4 (144 to 156 ms) of 139; HP 0 of 111; HOP 0 of 105. X4 looks at the newest `arm-<i>.toml`
of each project on every loop, so it sees about 40 % of the 300 files of an arm.

Arithmetic beforehand (Poisson). H at its ADR 0833 rate, 18 per 300 rounds, gives a mean of 27 in 450 rounds:
P(H ≤ 7) = 5.2 × 10^-6. At HO's rate (a mean of 6), P(H ≥ 8) = 0.26, so a run at that rate would usually not decide.
If P held files at H's rate, P(P ≤ 2 | mean 27) = 7.4 × 10^-10, and with H at the floor, 8, P(P = 0 | mean 8) = 3.4 ×
10^-4.

## Design

**The remedy P** (`write_new` in `crates/rcfd-project/src/atomic.rs`, on Windows only; on other systems the function
is unchanged, since an open file there does not refuse a rename and ADR 0832's refusals are Windows refusals):

1. open `path` with `OpenOptions::new().write(true).create_new(true)` and `share_mode(0)` (std's
   `std::os::windows::fs::OpenOptionsExt`, no unsafe code, no new crate); `AlreadyExists` gives
   `ProjectError::Exists`, any other error `ProjectError::Io` (op `create`), as now;
2. `write_all(bytes)`, then `sync_all()`, then close;
3. on an error of step 2 the file it created is removed (best effort) and `ProjectError::Io` (op `write`) is returned.

No temp file, no hard link. What it keeps of the current path, named:

- **No partial file seen by a reader** while the writer runs: with share mode 0 no other open of the file succeeds
  (Windows reports a sharing violation, error 32, which `atomic::read` retries as transient: `is_transient`) until the
  bytes are written, synced and the handle closed. Checked by W1.
- **No file replaced that should not exist**: `create_new` is one call of the file system that fails when the name
  exists (`CREATE_NEW`); nothing existing is opened for writing. Checked by W3 and the existing `records.rs` I1.
- **Its behaviour when the file exists**: `ProjectError::Exists`, the existing file untouched (W3).
- **Durability across power loss**: as now, the bytes are synced before success is returned (`sync_parent` is a no-op
  on Windows now and stays one).
- **Not kept: a writer killed in the middle.** The current path leaves at most a `.tmp` file and never a partial file
  under the final name; P, killed between steps 1 and 3, leaves a partial file under the final name, which a later
  `write_new` of that name refuses as existing. No crash test is registered for it, since the property is given up by
  design; it is the open question Q1.

P differs from ADR 0833's HP (which gave 0 holds) in two things: the creating process is the server, not the test, and
P syncs (HP did not). The rule below reads P as a whole.

**The measure.** For an arm, the trace X5 (ADR 0833's `trace4.ps1`, copied unchanged to the scratch folder) beside it,
and the scratch script `analyse5.py`: **h = the number of distinct case files `arm-<i>.toml` with a SEEN line whose
holders include `4:System`**, with each one's SEEN-to-FREE length, the number of distinct files the trace looked at,
and the arm's refusals (creation and `edit_case`) from its log. CPU (the average `LoadPercentage`) is recorded before
and after each run.

**The arms**, each ADR 0833's `arm_f998` (no `--canvas` server, one spinning thread per logical processor, 16 here,
`new_case` with `copy` then B1's two `edit_case` per round, the project in `%TEMP%`) with **450 rounds** (900
`edit_case`, the acceptance's number), as new ignored tests in `replace_hold_f978.rs`:

- **H**, `f998b_h_hold_count` (tag `bh`): the test binary built at the commit that adds the arms, before the remedy,
  in the target folder `target-h` (the old `write_new`).
- **P**, `f998b_p_hold_count` (tag `bp`): the test binary built at the remedy's commit, in the target folder `target`.

Both binaries are built first and then run back to back, H then P, by running the test executables directly, so that
the two arms see the machine as close in time as possible. Each `rcfd-mcp.exe`'s sha256 is logged with its commit.

**TC4**, the trace's control: ADR 0833's `f998_tc4_trace_control`, unchanged, with X5 beside it, in the outside root
`RCFD_F998_OUTSIDE` = the scratch folder's `outside`.

**The rule.** The run decides only if **H gives h ≥ 8**; then **P passes if h(P) ≤ 0.1 × h(H)**, and fails otherwise.
With H below 8 the run does not decide.

## Checks (rule 14)

Release profile, `CARGO_TARGET_DIR` in the scratch folder, `CARGO_BUILD_JOBS` 8. New fast tests in
`crates/rcfd-project/tests/write_new_f998.rs` (W1 and N3 on Windows only).

| Id | Check | Limit | Source | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|
| W1 | 3 reader threads `fs::read` the file being created, while 20 files of 8 MiB are made one after another by `write_new` | partial reads (a read that returns other bytes than the whole) = 0; reads refused by a sharing violation ≥ 5 (the readers met the write) | ADR 0122 A1 (a reader never sees a partial file), the hammer's overlap minimums | 0; hundreds | exact; > 20 | code verification |
| N3 | W1's set-up with a plain write without share mode 0 (test-local) | partial reads ≥ 1 | the negative control of ADR 0122 A1 (`hammer.rs`) | many | rule | code verification |
| W2 | `write_new` of 0, 1, 4095 and 2^20 + 3 bytes of a pattern | read back equal; no `.tmp` name in the folder | the brief (the bytes written equal) | holds | exact | code verification |
| W3 | `write_new` onto an existing file | `ProjectError::Exists`; the existing bytes unchanged | the brief (no file replaced, its behaviour when the file exists) | holds | exact | code verification |
| TC4 | X5 sees the test's 800 ms hold | the test's pid seen on `arm-0.toml`; one FREE, 0 to 720 ms after release | ADR 0833 TC4 (128 ms measured) | seen; 130 ms | 5.5 | code verification |
| H | the control (N1) | h ≥ 8 | the arithmetic above from ADR 0833's 18 of 300 | 27 | 3.4 | code verification |
| P | the rule | h(P) ≤ 0.1 × h(H) | the brief (a registered fraction of H's), the arithmetic above | 0 | rule | code verification |
| A | F-998's acceptance under P | 0 `edit_case` refusals of P's 900 | F-998's acceptance | 0 | exact | code verification |
| L1 | callers and existing tests: `rcfd-project` (`--features test-hooks`) `--lib`, `write_new_f998`, `hammer`, `records`, `replace_refused_f978`, `edit_lock_f755`, `edit_lock_f855`; `rcfd-workspace` `--lib`, `new_case_f271`, `mesh_f272`; `rcfd-runner` `--lib`, `run_records_adr0734`; `rcfd-mcp` `--lib`, `replace_hold_f978` (S1), `new_simulations_f271`, `edit_case_refused_f978` | pass (the files' own ignored tests stay ignored) | rule 7; the brief (every caller, every existing atomic-write and replace test) | pass | exact | unchanged default output |
| L2 | `cargo fmt --check` of `rcfd-project` and `rcfd-mcp`; `cargo clippy -p rcfd-project -p rcfd-mcp --release --all-targets -- -D warnings` | clean | rule 7 | clean | exact | unchanged default output |
| L3 | `docs_site`; `roadmap_proposals.py check` of the proposal; `process_report.py --check` | pass (the last: only a generated file not current may fail) | rules 7, 9 | pass | exact | unchanged default output |

Order: W1 to W3 and N3 (one run of `write_new_f998`), TC4, H, P, mutations, L1 to L3. A refusal of a file by the
operating system in a fast test is recorded, not run again.

### Negative controls and mutations (rule 16)

- **N1** is H: the unchanged path must still be held (h ≥ 8), or nothing is read.
- **N2** is TC4: the trace must see a known hold.
- **N3** (above): without share mode 0 the readers do see partial files, so W1 can fail.
- **MP1** (committed as `crates/rcfd-project/tests/write_new_f998.mutation.patch`): P without `share_mode(0)`. Must
  fail: W1. First line `# expect-fail: cargo test --release -p rcfd-project --test write_new_f998 --target-dir
  target/mutation w1_`.
- **MP2** (scratch): P writes all bytes but the last. Must fail: W2.
- **MP3** (scratch): P opens with `create(true).truncate(true)` instead of `create_new(true)`. Must fail: W3.

The mutations are run on a scratch copy of the final commit with their own target folder.

## Size

Size (agent): done; 0; 2; 1; 75000

(Expected done; no further registration; two heavy runs, H and P; one question, Q1, the guarantee given up; about 75
000 bytes read.)

## Decision rule

**done** for F-998 if the rule holds (H ≥ 8 and h(P) ≤ 0.1 × h(H)), W1 to W3, N3, TC4 and L1 to L3 hold, A gives 0
refusals and each of MP1 to MP3 is caught; **partial** otherwise, with the numbers. A run that does not decide (H < 8)
is partial. A missed limit is recorded with its number (rule 2).

## Not covered

- Which driver behind the System process holds the file, and whether it reacts to the hard link, the temp file's
  removal or the creating process; P is read as a whole. The machine's antivirus and indexer settings, not changed.
- The removal of a half-written file on a write error (step 3) has no test: no fault can be injected into `write_all`
  without a hook, and the brief gives this work `write_new` only.
- A writer killed in the middle (Q1). Linux and macOS, where `write_new` is unchanged.
- Readers in other processes that open with `fs::read` and do not retry see an error, not a partial file, while a new
  file is written; whether any caller's reader minds is checked only by the L1 tests.

## Amendment 1 (2026-11-27): a lint of the test file

Written after `write_new_f998` (W1 to W3, N3), TC4, H and P had run and before the mutations and L1 to L3; nothing
above is changed. The build of `write_new_f998.rs` warns that the fields `absent` and `other` of its `Counts` are never
read (they are read only by the derived `Debug` of the printed counts), which L2's clippy with `-D warnings` would
refuse. Correction: `#[allow(dead_code)]` on `Counts`, which changes no behaviour, no limit and no count. The runs already
made are not run again; MP1 to MP3 and L1 to L3 run on the corrected commit.

## Amendment 2 (2026-11-27): a diagnosis of L1's miss in `run_records_adr0734`

Written after L1 and L2 had run on c7e90e95 and before the run named here; nothing above is changed. L1's registered run
of `run_records_adr0734` failed `l1_l4_l6_launch_record_replay_and_rerun` at line 205, `argv[0]` of `launch.json`
against `solver()`, the path of `rcfd` beside `env!("CARGO_BIN_EXE_rcfd-runner")`. That miss stands as measured; L1 is
missed. The suspected cause is not `write_new`: `CARGO_TARGET_DIR` was spelled with `/` (`E:/projects/...`), so the
test's expected path mixes `/` and `\`, while the runner writes `current_exe()`, all `\`. Diagnosis, made once: the same
test at dc7bb9cb (before the remedy), from `git archive` into the scratch folder `hcopy`, built in `target-h` with the
same spelling of the target folder, with `--nocapture`. If it fails at the same line, the miss is of the set-up and not
of the remedy; if it passes, the miss is the remedy's. Either way L1 stays missed in this ADR.

## Addendum 1: results (2026-11-27)

Written after the runs; nothing above is changed. Release profile, `CARGO_TARGET_DIR` in the scratch folder,
`CARGO_BUILD_JOBS` 8. H's binaries were built at dc7bb9cb (`rcfd-mcp.exe` sha256 b95bcca8...), P's at fc3488b8
(05773487...). Each registered run was made once, in the registered order. Scratch logs are `run-<tag>.log`, the traces
`trace-<tag>.tsv`, and the analysis `analyse5.py`, which reproduces ADR 0833's counts (H 18, 141 to 237 ms; TC4 128
ms) from ADR 0833's files. Another agent's work kept the machine at 97 % to 100 % CPU before the load was added.

### Checks

| Id | Limit | Measured | Met |
|---|---|---|---|
| W1 | partial = 0; sharing violations ≥ 5 | partial 0, sharing violations 5691, full reads 32, absent 650 (CPU 97 %) | yes |
| N3 | partial ≥ 1 | partial 6, full 37, sharing violations 0 | yes |
| W2 | equal; no `.tmp` | passed | yes |
| W3 | `Exists`; unchanged | passed | yes |
| TC4 | pid seen; one FREE 0 to 720 ms after release | pid seen 277 ms after the hold began (MARK after 90 ms); one FREE, 43 ms after release; CPU 99 % to 100 % | yes |
| H | h ≥ 8 | **h = 0** (no SEEN line of any holder) among 398 files looked at; refusals 0 of 1350 (edit_case 0); 33.9 s; CPU 100 % to 100 % | **missed** |
| P | h(P) ≤ 0.1 × h(H), deciding only with H ≥ 8 | h = 0 among 382 files looked at; H missed its floor, so **the rule does not decide** | does not decide |
| A | 0 of P's 900 `edit_case` refused | **0 of 900** (creation 0 of 450); 31.9 s; CPU 100 % to 100 % | yes |
| L1 | pass | `rcfd-project` (`--features test-hooks`) lib 4, `edit_lock_f755` 7, `edit_lock_f855` 2 (1 ignored), `hammer` 3 (1 ignored), `records` 7, `replace_refused_f978` 1, `write_new_f998` 4; `rcfd-workspace` lib 18, `mesh_f272` 7, `new_case_f271` 4; `rcfd-runner` lib 36 (1 ignored); `rcfd-mcp` lib 18 (1 ignored), `edit_case_refused_f978` 5 ignored, `new_simulations_f271` 1 (1 ignored), `replace_hold_f978` 1 (10 ignored): all passed. **`run_records_adr0734`: 1 passed, 1 failed** (`l1_l4_l6_launch_record_replay_and_rerun`, line 205) | **missed** |
| L2 | clean | `cargo fmt -p rcfd-project -p rcfd-mcp --check` clean; clippy `-p rcfd-project -p rcfd-mcp --release --all-targets -D warnings` clean | yes |
| L3 | pass | in addendum 2 | see there |

**H's miss**: the trace saw no hold at all on the unchanged path, against ADR 0833's 18 in 300 rounds a day earlier
(P(h ≤ 7) was 5.2 × 10^-6 at that rate). It has no diagnosed cause. Like ADR 0833's refusal rate (15 to 0), the hold
rate changed between runs of the same arm on this machine; the machine was at 100 % CPU from other work before the
arm's load, while ADR 0833's arms ran from 76 % to 90 %. H, like ADR 0833's arms, asserts nothing: its limit is in the
analysis, so no `*_known_miss` test carries it, and the miss is held here with its number.

**L1's miss**, diagnosed by amendment 2: the run at dc7bb9cb (before the remedy) fails at the same line with left
`E:\projects\cfd\w49i-work\f998-plain-write-scratch\target-h\release\rcfd.exe` and right
`E:/projects/cfd/w49i-work/f998-plain-write-scratch/target-h\release\rcfd.exe`. The test compares path strings, and
the `/` spelling of `CARGO_TARGET_DIR` breaks the comparison; the miss comes from the set-up, not from `write_new`. It stays a miss
here: L1 was not run again.

### Mutations

| Id | Mutation | Caught by | Result |
|---|---|---|---|
| MP1 | `write_new` without `share_mode(0)` (committed patch `crates/rcfd-project/tests/write_new_f998.mutation.patch`) | W1 | **caught**: "W1: partial reads: Counts { full: 55, partial: 10, sharing: 0, ... }", exit 101 |
| MP2 | all bytes but the last | W2 | **caught**: "W2: bytes of 1", exit 101 |
| MP3 | `create(true).truncate(true)` for `create_new(true)` | W3 | **caught**: W3 panicked at line 141 (no error where `Exists` was due), exit 101 |
| N1 | H, the unchanged path, held | the rule | **not held** (h = 0): see H |
| N2 | TC4 | the trace | seen |
| N3 | a shared plain write | W1's set-up | partial 6 |

Each mutation changed exactly one line (`git diff --numstat` 1 + 1) of a scratch copy of c7e90e95.

Deviations: amendment 1 (a lint of the test file) and amendment 2 (the diagnosis run). Not run: nothing registered was
left out. Actual size: partial; 1 further registration; 2 heavy runs (H, P); 2 questions; about 85 000 bytes read.

Decision by the registered rule: **partial**. Every mutation was caught, and W1 to W3, N3, TC4 and A held (0 refusals of
900 `edit_case` under P). But the control H gave 0 holds against its floor of 8, so the rule that compares P with H
does not decide, and L1 missed in `run_records_adr0734` by a cause of the set-up.

## Open questions for the maintainer

- **Q1. The guarantee P gives up** (a writer killed in the middle leaves a partial file under the final name; the old
  path left at most a `.tmp` file). (a) Keep P for every caller of `write_new`; (b) use the plain write for new case
  files only (`rcfd-workspace` `cases.rs`, the files `edit_case` replaces), and keep the temp file and hard link for the
  immutable records (`run.json`, `case.snapshot`, the launch and trash records), which are never replaced and so cannot
  be refused by a hold; (c) revert P. Recommendation: (b), since the holds matter only for files edited after they are
  created; not taken here, since it changes callers outside `write_new`, which is scope.
- **Q2. The measure** (scope): on this day neither the refusals (ADR 0833) nor the holds (here) of the unchanged path
  came, so neither reads P against H. (a) Repeat H and P under F-998 (rule 13), interleaved in blocks of 50 rounds,
  2000 rounds each, the floor and fraction kept from this ADR; (b) take F-998 as met on A (0 of 900 under P) with the
  cause unread, which changes the acceptance's reading; (c) stop measuring and keep P on its fast checks.
  Recommendation: (a), since an interleaved run sees H and P under the same machine state; not taken here, since it is
  scope.

### For the lead: roadmap entries

> Status: partial.

> Note: ADR 0834, 2026-11-27, partial: write_new on Windows became a plain write (create_new with share mode 0, written and synced, no temp file or hard link), its fast checks and three mutations held and 0 of 900 edit_case were refused under arm H's load, but the unchanged control H showed no System hold (0 against a floor of 8), so the hold count did not compare the two paths.

> Gap: Missing: a registered comparison of the System process's holds of a new case file between the old write_new (temp file and hard link) and the plain write of ADR 0834. ADR 0833 saw 18 holds in 300 rounds on the old path; ADR 0834's control saw 0 in 450, so its rule did not decide, although the plain write gave 0 refusals of 900 edit_case. Also open: which callers keep the old path (ADR 0834 Q1).

> Follow-ups: one under F-998 (rule 13), no new item: "F-998: the holds of the old and the plain write_new, interleaved", H and P of ADR 0834 in alternating blocks of 50 rounds, 2000 rounds each, with ADR 0834's floor and fraction; its other limits are registered in its ADR.

The same entries are `docs/roadmap/proposals/0834-F-998.json`.

## Addendum 2 (2026-11-27): L3

Written after that run, on 53013224; nothing above is changed. `docs_site` 2 passed; `roadmap_proposals.py check` of
`docs/roadmap/proposals/0834-F-998.json` passed (partial to partial, gap set); `process_report.py --check` gave 1
failure, `docs/adr/index.md` not current, a generated file left to the lead (rule 9): **met** as registered.
