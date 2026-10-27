# ADR 0129: Per-test time limit, one measurement at five jobs, and an audit of tests that cannot fail

Status: **ACCEPTED 2026-10-27** (acceptance registered before any code or run). Written at main cb61148 on branch `wtp`, worktree
`<scratch dir>`, target `<target dir>`. Follows [ADR 0127](0127-test-gate-efficiency.md); the process is
described in [`docs/test-process.md`](../test-process.md). ADR 0128 is taken by branch `wfix`.

## Context

* A hung test held a gate slot for about 3.5 h on 2026-10-26 (`docs/test-process.md` section 2.4). Neither `cargo test` nor
  `scripts/gate_test.py` has a time limit.
* ADR 0127 measured the test phase on its floor at three jobs: test-seconds / 3. The combined gate `gate47` at cb61148 ran 2190 tests
  in 3122 s wall at `--jobs 3`; its `times.tsv` sums to 9296 test-seconds (154.9 min). The user approved one registered measurement at
  `--jobs 5` (option (b) of the efficiency report); six is out of scope.
* Branch `wfix` (ADR 0128) found one negative control that is always true (500 sleeps of 2 ms compared with 500 ms,
  `rcfd-project/tests/listing_budget.rs`). There may be more tests that cannot fail.

## Decision

1. **D1 time limit** in `scripts/gate_test.py`: `--timeout-factor F` (default 5), `--timeout-min S` (default 600),
   `--timeout-max S` (default 3600; `0` turns the limit off). A test's limit is `clamp(F * t_rec, min, max)`, where `t_rec` is its
   `seconds` in the `--order` TSV; a test without a record (no file, or not in it, or not a number) gets `max`. On expiry the runner
   kills that test's process tree only, records outcome `TIMEOUT` in the TSV, prints `test X ... FAILED (timeout after N s)` and goes
   on with the other tests. The runner never kills cargo or any process it did not start. Windows: the runner lists the test's
   descendants (Toolhelp snapshot through `ctypes`) and kills them and the test with one `taskkill /F /PID ...`, leaving out any
   `cargo.exe` and its subtree (`rcfd-cli/tests/workspace_graph.rs` starts `cargo metadata` as a child). Elsewhere the test runs in
   its own session and the runner kills its process group; that branch cannot run on the gate machine. After the kill the runner waits
   at most 30 s more for the output pipe (a process outside the tree may hold it) and then gives up on the output, never hangs.
2. **D2 memory sampling**: the runner samples the working set of its live test processes every 2 s and prints the largest sum
   (`peak summed working set`). Children that a test starts itself are not in the sum.
3. **D3** One registered measurement of the test stage at `--jobs 5` against `--jobs 3`; the default changes only by the rule below.
4. **D4** `scripts/limit_audit.py`: read-only scanner (Python 3.9 standard library) for five classes of weak tests, a triage of every
   hit in `docs/test-process.md`, assertion-only fixes of real weaknesses, and a seeded mutation spot-check of registered limits.
5. No change to solver code, `Cargo.toml` profiles, any registered limit, any existing assertion, or
   `crates/rcfd-api/tests/baseline_hashes.rs`. No result caching, no skipping of tests.

## Registered acceptance

### Part 1: time limit

| Id | Check | Pass |
|---|---|---|
| T1 | A synthetic test (a tiny cargo crate in a temp folder, built and run by `gate_test.py` through the same code path, in a slot; not a repo test) that starts a child process, writes its own pid and the child's pid to a file, and then sleeps for an hour, run with `--timeout-min 10 --timeout-max 10` and no `--order` | killed within limit + 5 s (test wall <= 15 s); the test's pid and its child's pid no longer exist afterwards (checked with `tasklist`); the run exits non-zero; the test is counted `failed` in its `test result:` line and printed `FAILED (timeout after 10 s)`; TSV outcome `TIMEOUT` |
| T2 | Negative control: the same synthetic test with a 15 s sleep, run with `--timeout-max 0` (off) | not killed: it passes with test wall >= 15 s, run exits 0. And with the 10 s limit on, the same 15 s test is killed (so T1 discriminates) |
| T3 | Full real run under defaults with `--order <scratch dir>/times.tsv`: the `--jobs 3` run of Part 2, on the commit that adds Part 1 (before any Part 3 test edit, so the tests are those of cb61148) | sorted (binary, test, outcome) list equals the one of `gate47/test.log` (`diff` empty); no test times out; max over tests of (test seconds / its limit) **< 0.5** |
| T4 | Unit tests of the clamp function | at least 10 cases incl. no record, zero time, huge time, limit off; all pass |

The process-group branch (non-Windows) cannot be run on this machine; it is reported as not exercised.

### Part 2: `--jobs 5`

Both runs on one `git archive` of the same commit with one target folder (the second build is a no-op), `--order
<scratch dir>/times.tsv`, time-limit defaults, each in a slot, back to back; CPU load sampled with `typeperf` every 30 s and the
other slot's occupancy recorded.

* **Prediction**: floor at 5 jobs = 9296 test-seconds / 5 = **31.0 min** (at 3 jobs: 51.6 min; gate47 measured 52.0 min). Contention
  (a solver test uses about two cores) will put the measured time above the floor.
* **Pass condition** (all of): the sorted (binary, test, outcome) list at 5 jobs equals the one at 3 jobs (which T3 compares with
  gate47); 0 failures at 5 jobs that
  are not failures at 3 jobs; no TIMEOUT; peak summed working set (D2) **< 12 GB**.
* **Decision rule**: `--jobs 5` becomes the default only if the pass condition holds **and** the test-phase wall
  time (the runner's `wall`) at 5 jobs is **at least 25 % below** the one at 3 jobs from the same pair. Otherwise the default stays 3
  and the addendum says why. Adopting means adding `--jobs 5` to the gate command in `docs/test-process.md` section 4 (the gate
  exports `RUST_TEST_THREADS=3`, which the script's default reads, so changing the script's fallback alone would change nothing;
  `RUST_TEST_THREADS` stays 3, as ADR 0127 left it). A test that fails only at 5 jobs is named as a load-sensitive finding.

### Part 3: tests that cannot fail

| Id | Check | Pass |
|---|---|---|
| A1 | Scanner self-test: one planted example per class (i) to (v) and one clean example | 5 of 5 classes found, clean example not flagged |
| A2 | Triage table in `docs/test-process.md` | every hit classified as real weakness, documented skip or false positive: **0 unclassified** |
| A3 | Every real weakness is either fixed by adding or correcting an assertion (no registered limit or other assertion changed), with a throwaway break in a scratch copy shown to fail the fixed test and then reverted, or listed as needing a design decision | all hits handled |
| A4 | Mutation spot-check: population = every registered check id in the acceptance sections of ADRs 0115 to 0126 that a non-ignored `#[test]` enforces (0127's goals are process measurements, not tests; ignored `*_known_miss` tests already fail and cannot discriminate). The enumerated, sorted population is committed as an amendment to this ADR **before** the draw, which is then with Python `random.Random(1290127).sample(population, 12)`. In a scratch copy the governed quantity is made 10x worse (or the control flipped) and only that test is run | 12 of 12 fail; each one that still passes is reported as a finding with its test name, nothing loosened or retuned |
| A6 | Stale binaries (ADR 0127 section 5.4): every scratch copy for A3 and A4 has its own `CARGO_TARGET_DIR`, and a break or mutation result counts only if the build output of that run shows `Compiling <the edited crate>` after the edit | rule, checked per run |
| A5 | Test list after the fixes | the sorted (binary, test) list is a superset of the one before: only additions |

A missed check is reported as missed with the measured value; no check is moved.

## Amendment 1 (2026-10-27, before the A4 draw): the A4 population

Enumerated from the acceptance sections and amendments of ADRs 0115 to 0126 (each id with its enforcing `#[test]`, checked by a
script for existence and ignore flag; nothing run). Known before writing this: the scanner hits (A2), not any mutation result. Population: 183 (ADR, id) pairs, sorted by (ADR, id) as strings; the enforcing test of each is in the enumeration
table of the drawn ones goes to `docs/test-process.md` section 6.4. Excluded: 23 (0115 N1 no test, 0115 P4 ignored, 0115 P6 no test, 0115 Z1 ignored, 0115 Z2 ignored, 0117 B no test, 0117 D-c no test, 0117 D-d ignored, 0117 F-a ignored, 0117 F-b no test, 0117 F-harness ignored, 0117 route-check no test, 0118 D1 no test, 0118 D2 no test, 0119 V1 no test, 0119 Z1 ignored, 0120 J1 ignored, 0120 W no test, 0123 S5 ignored, 0125 P1 ignored, 0126 R1 ignored, 0126 R2 ignored, 0126 R3 ignored).

* 0115: P1, P2, P3, P5
* 0116: G1, G2, G3, G4, G5, G6, G7, N1, N2
* 0117: D-a, D-b, D-d', N1, R1
* 0118: C1, C2, C3, C4, C5, E1, E2, E2b, E3, I1, I2, I3, I4, NC1, NI1, NI2, NS-S, NS1, NS2, NS3, S1, S2, S3, S4
* 0119: D1, D2, E1, E2, F1, G1, L1, L2, L3, M1, NC-E1, NC-E2, NC-F1, NC-G1, NC-L1, NC-L2, NC-L3, NC-M1, NC-R1, NC-R2, NC-R3, NC-S1, NC-Z1, NC-Z2, P1, R1, R1b, R2, R3, R3b, S1, Z1', Z2
* 0120: B, E, I, J1b, J2, J3, J4, R1, R2, U, X
* 0121: L1, L2, L3, L3b, L4, L5, L6, M1, T1, T2, U1, U2, U3, U4, U5, U6, U7, U8
* 0122: A1, A2, A2n, A3, I1, J1, J2, L1, M1, P1, Q1, R1, R2, R2c, T1, V1, W1
* 0123: S1, S10, S11, S2, S3, S4a, S4b, S4c, S5a, S6, S7, S8, S9
* 0124: C1, C10, C11, C12, C2, C3, C4, C5, C5b, C6, C6b, C7, C8, C9, H1, H2, H3, H4
* 0125: C1, C2, C3, D1, G1, G2, G3, K1, P1b, P2, R0, S1, T1, U1, V1, W1
* 0126: D1, D2, D3, D4, D5, D6, D7, P1, P2, P3, P4, T1, T2, T3, T4

The draw is `random.Random(1290127).sample(population, 12)` over this sorted list. A drawn check that does not run on this machine
(a GPU test that skips without an adapter, a release-only assertion run in debug) is reported as not exercised, not as detected.

## Addendum 2026-10-27: measured against the registered checks

### Part 1: time limit (implemented in de3df40)

| Id | Pass condition | Measured | Verdict |
|---|---|---|---|
| T1 | killed within limit + 5 s, test and child pids gone, run non-zero, counted failed, `FAILED (timeout after 10 s)`, TSV `TIMEOUT` | test killed at 10.2 s (whole run 10.8 s including the doc-test step); test pid 75572 and child pid 67776 (`ping`) not in `tasklist` afterwards; exit 101; `test result: FAILED. 1 passed; 1 failed`; line `test hang ... FAILED (timeout after 10 s)`; TSV `TIMEOUT` | **met** |
| T2 | 15 s sleep not killed with `--timeout-max 0`; killed with the 10 s limit | limit off: passes at 15.0 s, exit 0; limit on: `TIMEOUT` at 10.2 s, exit 101, both pids gone | **met** (discriminates) |
| T3 | `--jobs 3` run: (binary, test, outcome) list equals gate47's; no timeout; max time/limit < 0.5 | 2856 = 2856 lines (2190 ok, 666 ignored), `diff` empty; 0 timeouts; max ratio 0.264 (`transient_sources::t3d_...`, 949 s against its 3600 s limit) | **met** |
| T4 | at least 10 clamp cases, all pass | 15 cases (no record, zero, 0.07 s, exact min and max boundaries, huge, min = max, no minimum, limit off with and without record, negative max) | **met** |

The synthetic tests are `scripts/test_gate_test.py` (`GATE_TEST_E2E=1`, run in slot 1 at 09:51). The process-group branch for other
systems was not exercised. The cargo exclusion of the tree kill was not exercised by a test (no test of the repo times out while
`cargo metadata` runs).

### Part 2: `--jobs 5` (archive of de3df40 in `<scratch dir>`, logs there)

| | `--jobs 3` | `--jobs 5` |
|---|---|---|
| slot, time | slot 1, 09:52 to 11:03 (build 6 min 35 s, shared) | slot 1, 11:03 to 11:59 (build no-op) |
| other slot | busy 10:10 to 10:12 and from 10:23 (another gate): about 39 of the 64 test minutes | busy throughout |
| CPU, mean of 30 s `typeperf` samples in the test phase | 94 % | 100 % |
| test-phase wall (runner `wall`) | **3849 s (64.1 min)** | **3343 s (55.7 min)** |
| test-seconds (sum over tests) | 11 547 s (gate47: 9296 s) | 16 714 s |
| result | 2190 / 0 / 666 | 2190 / 0 / 666 |
| (binary, test, outcome) list | = gate47 | = `--jobs 3`, `diff` empty |
| timeouts; max time/limit | 0; 0.264 | 0; 0.521 (`t3d_...` 1877 s) |
| peak summed working set (runner, 2 s samples) | 2702 MB | 2704 MB |

* Prediction 31.0 min (9296 s / 5); measured 55.7 min. The machine was saturated (CPU 100 %), so every test ran longer: the
  test-seconds rose 45 % from 3 to 5 jobs and the 5-job run sat on its own floor (16 714 s / 5 = 55.7 min). The largest slowdowns
  were about 2x (`w34c_symmetry::a6_mirror_equivalence_graded` 140 s to 284 s, `t3d_...` 949 s to 1877 s).
* Pass condition: list identical, 0 new failures, no timeout, 2.7 GB < 12 GB: **met**. No test failed only at 5 jobs (no
  load-sensitive test found).
* Decision rule: wall time 13.2 % below the 3-job run, not the required 25 %: **the default stays 3**; the gate command is not
  changed. Why: with another gate in the other slot the processors are already full at 3 jobs (94 % CPU), so more test processes only
  stretch each test. The load was not equal: the other slot was busy for about 61 % of the 3-job run and all of the 5-job run, which
  works against 5 jobs; the measured gain is a lower bound for an idle machine but the gate machine is rarely idle.
* Observation: at 5 jobs under load the slowest test reached 52 % of its limit (the 0.5 check is registered for the default run
  only, T3). A default of 5 jobs would need the factor or the maximum looked at again.

### Part 3: tests that cannot fail (details: `docs/test-process.md` section 6)

| Id | Pass condition | Measured | Verdict |
|---|---|---|---|
| A1 | 5 of 5 planted classes found, clean example not flagged | 5 of 5 (7 planted hits), clean test not flagged (`scripts/test_limit_audit.py`) | **met** |
| A2 | every hit classified, 0 unclassified | 80 hits: 2 real weaknesses, 53 documented skips or report-only tests, 25 false positives; 0 unclassified | **met** |
| A3 | each real weakness fixed with a shown break, or listed for a decision | 0 fixed; listed: L1 sleep control (fix registered by `wfix`, ADR 0128 finding 12), schemes_wave13 T2 vacuous pass; also listed: 12 data-gated tests that always skip in a `git archive` gate, 7 child-entry tests counted as passed | **met** (all handled; no assertion-only fix was possible) |
| A4 | 12 of 12 detected, or each miss reported | 10 of 12; misses reported as findings, nothing changed: ADR 0123 S7 `s7_cancel_leaves_a_consistent_folder` (5 s limit, 0.116 s measured, 10x slower passes); ADR 0119 R3 `r3_stall_is_located` (asserts only "stall"; with the slope stall check off the strip check still reports "stalled", so the slope check is unguarded) | **missed** (2 findings) |
| A5 | test list only grows | no test edited, list unchanged | **met** |
| A6 | own target per scratch copy; `Compiling <crate>` in every counted run | `target-wtp-mut`; shown in all 12 counted runs | **met** |

Deviations: the scanner's helper resolution and constant rules were tightened after its first run on the repo (871 hits, mostly
`let mut worst = 0.0` taken as a constant and helper calls not followed); the self-test was written for the final rules. The scanner
covers the registered paths only, not the 99 inline `#[cfg(test)]` modules of other `src/` files. Three mutation runs were
repeated for set-up reasons (a compile error, a duplicated flag, a missing `rcfd.exe`), not to change a result. The first R3
mutation (strip stall check off) failed the test at the R3b line, a separate id; on review it did not break R3's own path, so it
was replaced by the slope-check mutation, which R3 does not detect. The replacement lowered the count from 11 to 10.

### Final gate

`git archive` of ea77b91 in `<scratch dir>` (slot 2, 12:06 to 13:36, another gate in slot 1 for most of it):
`cargo fmt --check` clean, `cargo clippy --workspace --all-targets -- -D warnings` clean, `python scripts/gate_test.py --order
gatetp-p2/times_j3.tsv`: **2190 passed, 0 failed, 666 ignored**, exit 0; 0 timeouts, max time/limit 0.458, peak summed working set
2636 MB, test phase 76.1 min (load). The (binary, test, outcome) list equals the `--jobs 3` run's and gate47's (`diff` empty), so
A5 holds. Later commits change only this ADR and `docs/test-process.md`. Main was still cb61148, so the rebase was a no-op.
