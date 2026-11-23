---
type: source
tags: [adr-thread, gate, ci, testing, edit-lock, rcfd-project, flaky-tests]
sources: [doc-test-process, doc-process-lead-process]
origin: docs/adr/ (ADR list below)
origin_commit: 09647baa
created: 2026-11-22
updated: 2026-11-23
status: draft
---

# ADR thread: the daily full gate in CI and the failures it found (F-723, F-751, F-755, F-761, F-849, F-853, F-854, F-855, F-866, F-876)

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 09647baa (first written at 2e94e5ef; ADR 0773 added at 09647baa, the earlier ADRs unchanged since).
**Kind:** ADR thread (registrations with result addenda, amendments by the lead, the lead's answers), 11 ADRs, 2026-11-16 to 2026-11-22.

ADR 0653 put the full [[gate]] into GitHub Actions as a daily run whose failed tests are attributed to ADRs and merges. Every other ADR here answers one failure that the daily gate then showed: the edit lock of `rcfd-project` (F-751, F-755, F-761, F-855), two immersed-solid tests near the per-test time limit (F-849), two wall-clock limits that failed on a loaded runner (F-854), three tests sharing one temporary folder (F-853), and the negative control of `listing_budget` L1 (F-866). The test-to-ADR index that the attribution reads is its own thread: [[adr-thread-test-to-adr-attribution]].

## Current state

- **The daily gate runs and is sharded.** F-723 done (ADR 0653): cron daily 02:00 UTC on main, skipped when the last green Windows gate tested the same SHA; failed tests re-run once (flaky if they pass), looked up in the test-to-ADR index and listed against the first-parent commits since the last green gate; optional bisect and issue opening. Amendment 3 (2026-11-18) adds parallel shards; amendment 4 (2026-11-21, by the lead): "a run without the input `shards` (a scheduled run) uses 4", after F-849 was done and three dispatched four-shard runs were green (1 h 52 min to 2 h 1 min each).
- **Per-test time limits** (F-849 done, ADR 0740): `scripts/gate_limits.tsv` gives `immersed_solid_f219::a3_two_d_two_strouhal_number` and `immersed_unsteady_force_f346::a2t_c1_two_d_two_on_t` 10 800 s; every other test keeps 3600 s. G1 met by the lead on 2026-11-21 on three consecutive four-shard gates (largest time 3885 s against the check limit 5400 s).
- **Wall-clock limits on a loaded runner** (F-854 done, ADRs 0749, 0750): A4 of `vof_dambreak_amg` now compares interleaved step-time sums; I2 of `monitors_filtered_f456` was re-defined by the maintainer's answer of 2026-11-20 to time "an increment of 100 samples per series of a store that grew by pushes, with the reallocation outside the timer, limit 2 ms unchanged". Row G (three consecutive full gates with both tests passing at the first attempt) met 3 of 3 on 2026-11-21.
- **The edit lock** (F-855 partial, ADRs 0751, 0758): F2 of `edit_lock_f751` failed in the gates of e9378ecb (4 of 10000 probe calls `LockBusy`) and f6e68b4a (1). ADR 0751 found and fixed one fault that fits the print (a journal lock left behind when its one removal fails); ADR 0758 removed a second (a waiter's deadline read from the wall clock, so a stalled process was refused) and amended F2 to accept, counted and printed, at most 4 `LockBusy` calls against a holder stalled for the whole wait. "Which cause the two gates met is not shown." The lead kept F-855 partial: "The item is done when full gates show F2 passing, or a failure whose printed class is read."
- **A replace refused on a newly written file** (F-876 done, ADR 0773, merged ecdfe05f on 2026-11-23): `adopt_f288::a_n_a_run_that_differs_in_one_item_is_not_adopted` of `rcfd-mcp` failed once in a left-out run with `Retry { op: replace, attempts: 12, PermissionDenied }` on the `run.json` of a run. The cause shown is "**a kernel-mode handle on a newly written file, held for up to 1.84 s, which refuses its replace**": 21 of 900 fresh replaces were refused with code 5, at every refused attempt the only holder was pid 4 (the System process), and 0 of 200 replaces of files 3 s or more old were refused. "Which driver opens it is not shown" (the machine's real-time scanner and indexer both act on new files; telling them apart needs a change of the machine's settings, which the brief excluded), and "that the left-out run of d08569e7 met this holder is not shown by any record of that run". The fix is in the test alone (change T): decoy (h) is written once with `solver_version` unknown instead of by a replace of a new `run.json`; `atomic.rs` and its 12 attempts are unchanged. A-N passes 20 of 20 without load and 20 of 20 under a load of new files. This is the same family as the edit-lock diagnoses of ADRs 0751 and 0758, whose model of loads and series ADR 0773 reuses ([[rcfd-mcp]], [[adr-thread-workspace-layer]] for F-288).
- **Open follow-ups (todo):** F-736 (first GitHub runs of the daily gate), F-764 (first real run of the feature-gated step), F-858 (the gate runs the edit-lock controls of ADR 0751 and keeps the project folder of a failed edit-lock test), F-859 (a forced control of the second resolve under the edit lock, ADR 0669 D1).
- **F-751 partial** (ADR 0669) stays partial: "Mutation M1 (no claim file) survived both forced-contention controls", later caught by F-755 (ADR 0676).

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0127](../../docs/adr/0127-test-gate-efficiency.md) | 2026-10-26 | registration + results (added by p3-records-a) | | A faster release test stage with the same tests (`scripts/gate_test.py`, one process per test, one pool) | G1, G2, G5 met; G3 (66.8 min against at most 51) and G4 (peak 2184 MB against 1.5 GB) missed, goals not moved | 0129 |
| [0129](../../docs/adr/0129-test-time-limit-jobs5-measurement-limit-audit.md) | 2026-10-27 | registration + results (added by p3-records-a) | | Per-test time limit, `--jobs 5` measurement, audit of tests that cannot fail | T1 to T4 met; `--jobs 5` 13.2 % faster, not the required 25 %, default stays 3; audit 80 hits, 2 real weaknesses, A4 mutation spot-check 10 of 12 | 0740 (per-test limits) |
| [0653](../../docs/adr/0653-f723-registration-daily-full-gate-with-failures-attributed-to-adrs.md) | 2026-11-16 | registration (hub), 4 amendments | F-723 | Daily full gate in CI with failures attributed to ADRs | done; fmt fix (os error 206), decide job, attribution, index; shards default 4 from 2026-11-21 | |
| [0669](../../docs/adr/0669-f751-registration-the-two-failures-of-the-first-daily-gate.md) | 2026-11-16 | registration (diagnosis + fix) | F-751 | The two failures of the first daily gate (run 43919555816) | partial; second resolve under the lock and a claim file; 200 of 200 after (59 failures before); M1 survived | 0676 |
| [0676](../../docs/adr/0676-f755-registration-a-forced-control-of-the-edit-locks-starvation-and-of-a-lock-left-behind.md) | 2026-11-17 | registration | F-755 | Forced control of the lock's starvation and of a lock left behind | done; old acquire starves 10 of 10, claim accepted 10 of 10 within 177 ms; drop retries (527 ms) | |
| [0681](../../docs/adr/0681-f761-registration-the-daily-gate-runs-the-feature-gated-edit-lock-controls.md) | 2026-11-17 | registration | F-761 | The daily gate runs the `test-hooks` controls | done by local evidence; "feature isolation" step and a separate test step | F-764 (todo) |
| [0740](../../docs/adr/0740-f849-registration-a-registered-time-limit-for-two-immersed-solid-tests-of-the-full-gate.md) | 2026-11-19 | registration | F-849 | A registered time limit for two immersed-solid tests | partial at the addendum, G1 met 2026-11-21 (item done) | |
| [0749](../../docs/adr/0749-f854-registration-two-wall-clock-limits-measured-so-that-load-alone-does-not-fail-them.md) | 2026-11-20 | registration | F-854 | Two wall-clock limits that load alone fails | partial; A4 fixed; I2 left with its finding, Q1 to the maintainer | 0750 |
| [0750](../../docs/adr/0750-f854-registration-i2-times-an-increment-of-a-store-with-spare-capacity.md) | 2026-11-20 | registration (follow-up under rule 13) | F-854 | I2 times an increment of a store with spare capacity | done; 0 of 30 failures under each load, margin 58 to 167; row G met | |
| [0751](../../docs/adr/0751-f855-registration-the-edit-locks-f2-lockbusy-diagnosed-under-loads.md) | 2026-11-21 | registration (diagnosis) | F-855 (ledger first read F-002) | F2's `LockBusy` in a full gate, diagnosed under loads | partial; journal-lock and claim removals retried; not reproduced without a forced cause; M2r, M6r survived | 0758 |
| [0753](../../docs/adr/0753-f853-registration-every-call-of-tmp-in-restart-w19c-gets-its-own-folder.md) | 2026-11-21 | registration | F-853 | Three `restart_w19c` tests share one temporary folder | done; 6 of 60 failures before, 0 of 60 after | |
| [0758](../../docs/adr/0758-f855-registration-a-stall-of-the-waiter-or-of-the-holder-separated-from-a-fault-of-the-edit-lock.md) | 2026-11-22 | registration | F-855 | A stall of the waiter or the holder separated from a fault of the lock | done by its rule, written partial by the lead; change W and amended F2 | |
| [0759](../../docs/adr/0759-f866-registration-the-control-of-l1-decides-and-asserts-on-the-ten-listings.md) | 2026-11-21 | registration | F-866 | The negative control of `listing_budget` L1 fails when ten listings take just under the budget | partial by its rule (link of line 11), written done by the lead | |
| [0773](../../docs/adr/0773-f876-registration-the-replace-of-run-json-refused-in-adopt-f288.md) | 2026-11-22 (results 2026-11-23) | registration (diagnosis) + amendments 1 and 2 | F-876 | Why the replace of `run.json` in `adopt_f288` A-N was refused with access denied after 12 attempts | done; cause H2 (a kernel-mode handle of pid 4 on a new file), change T in the test; A0 and A1 20 of 20; committed mutation M3 caught | two follow-ups proposed, not entered |

## Key numbers and limits

- **fmt on Windows** (ADR 0653): `cargo fmt --all` handed every target root "(63802 characters on the runner) to one rustfmt process, above the Windows limit of 32767 (os error 206)"; the step now runs `cargo fmt -p` per member (largest 20031 characters).
- **Index at F-723** (ADR 0653): 1053 of 1134 integration-test binaries (92.9 %) and 6986 of 7472 tests (93.5 %) map to an ADR; 20 of 20 sampled binaries correct; 10 of 10 mutations caught.
- **Shards** (ADR 0653, the lead's decision of 2026-11-19): four shards "2 h 32 min against 4 h 41 min of wall time, for about one and a half times the runner minutes (417 against 281)".
- **F-849 limit** (ADR 0740): 10 800 s = "twice the fastest measured time, 2247 s, times the largest runner ratio seen, 2.4"; G1 times 2299 / 2058, 3698 / 3801, 2146 / 3885 s in runs 44842718750, 44854704415, 44865647753.
- **F-751 fix** (ADR 0669): "a claim file beside the lock that a writer waiting 50 ms creates and the others respect (stale after 4 s)"; with the gate's per-test command, 200 runs each: "old code 59 failures (55 refused, 2 access denied, 2 LockBusy), fixed code 200 of 200 passed".
- **F-755** (ADR 0676): under the delay hook "the old acquire starved to LockBusy in 10 of 10 trials and the claim was accepted in 10 of 10 within 177 ms"; `EditLock::drop` retries a transient removal error "with the default retry (527 ms)".
- **F-854 I2** (ADRs 0749, 0750): 99 percent of what the old I2 timed "is the reallocation of a cloned store, not `push` (0.013 ms)"; the new test's largest medians 0.0135, 0.0220 and 0.0346 ms under no load, 16 spinning processes and 16 memory copiers; "the test catches only a push that costs 4.9 microseconds more per call".
- **F-855 change W** (ADR 0758): "a step between two looks now counts for at most 100 ms of EDIT_LOCK_WAIT"; under a build beside the test "the racer held the lock for 2.3 to 2.6 s in 4 of 20 runs on the old and the new library alike".
- **F-866** (ADR 0759): "the old control fails exactly when $T \le 500\ \text{ms} < 10\,w$"; the failed gate printed a mean of ten of 49.8 ms. The lead corrected a broken link in line 11 of the registration in the merge commit and entered the ADR in `docs/process/exceptions/adr-rewritten.txt`.

## Open questions

- **ADR 0773 Q1 (open at 09647baa):** A-N catches an adopted decoy "that is paused only by not ending" (`wait_end` has no time limit), "so a mutation of the key cannot be a committed mutation, and a regression of the key would hang the gate rather than fail it"; the agent stopped mutation M1 after 81 minutes. Options: (a) a follow-up item that lets A-N end on such an adoption; (b) leave it. Recommendation (a), shared by the lead. A second proposed follow-up covers failures seen under the load of new files and not diagnosed: "a read of status.json failed with os error 2" and "an adopted run reported queued and counted failed" (reads and waits of `rcfd-workspace` and `rcfd-runner`). Neither is entered at 09647baa.
- The lead records in ADR 0773 that "the agent of this item ran on Claude Opus 5.5, not on the model its brief names".
- No question of this thread is open with the maintainer. The lead's decisions are recorded in ADRs 0751 and 0758: change C of ADR 0751 not made; change W kept; "The ignored record of the old body of F2 is kept ... The lead has told the maintainer of it in the report of this stretch, so that he can overrule" (ADR 0758).
- Still to be seen on a hosted runner: the amended F2 (F-855), the feature-gated step (F-764), the first daily runs of ADR 0653's paths (F-736).
- The registration of ADR 0653 says "Not yet accepted by the maintainer" (as do ADRs 0669, 0676, 0681, 0740).

## Before the daily gate: the first Linux full gate (ADR 0137; added by job p3-records-b)

[ADR 0137](../../docs/adr/0137-linux-gate-platform-values.md) (registered 2026-10-28; index item F-866) answered the first `Full gate` run on `ubuntu-latest`, which "failed 16 tests". Every expected value had been recorded on Windows, "and libm's last bits differ between the Microsoft CRT and glibc". Rule: a test that asserts bit identity gets an x86_64 Linux expected value next to the Windows one "only if, in one Linux container, that value comes out at the test's reference commit and at HEAD and, where the CI log prints it, equals the CI value". By an amendment for the maintainer to confirm, "the L1 negative control applies the registered rule of ADR 0128 finding 12 on every run": the same L1 of `listing_budget` whose control F-866 (ADR 0759, above) later made decide and assert on the ten listings.

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0137](../../docs/adr/0137-linux-gate-platform-values.md) | 2026-10-28 | registration | F-866 | Per-platform expected values, the L1 control rule per run, reported defects | in Docker 12 of 16 pass; S4a, S4b (a Unix production defect), C3 (1 ulp) and P1 (unregistered check) reported; on Windows the 14 changed tests pass with the registered values; no production code changed | 0143 (S4a, S4b fixed; C3, P1 Linux known misses), 0759 (L1 control) |

- L1 in Docker printed "`10 w = 90.9 ms <= budget: the budget catches a listing 55x slower than measured`"; on Windows "worst listing 80.6 ms, the listing 10 times 844 ms > 500 ms".
- "The next `Full gate` run on `ubuntu-latest` is the real check: the container is Debian bookworm (glibc 2.36), the CI image Ubuntu 24.04 (glibc 2.39)". The example-project stores that failed the same gate got a Linux-built tree instead ([[adr-thread-example-projects-and-linux-values]]); the runner follow-up is on [[adr-thread-runner-records-and-run-sites]].

## Related

- [[gate]], [[edit-lock]], [[test-to-adr-index]], [[process-ledger]], [[push-main]], [[known-miss]], [[rcfd-project]]
- [[adr-thread-test-to-adr-attribution]], [[adr-thread-process-measures-and-roadmap-form]]
- [[roadmap-quality-docs-and-validation]]
- Earlier history (added by p3-records-a): the gate's test runner `scripts/gate_test.py` comes from ADR 0127, which found that "Running the test binaries takes 92 % to 95 % of the gate's test stage" and runs "each non-ignored test as its own process, from one pool of `--jobs` processes. No test is skipped and no result is cached, and the original `cargo test` remains the reference." ADR 0129 added the per-test time limit after "A hung test held a gate slot for about 3.5 h on 2026-10-26"; its two mutation-check misses (ADR 0123 S7 and ADR 0119 R3) "are reported as findings with nothing changed". See [[mutation-testing]], [[doc-test-process]].
