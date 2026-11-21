# ADR 0759: F-866 registration: the control of L1 decides and asserts on the ten listings

Status: **REGISTERED 2026-11-21, written before any code of F-866 and before any run of its checks.**
Before this text nothing was built, no test was run and no file was changed by this work. What was done: reading, and the
arithmetic below on the numbers the gate printed. Results go in addenda at the end; nothing above them is changed after a
run.

It answers the brief `docs/adr/briefs/f866-listing-budget-negative-control.md` and follows `docs/process/agent-rules.md`,
version 5. Branch `f866-listing-budget-negative-control` from main cd74f388, scratch
`w49i-work/f866-listing-budget-negative-control-scratch`. The records of the test concerned are
[ADR 0122](0122-wave47a-rcfd-project.md) (L1, the budget), [ADR 0128](0128-audit-fixes-runner-claude-project-tsstore-render-api.md)
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
