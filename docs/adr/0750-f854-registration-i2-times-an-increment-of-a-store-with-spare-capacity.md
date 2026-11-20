# ADR 0750: F-854 registration: I2 times an increment of a store with spare capacity; the increment that crosses the capacity is reported

Status: **REGISTERED 2026-11-20, written before any code of this registration of F-854 and before any run of its checks.**
Before this text no test of this registration was written or run and nothing was timed. What was done: reading, searches
of the tree and of the gate logs with `grep`, the arithmetic below, and a release build with `--no-run` of two unchanged
test binaries (`monitors_filtered_f456` of `rcfd-post`, `docs_site` of `rcfd-cli`), so that the later runs do not also
compile the dependencies. Results go in addenda at the end; nothing above them is changed after a run.

This is a further registration of roadmap item F-854 under rule 13: it adds no acceptance clause. It answers the brief
`docs/adr/briefs/f854-i2-store-increment.md` and follows `docs/process/agent-rules.md`, version 3. Parent records:
[ADR 0749](0749-f854-registration-two-wall-clock-limits-measured-so-that-load-alone-does-not-fail-them.md) (the loads, the
diagnostic D-I2, the mutations MI1 and MI2, question Q1) and
[ADR 0353](0353-f456-registration-monitors-get-builds-only-what-is-asked.md) (I2, which gets an amendment 4, committed
before the code). Branch `f854-i2-store-increment` from main 43597bd1, scratch `w49i-work/f854-i2-store-increment-scratch`.

## What is asked

The acceptance of F-854 is quoted in ADR 0749 and binds unchanged. The maintainer answered question Q1 of ADR 0749 on
2026-11-20 with "Go with your recommendation", which is option (b): I2 times an increment of 100 samples per series of a
store that grew by pushes, with the reallocation outside the timer, limit 2 ms unchanged; the increment that crosses the
end of the capacity is reported by a test without a limit. The answer is appended to ADR 0749 and ADR 0353 in the commit
before this one (0374a32e), from the brief, word for word. Nothing else is approved.

The brief asks for: the present I2 kept as a record with its body untouched; the new measurement as a new test; the
capacity-crossing increment measured and printed by a running test without a limit; no other test and no library code
changed; the failures of the new test in 30 runs under each of the loads L0, LS and LM of ADR 0749; a slowdown of
`MonitorStore::push` that the new test must still catch, with its mutation committed beside the test.

## What was read

- The brief; `docs/process/agent-rules.md` (version 3); ADR 0749 whole; ADR 0353 whole;
  `crates/rcfd-post/tests/monitors_filtered_f456.rs` whole; `crates/rcfd-post/src/monitors.rs` whole.
- `scripts/process_report.py` lines 310 to 372 (a registration is created in a commit that changes nothing else) and 430
  to 511 (the table of checks: the columns limit, source, margin and kind). `scripts/push_main.py`, the lines on mutation
  patches (the first line `# expect-fail: <command>`).
- Of the gate's records: `test.log` of `w49i-work/gate-46cea4f8` ("median 2.0807 ms, max 3.3185 ms") and of
  `w49i-work/gate-24a26223` ("median 3.3863 ms, max 3.8966 ms"), `attribution.md` of the latter (I2 failed and failed
  again on the re-run).
- Of the scratch folder of ADR 0749 (read only): `drive.py`, `mi1.patch`, `mi2.patch`, `D-I2.tsv`, the test `d_i2.rs` of
  its scratch copy, `mut-mi1.log` (the whole file `monitors_filtered_f456` under MI1 finished in 3.23 s).

Facts used:

- `MonitorStore::push` looks the series up in a `BTreeMap` (with `name.to_string()`), then pushes one `f64` onto each of
  the two vectors `x` and `y` of the series. `Series::x` and `Series::y` are public and `MonitorStore::get` returns the
  series, so a test can read the length and the capacity of every vector. The store has no method that reserves or
  truncates.
- I2 builds a store of four series of 1e5 samples by pushes once, and in each of 200 repeats clones it and times 400
  calls of `push` (samples 100,001 to 100,100 of each series). The clone has no spare capacity, so each of the eight
  vectors is reallocated inside the timer.
- D-I2 of ADR 0749 (five runs each; the clone, then one sample per series outside the timer, then 100 samples per series
  timed): median 0.0124 to 0.0131 ms without load, 0.0317 to 0.0345 ms under LM; maximum of the 200 repeats up to
  0.08 ms and 1.06 ms.
- MI1 of ADR 0749 against the unchanged I2: median 5.37 ms against 0.96 ms unmutated, so the 400 copies of 0.8 MB add
  4.4 ms, 11 microseconds each. MI2 (28 copies in the increment): 0.99 ms, about 0.3 ms added at most.

## Readings, registered

**R1. "A store that grew by pushes", "the reallocation outside the timer".** What separates such a store from the clone
that I2 times is that its vectors have spare capacity: a vector that grows by `push` is reallocated to a multiple of its
length when it is full, and the pushes up to the next such event write into memory it already owns. The new test times
that state. It is produced as D-I2 produced it, which is the measurement the answer names ("the reallocation outside the
timer, as D-I2"): the clone of a store built by pushes, then one push per series outside the timer, which makes the eight
reallocations and leaves every vector with spare capacity. That the timed state is the intended one is asserted, not
assumed (S1 below).

**R2. "At 1e5".** The timed increment is samples 100,001 to 100,100 of each series, onto a store that holds exactly 1e5
per series, as in I2. So the base is built by pushes to 99,999 per series, and the push outside the timer is sample
100,000. (D-I2 built 1e5, pushed sample 100,001 outside the timer and timed 100,002 to 100,101.)

**R3. The statistic.** The median of 200 repeats, at most 2 ms: the statistic ADR 0353 registered for I2 and its
amendment 3 kept. Best and maximum are printed.

**R4. The old I2 and rule 2.** Rule 2: "A defective set-up gets a corrected test by an amendment written and committed
before the corrected run; the old test stays as an ignored record", and "Existing tests and ignored records are not
edited". By the maintainer's answer the clause of F-456 does not mean the set-up of I2 (a store without spare capacity),
so this is the case of the defective set-up, not the case of a missed limit that is renamed `*_known_miss`. The old
`f456_i2_storing_an_increment_of_100_samples_at_1e5` gets one added line, an `#[ignore = "..."]` attribute that names
this ADR and the failing gate runs; its name, its doc comment, its body and its assertion with the 2 ms are untouched
(the form ADR 0748 took for its retired records). The other form the brief names, a test that reports without a limit,
cannot be had from this function with its body untouched, because the body holds the assertion.

**R5. "Neither test is ignored, skipped or retried by the gate to make it pass" (acceptance of F-854).** Read as: the
measurement and its limit do not leave the gate. They do not. The 2 ms limit runs in the gate on the measurement the
maintainer's answer names (the new test), and what the old I2 measured runs in the gate as well, by the same loops and
with the same printed quantities, in the new test without a limit (D2). The ignored function is the record rule 2 asks
for; nothing that it measured is dropped from a gate run. The gate's own handling of a test (`.github/workflows/gate.yml`,
`scripts/gate_test.py`) is not changed.

## Design

No library file changes. One new test file, `crates/rcfd-post/tests/monitors_store_increment_adr0750.rs`, with two
tests; one added line in `crates/rcfd-post/tests/monitors_filtered_f456.rs` (R4). A new file and not two more tests in
the old one, because rule 12 links a failing test to its record through the header of its file and rule 16 names the
committed mutation after the file: the header of the old file names ADR 0353, and the new tests are registered here.

The store of both tests: `MonitorStore::new("iteration")`, the four series "a", "b", "c", "d", unit "", sample `i` with
abscissa `i as f64` and ordinate `1.0 / i as f64`. The stored values do not enter the cost of a push (two `f64` are
copied), so the generator of I2 is not carried over. The timed loop is that of I2: for `i` in 100,001 to 100,100, for
each of the four names, `st.push(n, "", i as f64, 1.0)`; one `Instant` around the 400 calls; the time in milliseconds;
200 repeats; the 200 times sorted with `f64::total_cmp`; best `ms[0]`, median `ms[99]`, maximum `ms[199]`.

### D1. `f854_i2_storing_an_increment_of_100_samples_at_1e5_within_the_capacity` (the limit)

The base is built once by pushes to 99,999 samples per series. Each repeat: clone the base; push sample 100,000 to each
of the four series (outside the timer; this is where the eight vectors are reallocated); read the eight capacities and
assert that each series holds 100,000 samples and each capacity is at least 100,100; time the increment; assert that
each series holds 100,100 samples and that the eight capacities are what they were before the timer. After the 200
repeats the line is printed and the median is asserted to be at most 2.0 ms, with the message of I2
("I2: median ... ms above 2 ms").

Why the timed state is that of a live store. In a store that only grows, a vector of length `n` has capacity `c >= n`,
and the fraction of pushes that reallocate is of the order `1/n`. An increment of 100 samples at 1e5 crosses the end of
the capacity at most once per doubling of the stored length, in one increment of the roughly 1e3 that lie between 1e5
and 2e5; every other increment is the one D1 times. What the clone adds to the picture is only where the block lies,
not what the pushes do: the pushes write 800 bytes at the end of the used part of each vector (bytes 800,000 to
800,800 of the block), and the push outside the timer has already written the eight bytes before them, which lie on
the same page of 4096 bytes when the block starts on a page boundary; so as a rule no first use of a page falls inside
the timer. A live store meets a new page once in 512 samples per vector; that cost is not in D1 and is small against
the limit (it is part of what D2 times).

Not taken: a fresh store built by 4e5 pushes in each repeat (it is the literal "grew by pushes", but it multiplies the
set-up by 200 for the same timed state, and under the mutation MI1 one such build costs seconds, so the committed
mutation would run for many minutes in the lead's push script); one store without a clone (the store cannot be
truncated back to 1e5, and adding that would change library code).

### D2. `f854_i2x_the_increment_that_crosses_the_capacity_at_1e5_is_reported` (no limit)

The set-up of the old I2: the base is built once by pushes to 100,000 per series; each repeat clones it and times the
same increment directly, so every one of the eight vectors is full when the timer starts. Around the timer the eight
capacities are read, and the test asserts that each of them changed during the increment (the increment did cross the
end of the capacity; if a later standard library clones with spare capacity, this fails and says that the test no
longer measures a crossing) and that each series holds 100,100 samples. Best, median and maximum are printed. No time
is asserted. The reason goes in the comment of the test: the time is that of the system allocator handing out eight
blocks of 1.6 MB (ADR 0749, D-I2: about 99 percent of the time), it followed the memory load of the machine by a factor
of four with the code unchanged (ADR 0749, B-I2), and by the maintainer's answer of 2026-11-20 it is reported, not
limited.

D2 is the worst case of a crossing: all eight vectors cross in the same increment. In a live store built by pushes the
vectors of one series cross together and the series cross together only if they were fed in step.

### D3. Loads and driver

L0, LS and LM as registered in ADR 0749 ("Loads"), with the same definitions: L0 none of mine, the total processor load
sampled before the series; LS 16 processes `python -c "while True: pass"`; LM 16 Python processes that each hold a
`bytearray` of 64 MB and copy it without end; the load started 5 s before the test and stopped after it. 30 runs in
each series, each series run once, in the order L0, LS, LM. The driver is `drive.py` of ADR 0749 copied into the
scratch folder of this work, with two changes: the lines it keeps from the output are those of the new tests, and in
each run, while the load of that run is still on, it starts a second process for D2 after the process of D1 has ended.
Each test runs as the gate runs it: `<exe> <test> --exact --test-threads=1 --nocapture`, one process per test, working
directory the crate. A run of the series counts as failed if the process of D1 exits with a status other than 0.

## Table of checks

| Id | Check | Limit | Source of the limit | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|
| C-I2 | D1 as the gate runs it: failures of 30 runs under each of L0, LS, LM | 0 failures in each series; in each run the median of 200 at most 2 ms | the acceptance of F-456 ("an increment of 100 samples at 1e5 stored in at most 2 ms") through ADR 0353 I2, read by the maintainer's answer of 2026-11-20; the count: the decision rule of the brief | median 0.013 ms under L0, 0.015 to 0.03 ms under LS, 0.035 ms under LM (D-I2 of ADR 0749; LS was not measured there) | 57 to 154: large. The limit is the acceptance's and is not tightened here; see "What the limit catches" | code verification (a limit on a time) |
| S1 | D1, in each of the 200 repeats of each run: 100,000 samples per series and every capacity at least 100,100 before the timer; 100,100 samples and the same eight capacities after it | holds in every repeat (asserted by the test) | reading R1 of this registration, from the brief ("with the reallocation outside the timer") | holds: a vector that is reallocated by `push` at least doubles | n/a | code verification (the set-up of the timed state) |
| R-X | D2 as the gate runs it, 30 runs under each of L0, LS, LM: best, median and maximum of each run | none (reported) | the maintainer's answer of 2026-11-20 ("reported by a test without a limit") | medians 0.90 to 1.11 ms, 1.28 to 1.43 ms and 3.85 to 4.88 ms (B-I2 of ADR 0749, the same set-up) | n/a | code verification (reported without a limit) |
| S2 | D2, in each repeat: every one of the eight capacities changed during the timed increment; 100,100 samples per series after it | holds in every repeat (asserted by the test) | design D2 of this registration, from the brief ("the capacity-crossing increment") | holds: `Vec::clone` allocates the length | n/a | code verification (the set-up of the timed state) |
| E-old | `git diff` of `monitors_filtered_f456.rs` against main is one added line, the `#[ignore]` attribute of the old I2; `cargo test -p rcfd-post --release --test monitors_filtered_f456` gives 1 passed, 1 ignored | by reading; 1 passed, 0 failed, 1 ignored | rule 2 of `docs/process/agent-rules.md` ("the old test stays as an ignored record"; "existing tests ... are not edited") | holds | n/a | code verification (reading of the diff) |
| E-lib | `git diff --stat` against main names no file under `crates/*/src`, no golden, no hash table and not `.github/workflows/gate.yml` | none of them named | the brief ("no library code changes"; the decision rule) | holds | n/a | unchanged default output (no library file changes) |
| M-I2 | mutation MI1 (below) makes D1 fail at its limit and at no other assertion | fails there | rule 16; the brief | median about 4.4 ms, which is 2.2 times the limit | caught | code verification (negative control) |
| M-I2b | mutation MI2 (below), which adds less than the limit, leaves D1 passing | none (reported) | the brief ("what the smallest slowdown is that the test catches") | median about 0.3 ms: passes | n/a | code verification (negative control, reported) |
| P | `python scripts/process_report.py --check` on the last commit | 0 failures | the decision rule of the brief | 0 | n/a | code verification (the process check) |
| G | roadmap clause: 3 consecutive full gate runs of main with A4 and the I2 of this registration passing at the first attempt | 3 of 3 | the acceptance of F-854, reading 2 of ADR 0749 | not run here | lead | code verification (gate runs, not made here) |

Rule 7 after the code, in release: the new file; `cargo test -p rcfd-post --release --test monitors_filtered_f456`
(E-old); `cargo test -p rcfd-post --release --lib`; `cargo fmt -p rcfd-post --check`;
`cargo clippy -p rcfd-post --all-targets -- -D warnings`; `cargo test -p rcfd-cli --release --test docs_site` (this
work touches `docs/`). `baseline_hashes` is not run: no shared solver file changes. The ignored old I2 is not run by
this registration: its numbers under the three loads are B-I2 of ADR 0749, and D2 repeats its measurement.

## What the limit catches (arithmetic, before any run)

D1 fails when the median time of 400 calls is above 2 ms. With the estimate of 0.013 ms for the unchanged code (32 ns
a call) the smallest slowdown it catches is (2 - 0.013) / 400 = 4.97 microseconds added to every call of `push`, a call
about 155 times as dear as today; under LM (0.035 ms) it is 4.91 microseconds. A slowdown below that passes. This is
what the margin of 57 to 154 means: the limit of the acceptance bounds the cost of storing an increment at 2 ms and is
far from the cost of the code, so the test is a guard against a push whose cost depends on what is stored, not a
measure of small regressions.

For the kind of slowdown MI1 stands for, a copy of the series' abscissa on every push, a copy of 0.8 MB costs about
11 microseconds (ADR 0749), in proportion to the stored length. So D1 catches a push that is linear in the stored
length from about 4.97 / 11 x 1e5 = 4.5e4 stored samples per series on, and at the 1e5 of the test by a factor of 2.2.

## The slowdown the test must still catch, and the mutations

On a scratch copy of the tree in the scratch folder (the committed files of this branch, copied; its own target
directory); the unmutated copy passes the new file first.

- **MI1** (committed as `crates/rcfd-post/tests/monitors_store_increment_adr0750.mutation.patch`, first line
  `# expect-fail: cargo test --release -p rcfd-post --test monitors_store_increment_adr0750`): `MonitorStore::push`
  copies the abscissa vector of the series on every call (`std::hint::black_box(s.x.clone());` before the two pushes),
  the MI1 of ADR 0749. Expected: D1 fails at "I2: median ... above 2 ms" with a median of about 4.4 ms; S1 holds (the
  mutation does not touch the stored vectors); D2 passes, having no limit, and prints about 5.4 ms.
- **MI2** (scratch only): the same copy only when `s.x.len() % 16 == 0`, the MI2 of ADR 0749: 7 lengths in the
  increment, 28 copies, about 0.3 ms. Expected: D1 passes. It is the control from the other side: a slowdown below
  the limit is not caught, and the registration says so beforehand.
- Negative control of the timer: D2 against D1 in the same run. If the timer of D1 sees a reallocation where there is
  one, D2's median is far above D1's (expected about 1 ms against 0.013 ms without load). Reported with the series.

## Size

Size (agent): done; 0; 2; 0; 110000

(The two heavy runs are the series under LS and under LM, about five minutes each with every processor busy.)

## Decision rule

The brief's:

- **done**: D1 passes 30 of 30 runs under each of L0, LS and LM (C-I2), MI1 is caught (M-I2), and the process check
  gives 0 failures (P).
- **partial**: a registered run was not made, or D1 fails under a load.
- **stopped and reported**: library code or any other test would have to change. (The one added attribute line of the
  old I2 is the form rule 2 and the brief ask for and is not counted as a changed test.)

The roadmap status is a separate matter: row G is the lead's, and until it is run the status this registration can
propose for F-854 is partial, whatever its own verdict.

## What is not covered

- No run on a hosted runner, and none with other test processes beside this one as the gate has them (three at a
  time); the loads are those of ADR 0749, on the lead's machine.
- A slowdown of `push` below 4.9 microseconds a call (above). The limit is not tightened.
- The first use of a new page by a live store, and a crossing of the capacity in a store whose series were not fed in
  step: D2 reports the worst case only.
- What the reallocation costs and why (ADR 0749 left it unmeasured; it is not measured here either).
- The other wall-clock limits of the workspace.

## Addendum 1: results (2026-11-20)

Commits: 0374a32e (the maintainer's answer appended to ADR 0749 and ADR 0353, from the brief), a2a7bb44 (this
registration, alone), 5f02e6c9 (ADR 0353 amendment 4, before the code), af898fac (the new test file and the `#[ignore]`
line of the old I2), b46b4d68 (the committed mutation patch), then this addendum with the generated ADR index. Changed against
main 43597bd1: `crates/rcfd-post/tests/monitors_store_increment_adr0750.rs` (new) and its `.mutation.patch`, one added
line in `crates/rcfd-post/tests/monitors_filtered_f456.rs`, and files under `docs/adr`. No library file, no golden, no
hash table and not `gate.yml`.

All runs in release on the lead's machine (16 logical processors), the test executable built from the test files of af898fac started as the gate
starts it. Each series was run once, in the registered order, 22:00 to 22:12; the total processor load sampled before
the series was 15, 15 and 23 percent. Raw rows: `new-I2.tsv`, the mutation logs and `rule7.log` in the scratch folder,
not committed.

### The series (failures of 30; times in ms, the range over the 30 runs of the best, the median and the maximum of 200)

| Test | Load | Failed | Best of 200 | Median of 200 | Maximum of 200 |
|---|---|---|---|---|---|
| D1, within the capacity (limit 2 ms on the median) | L0 | 0 | 0.0109 to 0.0119 | 0.0120 to 0.0135 | 0.025 to 0.21 |
| D1 | LS | 0 | 0.0179 to 0.0207 | 0.0214 to 0.0220 | 0.027 to 2.09 |
| D1 | LM | 0 | 0.0212 to 0.0250 | 0.0295 to 0.0346 | 0.063 to 3.27 |
| D2, across the end of the capacity (no limit) | L0 | 0 | 0.82 to 0.87 | 0.89 to 0.94 | 1.16 to 3.61 |
| D2 | LS | 0 | 1.07 to 1.24 | 1.17 to 1.30 | 1.97 to 3.65 |
| D2 | LM | 0 | 1.44 to 2.60 | 3.41 to 4.16 | 6.3 to 10.9 |

D1 passed 90 of 90 runs. Its largest median is 0.0346 ms (under LM), a margin of 58 against the limit; without load the
margin is 148 to 167, under LS 91 to 93. The estimates held (0.013, 0.015 to 0.03 and 0.035 ms). Single repeats of D1
reached 2.09 ms under LS and 3.27 ms under LM (a repeat that lost its processor); the median does not see them.

The capacity-crossing increment, as measured by D2: a median of 0.89 to 0.94 ms without load, 1.17 to 1.30 ms under 16
spinning processes and 3.41 to 4.16 ms under 16 processes that copy memory, which is 70, 55 and 115 times the median of
D1 under the same load, in the same runs. Under LM every one of the 30 medians of D2 is above 2 ms: with the limit on
it, as the old I2 had it, it would have failed 30 of 30 (ADR 0749 measured 30 of 30 for the old I2). This is also the
negative control of the timer: where there is a reallocation inside the timer, the timer shows it. The assertions on
the capacities (S1, S2) held in every repeat of every run.

### Checks

| Id | Limit | Measured | Met |
|---|---|---|---|
| C-I2 | 0 failures in each series; median at most 2 ms | 0, 0, 0 of 30 (L0, LS, LM); largest median 0.0135, 0.0220, 0.0346 ms | **yes** |
| S1 | holds in every repeat | no panic in 90 runs of 200 repeats | yes |
| R-X | none | medians 0.89 to 0.94, 1.17 to 1.30, 3.41 to 4.16 ms | reported |
| S2 | holds in every repeat | no panic in 90 runs of 200 repeats | yes |
| E-old | one added line; 1 passed, 1 ignored | `git diff 43597bd1 -- crates/rcfd-post/tests/monitors_filtered_f456.rs`: one line added, the `#[ignore]` attribute; `cargo test -p rcfd-post --release --test monitors_filtered_f456`: 1 passed, 0 failed, 1 ignored | yes |
| E-lib | no library file, golden, hash table or `gate.yml` | `git diff --stat 43597bd1`: the two test files, the patch file and `docs/adr` only | yes |
| M-I2 | MI1 fails D1 at its limit only | median 4.63 ms, panic "I2: median 4.63... ms above 2 ms" at the last assertion; no capacity assertion failed; D2 passed and printed 5.88 ms | yes, caught |
| M-I2b | none | MI2: D1 passed with a median of 0.375 ms | reported: not caught, as registered |
| P | 0 failures | 0 failures, after `python scripts/process_report.py --write` had regenerated `docs/adr/index.md` (see deviations) | yes |
| G | 3 of 3 full gates | not run here | open, the lead's |

### Mutations (scratch copy of the tree of af898fac with its own target directory; the unmutated copy first: D1 0.0133 ms, D2 1.22 ms, 2 passed)

| Mutation | D1 | D2 | Caught by |
|---|---|---|---|
| MI1, a copy of the abscissa on every push (the committed patch) | median 4.63 ms, best 4.38 ms | 5.88 ms | "I2: median ... above 2 ms" of D1 |
| MI2, the copy on every sixteenth stored length | median 0.375 ms | 1.52 ms | not caught, as registered: it adds 0.36 ms, below the limit |

The smallest slowdown the test catches, with the measured medians: (2 - 0.0135) / 400 = 4.97 microseconds added to
every call of `push` without load, 4.91 microseconds under LM; a call costs 0.03 to 0.09 microseconds today. MI1 adds
11.5 microseconds a call (4.62 ms over 400 calls) and is caught with a factor of 2.3 over the limit; MI2's 28 copies
cost 12.9 microseconds each and pass. So a push whose cost is linear in the stored length is caught from about 4.3e4
stored samples per series on; a push 100 times as dear as today's is not. The limit is the acceptance's and was not
tightened.

Rule 7, in the worktree at the tree of af898fac, release: `cargo test -p rcfd-post --release --test monitors_store_increment_adr0750`
2 passed (0.96 s; D1 0.0158 ms, D2 1.27 ms, while the scratch copy was compiling); `--test monitors_filtered_f456`
1 passed, 1 ignored; `--lib` 29 passed; `cargo fmt -p rcfd-post --check` clean;
`cargo clippy -p rcfd-post --all-targets -- -D warnings` clean; `cargo test -p rcfd-cli --release --test docs_site`
passed, run after this addendum was written.

### Deviations and disclosures

- Three commits were made again after the runs. Amendment 4 of ADR 0353 as first committed (956cb4ea) named the rules by
  their path; the process check takes any registration that holds that path for one written under those rules and failed
  ADR 0353 on four counts (no size, no columns kind, margin and source). An appended line cannot remove a string, and a
  later commit that changes the line fails the append-only check, so the three commits not yet handed over were built
  again from the registration a2a7bb44: 956cb4ea, a9345ebe and 6d7fa7d9 became 5f02e6c9, af898fac and b46b4d68. The
  only difference is one line of amendment 4 ("By rule 2 of the agent rules (version 3):"); the test files and the
  patch are byte for byte those the series, the mutations and the rule 7 runs used, and none of them was run again.
  The registration itself and the commit before it are untouched.
- Order of the commits. The brief asks for the maintainer's answer "in the same commit as the registration or before
  it" and for the registration as the first commit, alone; the process check fails a registration whose creating commit
  changes another file. So the answer is the commit before the registration and the registration is alone but second.
  No code and no test came before either.
- `docs/adr/index.md` is regenerated by `python scripts/process_report.py --write` and committed with this addendum,
  on the lead's instruction for this work; rule 9 otherwise leaves that file to the lead. Without it the process check
  gives one failure ("generated file is not current").
- The old I2 is ignored (R4, R5). The acceptance of F-854 says that neither test is ignored "to make it pass"; the
  reading R5 is that its measurement stays in the gate as D2 and its limit as D1. It is the form the brief and option
  (b) of Q1 name, and it is pointed out here so that the lead can read it against the acceptance.
- The new tests are in a new file, not in `monitors_filtered_f456.rs` (Design). The name of the test that carries the
  limit is new; the acceptance of F-854 and row G name the old one.
- The rule 7 run of the new file was made while the scratch copy compiled; its numbers are not results of a series.

### Not run

Any run on a hosted runner; the three gate runs of row G; the ignored old I2 (as registered); the whole workspace
suite; `baseline_hashes` (no library file changed); the committed patch through `scripts/push_main.py` (the same patch
text was applied to the scratch copy with `git apply`, and `git apply --check` of the committed file passes in the
worktree).

### Size

Size (agent), actual: done; 0; 2; 0; 120000

(No amendment to this registration. The two heavy runs were the series under LS, 4.3 minutes, and under LM,
5.6 minutes.)

### Decision by the registered rule

**Done.** D1 passed 30 of 30 runs under each of L0, LS and LM, MI1 is caught at the limit, and the process check gives
0 failures. No library code and no other test changed. For the roadmap the item stays partial until row G is run.

### Open questions for the maintainer

None.

### For the lead: roadmap entries

Proposed status of F-854: partial (row G, the three consecutive full gates, is not run).

> F-854: ADR 0750, 2026-11-20, done by its registered rule. After the maintainer's answer of 2026-11-20 to Q1 of ADR 0749 (option (b); ADR 0353 amendment 4), I2 is `monitors_store_increment_adr0750::f854_i2_storing_an_increment_of_100_samples_at_1e5_within_the_capacity`: 100 samples per series onto a store of 1e5 whose vectors have spare capacity, the reallocation outside the timer and asserted to be outside it, median of 200 repeats, limit 2 ms unchanged. It failed 0, 0 and 0 of 30 runs without load, under 16 spinning processes and under 16 processes copying memory (largest medians 0.0135, 0.0220 and 0.0346 ms, a margin of 58 to 167). The increment that crosses the end of the capacity is printed by a second running test without a limit: medians 0.89 to 0.94, 1.17 to 1.30 and 3.41 to 4.16 ms under the same loads. The old `f456_i2` is an ignored record with its body untouched. The committed mutation (a copy of the abscissa on every push, 4.63 ms) is caught; the smallest slowdown caught is 4.9 microseconds a call. No library code changed; no run on a hosted runner.

> gap: Not run: three consecutive full gates of main with A4 of vof_dambreak_amg and the I2 of ADR 0750 (monitors_store_increment_adr0750, f854_i2_..._within_the_capacity) passing at the first attempt, and any repeat on a hosted windows-latest runner. The limit of 2 ms is 58 to 167 times the measured time, so the test catches only a push that costs 4.9 microseconds more per call.

No new item.
