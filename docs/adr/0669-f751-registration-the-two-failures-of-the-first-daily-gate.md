# ADR 0669: F-751 registration: the two failures of the first daily gate (P4's temporary path, and the edit lock's races)

Status: **REGISTERED 2026-11-16, written before any code of F-751 and before any run of its checks.** Written by the
implementing agent on branch `f751-work` (from 6600f7c9), worktree `w49i-work/f751`, scratch `w49i-work/f751-scratch`. Not yet
accepted by the maintainer. Every choice below that the maintainer would normally be asked about is taken on the maintainer's
standing instruction of 2026-11-11 (take the recommendation) and is marked "(standing instruction)". Results go in the addenda
at the end; nothing above the addenda is changed after a run.

Roadmap item F-751 (deps F-723, the daily gate of [ADR 0653](0653-f723-registration-daily-full-gate-with-failures-attributed-to-adrs.md)).
The binding acceptance:

> (1) f699_fidelity p4_table_errors_name_row_and_column_never_a_value: its leak search matched the cell '198' inside the runner's temp path (rcfd_f699_p4_1980); the old test made a record (ignored, limit intact) in favour of ADR 0642's P4c, which searches with the path removed, or the shared cause otherwise fixed, with no other test edited. (2) rcfd-project edit_lock_w48f two_writers_with_base_hashes_lose_no_update: fails 1 in 6 locally ('edit lock ... is held by another writer' returned to a cooperative writer); the cause located (lock retry or the test's expectation), fixed without changing a default result, and the test passing in 200 of 200 repeated runs; both shown by re-running the gate's per-test command.

Every limit not stated there is registered here. House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed
after a run; each registered run made once; existing tests and ignored records not edited; defaults bit-identical; no golden,
hash table or `baseline_hashes.rs` edit; no new dependency, no unsafe code; clean-room.

## What was read

`AGENT_RULES.md`; the brief; the gate artifact of run 43919555816 (commit d55c43c4: `test.log`, `progress.log`, `times.tsv`);
`scripts/gate_test.py` (each test is run as `<exe> <name> --exact --test-threads=1` with the cwd and `CARGO_MANIFEST_DIR` of its
package; tests in the binary's `--ignored` listing are reported `ignored` and not run); `crates/rcfd-api/tests/f699_fidelity.rs`
(P4, P4c) at d55c43c4 and at 6600f7c9; ADR 0642 (F-717, P4c); ADR 0135 (W48-15 (a), amendment 1 item 14, amendment 2);
`crates/rcfd-project/src/atomic.rs` (`acquire`, `replace_if_hash_with`, `atomic_write`, `is_transient`),
`crates/rcfd-project/src/paths.rs` (`resolve_in`, `rel_string`), `crates/rcfd-project/src/journal.rs` (`append`, its lock),
`crates/rcfd-project/src/lib.rs` (`ProjectError`); `crates/rcfd-project/tests/edit_lock_w48f.rs` and `journal_f273.rs`; the callers
of `replace_if_hash*`: `crates/rcfd-workspace/src/save.rs` (the Studio's save), `crates/rcfd-workspace/src/write.rs` (the MCP
`edit_case` tools), `crates/rcfd-workspace/src/lib.rs` (`LockBusy` to 1201), and `crates/rcfd-studio/tests/workspace_f270.rs`
(A3 holds the lock file by hand).

## Clause (1): already met on main by F-717

At the gate's commit d55c43c4 the test `p4_table_errors_name_row_and_column_never_a_value` ran; its search for cell texts ran
over the whole message, which contains the temporary path `...rcfd_f699_p4_<pid>`; the runner's process id 1980 contains the
`point` cell text `198`. F-717 (commit d314508c, ADR 0642, merged into main before 6600f7c9) already did what clause (1) asks:
P4 carries `#[ignore = "superseded set-up record (ADR 0642): ..."]` with its body unchanged (checked before this text by a
textual diff of the function body at d55c43c4 and at 6600f7c9: identical, 63 lines), and the corrected test
`p4c_table_errors_with_the_path_removed` searches the message with the path text removed and carries a positive control (its tag
holds `158`, so the old form fails on every run). F-751 changes nothing in `f699_fidelity.rs` or the fidelity code. The check is
the gate's own command (R1 below).

## Clause (2): the cause located before this text (disclosed)

### Runs made before this text (diagnosis only; not the registered checks)

1. **The unchanged test, unchanged code** (worktree at 6600f7c9, release, the gate's per-test command, 20 runs): 1 failure.
2. **Instrumented scratch copy** (`f751-scratch/diag`, a `git archive` of 6600f7c9 with `eprintln!` lines added in `acquire`
   and in `resolve_in`; never in the worktree). Loops of 8, 27, 27 and 150 runs of the same test (failures 3, 3, 3 and 20; in the
   150-run loop 19 of kind A and 1 of kind B), three kinds of failure, all in a cooperative writer, none of them a lost update:
   * (A) `PathRefused { reason: "resolves outside the project root" }` (the large majority): `resolve_in`'s defence-in-depth
     `canonicalize` of the target `cases/c.txt` returned `\\?\C:\$Extend\$Deleted\<id>`. `resolve_in` runs in
     `replace_if_hash_with` **before** the edit lock is taken. The other writer, under the lock, renames its temporary file over
     `cases/c.txt` (`atomic_write`); a `canonicalize` that opened the old file just before that rename asks for the final path of
     an unlinked file, which NTFS reports under `\$Extend\$Deleted` (POSIX delete semantics).
   * (B) `Io { op: "canonicalize" }` with "Access is denied" (os error 5) or "cannot find the file" (os error 2): the same race,
     the open landing on the replaced file in its delete-pending state, or between `exists()` and `canonicalize`.
   * (C) `LockBusy` (the gate's failure) was **not** seen locally in these runs. A second instrumented copy
     (`f751-scratch/diag2`) recorded the time each successful `acquire` waited, 15 runs of the test (about 650 acquires each):
     the bulk of the waits is 4 to 32 ms (one critical section of the other writer: the write with its fsync and the journal
     line), the maximum per run 31 ms to 375 ms, the counts per doubling above 32 ms falling by a factor of about 3 to 8. A
     waiter sleeps 2 ms between attempts, while the holder, once it lets go, takes the lock again within a fraction of a
     millisecond (re-read, hash, resolve, `create_new`); the waiter gets the lock only when one of its attempts lands in that gap.
     Nothing bounds the number of misses: the lock is not fair. On the gate runner (4 cores, the gate's parallel jobs, slower
     fsync) the same tail reaching the 2 s `EDIT_LOCK_WAIT` gives the gate's `LockBusy` after 3.98 s of test time. This is an
     inference from the measured tail, not a local reproduction of (C); F1 below reproduces the mechanism on purpose.
3. The arithmetic of the expected numbers below uses these measurements only as order-of-magnitude estimates.

### Which side is wrong: the library

The test's expectation is the documented contract: ADR 0135 W48-15 (a) and the doc of `ProjectError::LockBusy` ("another
cooperating writer held the file's edit lock for the whole wait") and of `replace_if_hash` (a mismatch is a conflict). In (A)
and (B) a valid path inside the project is refused or fails because of a cooperating writer's correct write; in (C) the lock was
free many times during the wait. All three are returned to the Studio (`save.rs`) and the MCP `edit_case` tools (`write.rs`) as
1004, 1200 (internal) or 1201 today. The test stays as it is.

## Design (fixed now)

### D1. Resolve again under the lock when the first resolve may have raced (cause A, B)

`replace_if_hash_with` keeps its order for every call whose first `resolve_in` succeeds (no change at all on that path). When the
first `resolve_in` fails with an error a concurrent rename can cause, the call takes the edit lock and resolves once more under it;
no cooperating writer can replace the file while the lock is held, so the second result is the file's own. Racy errors:
`PathRefused` with the reason "resolves outside the project root", and `Io` (the `symlink_metadata` walk, `canonicalize` of the
root or of the probe). Every other refusal (empty, NUL, absolute, prefix, `..`, colon, symlink or junction) is deterministic and
returned at once as today. The lock key needs the normalised relative path before the resolve: it is formed lexically (the
`Normal` components of `rel`, `CurDir` dropped, joined with `/`), which is exactly what `rel_string(root, resolve_in(..))` gives,
since `resolve_in` builds its result by pushing those same components on `root`. If `rel` has any other component, the first
error is returned. Under the lock the second `resolve_in`'s result (success or its own error) is returned.

Effect on results: a call that resolves at the first attempt is unchanged. A call refused for a racy reason by a real cause
returns the same error, after taking the lock (it may wait for a busy lock, and `.rcfd/locks` may be created). The ADR 0122 P1
limit is unchanged (check-then-use; non-cooperating writers are not excluded by the lock).

### D2. A claim that makes the edit lock fair for a writer that has waited (cause C)

Derivation (first principles; the idea is the hand-off of a starving waiter, as in Lamport's bakery algorithm where a waiter
that has announced itself is served before newcomers, Lamport 1974, cited from memory and used only for the idea): next to the
lock `.rcfd/locks/<fnv64>.lock` a claim file `.rcfd/locks/<fnv64>.claim` (`create_new`, content `<pid> <who> <time>`).

* A waiter that has waited at least `EDIT_CLAIM_AFTER = 50 ms` (above the 4 to 32 ms bulk of a normal wait, so an ordinary
  short wait creates no claim file) tries to create the claim; at most one writer holds it.
* A writer that does not hold the claim and finds a claim younger than `EDIT_CLAIM_STALE = 4 s` does not try to create the lock;
  it waits as for a busy lock (same 2 ms sleep, same `EDIT_LOCK_WAIT`, same `LockBusy`). A claim older than 4 s is left by a
  writer that crashed (a claimant waits at most `EDIT_LOCK_WAIT = 2 s` and removes its claim on every exit), and is removed.
* The claimant removes its claim as soon as it holds the lock, when it gives up (`LockBusy`), and on any error (a guard).
* Safety is still the lock's `create_new` alone: a writer that checked "no claim" just before a claim appears can still take the
  lock once; then the claimant waits one more critical section. A starving waiter waits at most about `EDIT_CLAIM_AFTER` plus
  one critical section plus one sleep after it claims.
* `pub fn edit_claim_path(root, rel)` beside `edit_lock_path`; constants `EDIT_CLAIM_AFTER`, `EDIT_CLAIM_STALE` public.
  `EDIT_LOCK_WAIT` and `EDIT_LOCK_STALE` keep their values; stale-lock breaking and its log line are unchanged.

Effect on results: an uncontended edit, or one that waits less than 50 ms, creates no claim and runs exactly as today. A genuinely
held lock still gives `LockBusy` after 2 s (the claim is removed then). The edit log gets no new line kind.

Callers: no signature changes. `rcfd-workspace` `save.rs` (Studio save) and `write.rs` (MCP `edit_case`, `set_field`) get fewer
1201 and no spurious 1004 / 1200 under contention; their mapping of errors is unchanged. `rcfd-studio`'s A3 test holds the lock
by hand for 200 ms and keeps working (its waiter claims, then gets the lock after the release).

## Checks (limits fixed now)

All runs are release builds with `CARGO_TARGET_DIR=f751-scratch/target`, made with the gate's per-test command
`<exe> <name> --exact --test-threads=1` from the package directory with `CARGO_MANIFEST_DIR` set (as `gate_test.py`), counting a
run as passed when its output has `test result: ok. 1 passed; 0 failed; 0 ignored` and exit 0 (the gate's rule).

| Check | Clause | What | Limit | Estimate behind it |
|---|---|---|---|---|
| R1a | (1) | `f699_fidelity --list --ignored` | lists `p4_table_errors_name_row_and_column_never_a_value` (so the gate reports it `ignored`) | done by F-717 |
| R1b | (1) | `f699_fidelity p4c_table_errors_with_the_path_removed --exact --test-threads=1`, 1 run | passes | ADR 0642 addendum: met |
| R1c | (1) | `git diff 6600f7c9 -- crates/rcfd-api/tests/f699_fidelity.rs` at the tip | empty (no test edited) | nothing to change |
| NC0 | (2) | the unchanged test on the unchanged code (exe built at 6600f7c9), 200 runs | at least 1 failure | 1 of 20 unchanged, 20 of 150 instrumented |
| L1 | (2) | `edit_lock_w48f two_writers_with_base_hashes_lose_no_update` on the fixed code, 200 runs | 200 of 200 pass | D1 removes A, B; D2 bounds C |
| L1t | (2) | the same 200 runs, test time each (the `finished in` value) | median at most 2 times NC0's median of its passing runs | the claim costs a few file operations per long wait |
| F1 | (2) | new forced-contention test: a lock-cycling holder (below), 10 trials | 10 of 10 edits accepted, each in under 1.0 s; no `.claim` file left | expected about 50 + 20 + 2 ms |
| F2 | (2) | new forced race test: a cooperative racer and a probe with a wrong base (below) | 10000 of 10000 probe calls return `Conflict`; the racer has 0 errors | D1 |
| F3a | (2) | a fresh claim file (made by the test) with no lock: `replace_if_hash` | `LockBusy` after at least 2.0 s and under 30 s; the file not written; the claim file is still there | D2 |
| F3b | (2) | the same claim file aged past `EDIT_CLAIM_STALE` + 5 s | the edit is accepted; the claim file is gone; no lock left | D2 |
| E1 | (2) | existing tests that run the changed code, each test by the gate's command once: `rcfd-project` `edit_lock_w48f` (all 4), `journal_f273` (all), `rcfd-workspace` `save_f270`, `journal_f273`, `mesh_f272`, `feed_replay_f275`, `rcfd-mcp` `edit_hook_f275`, `feed_f273`, `rcfd-studio` `workspace_f270`; `cargo test -p rcfd-project --release --lib` | all pass | no result changes |
| E2 | rule 7 | `cargo fmt -p rcfd-project --check`, `cargo clippy -p rcfd-project --all-targets -- -D warnings` | clean | |

F1 set-up (new file `crates/rcfd-project/tests/edit_lock_f751.rs`): a project; a holder thread cycles on the lock of
`cases/t<k>.txt`: wait while the claim exists (1 ms sleeps), `create_new` the lock (retry on failure), hold 20 ms, remove it, and
go round again at once; this is a cooperating writer that obeys the claim, with a gap between release and re-take of
microseconds instead of the library's fraction of a millisecond. 100 ms after the holder starts, the main thread calls
`replace_if_hash(root, "cases/t<k>.txt", None, b"x", "f1")`, timed. Trial k = 0..9 each on its own file.

F2 set-up (same file): a project with `cases/c.txt`; a racer thread loops `replace_if_hash` with the right base (appends a line)
until the probe ends; the probe thread makes 10000 calls of `replace_if_hash(root, "cases/c.txt", Some("fnv64:0000000000000000"),
b"p", "probe")`, each must be `Conflict`; any other result is recorded (not a panic in the thread) and counted.

F1 to F3 run once each (one registered run of the file). L1 and NC0 are run once each as 200-run batches by a script in the scratch
folder that logs every run.

## Negative controls and mutations (on scratch copies; what is expected to catch each)

| | Mutation | Expected to be caught by |
|---|---|---|
| NC0 | the code of 6600f7c9 | the existing test, at least 1 of 200 (above) |
| M1 | no claim is ever created (D2 off; the old acquire) | F1 (the waiter starves: an attempt lands in a microsecond gap; expected `LockBusy` in most trials) |
| M2 | no second resolve under the lock (D1 off) | F2 (estimate uncertain: from (A) about 1e-4 to 1e-3 per probe call, so 1 to 10 failures expected in 10000; if it survives F2 that is recorded, and L1's 200 runs are a second chance) |
| M3 | the claimant does not remove its claim after taking the lock | F1 (claim file left), F3b is not affected |
| M4 | a non-claimant ignores a fresh claim | F3a (the edit is accepted) |
| M5 | a stale claim is never removed | F3b (`LockBusy`) |
| M6 | the lexical key uses the platform separator instead of `/` | F2 (no: the first resolve rarely fails) and E1 `create_replace_and_conflict` (no); expected caught only by a test of the retry branch's key; recorded as a gap if it survives |

## Decision rule

* **done:** R1a to R1c, NC0, L1, L1t, F1 to F3, E1, E2 met; M1 to M5 caught.
* **partial:** L1 met and a mutation of M1 to M5 survives, or L1t or an F limit missed (an ignored `*_known_miss` record); or
  NC0 shows no failure (then L1 shows nothing for the races and only F1 / F2 carry the clause).
* **todo** (the lead does not take the change): L1 missed, or an E1 test fails because of the change.

## Limits (not covered)

* The gate's `LockBusy` itself is not reproduced locally as `LockBusy`; its cause is inferred from the measured tail of the lock
  waits and shown on purpose by F1 under M1. The gate's next daily run is the field check.
* Non-cooperating writers (an editor, Claude Code's own tools) can still race `resolve_in` and the write (ADR 0122 P1, ADR 0135).
* The journal's own lock (`journal::lock`, 1 ms sleeps, best effort, a line without `seq` on timeout) has the same unfair shape;
  not changed here.
* Breaking a stale claim, like breaking a stale lock, is not atomic against a second writer doing the same at the same moment.
* Linux and macOS: the races A and B need the Windows rename and final-path semantics; D1 and D2 run there too (no platform
  branch) but are only measured here on Windows 11.
