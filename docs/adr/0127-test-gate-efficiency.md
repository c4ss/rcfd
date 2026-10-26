# ADR 0127: A faster release test stage with the same tests (process per test, one pool)

Status: **ACCEPTED 2026-10-26** (goals registered before the first run of the new runner). Written at main cc0a8cb on branch `weff`,
worktree `<scratch dir>`, target `<target dir>`. Measurements and the full option list are in
[`docs/test-process.md`](../test-process.md).

## Context

The gate is `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` and
`cargo test --workspace --release --no-fail-fast` on a `git archive` copy, with `CARGO_BUILD_JOBS=3` and `RUST_TEST_THREADS=3`.
Measured from the logs of seven gates of waves 46 and 47 (no new run needed):

| Phase | Time |
|---|---|
| clippy (dev profile check) | 15 s to 75 s |
| release build of all test targets | 4 min 24 s to 7 min 54 s |
| running the test binaries (sum of `finished in`) | 72.6 min to 95.9 min |

The run phase is 92 % to 95 % of the test stage. Cargo runs the 400 test binaries one after another; inside a binary three threads share
the tests. A binary with one long test keeps the other two threads idle: `validation_blasius_plate` is one test (296 s), and
`transient_sources` alone took 848 s. A running test binary of another agent's gate used about one core (151 CPU seconds in about
150 s wall) and 30 MB to 135 MB of working set, so three tests from different binaries can overlap without new CPU or memory pressure
beyond today's three test threads.

## Decision

1. **D1** New script `scripts/gate_test.py` (Python standard library only; no new dependency, no build cost) replaces the
   `cargo test` line of the gate's test stage. It
   * builds with `cargo test <selection> --release --no-fail-fast --no-run` (the same command and profile as today, so the binaries
     are the ones `cargo test` would run);
   * lists every test binary (`--list`, `--list --ignored`) and runs each non-ignored test as its own process,
     `<binary> <name> --exact --test-threads=1`, from the package root, from one pool of `--jobs` processes (default
     `RUST_TEST_THREADS`, else 3: the same number of tests at once as today);
   * counts a test as passed only if the process exits 0 **and** its summary says exactly `1 passed; 0 failed; 0 ignored` (so a crash,
     an abort or a name that matches nothing is a failure);
   * prints the libtest lines (`Running`, `test X ... ok`, `test result:`), so the gate's summing `awk` is unchanged;
   * runs `cargo test <selection> --release --no-fail-fast --doc` afterwards for the doc-tests;
   * exits 0 only if the build, every test and the doc-tests pass.
2. **D2** No test is skipped and no result is cached. `--order <previous tsv>` only starts the slowest tests of an earlier run first;
   without it the order is binary by binary. Ignored tests stay ignored and are counted, not run.
3. **D3** No change to `Cargo.toml` profiles, test code, limits or solver code. The original `cargo test` command stays valid and
   can be run at any time as the reference.

## Registered goals (fixed before the first run)

| Id | Goal | Measured how |
|---|---|---|
| G1 | Equivalence: the multiset of (binary stem, test name, outcome) from the new runner on a `git archive` of 21557f6 equals the one from the original `cargo test` log of the wave 47E gate on the same commit (2093 passed, 0 failed, 659 ignored): **0 differences** | `diff` of both sorted lists |
| G2 | Negative controls in a throwaway copy: a panicking integration test, a panicking unit test and a `std::process::abort()` in a test are each reported FAILED, the stage summary shows them in `failed`, exit status non-zero: **3 of 3** | runner on the changed crate |
| G3 | Wall time of the test stage (build + tests + doc-tests) at 21557f6 with `--jobs 3`: **at most 51 min**, half of the original 102 min on the same commit | timestamps of the run |
| G4 | Memory: never more than `jobs` test processes at once, and the summed working set of the runner's test processes **at most 1.5 GB** at every 5 s sample; build phase unchanged | `tasklist` sampler, per-test peak working set in the TSV |
| G5 | After rebasing on main: the set of tests the runner executed and ignored equals the union of `--list` of the built binaries: **0 differences**; fmt and clippy clean | runner output vs listing |

A missed goal is reported as missed in the addendum with the measured value; the goal is not moved.

## Consequences

* Process per test is more isolation than today, not less: environment changes (`set_var` in five rcfd-assistant tests) and global
  state no longer leak between tests. Tests of different binaries now overlap. Audit before the first run: every fixed temp path
  (`rcfd_particles_case_out`, `rcfd_timectl_restart`, `w44a_jac`, `w37a_jac`, `elsewhere.toml`, the `rcfd-gui-*` folders,
  `rcfd_w26c_ensight_write`) and every file written under `CARGO_TARGET_TMPDIR` belongs to a single binary, so no two binaries share
  one; within a binary they already ran concurrently on three threads. Pid-suffixed folders can meet a reused pid; that gives the
  sharing a single-process binary already had, nothing new.
* Startup of about 2100 processes adds seconds, not minutes.
* The original `cargo test` remains the reference; if the runner and it ever disagree, the runner is wrong.
