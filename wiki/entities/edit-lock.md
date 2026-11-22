---
type: entity
tags: [rcfd-project, concurrency, testing, gate]
sources: [adr-thread-daily-gate-and-edit-lock]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Edit lock of the project folder (rcfd-project)

**What it is:** the lock that serialises writers of a project file in [[rcfd-project]] (`replace_if_hash`, `EditLock`), with a claim file for a writer that has waited and a journal lock; its tests were the most frequent failures of the daily full gate in November 2026 ([[adr-thread-daily-gate-and-edit-lock]]).
**Where:** `crates/rcfd-project` (`atomic.rs`); tests `edit_lock_w48f`, `edit_lock_f751` (F2), `edit_lock_f755` and `edit_lock_f855` (feature `test-hooks`), `edit_lock_f2/mod.rs`.

## Details

- **F-751 (ADR 0669):** the gate's failure of `two_writers_with_base_hashes_lose_no_update` had two causes: the path was resolved outside the lock (refused under `$Extend\$Deleted` or delete-pending), and the lock was unfair. Fix: a second resolve under the lock and a claim file "that a writer waiting 50 ms creates and the others respect (stale after 4 s)"; 59 failures in 200 runs before, 200 of 200 after ([[adr-thread-daily-gate-and-edit-lock]]).
- **F-755 (ADR 0676):** test-only hooks under the feature `test-hooks` (off in every default build); the old acquire starved in 10 of 10 trials, the claim was accepted in 10 of 10 within 177 ms; `EditLock::drop` retries a transient removal error (527 ms) and logs `edit_lock_remove_failed` ([[adr-thread-daily-gate-and-edit-lock]]).
- **F-761 (ADR 0681):** the daily gate runs `edit_lock_f755` in a step of its own with a "feature isolation" check ([[adr-thread-daily-gate-and-edit-lock]]).
- **F-855 (ADRs 0751, 0758, partial):** F2 ended 4 of 10000 probe calls in `LockBusy` in one gate and 1 in another. Fixed: journal-lock and claim removals retried (ADR 0751); a waiter's step between two looks counts for at most 100 ms of `EDIT_LOCK_WAIT` (change W, ADR 0758). F2 was amended to accept at most 4 `LockBusy` calls of class S (a holder stalled for the whole wait), counted and printed. "Which cause the two gates met is not shown" ([[adr-thread-daily-gate-and-edit-lock]]).

## Related

[[rcfd-project]], [[gate]], [[adr-thread-daily-gate-and-edit-lock]], [[known-miss]]

## Open questions

- F-858 (the gate runs the controls of ADR 0751 and keeps the project folder of a failed edit-lock test) and F-859 (a forced control of the second resolve, so that M2 and M6 of ADR 0669 are caught again) are todo ([[adr-thread-daily-gate-and-edit-lock]]).
- No hosted runner has run the amended F2 yet ([[adr-thread-daily-gate-and-edit-lock]]).
