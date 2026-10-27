# The test and gate process: measurements and options

Status: measured 2026-10-26 on the gate machine (16 logical processors, 96 GB, shared by up to six agents and two gate slots).
Decision record: [ADR 0127](adr/0127-test-gate-efficiency.md). No test, limit, profile or solver code was changed.

## 1. What the gate runs today

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --release --no-fail-fast        # CARGO_BUILD_JOBS=3, RUST_TEST_THREADS=3
```

It runs on a `git archive HEAD` copy with a fresh `CARGO_TARGET_DIR`, inside one of two slots (`mkdir <scratch dir>` or
`slot-2`).

## 2. Measured baseline

Sources: the logs of seven gates of waves 46 and 47 (`gate46*/`, `gate47*/` under `<project dir>`), a passive `tasklist` sampler
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
| 47E (21557f6) | 65 s | 6 min 26 s | 95.8 min | 2093 / 0 / 659 |

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
* Every gate starts with an empty target folder, so all 190 units are compiled again; the ten gate target folders of waves 46 and 47
  hold 3.8 GB to 5.1 GB each, 46 GB together.
* The 13 agent target folders (`target-w45a` to `target-w47e`) hold 0.2 GB to 23 GB each, 60 GB together, mostly `debug`; each agent
  compiles the same 173 dependencies once per profile.
* Peak memory during a release build with `-j 3`: three `rustc` plus links. The `tasklist` samplers saw up to 1.6 GB of `rustc`
  working set at once on the machine (several agents) and 0.9 GB to 1.1 GB (all agents counted) while this work's release builds ran.

### 2.4 Memory while testing

Per-test peak working set from the new runner at 21557f6 (exact, from the process counters): 2087 of the 2093 tests stay under
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
| 2 | More test processes per slot (`--jobs 5` or `6`) | at 3 the stage sits on its floor (test-seconds / 3 = 59 min); at 6 the floor is 29 min plus contention | CPU: a solver test uses about two cores, so 6 jobs in each of two slots ask for 24 of 16 processors; memory: worst case the six largest tests together, 9.6 GB, typical under 1 GB | proposed: needs a registered measurement and the user's decision on the concurrency rule |
| 3 | Start the slowest tests first, using the TSV of an earlier run as an ordering hint | small: binary order already finished 0.6 min above the floor (section 5) | none: every test still runs; a missing file falls back to binary order | yes, as `--order` |
| 4 | A persistent release target folder per slot (not per gate), cleared of `target/tmp` before each run | 2 min to 4 min of the 4.4 min to 7.9 min build (the 173 dependencies), and 4.4 GB of disk per gate | **measured pitfall** (section 5.4): binaries do not depend on the source folder's path, so a target shared by two copies reuses the other copy's binaries when the new copy's files are older than the last build; safe only if the copy is extracted with `tar -m` right before its build | proposed, with that rule |
| 5 | A time limit per test that fails the test (for example three times its last time, at least 30 min) | frees a slot from a hung test (section 2.4: about 3.5 h on 2026-10-26) | a very slow test that would have passed now fails; the limit must not hide behind a retry | proposed |
| 6 | Less memory per test thread | nothing to gain: 2087 of 2093 tests stay under 500 MB; the large ones are large by design (one-million-cell render, C-grid stacks) | none | no |
| 7 | `debug = 0` for the test profile | none: the release profile already has no debug info | none | no |
| 8 | `lld-link` instead of `link.exe` (the linker is in the Rust toolchain as `rust-lld`) | part of the link time of 384 test binaries, at most a minute or two of the build | a second linker in the gate; Windows `rust-lld` is not the default for MSVC targets | proposed, measure first |
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
python scripts/gate_test.py --tsv ../times.tsv --order <scratch dir>.tsv > ../test.log 2> ../progress.log
echo "test_exit=$?" > ../stage_test.txt
grep "^test result" ../test.log | awk '{p+=$4;f+=$6;i+=$8} END{print p" passed "f" failed "i" ignored"}' >> ../stage_test.txt
grep -E "^test .* FAILED" ../test.log >> ../stage_test.txt
cp ../times.tsv <scratch dir>.tsv
```

`progress.log` gets one line per finished test (outcome, seconds, peak working set). `times.tsv` holds the same per test. The old
command stays valid as the reference: `cargo test --workspace --release --no-fail-fast`.

## 5. Results

Registered run 1 (ADR 0127): `git archive 21557f6` in `<scratch dir>\src`, `--jobs 3`, no `--order`, slot 1 from 23:25 to
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

Throwaway copy `gate-weff/neg` of 21557f6 with three breaks in rcfd-core, own target folder `target-weff-neg`, runner and original
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

`git archive` of `weff` at e7fbbb1 (main 93fe245 with this change; two more crates, rcfd-project and rcfd-runner), `--order` from
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
