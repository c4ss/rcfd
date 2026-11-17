# ADR 0712: F-790 registration: delete the dormant rule H3 code and its record tests after the second reading

Status: **REGISTERED 2026-11-17, written before any code of F-790 and before any run of its checks.** Written by the implementation
agent on branch `f790-work` (from b3af848a, which holds F-772 / ADR 0701, F-786 / ADR 0704 and F-785 / ADR 0703), worktree
`w49i-work/f790`, scratch `w49i-work/f790-scratch`. Not yet accepted by the maintainer. Every choice below that the maintainer would
normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation) and is marked
"(standing instruction)".

Roadmap item F-790 (deps F-786), the follow-up proposed by ADR 0703 (its remaining question 3, repeated by ADR 0704). The deps are
met: ADR 0704 finished the second reading of the files of ADR 0701. No default of a solver changes; the work is in `scripts/` and the
ADR text only. House rules (`w49i-work/AGENT_RULES.md`) apply: limits fixed here and not changed after the run, each run made once,
no golden or hash table edit, `gate_attribute.py` and `gate.yml` not edited, ADRs 0679, 0684, 0690, 0701, 0703 and 0704 not edited.

## What F-790 asks

> Every limit not stated here is registered in the ADR before the run. The constant `RETRO_H3`, the tier-3 branch of `retro_units` and the tests of ADR 0701 and ADR 0703 that depend on it are removed in one commit; the index of `--primary git` is equal to the index before the commit in every field, `--primary registered` and `p0679` are equal, `gate_attribute.py` is unchanged, and `python -m unittest discover -s scripts` passes.

## What was read beforehand (disclosed)

`AGENT_RULES.md`; the brief; `scripts/adr_test_index.py` (the docstring lines 25 to 33, the constant at line 57, `retro_units` at
lines 279 to 330, `retro_primaries` at lines 659 to 682, the report line 767); `scripts/test_adr_test_index_h3.py` and
`scripts/test_adr_test_index_retro.py` in full; the "Remaining questions" and "For the lead" sections of ADR 0703 and the form of ADR 0704.

Measured beforehand (a baseline, not a result of F-790): `python -m unittest discover -s scripts` on b3af848a runs 139 tests, OK
(4 skipped). `cargo metadata --no-deps --format-version 1` was saved to `f790-scratch/meta.json` so that every index run uses the same
metadata.

## Design

The only user of the dormant code is the loop `for i, c in enumerate(cells if RETRO_H3 else ())` of `retro_units`, which with the
constant False iterates over an empty tuple. Removing the loop, the constant and the `cells` variable that only the loop uses cannot
change a return value of `retro_units`, since nothing was appended with tier 3 after ADR 0703. The name `RETRO_LIMIT` (used only by
that loop) is also removed if a grep shows no other use; it is not a named item of the acceptance, so this is registered here and the
removal is part of the same commit.

What stays, so that the index is equal in every field (standing instruction, the safe reading of "in every field"):

* the key `h3` of `coverage.primary.retro` (the value is 0 before and after), set in `retro_primaries` (`"h3": 0` in `stat`) and read
  by the report line. The line that counts `stat["h%d" % best[0]]` stays as it is;
* the text of the report line, which says "H3 removed by ADR 0703";
* the docstring of the module is updated in one place only: it no longer names the constant, and says that rule H3 was removed by
  ADR 0703 and its code deleted by ADR 0712. A docstring is not part of an index.

Tests removed (the ones that cannot pass without the constant, or that assert tier 3):

| file | removed | why |
|---|---|---|
| `test_adr_test_index_retro.py` | the lines of `setUpClass` and `tearDownClass` that save, set and restore the constant | the name no longer exists |
| `test_adr_test_index_retro.py` | `test_p2_p3_p4_tiers` | asserts `p4_h3` gets ADR 0116 from tier 3 |
| `test_adr_test_index_retro.py` | `test_counts` | asserts `h3` equals 1 and the file list of tier 3 |
| `test_adr_test_index_h3.py` | `test_constant_is_off` | asserts the constant |

The other tests of both files stay unchanged. Their fixtures that name H3 (`r_h3.rs`, the table in ADR 0116 of the fake workspace)
stay too: they are inputs of tests that stay, and with the rule gone `p4_h3` has no primary, which no remaining test looks at. What
`test_adr_test_index_h3.py` still asserts (a table row with a limit gives no primary; H0 to H2 unchanged; the `h3` key is 0) is true
without the constant and stays as a behavioural record of the removal. The H1 and H2 assertions of the removed `test_p2_p3_p4_tiers`
are also made by `test_a3_h1_and_h2_unchanged` of that file.

## Checks (limits fixed now)

The "before" index is made from the commit that holds this ADR alone (the registration commit), with `--metadata
f790-scratch/meta.json`, for the three values of `--primary`. The "after" index is made in the same way from the removal commit,
before the addendum is written. The ADR texts are the same in both (this ADR is in both), so a difference cannot come from the
documents. The runs are made once.

| id | check | limit | estimate behind it |
|---|---|---|---|
| C1 | `RETRO_H3` and the tier-3 branch gone | `grep -rn RETRO_H3 scripts` finds 0 lines; no `(3, ` tuple is built in `retro_units` | the design above |
| C2 | `--primary git` index | the JSON file after is byte-identical to the one before (`cmp`), so every field is equal, including `coverage.primary.retro.h3` = 0 | the loop iterated over an empty tuple |
| C3 | `--primary registered` and `--primary p0679` | byte-identical, both | rule H runs only with `git` |
| C4 | `gate_attribute.py` | `git diff` of it between the registration commit and the removal commit is empty | not edited |
| C5 | `python -m unittest discover -s scripts` from the worktree | OK, 0 failures, 0 errors; ran tests = 139 - 3 = 136; skipped = 4 | 3 tests removed, none added |
| C6 | one commit | the removal is one commit that changes exactly 3 files: `adr_test_index.py`, `test_adr_test_index_retro.py`, `test_adr_test_index_h3.py`; the ADR is not in it | the registration is the first commit, the addendum the last |
| C7 | the `--report` output (git) | 0 differing lines before and after | the report prints counts only |
| C8 | docs | `cargo test -q --release -p rcfd-cli --test docs_site` passes (run after the ADR is written) | the ADR is a new file under docs |
| C9 | no other file | `git diff --stat` of the removal commit shows no file outside `scripts/` | design |

Decision on the `h3` key (standing instruction): it stays, with value 0. Removing it would change the index in one field and the
acceptance says equal in every field. A later item may drop the key together with a documented change of the index.

## Negative controls and planned mutations

Made on a scratch copy of `scripts/` after the commit, one at a time, with `python -m unittest` on the two test files; each must be
caught by at least one test:

* M1 re-introduce a tier-3 emission in `retro_units` (an append of a tier-3 unit for a row with a limit): caught by the row-with-a-limit test of `test_adr_test_index_h3.py`;
* M2 remove the `h3` key from `stat`: caught by the `h3` key test of the same file;
* M3 drop the H1 branch of table rows: caught by `test_a6_a7_table_rows_of_h1_and_h2_still_count`;
* M4 drop the H2 phrase in table rows: caught by the same test.

If a mutation is not caught, that is reported, and a test is added by an amendment written before the corrected run.

## Decision rule

Done if C1 to C9 are met and all four mutations are caught. Partial if a check was not run or one limit is missed (a miss is
recorded with the number, the limit is not changed). Todo if the index differs and the cause is not diagnosed.

## Limits (what is not covered)

* The equality is checked on the repository at b3af848a plus this ADR, not on any other state of the tree.
* The docstring and the report line still mention rule H3 historically; they are text, not behaviour.
* The test fixtures that name H3 remain; they are inert.
* The `h3` key stays in the index: the dead key is not removed (see above).
* Only `scripts/` tests were run; no Rust crate is touched, so no cargo test of a solver crate is run (only `docs_site`).

## Addendum 1: results (2026-11-17)

Commits on branch `f790-work`: b0d49dd3 (this ADR alone, the registration), 5770b343 (the removal, one commit, 3 files), and the
commit of this addendum. The "before" indexes were made at b0d49dd3, the "after" indexes at 5770b343 (working tree equal to the
commit), all with `f790-scratch/meta.json`, once each. Files in `f790-scratch`: `before_*.json`, `after_*.json`, `report_*.txt`.

| id | check | limit | measured | met |
|---|---|---|---|---|
| C1 | `RETRO_H3` and tier 3 gone | 0 lines of `RETRO_H3` in `scripts/*.py`; no tier-3 tuple | 0 lines (grep over `*.py`); the loop, `cells` and `RETRO_LIMIT` (no other use) deleted | yes |
| C2 | `--primary git` index | `cmp` identical, `h3` = 0 | identical (11 791 224 bytes before and after) | yes |
| C3 | `--primary registered`, `p0679` | `cmp` identical | identical (11 811 933 and 11 815 035 bytes) | yes |
| C4 | `gate_attribute.py` | diff empty | empty | yes |
| C5 | `python -m unittest discover -s scripts` | OK; 136 tests; 4 skipped | `Ran 136 tests`, `OK (skipped=4)` | yes |
| C6 | one commit, 3 files | exactly 3 files, ADR not in it | `adr_test_index.py`, `test_adr_test_index_retro.py`, `test_adr_test_index_h3.py` (3 insertions, 26 deletions) | yes |
| C7 | `--report` output | 0 differing lines | identical for the three values of `--primary` | yes |
| C8 | `docs_site` | passes | `cargo test -q --release -p rcfd-cli --test docs_site`: 2 passed, 0 failed | yes |
| C9 | no file outside `scripts/` in the removal commit | none | none | yes |

Mutations (on a scratch copy `f790-scratch/mut`, one at a time, the two test files, 19 tests): M1 a tier-3 emission caught (2 failures:
the row-with-a-limit test and the `h3` key test); M2 the `h3` key removed caught (1 error, the `h3` key test); M3 H1 dropped in table
rows caught (2 failures, `test_a6_a7_*` and the `h3` key test, which counts h1); M4 H2 dropped in table rows caught (same 2). 4 of 4
caught. fmt and clippy do not apply (no Rust file touched).

Deviations: none from the registration. Not run: any solver crate test (no Rust file is touched). `docs_site` was run once after the
addendum was written (table). The removed tests of ADR 0701 (`test_p2_p3_p4_tiers`, `test_counts`) lose the assertion on the exact file list
`r_h1.rs -> [1, "0110"]`, which no remaining test makes (the H1 and H2 primaries are still asserted by the tests of
`test_adr_test_index_h3.py`).

**Decision by the registered rule: done.**

### Remaining questions

1. The dead key `h3` (always 0) in `coverage.primary.retro`, the "H3 removed" text of the report line and the inert H3 fixtures of
   the retro tests. Options: (a) keep as now; (b) a later item removes the key together with a documented, deliberate change of the
   index and the fixtures. Recommendation: (a); the key costs nothing, and a change of the index shape needs the consumers
   (`gate_attribute.py`) read first.
2. The lost assertion on the exact `files` list of the retro counts (above). Options: (a) accept; (b) a new test of that list for H1
   and H2 only. Recommendation: (b), small, in the next change to these scripts.

## For the lead: roadmap entries

Proposed status and note of F-790:

> Proposed status: done.

> Test-to-ADR index, gate attribution. Follow-up of F-785 (ADR 0703) and F-786 (ADR 0704), 2026-11-17. DONE. ADR 0712, 2026-11-17: the dormant rule H3 code of `scripts/adr_test_index.py` is deleted in one commit: the constant `RETRO_H3`, the tier-3 loop of `retro_units` and `RETRO_LIMIT`, with the 3 tests that depended on them (`test_p2_p3_p4_tiers` and `test_counts` of the tests of ADR 0701, `test_constant_is_off` of the tests of ADR 0703) and the setUp/tearDown lines of the constant; 3 files, 3 insertions and 26 deletions. The index of `--primary git`, `--primary registered` and `p0679` is byte-identical before and after (11 791 224, 11 811 933 and 11 815 035 bytes, `cmp`; the `--report` output too; the key `h3` stays, always 0), `gate_attribute.py` is unchanged, `python -m unittest discover -s scripts` runs 136 tests (139 before), OK; 4 of 4 mutations caught. The exact file list of the retro counts is no longer asserted by a test.

> Follow-up proposed: A test of the per-file tier list of the retro counts for rules H1 and H2. Deps F-790. Acceptance: Every limit not stated here is registered in the ADR before the run. A test of `scripts/test_adr_test_index_retro.py` asserts the `files` map of `coverage.primary.retro` (tier and ADR of each file) on its fake workspace for H1 and H2 files; the index of `--primary git`, `--primary registered` and `p0679` is equal to the index before the commit in every field; `gate_attribute.py` is unchanged; a planted change of a tier in `retro_primaries` is caught; `python -m unittest discover -s scripts` passes.
