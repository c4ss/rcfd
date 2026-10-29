# ADR 0163: Job 48AC, ADR 0155's H1 (`case_hashes_w48u`) compared after a documented normalisation of the one new default-valued key `solver.corrector_control`; rcfd-mcp `t2` gets its own temp parent

Status: **registered** 2026-10-29, before any code of this job was written and before any run of this job (set-ups, limits
and the not-run list are fixed here); results in the addendum below. House rules of
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
