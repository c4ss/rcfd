# ADR 0190: Expected hashes in table files, and the Linux value of `w49r_default_meshes_are_bit_identical`

Status: **REGISTERED 2026-11-05**, committed before any changed test ran. The probe that measured the Linux values ran first and is
disclosed below. Not yet accepted by the maintainer. Results go in the addendum at the end.

Read at main 201ea17. F-274's registration took number 0189 on main first, so this ADR is number 0190; it was registered as 0189 and renumbered before merge.

The registered expected hashes of the tests under `crates/*/tests` move out of the test source into one table file per test file,
`tests/fixtures/expected_hashes/<test_file>.tsv` of the crate, read with `include_str!` through one shared helper. No value changes,
and each test behaves as before on each platform. `crates/rcfd-api/tests/baseline_hashes.rs` is moved as values only, in a commit of
its own that the maintainer can drop without touching the others. One thing is added: x86_64 Linux values for
`w49r_default_meshes_are_bit_identical`, B3 `0xb2563dff49086f53` and B2 `0xb9ff46541fae1983`, which meet the rule of
[ADR 0137](0137-linux-gate-platform-values.md) at reference commit f556d86 and at HEAD 201ea17 in one Ubuntu 24.04 container
(glibc 2.39). B3 equals the CI value; CI never printed B2, so the next Linux full gate is the first outside check of it.

House rules as in [ADR 0130](0130-wave48a-overview-and-run-record-contract.md): no limit changes; defaults of existing code stay
bit-identical; `baseline_hashes.rs` is not re-recorded. No production code changes, no `unsafe`, no new crate, no new dependency, no
test caching.

## The problem

The Ubuntu `Full gate` on eef6c09 failed `w49r_default_meshes_are_bit_identical`
(`crates/rcfd-api/tests/fan01_hub_mesh_w49r.rs:624`): B3 has the expected 4920 cells, but its content hash on CI is
`0xb2563dff49086f53` (decimal 12850526753399533395) against the expected `0xbe64f588a252d0b4`. The test stopped at B3, so CI did not
print B2. The test was added after ADR 0137 and has only the Windows values. The mesh generators call the platform's libm, whose
last bits differ between the Microsoft CRT and glibc.

Separately, the expected values are scattered through the test source in several forms: `0x` literals with and without separators,
16-digit hex strings, decimals, SHA-256 strings, tuples with step counts, and `cfg!(windows)` branches. Adding a platform value means
editing test logic each time, and nothing shows the values of one test side by side with where they came from.

## Decision

### Linux value for the w49r test

The rule is ADR 0137's, unchanged. A Linux value is accepted only if, in one Linux container, that value comes out of the test's
computation at the test's reference commit and at HEAD and, where the CI log prints it, equals the CI value. Otherwise no value is
recorded and the test is reported.

The reference commit is **f556d86**, the parent of dd73d0c. The test's comment says the hashes were "recorded on the unchanged
generator", and ADR 0181 gives the same two values for "the unchanged generator".

* dd73d0c is the commit that changes the generator (`crates/rcfd-mesh/src/fan_cgrid.rs`, `crates/rcfd-api/src/fan_case.rs`), so it is
  the code under test, not the reference. Its diff of this test keeps the two values and adds the explicit `"capped"` form.
* a4e9402 changes only the settings hash of the key (`hash_omit_default`) and adds another test. It does not touch the mesh and is
  part of HEAD.
* 776d2e0 added the test. Between it and f556d86 no commit touches `crates/rcfd-mesh`, `crates/rcfd-core`, the fan case code or
  `Cargo.lock` (ADR text only), so f556d86 is the last commit with the unchanged generator and the test's own text.

The test's computation at f556d86 has one form, the case without the key, because `c_grid_grading` does not parse there. The probe
prints that form on both commits, and at HEAD also the case with the explicit default. `case_toml`, `F3` and `F2` are the same text
on both commits.

With Linux rows added, the test prints and skips on any platform other than Windows and x86_64 Linux, as ADR 0137 sets out. Before,
it asserted the Windows values everywhere, which could only pass on Windows.

### Table files

Each test file with registered hashes gets `tests/fixtures/expected_hashes/<test_file>.tsv` in its crate. The columns are key,
platform (`any`, `windows`, `linux-x86_64`) and hash, separated by tabs. `#` lines carry the provenance the source comments carried
(commit, ADR, how the value was produced), moved rather than dropped.

* A hash is written as `0x` and 16 lowercase hex digits. Values that were decimals or quoted strings in the source are normalised to
  this form; they are the same numbers. SHA-256 values stay as 64 hex digits.
* The helper is `crates/rcfd-core/tests/support/expected_hashes.rs`, included with `#[path]`. It adds no crate and no edge to the crate
  graph. `crates/rcfd-core/tests/expected_hashes_table.rs` tests it, with negative controls, and parses every table of the workspace.
* An unknown key, a duplicate row, an `any` row beside a platform row, an unknown platform and a malformed hash all fail with a
  message naming the table and the key or line.
* A shared value stays `any`. A `cfg!(windows)` split becomes two rows, also where the two values are equal (`cube`, `sphere`, the
  cavity fingerprints of `w33b_default_bits`), because `any` would make a third platform assert where it now prints. A site that prints
  on other platforms keeps printing. A value that is compared on all platforms keeps `any`, even if it was recorded on Windows (the
  coupled row of the report-only `w34c_component_diagonal_default`).
* Cell and step counts stay in the source: they are not hashes, and the tests use them as limits with their own per-platform logic.

Scope is every hard-coded expected content or bit hash asserted or compared in a test under `crates/*/tests`. That includes the SHA-256
expected values of `render_speed_w48q` and `aoa_sweep_w49d`, which a grep for `0x` and 16 digits does not find, and the decimal
fingerprints of `w41d_tet_quality`.

Left in place because they are not expected hashes:

* the FNV-1a offset basis and prime, and the other multipliers and seeds (`0x9E3779B97F4A7C15`, `0x2545F4914F6CDD1D`, the LCG constants,
  the signalling NaN pattern in `rcfd-tsstore/tests/store.rs`);
* `X43A_EY`, `X43A_EV` and the Linux pair of `vof_bubble_refinement_w40d.rs`, which are error values compared with a 1e-9 tolerance
  (ADR 0137 already says so);
* the `fnv64:...` strings of the journal tests (`rcfd-project` and `rcfd-workspace`), synthetic input data;
* the sample key strings of `rcfd-assistant` tests, which only look like hashes;
* `mesh_transform_u`'s comparison with a committed run's mesh hash, which reads the value from a file;
* `w48ab_fingerprints`, which only prints: it has no expected value.

`crates/*/src` and `crates/rcfd-api/tests/fixtures/case_hashes_6867e40.txt` (already a table) are out of scope, as is the
`non_conformal.rs` precedent in `src`.

### The fixture walkers

`crates/rcfd-api/tests/case_hashes_w48u.rs` walks `tests/fixtures` of every crate, but only takes `.toml` and `.json`, so `.tsv` files
are not cases and its recorded fixture is untouched. `w48v_case_hashes` walks only `cases/` and the example projects. The other tests
that read fixtures open named files or named folders. The workspace test above also requires every file in a table folder to be a
`.tsv`, so this cannot change unnoticed.

### Checks

1. A script outside the repo compares all hash literals of each test file at 201ea17 with the table rows, as numbers (SHA-256 as text).
   The only difference allowed in the whole branch is the two new Linux rows of the w49r test, plus the platform of its two existing
   rows going from `any` to `windows` in the Linux commit.
2. `baseline_hashes.rs` is moved in its own commit, and the commits after it apply and build without it.
3. On Windows, release: `cargo fmt --all --check`; clippy `-D warnings` on the changed crates, all targets; the changed test files (at
   least the non-ignored tests), `case_hashes_w48u` and `w48v_case_hashes`.
4. In the Linux container: the same changed test files, with the w49r test passing on its Linux values.

## What was run before this ADR was written (disclosed)

One probe, on `git archive` copies, before any test was changed: Docker image `rcfd-linux:ubuntu24.04-rust1.99.0`, built from
`scripts/docker/ubuntu-24.04-rust.Dockerfile` on the local `ubuntu:24.04`, `--cpuset-cpus 0-3 --memory 10g --init`, release, one
container. glibc 2.39-0ubuntu8.9, rustc 1.99.0, the same glibc version as the CI image. The older `rust:1.99-bookworm` (glibc 2.36) was
not used. A `#[test]` appended to the copy of the test file printed B3 and B2 without asserting, using the file's own `case_toml`.
The same container ran both commits in turn.

| Form | f556d86 (reference) | 201ea17 (HEAD) | CI log, eef6c09 | Windows (registered) |
|---|---|---|---|---|
| B3, case without the key | 4920 cells, `0xb2563dff49086f53` | same | `0xb2563dff49086f53` | `0xbe64f588a252d0b4` |
| B3, `c_grid_grading = "capped"` | key does not parse | 4920 cells, `0xb2563dff49086f53` | not printed separately | same as above |
| B2, case without the key | 13627 cells, `0xb9ff46541fae1983` | same | not printed | `0xcae52c4997c9463f` |
| B2, `c_grid_grading = "capped"` | key does not parse | 13627 cells, `0xb9ff46541fae1983` | not printed | same as above |

So the rule is met for both values: B3 on all three conditions, B2 on the two that can be checked. Between eef6c09 and 201ea17 there is
no change in `crates/rcfd-mesh`, `crates/rcfd-api/src`, `crates/rcfd-core` or `Cargo.lock`. The B3 value is also the one ADR 0137
measured for `g6_fan_meshes_unchanged` in the older image and registered for that test.

## What was not run

The full Linux and Windows workspace gates. The ignored tests of the changed files, which only compile here, including
`rotating_channel_matches_pre_adr0005_hash`, the report-only `w34c_component_diagonal_default` and the `render_speed_w48q` Q3 (2.7 GB)
test. macOS and other platforms, where a test with split rows prints and skips by reading the code, not by a run.
