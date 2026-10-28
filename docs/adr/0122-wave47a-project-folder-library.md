# ADR 0122: Wave 47A, `rcfd-project`, the project folder library (Studio layer L1)

Status: **ACCEPTED 2026-10-26** (checks and limits registered before the registered runs). Written at main 48b30bc on branch
`w47a`, target `<target dir>`. House rules of ADRs 0110-0117: limits fixed before runs; a missed limit becomes an
ignored `*_known_miss` test with the limit intact; a defective set-up is corrected by an amendment before the corrected run; defaults of
existing code are bit-identical (this ADR adds a crate and touches no existing code); `crates/rcfd-api/tests/baseline_hashes.rs` is not
re-recorded. Parent: `docs/ui-restart-plan.md` sections 3, 4 and 8 (the project folder contract).

## Context

The Studio restart (plan section 3) puts the project folder at layer L1: every later layer (run service, time-series store, app shell,
Claude Code plugin) reads and writes it, so it is built first, headless, with no UI and no solver dependency. The folder layout is fixed
by the plan section 8 contract; this crate codes against it.

## Decisions

1. **New crate `crates/rcfd-project`** (workspace member). Dependencies: `serde`, `serde_json`, `thiserror` and `rcfd-core` (for
   `rcfd_core::hash::fnv64` only). No new third-party dependency. `rcfd-core` is the light core crate (serde, serde_json, thiserror), not
   a solver crate.
2. **Hash.** `case_snapshot_hash` and `mesh_hash` are `"fnv64:<16 hex>"` strings from the repository's FNV-1a 64
   (`rcfd_core::hash`). The workspace has no cryptographic hash, so none is added. FNV-1a detects accidental change, not tampering.
   Hashes are strings because JavaScript and Python clients read JSON numbers as doubles (53-bit mantissa).
3. **Records** (typed, serde, unknown fields ignored so the contract can grow additively):
   * `rcfd-project.json`: `schema_version`, `name`, `created`, `solver_version_created` (as the contract).
   * `run.json`: `id`, `case` (project-relative path), `case_snapshot_hash`, `mesh` and `mesh_hash` (optional), `solver_version`,
     `settings` (free JSON), `created` (RFC 3339 UTC with milliseconds). Written once with an atomic create that never replaces
     (temp file, fsync, hard link, remove temp).
   * `status.json`: `state` (`queued`, `running`, `paused`, `finished`, `failed`, `cancelled`), `step`, `time`, `pid` (null when not
     owned), `updated`. Replaced atomically.
   * `events.jsonl`: `{"t": ..., "kind": "state", "from": ..., "to": ...}`, `{"kind": "warning", "message"}`,
     `{"kind": "stop", "reason"}`.
   * `residuals.jsonl`: `step`, `time`, `residuals` and `monitors` (name to number). `metrics.json`: a flat map name to number.
   * Additive to the contract (reported to the lead): the `case` and `mesh` path fields of `run.json`, the event shapes, and the flat
     form of `metrics.json`. Nothing in the contract is changed.
4. **Run id** `r-<UTC yyyymmddThhmmss>-<4 hex>`. Ownership comes from `create_dir`, which is atomic and fails if the directory exists,
   across threads and processes; on a clash the suffix is probed linearly from a random start (std `RandomState`, OS-seeded, mixed with
   the clock), so a free suffix is always found while the second has one (65 536 per second). Ids sort by time to the second; the list
   orders by `created` (milliseconds), then id. UTC formatting uses the published civil-from-days algorithm of H. Hinnant
   ("chrono-Compatible Low-Level Date Algorithms").
5. **Atomic replace**: temp file `.<name>.<pid>.<n>.tmp` in the same directory, `write_all`, `sync_all`, `rename`. A transient error
   (Windows `PermissionDenied`, sharing or lock violation) is retried: 12 attempts, sleep 1 ms doubling to a 100 ms cap (about 0.63 s
   in total), then a located `Retry` error naming the operation, the file, the attempts and the OS error; the temp file is removed.
   On Unix the parent directory is fsynced after the rename; on Windows std cannot open a directory handle without FFI (the crate
   forbids `unsafe`), so the rename is atomic but its durability across power loss is not forced (a `shortcut:` comment marks it).
6. **JSONL**: one `write_all` of record plus `\n` per line. The reader ignores a last line without `\n` and reports it (offset,
   length); a complete line that does not parse is a located error (file, 1-based line, column). The appender truncates a torn tail on
   open before appending, so a fragment is never glued to the next record. Limit: one writer per file at a time.
7. **Path safety**: `resolve_in(root, rel)` refuses empty paths, NUL, absolute paths, drive and UNC and verbatim prefixes, `..`
   components, components with `:` (drive-relative paths, NTFS alternate streams) and any existing component that is a symlink or a
   junction (`is_symlink` is true for name-surrogate reparse points); then the deepest existing ancestor must canonicalize under the
   canonical root. Errors are `PathRefused { input, component, reason }`. Limit: check-then-use; a link created between the check
   and the use is not caught.
8. **Watcher**: poll-based. `poll()` rescans and returns a token (a generation counter that moves only when something changed);
   `changes_since(t)` lists the project-relative paths created, modified or deleted after `t`. Signature: length, mtime and, for files
   up to 1 MiB, the FNV-1a hash of the content. `.rcfd/` and temp files are ignored. Larger files are compared by length and mtime only
   (a same-length rewrite of a file over 1 MiB inside one mtime tick is missed: stated limit).
9. **Migration hook**: `MIGRATIONS[n]` turns a version-`n` manifest JSON into version `n+1`. `open` migrates in memory only and reports
   `migrated_from`; `save_manifest` persists. A newer `schema_version` is refused with a located error. Version 0 is a constructed format
   (`title`, `created_unix`): no version-0 project was ever written; it exists to exercise the hook.
10. **Query API**: `runs()` (oldest first, with state, step, time and the numeric entries of `metrics.json`; incomplete run directories
    are listed in `skipped`, not errors), `runs_in_state`, `latest_finished(case)`, `run_summary(id)`, `cases`, `meshes`, `designs`,
    `validate`.

## What I already knew when registering (rule d, disclosed)

The crate and every test below were written before this ADR so that each set-up could be smoked (debug build, one run each, this
machine under load from other agents). The smoke values below are all the output I had seen. Smoke findings that changed a set-up before
registration:

* The child-process handshake first looked for a line starting with `READY`; libtest prints `test child_entry ... ` on the same line,
  so the parent hung. Fixed to search the line; child writers now also stop by themselves after 30 s.
* J2's split-write control first wrote the two halves of a line back to back: 2 torn tails in 10 kills, so a "torn >= 1" control would
  fail with probability about 0.8^10 = 0.11. A 1 ms sleep between the halves was added: 8 of 10.
* The A3 Windows smoke showed OS error 5 (access denied, `PermissionDenied`) for a rename onto a file held open without delete sharing.

## Registered checks

Tests in `crates/rcfd-project/tests/` (child processes re-run the test binary with `RCFD_PROJECT_CHILD` set; `child_entry` is a no-op
without it, so it counts as one passing test per run).

| Id | Check | Limit | Negative control (must discriminate) | Smoke (debug) |
|---|---|---|---|---|
| R1 | Round-trip of every record: manifest, `run.json` with and without mesh, `status.json` in all 6 states, 4 event kinds, residual line, metrics; contract literals re-serialize to the same JSON; an unknown field is accepted | exact equality | 5 broken records refused (missing field, unknown state, wrong type, unknown event kind, `run.json` without hash); a parse error names the file | pass |
| I1 | `run.json` is never replaced | second create is `Exists`, content unchanged, no temp left | the replacing writer does replace | pass |
| A1 | Atomic replace under readers, threads: 1 writer, 3 readers, 2 s, 1 MiB self-validating payload (header with length and FNV) | 0 partial reads; overlap shown by writes >= 20 and reads >= 200; no temp left | in-place `fs::write` under the same readers: partial >= 1 | 109 writes, 1080 reads, 0 partial; control 20 partial of 874 |
| A2 | Atomic replace across a kill: child writes 1 MiB payloads in a loop, killed 20 times after 5 + (23 i mod 58) ms while a reader thread hammers; then the file is read | 0 bad observations (partial reads plus invalid file after kill); reads >= 20 | in-place writer child: bad >= 1 | 0 bad in 121 and 127 reads; control 80 and 14 bad |
| A3 | Windows only: rename onto a file held open without delete sharing | held 50 ms: succeeds, waited >= 40 ms; held throughout: `Retry` error with 12 attempts in < 2 s, old content kept, no temp left | 1 attempt: the 50 ms hold is fatal | 69.8 ms; gave up after 538 ms (OS error 5) |
| J1 | Torn-line tolerance on fixtures | committed lines returned, torn tail reported with exact offset and length; a parsing line without `\n` is still torn; repair removes exactly the 11 torn bytes | strict per-line reader fails on the fixture; appending without repair gives a located error at line 2 | pass |
| J2 | JSONL across a kill: one-write appender child killed 10 times (same delay schedule); 200-entry residual lines | reader never errors; every committed line equals the written record; reopen removes exactly the torn bytes; re-read has no torn tail | split-write appender child (two writes 1 ms apart): torn tails >= 1, with the same tolerance asserts | 0 torn; control 8 of 10 |
| R2 | Run-id uniqueness: 8 threads x 32 and 4 processes x 64 creations at once on one project | 512 distinct ids, 512 listed, 0 skipped, `validate` empty, list in time order | (see R2c) | pass |
| R2c | Forced contention: 16 threads behind a barrier claim in one fixed second from a 16-value suffix space | 16 distinct ids (deterministic by linear probing) | non-exclusive `create_dir_all` with random suffix: distinct < 16 (chance of a false pass 16!/16^16 = 1.1e-6) | 34 and 30 clashes resolved; control 14 and 9 distinct |
| P1 | Path escapes: 8 attempts on every platform (`..` forms, absolute, root-relative, a directory link inside `cases/` to an outside folder and a file through it) plus 5 on Windows (backslash `..`, drive-relative, UNC, plain and verbatim absolute), plus 4 refusal-only inputs (empty, blank, alternate stream, NUL) | every one refused with `PathRefused` naming the input; 5 ordinary paths accepted under the root | every escape attempt really leaves the root under a naive `root.join` (lexically, by canonicalizing, or by reading the outside file) | 17 refused (13 escapes); junction made with `mklink /J` |
| M1 | Migration hook on the constructed v0 fixture | migrated name and `created` exact; open does not write; `validate` reports the unsaved migration; after `save_manifest` reopen is current and valid | without the hook: located error; fixture does not parse as the current schema; newer schema refused; broken v0 fails naming the step | pass |
| W1 | Watcher: modify, create in a new directory, delete, same-length rewrite with mtime restored; then an atomic replace and a `.rcfd/` write | exactly the 4 changed paths; nothing after the next token; then exactly the replaced file; an idle poll keeps the token | length and mtime alone (hash limit 0) miss the same-length rewrite | pass |
| V1 | `validate` on a fresh project and after 4 planted defects (edited snapshot, torn events line, stale temp file, missing directory) | 0 issues, then exactly 4 at the right files | the planted defects are the control | pass |
| Q1 | Query API on 5 runs of 2 cases with mixed states | order, state filter, numeric metrics only, `latest_finished` skips later failed and running runs of the case and a later finished run of another case; a half-created run directory is skipped and reported | built into the fixture | pass |
| L1 | Listing 500 runs (1/3 finished with metrics), release build, own test binary | slowest of 3 warm `runs()` <= 500 ms | the same listing path with 2 ms extra per run must exceed 500 ms | debug: 166 ms; control 1.31 s |
| T1 | UTC formatting: known dates (1970-01-01, 2000-02-29, 2000-03-01, 2024-02-29, 1 700 000 000 s) and 100 000 consecutive days | exact | consecutive-day check fails on any skipped or repeated day | pass |

Limit reasons: 0 partial reads, 0 bad observations and exact sets are correctness properties, not tolerances. The A1 overlap minimums
(20 writes, 200 reads) are a fifth of the debug smoke so that the check cannot pass with readers that never ran. The A3 40 ms is the
50 ms hold minus 10 ms of timer slack; 2 s bounds the about 0.63 s retry budget with margin for load. L1: 500 ms is half of the 1 s
"flow of thought" response limit (J. Nielsen, *Usability Engineering*, 1993, chapter 5; recalled from memory, UNVERIFIED), leaving
margin for a loaded machine; the 2 ms per run control is 1 s over 500 runs, twice the limit. The J2 control asserts a probability:
with 8 of 10 in smoke, a false miss needs 10 kills without a torn tail.

What the kill checks do not test: process kill leaves the OS cache intact, so they test atomicity (old or new complete file, no torn
line accepted), not durability across power loss.

## Consequences

* The run service (47B), time-series store (47C) and the plugin (wave 48) can use `rcfd_project::atomic`, `jsonl`, `paths` and
  `Watcher` instead of writing their own.
* No existing crate changes; defaults are untouched.

## Addendum 2026-10-26: measured values (registered run, release, `cargo test -p rcfd-project --release`, this machine with other agents' gates running)

| Id | Limit | Measured | Negative control |
|---|---|---|---|
| R1, I1, J1, M1, W1, V1, Q1, T1 | exact | pass | each control discriminated (pass) |
| A1 | 0 partial; writes >= 20, reads >= 200 | 0 partial; 589 writes, 1270 reads; 0 reader open errors; no temp left | in-place writer: 862 partial of 1088 reads |
| A2 | 0 bad; reads >= 20 | 0 bad in 162 concurrent reads + 20 post-kill reads | in-place writer: 99 bad |
| A3 | >= 40 ms; `Retry` with 12 attempts in < 2 s | replaced after 71.5 ms; gave up after 537 ms (OS error 5, access denied) | 1 attempt fails (pass) |
| J2 | reader never errors; repair exact | 0 torn tails in 10 kills of the one-write appender, all asserts pass | split writer: 10 torn tails in 10 kills, all tolerated and repaired |
| R2 | 512 distinct, listed, valid | 512 distinct (ids in seconds 14:33:20 to 14:33:23 UTC), 0 skipped, `validate` empty | (R2c) |
| R2c | 16 distinct | 16 distinct, 33 clashes resolved by probing | non-exclusive: 8 distinct of 16 |
| P1 | all refused, located | 17 of 17 refused (13 escapes, junction made); 5 ordinary paths accepted | every escape left the root under a naive join |
| L1 | <= 500 ms | 82.8 ms (slowest of 3 warm listings of 500 runs; creating them took 5.5 s) | 2 ms extra per run: 1.28 s |

No misses, no amendments after registration. Deviation: clippy (`double_ended_iterator_last`) asked for `.rev().find(..)` in place of
`.filter(..).last()` in `latest_finished`; the edit landed while the registered release run was building. The two forms return the
same element; Q1 and L1 check the result.

The J2 one-write appender never left a torn line in 10 kills: `TerminateProcess` does not interrupt a single small `WriteFile`, so the
torn-tail path is exercised by the split-writer control and the J1 fixtures, not by the production appender.

## Amendment 1 (2026-10-26, registered before the corrected run): A2 negative control

What happened. The first workspace gate (archive of 95e1e3c) failed one test outside this crate:
`crates/rcfd-cli/tests/workspace_graph.rs::layering_matches_architecture_doc` panics on any crate not in its layer table. Fixed by
placing `rcfd-project` (depends on `rcfd-core` only) in layer 1 of that table and in `docs/architecture.md`. After the rebase onto
8ca5c5b a debug run of `cargo test -p rcfd-project` failed the A2 negative control (0 bad observations); three debug reruns gave 1, 8
and 4. The set-up is defective in debug builds: the child builds each 1 MiB payload inside its loop, which takes longer than the write,
so the file is complete most of the time. The registered release run (99 bad) was not affected, but a control that fails in debug
does not discriminate reliably.

Diagnosis while correcting (smoke, debug, disclosed): with payloads precomputed, the child rewrote the file so fast that in 3 of 13
runs the parent's concurrent reader blocked inside `File::open` for over 60 s after a kill (a file being truncated in place by a
killed process; a filter driver such as the antivirus is suspected, not diagnosed). With the reader off, one `fs::write` per file
gave only 1 to 5 bad of 20 (a kill does not interrupt one `WriteFile`; it lands either before or after it).

Corrected control (A2n): the child precomputes two payloads and rewrites the file in place in 8 KiB `write_all` pieces (as a
`BufWriter` would) in a loop; no concurrent reader; the file is read after each of the 20 kills (same delay schedule). Limit: bad >= 1.
Smoke (debug, 8 runs): 14 to 19 bad of 20, no hang. The old control stays as the ignored
`a2_negative_control_in_place_writer_killed_leaves_partial_files_known_miss`. The positive A2 check is unchanged.

Consequence for the run service (47B): never rewrite a file in place that readers open; an in-place rewrite killed mid-way both
exposes partial content and, on this machine, could block readers' `open`. Atomic replace never showed either.

## Amendment 2 (2026-10-26, registered before the corrected run): A2n without truncation

Run together with the other tests of `tests/process.rs` (debug smoke), the amendment-1 control hung again: the parent's post-kill
`File::open` blocked after a kill of the truncating child, so the hang does not need a concurrent reader. Corrected A2n: the child
writes the file once, then overwrites it in place from offset 0 without truncating (one handle, `rewind`, 8 KiB `write_all` pieces,
two precomputed payloads of equal length), so a kill leaves a mix of the two generations that fails the hash. Same 20 kills, same
schedule, read after each kill, limit bad >= 1. Smoke (debug, whole `process` binary, 6 runs): 19 to 20 bad of 20, no hang; the
positive A2 in the same runs: 0 bad. The registered run of A2n is the final gate.

## Final gate (2026-10-26, fresh archive of 29643bb on main 8ca5c5b)

* fmt: clean. Tests (`cargo test --workspace --release --no-fail-fast`): **2137 passed, 0 failed, 662 ignored**. This crate adds 27
  passed (2 unit, 25 integration including the no-op `child_entry`) and 1 ignored (the amendment-1 known miss); the corrected A2n
  control passed in this run (its count is not printed by the gate, which captures test output).
* Workspace clippy: **exit 101, not caused by this branch**: two errors in `crates/rcfd-api/src/design/ledger_tests.rs:134`
  (`cloned_ref_to_slice_refs`) and `crates/rcfd-api/src/design/uq_tests.rs:295` (`type_complexity`), files this branch does not
  touch, present on main 8ca5c5b. `cargo clippy -p rcfd-project -p rcfd-cli --all-targets -- -D warnings`: clean.

Found defect, not fixed (R1 covers finite values only): serde_json writes NaN and infinity as `null` and refuses `null` for an
`f64`, so a diverged run whose `residuals.jsonl` or `status.json` holds NaN would make the whole residual file unreadable and drop
the run into `skipped` (probe: `{"step":1,"time":null,"residuals":{"p":null},...}` gives "invalid type: null, expected f64"). An
additive reader-side fix (read `null` as NaN) is proposed for the roadmap.

## Amendment 3 (2026-10-28, registered before the corrected run): A1 run length on a slow disk

What happened. The first full gate in CI (`gate.yml`, windows-latest, run 40670036763, main a89ccb3) failed
`hammer::a1_atomic_replace_never_exposes_a_partial_file` on the overlap minimum: `writes: 15, reads: 2828, partial: 0, open_errors: 0`
(writes >= 20 not reached). No partial read was seen; the in-place control passed in the same gate.

Cause, reproduced on this machine before writing this amendment (release, `RCFD_PROJECT_SCRATCH` on the SATA HDD `D:`): 15 and 17
writes in 2 s, 0 partial, the control 351 and 349 writes with 933 and 886 partial reads; on the NVMe temp folder `C:` 489 writes, 1377
reads, 0 partial (control 853 partial of 1199). `atomic_write` fsyncs each 1 MiB temp file (`sync_all`); on a slow disk one fsync
takes about 130 ms, so a fixed 2 s holds about 15 writes. The in-place control does not fsync and is not affected. The set-up is
defective: its overlap minimums depend on the disk's fsync latency, which the check does not test.

Correction (same limits, same payload, readers, writer and asserts): the hammer runs at least the registered 2 s and then continues
until writes >= 20 and reads >= 200, with a hard cap of 30 s of writing; reaching the cap without both minimums fails the overlap
assert, as before. The elapsed time is printed. The negative control uses the same stopping rule (it reaches the minimums within the
2 s). The positive check keeps `atomic_write` with its fsync (not `atomic_write_unsynced`) and 1 MiB, so what A1 proves is unchanged:
no reader ever sees a partial file while a fsyncing atomic writer replaces it. The original fixed-2 s test stays as the ignored
`a1_atomic_replace_never_exposes_a_partial_file_known_miss` with its limits intact. This machine passes with and without the
correction on `C:`; the CI runner is the discriminating check.

Corrected runs (2026-10-28, release, this machine, after commit 725c696; local runs, not a gate):

* `C:` (NVMe), 3 runs: A1 439 / 425 / 454 writes, 1105 / 1462 / 1136 reads, 0 partial, 2.00-2.01 s; control 392 to 730 partial.
* `D:` (HDD), 3 runs alone: A1 21 / 26 / 28 writes, 0 partial, 2.02-2.07 s; control 225 to 359 partial. The HDD was faster than in
  the diagnosis (the fixed-2 s known miss passed once with 22 writes).
* `D:`, 4 test processes at once (a loaded slow disk), 3 rounds: corrected A1 always 20 writes, 5421 to 9505 reads, 0 partial, in
  6.09 to 7.93 s; the fixed-2 s known miss in the same runs 5 to 14 writes (fails, as in CI); the control under the same load 261 to
  558 partial (passes).
