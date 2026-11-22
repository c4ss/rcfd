---
type: concept
tags: [process, testing]
sources: [doc-glossary, doc-readme, doc-contributing, doc-process-lead-process, doc-process-documentation-review, doc-process-handover-2026-11-19, doc-audit-report, doc-test-process, doc-releasing, doc-changelog]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Gate (full check of a commit)

`cargo fmt --check`, `cargo clippy` with warnings as errors, and the whole test suite in release mode on a clean copy of the commit; over an hour; runs on `main` in CI, not before each merge ([[doc-glossary]]).

## How rcfd uses it

- Run with `python scripts/gate_test.py`; a merge is first checked by the tests its changes select ([[doc-readme]]).
- [[doc-roadmap-overview]] says the whole suite runs as a gate every day.
- The lead's side: when a full run finishes, a ledger line with commit, date, passed, failed, ignored and each failed test's primary ADR and item (`scripts/gate_attribute.py`); on green, tag `verified` moves to that commit; on a failure, the fix is issued against the failing test's ADR before new work on that line; "Main may be red on a test that was left out of a push budget until that fix lands" ([[doc-process-lead-process]], [[process-ledger]]).
- Before a push, [[push-main]] runs the tests the diff selects within 30 minutes of recorded time; the rest start in the background after the push ([[doc-process-lead-process]]).
- While the newest gate line is red, no agent is started except on the fix (stop condition, 2026-11-21) ([[doc-process-lead-process]]).
- Maintainer decisions of 2026-11-18: merging never stops for the gate; no failing test is quarantined (marked ignored to pass); main may run ahead of the newest green full run by what one gate a day can handle ([[doc-process-documentation-review]]).
- History: last green hosted gate eef6c094 (2026-11-05); local green on 49fbf1a3 (2026-11-12, 3,548 passed, 1,023 ignored); red at `205889bf` on one test of F-848 and green at `39269587` (5884 passed, 1698 ignored, 281 minutes) on 2026-11-18 ([[doc-process-documentation-review]]). A four-shard trial ran 5886 tests in 2 h 32 min against 4 h 41 min for about 1.5 times the runner minutes; the scheduled run stayed one job ([[doc-process-handover-2026-11-19]]).
- Two immersed-solid tests ran within a factor of 1.2 of the 3600 s per-test limit (one timed out); a draft item for them is in [[doc-process-handover-2026-11-19]].
- On 2026-10-14 the suite was 458 passed, 0 failed, 16 ignored ([[doc-audit-report]]; unreviewed model output).
- Measured gate process (2026-10-26, ADR 0127): `scripts/gate_test.py` runs each test as its own process on a `git archive HEAD` copy with a fresh target directory. At the same commit it "cut the test stage from 103 min (47E gate) to 66.8 min (-35 %)" ([[doc-test-process]]).
  - The per-test time limit is clamp(5 x last, 10 min, 60 min) (ADR 0129).
  - `--jobs 5` gave only -13.2 %, below the registered 25 %, "so the default stays 3".
- Tests that cannot fail ([[doc-test-process]]):
  - The `limit_audit.py` scan reports 80 hits, 2 of them real weaknesses.
  - 12 data-gated tests always skip in the gate and count as passed.
  - The mutation spot-check detected 10 of 12 (ADR 0123 S7 and ADR 0119 R3 not detected).
- Daily `Full gate` (ADR 0653) at 02:00 UTC on `main` ([[doc-test-process]]):
  - fmt runs per package, because `cargo fmt --all` exceeds the Windows command-line limit (os error 206).
  - Failures are attributed to ADRs by `gate_attribute.py`.
  - `adr_test_index.py` maps 93.5 % of tests.
- Per-merge checks are the nine-item list of `scripts/push_main.py --run`, with a 30-minute diff-selected budget, committed mutation patches and retry-twice flaky detection (ADR 0752) ([[doc-test-process]]).
- Releases: a release tag goes only on a `main` commit on which the full gate passed, "A gate on any other commit does not count" ([[doc-releasing]]). The gate does not run the about 660 `#[ignore]` tests ([[doc-releasing]]).
- The 0.1.0 changelog repeats that the gate "runs on `main` in CI on Windows, not before each merge" ([[doc-changelog]]).

## Related

- [[doc-test-process]], [[doc-releasing]], [[verification-vs-validation]]
- [[bit-identical]]
- [[known-miss]]
- [[push-main]]
- [[process-ledger]]
- [[lead]]
