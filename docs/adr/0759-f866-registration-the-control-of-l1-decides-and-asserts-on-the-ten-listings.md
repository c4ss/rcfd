# ADR 0759: F-866 registration: the control of L1 decides and asserts on the ten listings

Status: **REGISTERED 2026-11-21, written before any code of F-866 and before any run of its checks.**
Before this text nothing was built, no test was run and no file was changed by this work. What was done: reading, and the
arithmetic below on the numbers the gate printed. Results go in addenda at the end; nothing above them is changed after a
run.

It answers the brief `docs/adr/briefs/f866-listing-budget-negative-control.md` and follows `docs/process/agent-rules.md`,
version 5. Branch `f866-listing-budget-negative-control` from main cd74f388, scratch
`w49i-work/f866-listing-budget-negative-control-scratch`. The records of the test concerned are
[ADR 0122](0122-wave47a-project-folder-library.md) (L1, the budget), [ADR 0128](0128-audit-fixes-runner-claude-project-tsstore-render-api.md)
(finding 12 and amendment 3) and [ADR 0137](0137-linux-gate-platform-values.md) ("L1 negative control (ADR 0128 finding 12,
applied per run)").

## What is asked

The acceptance of F-866 (`docs/roadmap.json`): "Every limit not stated here is registered in the ADR before the run. The
negative control of rcfd-project listing_budget::l1_listing_hundreds_of_runs_within_budget no longer fails on a machine
whose worst listing times ten is above the budget while ten listings together are below it, shown by a control that fails
before the change with the times of the gate of 62a6fec3 (worst of three 51.59 ms, ten listings 498.15 ms, budget 500 ms);
the budget of 500 ms and the assertion that a listing stays within it are unchanged, and the control still fails a listing
that is ten times slower, shown by a mutation. The test is not ignored or retried."

The brief adds: the lead's reading of the defect checked; how close the listing is to a tenth of the budget in the three
gates; the new control cannot fail on a machine of any speed when the listing and the budget are sound; repeats on this
machine and under a registered load that brings the listing near a tenth of the budget; three mutations (the budget raised
tenfold, the listing made ten times slower, the control's condition inverted); no file under `crates/rcfd-project/src/`
changed.

## What was read

- `AGENTS.md`; `docs/process/agent-rules.md` (version 5) whole; the brief whole; `crates/rcfd-project/tests/listing_budget.rs`
  whole (97 lines) and the function `scratch` of `tests/common/mod.rs`; the entry F-866 of `docs/roadmap.json`.
- ADR 0128: the summary lines 11 and 23 to 24, finding 12 (lines 196 to 206), amendment 3 (lines 331 to 335) and row 12 of
  its table of measured values (line 355). The other findings of that ADR were not read. ADR 0137: lines 1 to 200, with the
  section on L1 and the two addenda of results. ADR 0122: row L1 of its table of checks and the reason of the limit.
- The artifacts of the three gates under `w49i-work/gate-62a6fec3`, `gate-a5c25ae8` and `gate-f6e68b4a`: `test.log`,
  `progress.log` and `times.tsv` of each shard, searched for the test and its printed lines; `attribution.json` of 62a6fec3.
- `scripts/process_report.py` lines 355 to 372 and 476 to 522 (the registration lint); `scripts/push_main.py` (the lines
  on mutation patches); `crates/rcfd-project/tests/edit_lock_f855.mutation.patch` and `docs/roadmap/proposals/0757-F-867.json`
  as examples of form; ADR 0753 for the style of a registration of a test defect.

No published source is used: the design is arithmetic on durations, derived below.

## The defect, by reading and arithmetic

The control as it stands (lines 70 to 93 of the test): with `w` the slowest of three warm listings, `T` the time of ten
further listings in a row and `B` = 500 ms,

    if 10 w > B:  assert T > B        else: print, not asserted

`T` is ten times the mean of ten listings; `w` is the maximum of three other listings. Nothing orders `10 w` and `T`: the
maximum of three samples is on average above the mean, so `10 w > T` is the usual case. The control therefore fails exactly
when

    T <= B < 10 w

which says nothing about the listing or the budget. The gate of 62a6fec3 printed (shard 4, `test.log` line 8379)
`worst of 3 listings 51.5937ms (budget 500ms); the listing 10 times: 498.1544ms`: `10 w` = 515.937 ms > 500 ms >= 498.1544 ms
= `T`, so the control was asserted and failed. The mean of the ten listings was 49.815 ms, 0.37 % under a tenth of the
budget; the worst of three was 3.2 % over it; `w` over the mean is 1.036. **The lead's reading holds.**

The same band existed since amendment 3 of ADR 0128; it was not met before because the machines were far from it: about
6 ms on Linux (ADR 0137) and 80 to 196 ms on the maintainer's Windows machine (ADR 0128 line 355, ADR 0137).

**The three gates.** Only the failed run printed its times. In `gate-a5c25ae8` (shard 3) and `gate-f6e68b4a` (shard 3) the
test passed, the runner captures the output of a passing test, and no line of L1 is in any `test.log`; the re-run of
62a6fec3 that passed left no printed line either. What the artifacts give for the passing gates is the whole time of the
test (`times.tsv`): 16.40 s and 18.66 s, against 15.73 s in the failed run, of which 14.89 s was the creation of the 500
runs. So the distance of the listing from a tenth of the budget on the runner is known from one run (49.8 ms mean,
51.6 ms worst of three) and is not known for the other two; the whole times say only that the runner was of the same
speed within 20 %. This is reported as it is; it is not a check with a limit.

## Design

### The rule that replaces the control (an amendment of ADR 0128 finding 12 and of ADR 0137's reading of it)

This section is that amendment; neither ADR is edited. With `T` the measured time of ten listings in a row, as now, `B`
the constant `BUDGET` of the test, and `R` = 500 ms written a second time in the control as the registered value of
ADR 0122 L1:

    if T > R:  assert not (T <= B)    ("the budget rejects a listing as slow as these ten together")
    else:      print T and the factor B / (T / 10) that the budget does catch; not asserted

The decision and the assertion are on the same measured quantity, `T`. `w` no longer enters the control. The ten listings
stay in the test: they are a listing ten times slower than the one measured, really run and really timed, which is what
finding 12 put in place of the 500 sleeps.

**It cannot fail when the budget is sound.** The control fails if and only if `R < T <= B`. With `B` = `R` = 500 ms that
set of `T` is empty: for every duration, so on a machine of any speed and under any noise, the control passes. No clock
relation between two measurements is left in it.

**What it still tests.** That the budget in force rejects a real, measured, ten times slower listing wherever the
registered budget of 500 ms says it must. It fails when `BUDGET` is larger than the registered 500 ms and the ten listings
of the machine lie between the two (mutation M1), and the pure checks below fail for such a budget on every machine. That
a real slowdown of the listing is caught is shown, as finding 12 asked ("a throwaway run with the listing made 10x slower
must fail L1"), by mutation M2, where the budget assertion itself fails.

**What it no longer tests, said plainly.** (a) Any relation between the time of ten listings and the time of one: the old
assertion `T > B` given `10 w > B` was a statement about the noise of the clock, and it is the defect. (b) With `B` = `R`
the asserted branch is true by arithmetic, so on a sound test the control is a guard of the budget constant against the
registered value, exercised with a measured duration; it is not an independent measurement. (c) As before, it does not
test that `w` is measured around the listing: a test that timed nothing would have `w` near 0 and pass the budget, under
the old rule as under the new one (under the old rule `10 w <= B` and nothing was asserted). (d) On a machine where
`T <= 500 ms` (the Linux gate, and the Windows runner about half the time) nothing is asserted by the control in L1, as
under ADR 0137; the pure checks C1 to C3 run there too.

Why this candidate of the brief and not the other ("the budget assertion against a duration multiplied by ten"): both have
the same logic; `T` is a measured run of the listing path, `10 w` is a product, and finding 12's replacement was "runs the
listing path 10 times".

Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation): the rule above, among the
candidates the brief left open. It changes no default result, no golden and no hash table.

### The code

Only `crates/rcfd-project/tests/listing_budget.rs` changes. Lines 10 to 13 (`RUNS`, `BUDGET`) and the budget assertion
(lines 94 to 96, `if !cfg!(debug_assertions) { assert!(worst <= BUDGET, ...) }`) stay byte for byte. The control becomes a
pure function of the two measured durations,

    struct Measured { worst: Duration, ten: Duration }
    fn control(m: &Measured) -> Result<bool, String>     // Ok(true) asserted and held, Ok(false) not asserted, Err failed

which L1 calls after the ten listings and turns into a panic on `Err`; the listing itself is called through one helper
`list(&Project)` in the three measured listings and in the ten, so that M2 has one place. Three small tests of the
function are added to the file (below). They are pure and take microseconds to milliseconds; the header's "own test
binary so no sibling test competes with it" stays true of any test that touches the disk, and the gate runs each test of
a binary by its name. The header gains "ADR 0759 (F-866)" (rule 12).

This is an edit of the control of an existing test, which rule 2 otherwise forbids ("existing tests ... are not edited";
"the old test stays as an ignored record"). It is done because the brief orders it and the acceptance of F-866 says "the
test is not ignored or retried"; the old rule is kept as the committed mutation patch M0, which the lead's push script
applies and expects to fail.

### Order of the commits

1. This registration, alone.
2. "Before": the control extracted into `control(&Measured)` **with the old rule unchanged** (`if 10 w > B: T > B`), the
   helper `list`, and the three tests C1 to C3. At this commit C1 fails, which is check A1. ADR 0128 used the same order
   ("the builders are introduced as a seam returning the old flags first").
3. "After": the new rule in `control`, the mutation patch, the proposal file.
4. Addendum 1.

## The checks of the function (pure, the same on every machine)

- **C1**, `f866_c1_the_times_of_gate_62a6fec3_do_not_fail_the_control`: `control` with `worst` = 51 593 700 ns and `ten` =
  498 154 400 ns (the two printed numbers) is not `Err`. Fails before (old rule: 515.937 ms > 500 ms and 498.1544 ms is not
  above 500 ms), passes after (498.1544 ms is not above `R`: `Ok(false)`).
- **C2**, `f866_c2_the_control_asserts_above_the_registered_budget_and_the_budget_rejects`: with `worst` = `ten` / 10,
  `ten` = 500 ms gives `Ok(false)`; `ten` = 500 ms + 1 ns, 501 ms, 844 ms (ADR 0137) and 5 s each give `Ok(true)`; and
  ten times the runner's worst, 515.937 ms, is above `BUDGET` (arithmetic on the constant).
- **C3**, `f866_c3_the_control_cannot_fail_for_any_duration`: for `ten` from 0 to 20 s in steps of 100 us (200 001 values)
  and, for each, `worst` in {0, `ten`/20, `ten`/10, `ten`/5, `ten`}, `control` is never `Err`.

The registered "fails before" is C1. Under the old rule C3 fails as well (for example `ten` = 400 ms with `worst` =
80 ms); what C2 and C3 give at commit 2 is reported and is not a check.

## The repeats and the band, registered

Every cargo command runs with `CARGO_TARGET_DIR` under the scratch folder and `RCFD_PROJECT_SCRATCH` set to a folder
`proj-tmp` under the scratch folder (the test leaves its 500 runs behind; the folder is emptied between series).
The command is that of the brief: `cargo test -p rcfd-project --release --test listing_budget` (debug: without
`--release`). A run counts as failed when the command exits non-zero. Each series is run once.

- **Calibration** (commit 2, no limit): 3 runs in release, unloaded; `w0` is the median of the three printed `worst`.
  ADR 0137 measured 80.6 ms on the maintainer's machine and ADR 0128 160 to 196 ms, so `w0` above the band is expected.
- **The band** is `worst` between 40 ms and 62.5 ms (a tenth of the budget, 25 % down and up): there `10 w` and `T`
  straddle 500 ms. The means to bring the listing into it is fixed now, by `w0`, and not searched afterwards:
  - `w0` in the band: none needed.
  - `w0` < 40 ms: a load, `load.py K` in the scratch folder: K processes, each in a loop writing a 4 KiB file in its own
    scratch folder, reading it back, deleting it and then doing about 1 ms of arithmetic. Ladder K = 4, 8, 16, 32, two
    runs of the test at commit 2 per rung, stopped at the first rung whose mean `worst` is above 62.5 ms; the rung whose
    mean `worst` is nearest 50 ms is taken. K stops at 32 (twice the 16 logical processors) because the machine is shared
    with the agent of F-855, whose runs are sensitive to load; the load runs only while a series runs.
  - `w0` > 62.5 ms: no load can do it, since a load only makes the listing slower. Then the band is produced on a scratch
    copy of the tree (`git archive`), in which the one line `const RUNS: usize = 500;` is replaced by
    `round(500 * 50 ms / w0)` (the listing time is proportional to the number of runs to first order). This is a
    **deviation from the brief's words "under a registered load"**, decided here before any run and for the reason given;
    the copy is not the registered test L1 and its results are reported as those of the copy.
  The band **counts as reproduced** if the median `worst` of the 20 runs of series S3 is between 40 and 62.5 ms.
- **S1**, after the change, release, unloaded, the committed test: 20 runs.
- **S2**, after the change, debug, unloaded, the committed test: 3 runs.
- **S3**, after the change, release, in the band by the means fixed above: 20 runs.
- **S0**, the old rule (commit 2), release, in the band by the same means: 20 runs; the number of failed runs and the
  message of each are reported, without a limit. The probability of the old failure in the band is not known beforehand;
  from the one gate run and `w`/mean of 1.036, between a tenth and a half of the runs when the mean is within 3 % of
  50 ms.

## Table of checks

| Id | Check | Limit | Source of the limit | Estimate | Margin | Kind |
| --- | --- | --- | --- | --- | --- | --- |
| A1 | acceptance, "shown by a control that fails before the change with the times of the gate of 62a6fec3": C1 at commit 2 (old rule in `control`) | fails, with the message of the control | the acceptance of F-866; the decision rule of the brief | fails | n/a | code verification |
| A2 | acceptance, "no longer fails on a machine whose worst listing times ten is above the budget while ten listings together are below it": C1 at commit 3 and C3 | pass | the acceptance of F-866; the brief ("it cannot fail on a machine of any speed when the listing and the budget are sound") | pass | n/a | code verification |
| A3 | acceptance, "the budget of 500 ms and the assertion that a listing stays within it are unchanged": `git diff cd74f388` of the test file removes or changes no line among lines 10 to 13 and 94 to 96 of the file at cd74f388 | no such line | the acceptance of F-866 | holds | n/a | code verification |
| A4 | acceptance, "the control still fails a listing that is ten times slower, shown by a mutation": M2 on a scratch copy, release, unloaded, one run | L1 fails at the budget assertion | the acceptance of F-866; ADR 0128 finding 12 ("a throwaway run with the listing made 10x slower must fail L1") | fails: worst about 800 ms to 1.6 s against 500 ms (ADR 0137, ADR 0128 line 355) | 1.6 | code verification (negative control) |
| A5 | acceptance, "the test is not ignored or retried": the file after the change has no `#[ignore]`, no loop around the control and no second attempt, by reading the diff | none present | the acceptance of F-866 | holds | n/a | code verification |
| C2 | the control asserts above the registered budget and the budget rejects (test C2) | passes | the brief, task 2 ("the control still fails for a listing that is ten times slower than measured") | passes | n/a | code verification |
| S1 | 20 runs in release, unloaded, after the change | 0 failed of 20 | the brief, task 3 ("the test passes a registered number of repeats on your machine") | 0 | n/a | code verification |
| S2 | 3 runs in debug, unloaded, after the change | 0 failed of 3 | the brief ("the debug build of the same test") | 0 | n/a | code verification |
| S3 | 20 runs in release in the band, after the change; and the band reproduced (median `worst` in 40 to 62.5 ms) | 0 failed of 20; band reproduced | the brief, task 3 ("under a registered load that brings the listing near a tenth of the budget"); the decision rule of the brief | 0; reproduced | n/a | code verification |
| S0 | 20 runs in release in the band, old rule | none (failed runs reported) | the brief, task 1 ("show the defect before changing anything") | 2 to 10 failed | n/a | code verification (reported without a limit) |
| M0 | mutation: the old rule put back in `control` (the committed patch) | C1 fails | rule 16 of `docs/process/agent-rules.md` | fails | n/a | code verification (negative control) |
| M1 | mutation: `BUDGET` 5000 ms | caught: C2 fails; L1 reported | the brief, task 3 ("the budget raised tenfold") | C2 fails at 500 ms + 1 ns; L1's control fails if 500 ms < T <= 5 s | n/a | code verification (negative control) |
| M3 | mutation: the condition of the control inverted (`T <= R` in place of `T > R`) | caught: C1 or C2 fails; L1 reported | the brief, task 3 ("the control's condition inverted") | C1 fails (498.1544 ms asserted, within the budget) | n/a | code verification (negative control) |
| G | the three gates: the printed times of L1 and the whole time of the test, from the artifacts | none (reported) | the brief, task 1 ("report from the three gates' logs") | given above | n/a | code verification (reading, reported) |
| E | `git diff --stat cd74f388` names the test file, its mutation patch, this ADR and the proposal file only; nothing under `crates/rcfd-project/src/` | no other file | the brief ("File you own"; "change no file under `crates/rcfd-project/src/`") | holds | n/a | unchanged default output (no library file changes) |
| F | `cargo fmt -p rcfd-project --check`; `cargo clippy -p rcfd-project --all-targets -- -D warnings`; `cargo test -p rcfd-project --release --lib`; `cargo test -p rcfd-cli --release --test docs_site` | clean; clean; passes; passes | rule 7 of `docs/process/agent-rules.md` | clean | n/a | code verification (the house checks) |
| P | `python scripts/process_report.py --check` and `python scripts/roadmap_proposals.py check` on the last commit | no failure of this work | rules 7 and 9 of `docs/process/agent-rules.md` | none | n/a | code verification (the process check) |

A4's limit is the registered budget; its margin is 800 ms over 500 ms. If `w0` is at or below 50 ms on this machine, ten
times the listing is not above the budget and M2 cannot be caught by the unchanged budget here: that is the second branch
of finding 12, it is reported with the numbers, and by the decision rule the mutation survives.

## Mutations

All on a scratch copy of the tree at the last code commit, release, each run once; what failed and its message are
recorded.

- **M0** (committed, `crates/rcfd-project/tests/listing_budget.mutation.patch`, first line
  `# expect-fail: cargo test -p rcfd-project --release --test listing_budget`): the old rule in `control`. C1 must fail.
- **M1**: `BUDGET` = `Duration::from_millis(5000)`.
- **M2**: `list` calls the listing ten times in place of once (nine extra `p.runs()` before the one returned).
- **M3**: `m.ten > R` becomes `m.ten <= R`.

## Size

Size (agent): done; 0; 1; 0; 70000

The one heavy run is the sum of the series (about 70 runs of a test of 6 to 20 s each); no single test takes two minutes.

## Decision rule

- **done** only if A1 fails before and A2 passes after, M1, M2 and M3 are each caught (M2 by A4), A3, A5, C2, S1, S2 and
  S3 are met with the band reproduced, and E, F and P hold.
- **partial** if a mutation survives, or the band is not reproduced (S3's median `worst` outside 40 to 62.5 ms), or a
  check other than A1 and A2 is missed or not run; the addendum names which.
- **stopped and reported** if the rule above turns out to be able to fail on a sound listing, or not to catch M1, and no
  other control is found; or if the defect needs a change under `crates/rcfd-project/src/`.
- **todo** if A1 does not fail before or A2 does not pass after.

A run that fails in S1, S2 or S3 is not repeated and no limit is changed (rule 2); it is diagnosed and reported.

## What is not covered

- The budget itself: whether 500 ms is the right limit for the runner, whose listing is at a tenth of it, is not examined.
- The other platform: nothing is run on Linux; the pure checks are the same there and L1's control prints as before.
- The edit lock's F2 (F-855) and every file under `crates/rcfd-project/src/`.
- Whether `w` is measured around the listing (point (c) above), under the old rule and the new one alike.

## Amendment 1 (2026-11-21): the scaled copy of the band, corrected; written before its runs

Written after the calibration, A1, S1, S2, S0 and S3 were run and before S0b, S3b, the mutations and the house checks.
Nothing of the test, the rule, the budget or the band changes; the set-up that brings the listing into the band was
defective and is corrected once, by rule 2 ("a defective set-up gets a corrected test by an amendment written and
committed before the corrected run"). S0 and S3 stay as records and are reported in the addendum as they came out.

**What came out.** `w0` = 117.0 ms (calibration: 121.9, 117.0, 113.8 ms), above the band, so the registered means was the
scaled copy with `RUNS` = round(500 x 50 / 117.0) = 214. With 214 runs the median `worst` was 34.57 ms in S0 and 34.58 ms
in S3, under the band's 40 ms: **the band is not reproduced by S3 as registered.** Two things were wrong in the set-up,
both in the registered derivation and not in the test: the three calibration runs read high (the 20 runs of S1 gave a
median `worst` of 98.2 ms), and the listing time is not proportional to the number of runs (ten listings took 925.7 ms
with 500 runs and 328.9 ms with 214: 0.185 and 0.154 ms per run and listing).

**The corrected set-up.** The quantity that decides the old failure is the time of the ten listings against 500 ms, so
the copy is aimed at a mean listing of 50 ms, by a straight line through the two measured medians of the ten listings
after the change, (214 runs, 32.89 ms) from S3 and (500 runs, 92.57 ms) from S1:
`RUNS` = 214 + (50 - 32.89) x 286 / (92.57 - 32.89) = 296.0, so `RUNS` = 296. Nothing else in the copy changes.

- **S3b**, after the change, release, the copy with `RUNS` = 296: 20 runs; limit 0 failed of 20; the band counts as
  reproduced if the median `worst` of these 20 runs is between 40 and 62.5 ms (the band of the registration, unchanged).
- **S0b**, the old rule (the file of commit 80bb50a2 with `RUNS` = 296), release: 20 runs, reported without a limit.
  **A correction of the count:** at that commit C1, C2 and C3 are red by design, so the command exits non-zero in every
  run and the registered count "the command exits non-zero" says nothing; that was an oversight of the registration, and
  S0 gave 20 of 20 by it. For S0 and S0b the count reported is the runs whose log has the line
  `test l1_listing_hundreds_of_runs_within_budget ... FAILED`.

**The decision rule**, amended in one place: where it says S3, read S3b for the 20 runs and for the band. S3 as
registered stays in the table of results as run, with its 0 failed of 20 and its band not reproduced. This is a second
attempt at one set-up after seeing the first; the lead may read the registered rule without this amendment, by which the
outcome is partial for the band alone. If S3b's median is outside the band too, no third set-up is tried and the outcome
is partial.

| Id | Check | Limit | Source of the limit | Estimate | Margin | Kind |
| --- | --- | --- | --- | --- | --- | --- |
| S3b | 20 runs in release on the copy with `RUNS` = 296, after the change; and the band reproduced (median `worst` in 40 to 62.5 ms) | 0 failed of 20; band reproduced | the brief, task 3; the decision rule of the brief | 0; median `worst` about 53 ms | n/a | code verification |
| S0b | 20 runs in release on the copy with `RUNS` = 296, old rule: runs in which L1 failed | none (reported) | the brief, task 1 | 3 to 10 | n/a | code verification (reported without a limit) |

One more disclosure of the runs made so far: every run of a series was made with `-- --nocapture` after the registered
command, so that a passing run prints its times; and the proposal file is committed with the addendum, not with the
change, because its text states the outcome.

## Addendum 1: results (2026-11-22)

Commits: cb061a7b (this registration, alone), 80bb50a2 ("before": the old rule as the function `control`, C1 to C3),
67367620 (the new rule and the mutation patch), dc3aceba (amendment 1), and the commit of this addendum with the proposal
file. Machine: the maintainer's Windows machine, 16 logical processors, shared with other work; release unless said.
Logs are in the scratch folder under `logs/`.

### The lead's reading and the gates (G)

The reading holds, as derived in the registration from the test and the printed line of the gate: the old control fails
exactly when `T <= 500 ms < 10 w`. Of the three gates only the failed run printed its times (mean of ten 49.8 ms, worst
of three 51.6 ms); the two passing gates give the whole time of the test alone (16.40 s and 18.66 s against 15.73 s).

### Checks

| Id | Limit | Measured | Met |
| --- | --- | --- | --- |
| A1 | C1 fails at commit 2 | failed: `the control fails on a sound listing: Err("negative control: the budget does not catch a 10x slower listing")`. C2 and C3 failed there too (C2 at 500 ms + 1 ns: `Ok(false)`, the old rule with `worst` = `ten`/10 rounds down; C3 first at `worst` 50.1 ms, `ten` 50.1 ms); L1 passed | **yes** |
| A2 | C1 and C3 pass after | pass, in each of the 23 runs of S1 and S2 and the 40 of S3 and S3b | **yes** |
| A3 | lines 10 to 13 and 94 to 96 of the file at cd74f388 unchanged | each of the seven lines is in the file as it was; `git diff cd74f388` removes none of them | **yes** |
| A4 | M2: L1 fails at the budget assertion | failed: `listing 500 runs took 1.9364767s > 500ms` (the ten listings 18.45 s; C1 to C3 pass) | **yes** |
| A5 | no `#[ignore]`, no loop around the control, no second attempt | none in the file | **yes** |
| C2 | passes | passes in every run after the change | **yes** |
| S1 | 0 failed of 20 | 0 of 20; `worst` median 98.2 ms (86.5 to 119.3), ten listings median 925.7 ms (857.7 to 1050.0); the control asserted and held in all 20 | **yes** |
| S2 | 0 failed of 3 (debug) | 0 of 3; `worst` 98.3, 86.7, 97.9 ms | **yes** |
| S3 | 0 failed of 20; band reproduced | 0 of 20 on the copy with `RUNS` = 214; median `worst` 34.58 ms (29.0 to 73.5), ten listings median 328.9 ms: **band not reproduced**, under 40 ms. Run 17 (`worst` 73.5 ms, ten 365.3 ms) is a pair the old rule fails; it passed | failed 0: **yes**; band: **no** |
| S3b (amendment 1) | 0 failed of 20; band reproduced | 0 of 20 on the copy with `RUNS` = 296; median `worst` 61.66 ms (41.6 to 154.4), ten listings median 574.7 ms (431.7 to 1088.3): in the band, at its upper edge. The first five runs had ten listings of 432 to 559 ms; from run 6 the machine was slower by a load that is not of this work (ten listings up to 1.09 s) | **yes** |
| S0 | none | on the copy with `RUNS` = 214, old rule: L1 failed in **3 of 20** (runs 1, 16, 18: `worst` 51.8, 52.4, 50.7 ms with ten listings 323.0, 429.5, 310.0 ms), each with the message of the gate. By the registered count (the command exits non-zero) 20 of 20, because C1 to C3 are red at that commit (amendment 1) | reported |
| S0b (amendment 1) | none | on the copy with `RUNS` = 296, old rule: L1 failed in 0 of 20. The machine was under the foreign load for the whole series: median `worst` 104.9 ms (70.5 to 279.6), ten listings 674 ms to 1.21 s, so the series was not in the band and shows nothing about it | reported |
| M0 | C1 fails | fails (and C2, C3); the committed patch applies to the tip | **yes** |
| M1 | caught | C2 fails (`left: Err(..)`, `right: Ok(true)` at 500.000001 ms), C3 fails, and L1 fails at the control (ten listings 1.18 s, budget 5 s) | **yes** |
| M3 | caught | C1, C2 and C3 fail; L1 passes (not asserted on this machine, as expected) | **yes** |
| E | no other file | `git diff --stat cd74f388`: the test file, its mutation patch, this ADR, and the proposal file; nothing under `crates/rcfd-project/src/` | **yes** |
| F | clean; clean; passes; passes | `cargo fmt -p rcfd-project --check` clean; `cargo clippy -p rcfd-project --all-targets -- -D warnings` clean; `--lib` 4 passed; `cargo test -p rcfd-cli --release --test docs_site` **FAILED** (1 passed, 1 failed): `docs_site_builds_with_no_broken_links` names one link, in this registration, `0122-wave47a-rcfd-project.html` (see "Failed" below) | first three: **yes**; `docs_site`: **no** |
| P | no failure of this work | `python scripts/process_report.py --check`: 1 failure, `generated file is not current: docs/adr/index.md`, left to the lead (rules 7 and 9); `python scripts/roadmap_proposals.py check docs/roadmap/proposals/0759-F-866.json` accepts the file (F-866: todo to partial) | **yes** |

### The new rule, and what it no longer tests

The control decides and asserts on one measured duration, the time of the ten listings: above the registered 500 ms the
budget in force must reject it, at or below nothing is asserted. With the budget at its registered value no pair of
durations fails it. It no longer tests any relation between ten listings and one (that relation was the defect), and
with a sound budget its asserted branch is true by arithmetic: it guards the budget constant, with a measured duration,
and the proof that a real slowdown is caught is the budget assertion under M2.

### Deviations and disclosures

- The band was produced by a scaled copy and not by a load, as registered beforehand (`w0` = 117.0 ms was above the band,
  and a load only slows the listing). The first copy missed the band; amendment 1 corrected the set-up once before S3b.
  By the registration without the amendment the band is not reproduced.
- S3b's median is inside the band by 0.8 ms, and it got there with the help of a load that this work did not start; the
  first five runs, before it, had `worst` of 41.6 to 57.0 ms, all in the band; an unloaded series of 20 was not run.
- S0b did not run in the band and reproduced nothing; the live reproduction of the old failure is S0 (3 of 20).
- The count registered for S0 was wrong (amendment 1).
- Every run of a series had `-- --nocapture` after the registered command.
- The proposal file is committed with this addendum, not with the change.
- At commit 80bb50a2 the test binary is red by design (C1 to C3).
- ADR 0128 was read in its passages on L1, not whole.

### Failed

`docs_site` fails because of this registration and nothing else: line 11 links ADR 0122 as `0122-wave47a-rcfd-project.md`,
a file name that was written without looking it up; the file is `0122-wave47a-project-folder-library.md`. The test
lists that one link and no other (857 pages, 3937 relative links). It was run once, on the tree of dc3aceba, before this
addendum, which adds no link. An ADR is append-only from its creating commit (rule 18), so the line is not corrected
here, and an amendment cannot make the link resolve. **For the lead:** the correction is that one file name in line 11,
with the entry it needs in `docs/process/exceptions/adr-rewritten.txt`; the merge cannot pass `docs_site` without it.
After it, `docs_site` was not run by this work.

### Not run

Nothing on Linux. No load ladder (not the registered branch for this `w0`). The mutations were run on the scratch copy
only; the lead's push script applies M0. The whole crate's integration tests and the workspace suite, by rule 7.

### Size

Actual: partial (registered: done); 0 further registrations, 1
amendment; 1 heavy run (108 runs of the test and 4 mutation runs, about 25 minutes, and one release build of
`rcfd-cli` for `docs_site`); 0 questions; about 75000 bytes read.

### Decision by the registered rule

A1 failed before and A2 passes after; M1, M2 and M3 are caught; A3, A5, C2, S1 and S2 are met; E and P hold. S3 as
registered: 0 failed, band not reproduced. S3b by amendment 1: 0 failed, band reproduced. F is missed: `docs_site` fails
on the link of this registration. **Partial**, by the clause "a check other than A1 and A2 is missed": for `docs_site`
under the rule as amended, and for the band as well under the rule as first registered. Every clause of the acceptance
of F-866 is met; no code or test stands between this and done, only the corrected link and a passing `docs_site`, and
the lead's reading of amendment 1.

### Open questions for the maintainer

None. Two points for the lead, not the maintainer. (1) The link of line 11: correct it on main with an exception entry
(recommended), or leave the registration and have `docs_site` pass by another means, of which this work knows none.
(2) Whether amendment 1 is accepted as rule 2's correction of a defective set-up: accept it (recommended, since C3 shows
for every duration that the control cannot fail, and that does not rest on the band runs), or hold to the first
registration, by which the band is not reproduced on this machine.

### For the lead: roadmap entries

Status proposed: partial; done once the link is corrected and `docs_site` passes, if the lead accepts amendment 1.

> ADR 0759, 2026-11-22, partial: the control of listing_budget L1 decides and asserts on the time of the ten listings (above the registered 500 ms the budget must reject them, else nothing is asserted), so it fails for no pair of durations while BUDGET is 500 ms; the pure check C1 with the times of gate 62a6fec3 failed before and passes after; 0 failed of 20 release, 3 debug and 20 runs in the band (scaled copy, RUNS 296, median worst 61.7 ms, by amendment 1; the first copy with RUNS 214 was under the band at 34.6 ms); the old rule failed L1 in 3 of 20 runs on the first copy; mutations budget 5 s, listing ten times slower, condition inverted and old rule all caught; budget and its assertion byte for byte; docs_site fails on one wrong link in the registration itself.

> Every clause of the acceptance is met. Missing: docs_site fails on one link of the registration (line 11 of ADR 0759 names 0122-wave47a-rcfd-project.md; the file is 0122-wave47a-project-folder-library.md), which an agent may not edit; the lead corrects it and runs docs_site. The band of the brief was reproduced only by the set-up of amendment 1 (the first set-up gave 34.6 ms, under 40 ms).

No follow-up item. The same entries are in `docs/roadmap/proposals/0759-F-866.json`. `docs/adr/index.md` is not current
and is left to the lead.

## The lead's reading (2026-11-22), at the merge

The report was read when it came (2026-11-22, 00:08 UTC). Checked against the branch: the registration is the first
commit and alone; the budget of 500 ms and its assertion are unchanged; the only source file changed is
`crates/rcfd-project/tests/listing_budget.rs`.

The one failure the agent reports is corrected by the lead in the merge commit: in line 11 the file name of the link to
ADR 0122 reads `0122-wave47a-project-folder-library.md`, where the registration had a name that does not exist. This is a
deletion in an ADR after its creating commit, which the rules forbid; the lead takes it on himself because a link cannot
be made to resolve by an appended line and no merge passes `docs_site` without it. The ADR is entered in
`docs/process/exceptions/adr-rewritten.txt` with the reason. Nothing else of the registration is changed.

Amendment 1 (the scaled copy with 296 runs) is accepted: it was committed before its runs. What it shows is weak and is
taken as that: the copy sat barely inside the band, and the old rule failed in 3 of 20 runs on the first copy and in none
on the second. The proof of the change is C1, on the gate's own times, which does not depend on a machine.

Status: **done**. Every clause of the acceptance is met, and the missing link was the agent's only reason for partial.
What the new control no longer does is stated by the agent and repeated here for the next reader: with the budget as
registered it cannot fail, so it guards the constant; a listing ten times slower is caught by the budget assertion itself
(mutation 2), not by the control.
