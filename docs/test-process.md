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
* Peak memory during a release build with `-j 3`: three `rustc` plus links; the passive sampler saw at most 0.4 GB of `rustc` working
  set from one gate.

### 2.4 Memory while testing

The passive sampler (all agents, 5 s) saw single test processes from 30 MB to 135 MB for the rcfd-fv and rcfd-api solver tests, and
one test of a wave 47 branch (`rcfd-tsstore` `scale`) at 1.4 GB. Memory pressure comes from many agents building at once more than
from the test stage.

## 3. Options, ranked by saving against risk

Times are for the test stage at 47E (103 min). "Robustness" is what the option could cost.

| Rank | Option | Saving | Robustness cost | Done? |
|---|---|---|---|---|
| 1 | Run tests of different binaries at the same time: one process per test from one pool of 3 (`scripts/gate_test.py`) | measured in section 5 | tests now overlap across binaries (audited: no shared fixed path); more isolation inside a binary than today | **yes** (ADR 0127) |
| 2 | Start the slowest tests first, using the TSV of an earlier run as an ordering hint | the tail after the pool drains (section 5) | none: every test still runs; a missing file falls back to binary order | yes, as `--order` |
| 3 | A persistent release target folder per slot (not per gate), cleared of `target/tmp` before each run | 2 min to 4 min of the 4.4 min to 7.9 min build (the 173 dependencies), and 4.4 GB of disk per gate | a stale artefact is possible only through a cargo fingerprint bug; workspace crates still rebuild because `tar -m` gives new times | proposed |
| 4 | Lower the pool's memory: nothing to gain; tests are 30 MB to 135 MB each | none | none | no |
| 5 | `debug = 0` for the test profile | none: the release profile already has no debug info | none | no |
| 6 | `lld-link` instead of `link.exe` (the linker is in the Rust toolchain as `rust-lld`) | part of the link time of 384 test binaries, at most a minute or two of the build | a second linker in the gate; Windows `rust-lld` is not the default for MSVC targets | proposed, measure first |
| 7 | Merge `tests/*.rs` into fewer binaries | fewer links (365 today), at most a few minutes | test names change (module prefix), so the before/after list no longer matches by name; one panic that aborts kills the whole binary | no |
| 8 | sccache | dependency compiles across agents and gates | a cache outside cargo; a new tool (Apache-2.0); not needed if option 3 is taken | no |
| 9 | One target folder shared by all agents with a lock | disk and dependency compiles | agents block each other on the build lock; a broken build in one folder affects all | no |
| 10 | `cargo nextest` (Apache-2.0 or MIT) | the same as option 1 | a new tool to install and pin; the stdlib script does the same scheduling | no |
| 11 | Split the gate: fast tier (fmt, clippy, crates touched by the diff and their reverse dependencies from `cargo metadata`) and the full tier still mandatory before a merge or a wave | minutes for an agent's own checks | the fast tier skips tests whose code did not change, which the user ruled out for now ("no test caching"); allowed only as a pre-check, never as the gate | proposed only |
| 12 | Shard one gate across both slots | half of the run phase | doubles one gate's concurrency against the two-slot rule; the wave queue gets slower | no |
| 13 | Ignore long tests by default, with a named `--ignored` full run that stays in the final gate | large for agents' own runs | ignored is already used for `*_known_miss` tests; two meanings of ignored make the count harder to read | no |
| 14 | Result caching or skipping unchanged tests | most of the run phase | not allowed now (user decision) | no |

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

To be filled from the registered runs (ADR 0127 addendum).
