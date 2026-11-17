# ADR 0705: F-769 registration: ADRs for the five feature groups that no ADR describes, and the index of them

Status: **REGISTERED 2026-11-17, written before the five group ADRs (0706 to 0710) and before any run of its checks.** Not yet accepted
by the maintainer. Roadmap item F-769 (deps F-760, [ADR 0685](0685-f760-registration-adr-text-for-the-25-unmapped-integration-test-binaries.md)
addendum 1, open question 1, option (b)). Number 0705 given by the lead for this registration and the index; 0706 to 0710 for the five
groups. Branch `f769-work` from 410900e2, worktree `w49i-work/f769`, scratch `w49i-work/f769-scratch`. No Rust file, golden, hash table or
script is edited. Results go in the addenda at the end; nothing above them is changed after a run.

These ADRs are **retrospective**. Every test file they describe was committed on 2026-10-13 or 2026-10-14 (or the F-105 commit of
2026-10-13), before this ADR and before most ADRs existed, and none had an ADR of its own. Each group ADR records what its files check and
which limits they carry, read from the code and from the roadmap notes of the items that delivered them. It is **not** a decision taken
before the tests; where a limit came from a measurement the roadmap note records, the ADR says so, and the ADR does not claim a derivation
the files do not contain. The choices in this item (one ADR per group, what counts as a limit) are taken on the maintainer's standing
instruction of 2026-11-11 (take the recommendation).

## What F-769 asks

> Every limit not stated here is registered in the ADR before the run. A short registration ADR per group states what the files
> `rcfd-assistant/tests/physics_workflow.rs`, `rcfd-assistant/tests/workflow_engine.rs`, `rcfd-turb/tests/channel_wall_function.rs`,
> `rcfd-cli/tests/physics_cases.rs`, `rcfd-api/tests/mesh_params.rs` and `rcfd-core/tests/table_expr.rs` check and which limits they carry,
> read from the code and the roadmap notes, with no Rust file edited; the index maps these files by a reading of that ADR (evidence E2,
> not only a word); a registered sample is read.

## What was read (before any edit)

* ADR 0685 (the plan, the five binaries it leaves to this item, its sample and its open question 1), ADR 0701 (rules H0 to H3 of
  `scripts/adr_test_index.py`, `--primary git`), ADR 0704 (second reader; it recommends that the notes of ADR 0067 and 0123 on the two
  `workflow.rs` files move into the ADRs this item writes).
* The six files of the acceptance in full, and the two `workflow.rs` files (`rcfd-assistant/tests/workflow.rs`, F-148, 9 tests;
  `rcfd-api/tests/workflow.rs`, F-120 and F-121, 5 tests) for their test names, headers and the asserts that carry a limit.
* The notes in `docs/roadmap.json` of F-040, F-042, F-043, F-060, F-090, F-105, F-120, F-121, F-147 and F-148; `docs/case-schema.md` at
  the passage on `nx`/`ny` and `lx`/`ly` of `cavity_2d` (wave 11); `git log --diff-filter=A` for the six files (adding commits `f71f6e2c`
  2026-10-13 for four, `40473ad5` 2026-10-13 for `table_expr`, `0704ccaf` 2026-10-14 for `mesh_params`).
* One run of the unchanged index (`python scripts/adr_test_index.py --primary git`, 46.6 s) saved as `f769-scratch/before.json`. It shows,
  for the files of this item: `table_expr.rs` (2 tests), `channel_wall_function.rs` (2), `physics_cases.rs` (7), `mesh_params.rs` (4) and
  `physics_workflow.rs` (5) have primary ADR **0685** with evidence E2, by rule H2 (ADR 0685 says no ADR describes them);
  `workflow_engine.rs` (1) and `rcfd-api/tests/workflow.rs` (5) have primary 0123 and `rcfd-assistant/tests/workflow.rs` (9) has 0067, all by
  rule H1 (the "Test files" notes of ADR 0685). **The estimates below were made after seeing this**, and the rule of the script (the lowest
  number wins inside a tier) was read before them.
* No test of the eight files was run for this item. The limits are read from the code, so the ADRs register what a passing run must show;
  they do not report a measurement of it (see Limits).

## Design

**Five group ADRs, one per feature group of ADR 0685's open question, each short.** Each has: a status line (registered 2026-11-17,
retrospective), what the files check, a table of limit rows with a stable id (`P1`, `W2`, ...), the test function and the source of the
limit (the assert in the file, or the roadmap note that records where the number came from), what the files do not check, and the
citation of the files as `crates/<crate>/tests/<file>.rs` (the form the index matches for evidence E2). A limit row has one of three
kinds: **numeric** (a tolerance or threshold in an assert), **exact** (an equality on a count, a name, a state, an exit code, a list) or
**structural** (a field or message must be present). Every `#[test]` function of the files is named in a row.

| ADR | group | files (crate-relative) | tests |
|---|---|---|---|
| 0706 | assistant setup helpers and workflow engine | `rcfd-assistant/tests/physics_workflow.rs` (5), `rcfd-assistant/tests/workflow_engine.rs` (1); and, as ADR 0704 recommends, `rcfd-assistant/tests/workflow.rs` (9, F-148) and `rcfd-api/tests/workflow.rs` (5, F-120, F-121) | 20 |
| 0707 | the F-040 wall-function channel | `rcfd-turb/tests/channel_wall_function.rs` | 2 |
| 0708 | the shipped turbulence and compressible examples through the binary | `rcfd-cli/tests/physics_cases.rs` | 7 |
| 0709 | the mesh generator parameters | `rcfd-api/tests/mesh_params.rs` | 4 |
| 0710 | tables called from expressions | `rcfd-core/tests/table_expr.rs` | 2 |

The two `workflow.rs` files are included in 0706 because their checks are the same feature (the assistant tools over the workflow engine,
the engine and the headless methods under it): `rcfd-assistant/tests/workflow.rs` checks the setup helpers that `physics_workflow.rs`
extends, and `rcfd-api/tests/workflow.rs` checks the engine that `workflow_engine.rs` drives through the assistant. The existing notes of
ADR 0067 and ADR 0123 are not edited (the text above their "Test files" sections stays, and the sections are ADR 0685's); 0706 says what
they do not describe.

**Limits fixed here, before the group ADRs are written** (the group ADRs register what each file carries; these are the checks of this
item on them):

| id | limit | estimate behind it |
|---|---|---|
| B1 | each of the 8 files is named by path (`crates/<crate>/tests/<file>.rs`) in its group ADR in the table above, with a table of limit rows | 8 files, 5 ADRs |
| B2 | each of the 35 `#[test]` functions of the 8 files (5 + 1 + 9 + 5 in 0706 as 20, 2, 7, 4, 2) is named in a row of its group ADR: **35 of 35** | counted with `grep -c '#\[test\]'`: 5, 1, 9, 5, 2, 7, 4, 2 = 35 |
| B3 | every numeric literal that stands in an `assert*` line of the six files of the acceptance and has a decimal point, an exponent, or at least two digits (a tolerance or threshold, not a 0 or 1) appears in the group ADR: **at least 95 %** of the distinct (file, literal) pairs, by a script run once after the ADRs are written | the ADRs are written from the code; the 5 % allowance is for literals that belong to a test input and not to a limit, which the script cannot tell apart |
| B4 | each group ADR says in its first section that the tests predate it and that it records what they check, not a decision taken before them (the word `predate`): **5 of 5** | the text of this ADR |
| B5 | index after the edit, `--primary git`: every test of the 8 files has its group ADR in `adrs` **with an E2 evidence entry** (not only E7): **35 of 35** tests; tests that lose an ADR: **0**; tests outside the 8 files whose `adrs` change: **at most 10** (a stem shared with another crate, or an ADR text that names a word of the index) | the group ADRs cite the files by `crates/.../tests/<file>.rs`, the form E2 matches; `workflow` is a stem of two crates, so the path form is needed |
| B6 | the `primary_adr` of the tests of the 8 files is **reported** next to B5 (a number, not a limit) | the estimate is that it does not change: 0685 (5 files), 0123 (2), 0067 (1) win inside their tiers by the lowest number, and a group ADR has a higher number. If it does not change, that is a finding about rule H of `adr_test_index.py` (F-785 changes H3 on another branch), not a miss of this item |
| B7 | a registered sample of the limit rows is read (below): **at least 9 of 10 agree** with the code and the note they cite | 5 ADRs x 2 rows |
| B8 | `git diff --stat 410900e2` shows files only in `docs/adr/` | the acceptance: no Rust file edited |
| C1 | `python -m unittest discover -s scripts` from the worktree: all pass | 93 run, 4 skipped at ADR 0685 |
| C2 | `cargo test -q --release -p rcfd-cli --test docs_site` passes (files under `docs/` changed), `CARGO_TARGET_DIR=w49i-work/f769-scratch/target` | passed at ADR 0685 |

**Registered sample (B7), before it is read.** A row is a numbered limit row of a group ADR (`ids` as written in the table of that ADR).
After the five ADRs are committed, the rows of each ADR are listed in the order they appear in its tables, and
`random.Random(769).sample(rows_of_that_ADR, 2)` is drawn for each of 0706, 0707, 0708, 0709, 0710 (the pool of 0706 includes the rows of
the two `workflow.rs` files; an ADR with fewer than two rows gives all of them). The 10 rows are read **against the file**: the test
function named in the row is opened, the assert or the roadmap-note sentence the row cites is found and compared with the row's number,
kind and source; the row **agrees** if the number (or the exact value or the structural field) equals the one in the code, the test
function is the one named, and the stated source says what the row says it says. It **disagrees** if any of these fails, and the
disagreement is reported with the line. I read my own text (there is no second reader), so a second reading such as ADR 0704's is left
open (Limits). The sample is read once, no row is replaced.

Decision rule: **done** if B1 to B5, B7, B8, C1 and C2 are met (B6 is reported); **partial** if B3 or B5 or B7 is missed, or a clause of
the acceptance is not met (the clause "the index maps these files by a reading of that ADR (evidence E2, not only a word)" is read as B5;
if B6 shows that the group ADR never becomes the primary, the item still says so and proposes the follow-up below).

## Negative controls and planned mutations

There is no code. N1: the index is run on the same script and the same inputs before (`before.json`) and after; the only inputs that
differ are the six new ADR files. N2: B3 is run once on the ADR text with one group ADR's literal list deleted in a scratch copy, to show
that the script reports misses (a control of the check, not of the tests). N3: the 10 sample rows are chosen by a fixed seed and the draw
code is saved in the scratch folder, so the draw can be repeated.

## Limits (what is not covered)

* The eight files are **not run** by this item: the ADRs register what a passing run shows. Whether each limit holds today is the daily
  gate's information (ADR 0653), not this item's. No limit is "met" or "missed" here in the sense of F-xxx registrations; B-rows measure the
  text and the index.
* Limits that came from a roadmap note (the 2.1 %, 2.3 %, 2.4 % deviations of F-040, 18.17 against 17.4 of F-043) are recorded with that
  source; the ADR does not re-derive them.
* The sample is 10 of the rows, read by the author of the rows. It is not a second reading.
* The primary ADR of a test is decided by the script on this base. It may still be 0685, 0123 or 0067 after this item (B6).
* The notes in ADR 0067 and ADR 0123 on the two `workflow.rs` files are not moved or edited.

## Addendum 1: results (2026-11-17)

The five group ADRs (0706 to 0710) were written and committed (`d07421bf`) after this registration (`1dabf4b3`). B2 and B3 were measured with
`f769-scratch/b23.py` (written after the ADRs, before its single run on them; it matches a literal as written in the ADR, accepting `X.0`
for `X`), the index was run once after the edit (`after.json`, 46 s) against `before.json`, the sample was drawn and read once. No Rust
file, golden, hash table or script is edited; the eight files were not run.

| id | limit | measured | met |
|---|---|---|---|
| B1 | 8 files named by path with a limit table | 8 of 8 (0706: 4 files, 0707 to 0710: 1 each) | yes |
| B2 | 35 of 35 `#[test]` names in the group ADR | **35 of 35** | yes |
| B3 | at least 95 % of the (file, literal) pairs of the assert lines of the six files | **24 of 25 (96.0 %)**; the miss is `0.10` of `physics_cases.rs` (`assert!((col("u_mean") / 17.4 - 1.0).abs() < 0.10`), which ADR 0708 writes as "10 %". Control N2 (the 0708 text removed): 21 of 25 (84 %), and the script lists `0.01` and `0.10`, so it does report misses | yes |
| B4 | the word `predate` in each of the 5 ADRs | 5 of 5 (first section of each) | yes |
| B5 | every test of the 8 files has its group ADR in `adrs` with an E2 entry: 35 of 35; tests losing an ADR 0; tests outside the 8 files whose `adrs` change at most 10 | 35 of 35 with E2; 0 lose an ADR; **32 outside tests gain an ADR, limit 10: missed** (below) | **no** |
| B6 | primary ADR of the 35 tests, reported | **unchanged for all 35**: `table_expr`, `channel_wall_function`, `physics_cases`, `mesh_params`, `physics_workflow` (20 tests) stay at 0685, `workflow_engine` and `rcfd-api/tests/workflow.rs` (6) at 0123, `rcfd-assistant/tests/workflow.rs` (9) at 0067; the number of tests with a primary is 7451 before and after | reported (as estimated) |
| B7 | sample of 10 rows, at least 9 agree | **10 of 10 agree**, one with a reservation (below) | yes |
| B8 | only files in `docs/adr/` | 6 files, all in `docs/adr/` (389 lines added before this addendum), 0 removed | yes |
| C1 | `python -m unittest discover -s scripts` | 132 run, OK, 4 skipped | yes |
| C2 | `cargo test -q --release -p rcfd-cli --test docs_site` | 2 passed, 0 failed (`CARGO_TARGET_DIR=w49i-work/f769-scratch/target`) | yes |

**B5, the miss.** The 32 tests outside the 8 files that gained an ADR are 19 tests of `crates/rcfd-api/src/sim.rs` (ADR 0709, evidence
E5: the ADR text cites the path `rcfd_api::sim::build_mesh`, and the index maps every unit test of a cited module), 8 of
`crates/rcfd-core/src/table.rs` (ADR 0710, E5, `rcfd_core::table::Table`) and 5 of `crates/rcfd-api/tests/tables.rs` (ADR 0710, E2: the
"does not check" section names that file by path to say what it holds). The limit of 10 had an allowance for a shared stem and a word of the
index, and did not foresee the module rule. The limit is not changed. Diagnosis: the 8 `table.rs` tests (unit tests of the tables the file
uses) and the `tables.rs` tests (the settings-tree route of the same F-105 feature) are related to ADR 0710 in a way a reader would accept;
the 19 `sim.rs` tests are not about the generator parameters (they run cases) and the gain for them is an artifact of citing
`build_mesh`. I did not remove the citations to get under the limit.

**Primary (B6).** Rule H of the script gives the tier-2 note of ADR 0685 (H2) and the tier-1 "Test files" notes of 0067 and 0123 priority
over a table row of a higher-numbered ADR (H3, or H2 from the same tier with a higher number). So the index reads each of the 8 files
as described by an ADR (E2, `adrs` has the group ADR) but still names 0685, 0123 or 0067 as the primary. A group ADR would become the
primary only by a tier-1 section of the form `## Test files (added <date>, ADR dddd)` (a heading made for the notes that ADR 0685
appended, and still behind the lower number of 0067/0123), or by a change of the script (F-785 changes H3 on another branch). I did not
write a section of that form to win the rule.

**Sample (B7).** Drawn after the five ADRs were committed, as registered: `random.Random(769).sample(rows, 2)` for each of 0706 (24
rows), 0707 (8), 0708 (7), 0709 (4), 0710 (5), giving H1, H6; W1, W2; C1, C2; M1, M4; T1, T2. Each row was read against the file:

| row | file and test | reading |
|---|---|---|
| H1 | `physics_workflow.rs`, `laminar_settings_leave_the_suggestions_unchanged` | agree: `assert_eq!` of the two calls on `{"turbulence": {"model": "laminar"}, "solver": {"algorithm": "simplec"}}` |
| H6 | `workflow_engine.rs` | agree: the `asked` list is the nine names of the row, in that order; the three read-only calls (`list_workflow_tasks`, `check_case_headless`, `read_status_stream`) are scripted and not in it |
| W1 | `channel_wall_function.rs`, `wall_distance_is_exact_on_the_channel_mesh` | agree with a reservation: 1e-13 on the uniform mesh, on the 3 x 20 graded mesh (`Symmetric(5.0)`) and for `xmin` equal to x, and `nope` is an error, as the row says; but the row says "12-cell uniform channel mesh", and the mesh is 4 x 12 cells (12 across the channel); the row is imprecise in the count, the limit is the stated one |
| W2 | same, `channel_recovers_the_log_law_and_the_wall_shear` | agree: `ch.res < 1e-10` and the loop `for it in 0..8000` with the break at `res < 1e-10` |
| C1 | `physics_cases.rs`, `every_physics_example_validates` | agree: the five paths of the row, `status.code() == Some(0)` |
| C2 | same, `sod_runs_and_writes_compressible_fields` | agree: exit 0, the three arrays `"rho"`, `"mach"`, `"T"`, `monitors.csv` `is_file()` |
| M1 | `mesh_params.rs`, `cavity_2d_honours_ny` | agree: `nx = 20`, `ny = 2`, 40 cells |
| M4 | same, `unsupported_mesh_options_are_rejected_not_ignored` | agree: seven `rejects` calls with the seven paths of the row (two `mesh.wall_layers`, `mesh.periodic_x`, `mesh.periodic_y`, three `mesh.grading`) |
| T1 | `table_expr.rs`, `table_calls_are_dimension_checked_and_interpolate` | agree: `Dimension::KINEMATIC_VISCOSITY * Dimension::DENSITY` for `nu_t(Tw) * nu_t_rho(Tw)` |
| T2 | same | agree: `(c.eval - want).abs() < 1e-15 * want.abs().max(1.0)`, with `want` about 7.10e-4 so the bound is 1e-15 absolute (computed by hand: 0.715e-6 times 993.2) |

Not read: the other 22 rows of 0706, 6 of 0707, 5 of 0708, 2 of 0709, 3 of 0710 (38 in all). The author of the rows read them; there was no second reader.

Deviations: the miss of B5; the script of B2/B3 accepts `X.0` for `X`, written after the ADRs and before its run, which is a choice not
registered in the text of B3 (the `.0` forms in ADRs 0707 to 0710 are written as in the code, so the figure does not depend on it here, but
that was not checked by a second run); the imprecision of W1 above. Not run: the eight test files, the other `--primary` rules of the index
(`registered`, `p0679`).

Decision by the registered rule: **partial** (B5 missed, with a diagnosed cause; B1 to B4, B7, B8, C1, C2 met; B6 reported).

Remaining questions:

1. The group ADRs are not the primary of the files (B6). Options: (a) leave: E2 evidence stays, and the lowest-numbered ADR stays the
   attribution of the daily gate; (b) a rule in the script that a retro ADR that carries a limits table for the file outranks an older
   list note (F-785 touches H3 on another branch; coordinate); (c) move the notes of 0067, 0123 and 0685 into these ADRs (not allowed by
   ADR 0685's "the text above it is not edited"). Recommendation: (b), after F-785 is merged.
2. The module citations raise the E5 mapping of unrelated unit tests (19 of `sim.rs`). Options: (a) accept; (b) rewrite the citation in 0709
   without the module path (an edit of an ADR after the run, which needs an amendment under the house rule); (c) leave to the script
   (E5 of a cited module path maps all unit tests of that module). Recommendation: (a) now, (c) at the next change of the index.
3. The limit rows have one reader. Options: a second reading of a larger sample (ADR 0704 style), or none. Recommendation: a second reader
   of at least 15 rows, in particular rows of 0706 not read here, because H11 to H24 were written from tests whose thresholds are rule of
   thumb.

## For the lead: roadmap entries

Proposed status and note of F-769:

> Proposed status: partial.

> ADRs for the five feature groups that no ADR describes, follow-up of F-760 (ADR 0685, 2026-11-16). PARTIAL. ADR 0705, 2026-11-17 (registration and index), ADRs 0706 to 0710 (one per group): each says plainly that the tests predate it and records what they check, read from the code and the roadmap notes of F-040, F-043, F-105, F-120, F-121, F-148; no Rust file, script, golden or hash table edited, the eight files not run. 0706 the assistant setup helpers and workflow engine (`physics_workflow.rs`, `workflow_engine.rs`, and as ADR 0704 recommends `rcfd-assistant/tests/workflow.rs` and `rcfd-api/tests/workflow.rs`; 24 limit rows), 0707 the F-040 wall-function channel (`channel_wall_function.rs`; 1e-13 wall distance, y+ 5e-3, wall shear 2e-3, interior 0.03 against 2.1 to 2.4 percent recorded), 0708 the shipped turbulence and compressible examples through the binary (`physics_cases.rs`; exit codes, fields, y+ within 0.01 of 1, bulk velocity within 10 percent of 17.4 against 18.17 recorded), 0709 the mesh generator parameters (`mesh_params.rs`), 0710 tables called from expressions (`table_expr.rs`; 1e-15 absolute, NaN outside the range, the test named "with a span" does not check the span). Measured: all 35 tests named (35 of 35), 24 of 25 tolerance literals of the six files carried (96 percent, limit 95), all 35 tests have their group ADR with E2 evidence and none lost an ADR, sample of 10 rows read 10 of 10 agree (one imprecise count), unit tests of `scripts` and docs_site pass. MISSED: at most 10 tests outside the eight files gain an ADR, measured 32 (19 `sim.rs` and 8 `table.rs` tests through a cited module path, 5 `tables.rs` tests through a path named in a "does not check" section). The primary ADR of the 35 tests is unchanged (0685, 0123, 0067: the lowest number wins inside a rule H tier), a finding for the script, not for the ADRs. Not covered: a second reader, running the files, moving the notes of 0067 and 0123.

> Follow-up proposed: Primary ADR of the files that a retro-registration ADR describes (group ADRs 0706 to 0710 outranking the list notes of ADRs 0067, 0123 and 0685). Deps F-769, F-785. Acceptance: Every limit not stated here is registered in the ADR before the run. A rule in `scripts/adr_test_index.py`, registered with its mutations, under which the ADR that carries a limits table for a file and says that its tests predate it is the primary of the 35 tests of the eight files of ADR 0705, and no primary of any other test changes by more than a registered count; the unit tests of `scripts` pass.

> Follow-up proposed: A second reader for the limit rows of ADRs 0706 to 0710. Deps F-769. Acceptance: Every limit not stated here is registered in the ADR before the run. A reader other than the author of the rows reads a registered sample of at least 15 rows of ADRs 0706 to 0710, including 10 of the 22 rows of 0706 not read in ADR 0705, against the files and the roadmap notes they cite, with at least 90 percent agreement, and the disagreements are listed with the line.
