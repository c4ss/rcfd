# ADR 0714: F-792 registration: a test of the per-file tier list of the retro counts for rules H1 and H2

Status: **REGISTERED 2026-11-17, written before any code of F-792 and before any run of its checks.** Written by the implementation
agent on branch `f792-work` (from 68f66132, which holds F-772 / ADR 0701, F-785 / ADR 0703 and F-790 / ADR 0712), worktree
`w49i-work/f792`, scratch `w49i-work/f792-scratch`. Not yet accepted by the maintainer. Every choice below that the maintainer would
normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation) and is marked
"(standing instruction)".

Roadmap item F-792 (deps F-790), the follow-up proposed by ADR 0712 (its remaining question 2, option b). The dep is met: ADR 0712
deleted the dormant rule H3 code and, with `test_counts`, the only assertion on the `files` map. No solver and no script other than
one test file changes. House rules (`w49i-work/AGENT_RULES.md`) apply: limits fixed here and not changed after the run, each run made
once, no golden or hash table edit, `gate_attribute.py` and `adr_test_index.py` not edited, ADRs 0679, 0684, 0690, 0701, 0703 and
0712 not edited.

## What F-792 asks

> Every limit not stated here is registered in the ADR before the run. A test of `scripts/test_adr_test_index_retro.py` asserts the `files` map of `coverage.primary.retro` (tier and ADR of each file) on its fake workspace for H1 and H2 files; the index of `--primary git`, `--primary registered` and `p0679` is equal to the index before the commit in every field; `gate_attribute.py` is unchanged; a planted change of a tier in `retro_primaries` is caught; `python -m unittest discover -s scripts` passes.

## What was read beforehand (disclosed)

`AGENT_RULES.md`; the brief; ADR 0712 (the addendum and remaining question 2) and the form of ADRs 0572 and 0573;
`scripts/test_adr_test_index_retro.py` in full (the fixture `FILES`, `Retro`, `RetroOnGit`); `scripts/adr_test_index.py`
`retro_units` (lines 277 to 318), `retro_primaries` (lines 652 to 676) and the call in `build` (line 613); the removed
`test_counts` as it stood in 0fee77d1 (`(h1, h2, h3) == (2, 3, 1)` and `files[r_h1.rs] == [1, "0110"]`).

Measured beforehand (a baseline, not a result of F-792): `python -m unittest discover -s scripts` on 68f66132 runs 136 tests, OK
(4 skipped). `cargo metadata --no-deps --format-version 1` was saved to `f792-scratch/meta.json` so that every index run uses the same
metadata.

## Design

`retro_primaries` returns `stat` with `stat["files"][t["file"]] = [tier, adr]` for each file whose E2-only tests got a primary by
rule H, and `h1`, `h2`, `h3`, `none` counted per test. The new test reads `idx["coverage"]["primary"]["retro"]` of the existing
class `Retro` (its fake workspace, `build(root, meta, "git")`), and compares the whole `files` map with an expected dict written
in the test, not only one entry, so that an extra or a missing file is caught.

The expected map is derived by hand from the fixture `FILES` and `retro_units` (tier 1: a unit inside a `Test files (added ...,
ADR dddd)` section; tier 2: a paragraph or table row with `without an ADR`; the best is the minimum `(tier, ADR number)` among the
candidates, and the candidate ADR must be in the test's `adrs`). The fake workspace is not a git repository, so rule G gives no
primary and every E2-only test goes to rule H.

| file (crates/rcfd-w/tests/) | candidates (tier, ADR) | expected entry |
|---|---|---|
| `r_h1.rs` | (1, 0110) from the section of 0110 | `[1, "0110"]` |
| `r_tier_a.rs` | (1, 0130) section note; (2, 0120) phrase | `[1, "0130"]` |
| `r_h2.rs` | (2, 0115) | `[2, "0115"]` |
| `r_tier_b.rs` | (2, 0120) | `[2, "0120"]` |
| `r_low.rs` | (2, 0125), (2, 0140) | `[2, "0125"]` |

Files that must not be in the map: `r_h3.rs` (a table row with a limit and no phrase: no tier since ADR 0712), `r_n1.rs` to
`r_n4.rs`, `r_n7.rs` (negative controls), `r_e1_f020_x.rs` and `r_hdr.rs` (primary from E1 and E0, not E2-only, so rule H does not
see them even though ADR 0170 has the phrase for both). The counts: `h1` = 2, `h2` = 3, `h3` = 0 (the key stays, ADR 0712).

Two tests are added to class `Retro`:

* `test_files_map_h1_h2`: `r["files"]` is equal, as a dict, to the five entries above; every value is a two-element list whose
  first item is 1 or 2.
* `test_retro_counts_agree_with_files`: `(h1, h2, h3) == (2, 3, 0)`, and the number of files with tier 1 (resp. 2) in the map
  equals `h1` (resp. `h2`) (one test per file here, so the count per test equals the count per file in this fixture).

Nothing else is changed: no fixture is added or edited (the existing fixture already has two H1 files and three H2 files, which is the
case wanted), `adr_test_index.py` is not touched. The test commit changes exactly one file.

## Checks (limits fixed now)

The "before" indexes are made from the commit that holds this ADR alone (the registration commit), with `--metadata
f792-scratch/meta.json`, for the three values of `--primary`; the "after" indexes from the test commit, before the addendum is
written. The ADR texts are the same in both. The runs are made once.

| id | check | limit | estimate behind it |
|---|---|---|---|
| C1 | `files` map of the fake workspace | equal to the 5-entry dict above, exactly (0 extra, 0 missing, 0 differing tier or ADR) | derivation above |
| C2 | counts | `(h1, h2, h3) == (2, 3, 0)`; tier counts of the map equal `h1`, `h2` | derivation above |
| C3 | `--primary git` index | the JSON file after is byte-identical to the one before (`cmp`), so every field is equal | only a test file is added |
| C4 | `--primary registered` and `--primary p0679` | byte-identical, both | same |
| C5 | `gate_attribute.py` and `adr_test_index.py` | `git diff` between the registration commit and the test commit is empty for both | not edited |
| C6 | `python -m unittest discover -s scripts` from the worktree | OK, 0 failures, 0 errors; ran tests = 136 + 2 = 138; skipped = 4 | 2 tests added |
| C7 | one commit, one file | the test commit changes exactly `scripts/test_adr_test_index_retro.py`, insertions only | design |
| C8 | `--report` output | 0 differing lines before and after, for the three values of `--primary` | the report prints counts only |
| C9 | docs | `cargo test -q --release -p rcfd-cli --test docs_site` passes (run after this ADR is written) | the ADR is a new file under docs |
| C10 | planted mutations | each of M1 to M5 below is caught by at least one of the two new tests (run alone) | below |

Decision (standing instruction): the expected values are written in the test as literals, not computed from the fixture, so the
test cannot agree with a wrong implementation by construction.

## Negative controls and planned mutations

Made on a scratch copy of `scripts/` (`f792-scratch/mut`), one at a time, never in a commit. Each is run against the two new tests
alone and then against the whole retro test file, to say which tests catch it:

* M1 in `retro_primaries`, write the tier of the `files` entry as 2 (`[2, best[1]]`): must be caught by the files-map test (the H1
  files `r_h1.rs` and `r_tier_a.rs`);
* M2 in `retro_primaries`, write the tier as `best[0] + 1`: caught by the files-map test (all five);
* M3 in `retro_primaries`, count every file under `h2` (`stat["h2"] += 1` for all): caught by the counts test; the files-map test
  is unaffected (a record of what each test is for);
* M4 in `retro_units`, emit the section unit as tier 2 instead of 1: caught by the files-map test (the H1 files);
* M5 (the planted change of a tier named by the acceptance) in `retro_primaries`, take `max` instead of `min` over the candidates:
  caught by the files-map test (`r_tier_a.rs` gets `[2, "0120"]`, `r_low.rs` gets `[2, "0140"]`).

If a mutation is not caught by a new test, that is reported, and a test is added by an amendment written before the corrected run.

## Decision rule

Done if C1 to C10 are met. Partial if a check was not run or one limit is missed (a miss is recorded with the number, the limit
is not changed). Todo if the index differs and the cause is not diagnosed.

## Limits (what is not covered)

* The map is asserted on the fake workspace only, not on the real repository (its `files` map has thousands of entries and changes
  with every ADR; a pin of it would be a hash table of the kind the house rules forbid editing).
* Only tiers 1 and 2 exist (ADR 0712). The `none` count and the tests of the real workspace are not asserted.
* Rule G (git) interplay is covered only by the existing `RetroOnGit`, not by the new tests.
* The equality of the indexes is checked on 68f66132 plus this ADR, not on any other state of the tree.

## Addendum 1: results (2026-11-17)

Commits on branch `f792-work`: 539e6f66 (this ADR alone, the registration), 97a2a574 (the test, one file, 20 insertions), and the
commit of this addendum. The "before" indexes were made at 539e6f66, the "after" indexes at 97a2a574 (working tree equal to the
commit), all with `f792-scratch/meta.json`, once each. Files in `f792-scratch`: `before_*.json`, `after_*.json`, `report_*.txt`.

| id | check | limit | measured | met |
|---|---|---|---|---|
| C1 | `files` map | equal to the 5-entry dict | equal (`test_files_map_h1_h2` passes in the one run) | yes |
| C2 | counts | `(2, 3, 0)`; tier counts of the map equal `h1`, `h2` | as limit (`test_retro_counts_agree_with_files` passes) | yes |
| C3 | `--primary git` index | `cmp` identical | identical (11 889 642 bytes before and after) | yes |
| C4 | `--primary registered`, `p0679` | `cmp` identical | identical (11 908 047 and 11 911 149 bytes) | yes |
| C5 | `gate_attribute.py`, `adr_test_index.py` | diff empty | empty | yes |
| C6 | `python -m unittest discover -s scripts` | OK; 138 tests; 4 skipped | `Ran 138 tests`, `OK (skipped=4)` | yes |
| C7 | one commit, one file, insertions only | `test_adr_test_index_retro.py` | 1 file, 20 insertions, 0 deletions | yes |
| C8 | `--report` output | 0 differing lines | identical for the three values of `--primary` | yes |
| C9 | `docs_site` | passes | `cargo test -q --release -p rcfd-cli --test docs_site`: 2 passed, 0 failed (run after the commits, before this addendum) | yes |
| C10 | mutations M1 to M5 | each caught by a new test | 5 of 5 caught, see below | yes |

Mutations (scratch copy `f792-scratch/mut`, never in a commit; new tests alone, then the whole retro file of 15 tests):

| mutation | new tests alone | whole file |
|---|---|---|
| M1 tier of `files` entry always 2 | 2 failures (files map and counts) | the same 2 only |
| M2 tier written as `best[0] + 1` | 2 failures | the same 2 only |
| M3 every file counted under `h2` | 1 failure (counts), files map unaffected as registered | the same 1 only |
| M4 section unit emitted as tier 2 | 2 failures | 3 (also `test_p5_tier_beats_a_lower_number_of_a_lower_tier`) |
| M5 `max` instead of `min` over candidates | 2 failures | 4 (also p5 and `test_p6_lowest_number_in_a_tier`) |

M1, M2 and M3 are caught by no other test of the file: before F-792 a wrong tier or a wrong count in the `files` map or in `h1`/`h2`
passed all tests. Deviation: the registration expected M1, M2 and M5 to be caught by the files-map test; the counts test catches
them too (the tier counts of the map then disagree with `h1`/`h2` or the map changes), which is a stronger result, not a
change of a limit. fmt and clippy do not apply (no Rust file touched).

Not run: any solver crate test (no Rust file is touched). The map of the real repository is not asserted (Limits).

**Decision by the registered rule: done.**

### Remaining questions

1. The tier of a `files` entry is asserted, but `none` is not (it depends on how the fixture's negative controls get an E2
   primary). Options: (a) leave; (b) assert `none` = 6 on the fixture. Recommendation: (a); `none` has no consumer, and the
   negative controls already assert `primary_adr is None` per test.
2. `h3` key and the inert H3 fixtures (question 1 of ADR 0712). Recommendation: unchanged, (a).

## For the lead: roadmap entries

Proposed status and note of F-792:

> Proposed status: done.

> Test-to-ADR index, gate attribution. Follow-up of F-790 (ADR 0712), 2026-11-17. DONE. ADR 0714, 2026-11-17: two tests added to `scripts/test_adr_test_index_retro.py` (one commit, one file, 20 insertions, no change to `adr_test_index.py` or `gate_attribute.py`) that restore the assertion lost with `test_counts` (ADR 0712): the whole `files` map of `coverage.primary.retro` on the fake workspace equals five literal entries of tier and ADR (H1: `r_h1.rs` 0110, `r_tier_a.rs` 0130; H2: `r_h2.rs` 0115, `r_tier_b.rs` 0120, `r_low.rs` 0125), and the counts `(h1, h2, h3) == (2, 3, 0)` agree with the tiers of the map. The index of `--primary git`, `--primary registered` and `p0679` is byte-identical before and after (11 889 642, 11 908 047 and 11 911 149 bytes, `cmp`; the `--report` output too), `python -m unittest discover -s scripts` runs 138 tests (136 before), OK; 5 of 5 planted changes in `retro_primaries` and `retro_units` caught (3 of them, the wrong tier written to the map, always 2 and `best[0] + 1`, and the miscount under `h2`, by the new tests only). Not covered: the `files` map of the real repository, the `none` count.

No follow-up is proposed (remaining question 1 recommends leaving `none` unasserted).
