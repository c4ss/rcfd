# The test and gate process: measurements and options

Status: measured 2026-10-26 on the gate machine (16 logical processors, 96 GB, shared by up to six agents and two gate slots).
Decision record: [ADR 0127](adr/0127-test-gate-efficiency.md). No test, limit, profile or solver code was changed.
This page is for whoever runs or changes the gate. It records what the gate runs, where its time and memory go, the options ranked by
saving against risk, the new test stage that runs each test as its own process, its measured results, and the scan for tests that
cannot fail.

## 1. What the gate runs today

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --release --no-fail-fast        # CARGO_BUILD_JOBS=3, RUST_TEST_THREADS=3
```

It runs on a `git archive HEAD` copy with a fresh `CARGO_TARGET_DIR`, inside one of two slots (`mkdir <work dir>/slot-1` or
`slot-2`).

## 2. Measured baseline

Sources: the logs of seven gates of waves 46 and 47 (`gate46*/`, `gate47*/` under `<work dir>`), a passive `tasklist` sampler
(every 5 s) while other agents' gates ran, and a CPU-time sample of a running test binary. Stable Rust 1.99 has no per-test timing
(`--report-time` needs `-Z unstable-options`) and `cargo nextest` is not installed, so per-test times come from the new runner (section 5).

### 2.1 Phases

| Gate | clippy | release build | test binaries run (sum of `finished in`) | result |
|---|---|---|---|---|
| 46 (base) | 44 s | 4 min 43 s | 72.6 min | 2028 / 0 / 649 |
| 46A | 74 s | 5 min 35 s | 94.5 min | 2042 / 0 / 651 |
| 46B | 26 s | 4 min 24 s | 94.1 min | 2059 / 0 / 654 |
| 46C | 15 s | 5 min 56 s | 86.5 min | 2040 / 0 / 651 |
| 47C | 40 s | 7 min 54 s | 90.1 min | |
| 47D | 24 s | 7 min 08 s | 95.9 min | |
| 47E (63fad82) | 65 s | 6 min 26 s | 95.8 min | 2093 / 0 / 659 |

The test stage is 92 % to 95 % running tests and 5 % to 8 % building. Clippy is under 1.5 min. Wall time of the 47E test stage from
file times: 103 min.

### 2.2 Where the run time goes

* 400 test sections: 365 integration-test binaries (`tests/*.rs`, each its own binary and its own link), 19 unit-test binaries
  (17 libraries, 2 programs) and 16 doc-test sections with 0 doc-tests.
* Cargo runs them one after another; `RUST_TEST_THREADS=3` only parallelises inside a binary.
* Run time per binary (47E): 237 under 1 s, 88 from 1 s to 10 s, 54 from 10 s to 60 s, 18 from 1 min to 5 min, 3 over 5 min.
  The 296 binaries under 5 s together take 2.9 min. The ten slowest take 53.6 of the 95.8 min.

| Binary (47E) | Tests run / ignored | Time |
|---|---|---|
| rcfd-api `transient_sources` | 17 / 7 | 848 s |
| rcfd-fv `vof_waves` | 6 / 2 | 451 s |
| rcfd-api `wall_motion` | 10 / 5 | 435 s |
| rcfd-api `validation_blasius_plate` | 1 / 6 | 296 s |
| rcfd-api `fan_sector` | 7 / 3 | 275 s |
| rcfd-fv `cavitating_venturi` | 2 / 2 | 263 s |
| rcfd-api `validation_cavity_ghia` | 3 / 1 | 200 s |
| rcfd-fv `w34c_symmetry` | 9 / 4 | 167 s |
| rcfd-fv `rotating_mesh_study` | 5 / 3 | 167 s |
| rcfd-fv `popbal_transport` | 3 / 0 | 114 s |

| Crate (47E) | Run time | Passed | Ignored | Binaries |
|---|---|---|---|---|
| rcfd-api | 55.3 min | 685 | 247 | 135 |
| rcfd-fv | 31.4 min | 494 | 230 | 98 |
| rcfd-turb | 4.7 min | 78 | 79 | 27 |
| rcfd-mesh | 2.5 min | 189 | 44 | 19 |
| rcfd-claude and rcfd-cli | 0.8 min | 99 | 8 | 26 |
| all other crates | 0.6 min | 548 | 51 | 79 |

**Idle threads.** A binary whose time is one long test keeps two of the three test threads idle: `validation_blasius_plate` is one
test for 296 s, `cavitating_venturi` two tests for 263 s, `flat_plate_sst` two for 107 s. The running `validation_blasius_plate` of another
agent's gate used about two cores (CPU time 2 min 31 s at 20:25:59, 8 min 47 s at 20:29:05, from `tasklist /v`); the `rayon` loops
in rcfd-linalg and rcfd-fv parallelise only part of a solve. Three such tests at once use about 6 of the 16 logical processors. The
`rayon` reductions sum fixed chunks in order, so their results do not depend on the number of threads or on load.

### 2.3 Build

* Release profile: none is set in `Cargo.toml`, so the defaults apply: `opt-level = 3`, `debug = false` (and debug info stripped),
  `codegen-units = 16`, `incremental = false`, `lto = false` (thin local). The dev profile used by clippy: `debug = true`,
  `incremental = true`.
* 190 compile units per release build: 17 workspace crates and 173 dependency crates (most of them for rcfd-gui: egui, winit, glutin,
  image; and rustls, ring for rcfd-assistant).
* Build time split, from file times of run 2 (section 5.6; dependencies already built): the 19 workspace libraries were done 92 s
  after the build started, the last of the 400 test executables 323 s after it. So compiling and linking the test binaries takes
  about 3.9 min of the 5.4 min with `-j 3`; link time alone was not measured.
* Every gate starts with an empty target folder, so all 190 units are compiled again; the ten gate target folders of waves 46 and 47
  hold 3.8 GB to 5.1 GB each, 46 GB together.
* The 13 agent target folders (`target-w45a` to `target-w47e`) hold 0.2 GB to 23 GB each, 60 GB together, mostly `debug`; each agent
  compiles the same 173 dependencies once per profile.
* Peak memory during a release build with `-j 3`: three `rustc` plus links. The `tasklist` samplers saw up to 1.6 GB of `rustc`
  working set at once on the machine (several agents) and 0.9 GB to 1.1 GB (all agents counted) while this work's release builds ran.

### 2.4 Memory while testing

Per-test peak working set from the new runner at 63fad82 (exact, from the process counters): 2087 of the 2093 tests stay under
500 MB, most under 100 MB. Six are larger: rcfd-post `render_pixels::r2_one_million_cells_render_in_under_two_seconds` 2720 MB,
rcfd-api `rules_w19c::e2_no_error_level_false_positives_and_warnings_are_only_sv103` 2692 MB, three rcfd-mesh C-grid tests at
1225 MB to 1290 MB and rcfd-mesh `fan::m4_quality_gates_and_m5_first_cell` 540 MB. The original `cargo test` reaches the same peaks:
the passive sampler saw `rules_w19c` of another agent's gate at 2460 MB, 2532 MB summed over its test processes. The same sampler saw
up to 1.6 GB of `rustc` working set (four compilers of other agents). The test stage is not where the memory goes; builds are.

**Hung tests hold a slot.** On 2026-10-26 a wave 47 gate held slot 1 from 18:04 to about 21:45 while one test
(`c7_c8_scrubbing_a_1m_cell_store_with_20_fields`) sat at 8 CPU seconds; neither `cargo test` nor the new runner has a time limit.

## 3. Options, ranked by saving against risk

Times are for the test stage at 47E (103 min). "Robustness" is what the option could cost.

| Rank | Option | Saving | Robustness cost | Done? |
|---|---|---|---|---|
| 1 | Run tests of different binaries at the same time: one process per test from one pool of 3 (`scripts/gate_test.py`) | measured: test stage 102 min to 67 min (section 5) | tests now overlap across binaries (audited: no shared fixed path); more isolation inside a binary than today | **yes** (ADR 0127) |
| 2 | More test processes per slot (`--jobs 5` or `6`) | at 3 the stage sits on its floor (test-seconds / 3 = 59 min); at 6 the floor is 29 min plus contention | CPU: a solver test uses about two cores, so 6 jobs in each of two slots ask for 24 of 16 processors; memory: worst case the six largest tests together, 9.6 GB, typical under 1 GB | measured at 5 (ADR 0129, section 5.7): -13 % under load, below the registered 25 %: **not adopted**, default stays 3 |
| 3 | Start the slowest tests first, using the TSV of an earlier run as an ordering hint | small: binary order already finished 0.6 min above the floor (section 5) | none: every test still runs; a missing file falls back to binary order | yes, as `--order` |
| 4 | A persistent release target folder per slot (not per gate), cleared of `target/tmp` before each run | measured about 2 min: 7 min 20 s with an empty folder (run 1) against 5 min 22 s with the dependencies already built (run 2); and 4.4 GB of disk per gate | **measured pitfall** (section 5.4): binaries do not depend on the source folder's path, so a target shared by two copies reuses the other copy's binaries when the new copy's files are older than the last build; safe only if the copy is extracted with `tar -m` right before its build | proposed, with that rule |
| 5 | A time limit per test that fails the test (for example three times its last time, at least 30 min) | frees a slot from a hung test (section 2.4: about 3.5 h on 2026-10-26) | a very slow test that would have passed now fails; the limit must not hide behind a retry | **yes** (ADR 0129): `clamp(5 x last time, 10 min, 60 min)`, section 4 |
| 6 | Less memory per test thread | nothing to gain: 2087 of 2093 tests stay under 500 MB; the large ones are large by design (one-million-cell render, C-grid stacks) | none | no |
| 7 | `debug = 0` for the test profile | none: the release profile already has no debug info | none | no |
| 8 | `lld-link` instead of `link.exe` (the linker is in the Rust toolchain as `rust-lld`) | part of the 3.9 min in which the test binaries are compiled and linked (section 2.3); link time alone not measured | a second linker in the gate; Windows `rust-lld` is not the default for MSVC targets | proposed, measure first |
| 9 | Merge `tests/*.rs` into fewer binaries | fewer links (365 today), at most a few minutes | test names change (module prefix), so the before/after list no longer matches by name; one panic that aborts kills the whole binary | no |
| 10 | sccache | dependency compiles across agents and gates | a cache outside cargo; a new tool (Apache-2.0); not needed if option 4 is taken | no |
| 11 | One target folder shared by all agents with a lock | disk and dependency compiles | agents block each other on the build lock; a broken build in one folder affects all | no |
| 12 | `cargo nextest` (Apache-2.0 or MIT) | the same as option 1 | a new tool to install and pin; the stdlib script does the same scheduling | no |
| 13 | Split the gate: fast tier (fmt, clippy, crates touched by the diff and their reverse dependencies from `cargo metadata`) and the full tier still mandatory before a merge or a wave | minutes for an agent's own checks | the fast tier skips tests whose code did not change, which the user ruled out for now ("no test caching"); allowed only as a pre-check, never as the gate | proposed only |
| 14 | Shard one gate across both slots | half of the run phase | doubles one gate's concurrency against the two-slot rule; the wave queue gets slower | no |
| 15 | Ignore long tests by default, with a named `--ignored` full run that stays in the final gate | large for agents' own runs | ignored is already used for `*_known_miss` tests; two meanings of ignored make the count harder to read | no |
| 16 | Result caching or skipping unchanged tests | most of the run phase | not allowed now (user decision) | no |

## 4. The new test stage

`scripts/gate_test.py` replaces the `cargo test` line. Its first line is the same build command with `--no-run`, so the binaries are
the ones `cargo test` would run. It lists every binary, runs each non-ignored test as its own process from the package root, and
prints the libtest lines, so the gate's summing `awk` is unchanged. A test passes only if its process exits 0 and reports exactly one
passed test, so a crash, an abort or a name that matched nothing fails it. Doc-tests run afterwards with `cargo test --doc`.

```bash
cd "$GATE/src"
export TEMP=$GATE/tmp TMP=$GATE/tmp CARGO_TARGET_DIR=$GATE/target CARGO_BUILD_JOBS=3 RUST_TEST_THREADS=3
python scripts/gate_test.py --tsv ../times.tsv --order <work dir>/gate-times-$SLOT.tsv > ../test.log 2> ../progress.log
echo "test_exit=$?" > ../stage_test.txt
grep "^test result" ../test.log | awk '{p+=$4;f+=$6;i+=$8} END{print p" passed "f" failed "i" ignored"}' >> ../stage_test.txt
grep -E "^test .* FAILED" ../test.log >> ../stage_test.txt
cp ../times.tsv <work dir>/gate-times-$SLOT.tsv.tmp && mv -f <work dir>/gate-times-$SLOT.tsv.tmp <work dir>/gate-times-$SLOT.tsv
```

`$SLOT` is `slot-1` or `slot-2`: one order file per slot, replaced by a rename, so a gate never reads a half-written file. The script
needs Python 3.9 or later (installed: 3.9.13, PSF licence), standard library only, no build step; it uses no syntax newer than 3.9.
`gate46/stage.sh` and the other stage scripts are not changed by this work; the lines above work once the script is on main, because
the gate runs on a `git archive` that must contain it.

**Time limit (ADR 0129).** Each test gets `clamp(F x its seconds in the --order TSV, min, max)` with `--timeout-factor F`
(default 5), `--timeout-min` (600 s) and `--timeout-max` (3600 s); a test with no record gets the maximum, `--timeout-max 0` turns the
limit off. On expiry the runner kills that test and the processes it started (never a `cargo.exe`, which `workspace_graph` starts,
and nothing the runner did not start), prints `test X ... FAILED (timeout after N s)`, writes outcome `TIMEOUT` to the TSV and goes on.
The last line of the test phase also gives the peak summed working set of the running test processes (sampled every 2 s) and the
largest ratio of a test's time to its limit. Tests: `python -m unittest scripts/test_gate_test.py` (the end-to-end part needs
`GATE_TEST_E2E=1` and a slot).

`progress.log` gets one line per finished test (outcome, seconds, peak working set). `times.tsv` holds the same per test. The old
command stays valid as the reference: `cargo test --workspace --release --no-fail-fast`.

## 5. Results

Registered run 1 (ADR 0127): `git archive 63fad82` in `<work dir>\gate-weff\src`, `--jobs 3`, no `--order`, slot 1 from 23:25 to
00:32 on 2026-10-26/27, another agent's gate in slot 2 throughout. Logs: `gate-weff/test_full.log`, `progress_full.log`,
`times_full.tsv`, `mem_full.log`.

### 5.1 Time

| | Original (47E gate, same commit) | New runner |
|---|---|---|
| release build | 6 min 26 s | 7 min 20 s (same command with `--no-run`) |
| tests | 95.8 min (sum of binaries) | 59.4 min wall |
| doc-tests | included above | 2 s |
| **test stage** | **103 min** | **66.8 min (-35 %)** |

* Test-seconds of the 2093 tests, each in its own process: 10 578 s (176 min). Divided by 3 jobs: 58.8 min, the floor for
  `--jobs 3`. The run finished 0.6 min above it, so ordering has almost nothing left to gain at 3 jobs.
* The slowest test is rcfd-api `transient_sources::t3d_solved_potential_cancels_the_motional_current_converged_outer_iterations`,
  969 s; eight tests over 5 min take 36 % of all test-seconds, 38 tests over 1 min take 65 %. The 1652 tests under 1 s take 171 s
  together, process start included.
* Same load for both: replaying the original scheduling (each binary in turn, three threads taking tests in `--list` order) with
  run 1's per-test times gives 109.6 min for the tests, against 59.4 min measured: -46 % for the test phase, -43 % for the stage,
  without the load difference between the two days.
* The same test in its own process took 4 % to 22 % longer than in the original run where comparable (one-test binaries:
  `validation_blasius_plate` 296 s to 361 s, `vof_droplet` 39 s to 41 s); both runs shared the machine with another gate.

### 5.2 Memory

The runner never has more than `--jobs` test processes by construction (each pool worker waits for its process). Largest summed
working set of test processes in a 5 s sample: 2184 MB, almost all of it `rules_w19c` (2160 MB). The sampler cannot tell this run's
processes from the other slot's when both run the same binary (same name: the hash does not depend on the folder), so this is an upper
bound. Per-test peaks are in section 2.4.

### 5.3 Equivalence

The sorted list of (binary, test, outcome) from the runner's log equals the one from the original 47E log at the same commit:
2752 lines each (2093 ok, 659 ignored), `diff` empty. Both are built by the same `awk` over the `Running` and `test ... ok` lines.

### 5.4 Negative controls and a pitfall

The first negative-control run reused a shared target folder and **passed** (63 passed, 0 failed): cargo did not rebuild, because the
throwaway copy had been extracted and edited before the other copy's build, so its files were older than the binaries, and binary names
do not depend on the folder. The runner was not at fault (it ran what cargo built), but the same trap applies to option 4. The controls
were repeated with their own target folder (section 5.5).

### 5.5 Negative controls

Throwaway copy `gate-weff/neg` of 63fad82 with three breaks in rcfd-core, own target folder `target-weff-neg`, runner and original
`cargo test` on `-p rcfd-core`:

| Break | Runner | Original `cargo test` |
|---|---|---|
| `units::tests::basic_conversions` (unit test) asserts 10.5 instead of 10.0 | FAILED | FAILED |
| `tabular::t1_piecewise_linear_exactness` (integration test) compares with `y + 1.0` | FAILED | FAILED |
| `table_expr::table_calls_are_dimension_checked_and_interpolate` calls `std::process::abort()` | FAILED (exit 0xc0000409, no summary) | binary dies: no `test result` line, the test is not named in the `FAILED` lines |
| summary from the gate's `awk` | 60 passed, 3 failed, 2 ignored; exit 101 | 59 passed, 2 failed, 2 ignored; exit 101 |

All three are caught (G2: 3 of 3). The abort shows a gain in robustness: in the original process an aborting test takes its whole
binary down, so the gate's summary undercounts (here the sibling test `bad_calls_are_rejected_with_a_span` is neither passed nor
failed) and the failing test is named only in cargo's error line. The runner names it and still runs its neighbours.

### 5.6 Run 2 on the rebased branch

`git archive` of `weff` at e7fbbb1 (main da5cc80 with this change; two more crates, rcfd-project and rcfd-runner), `--order` from
run 1, slot 2 from 00:43 to 01:48 on 2026-10-27, another agent's gate in slot 1:

| | Run 2 |
|---|---|
| `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` | clean, clean (49 s) |
| release build | 5 min 22 s |
| tests | 58.7 min wall; 10 568 test-seconds, floor 58.7 min |
| test stage | 64.2 min |
| result | 2153 passed, 0 failed, 663 ignored |
| listing check | the 2816 (binary, test) pairs the runner ran or counted as ignored equal the 2816 of `cargo test --workspace --release -- --list` (395 binaries, 18 doc-test sections), `diff` empty |

Ordering by the earlier run's times put the test phase on its floor (run 1: 0.6 min above it), as expected a small gain. Some tests
start their own executable as a child process (rcfd-project `process`, rcfd-gpu `fallback`, rcfd-api `gpu_case` and the design ledger
tests), so the number of test processes can briefly exceed `--jobs`, as it could inside the original binaries.

### 5.7 Five jobs (ADR 0129)

One archive of 47d73c2 (`<work dir>\gatetp-p2`), `--order` from gate47, time-limit defaults, `--jobs 3` then `--jobs 5` back
to back in slot 1. Another gate held slot 2 for about 61 % of the 3-job run and all of the 5-job run; CPU was 94 % and 100 %.

| | `--jobs 3` | `--jobs 5` |
|---|---|---|
| test phase (wall) | 64.1 min | 55.7 min (-13.2 %) |
| test-seconds | 11 547 | 16 714 (+45 %: every test ran longer) |
| result, list | 2190 / 0 / 666, = gate47 | 2190 / 0 / 666, = `--jobs 3` |
| peak summed working set | 2702 MB | 2704 MB |
| max test time / limit | 0.264 | 0.521 |

Prediction was the floor 9296 s / 5 = 31.0 min. Under load the processors are already full at three jobs, so five only stretch each
test; the gain stays below the registered 25 % and the default stays 3. No test failed only at five jobs.

## 6. Tests that cannot fail (ADR 0129)

### 6.1 Scanner

`python scripts/limit_audit.py` (read-only, Python 3.9 standard library) reads `crates/**/tests/*.rs` and `crates/**/src/**/*tests*.rs`
and prints `file:line: (class) line` for five classes: (i) an assertion whose operands are only literals and constants, or a
sleep/elapsed value in a test that sleeps; (ii) a `#[test]` with no assertion, panic, `unwrap`/`expect`, no `?` in a test returning a
value and no `should_panic`, counting helper functions and macros of the same file (and of `tests/<folder>/` modules) that check;
(iii) a test named with the word control(s) or negative whose assertions do not express a failure; (iv) an early `return` before
the first assertion; (v) `assert!(true)` or a value compared with itself. Self-test: `python -m unittest scripts/test_limit_audit.py`
(one planted example per class, all five found, and a clean test with a helper assertion, a comment and a string that is not flagged).
Not scanned: the 99 inline `#[cfg(test)]` modules in other `src/` files (the registered scope).

On the tests of main 35d9505 it reports 80 hits: (i) 15, (ii) 2, (iii) 13, (iv) 50, (v) 0. Every hit was read.

### 6.2 Triage

Real weakness: 2. Documented skip (or documented report-only test): 53. False positive: 25. Unclassified: 0.

| Hit | Class | Line | Classification | Reason |
|---|---|---|---|---|
| `crates/rcfd-api/src/design/ledger_tests.rs:187` | (iv) | `return;` | documented skip | child-process entry point: does nothing unless the parent test sets its environment variable; the parent asserts |
| `crates/rcfd-api/tests/bc_catalogue.rs:560` | (i) | `assert!((om0 - 1.597191).abs() / 1.597191 < 1e-6);` | false positive | reference arithmetic: the test's own hand value against the registered number (guards a transcription error); the code is compared with that value by the neighbouring assertion |
| `crates/rcfd-api/tests/bc_catalogue.rs:569` | (i) | `assert!((e1 - 1.265625e-6).abs() / 1.265625e-6 < 1e-9);` | false positive | reference arithmetic: the test's own hand value against the registered number (guards a transcription error); the code is compared with that value by the neighbouring assertion |
| `crates/rcfd-api/tests/divergence_guard_w32c.rs:50` | (iii) | `fn unset_key_runs_a_stable_case_to_the_end_and_a_negative_value_is_...` | false positive | 'negative' is a negative input value; the rejection is checked with `unwrap_err` |
| `crates/rcfd-api/tests/duct_diagnosis.rs:112` | (iii) | `fn a_control_walls_at_stream_velocity_hold_plug_flow() {  [fn a_con...` | false positive | positive control: expects success and asserts it (plug flow, valid keys, first-order band 0.9 to 1.15) |
| `crates/rcfd-api/tests/gpu_case.rs:106` | (iv) | `return;` | documented skip | child-process entry point: does nothing unless the parent test sets its environment variable; the parent asserts |
| `crates/rcfd-api/tests/gpu_case.rs:261` | (iv) | `return;  [skip message]` | documented skip | no CUDA device: `GPU TEST SKIPPED` on stderr; `RCFD_GPU_REQUIRE=1` turns the skip into a failure |
| `crates/rcfd-api/tests/gpu_case.rs:271` | (iv) | `return;  [skip message]` | documented skip | no CUDA device: `GPU TEST SKIPPED` on stderr; `RCFD_GPU_REQUIRE=1` turns the skip into a failure |
| `crates/rcfd-api/tests/mixing_plane_run.rs:597` | (iii) | `fn mp4_control_single_block() {  [fn mp4_control_single_block]` | documented skip | `#[ignore]`d diagnostic, documented as prints only / reported only |
| `crates/rcfd-api/tests/report_conductivity_vd.rs:62` | (iii) | `fn w2_positive_controls_validate() {  [fn w2_positive_controls_vali...` | false positive | positive control: expects success and asserts it (plug flow, valid keys, first-order band 0.9 to 1.15) |
| `crates/rcfd-api/tests/validation_refs_w31c.rs:220` | (i) | `assert!((c - 0.1752).abs() < 5e-5);` | false positive | reference arithmetic: the test's own hand value against the registered number (guards a transcription error); the code is compared with that value by the neighbouring assertion |
| `crates/rcfd-api/tests/validation_refs_w31c.rs:237` | (i) | `assert!((0.073 / 2f64.powf(0.25) - alpha).abs() < 5e-5);` | false positive | reference arithmetic: the test's own hand value against the registered number (guards a transcription error); the code is compared with that value by the neighbouring assertion |
| `crates/rcfd-api/tests/w35b_linear_upwind.rs:274` | (iii) | `fn d_controls() {  [fn d_controls]` | documented skip | `#[ignore]`d diagnostic, documented as prints only / reported only |
| `crates/rcfd-api/tests/wall_options.rs:160` | (i) | `const _: () = assert!(BUILD_HOOK_INSTALLED);` | false positive | compile-time `const` assertion of a build-hook flag: fails the build if the flag is false |
| `crates/rcfd-claude/tests/driver_fake.rs:305` | (i) | `assert!(dt < CANCEL_LIMIT, "{dt:?}");  [sleep/elapsed]` | false positive | elapsed time of the code under test (cancel, retry, pause), not of the test's own sleeps; fails if that code is slower or faster than the limit |
| `crates/rcfd-claude/tests/driver_fake.rs:313` | (iii) | `fn d6_negative_control_plain_kill_orphans_the_grandchild() {  [fn d...` | false positive | asserts the expected failure: `assert!(orphaned)` (a plain kill leaves the grandchild) |
| `crates/rcfd-cli/tests/python_w20c.rs:30` | (iv) | `return;  [skip message]` | documented skip | no Python interpreter: prints `SKIPPED` |
| `crates/rcfd-fv/tests/adaptive_step.rs:327` | (iii) | `fn e2_error_control_reaches_the_requested_error_on_a_two_timescale_...` | false positive | 'error control' is the step controller, not a control experiment |
| `crates/rcfd-fv/tests/adaptive_step.rs:664` | (iv) | `return it;` | false positive | `return it` inside a closure (iteration count), not an early exit of the test |
| `crates/rcfd-fv/tests/masstransfer.rs:283` | (i) | `assert!((coef - 0.943).abs() / 0.943 <= 3e-4, "{coef}");` | false positive | reference arithmetic: the test's own hand value against the registered number (guards a transcription error); the code is compared with that value by the neighbouring assertion |
| `crates/rcfd-fv/tests/pseudo_control.rs:328` | (iii) | `fn p4_control_fixed_cfl1() {  [fn p4_control_fixed_cfl1]` | documented skip | `#[ignore]`d diagnostic, documented as prints only / reported only |
| `crates/rcfd-fv/tests/schemes_wave13.rs:452` | (iv) | `return;` | real weakness | T2 returns green when plain BDF2 overshoots less than 0.02; ADR 0016 registered that case as 'reported as vacuous, not as passed', and the measured 2.467e-2 is only 23 % above it. Needs a decision (section 6.3) |
| `crates/rcfd-fv/tests/vof_bubble_diagnosis_w41c.rs:95` | (iii) | `fn d2c_viscosity_ratio_one_control() {  [fn d2c_viscosity_ratio_one...` | documented skip | `#[ignore]`d diagnostic, documented as prints only / reported only |
| `crates/rcfd-fv/tests/vof_bubble_refinement_w40d.rs:247` | (iii) | `fn b1xc_hysing_bubble_all_options_indicator_fraction_control_known_...` | documented skip | `#[ignore]`d known miss of ADR 0108 (XC control); its helper asserts the limits |
| `crates/rcfd-fv/tests/w33b_transient_order.rs:542` | (iii) | `fn o8_euler_controls() {  [fn o8_euler_controls]` | false positive | positive control: expects success and asserts it (plug flow, valid keys, first-order band 0.9 to 1.15) |
| `crates/rcfd-fv/tests/w36b_sheared.rs:207` | (iii) | `fn part_x_controls_without_shear() {  [fn part_x_controls_without_s...` | documented skip | `#[ignore]`d diagnostic, documented as prints only / reported only |
| `crates/rcfd-gpu/tests/errors.rs:12` | (iv) | `return;  [skip message]` | documented skip | no CUDA device: `GPU TEST SKIPPED` on stderr; `RCFD_GPU_REQUIRE=1` turns the skip into a failure |
| `crates/rcfd-gpu/tests/errors.rs:45` | (iv) | `return;  [skip message]` | documented skip | no CUDA device: `GPU TEST SKIPPED` on stderr; `RCFD_GPU_REQUIRE=1` turns the skip into a failure |
| `crates/rcfd-gpu/tests/f1.rs:12` | (iv) | `return;  [skip message]` | documented skip | no CUDA device: `GPU TEST SKIPPED` on stderr; `RCFD_GPU_REQUIRE=1` turns the skip into a failure |
| `crates/rcfd-gpu/tests/fallback.rs:12` | (iv) | `return;` | documented skip | child-process entry point: does nothing unless the parent test sets its environment variable; the parent asserts |
| `crates/rcfd-gpu/tests/kernels.rs:12` | (iv) | `return;  [skip message]` | documented skip | no CUDA device: `GPU TEST SKIPPED` on stderr; `RCFD_GPU_REQUIRE=1` turns the skip into a failure |
| `crates/rcfd-gpu/tests/kernels.rs:55` | (iv) | `return;  [skip message]` | documented skip | no CUDA device: `GPU TEST SKIPPED` on stderr; `RCFD_GPU_REQUIRE=1` turns the skip into a failure |
| `crates/rcfd-gpu/tests/kernels.rs:97` | (iv) | `return;  [skip message]` | documented skip | no CUDA device: `GPU TEST SKIPPED` on stderr; `RCFD_GPU_REQUIRE=1` turns the skip into a failure |
| `crates/rcfd-gpu/tests/leaks.rs:13` | (iv) | `return;  [skip message]` | documented skip | no CUDA device: `GPU TEST SKIPPED` on stderr; `RCFD_GPU_REQUIRE=1` turns the skip into a failure |
| `crates/rcfd-gpu/tests/solvers.rs:79` | (iv) | `return;  [skip message]` | documented skip | no CUDA device: `GPU TEST SKIPPED` on stderr; `RCFD_GPU_REQUIRE=1` turns the skip into a failure |
| `crates/rcfd-gpu/tests/solvers.rs:89` | (iv) | `return;  [skip message]` | documented skip | no CUDA device: `GPU TEST SKIPPED` on stderr; `RCFD_GPU_REQUIRE=1` turns the skip into a failure |
| `crates/rcfd-io/tests/fluent_public.rs:91` | (iv) | `return;  [skip message]` | documented skip | public data set not fetched: prints `skipped` with the fetch command; always skipped in a `git archive` gate (section 6.3) |
| `crates/rcfd-io/tests/interchange.rs:672` | (iv) | `return;` | documented skip | export helper for a manual external check, runs only when its output folder variable is set |
| `crates/rcfd-io/tests/plot3d_public.rs:27` | (iv) | `return;  [skip message]` | documented skip | public data set not fetched: prints `skipped` with the fetch command; always skipped in a `git archive` gate (section 6.3) |
| `crates/rcfd-io/tests/w26c_ensight.rs:518` | (iv) | `return;` | documented skip | export helper for a manual external check, runs only when its output folder variable is set |
| `crates/rcfd-mesh/tests/cgrid_e_w34a.rs:740` | (ii) | `fn r_e_robustness_scan_construction_33a_reported() {  [fn r_e_robus...` | documented skip | report only, documented 'reported, not gated' (the gated twin is the known miss `r_e_robustness_scan_construction_e_known_miss`) |
| `crates/rcfd-mesh/tests/cgrid_w34a.rs:471` | (ii) | `fn w_falsifiers() {  [fn w_falsifiers]` | documented skip | `#[ignore]`d falsifier run by hand, prints its verdict (ADR 0072 amendment 2) |
| `crates/rcfd-post/tests/glb_check.rs:402` | (iv) | `return;` | documented skip | export helper for a manual external check, runs only when its output folder variable is set |
| `crates/rcfd-project/tests/hammer.rs:131` | (i) | `assert!(  [sleep/elapsed]` | false positive | elapsed time of the code under test (cancel, retry, pause), not of the test's own sleeps; fails if that code is slower or faster than the limit |
| `crates/rcfd-project/tests/hammer.rs:155` | (i) | `assert!(spent < Duration::from_secs(2), "{spent:?}");  [sleep/elapsed]` | false positive | elapsed time of the code under test (cancel, retry, pause), not of the test's own sleeps; fails if that code is slower or faster than the limit |
| `crates/rcfd-project/tests/listing_budget.rs:79` | (i) | `assert!(  [sleep/elapsed]` | real weakness | 500 sleeps of 2 ms always exceed the 500 ms budget: the control cannot fail. Fix owned by branch `wfix` (ADR 0128 finding 12), not repeated here |
| `crates/rcfd-project/tests/process.rs:27` | (iv) | `return;` | documented skip | child-process entry point: does nothing unless the parent test sets its environment variable; the parent asserts |
| `crates/rcfd-project/tests/process.rs:49` | (iv) | `return;` | false positive | `return` inside the child body's write loop after its deadline, not a skip |
| `crates/rcfd-project/tests/process.rs:67` | (iv) | `return;` | false positive | `return` inside the child body's write loop after its deadline, not a skip |
| `crates/rcfd-project/tests/process.rs:90` | (iv) | `return;` | false positive | `return` inside the child body's write loop after its deadline, not a skip |
| `crates/rcfd-project/tests/watch.rs:76` | (iii) | `fn w1_negative_control_length_and_mtime_alone_miss_the_same_tick_re...` | false positive | asserts the expected miss: the change list without `cases/d.json` |
| `crates/rcfd-reactor/tests/accel.rs:482` | (iv) | `return println!("D2 skipped: GRI-Mech data not fetched");  [skip me...` | documented skip | GRI-Mech 3.0 files not fetched: prints `skipped` (the three silent `gri() else` returns are in `#[ignore]`d tests). The gate's `git archive` has no `validation-data/data`, so these always skip in the gate (section 6.3) |
| `crates/rcfd-reactor/tests/accel.rs:492` | (iv) | `let Some(m) = gri() else { return };` | documented skip | GRI-Mech 3.0 files not fetched: prints `skipped` (the three silent `gri() else` returns are in `#[ignore]`d tests). The gate's `git archive` has no `validation-data/data`, so these always skip in the gate (section 6.3) |
| `crates/rcfd-reactor/tests/accel.rs:499` | (iv) | `return println!("D3 skipped: GRI-Mech data not fetched");  [skip me...` | documented skip | GRI-Mech 3.0 files not fetched: prints `skipped` (the three silent `gri() else` returns are in `#[ignore]`d tests). The gate's `git archive` has no `validation-data/data`, so these always skip in the gate (section 6.3) |
| `crates/rcfd-reactor/tests/accel.rs:508` | (iv) | `let Some(m) = gri() else { return };` | documented skip | GRI-Mech 3.0 files not fetched: prints `skipped` (the three silent `gri() else` returns are in `#[ignore]`d tests). The gate's `git archive` has no `validation-data/data`, so these always skip in the gate (section 6.3) |
| `crates/rcfd-reactor/tests/accel.rs:515` | (iv) | `return println!("D4 skipped: GRI-Mech data not fetched");  [skip me...` | documented skip | GRI-Mech 3.0 files not fetched: prints `skipped` (the three silent `gri() else` returns are in `#[ignore]`d tests). The gate's `git archive` has no `validation-data/data`, so these always skip in the gate (section 6.3) |
| `crates/rcfd-reactor/tests/accel.rs:755` | (iv) | `let Some(r) = i4_run(true) else { return };` | documented skip | GRI-Mech 3.0 files not fetched: prints `skipped` (the three silent `gri() else` returns are in `#[ignore]`d tests). The gate's `git archive` has no `validation-data/data`, so these always skip in the gate (section 6.3) |
| `crates/rcfd-reactor/tests/accel.rs:770` | (iv) | `let Some(r) = i4_run(true) else { return };` | documented skip | GRI-Mech 3.0 files not fetched: prints `skipped` (the three silent `gri() else` returns are in `#[ignore]`d tests). The gate's `git archive` has no `validation-data/data`, so these always skip in the gate (section 6.3) |
| `crates/rcfd-reactor/tests/accel.rs:779` | (iv) | `let Some(r) = i4_run(false) else { return };` | documented skip | GRI-Mech 3.0 files not fetched: prints `skipped` (the three silent `gri() else` returns are in `#[ignore]`d tests). The gate's `git archive` has no `validation-data/data`, so these always skip in the gate (section 6.3) |
| `crates/rcfd-reactor/tests/accel.rs:846` | (iv) | `let Some((worst, _, _, speed)) = i5_run() else { return };` | documented skip | GRI-Mech 3.0 files not fetched: prints `skipped` (the three silent `gri() else` returns are in `#[ignore]`d tests). The gate's `git archive` has no `validation-data/data`, so these always skip in the gate (section 6.3) |
| `crates/rcfd-reactor/tests/accel.rs:858` | (iv) | `let Some((_, dtmax, _, _)) = i5_run() else { return };` | documented skip | GRI-Mech 3.0 files not fetched: prints `skipped` (the three silent `gri() else` returns are in `#[ignore]`d tests). The gate's `git archive` has no `validation-data/data`, so these always skip in the gate (section 6.3) |
| `crates/rcfd-reactor/tests/accel.rs:867` | (iv) | `let Some((_, _, el, _)) = i5_run() else { return };` | documented skip | GRI-Mech 3.0 files not fetched: prints `skipped` (the three silent `gri() else` returns are in `#[ignore]`d tests). The gate's `git archive` has no `validation-data/data`, so these always skip in the gate (section 6.3) |
| `crates/rcfd-reactor/tests/accel.rs:874` | (iv) | `return println!("C2 skipped: GRI-Mech data not fetched");  [skip me...` | documented skip | GRI-Mech 3.0 files not fetched: prints `skipped` (the three silent `gri() else` returns are in `#[ignore]`d tests). The gate's `git archive` has no `validation-data/data`, so these always skip in the gate (section 6.3) |
| `crates/rcfd-reactor/tests/cell.rs:41` | (i) | `assert!((dt_ad - 2000.0).abs() < 1e-9, "hand DeltaT_ad {dt_ad}");` | false positive | reference arithmetic: the test's own hand value against the registered number (guards a transcription error); the code is compared with that value by the neighbouring assertion |
| `crates/rcfd-reactor/tests/cell.rs:135` | (i) | `assert!((hand - 1149.4339).abs() < 1e-3, "hand arithmetic {hand}");` | false positive | reference arithmetic: the test's own hand value against the registered number (guards a transcription error); the code is compared with that value by the neighbouring assertion |
| `crates/rcfd-reactor/tests/combustion.rs:396` | (iv) | `return println!("B7 skipped: GRI-Mech data not fetched");  [skip me...` | documented skip | GRI-Mech 3.0 files not fetched: prints `skipped` (the three silent `gri() else` returns are in `#[ignore]`d tests). The gate's `git archive` has no `validation-data/data`, so these always skip in the gate (section 6.3) |
| `crates/rcfd-reactor/tests/combustion.rs:779` | (iv) | `return println!("P3 skipped: GRI-Mech data not fetched");  [skip me...` | documented skip | GRI-Mech 3.0 files not fetched: prints `skipped` (the three silent `gri() else` returns are in `#[ignore]`d tests). The gate's `git archive` has no `validation-data/data`, so these always skip in the gate (section 6.3) |
| `crates/rcfd-reactor/tests/combustion.rs:809` | (iv) | `let Some(g) = gri() else { return };` | documented skip | GRI-Mech 3.0 files not fetched: prints `skipped` (the three silent `gri() else` returns are in `#[ignore]`d tests). The gate's `git archive` has no `validation-data/data`, so these always skip in the gate (section 6.3) |
| `crates/rcfd-reactor/tests/combustion.rs:825` | (iv) | `return println!("P3b skipped: GRI-Mech data not fetched");  [skip m...` | documented skip | GRI-Mech 3.0 files not fetched: prints `skipped` (the three silent `gri() else` returns are in `#[ignore]`d tests). The gate's `git archive` has no `validation-data/data`, so these always skip in the gate (section 6.3) |
| `crates/rcfd-reactor/tests/reactor.rs:288` | (iv) | `return println!("R3 skipped: GRI-Mech data not fetched");  [skip me...` | documented skip | GRI-Mech 3.0 files not fetched: prints `skipped` (the three silent `gri() else` returns are in `#[ignore]`d tests). The gate's `git archive` has no `validation-data/data`, so these always skip in the gate (section 6.3) |
| `crates/rcfd-reactor/tests/reactor.rs:308` | (iv) | `return println!("R3 skipped: GRI-Mech data not fetched");  [skip me...` | documented skip | GRI-Mech 3.0 files not fetched: prints `skipped` (the three silent `gri() else` returns are in `#[ignore]`d tests). The gate's `git archive` has no `validation-data/data`, so these always skip in the gate (section 6.3) |
| `crates/rcfd-reactor/tests/reactor.rs:329` | (iv) | `return println!("R7 skipped: GRI-Mech data not fetched");  [skip me...` | documented skip | GRI-Mech 3.0 files not fetched: prints `skipped` (the three silent `gri() else` returns are in `#[ignore]`d tests). The gate's `git archive` has no `validation-data/data`, so these always skip in the gate (section 6.3) |
| `crates/rcfd-reactor/tests/reactor.rs:373` | (iv) | `return println!("R7 skipped: GRI-Mech data not fetched");  [skip me...` | documented skip | GRI-Mech 3.0 files not fetched: prints `skipped` (the three silent `gri() else` returns are in `#[ignore]`d tests). The gate's `git archive` has no `validation-data/data`, so these always skip in the gate (section 6.3) |
| `crates/rcfd-reactor/tests/reactor.rs:461` | (iv) | `return println!("R7 skipped: GRI-Mech data not fetched");  [skip me...` | documented skip | GRI-Mech 3.0 files not fetched: prints `skipped` (the three silent `gri() else` returns are in `#[ignore]`d tests). The gate's `git archive` has no `validation-data/data`, so these always skip in the gate (section 6.3) |
| `crates/rcfd-runner/tests/runner_w47b.rs:500` | (i) | `assert!(dt <= 2.0);  [sleep/elapsed]` | false positive | elapsed time of the code under test (cancel, retry, pause), not of the test's own sleeps; fails if that code is slower or faster than the limit |
| `crates/rcfd-tsstore/tests/scale.rs:114` | (iv) | `let Ok(dir) = std::env::var(C8_ENV) else { return };` | documented skip | child-process entry point: does nothing unless the parent test sets its environment variable; the parent asserts |
| `crates/rcfd-tsstore/tests/store.rs:326` | (iv) | `let Ok(dir) = std::env::var(C5_ENV) else { return };` | documented skip | child-process entry point: does nothing unless the parent test sets its environment variable; the parent asserts |
| `crates/rcfd-tsstore/tests/store.rs:421` | (iv) | `let Ok(dir) = std::env::var(C5B_ENV) else { return };` | documented skip | child-process entry point: does nothing unless the parent test sets its environment variable; the parent asserts |
| `crates/rcfd-turb/tests/step_realizable_a0.rs:32` | (i) | `assert_eq!(REALIZABLE_A0_DEFAULT, 4.04);` | false positive | pins a library constant to its documented value; fails if the constant is edited |
| `crates/rcfd-turb/tests/step_realizable_a0.rs:33` | (i) | `assert_eq!(REALIZABLE_A0_ORIGINAL, 4.0);` | false positive | pins a library constant to its documented value; fails if the constant is edited |

### 6.3 Real weaknesses and items that need a decision

No hit could be fixed by adding or correcting an assertion alone, so no test was edited (A3: 0 fixes, 2 real weaknesses listed, and
one gate-level gap found while triaging the skips):

1. **`rcfd-project/tests/listing_budget.rs` L1 negative control** (class i): `slow` is the sum of 500 sleeps of 2 ms plus the listing,
   compared with the 500 ms budget, so it is always above the budget. Branch `wfix` (ADR 0128, finding 12) already registered the
   fix and its decision rule; not repeated here to avoid two edits of one test.
2. **`rcfd-fv/tests/schemes_wave13.rs` T2** (class iv): when plain BDF2 overshoots less than 0.02 the test prints "vacuous" and
   returns green. ADR 0016 registered that case as "reported as vacuous, not as passed"; measured 2.467e-2 is 23 % above the
   threshold, so a small change of the wave or the scheme would turn T2 into a silent pass. Options: (a) keep; (b) fail the vacuous
   case (an `assert!(ob2 >= 0.02, ...)` before the limit check, which no longer passes the vacuous case); (c) ignore T2 and add a
   corrected set-up with more overshoot. Recommendation: (b), it matches the registered wording. Not done: it changes what the
   test accepts, which is the user's decision.
3. **Data-gated tests always skip in the gate.** `validation-data/data` (GRI-Mech 3.0, the public Fluent and PLOT3D files) is not in
   git, and the gate runs on a `git archive`, so 12 non-ignored tests return early and count as passed in every gate: rcfd-reactor
   `accel` d2, d3, d4, i4, i5 and c2 (GRI-Mech), `combustion` b7, p3, p3b, `reactor` r3, rcfd-io `fluent_public` and
   `plot3d_public`. GPU tests have `RCFD_GPU_REQUIRE=1` to turn their skip into a failure; these have no such switch. Options: (a)
   copy the fetched folder into the gate copy and add a `RCFD_DATA_REQUIRE=1` switch that fails a skip; (b) keep and list them as
   not run by the gate; (c) count a skip separately in the runner (needs a skip marker the runner can read). Recommendation: (a).
4. Child-process entry points (`*_child_body`, `child_entry`, `l3_kill_child`: 7 tests) pass without doing anything when run alone
   and are counted as passed tests. Documented and harmless; they inflate the passed count by 7.
5. **ADR 0123 S7 cancel time** (from the mutation spot-check, section 6.4): 5 s limit against 0.116 s measured; a 10x slower
   cancel passes. Options: (a) keep; (b) a new registered limit derived from the measured time (needs a registered derivation, not
   a tuned number). Recommendation: (a) unless cancel latency matters to the Studio UI, then (b).
6. **ADR 0119 R3** (section 6.4): the test accepts any error with "stall", so the rising-characteristic check is unguarded. Options:
   (a) keep (R3's registered wording is "an error containing 'stall'"); (b) add a new test asserting the "rising fan
   characteristic" message at the R3 point, with R3 unchanged. Recommendation: (b), an addition, no limit changed.

### 6.4 Mutation spot-check of registered limits

Population (ADR 0129 amendment 1): 183 registered check ids of ADRs 0115 to 0126 that a non-ignored test enforces. Draw:
`random.Random(1290127).sample(population, 12)`. Each mutation was made in a scratch `git archive` copy (`<work dir>\wtp-mut`, own
target `target-wtp-mut`, every run showed `Compiling <crate>` after the edit), only that test was run in release, and the file was
restored before the next one. Rule: the quantity the limit governs made 10x worse in the code under test where one line does it,
at the measurement in the test for an elapsed time or a per-specimen error; a discrete check is broken by an off-by-one or by
turning the guarded behaviour off. Unmutated, every one of these tests passed in the `--jobs 3` run at the same tests.

| ADR id | Test | Mutation | Result |
|---|---|---|---|
| 0118 C1 | `fan_design_w46a::c1_default_fan_is_accepted` | `Limits::default().min_wall` 8e-4 to 8e-3 | detected (fails at :568) |
| 0124 C11 | `store::c11_lookup_by_step_and_time_and_writer_preconditions` | `Pick::Floor` returns the next snapshot | detected (:727) |
| 0124 C5 | `store::c5_writer_killed_mid_snapshot_leaves_earlier_snapshots_readable` | scan recovery drops the last complete snapshot | detected (:398) |
| 0120 J3 | `w46c_rotordyn::w46c_j3_supercritical_limit` | `response` translational displacement x10 | detected (:167, \|X\|/e - 1 = 9.0) |
| 0118 I3 | `fan_design_w46a::i1_i2_i3_unbalance_equals_the_sum_of_mass_times_offset` | static unbalance x10 in the I3 loop | detected (:1089) |
| 0123 S7 | `runner_w47b::s7_cancel_leaves_a_consistent_folder` | measured cancel time x10 | **not detected**: 0.116 s x10 = 1.16 s <= 5.0 s |
| 0118 C4 | `fan_design_w46a::c4_structural_violations_are_located` | `n_blades` upper bound 15 to 150 | detected (:806, no `n_blades` violation) |
| 0119 R3 | `fan_rom_w46b::r3_stall_is_located` | rising-characteristic (slope) stall check of the free `operating_point` turned off (rerun; see below) | **not detected**: the error still says "stalled: 40 of 40 strips", which contains "stall" |
| 0122 A2 | `process::a2_atomic_replace_survives_kill_mid_write` | `atomic_write` writes in place instead of rename | detected (:194, bad observations > 0) |
| 0121 L6 | `ledger_tests::l6_rerun_check` | `compare_metrics` accepts every finite difference | detected (:522, the negative control passes) |
| 0119 L3 | `fan_rom_w46b::l3_drag_terms` | drag loss x10 | detected (:446) |
| 0119 NC-L3 | `fan_rom_w46b::l3_drag_terms` | annulus drag term removed from the model | detected (:445, "NC-L3 does not discriminate") |

**10 of 12 detected.** Finding 1: ADR 0119 R3 asserts only that the error contains "stall". Two separate checks produce such an
error: the slope check of the free `operating_point` ("rising fan characteristic ... stall region", the message the ADR 0119
addendum recorded for R3) and the strip check of `FanRom::operating_point` ("is stalled: n of 40 strips"). With the slope check off,
the strip check still fails the R3 point, so R3 passes; the slope check is not guarded by any assertion of this test. The first R3
mutation (strip check off) made the test fail, but at :545, the R3b part (a separate id), so it was counted as a set-up defect of the
mutation and replaced by the slope-check mutation before the result was written down. Finding 2: ADR 0123 S7 registers "three cancels within 5 s"; measured 0.116 s, so a cancel path 10 times
slower (and anything up to about 40 times) still passes. The limit stays as registered; whether S7 should get a tighter limit or a
relative one is a design decision (section 6.3 item 5); R3 is item 6. First attempts that did not count: J3 did not compile (a `f64` times a
complex value) and L6 had a duplicated test flag; both were corrected and rerun. The first S7 run failed only because the scratch
target had no `rcfd.exe` (the test says "build it: cargo build -p rcfd-cli"), not because of the mutation; after building it the
mutated S7 passed.

Test list (A5): no test was added, removed or edited, so the list is unchanged (checked again in the final gate).
