# ADR 0644: F-705 registration: the design store header after a torn first write

Status: **REGISTERED 2026-11-16, written before any code of F-705 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-705 was written and no test was run in the branch. The diagnosis
below is from reading the source only (no diagnostic run). Results go in the addenda at the end; nothing above the addenda is
changed after a run.

Roadmap item F-705 (deps F-268), proposed by [ADR 0623](0623-f268-remaining-ledger-logging-and-reduced-order-bands.md) open
question R3. Parents: [ADR 0067](0067-wave32c-design-loop.md) (the evaluation store), [ADR 0121](0121-wave46d-design-ledger-and-tolerance-study-driver.md)
(truncation in place of a torn last line), ADR 0623 (the test-only kill hook of `Study` and the LG5 kill test). Number 0644
given by the lead. Branch `f705-work` from local main d40cd665, worktree `w49i-work/f705`, scratch `w49i-work/f705-scratch`.

House rules (`w49i-work/AGENT_RULES.md`) apply: limits fixed here; each registered run made once; existing tests not edited;
defaults bit-identical; no edit of `baseline_hashes.rs`, of a hash table or of a golden; no new dependency, no unsafe code;
clean-room; the roadmap files are not edited. Every choice the maintainer would normally be asked about is taken on the
maintainer's standing instruction of 2026-11-11 (take the recommendation), except where listed as open.

Clean-room: the fix is a property of an append-only line file (first principles, below). No source outside the repository was
read; no vendor code, text or install was used.

## What F-705 asks

The acceptance, binding:

> A study store whose first append is killed while writing the header line reopens and resumes with the header present at every kill point inside the first append, and every existing store test keeps its bytes. Every limit not stated here is registered in the ADR before the run.

Clauses: **a1** at every kill point inside the first append the store reopens, the study resumes, and the header is present;
**a2** every existing store test keeps its bytes (the stores the existing code writes are unchanged, and the existing tests pass
unedited).

## What was read

`AGENT_RULES.md`; the brief; ADR 0623 ("What exists", LG5, R3, roadmap entries); `crates/rcfd-api/src/design/store.rs`
(`Store::open_impl`, `Store::append`, its two unit tests); `study.rs` (`Study`, the `#[cfg(test)]` hook `test_kill_at`,
`test_events`, `test_killed`, `test_event`, `eval_points` lines 545-565: the store event is counted before `Store::append`, the
ledger event before `Ledger::append`); `study_ledger.rs` and `study_ledger_tests.rs` (LG study, `nm()`, `sink`, `ledger`,
`lg5_resume_after_a_kill_at_every_write_event`, which skips the torn kill point at event 1); `ledger.rs` (entries carry their
own `format` member; the ledger has no header line); `tools.rs` line 701 (the MCP tools open `evaluations.jsonl` through the
same `Store::open`); `docs/design-loop.md` (the store section).

## Diagnosis (from the source, no run)

`Store::append` writes the header only when `fresh = !p.exists()`, evaluated before the file is opened with `create(true)`,
and writes header and records of the batch with one `write_all`. A kill inside that first `write_all` leaves the file holding a
byte prefix `T[..p]` of the first append's text `T = H + R` (`H` the header line with its newline, `R` the records of the first
batch), for some `p` in `0..=|T|`: `p = 0` is a kill after the file was created and before any byte reached it.

- `p = 0`: `Store::open` reads an empty text (`complete`), keeps the file; the resumed `append` sees an existing file
  (`fresh = false`) and writes records without a header.
- `0 < p < |H|` (header torn, including `p = |H| - 1`, the header complete without its newline): the last line is incomplete,
  `Store::open` truncates the file to the last newline, which is offset 0; then as for `p = 0`.
- `|H| <= p <= |T|`: the header is complete; a torn record is truncated to its last complete line; the resumed `append` is
  `fresh = false` and correctly writes no header.

In the first two cases the resumed run itself succeeds (the records are appended), but the file's first line is a record, so
every later `Store::open` refuses it ("not a design store (missing header line)"). The number of failing kill points is
therefore `|H|` exactly. For the LG5 study (signature `lg`) the header is
`{"format":"rcfd-design-store","signature":"lg","version":1}` (serde_json's map orders the keys; the length does not depend on
the order): 59 bytes plus the newline, `|H| = 60`. LG5 (ii) at event 1 appends the fragment `{"format":"rcfd-` to an absent
file, which is the case `p = 16`; ADR 0623 left it out.

The cause is that "fresh" is decided by the existence of the file, while the invariant the reader needs is "the first line is
the header", which is a property of the file's contents: a file with no complete line holds no header whether it exists or not.

## Design

D1. In `Store::append`, the header is written when the opened file is empty: `fresh` is the length of the open append handle
(`File::metadata().len() == 0`), read after the open and before the write, in place of `!p.exists()` before the open.
Derivation: after `Store::open` with repair, a file holds either nothing or a sequence of complete lines; a non-empty repaired
file of a store opened successfully starts with a complete header line (line 0 was checked). So "length 0" is exactly "no header
present", and writing the header then restores the invariant, for every `p < |H|`. For `p >= |H|` the length is at least `|H|`
and nothing changes. For an absent file the handle opened with `create(true)` has length 0, so the absent-file case writes the
same header as before: every store the existing code writes successfully is written with the same bytes (a2). The length is read
from the handle so that the decision is about the file that is written (no gap between the check and the open).

D2. Nothing else changes: `Store::open`, `open_read`, the header text, the record text, the hook and the ledger. A store
opened with `open_read` (no repair) on a torn header is not appended to by the library (read-only callers), so D1 is not needed
there. The ledger has no header line (every entry carries `format`), so the defect has no ledger counterpart.

Rejected: removing the file in `Store::open` when the repair truncates it to 0 bytes. It does not cover `p = 0` (an empty file,
nothing to repair) and deletes in place of truncating, against ADR 0121's choice.

Default bits: a store whose file exists and is empty was previously written without a header and could not be reopened; it now
gets the header. This is the only change of written bytes, and it only touches files that were unreadable before.

## Set-ups

- **LG5 study**: the LG study of ADR 0623 with `Method::NelderMead { budget: 16, restarts: 0, seed: 623_005 }`, workers 1, file
  store (signature `lg`) and file ledger with the sink (clock fixed), as in `lg5_resume_after_a_kill_at_every_write_event`. The
  helpers of `study_ledger_tests.rs` are private, so the new test module carries its own copies (same definitions).
- **T**: the first append's text, read from a run killed at write event 2 (the ledger write after the first store write); the
  test asserts that event 2 is the ledger event and that the store file then has exactly the records of one batch plus the
  header. `|T|` is read from that run, not chosen.
- **Kill inside the first append at byte p**: a run with `test_kill_at = Some(1)` (the store event: no byte written, the store
  file absent), then the store file is created and `T[..p]` written to it (for `p = 0` an empty file is created), for every `p`
  in `0..=|T|`; then a new `Study` over `Store::open` of the file, `set_ledger_sink`, the method run to the end.
- **Reference**: the uninterrupted LG5 run (store bytes `B`, sorted ledger entry lines).

## Checks (limits fixed now)

| Check | Clause | What | Limit |
|---|---|---|---|
| TH1 | a1 | LG5 study killed inside the first append at every `p` in `0..=|T|`, then resumed | at every point: `Store::open` after the kill succeeds, the resumed run ends `Ok`; the resumed store file's first line is the header (`format` `rcfd-design-store`, `signature` `lg`, `version` 1); the store file bytes equal `B`; a second `Store::open` of it succeeds with the reference's records; ledger 0 duplicate, 0 missing, 0 extra (store key, condition) pairs, entry set equal to the reference, 0 skipped lines. Number of points `|T| + 1`; 0 failing points |
| TH2 | a1 | store level: `Store::open` + `append([rec 0])` on a file holding the prefix `H'[..p]` of the header `H'` of signature `sig`, for every `p` in `0..|H'|`, and on an absent file | every file's bytes equal the absent-file case's bytes; `Store::open` of each succeeds with 1 record |
| SB1 | a2 | FNV-1a 64 digests of the store files of: S1 the scenario of the existing `round_trip_is_exact_and_partial_last_line_is_dropped` (open, 5 records, torn last record, reopen, append record 4); S2 the LG study (`Doe::Lhs { n: 32, seed: 623_001 }`) workers 1 with the sink, its store and its ledger; S3 the uninterrupted LG5 store `B`; S4 the LG5 store resumed after the kill before write event 3 | digests after D1 equal to the digests measured on the unchanged library (procedure below) for all 5 files |
| DF1 | a2 | existing tests | no existing test, test file body, golden or hash table edited (git diff: `store.rs` gains D1 and a `mod` line, the new test file is new); passing in release: every `design::` unit test of rcfd-api (`store`, `study`, `study_ledger`, `ledger`, `uq`, `sensitivity`, `tools`, ...), the rcfd-api integration tests that open a store (`design_e2e_w32c`, `f249_default_bits`, `f249_moo`, `f260_cmaes`, `f297_default_bits`, `f297_moo`, `f697_fan_objective`, `fan_rom_design_w46b`, `w35b_steady_guard`), and the rcfd-mcp tests (tools-list goldens); LG5 keeps its 55 points and 0 failures |

SB1 procedure (registered): the test module is committed with D1 not yet applied; SB1 is run once on that unchanged library,
printing the five digests; the digests are then written into the test as constants in the same commit as D1, and SB1 runs once
more after D1 as the registered check. TH1 and TH2 are run once on the unchanged library as negative control NC1 and once after
D1 as the registered checks.

`baseline_hashes`: `store.rs` is not a solver file and the baseline cases write no design store (to be confirmed by grep in the
addendum); `cargo test -p rcfd-api --release --test baseline_hashes` is run once anyway if the shared machine allows.

Run time: TH1 is about `|T| + 1` short studies (estimate `|T|` about 1000 to 1500 bytes for a first Nelder-Mead batch of 3
records of 2 conditions); if it exceeds about two minutes in release it becomes an ignored heavy record run once, with a fast
test over `p` in `0..=|H|` plus `p = |T|` covering the same path.

## Negative control and mutations

**NC1** (unchanged library): TH1 fails at exactly the `|H| = 60` points `p = 0..=59` and at no other point; TH2 fails at every
`p` in `0..|H'|`. Predicted from the diagnosis; a different count is reported as a diagnosis error.

Mutations (scratch copy of the fixed tree; each must fail a check): **M1** the header written on every append (`fresh = true`)
-> TH1 (bytes), SB1, existing round-trip test; **M2** `Store::open` not truncating a torn last line (repair off) -> TH1 (a torn
header stays and the records follow the fragment on its line); **M3** the header written without its newline on the empty-file
path -> TH1, TH2; **M4** `fresh` read from the path's existence again, but after the open (`p.exists()` after `create(true)`,
always true) -> TH1, TH2, SB1 (no header on a new store).

## Decision rule

**done** when TH1, TH2, SB1 and DF1 are met, NC1 holds as predicted and every mutation fails a check; **partial** when DF1 and
SB1 hold but TH1 or TH2 misses (an ignored `*_known_miss` record); **todo** (the lead does not take the code) when DF1 or SB1
fails.

## Limits (not covered)

- The kill is simulated: the hook stops before the first store write and the test writes a byte prefix of the first append; a
  real killed process, a write reordered by the file system or a crash that loses data already written (no `fsync`) are not
  covered.
- Only the first append is covered by TH1; kills in later appends are LG5's (one fragment per event).
- `Store::open_read` on a torn header keeps returning an empty store without repairing (unchanged).
- Two processes appending to one store are not covered (unchanged).

## Addendum 1: results (2026-11-16)

Commits: 70c8a98b (this registration, alone), cb329680 (the test module `store_header_tests.rs` with the `mod` line in
`store.rs`, library unchanged, SB1 digests zero), 7b2a57c0 (D1, the SB1 digests measured on cb329680, TH1 split into the heavy
record and a fast test). Logs in `w49i-work/f705-scratch`: `nc1_sb1_unfixed.log` (NC1 and the SB1 reference, on cb329680),
`checks_fixed.log` (TH1, TH1 fast, TH2, SB1 on 7b2a57c0, `--include-ignored`), `df1_lib.log`, `df1_int.log`, `df1_mcp2.log`,
`baseline.log`, `clippy.log`, `mut_M1.log` to `mut_M4.log`. All in release, `--test-threads=1` for the store module.

| Check | Limit | Measured | Met |
|---|---|---|---|
| TH1 (a1) | all `|T| + 1` points: open, resume `Ok`, header first, bytes = `B`, reopen with the reference's records, ledger 0 dup / 0 missing / 0 extra / same set / 0 skipped | `|T|` = 1353 bytes (header 60 + 3 records), 1354 points, 0 failing | yes |
| TH1 fast | same at `p` in `0..=60` and `p = 1353` | 62 points, 0 failing | yes |
| TH2 (a1) | every `p` in `0..|H'|` bytes equal to the absent-file case, 1 record on reopen | `|H'|` = 61 (signature `sig`), 0 failing | yes |
| SB1 (a2) | 5 digests equal to the unchanged library's | S1 0x243132928430f7ff, S2 store 0xd04629e2d4b89b60, S2 ledger 0x62d3f21775a730f4, S3 and S4 0x57a502a8837e6f5f, all equal | yes |
| DF1 (a2) | no existing test, golden or hash table edited; listed tests pass | `git diff d40cd665`: `store.rs` (D1: line 291 removed, lines 299-305 added; the `mod` lines 382-385), the new test file, this ADR, `docs/design-loop.md`; `design::` unit tests 79 passed, 4 ignored (heavy records, TH1 among them), 0 failed; LG5 28 events, 55 points, 0 failing; the 9 integration tests 81 passed, 0 failed; rcfd-mcp 89 passed, 0 failed (goldens included) | yes |
| NC1 | TH1 fails at exactly `p = 0..=59`; TH2 at every `p` | on cb329680: TH1 60 failing, `p = 0..=59` ("no header; bytes differ (6895 vs 6955); reopen: ... not a design store"); TH2 61 of 61 failing | yes, as predicted |

Mutations (scratch copy `f705-scratch/mut` of 7b2a57c0, `cargo test --lib design::store`, TH1 heavy not run):

| Mutation | Caught by |
|---|---|
| M1 header on every append | TH1 fast, SB1, existing `round_trip_is_exact_and_partial_last_line_is_dropped` (TH2 passes: one header either way) |
| M2 `Store::open` without repair | TH1 fast, TH2, SB1, existing round-trip test |
| M3 header without newline | TH1 fast, TH2, SB1, both existing store tests |
| M4 `fresh = !p.exists()` after the open | TH1 fast, TH2, SB1, both existing store tests |

Four of four caught. `cargo fmt -p rcfd-api --check` clean; `cargo clippy -p rcfd-api --all-targets -- -D warnings` clean.
`baseline_hashes` (no design store in it, grep): 3 passed, 1 ignored.

Deviations, disclosed:

- TH1 took most of the 140 s of the unfixed run, so it became the ignored heavy record with the fast test registered for that
  case; it was run once after D1 with `--include-ignored`. The SB1 constants were written after the run on cb329680, as
  registered.
- The first rcfd-mcp run stopped at `adopt_f288` because the `rcfd` and `rcfd-runner` binaries were not built in the scratch
  target (a set-up fault, not a result); after `cargo build --release -p rcfd-cli -p rcfd-runner` the whole crate was run once
  with `--no-fail-fast`, all passed.
- The test module hangs under `store.rs` (as registered) and copies the LG helpers of `study_ledger_tests.rs`, which are private.

Not run: the whole workspace suite (the lead's gate).

Decision by the registered rule: TH1, TH2, SB1 and DF1 met, NC1 as predicted, four of four mutations caught: **done**.

Remaining questions:

- Q1, LG5 (ii) at event 1. ADR 0623's LG5 still skips that point (the existing test is not edited). Options: (a) leave it,
  TH1 covers it with every byte; (b) a new LG5 variant without the skip. Recommendation (a): TH1's `p = 16` is exactly that
  fragment.
- Q2, durability. The store neither `fsync`s nor survives a power loss that drops written bytes; a kill of the process is what is
  covered. Options: (a) leave; (b) `sync_data` after each append (slower on a study of many small batches). Recommendation (a)
  until a study is reported lost to a power cut; not taken here because it changes timing of every study.

## For the lead: roadmap entries

> Proposed status: done.

> Design loop. ADR 0644, 2026-11-16: the design store header after a torn first write. Cause (diagnosed and registered before the fix): Store::append wrote the header only when the file did not exist, so a kill inside the first append left an empty file, or a torn header that Store::open truncates to 0 bytes, and the next append wrote records without a header, after which the store could no longer be opened. Fix: the header is written when the opened file is empty (length of the append handle). The LG5 Nelder-Mead study with the ledger sink killed at every byte of its first append (1354 points, |T| = 1353 bytes, header 60) reopens, resumes to the uninterrupted store's bytes with the header present and the ledger without duplicate or missing pairs at every point; on the unchanged library exactly the 60 header points failed. Store bytes of the existing paths unchanged (5 FNV digests measured before the fix), every existing store, study, ledger, design integration and rcfd-mcp test passes unedited, baseline hashes pass. Four of four mutations caught. Not covered: a real killed process, a power loss without fsync, two writers.

> F-268 note: append "The kill inside the first store write, left out of LG5, is covered at every byte of the first append by ADR 0644 (F-705)."
