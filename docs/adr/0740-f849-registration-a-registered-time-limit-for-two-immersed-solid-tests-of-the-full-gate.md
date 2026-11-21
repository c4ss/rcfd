# ADR 0740: F-849 registration: a registered time limit for two immersed-solid tests of the full gate

Status: **REGISTERED 2026-11-19, written before any code of F-849 and before any run of its checks.** Not yet accepted by the
maintainer. Results go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-849 (deps F-219, F-346, F-723). Number 0740 given by the lead. It answers the brief
`docs/adr/briefs/f849-gate-test-time-limit.md` under the rules `docs/process/agent-rules.md`, version 3. Branch
`f849-gate-test-time-limit` from main b52b882a, scratch `w49i-work/f849-gate-test-time-limit-scratch`.

Size (agent): partial; 0; 2; 0; 345000

Every choice below that the maintainer would normally be asked about is taken on the maintainer's standing instruction of
2026-11-11 (take the recommendation). No question is left open by rule 6: nothing here changes a default result, a golden or
a hash table, and the scope is the item's.

## What F-849 asks

The acceptance, binding, verbatim:

> Every limit not stated here is registered in the ADR before the run. Each of the two tests (immersed_solid_f219::a3_two_d_two_strouhal_number and immersed_unsteady_force_f346::a2t_c1_two_d_two_on_t) finishes in at most half of its time limit in each of three consecutive full gates on windows-latest, by one of two routes chosen and justified in the registration: (a) a shorter set-up, with the registered physical checks of ADR 0218 and ADR 0249 (Strouhal number, maximum drag and lift, their limits) unchanged and the measured values before and after recorded; or (b) a higher time limit for these two tests only, registered with the measured times it rests on. No test is ignored, skipped or removed, and no numeric limit of ADR 0218 or ADR 0249 changes.

Clauses: **c0** every limit registered here; **c1** each of the two tests takes at most half of its time limit in each of
three consecutive full gates on windows-latest; **c2** the route is chosen and justified here, and for route (b) the limit is
for these two tests only and rests on measured times; **c3** no test is ignored, skipped or removed and no numeric limit of
ADR 0218 or ADR 0249 changes.

## What was read and computed before this text (disclosed)

Read: the brief; the rules; the roadmap entry of F-849 with its note; ADR 0218, ADR 0249 and ADR 0653 in full;
`scripts/gate_test.py`, `.github/workflows/gate.yml`, `docs/process/tolerances.md`; of `scripts/gate_attribute.py` the re-run
(`run_test`, `attribute`, the options); `scripts/test_gate_test.py`, `scripts/test_gate_hooks_f761.py`; of
`scripts/push_main.py` the handling of a mutation patch; the two test functions and their headers.

Run: no code of F-849 exists and no check below was run. Read-only calls to GitHub (`gh run list`, `gh run download` of the
log artifacts, `gh api .../jobs` for the step times) for the five gate runs since 2026-11-16, and two scratch Python programs
(`spread.py`, `spread2.py`, in the scratch folder, not committed) that divide the seconds of each test in those logs by its
seconds in the fastest of them. Their numbers are below and fix the limit.

### The measured times (windows-latest, release, three tests at once)

| Gate run | Commit | Kind | `a3_two_d_two_strouhal_number` | `a2t_c1_two_d_two_on_t` | Runner against the fastest run (median ratio of the tests that took at least 100 s there; largest ratio of a test of at least 300 s) |
| --- | --- | --- | --- | --- | --- |
| 43919555816 | d55c43c4 | one job | 2829 s | 2543 s | 1.31; 1.42 |
| 44060003415 | d56933a8 | one job | time-out at 3600 s | time-out at 3600 s | 1.61; 1.92 (from `progress.log`; the step ran out of its 290 min, so no `times.tsv`) |
| 44276505780 | 205889bf | one job | 2066 s | 2247 s | 1 (the base of the ratios) |
| 44316638137 | 39269587 | one job | 3060 s | 3230 s | 1.31; 1.48 |
| 44361504875 | dd2ab837 | shard 1 of 4 | (not in this shard) | 2880 s | 1.28; 1.36 |
| 44361504875 | dd2ab837 | shard 2 of 4 | time-out at 3600 s | (not in this shard) | 1.76; 1.95 |
| 44361504875 | dd2ab837 | shard 3 of 4 | | | 0.98; 1.23 |
| 44361504875 | dd2ab837 | shard 4 of 4 | | | 1.56; 1.81 |

The roadmap note names three of the `a3` times and two of the `a2t` times. The logs add the run 43919555816 and the gate of
2026-11-17 (44060003415), in which both tests timed out: `a3` timed out in two of the five gates and `a2t` in one.

What the ratios say:

- Eight runners, medians from 0.98 to 1.76: the spread between runners is a factor of 1.8 in the median. The two tests
  follow the runner (their own ratios in the three complete runs are 1.13 to 1.48, inside the range of the other long tests).
- The largest ratio of any single test of at least 300 s is 1.95, of at least 100 s 2.38 (one test of 48, run 43919555816).
- On the slowest runner seen the two tests would take about 1.76 to 1.95 times their fastest time: `a3` 3640 to 4020 s,
  `a2t` 3950 to 4370 s. That agrees with the three time-outs. The true times of the timed-out runs are not known.

Other facts used. In a gate of one job no `--order` file is given, so every test has the limit `--timeout-max` (3600 s); in
a sharded gate `--timeout-min 3600` gives the same. The attribution step re-runs a failed test with `--rerun-timeout` 3600 s
inside a step of 40 min (2400 s): in shard 2 of run 44361504875 that step was killed after 40 min in the re-run of `a3` and
wrote no `attribution.json` (the step concluded `failure`, "upload attribution" was skipped). Local times of the two tests
(ADR 0218 addendum 1, ADR 0249 addendum 1; this machine, other jobs running): 1729 s and 2234 s.

## The route (c2): (b), a higher limit for these two tests only

Route (a) is not taken, for three reasons.

1. It cannot meet the acceptance by itself. The acceptance forbids ignoring, skipping or removing a test, and rule 2
   forbids editing one. A shorter test would be a new test beside the two; the two long tests stay in the gate with their
   run time, and their limit is then route (b) anyway.
2. It would add run time to a gate that is short of it, not remove any.
3. The two tests are the registered records of ADR 0218 A3 and ADR 0249 A2T and C1r (the latter holds the run of ADR 0218
   to its printed digits). A shorter set-up (fewer steps, a coarser mesh or a larger time step) is a different computation
   with different measured values, which ADR 0249 has shown to move the lift maximum by 2.7 percentage points between two
   meshes of the same spacing at the cylinder. Nothing is wrong with the result of either test; only the time limit of the
   runner is too close.

Route (b) changes no Rust file. What it costs: a hang of one of these two tests is found after the higher limit, not after
one hour.

## Design (fixed now)

**D1. The limit: 10 800 s for each of the two tests.** Rule: twice the product of the test's fastest measured time and the
largest ratio seen for any test of at least 100 s on any of the eight runners, 2.38, rounded up to 2.4; the larger of the two
values, rounded up to a quarter of an hour, for both. `a2t`: 2 x 2247 x 2.4 = 10 786 s; `a3`: 2 x 2066 x 2.4 = 9917 s; hence
10 800 s (3 h). Half of it, the number of the acceptance, is 5400 s: 2.40 times the fastest `a2t` and 2.61 times the fastest
`a3`, against a largest runner median of 1.76 and a largest long-test ratio of 1.95. The estimate for the slowest runner seen
is 4370 s (`a2t`), so the margin of the acceptance's half is 1.24 on that runner and 1.67 to 2.6 on the four measured runs.
The limit is not derived from one run: it uses the fastest time of each test and the spread of eight runners.

**D2. The table.** New file `scripts/gate_limits.tsv`: a header `binary`, `test`, `limit_s`, `adr`, lines starting with `#`
as comments, and two rows, `immersed_solid_f219` / `a3_two_d_two_strouhal_number` / 10800 / 0740 and
`immersed_unsteady_force_f346` / `a2t_c1_two_d_two_on_t` / 10800 / 0740. A row names one test by the binary stem and the
exact test name that `times.tsv` holds.

**D3. `scripts/gate_test.py --limits FILE`.** With the option, a test named by a row has the limit of its row, whatever the
`--order` record and `--timeout-factor`, `--timeout-min`, `--timeout-max` give; every other test has the limit it has today
(`limit_s`); `--timeout-max 0` still turns every limit off. The run prints `gate_test: limits FILE: n rows, m name a test of
this run`. A malformed table (a missing column, a limit that is not a positive number, a test named twice) stops the run
before the build. Without the option nothing changes: the default of the option is no table.

**D4. The workflow.** The tests step passes `--limits scripts/gate_limits.tsv` (one job and shards alike). The step of the
test-hooks feature does not (it runs one binary of another crate).

**D5. The attribution re-run (ADR 0653 Q3: the job limit of 340 min has to hold the longer re-run).** Today a re-run can
take 3600 s in a step of 2400 s, and with D1 it could take 10 800 s. In `scripts/gate_attribute.py attribute`:

- `--limits FILE`: the time limit of the re-run of a test named in the table is its row's limit; of every other test
  `--rerun-timeout` as now.
- A re-run budget in seconds, either `--rerun-budget S` or computed from `--job-start-file F` (a file holding the epoch
  second at which the job started) and `--job-minutes M`: `min(12 000, 60 M - (now - start) - 900 - bisect)`, not below 0,
  where `bisect` is 60 times `--bisect-minutes` when `--bisect` is given and else 0; 1500 s when the file cannot be read (what
  the old step of 40 min could hold). 900 s are kept for the candidates, the JSON, the upload and the end of the job. The
  cap of 12 000 s holds one re-run of 10 800 s and 20 min of others.
- With a budget, the re-runs are made in the order of their limits, shortest first (file order among equals), each with
  the limit `min(its limit, what is left of the budget)`. A re-run cut by the budget below its own limit has the outcome
  `TIMEOUT (cut at N s by the re-run budget; its limit is L s)`; when less than 60 s are left the outcome is `not run (re-run
  budget used up)`. Neither is flaky. The attribution always reaches its JSON and summary.
- Without these options the re-runs are what they are today (file order, `--rerun-timeout` each, no budget).

In `gate.yml`: the fmt step, the first `run` step of the job, first writes `date +%s` to `$RUNNER_TEMP/gate_job_start`; the
attribution step passes `--limits scripts/gate_limits.tsv --job-start-file "$RUNNER_TEMP/gate_job_start" --job-minutes 340`
and its `timeout-minutes` goes from 40 to 240 (14 400 s, above the cap of 12 000 s plus the default bisect of 1800 s). The
job keeps `timeout-minutes: 340` and its twelve steps. Arithmetic: the long re-run fits whole when the attribution starts
at most 20 400 - 900 - 10 800 = 8700 s (145 min) into the job. The jobs of the four-shard trial reached it after 68 to 111
min, so a shard holds it. A gate of one job reaches it after about 3.5 h or more; there the re-run of one of the two tests
is cut at what is left (at most about 6600 s), which is recorded as such and loses nothing else.

**D6. The check of c1 from the logs: `scripts/gate_limit_check.py`.** `python scripts/gate_limit_check.py --limits
scripts/gate_limits.tsv GATE GATE GATE`, each `GATE` a folder that holds the downloaded log artifacts of one gate run (one
`times.tsv`, or one per shard; `times_hooks.tsv` is not read). For every row of the table and every gate it requires exactly
one row of that test in the gate's times, with outcome `ok` and seconds at most 0.5 times the row's limit. It prints one
line per test and gate (seconds, half limit, verdict) and exits 0 only if at least three gates were given and every line
is met. A test that is missing, ignored, failed, timed out, listed twice or above half its limit fails the check. That the
three gates are consecutive full gates on windows-latest at commits that hold this work is for the lead to see from the
run list; the script prints the folders it read.

**D7. Documents.** `docs/test-process.md`: the table and the option in section 4, the re-run budget in section 7.2. A dated
pointer to this ADR is appended to ADR 0653.

**What stays.** No file under `crates/` changes; no test is added, edited, ignored or removed; no limit of ADR 0218 or ADR
0249 is touched; `gate_test.py` and `gate_attribute.py` without the new options behave as before; no new dependency (Python
standard library only).

## Checks

The limit column is fixed now. Kinds are those of rule 14. "Acceptance" in the source column is the acceptance of F-849;
"budget row" is the row "Run time or memory, as a budget" of `docs/process/tolerances.md`; "bit-identity row" is its row
"Bit-identity of default output".

| Id | Check | Limit | Source of the limit | Estimate | Margin (limit / estimate) | Kind |
| --- | --- | --- | --- | --- | --- | --- |
| G1 (c1) | `gate_limit_check.py` on the logs of three consecutive full gates on windows-latest at commits that hold this work | exit 0: each of the two tests once per gate, outcome `ok`, at most 5400 s | acceptance (half of the limit of D1); budget row | 2066 to 3230 s measured, about 4370 s on the slowest runner seen | 1.24 to 2.6 | unchanged default output |
| G2 (c2) | the table of D2 as committed | exactly the two rows of D2, 10 800 s each; each names a `#[test]` function of `crates/rcfd-fv/tests/<binary>.rs` that carries no `#[ignore]` (read from the source) | acceptance ("these two tests only"); D1 | | exact | code verification |
| N1 (c3) | `git diff --name-only main...HEAD -- crates` | no file | acceptance; bit-identity row | 0 files | exact | unchanged default output |
| N2 (c3) | the two test binaries built in release from this branch: `--list` names the two tests, `--list --ignored` does not | both listed, neither ignored | acceptance | | exact | unchanged default output |
| T1 | `immersed_solid_f219 a3_two_d_two_strouhal_number --exact --test-threads=1`, once, release, this machine, with T2 running beside it | passes (its registered assertions, unedited); at most 5400 s | acceptance (half of the limit); budget row | 1729 s (ADR 0218) | 3.1 | unchanged default output |
| T2 | `immersed_unsteady_force_f346 a2t_c1_two_d_two_on_t --exact --test-threads=1`, once, release, this machine, with T1 running beside it | passes (its registered assertions, unedited, C1r to the printed digits); at most 5400 s | acceptance (half of the limit); budget row | 2234 s (ADR 0249) | 2.4 | unchanged default output |
| L1 | the table reader: the committed table; a comment and an empty line; and an error for each of a missing column, a limit that is not a number, a limit of 0, a test named twice | parsed to the two keys; each bad table refused with a message that names the line | D3 | | exact | code verification |
| L2 | the limit of a test: named in the table (with and without an `--order` record, with any factor, minimum and maximum); not named (the fifteen cases of `test_gate_test.Clamp`); a test of the same binary with another name; a test of the same name in another binary; `--timeout-max 0` | the row's limit; `limit_s` of today; `limit_s`; `limit_s`; no limit for both | D3; bit-identity row | | exact | unchanged default output |
| L3 | `gate_limit_check.py` on fixture folders: three good gates of one job; three good sharded gates; and one defect each: seconds above half the limit, exactly half (met), `TIMEOUT`, `FAILED`, `ignored`, the row missing, the test in two shards, two gates only | exit 0 for the good sets and for exactly half; exit 1 for every defect, the output naming the test and the gate | D6; acceptance | | exact | code verification |
| L4 | the re-run of the attribution with a fake runner: without the new options; with `--limits`; with a budget that holds all; that cuts the last; that is used up; the budget formula (start file, bisect, cap, missing file) | without options the calls of today (file order, 3600 s each); the table limit for the named test only; shortest limit first; the cut outcome with its seconds and limit; `not run (re-run budget used up)`; neither flaky; the formula of D5 to the second; the JSON written in every case | D5; bit-identity row | | exact | code verification |
| L5 | `gate.yml` as text: the tests step passes `--limits scripts/gate_limits.tsv` once, in the command both branches share; the hooks step does not; the fmt step writes the start file before anything else; the attribution step passes the three options with the job's own `timeout-minutes`; the attribution step's limit in seconds is at least 12 000 + 1800; the cap of D5 is at least the largest limit of the table | as stated; 340; 14 400 >= 13 800; 12 000 >= 10 800 | D4, D5; ADR 0653 Q3 | | exact | code verification |
| E1 | end to end (with `GATE_TEST_E2E=1`): a temp crate with two tests that each sleep 20 s, through `gate_test.py --timeout-min 5 --timeout-max 5 --limits T`, `T` naming one of them with 120 s | the named test `ok` after at least 20 s; the other `TIMEOUT` within 15 s; exit not 0; the `limits` line says 1 row, 1 names a test | D3; acceptance ("these two tests only") | | exact | code verification |
| E1n (negative control of E1) | the same without `--limits` | both `TIMEOUT` | D3 | | exact | code verification |
| W1 | the existing tests of the gate scripts, unedited: `scripts/test_gate_test.py` (with `GATE_TEST_E2E=1`), `test_gate_hooks_f761.py`, `test_gate_shards.py`, `test_gate_attribute.py` | every test passes | rule 7; bit-identity row | | exact | unchanged default output |
| W2 | `actionlint` on `.github/workflows/gate.yml` | 0 findings | ADR 0653 S4 | | exact | code verification |
| W3 | `cargo test -p rcfd-cli --release --test docs_site` | passes | rule 7 (`docs/` is touched) | | exact | unchanged default output |

The new unit tests are in `scripts/test_gate_limits_f849.py` (G2, L1 to L5, E1, E1n). No row is of kind validation: the
acceptance claims no agreement with a measured case.

Negative controls: the defect fixtures of L3 (each must make the check fail), E1n, and in L2 the two near misses (the same
binary with another test, the same test name in another binary).

Mutations (rule 16), each applied alone to a scratch copy of the final scripts, `python -m unittest
scripts/test_gate_limits_f849.py` run once (with `GATE_TEST_E2E` unset, so the unit tests must catch them), the file
restored and compared byte for byte:

| Id | Mutation | Must fail |
| --- | --- | --- |
| M1 | `gate_test.py`: the table is not consulted (every test gets `limit_s`) | L2 |
| M2 | `gate_test.py`: the table is matched by binary only | L2 |
| M3 | `gate_limit_check.py`: the seconds are compared with the whole limit, not half | L3 |
| M4 | `gate_limit_check.py`: a missing row counts as met | L3 |
| M5 | `gate_attribute.py`: the budget does not cap a re-run | L4 |
| M6 | `gate.yml`: `--limits` removed from the tests step | L5 |

Committed (rule 16): M1 as `scripts/test_gate_limits_f849.mutation.patch`, first line `# expect-fail: python -m unittest
scripts/test_gate_limits_f849.py`.

Heavy runs: T1 and T2, two, started together (in the gate they overlap as well). The release build of the two test
binaries of `rcfd-fv` and of `docs_site` is a build, not a heavy run. No run of the workflow is started.

## Decision rule

- **done**: G1 met on three consecutive full gates, and G2, N1, N2, T1, T2, L1 to L5, E1 with E1n, W1 to W3 met and every
  mutation caught.
- **partial**: everything but G1 met and every mutation caught, G1 not yet run (the expected outcome here: the three gates
  need the merge and three days or three dispatches, which the lead reads with D6); or G1 met and another check missed.
- **todo**: a check of L1 to L5 or E1 missed without a diagnosed cause, or N1 or N2 missed.

T1 or T2 failing one of its own registered assertions would be a finding about main and not about this item (no Rust file
changes); it is reported and the item is not done on it. T1 or T2 above 5400 s on this machine is a miss with the number
recorded; the limit of D1 is not changed after it.

A failed G1 after the merge (a gate in which one of the two tests takes more than 5400 s and passes, or times out at
10 800 s) is a miss of the acceptance with the limit intact: it is diagnosed from the runner ratios of that gate, and a
change of the limit is a new registration under F-849 (rule 13).

## Not covered

- The three gates of c1: nobody can run them from here.
- A gate of one job on a slow runner can run out of the 290 min of its tests step whatever the limit of one test is (the
  gate of 2026-11-17, run 44060003415: build 34 min, tests still running at 4 h 10 min). That is the reason for the shards
  (ADR 0653 Amendment 3) and is not changed here. In such a gate no `times.tsv` is written, and G1 fails on it.
- In a gate of one job the tests start in the order of the binaries, the two tests about 2 h 20 min to 3 h 10 min after
  the start of the tests step. Starting them first is not done here: it changes the order of the run.
- The true times of the three timed-out runs. The estimate of D1 for the slowest runner is a product of ratios.
- A runner more than 2.4 times slower than the fastest seen.
- The run time of the tests themselves: nothing is made faster.
- The other long tests of the gate (`a1_a2_two_d_one_drag_and_lift_on_m3` 1159 to 1684 s, `a2_two_d_two_on_m2` 963 to 1647 s,
  `immersed_piso_stop_f355::a_two_d_two_on_m2_strouhal` 991 to 1994 s): on the slowest runner seen they stay below 3600 s by
  a factor of 1.8 or more and get no row.
- The bisect of the attribution: its own budget and its probe limit are as before.

## Amendment 1 (2026-11-19, before the corrected run of N1): the command of N1 named a ref that is not the base

N1 as registered is `git diff --name-only main...HEAD -- crates`. Its first run listed four files under `crates/`
(`rcfd-api/tests/case_schema_page.rs`, `rcfd-cli/tests/docs_site.rs`, `rcfd-runner/src/site/remote.rs`,
`rcfd-runner/tests/sites_adr0739.rs`). None of them is touched by this branch: in this checkout the local ref `main` is
39269587, 44 commits behind b52b882a, the commit this branch was made from (the header of this ADR), so the command compared
with an older tree. That run is void: a defective set-up, the command and not the branch. It was the only check run with
that ref.

Corrected check, run once after this amendment is committed:

| Id | Check | Limit | Source of the limit | Estimate | Margin | Kind |
| --- | --- | --- | --- | --- | --- | --- |
| N1' (c3; replaces N1) | `git diff --name-only b52b882a HEAD -- crates` | no file | acceptance; bit-identity row | 0 files | exact | unchanged default output |

The limit is unchanged. Known before this run: the three commits of the branch so far were made by me and hold no file
under `crates/`.

## Addendum 1: results (2026-11-19)

Measured against the limits registered above and in Amendment 1; no limit was changed. Commits: the registration 607b416b;
the code and tests ea354d95; the committed mutation 2caadc62; Amendment 1 2a7f5220, before the run of N1'. T1 and T2 ran on
the binaries built from 607b416b; no Rust file differs in the later commits.

**Built.** `scripts/gate_limits.tsv` (the two rows of D2); `scripts/gate_test.py` (`read_limits`, `test_limit`, the option
`--limits`, the `limits` line); `scripts/gate_limit_check.py` (D6); `scripts/gate_attribute.py` (`--limits`,
`--rerun-budget`, `--job-start-file`, `--job-minutes`; `rerun_budget`, `read_start`, `rerun_all`);
`.github/workflows/gate.yml` (D4, D5); `scripts/test_gate_limits_f849.py` (28 tests, 2 of them end to end) and its
mutation patch; `docs/test-process.md` sections 4 and 7.2; a pointer appended to ADR 0653. No file under `crates/`.

### Results

| Id | Limit | Measured | Met |
| --- | --- | --- | --- |
| G1 (c1) three consecutive full gates on windows-latest | each test `ok`, at most 5400 s | **not run**: the gates need the merge | not run |
| G2 (c2) the committed table | the two rows, 10 800 s, each a `#[test]` without `#[ignore]` | as required | yes |
| N1 (c3) | no file under `crates/` | void: defective command (Amendment 1) | not counted |
| N1' (c3) `git diff --name-only b52b882a HEAD -- crates` | no file | no file | yes |
| N2 (c3) `--list` and `--list --ignored` of the two release binaries | both listed, neither ignored | both listed, neither ignored | yes |
| T1 `a3_two_d_two_strouhal_number`, once, release, this machine, beside T2 | passes; at most 5400 s | passed; 2033.8 s (St 0.30073, max `C_D` 3.2462, max `C_L` 1.0113, the numbers of ADR 0218) | yes |
| T2 `a2t_c1_two_d_two_on_t`, once, release, this machine, beside T1 | passes; at most 5400 s | passed; 2055.4 s (C1r held: 3.24624, 1.01134, 0.30073; D1p 1.2e-7 and 1.9e-6) | yes |
| L1 the table reader | parsed; each bad table refused with its line | 3 tests pass (eight bad tables) | yes |
| L2 the limit of a test | the row's limit; `limit_s` for every other test; off with `--timeout-max 0` | 3 tests pass (15 clamp cases, 5 near misses) | yes |
| L3 `gate_limit_check.py` on fixtures | exit 0 for the good sets and exactly half; exit 1 for every defect | 10 tests pass | yes |
| L4 the re-runs of the attribution | as registered | 8 tests pass | yes |
| L5 `gate.yml` as text | as registered; 340; 14 400 >= 13 800; 12 000 >= 10 800 | 2 tests pass | yes |
| E1 end to end, one of two 20 s tests named with 120 s, the limit 5 s | named `ok` after at least 20 s; the other `TIMEOUT` within 15 s; exit not 0; the `limits` line | `ok`; `TIMEOUT` after 5 s; exit 101; `1 rows, 1 name a test of this run` | yes |
| E1n the same without the table | both `TIMEOUT` | both `TIMEOUT` | yes |
| W1 the existing tests of the gate scripts, unedited, with `GATE_TEST_E2E=1` | every test passes | 37 run, OK, none skipped | yes |
| W2 `actionlint` on `gate.yml` | 0 findings | 0 findings | yes |
| W3 `cargo test -p rcfd-cli --release --test docs_site` | passes | 2 passed (run before this addendum was appended) | yes |

The estimates against the measurements: T1 1729 s estimated, 2034 s measured; T2 2234 s estimated, 2055 s measured. On this
machine the two tests take 0.38 of the acceptance's 5400 s.

### Mutations

Each alone on a scratch copy (`mutate.py` in the scratch folder: an exact replacement checked to occur once),
`python -m unittest scripts/test_gate_limits_f849.py` once without `GATE_TEST_E2E`, the file restored and equal byte for
byte after each. The unmutated copy passes.

| Id | Must fail | Result |
| --- | --- | --- |
| M1 the table not consulted | L2 | caught: `test_l2_a_named_test_has_the_limit_of_its_row_whatever_the_options_give` |
| M2 the table matched by binary only | L2 | caught: `test_l2_every_other_test_has_the_limit_it_had` |
| M3 the whole limit, not half | L3 | caught: `test_l3_seconds_above_half_the_limit` |
| M4 a missing row counts as met | L3 | caught: `test_l3_a_missing_row` |
| M5 the budget does not cap a re-run | L4 | caught: two tests of L4 |
| M6 `--limits` removed from the tests step | L5 | caught: `test_l5_the_tests_step_passes_the_table_in_the_command_both_forms_share` |

Committed: M1 as `scripts/test_gate_limits_f849.mutation.patch`; applied with `git apply`, its command fails (1 failure),
removed with `git apply -R`, the tree clean.

### Tools

No Rust crate is touched, so `cargo fmt` and `cargo clippy` do not apply and were not run. `scripts/limit_audit.py` on the
new test file: 0 hits. `python scripts/process_report.py --check`: one failure, `docs/adr/index.md` is not current, which
is the lead's file (rule 9).

### Deviations and disclosures

- Amendment 1: N1 named the local ref `main`, which is not the base of the branch.
- Not in the registration: with a budget the attribution JSON gains the key `rerun_budget_s`; without a budget the JSON
  has the keys it had. With a budget all re-runs are made before the first bisect and the bisect's minutes count from the
  end of the re-runs; without a budget the order and the count are those of before.
- Not registered, printed only: `gate_limit_check.py` on the downloaded logs of real gates. Runs 43919555816, 44276505780
  and 44316638137 give "met" (2066 to 3230 s of 5400 s); a set with run 44361504875 (a time-out in shard 2) and run
  44060003415 (no `times.tsv`) gives "MISSED" and names both. Those gates ran with the old limit and are not G1.
- The new unit tests passed at their first run; the end-to-end tests, W1 and the mutations were each run once. W1 also ran
  the end-to-end test of `test_gate_attribute.py`.
- T1 and T2 ran before the code commit, on binaries of the registration commit (the Rust tree is the same).
- Not run: the workflow on GitHub; G1; the attribution step with its budget on a runner (the start file in `RUNNER_TEMP`,
  the step limit of 240 min); a sharded gate with the table.

### Size, actual

Size (agent): partial; 0; 2; 0; 360000

### Decision by the registered rule

G2, N1', N2, T1, T2, L1 to L5, E1 with E1n and W1 to W3 are met and all six mutations were caught; G1 was not run. The
rule gives **partial**. Clause c1 of the acceptance is not shown until `gate_limit_check.py` is met on three consecutive
full gates on windows-latest.

### Open questions for the maintainer

None. Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation): route (b); one limit of
10 800 s for both tests; a sharded gate counts as a full gate for G1 (the times of all its shards are read together).

### For the lead: roadmap entries

> Proposed status of F-849: partial.

> ADR 0740, 2026-11-19, partial: route (b), scripts/gate_limits.tsv gives immersed_solid_f219::a3_two_d_two_strouhal_number and immersed_unsteady_force_f346::a2t_c1_two_d_two_on_t a limit of 10800 s in the full gate (gate_test.py --limits; twice the fastest measured time, 2247 s, times the largest runner ratio seen, 2.4, from five gates and eight runners with three time-outs at 3600 s), every other test keeps 3600 s, no Rust file changes, both tests pass locally in 2034 and 2055 s, the re-runs of the attribution get a budget from what the 340 min of the job have left, and six of six mutations are caught; the three consecutive gates are not yet run.

> Gap of F-849: the three consecutive full gates on windows-latest of the acceptance have not run. After the merge, download the logs of three consecutive gates (gh run download RUN -p "gate-windows-latest-*" -D DIR, one folder per gate) and run python scripts/gate_limit_check.py --limits scripts/gate_limits.tsv DIR1 DIR2 DIR3: exit 0 means each of the two tests passed in at most 5400 s in each gate. A gate of one job on a slow runner can still run out of its 290 min tests step.

No follow-up item is proposed.

## Result of G1 (2026-11-21), by the lead

G1 is met: `python scripts/gate_limit_check.py --limits scripts/gate_limits.tsv` on the logs of three consecutive full
gates of main on windows-latest gives "met (2 tests, 3 gates)", exit 0. Each gate was dispatched with four shards and is
green; no full gate of main lies between them.

| Run | Commit | Passed; failed; ignored | `a3_two_d_two_strouhal_number` | `a2t_c1_two_d_two_on_t` |
|---|---|---|---|---|
| 44842718750 | 5637150a | 5936; 0; 1727 | 2299 s | 2058 s |
| 44854704415 | a9570de6 | 5936; 0; 1727 | 3698 s | 3801 s |
| 44865647753 | 5819a15c | 5936; 0; 1727 | 2146 s | 3885 s |

The limit of the check is 5400 s; the largest time is 3885 s, a margin of 1.39. The three commits differ in the ledger,
the roadmap and one ADR only, so the spread of 2058 to 3885 s for one test is the runner's. Two four-shard gates before
these three also met the limit (46cea4f8: 4034 s and 3805 s; e9378ecb: 3020 s and 2982 s) and are not counted, because
they were red on other tests (F-854, F-855). With this the decision rule gives done. Not shown by any run: a gate of one
job, which did not finish within its tests step on 2026-11-20 (run 44711018197). Artifacts:
`E:/projects/cfd/w49i-work/gate-5637150a`, `gate-a9570de6`, `gate-5819a15c`.
