# ADR 0832: F-978 registration: how long a refused case file stays held, and the replace budget it calls for

Status: **REGISTERED 2026-11-27, written before any code of F-978 in this work and before any run of its checks.** Before
this text nothing was changed, built or run but reading. Results go in addenda at the end; nothing above them is changed
afterwards. Written by Claude Opus 5.5 (`claude-opus-5-5`), as a background agent of the lead.

It answers the brief `docs/adr/briefs/f978-replace-hold.md` ([the file](briefs/f978-replace-hold.md)) and follows
[docs/process/agent-rules.md](../process/agent-rules.md), version 5. Branch `f978-replace-hold` from 247a77c3, scratch
`w49i-work/f978-replace-hold-scratch`. It is registered under F-978 (rule 13) and is the follow-up (a) of
[ADR 0830](0830-f978-registration-the-refused-edit-case.md) Q1, as the lead answered it.

## What was read

`AGENTS.md`, the rules (version 5), the brief, ADR 0830 and `crates/rcfd-project/src/atomic.rs`, whole;
`crates/rcfd-mcp/tests/edit_case_refused_f978.rs` and `crates/rcfd-project/tests/replace_refused_f978.rs` whole; the
heads of `canvas_process_f978.rs`, `edit_lock_f855.rs` and the two `tests/common/mod.rs`; ADR 0830's scratch tools
`trace.ps1` (X''), `run_arm.sh`, `analyse.py`, `final.sh` and its logs of arm B; F-978's acceptance (by a script); the
callers of `replace_if_hash_with` (S1: `grep -rn "replace_if_hash_with(" crates --include=*.rs`): `rcfd-workspace`
`write.rs` 160 and 180 (`edit_case`), `save.rs` 138, `rcfd-runner` `config.rs` 270. Bytes read: about 95 000.

Arithmetic beforehand. DEFAULT_RETRY sleeps 1 + 2 + 4 + ... + 64 + 4 × 100 = 527 ms in 12 attempts; ADR 0830's
refusals took 587 to 619 ms from call to answer. ADR 0830's F1, `EDIT_REPLACE_RETRY` = 27 attempts, 1 ms doubling to 100
ms, sleeps 127 + 19 × 100 = 2027 ms. ADR 0830 saw 5 refusals in 600 replaces (A3 and B), 0.83 %; 900 replaces give an
expected 7.5 refusals and none with a chance of e^-7.5 = 0.06 %. One look of the trace takes about 71 ms (ADR 0830
amendment 2).

## Design

**X3, the trace.** ADR 0830's X'' (`trace.ps1`: priority High, `NtQueryInformationFile` class 47, a line whenever a
file's set of holders changes, its own pid left out), copied to the scratch folder as `trace3.ps1` with one change: it
also follows **marked** files. A test that sees a refusal writes, at once, a marker `mark-<i>-<op>.txt` beside the project
folder (in `%TEMP%\rcfd-mcp-w48f-f978-*`, outside the project) holding the file's name. On every loop X3 lists the
markers; a new marker adds its file to the files it looks at and writes `MARK <ms> <file> <marker>`; a marked file is
looked at on every loop until a look shows it **free**, that is held by no process whose name does not start with `rcfd`
(the replacing `rcfd-mcp` server), and then X3 writes `FREE <ms> <file> <marker>` and stops following it. A file seen
held by a process not named `rcfd*` is followed in the same way until free, marked or not. Everything else is X''. Look
interval: one loop looks at the newest `arm-<i>.toml`, `process-cavity.toml` where it exists and the followed files, one
look each (about 71 ms), then sleeps 2 ms.

**TC3, its control** (a test, ignored, run beside X3): a project folder with `cases/arm-0.toml` and `cases/arm-1.toml`
(the newest); the test opens `arm-0.toml` (std, a hold of the test's pid), writes a marker for it, holds it 800 ms,
releases it and prints the release time. X3 must see the test's pid on `arm-0.toml`, and its `FREE` line must come within
300 ms after the release (the end of a hold is placed to within one loop of about 3 looks, 3 × 71 + 2 = 215 ms).

**H, the arm of the hold.** A new ignored test, `crates/rcfd-mcp/tests/replace_hold_f978.rs`, that is arm B of ADR 0830
(without the `--canvas` server, the load L of 16 spinning threads, `new_case` then the two `edit_case` of B1 per round)
with **300 rounds** (900 replaces) and the marker written on each refusal; CPU recorded before and after; X3 beside it.

**The measure.** For each refusal, from the arm's line (`t0` the call's start, `t1` its answer) and X3's lines on that
file: **N = FREE − t0**, the time from the call's start until the file is seen free, an upper bound of the time a replace
needed to outlast the hold; also the holders X3 saw from `t0` to `FREE`, by process. Reported: every N, its largest value
N_max, the sorted list, the holders. An analysis script in the scratch folder (`analyse3.py`) computes them.

**The rule for F1.** F1 is ADR 0830's F1 exactly: `EDIT_REPLACE_RETRY: Retry` = 27 attempts, 1 ms doubling to 100 ms
(2027 ms of sleep), used by `replace_if_hash_with` for its replace instead of `DEFAULT_RETRY`. It is made if H gives at
least one refusal, every refusal has a `FREE` line, and **1.5 × N_max ≤ 2027 ms** (N_max ≤ 1351 ms; the margin 1.5 lets
a later hold exceed the largest seen by half, on a bound N that is already an upper bound). Otherwise F1 is not made and
N_max is reported (a hold of 2 s or more, or between 1.35 s and 2 s, is a question for the maintainer). The change is
`atomic.rs` only: a replace that succeeds writes the same bytes and journal line as before; a refusal comes after 27
attempts instead of 12. `DEFAULT_RETRY` and its other users (`atomic_write`, `read`, the locks, the claims) are
unchanged; the callers (`edit_case`, `save`, the runner's config) change no code.

**B1 to B3** once, `canvas_process_f978` as ADR 0826 registered it, with X3 beside it, CPU recorded, on the code as it
stands after F1's rule (with F1 if made).

## Checks (rule 14)

Release profile, `CARGO_TARGET_DIR` under the scratch folder, `CARGO_BUILD_JOBS` 8, `RCFD_BIN` from that target. New test
files: `crates/rcfd-mcp/tests/replace_hold_f978.rs` (TC3, H, H2; ignored, run once each) and, if F1 is made,
`crates/rcfd-project/tests/replace_hold_f978.rs` (U0, U2; Windows and `--features test-hooks`, for
`test_hooks::hold_without_delete_sharing`).

| Id | Check | Limit | Source | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|
| TC3 | X3 sees the test's 800 ms hold of a marked, not newest file | the test's pid seen on `arm-0.toml`; FREE − release in 0 to 300 ms | the brief (a control of a registered length) | seen; 100 ms | 3 | code verification |
| H | the arm of the hold: refusals and N for each | at least 1 refusal, each with a FREE line (else the hold is not measured) | the brief (the hold's length after a refusal) | about 7 refusals | rule | code verification |
| R | the rule for F1 | 1.5 × N_max ≤ 2027 ms for F1 | the brief (F1 under 2 s with a registered margin) | N_max about 800 ms | 1.7 | code verification |
| U0 | F1: the target held without delete sharing for 3.0 s through `replace_if_hash` | `ProjectError::Retry`, 27 attempts, os error 5, the file's old bytes, at least 2.0 s spent | ADR 0830 U0; F1 | as limit | exact | code verification |
| U2 | F1: the same hold released after 1.0 s | Ok, the new bytes, at least 0.9 s spent, `old_hash` and `new_hash` of the two contents; a reader every 5 ms in the wait reads the old or the new bytes whole, nothing else | ADR 0830 U2; the brief (bytes and atomicity unchanged) | Ok | 2.03 | code verification |
| H2 | H again after F1, same load and rounds, X3 beside it | 0 refusals of 900 | the brief (refuses none in the registered rounds) | 0 | exact | code verification |
| B1 | `canvas_process_f978` once, X3 beside it, CPU recorded | every assertion of B1 | ADR 0826 B1 | holds | exact | code verification |
| B2 | the same run | as ADR 0826 B2 | ADR 0826 B2 | holds | exact | code verification |
| B3 | the same run | as ADR 0826 B3 | ADR 0826 B3 | holds | exact | unchanged default output |
| T3 | `rcfd-runner` processes of this work's target alive after the B1 run | 0 | ADR 0828 T3 | 0 | exact | code verification |
| L1 | `--lib` of `rcfd-project`, `rcfd-mcp`, `rcfd-workspace`; `rcfd-project` `hammer`, `edit_lock_f755`, `edit_lock_f855` and `replace_refused_f978` (the last two with `test-hooks` as their files say) | pass | rule 7 | pass | exact | unchanged default output |
| L2 | `cargo fmt -p <crate> --check` and `cargo clippy --release --all-targets -- -D warnings` of `rcfd-project` and `rcfd-mcp` | clean | rule 7 | clean | exact | unchanged default output |
| L3 | `docs_site`; `roadmap_proposals.py check` of the proposal; `process_report.py --check` | pass (the last: only a generated file not current may fail) | rules 7, 9 | pass | exact | unchanged default output |

If F1 is not made, U0, U2 and H2 are not run and `rcfd-workspace --lib` is left out (no crate but `rcfd-mcp`'s tests
changed). A refusal by the operating system in B1 is recorded as it comes and not run again. Order: TC3, H, (F1, U0, U2,
H2), B1, mutations, L1 to L3.

### Negative controls and mutations (rule 16)

- **N1** is U0: a hold that F1 cannot outlast is still refused with os error 5.
- **M1** (committed as `crates/rcfd-project/tests/replace_hold_f978.mutation.patch`, if F1 is made):
  `replace_if_hash_with` back to `atomic_write` (DEFAULT_RETRY). Must fail: U2. First line `# expect-fail: cargo test
  --release -p rcfd-project --features test-hooks --test replace_hold_f978 --target-dir target/mutation`.
- **M2** (scratch, if F1 is made): `EDIT_REPLACE_RETRY.attempts` 27 → 26. Must fail: U0 (26 attempts).
- **M3** (scratch): X3 without the following of markers. Must fail: TC3 (the test's pid on `arm-0.toml` not seen).

Mutations run on a scratch copy of the final commit with their own target folder.

## Size

Size (agent): done; 0; 3; 0; 95000

(Expected done; no further registration; three heavy runs: H, H2 and B1; no question expected; about 95 000 bytes read.)

## Decision rule

**done** for F-978 if the hold is measured (H with at least one refusal, each with a FREE line, and TC3 met), F1 is made
by its rule and U0, U2 and H2 hold, B1 to B3 hold in their one run and each mutation is caught; **partial** otherwise,
naming what failed. A missed limit is recorded with its number and held as a `*_known_miss` record (rule 2).

## Not covered

- What the System process does with the file (which driver holds the kernel-mode handle); Defender's settings.
- The other users of `DEFAULT_RETRY`, which F1 leaves as they are; Linux and macOS, where an open file does not block a
  rename.
- A hold shorter than one loop of X3 that ends before the first look after a refusal: it is seen as free at once, and N
  is still its upper bound.

## Addendum 1: results (2026-11-27)

Written after the runs; nothing above is changed. Release profile, `CARGO_TARGET_DIR` in the scratch folder,
`CARGO_BUILD_JOBS` 8. Each registered run was made once, in the registered order; the scratch logs are `run-<tag>.log`
and `trace-<tag>.tsv`, analysed by `analyse3.py`. Another agent's solves kept the machine at 90 % to 100 % CPU.

### Checks

| Id | Limit | Measured | Met |
|---|---|---|---|
| TC3 | the test's pid seen on `arm-0.toml`; FREE − release 0 to 300 ms | pid seen (354 ms after the hold began; MARK picked up after 188 ms); **FREE − release 479 ms**; CPU 100 % to 100 % | **missed** (the second clause) |
| H | at least 1 refusal, each with a FREE line | **15 refusals of 900** (11 files: 7 refused at edit1 only, 4 at edit1 and edit2), each after the 12-attempt budget (557 to 613 ms); CPU 90 % to 96 %, 28.99 s; **8 refusals without a FREE line** | **missed** (the second clause) |
| R | 1.5 × N_max ≤ 2027 ms | the 7 with FREE: N = 1762, 1764, 1776, 1789, 1885, 1992, 2112 ms; the other 4 files were still held by `4:System` at X3's last look of the folder: N ≥ 3296, 8114, 9854 and 18810 ms (holds seen ≥ 2458, 7528, 9384, 18238 ms); so N_max ≥ 18810 ms | **missed**: F1 not made |
| U0, U2, H2 | F1's | not run: F1 not made | not run |
| B1 | every assertion | held: run finished, 10 steps, journal seq 1 to 5 against 1 to 5; CPU 100 % to 99 %, 7.57 s; X3 saw `4:System` holding `process-cavity.toml` (SEEN) without a refusal | **yes** |
| B2 | as ADR 0826 | refused at `panels[0].kind` (1003) | **yes** |
| B3 | as ADR 0826 | the description names `process`; the plain `read_view` has no `process` | **yes** |
| T3 | 0 | 0 `rcfd-runner` of this work's target alive after B1 | yes |
| L1, L2, L3 | pass, clean | in the last run, reported with the commit | see the report |

Every hold seen during or after a refusal, in H and B1, was the System process (`4:System`, a kernel-mode handle); no
other holder was seen on any case file. The holds split in two: 7 ended 1.16 to 1.51 s after the refusal (N 1.76 to
2.11 s, already more than 2 s at the largest), and 4 lasted at least 2.5 s to 18.2 s, until the folder was removed;
those 4 are exactly the files whose edit2 was refused too. A longer replace budget under 2 s would not have outlasted
any of the 7 with margin, nor any of the 4 at all.

**TC3's miss, diagnosed** (a timing in `tooltime3.ps1`, not a check, after TC3 and before H): one look took 76.5 ms and
the loop's `Sleep(2)` 15.7 ms (the timer's resolution) at 85 % CPU, so a loop of two or three looks is about 170 to 245
ms, and at 100 % CPU X3 was later; the limit of 300 ms assumed 215 ms. N is an upper bound by up to about 0.5 s; the
lower bounds of the 4 long holds do not depend on it. The miss is held here with its number: TC3's limit is in the
scratch analysis, not in an assertion of the test, so no `*_known_miss` test can carry it (deviation).

### Mutations

| Id | Mutation | Caught by | Result |
|---|---|---|---|
| M3 | X3 without the following of markers (`trace3-m3.ps1`), one run beside TC3 | TC3 | **caught**: the test's pid not seen on `arm-0.toml`, no MARK, no FREE |
| M1 | `replace_if_hash_with` back to `DEFAULT_RETRY` | U2 | not made: it mutates F1, which was not made |
| M2 | `EDIT_REPLACE_RETRY.attempts` 27 → 26 | U0 | not made: as M1 |

No mutation patch is committed: the registered one (M1) mutates F1, which its rule did not allow; M3 mutates a scratch
tool, not a committed file.

Deviations: the diagnosis of TC3 above; nothing else. Not run: U0, U2, H2, M1, M2 (F1 not made), `rcfd-workspace
--lib` (left out by the registration when F1 is not made).

Actual size: partial; 1 further registration (below); 2 heavy runs (H and B1; TC3 and M3 are light); 2 questions; about
100 000 bytes read.

Decision by the registered rule: **partial**. B1 to B3 held in their one run, and M3 was caught. But TC3 missed its
second clause, 8 refusals have no FREE line, the hold is 2 s or more (N_max ≥ 18.8 s), so F1 was not made, and M1 and
M2 were not made.

## Open questions for the maintainer

- **Q1. A hold of 2 s or more** (the brief's question; scope): the System process holds a newly created case file for 1.2
  s to more than 18 s after a refused replace, under load. (a) A registration under F-978 that runs H with the project
  folder outside `%TEMP%` (the scratch folder on E:), and H with the case created by a plain write instead of the
  hard link of `write_new`, so that the folder or the creation path is shown to cause or not cause the hold; (b) a longer
  budget anyway (it would need more than 20 s, longer than `EDIT_LOCK_WAIT`, and blocks the lock that long); (c) an
  `edit_case` refusal that names the hold as transient so the client can retry, without changing the replace. Recommendation:
  (a), since neither budget nor retry is chosen well before the cause is known; not taken here, since it is a question of
  scope.
- **Q2. F-978's scope**: the acceptance of F-978 is the process view, which B1 to B3 have now held in two runs (ADR
  0830, ADR 0832). (a) Keep the refused replace under F-978 (rule 13) until it is decided; (b) move it to a new item of
  its own with the acceptance "0 refusals of 900 edits under arm H's load", and let the lead decide F-978 on its own
  acceptance. Recommendation: (b), since the hold is a property of the replace on Windows, not of the process view.

### For the lead: roadmap entries

> Status: partial.

> Note: ADR 0832, 2026-11-27, partial: B1 to B3 held in their one registered run (CPU 100 %); a trace that follows a refused case file showed the System process holding it in 15 of 15 refusals of 900 edits under load, 7 holds ending 1.2 to 1.5 s after the refusal and 4 lasting 2.5 s to more than 18 s, so the hold is 2 s or more and by its rule the longer replace budget F1 was not made.

> Gap: B1 to B3 hold (ADR 0830, ADR 0832). Missing: a remedy for the refused first edit_case under load. The System process holds a newly created case file 1.2 s to more than 18 s after the refusal (15 refusals of 900 edits, ADR 0832), longer than any replace budget under 2 s; no rcfd process holds it. A registration that runs the arm outside the temp folder and with the case created without a hard link, to show what causes the hold.

> Follow-ups: one under F-978 (rule 13), no new item unless the maintainer takes Q2 (b): "F-978: what makes the System process hold a new case file", arm H of ADR 0832 with the project outside the temp folder and with the case created by a plain write; its limits are registered in its ADR.

The same entries are `docs/roadmap/proposals/0832-F-978.json`.
