# ADR 0749: F-854 registration: two wall-clock limits measured so that the load of the machine alone does not fail them

Status: **REGISTERED 2026-11-20, written before any code of F-854 and before any run of its checks.** Before this text
no test of this item was run and nothing was timed. What was done: reading, searches of the tree with `grep`, and a
release build of the two unchanged test binaries with `--no-run` (so that the first registered run does not also
compile). Results go in addenda at the end; nothing above them is changed after a run.

It answers the brief `docs/adr/briefs/f854-wall-clock-limits.md` and follows `docs/process/agent-rules.md`, version 3.
Roadmap item F-854. Branch `f854-wall-clock-limits` from main 0360c638, scratch
`w49i-work/f854-wall-clock-limits-scratch`. The limits belong to
[ADR 0052](0052-wave28a-vof-wiring.md) (A4) and
[ADR 0353](0353-f456-registration-monitors-get-builds-only-what-is-asked.md) (I2); each gets an appended amendment,
committed before the code it allows.

## What is asked

The acceptance of F-854 in `docs/roadmap.json`, quoted and binding: "Every limit not stated here is registered in the ADR
before the run. For rcfd-fv vof_dambreak_amg::a4_dam_break_with_amg (AMG wall time not above CG wall time) and rcfd-post
monitors_filtered_f456::f456_i2_storing_an_increment_of_100_samples_at_1e5 (median not above 2 ms): a registered number of
repeated runs on windows-latest shows how often each fails and whether the cause is the load of the runner or the code;
the measurement is then made robust under the registration (for example the best of a registered number of repeats) so
that the registered number of consecutive gate runs passes. No limit is loosened without the ADR that owns it being
amended, and neither test is ignored, skipped or retried by the gate to make it pass."

**Readings (registered here).**

1. "Repeated runs on windows-latest": the brief places the repeated runs on the lead's machine (Windows 11, 16 logical
   processors), without load and under a registered load. No run on a hosted `windows-latest` runner is made by this
   registration; that part of the clause is covered only by the gate's own record of 46cea4f8 (one failure, one passing
   re-run of each test) and by the gate runs that follow the merge.
2. "The registered number of consecutive gate runs": **3** full gate runs of main after the merge, each with both tests
   passing at the first attempt. They are the lead's to run. Until then the roadmap status this registration can propose
   is partial, whatever its own verdict.
3. "No limit is loosened": the two numbers stay ("at most that of plain CG", 2 ms). What changes is how the time that is
   compared with the limit is taken. Whether that is a loosening is for the owning ADR to say, so each change is an
   amendment there.
4. The brief and rule 2. Rule 2 has an existing test left unedited and a corrected test added beside an ignored record.
   The brief says otherwise for this item: the measurement is changed in the two existing tests, every other
   assertion is untouched and no test is ignored. The brief is followed; the earlier measurement stays readable in the
   history and is quoted in the amendments.

## What was read (no run)

- The brief; `docs/process/agent-rules.md` (version 3); `crates/rcfd-fv/tests/vof_dambreak_amg.rs`;
  `crates/rcfd-post/tests/monitors_filtered_f456.rs`; ADR 0052 whole; ADR 0353 whole; ADR 0229, section
  "Gate (2026-11-12)"; `.github/workflows/gate.yml` (the test step) and the head of `scripts/gate_test.py` (one process
  per test, `--test-threads=1`, `--jobs` 3 unless `RUST_TEST_THREADS` is set).
- The gate's records in `w49i-work/gate-46cea4f8`: the two `attribution.md`, the two printed lines of `test.log`, the
  rows of `times.tsv` (A4 62.10 s, I2 0.82 s); the rows of the same tests in `gate-10ba1a34` and `gate-252c9279`
  (A4 39.0 s and 44.64 s, I2 0.71 s; a passed test prints nothing there, so the split of those times into CG and AMG
  is not on record).
- `crates/rcfd-fv/src/fvmatrix.rs` lines 64 to 85 (`solve` is CG with incomplete Cholesky, `solve_amg_with` builds the
  hierarchy on every call), `crates/rcfd-fv/src/pressure.rs` lines 470 to 505, `crates/rcfd-fv/src/vof.rs` lines 1825 to
  1860 and `VofStepInfo`; `crates/rcfd-linalg/src/vecops.rs` and `matrix.rs` for the use of rayon (`CHUNK` = 4096, so
  the 4800-cell products and dot products are split into two tasks on the global pool);
  `crates/rcfd-post/src/monitors.rs`, `push`.

Facts used:

- A4 times `run(false)` and then `run(true)`, each with one `Instant` around its whole time loop. The two runs are
  about 260 steps each (`3.6 sqrt(2 A / g)` = 0.3886 s at `dt` = 1.5e-3) and share no state.
- I2 times 400 calls of `push` (800 pushes onto vectors) after `base.clone()`. A cloned `Vec` has no spare capacity,
  so each of the eight vectors is reallocated once per repeat (0.8 MB copied into a new block of 1.6 MB; ADR 0353
  addendum 1, by inspection). The work of a repeat is the same in every repeat.
- The standard library has no measure of the processor time of the process, and reading one from the system needs
  `unsafe` or a new crate, which rule 5 excludes. `VofStepInfo` carries no time of the pressure solves, and adding
  one changes a shared solver file.

## Design

### D0. The principle

A measured time is the time the code needs on the idle machine plus what the machine added: `t = w + e`, `e >= 0`.
`e` is waiting for a processor, a shared core, a contended memory bus. A limit on `t` is a limit on the code only as
far as `e` is small or cancels. Two ways follow from this and nothing else:

- where the same work is repeated, the smallest `t` is the one with the smallest `e`, and it is never below `w`
  (the minimum of repeats);
- where two times are compared, the two are taken through the same stretch of time, so the same `e` per unit of work
  falls on both (interleaving).

A slowdown of the code raises `w`, and with it every repeat and every interleaved part, so both measures still see
it. Neither removes a load that slows one kind of work more than the other; whether that occurs is what the
baseline under a steady load shows (B below).

### D1. A4: the two runs advance in turn, one step each

The two solvers are built first. Then the steps are taken in pairs: one step of the CG run and one step of the AMG
run, the order within the pair alternating with the parity of the pair, until both have passed `3.6 t_unit`. Each
step is timed with its own `Instant` from before `step` to after the bookkeeping of that step (volume, bounds,
finiteness, front), which is what the loop of today contains, and the times are summed per run. `secs` of a run is
that sum. The assertion and its limit are unchanged: `am.secs <= cg.secs`, "A4 wall time".

- What is compared is still the wall time of the AMG run against the wall time of the CG run, each without the
  other's steps. Only the 260 pieces of each are now taken alternately instead of one run after the other, so a load
  that comes and goes over more than a few steps (a step is 30 to 80 ms) falls on both.
- The solvers share nothing, so every other number of the test (iterations, front, volume, bounds, Courant number)
  is the number of today; check E1.
- The cost of the test does not rise: the same steps.
- Not taken: the best of several interleaved whole runs (each repeat adds 28 s on this machine and about 40 s on the
  runner; three repeats pass the two minutes of rule 7); the time of the solves alone and a processor-time measure
  (see "Facts used").
- Known bias, against AMG: alternating two solvers costs each step some cache misses that a run alone does not have,
  the same count for both, and the AMG step is the shorter one.

### D2. I2: the best of the 200 repeats

The repeats, the store, the pushes and what is timed stay as they are. The value compared with 2 ms is the minimum
of the 200 times instead of their median: `ms[0]` after the sort. The printed line gains the minimum and keeps the
median and the maximum.

- By D0 the minimum is the repeat least disturbed by the machine and is not below the cost of the code, because
  every repeat does the same pushes and the same eight reallocations.
- A slowdown of `push` or of the growth of a series is in every repeat, so it is in the minimum.
- What the minimum does not see, and the median did: a cost that occurs only in some repeats. The repeats are
  identical by construction, so such a cost is the machine's or the allocator's, not `push`'s.
- Not taken: a store cloned with spare capacity (question QF3 of ADR 0353; it changes what is stored, not how the
  time is taken).

### D3. Contingency, registered

If the baseline B shows A4 failing under the steady load LS (a load that is the same during both runs), the cause
is not the order of the two runs and D1 cannot remove it. If I2 after the change fails under a load, the minimum
does not remove it. In either case nothing is tuned: an amendment to this ADR, committed before any further code
or run, either registers another measure with its derivation or leaves the test as it is with the finding.

## Loads (registered, reproducible)

Driver: a Python 3.9 script in the scratch folder starts the load, runs the test executable as the gate does
(`<exe> <test> --exact --test-threads=1 --nocapture`, one process, working directory the crate), records the exit
status and the printed line, and stops the load. `H` processes of one of two kinds:

- spin: `python -c "while True: pass"`;
- copy: a Python loop that holds a `bytearray` of 64 MB and without end makes a `bytes` copy of it (each pass
  allocates, copies and frees 64 MB).

| Load | Definition | Used for |
|---|---|---|
| L0 | none of mine; the machine as it is, its total processor load sampled before the series (`typeperf`, 3 samples) | A4, I2 |
| LS | steady: 16 spin processes (one per logical processor), started 5 s before the test, stopped after it | A4, I2 |
| LL | late: 32 spin processes started 20 s after the test is launched, stopped after it (on this machine the CG run of today's A4 ends near 20 s, so the load falls on the AMG run) | A4 |
| LM | memory: 16 copy processes, started 5 s before the test, stopped after it | I2 |

LL is not applied to I2 (the test lasts under a second) and LM is not applied to A4 (4800 cells; the working set is
small, and the series would add about 15 minutes of full load on a shared machine).

Repeats: **10** runs of A4 and **30** runs of I2 in every series. A series is run once. Order: all baseline series
(the unchanged tests at 0360c638), then the amendments and the code, then the same series on the changed tests.

## Table of checks

Kind: all rows are code verification (a time of the code against a limit of its own ADR); none is validation.

| Id | Check | Limit | Source of the limit | Estimate | Margin |
|---|---|---|---|---|---|
| B-A4 | baseline, unchanged A4: failures of 10 under L0, LS, LL, with the CG and AMG times of each run | none (reported) | brief ("show the cause") | L0 0 of 10 (7.5 to 8.0 s against 19.6 to 20.1 s, the lead's three runs); LS 0 of 10 if the cause is the order of the runs; LL at least 8 of 10 | n/a |
| B-I2 | baseline, unchanged I2: failures of 30 under L0, LS, LM, with median and maximum of each run | none (reported) | brief | L0 0 of 30 (median 0.91 to 0.98 ms); LS and LM not known: a failure needs 2.1 times the idle median | n/a |
| C-A4 | changed A4: failures of 10 under L0, LS, LL | 0 failures in each series; in each run `am.secs <= cg.secs` | ADR 0052 A4 ("wall time at most that of plain CG"); the count: the brief's decision rule | AMG/CG 0.39 idle (7.75/19.85), up to 0.6 with the interleaving and under load | 1.7 to 2.6 |
| C-I2 | changed I2: failures of 30 under L0, LS, LM | 0 failures in each series; in each run the minimum of 200 at most 2 ms | ADR 0353 I2, from the acceptance of F-456 ("stored in at most 2 ms"); the count: the brief | minimum 0.85 ms idle, up to 1.5 ms under load | 1.3 to 2.4 |
| E1 | the printed line of the changed A4 equals that of the baseline in every field but the two wall times (iterations, D1, D2, D3, bounds, Courant number, front difference) | equal text | `docs/process/tolerances.md`, bit-identity of default output (the solvers are untouched) | equal | n/a |
| E2 | `git diff` of the two test files touches only the timing: in A4 the helper that runs the steps, the header comment and none of the ten assertions; in I2 the lines after the sort and the header comment | by reading | brief ("every other assertion untouched") | holds | n/a |
| M-A4 | mutation MA1 (below) makes the changed A4 fail at "A4 wall time" and at no other assertion | fails there | rule 16; brief | AMG run = CG run + one more CG solve + the AMG solve | caught |
| M-I2 | mutation MI1 (below) makes the changed I2 fail at its assertion | fails there | rule 16; brief | 400 copies of 0.8 MB, about 40 ms against 2 ms | caught |
| G | roadmap clause: 3 consecutive full gate runs of main with both tests passing at the first attempt | 3 of 3 | F-854 acceptance, reading 2 | not run here | lead |

Rule 7 after the code: the two test files in release, `cargo fmt -p rcfd-fv -p rcfd-post --check`,
`cargo clippy -p rcfd-fv -p rcfd-post --all-targets -- -D warnings`, and `cargo test -p rcfd-cli --release --test
docs_site` (this work touches `docs/`). No library file changes, so `--lib` and `baseline_hashes` have nothing to
show and are not run.

## The slowdown each measurement must still catch, and the mutations

Scratch copy of the tree in the scratch folder; the unmutated copy passes first.

- **A4: the AMG path costs more than the CG path.** MA1 (committed as
  `crates/rcfd-fv/tests/vof_dambreak_amg.mutation.patch`): `FvMatrix::solve_amg_with` first runs the plain solve
  twice on a copy of the start vector and discards it, then solves as today. The pressure of the AMG run and its
  iteration counts are unchanged, so only the time moves: AMG run = everything the CG run does outside the pressure
  solve + two CG solves + the AMG solve per corrector, which is above the CG run by one CG solve and the AMG solve,
  on any machine and under any load. MA2 (scratch only): the same with one discarded solve; above the CG run by the
  AMG solve alone, the smallest slowdown of this kind that reverses the order.
- **I2: storing costs more than 2 ms.** MI1 (committed as
  `crates/rcfd-post/tests/monitors_filtered_f456.mutation.patch`): `MonitorStore::push` copies the abscissa vector of
  the series on every call (`std::hint::black_box(s.x.clone())`), a push whose cost grows with the stored length.
  MI2 (scratch only): the same copy only when `s.x.len() % 16 == 0`: 28 copies of 0.8 MB in a repeat, estimated at
  3 ms more; the smaller slowdown, expected caught.
- Negative control of the series themselves: B under LL is expected to fail and C under LL to pass on the same load.
  If B does not fail under any registered load, the cause is not shown; that is reported as it is (brief).

## Size

Size (agent): done; 0; 4; 0; 250000

(The four heavy runs are the loaded series of A4, about 10 to 20 minutes each with every processor busy.)

## Decision rule

The brief's, for this registration:

- **done**: both changed tests pass every registered repeat without load and under their loads (C-A4, C-I2: six
  series, 0 failures), E1 and E2 hold, and MA1 and MI1 are caught at the timing assertion.
- **partial**: one of the two tests is left as it is with its finding, or a registered run was not made.
- **stopped and reported**: anything other than the measurement of these two times would have to change.

The roadmap status is a separate matter (reading 2): row G is not run here.

## What is not covered

- No run on a hosted runner. The gate runs 3 test processes at once on a runner whose processor count was not
  looked up; the loads here are of a different kind (spinning and copying, not other tests).
- A load that slows AMG's work more than CG's, or that leaves none of 200 repeats undisturbed, is not removed by
  these measures (D3).
- The baseline cannot say why the runner's AMG run took 35.1 s; it can only show whether a load placed on the second
  run reproduces the failure and a steady one does not.
- The other wall-clock limits of the workspace are not examined.

## Amendment 1 (2026-11-20): I2 by the contingency D3; a diagnostic registered

Written after the six baseline series and the three series of the changed I2, while the series of the changed A4 run,
and before any further code or run for I2. Nothing above is changed. The full numbers go in addendum 1.

What the runs gave for I2 (failures of 30; the unchanged test is judged on the median, the changed one on the best):

| Load | Unchanged test | Changed test | Best of 200, ms (changed) | Median, ms (changed) |
|---|---|---|---|---|
| L0 | 0 | 0 | 0.81 to 1.06 | 1.07 to 1.18 |
| LS | 0 | 0 | 1.15 to 1.31 | 1.29 to 1.52 |
| LM | 30 | 21 | 1.42 to 2.70 | 3.17 to 4.61 |

Check C-I2 is missed under LM. D3 applies: the minimum does not remove the failure.

**Finding.** Under processes that move memory without end, every one of the 200 repeats is slower, the best one
included: the load is there the whole time, and there is no undisturbed repeat for a statistic to find. The best
repeat lies 5 to 10 percent under the median without load and under LS; it takes out the scatter and none of a
load that lasts. By inspection (ADR 0353 addendum 1) what a repeat times is mostly the eight reallocations of
0.8 MB that the cloned store forces, which is memory traffic of the system allocator and not `push`; the diagnostic
below measures that share. No way of taking the time of these same pushes (a statistic of the repeats, parts taken
in turn, a time that leaves out waiting) removes a slowdown of the memory itself. With the limit at 2 ms and the
store cloned as registered, I2 does not hold on this machine while its memory is saturated, whatever the measure.

**Decision (by D3 and the brief: "report it and leave that test as it is").** I2 is left as it was at 0360c638:
`crates/rcfd-post/tests/monitors_filtered_f456.rs` is restored to that content, byte for byte, in a commit after
this amendment, and amendment 2 of ADR 0353 is withdrawn by an amendment 3 there, committed with this one. The
restored test is the test of series B-I2, so it is not run again. Not taken, because it would change more than how
the time is taken (what an increment is, or the form of the limit), and so belongs to the owner of ADR 0353: a
store that keeps spare capacity; a limit relative to a reference copy. They are listed as options in addendum 1.
Keeping the best of 200 without its criterion met is not taken either: it is a looser judgement (the minimum is
never above the median) with no registered result behind it.

Consequences for the checks: C-I2 is missed under LM and its change withdrawn; M-I2 has no changed test to run
against, so no mutation patch is committed beside the I2 test, and MI1 and MI2 are run on the scratch copy against
the restored test only as a record that it catches them. A4 is not touched by this amendment.

**Diagnostic D-I2 (scratch copy only, nothing committed, no limit).** The I2 test with one change: after
`base.clone()` one sample is pushed to each of the four series outside the timer (this makes the eight
reallocations), and the next 100 samples are timed as before. 5 runs under L0 and 5 under LM; best, median and
maximum reported. It shows what share of I2's time is the reallocation and what the load does to the rest.
Prediction: under 0.1 ms without load (the estimate of ADR 0353 before its run), under 0.5 ms under LM.

By the decision rule the verdict of this registration can now be at most **partial** (one of the two tests is left
as it is with its finding).

## Addendum 1: results (2026-11-20)

Commits: 37e4cf5c (this registration, alone), 818b3c92 (ADR 0052 amendment 1 and ADR 0353 amendment 2, before the
code), f2bfa2ae (both tests changed), 2f850e7d (amendment 1 above and ADR 0353 amendment 3, before the restoring),
ae2b088c (I2 restored), 69ce9114 (the mutation patch of A4). Changed against 0360c638 in the end:
`crates/rcfd-fv/tests/vof_dambreak_amg.rs` and its `.mutation.patch`; `crates/rcfd-post/tests/monitors_filtered_f456.rs`
is byte for byte what it was. No library file, no golden, no hash table and not `gate.yml`.

All runs in release on the lead's machine (16 logical processors), each test executable started as the gate starts
it. Every series was run once, in the registered order. No cargo or rustc process of anyone else was seen when the
series began; the total processor load sampled before each series was 11 to 45 percent (the higher values straight
after the load of the series before had been stopped), from what I do not know. Raw rows: `base-A4.tsv`,
`base-I2.tsv`, `new-A4.tsv`, `new-I2.tsv`, `D-I2.tsv` and the mutation logs in the scratch folder, not committed.

### A4 (failures of 10; wall times in seconds; ratio = AMG / CG of a run)

| Series | Load | Failed | AMG | CG | Ratio |
|---|---|---|---|---|---|
| B-A4, unchanged | L0 | 0 | 7.0 to 7.3 | 18.7 to 19.5 | 0.36 to 0.39 |
| B-A4, unchanged | LS | 0 | 24.4 to 25.7 | 45.0 to 49.4 | 0.50 to 0.56 |
| B-A4, unchanged | LL | **10** | 25.1 to 28.3 | 19.6 to 20.1 | 1.28 to 1.41 |
| C-A4, changed | L0 | 0 | 7.2 to 7.7 | 18.4 to 19.9 | 0.39 to 0.40 |
| C-A4, changed | LS | 0 | 22.6 to 24.8 | 42.1 to 45.3 | 0.52 to 0.56 |
| C-A4, changed | LL | 0 | 11.2 to 14.6 | 25.8 to 29.2 | 0.43 to 0.50 |

The cause is shown: a steady load does not fail the unchanged test and the same kind of load placed on its second run
fails it 10 times of 10, as predicted. With the runs taken in turn the same late load leaves the ratio at 0.43 to 0.50.
The smallest margin of the changed test is 1.8 (ratio 0.56 under LS). A steady load slows the AMG run more than the CG
run (ratio about 0.54 against 0.38); the changed measure does not remove that and did not need to.

### I2 (failures of 30; times in ms, the range over the 30 runs)

| Series | Load | Failed | Best of 200 | Median of 200 | Maximum of 200 |
|---|---|---|---|---|---|
| B-I2, unchanged (median judged) | L0 | 0 | not printed | 0.90 to 1.11 | 1.15 to 2.62 |
| B-I2, unchanged | LS | 0 | not printed | 1.28 to 1.43 | 1.79 to 4.13 |
| B-I2, unchanged | LM | **30** | not printed | 3.85 to 4.88 | 6.2 to 11.1 |
| C-I2, changed (best judged), withdrawn | L0 | 0 | 0.81 to 1.06 | 1.07 to 1.18 | 1.28 to 2.14 |
| C-I2, changed, withdrawn | LS | 0 | 1.15 to 1.31 | 1.29 to 1.52 | 2.11 to 3.97 |
| C-I2, changed, withdrawn | LM | **21** | 1.42 to 2.70 | 3.17 to 4.61 | 6.4 to 12.1 |

The unchanged test does not fail under processes that only compute and fails every time under processes that move
memory; the gate's median of 2.0807 ms lies between the two. The best of 200 does not hold under LM (amendment 1), so
I2 is left as registered in ADR 0353.

Diagnostic D-I2 (scratch, 5 runs each, the eight reallocations made outside the timer): without load best 0.011,
median 0.012 to 0.013, maximum 0.02 to 0.08 ms; under LM best 0.021 to 0.025, median 0.032 to 0.035, maximum 0.10 to
1.06 ms. So about 99 percent of what I2 times (a median of 1.07 ms without load) is the reallocation that the cloned
store forces, and the 400 calls of `push` themselves take 0.013 ms, in line with the estimate ADR 0353 made before
its run. MI1 below puts a warm copy of 0.8 MB at about 0.011 ms, so the cost of the reallocation is not the copying
of the samples; what it is (new pages from the system, going by the size of the blocks) was not measured.

### Checks

| Id | Limit | Measured | Met |
|---|---|---|---|
| B-A4 | none | 0, 0, 10 of 10 failed (L0, LS, LL) | reported; as predicted |
| B-I2 | none | 0, 0, 30 of 30 failed (L0, LS, LM) | reported |
| C-A4 | 0 failures in each series | 0, 0, 0 of 10 | **yes** |
| C-I2 | 0 failures in each series | 0, 0, 21 of 30 | **no** (LM); the change is withdrawn |
| E1 | printed line equal but for the two wall times | one and the same text in all 60 runs of A4, unchanged and changed | yes |
| E2 | the diff touches only the timing | A4: `run` replaced by `Live`, `start`, `run_both`, the two calls replaced by one, the header comment; no assertion and not the printed line. I2: no difference | yes |
| M-A4 | MA1 fails at "A4 wall time" only | AMG 40.6 s against CG 18.6 s, panic "A4 wall time", every other field as in E1 | yes |
| M-I2 | MI1 fails the changed I2 | not run: there is no changed I2 | not run |
| G | 3 consecutive full gates | not run here | open |

### Mutations (scratch copy; the unmutated copy passed both files first: A4 7.4 s against 18.5 s, I2 median 0.96 ms)

| Mutation | Result | Caught by |
|---|---|---|
| MA1, two discarded plain solves before each AMG solve (committed patch) | AMG 40.6 s, CG 18.6 s | "A4 wall time" |
| MA2, one discarded plain solve | AMG 24.2 s, CG 18.8 s | "A4 wall time" |
| MI1, a copy of the abscissa on every push, against the restored I2 | median 5.37 ms | "I2: median ... above 2 ms" |
| MI2, the copy on every sixteenth sample, against the restored I2 | median 0.99 ms | **not caught** |

MI2 is not caught because my estimate of it was wrong by a factor of ten: 28 copies cost about 0.3 ms, not 3 ms, so
the mutated store still stores the increment in under 2 ms and the test is right to pass. It is not a slowdown beyond
the limit.

Rule 7, in the worktree at 69ce9114: `cargo test -p rcfd-fv --release --test vof_dambreak_amg` 1 passed (26.4 s);
`cargo test -p rcfd-post --release --test monitors_filtered_f456` 2 passed; `cargo fmt -p rcfd-fv -p rcfd-post --check`
clean; `cargo clippy -p rcfd-fv -p rcfd-post --all-targets -- -D warnings` clean; `cargo test -p rcfd-cli --release
--test docs_site` 2 passed, run again after this addendum was written.

### Deviations and disclosures

- I2 was changed (f2bfa2ae) and restored (ae2b088c); the amendments that precede each are 818b3c92 and 2f850e7d.
- No mutation patch is committed beside the I2 test, because the test is unchanged. Rule 16 asks for one committed
  mutation; it is MA1.
- The estimate of MI2 was wrong (above). The estimate of C-I2 under load ("up to 1.5 ms") was wrong for LM.
- During the first run of C-A4 under L0 two commits were made on this machine (that run: 7.7 s against 19.9 s).
- D1 says "the 260 pieces" where the facts say "about 260 steps"; the loop of the test decides the count.

### Not run

Any run on a hosted runner; the three gate runs of row G; LM on A4 and LL on I2 (as registered); the whole
workspace suite; `--lib` and `baseline_hashes` (no library file changed).

### Size

Size (agent), actual: partial; 0; 4; 1; 260000

(One amendment to this registration and no further registration. The four heavy runs were the loaded series of A4,
7 to 13 minutes each; the two LM series of I2, 5 minutes each with the memory loaded, come on top.)

### Decision by the registered rule

**Partial.** A4: the changed test passes every registered repeat without load and under both loads, E1 and E2 hold
and both mutations are caught at the timing assertion. I2 is left as it is with its finding: no way of taking the
time of the same pushes holds 2 ms while the memory of the machine is loaded. Nothing other than the measurement of
the two times was changed.

### Open questions for the maintainer

**Q1. What I2 is to time.** D-I2 shows that I2 times the reallocation of a store without spare capacity (99 percent)
and not `push`. In a live store that state is the one increment per doubling that crosses the end of the capacity;
every other increment costs 0.013 ms.

- (a) Leave I2 as registered. It passes on an idle machine with a margin of 1.8 to 2.2 and fails when the memory of
  the runner is loaded, as in the gate of 46cea4f8; the gate's re-run then calls it flaky.
- (b) A further registration under F-854 (rule 13) with ADR 0353 amended: I2 times an increment of a store that
  grew by pushes (the reallocation outside the timer, as D-I2), limit 2 ms unchanged. It measured 0.035 ms under the
  memory load, a margin of 57, and it still fails a `push` whose cost grows with the stored length (MI1 adds 4.4 ms).
  The increment that crosses the capacity is then reported by the test without a limit, or kept as an ignored
  record. This changes what "an increment" means in the acceptance of F-456, which is why it is not taken here.
- (c) The best of 200 as in the withdrawn amendment 2: 21 of 30 failures under the memory load instead of 30,
  nothing gained under a load that only computes; a looser judgement that is still not robust.

Recommendation: (b). The clause of F-456 is about what the store costs a running solver, and (b) times the code of
the store; (a) keeps a limit on the system allocator in the gate.

### For the lead: roadmap entries

Proposed status of F-854: partial.

> F-854: ADR 0749, 2026-11-20, partial. A4 of `vof_dambreak_amg` now advances the plain-CG run and the AMG run in turn, one step each, and compares the sums of the step times (ADR 0052 amendment 1, limit unchanged): the unchanged test failed 0, 0 and 10 of 10 runs without load, under 16 spinning processes and under 32 started during its AMG run; the changed test 0, 0 and 0 of 10 (AMG/CG 0.39 to 0.56), and two mutations that make the AMG path dearer than CG are caught at "A4 wall time". I2 of `monitors_filtered_f456` is left as registered: it failed 0, 0 and 30 of 30 runs without load, under 16 spinning processes and under 16 processes copying memory, and judged on the best of its 200 repeats still 21 of 30 under the last (ADR 0353 amendments 2 and 3, the change withdrawn); 99 percent of what it times is the reallocation of a cloned store, not `push` (0.013 ms). No run on a hosted runner.

> gap: I2 of monitors_filtered_f456 still fails when the runner's memory is loaded: it times the reallocation of a cloned store (99 percent of 1.07 ms against 2 ms), which no way of taking the time removes; question Q1 of ADR 0749 (what I2 is to time) is with the maintainer. Not run: the 3 consecutive full gates of main with both tests passing at the first attempt, and any repeat on a hosted windows-latest runner.

No new item. If Q1 is answered (b), the work is a further registration under F-854.

## Amendment 2 (2026-11-20), by the lead: the kinds of the checks

The table of checks of this registration has no column "kind" (agent rules, version 3, rule 14); the process report
stopped the merge on that. A registration cannot be edited, so the kinds are given here, one row per check of the table.
None of the rows is a validation or a solution verification: every row checks the test code or the way a time is taken.
Nothing above this section is changed, and no result changes.

| Id | Kind |
|---|---|
| B-A4 | code verification (baseline, reported without a limit) |
| B-I2 | code verification (baseline, reported without a limit) |
| C-A4 | code verification (a limit on a time) |
| C-I2 | code verification (a limit on a time) |
| E1 | unchanged default output (the printed line of the test) |
| E2 | code verification (reading of the diff) |
| M-A4 | code verification (negative control) |
| M-I2 | code verification (negative control) |
| G | code verification (gate runs, not made) |
