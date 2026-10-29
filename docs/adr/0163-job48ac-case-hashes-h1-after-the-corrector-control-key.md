# ADR 0163: Job 48AC, ADR 0155's H1 (`case_hashes_w48u`) compared after a documented normalisation of the one new default-valued key `solver.corrector_control`; rcfd-mcp `t2` gets its own temp parent

Status: **registered** 2026-10-29, before any code of this job was written and before any run of this job (set-ups, limits
and the not-run list are fixed here); **results in the addendum** (2026-10-29: L1, L2, L3 met; L4, L6, L7 not run and L5
incomplete because the suite run was stopped by the session's low-memory reaper; two failures in the partial L5 run, both
outside this job's files, see Q1; one set-up deviation, D-1). House rules of
[ADR 0130](0130-wave48a-overview-and-run-record-contract.md): limits fixed before the runs; defaults stay bit-identical (no
library code is touched); `crates/rcfd-api/tests/baseline_hashes.rs` is not edited or re-recorded; the record
`crates/rcfd-api/tests/fixtures/case_hashes_6867e40.txt` (R0) is not edited or re-recorded; no unsafe code, no new crate.
Parents: [ADR 0155](0155-job48u-rotate-the-mesh-instead-of-the-flow.md) (H1 and its record R0, base 6867e40),
[ADR 0156](0156-job48v-adaptive-pressure-correctors.md) (the key `solver.corrector_control`, default `"fixed"`,
`Schema::hash_omit_default`, D6), [ADR 0160](0160-job48z-end-time-stamp-corrector-count-record-sv301-wiring.md) deviation 3 /
question 6 and [ADR 0162](0162-job48ab-adaptive-correctors-follow-the-time-step.md) question 6 (both report the H1 failure and
the `%TEMP%\x` failure of rcfd-mcp `t2` as pre-existing). Number: no ADR 0163 or above exists in this worktree, the main
checkout or any sibling worktree on disk (checked 2026-10-29). Base: local main 6366587 (the worktree was fast-forwarded to it
before any work).

## The problem

H1 (ADR 0155) asserts that every case file of the repository is unchanged by `[mesh.transform]`: for each case source the
settings content hash, the FNV-64 of the text `Case::to_toml` and `Case::to_json_string` write, and the mesh content hash equal
R0 (made on 6867e40). ADR 0156 later added `solver.corrector_control` (default `"fixed"`). Every valid case's settings tree is
filled with schema defaults, so the written TOML/JSON now lists the new key; the settings hash omits the key at its default
(`hash_omit_default`), so settings and mesh hashes still match, but the text columns of every valid source differ (ADR 0160:
"the same 30 entries differ"). H1 has failed on main since ADR 0156.

## Decision (set-up)

S1. **Normalisation, not re-recording.** The test keeps R0 as the only record. The `toml` and `json` columns of the current
code are computed from the case's settings value (`c.tree.to_json()`) with exactly one entry removed: `solver.corrector_control`,
and only when its value is the string `"fixed"` (the default). The removal uses `serde_json::Map::shift_remove` (order-preserving
whatever serde_json features are unified). The normalised value is serialised with the library's own serialisers
(`toml::to_string_pretty` as `Case::to_toml`, `serde_json::to_string_pretty` as `SettingsTree::to_json_string`). The
`settings` and `mesh` columns are not normalised (computed exactly as before).

S2. **Controls inside the test** (all asserted, every run):
* C1 (the normaliser serialises as the library does): for every valid source, serialising the *unmodified* value with the same
  two functions gives byte-identical text to `Case::to_toml()` / `Case::to_json_string()`.
* C2 (the key is present at its default): every valid source's value has `solver.corrector_control == "fixed"`; a source with
  another value is not normalised and so fails H1 against R0 (a non-default value is a real difference).
* C3 (the difference is exactly the key): the number of valid sources whose raw (unnormalised) text columns differ from R0 is
  counted and printed; after normalisation every R0 line must equal the current line (H1 as before). Together with C1 this
  shows the only difference between the written text now and on 6867e40 is the one removed entry.
* C4 (H1's sensitivity is kept): a mutated copy of one valid case (`solver.corrector_control = "adaptive"`) gives a normalised
  line different from its R0 line, so the normalisation cannot mask a non-default value.
* The existing R0 control (`unknown setting 'transform'`) and the `valid >= 20 && meshes >= 20` set-up check are kept.

S3. **The record mode** (`RCFD_W48U_RECORD=1`) is unchanged in meaning (writes raw columns; for the base code only). Not run.

S4. **rcfd-mcp `design_tests::t2`**: first confirm (run D1) that it passes on unmodified code with `TEMP` and `TMP` pointing at a
fresh empty directory. The defect: `sandbox("t2")` puts the sandbox root directly under `std::env::temp_dir()`, so the bad
paths `../x` and `a/../../x` resolve to `%TEMP%\x`, and the assertion `!root.parent().join("x").exists()` depends on the
shared temp directory. Fix (cheap, test-only): t2's root is nested one level deeper, under its own fresh parent
(`temp_dir()/rcfd_mcp_design_t2_parent_<pid>/root`, removed and recreated at the start), so `root.parent()/x` is private to the
test. `sandbox` gets a helper `sandbox_at(root)`; other tests keep their roots. Nothing on disk outside the test's own
directories is touched (the stray `%TEMP%\x` dated 2026-10-15 is left alone).

## Limits (fixed before the runs)

| id | what | limit |
| --- | --- | --- |
| L1 | `case_hashes_w48u` H1, normalised | passes; every R0 line found and equal; C1, C2, C4 hold |
| L2 | C3 | raw text columns differ for every valid source whose tree has `[solver]` filled by defaults (expected: all valid R0 lines, ADR 0160 counted 30), and for no invalid line; settings and mesh columns differ for none |
| L3 | D1: rcfd-mcp `t2` on unmodified code, `TEMP`/`TMP` = fresh empty dir | passes |
| L4 | rcfd-mcp `t2` after the fix, normal `TEMP` (with the stray `x`) | passes |
| L5 | `cargo test -p rcfd-api --no-fail-fast` | 0 failures |
| L6 | `cargo test -p rcfd-mcp` | 0 failures |
| L7 | `cargo clippy -p rcfd-api -p rcfd-mcp --all-targets -- -D warnings`; `cargo fmt -p rcfd-api -p rcfd-mcp --check` | clean |

A missed limit is reported in the addendum, not hidden; no limit is changed after a run.

## Not run (by design)

* The record mode `RCFD_W48U_RECORD=1` (R0 stays the 6867e40 record).
* `baseline_hashes.rs` beyond its normal run inside L5 (not edited).
* Test suites of crates other than rcfd-api and rcfd-mcp (no other crate is touched).
* Linux/macOS (H1 compares mesh hashes only on Windows, ADR 0137; unchanged).

## Addendum (results, 2026-10-29)

Branch `worktree-agent-a3060ad185b4fedf6`, base local main 6366587. Commits: 5d6aeb6 + 1a9d436 (this registration, before any
code or run), 47c253e (the test changes). Files changed: `crates/rcfd-api/tests/case_hashes_w48u.rs`,
`crates/rcfd-mcp/src/design_tests.rs`, this ADR. Not changed: the record R0, `baseline_hashes.rs`, any library code.
`CARGO_TARGET_DIR=E:\projects\cfd\rcfd\target-48ac`.

| id | limit | result | verdict |
| --- | --- | --- | --- |
| L1 | H1 passes after normalisation; C1, C2, C4 hold | `h1_case_files_are_unchanged_against_the_base_record ... ok`: "41 recorded sources (30 valid, 30 meshes built), 44 now, 0 differ"; C1 asserted for every valid current source; C2 asserted for the 30 recorded valid sources; C4: `cases/axial_fan_cgrid/axial_fan_cgrid.toml` with `corrector_control = "adaptive"` gives toml 5f70e423bab8ce24, json cdfaf1b0e42d6037, neither equal to its R0 columns | met |
| L2 | raw text columns differ for the valid sources only, settings and mesh columns for none | "30 of 30 recorded valid sources differ raw (text columns only), 0 after normalisation"; the C3 assertion (settings and mesh columns equal raw for every recorded source) holds; the 11 invalid R0 lines are not normalised and all equal R0 | met |
| L3 | D1: unmodified t2 with `TEMP`/`TMP` = fresh empty dir passes | `design_tests::t2_paths_are_sandboxed_and_names_are_restricted ... ok` (TEMP=TMP=a fresh scratch dir `tmp48ac_d1`); control with the normal TEMP on the same unmodified code: fails at `design_tests.rs:227` `assertion failed: !root.parent().unwrap().join("x").exists()` | met |
| L4 | fixed t2 with normal TEMP passes | not run (D-2) | not run |
| L5 | `cargo test -p rcfd-api --no-fail-fast`: 0 failures | stopped after 88 test binaries had finished (about 3 h of the debug suite, the machine shared with other jobs); in those: 521 passed, 2 failed, 178 ignored; `case_hashes_w48u` passed inside it | incomplete; 2 failures (Q1) |
| L6 | `cargo test -p rcfd-mcp`: 0 failures | not run (D-2) | not run |
| L7 | clippy `-D warnings` and fmt `--check`, rcfd-api and rcfd-mcp | not run as registered (D-2); `cargo fmt -p rcfd-api -p rcfd-mcp` was applied before the test run and commit 47c253e, and both test builds compiled without warnings | not run |

What the record shows (the brief's "show it"): for all 30 valid R0 sources the settings content hash and the mesh content hash
(compared, Windows) of the current code equal R0 unchanged; the raw toml and json hashes of all 30 differ; after removing the
single entry `solver.corrector_control = "fixed"` from the settings value, both text hashes of all 30 equal R0, and C1 shows
that the serialisation used is byte-identical to `Case::to_toml` / `Case::to_json_string`. So the written text of every
recorded case differs from 6867e40 by exactly that one default-valued key. Three sources not in R0 (the `docs/case-schema.md`
block 46e761937bab06b8, `example-projects/wind-tunnel/cases/naca0012-aoa{2,4}-rotated.toml`) are printed, not compared, as H1
already did.

The stray `%TEMP%\x` (dated 2026-10-15) still exists after the job (`Test-Path` true); nothing outside the tests' own
directories was deleted.

### Deviations

D-1. S1 registered `serde_json::Map::shift_remove`; it does not compile here (the method needs serde_json's `preserve_order`
feature, so this build's `Map` is the key-sorted BTreeMap). The test uses `Map::remove`, which on a BTreeMap cannot reorder
the remaining keys; with `preserve_order` it could only swap keys, which would make the line differ from R0 (a false failure,
never a false pass). Changed after the compile error and before the first H1 run; no amendment was written beforehand.

D-2. The background chain (rcfd-api suite, then rcfd-mcp suite, clippy, fmt check) was stopped by the Claude Code session's
low-memory reaper during the rcfd-api suite; as instructed by that notice it was not restarted. Hence L4, L6, L7 not run and
L5 incomplete.

### Not run

L4, L6, L7, the rest of L5 (the rcfd-api binaries after the 88th), and the registered not-run items (record mode, other
crates, Linux/macOS).

### Questions

Q1. Two failures in the partial L5 run, both in files and code this job did not touch (it changed test files only):
* `baseline_hashes::shipped_cases_match_registered_hashes`: `kepsilon_channel: got (60, da0ee795fd177d8e), registered (60,
  6ddc55969af2a835)`. ADRs 0158, 0160 and 0162 record this case as known to fail on this machine at earlier commits
  (intermittently: it passed in the 0160 and 0162 runs).
* `fan01_stall_w43c::w45a_p5_defaults_are_bit_identical_to_main`: absent and `none` 0xb1aed685ee7279c0 (guard
  `velocity_runaway`), `barth_jespersen` 0x3cd6036eafc92b11, against the recorded 0xec721116c0963182 / 0x26e5a0c524957698.
  ADR 0162 reports H1 as rcfd-api's only failure at its base, so this is either new on main 6366587 (the merge of 48Y, 48Z,
  48AA, 48AB) or load-dependent like `kepsilon_channel`. Not investigated (out of scope; not rerun because of D-2).
  Recommendation: rerun the two tests alone on an idle machine at 6366587 and at 444255c; if 6366587 fails and 444255c passes,
  bisect the four merges.

Q2. Rerun L4, L6, L7 and the full L5 when memory allows (the commands are those registered).
