# ADR 0717: F-795 registration: corrections of rows H22 and W1 and a reading of the 18 unread limit rows of ADRs 0706 to 0710

Status: **REGISTERED 2026-11-17, written before any of the 18 rows of F-795 was read against a file and before any addendum was
written.** Written by the reading agent on branch `f795-work` (from 85313e53, which holds ADRs 0705 to 0710 and 0715), worktree
`w49i-work/f795`, scratch `w49i-work/f795-scratch`. Not yet accepted by the maintainer. Every choice below that the maintainer would
normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation) and is marked
"(standing instruction)".

Roadmap item F-795 (deps F-794, done), the follow-up proposed by ADR 0715. No code is written. The files changed are this ADR and,
by appended addenda only, the group ADRs that hold a row to correct (0706 and 0707 at least).

House rules (`w49i-work/AGENT_RULES.md`) apply: limits fixed here and not changed after the reading, the reading made once, existing
tests not edited, no Rust file, script, golden or hash table edited, clean-room.

## What F-795 asks

> Every limit not stated here is registered in the ADR before the run. An addendum to ADR 0706 corrects row H22 (the test asserts that the first failed task has a message containing `nonexistent`, not which task it is) and an addendum to ADR 0707 corrects row W1 (a uniform 4 x 12 mesh), with the text above each addendum unchanged; the 18 rows not read in ADR 0705 or ADR 0715 (H5, H7, H9, H10, H11, H13, H14, H15, H17, H20, H21, H23, W6, W7, W8, C3, C6, T5) are read by a reader other than the author of the rows against the criterion of ADR 0715, every disagreement is listed with the line and corrected by the same addenda, and at least 16 of the 18 agree; no Rust file is edited and docs_site passes.

## What was read beforehand (disclosed)

* `AGENT_RULES.md` and the brief.
* The entry of F-795 in `docs/roadmap.json` (its acceptance, quoted above, states what is wrong in H22 and in W1; its note is one
  line). No other roadmap entry was opened.
* ADR 0715, lines 1 to 145: the registration, with the criterion. Its two addenda and its "For the lead" section (line 146 on)
  were **not** opened. Two things about its result reached me anyway: the acceptance above (H22 and W1 are wrong), and a commit
  subject of `git log` ("19 of 20 agree"). I do not know its readings, its reasons or its reservations.
* ADRs 0706 to 0710 in full (98, 55, 39, 39 and 46 lines), which is needed to read their rows. Of ADR 0705 only the headings.
* The "For the lead" section of ADR 0572 and the top of ADR 0714, for the form; the docstring of `scripts/adr_test_index.py`.
* `git diff --stat 68f66132 85313e53` over the five test files, the assistant test folder and `cases/`: empty, so the files ADR
  0715 read are the files at 85313e53.
* The test-to-ADR index of the untouched tree was written to the scratch folder (`index_base.json`, 7468 tests with a primary
  ADR) as the baseline of check K9.
* No test file, no source file, no case file and no roadmap note of F-040, F-042, F-043, F-060, F-105, F-120, F-121, F-147 or F-148
  was opened.

I did not write the rows, ADR 0705, ADR 0715 or the tests.

## The rows (fixed by the acceptance, no draw)

The 18 rows are the ones the acceptance lists, all of them: 0706 **H5, H7, H9, H10, H11, H13, H14, H15, H17, H20, H21, H23** (12),
0707 **W6, W7, W8** (3), 0708 **C3, C6** (2), 0710 **T5** (1). Arithmetic: 48 rows in the five ADRs, 10 read by ADR 0705, 20 by
ADR 0715, 48 - 30 = 18; ADR 0709 has none left. No row is replaced; a row that cannot be judged counts as a disagreement. Each
row is read once.

## The criterion (ADR 0715, adopted by reference)

The criterion is the section "What "a row agrees" means" of ADR 0715 (its lines 56 to 96), unchanged, with one substitution: the
commit is 85313e53 instead of 68f66132 (the files are identical, see above; the roadmap notes are read at 85313e53). Its opening and
its clauses, quoted:

> A row is read against (1) the test file its section names, at 68f66132, (2) any source file the row cites, (3) the note in `docs/roadmap.json` of any roadmap item the row cites. The row **agrees** if every clause below that applies holds, and **disagrees** if one fails. A row that makes several claims agrees only if all of them hold.

> **A. The test exists.** The test function the row names is a `#[test]` function of the file the section names, spelled as in the row. "same" means the function of the nearest row above that names one. A table without a test column (H6 to H10): the assert must be in a `#[test]` function of the section's file; if the row's text names a function, that one. If the test is `#[ignore]`d and the row does not say so, the row disagrees.

> **B. It checks what the row says.** An assert of that test (or of a helper the test calls) checks the quantity or behaviour the row describes. An assert that the test can skip (inside a branch that the test's own set-up does not always take) does not count unless the row says it is conditional. A row is not judged on what it leaves out: other asserts of the test that the row does not mention are no disagreement, unless the row says "only", "all" or gives a count, in which case the count or the claim is checked.

> **C. With the limit the row states.** Numbers are compared after normalisation: `10 %` equals `0.10` equals `1e-1`; `1e-13` equals `1.0e-13`; a limit the row gives as a relative band of a reference (`within 10 percent of 17.4`) equals a code literal for the same band (`0.1 * 17.4`, or `1.74`) if the two are arithmetically equal to three significant figures. Rounding: a row that states a number different from the code's literal disagrees, however close (0.03 against 0.031), unless the row marks it with "about", "roughly" or "~", in which case it agrees within 5 percent relative. Sense: an upper bound must be an upper bound, a lower a lower, a two-sided band two-sided; absolute against relative must match. `<` and `<=` are not distinguished. Units: the unit the row gives must be the unit of the quantity in the code (as the code's own units, comments or the set-up make it). A row that gives no unit for a dimensionless or SI number agrees. A wrong unit disagrees. Kind "exact": the test asserts equality (`assert_eq!`, `==`, a match on a variant or a string) with the value the row states. Kind "structural": the test asserts the presence, shape, count or order the row states.

> **D. Mesh and set-up descriptions.** A number the row gives for the mesh or set-up (cells, lengths, Reynolds number, a wall temperature) must equal the one the test builds or loads, with the normalisation and the "about" rule of C. A description with no number ("the channel mesh") is not judged.

> **E. The source says what the row says it says.** If the row's source column, or its text, attributes a number or a statement to a roadmap note, the `note` (or `acceptance`) of that item in `docs/roadmap.json` at 68f66132 contains it, with the normalisation of C. If the source is the code or the file, A to C cover it. A table without a source column (H11 to H24) has no clause E unless the row's text cites a note.

> **Recorded values.** A row may quote a measured value ("2.1 to 2.4 percent recorded"). If it is attributed to a note or a code comment, it is checked there under E. If it depends on runtime output and nothing written carries it, the named test is run once (release, that test binary only) and the value compared if the test prints it; if it cannot be checked, the row gets a **reservation** and is judged on its other clauses.

Where a clause leaves a case open, the reading that is harder on the row is taken, and the table says so (standing instruction).
Two cases are fixed now because the rows quoted in the group ADRs raise them:

* A statement the row attributes to a code comment ("the file's comment says ...") is checked against that comment under E.
* A number the row derives itself and marks "about" (H5 "last about 1e-9.8") is recomputed from the test's set-up under D with
  the 5 percent rule applied to the number as written (here the exponent).

Each row is recorded with: the ADR file and line of the row, the file and line of the assert, agree or disagree, the failing
clause and a reason, and any reservation.

## The addendum rule

A correction is an addendum **appended at the end** of the group ADR that holds the row, headed
`## Addendum 1: corrections of rows (ADR 0717, 2026-11-17)`. It names each corrected row, quotes the words that are wrong, says
what the code does with the file and line, and gives the corrected row in full. The row in the table above it stays as written;
the addendum says that its text replaces the row's for any reader. Nothing above the addendum is edited, not even a typing error.

* H22 (0706) and W1 (0707) are corrected whatever the reading finds, because the acceptance asks for it. Before writing each I
  read the test myself. If the source supports what the acceptance says is wrong, the correction is written. If it does not, no
  correction is written against the source: the finding is recorded here and listed as an open question (rule 6: not taken).
* Every row of the 18 judged "disagree" is corrected in the addendum of its ADR (0706, 0707, 0708 or 0710), in the same way.
* A defect seen in passing in a row outside the 18 and outside H22 and W1 is listed in this ADR as an observation, not counted,
  and not corrected here (standing instruction: the rows read by ADRs 0705 and 0715 are theirs).
* A reservation alone leads to no correction.
* The addenda avoid the phrases that rule H2 of `scripts/adr_test_index.py` reads as a registration, and so does this ADR.

## Checks

| # | check | limit | estimate behind it |
|---|---|---|---|
| K1 | the rows read are the 18 of the acceptance, each once | exact, 18 | the list is given |
| K2 | agreement = rows that agree / 18 | **at least 16 of 18** | the acceptance; ADR 0705 found 10 of 10 and ADR 0715 19 of 20 on other rows, so 1 or 2 of 18 is the expected order |
| K3 | every disagreement is listed with the ADR file and line of the row, the file and line of the code or note, and the failing clause | all of them | by construction |
| K4 | every disagreement, H22 and W1 are corrected by an addendum of their ADR | all of them | by construction |
| K5 | the text above each addendum is unchanged: for every ADR file edited other than this one, `git diff --numstat 85313e53` shows **0** deleted lines, and the first N lines of the file are byte-identical to `git show 85313e53:<file>`, N its line count there (98, 55, 39, 39, 46) | exact | appended text only |
| K6 | no Rust file, script, golden or hash table is edited: `git diff --name-only 85313e53` lists only files under `docs/adr/` ending in `.md` | exact, 0 other files | no code is written |
| K7 | `cargo test -q --release -p rcfd-cli --test docs_site` after the last ADR edit | passes | ADR files only |
| K8 | `python -m unittest discover -s scripts` from the worktree | passes, 0 failures and 0 errors | the scripts read `docs/adr` |
| K9 | the test-to-ADR index after the edits against the baseline: tests whose `primary_adr` changed | **0** | the new texts cite test files (evidence E2) but 0717 is a higher number than the ADRs that already cite them and carries no H2 phrase; the number of tests whose `adrs` list gains 0717 is reported, no limit |
| K10 | reported, no limit: agreement per ADR; agreement if every reservation is counted as a disagreement; agreement of all 48 rows over the three readings | none | information |

Counting: a row is one unit whatever the number of claims in it. A reservation is not a disagreement in K2 and is counted in K10.
H22 and W1 are not among the 18 and do not enter K2.

## Negative controls

* N1 (the reader's leniency, as ADR 0715 N1). After the reading, for every row judged "agree", each numeric literal of the row's
  limit cell is searched as written in the cited test file by a script in the scratch folder. A literal not found as written must
  be explained in the table by a named normalisation of clause C or D (percent, band, exponent form, a count of listed things,
  a value the row derives) or the row is changed to "disagree". The script cannot change a "disagree" to "agree". This is the one
  permitted revision of a reading and it goes one way.
* N2 (the addendum check can fail). K5's commands are run on a scratch copy of ADR 0707 in which one character above the
  addendum is changed; they must report it (the prefix is not identical).
* N3. There is no planted wrong row, as in ADR 0715: a limit of the method.

No code is changed, so there are no mutations other than N2.

## Order of work

1. This file is committed alone.
2. H22, W1 and the 18 rows are read against the test files and the roadmap notes and recorded in Addendum 1 of this ADR. That is
   committed.
3. The addenda of the group ADRs are written and committed; K5 to K9 are run.
4. Only then are the addenda of ADR 0715 opened. No row overlaps, so what ADR 0715 says is reported next to this reading for
   information; H22 and W1 are compared with its account.

A named test is run only under the "recorded values" clause (release, `CARGO_TARGET_DIR` in the scratch folder, that test binary
only), never a whole crate's suite.

## Decision rule

* **done**: K1 and K3 to K9 hold and K2 is at least 16 of 18.
* **partial**: the rows were read, listed and corrected but K2 is below 16 of 18; or H22 or W1 could not be corrected as the
  acceptance words it because the source says otherwise; or K7, K8 or K9 fails for a reason outside these files.
* **todo**: the rows could not be read.

## Limits (what is not covered)

* After this reading every one of the 48 rows has been read once by a reader other than its author; none has been read twice.
* The reading says whether a row matches the code and the notes. It does not say whether the test's limit is a good one or whether
  the prose outside the row tables is right. Tests are not run except under the "recorded values" clause.
* A row is not judged on what it leaves out (clause B).
* One reader, who knows the totals of the two earlier readings and that H22 and W1 are wrong (disclosed above).
* The corrected rows of the addenda are written by this reader and are not read by another.

## Addendum 1: the reading (2026-11-17)

Written after the registration was committed (a1cf7f66) and before any addendum of a group ADR was written and before the addenda
of ADR 0715 were opened. Files read, all at 85313e53: the seven test files in full or in the part that holds the rows' tests
(`physics_workflow.rs` lines 95 to 146, `workflow_engine.rs` all, assistant `workflow.rs` lines 1 to 150, api `workflow.rs` lines
1 to 263, `channel_wall_function.rs` all, `physics_cases.rs` lines 1 to 107, `table_expr.rs` all); the note and acceptance of F-040
in `docs/roadmap.json` (the only note a row of the 18 or W1 cites); `cases/wedge/wedge.toml`; the exit-code table of
`docs/run-control.md` (lines 73 to 81); two lookups in source files for H22 and W8 (`crates/rcfd-api/src/workflow.rs` lines 143 and
144, the doc comment at `crates/rcfd-mesh/src/gen.rs` line 321). No test was run: the only recorded values of the 18 rows (W7) are
attributed to the F-040 note and were checked there under clause E. None of the seven files has an `#[ignore]` (`grep -c`: 0 each).

Short names: PW = `crates/rcfd-assistant/tests/physics_workflow.rs`, WE = `crates/rcfd-assistant/tests/workflow_engine.rs`, AW =
`crates/rcfd-assistant/tests/workflow.rs`, SW = `crates/rcfd-api/tests/workflow.rs`, CH =
`crates/rcfd-turb/tests/channel_wall_function.rs`, PC = `crates/rcfd-cli/tests/physics_cases.rs`, TE =
`crates/rcfd-core/tests/table_expr.rs`. "ADR line" is the line of the row in its group ADR.

### The 18 rows

| row | ADR line | test file and lines of the asserts | reading | reason |
|---|---|---|---|---|
| H5 | 0706:46 | PW 120 to 146 (asserts 129, 134, 135, 139, 144, 145) | agree | the laminar tree with and without `res_nut` has equal verdicts (129); the rising series is `1e-3 * 10^(i/5)`, `0..50`, on `k_epsilon` with `relax 0.7`, advice contains `res_nut` and the changes contain `turbulence.relax`; transient compressible verdict equal to the quoted string; steady with `steady_tolerance 1e-6`, first finding series `res_rho`, verdict `converged`. D: the last sample is 10^(-49/5) = 10^-9.8, as the row says |
| H7 | 0706:59 | WE 126, 127, 129, 138, 139 | agree | `next == mesh_check`, 5 tasks, `exit_code == 0`, `ran` equal to the four ids in order, `ran[3]` outcome state `converged` |
| H9 | 0706:61 | WE 152, 153, 155 to 158, 159, 160, 163 to 173 | agree | `max_steps 15` (75); outcome `step_limit`; `restart.rcfd` exists; the triple (`step_limit`, 15, true); the final text equal to the quoted sentence; 6 requests; seven tool names looked up in `reqs[0]` (count checked: seven) |
| H10 | 0706:62 | WE 209, 210, 216 | agree | the assert of 210 sits in a branch on the tool name, but 216 asserts `saw_error`, which is only set in that branch, so the branch cannot be skipped (clause B) |
| H11 | 0706:71 | AW 35 to 50 | agree | 60 samples `0.1 * 1.15^i` for both series; verdict and the pattern of `res_u` equal `diverging`; the three paths; `(0.05..0.9).contains` is the half-open band the row writes; `alpha_u 0.9` at AW 18 |
| H13 | 0706:73 | AW 65 to 68 | agree | 80 samples, 1e-2 and 5e-2 alternating; pattern and verdict `oscillating` |
| H14 | 0706:74 | AW 73 to 81 | agree | `decay(30, 0.3)` then 70 samples of 3e-4; tolerance 1e-6 at AW 19; pattern `stagnating`; a change at `solver.non_orthogonal_correctors` |
| H15 | 0706:75 | AW 86 to 92 | agree | 60 samples at rate 0.1: `converging` and an empty list of changes; 200 at 0.2: `converged`; two samples: `not enough data` |
| H17 | 0706:77 | AW 122 to 146 | agree | six patches (count checked); each of the seven statements of the row has its assert (134, 135, 136 to 139, 140, 141 and 142, 143, 145 and 146); `settings[1]` is the second setting |
| H20 | 0706:87 | SW 72, 76, 83, 84, 97, 99, 100, 102, 113, 117, 118, 120, 129, 135 | agree | every clause of the row has its assert with the row's value (1 task at 117, 5 tasks at 135, `physics.nu 0.02` at 125) |
| H21 | 0706:88 | SW 153, 158 to 162, 164, 167, 170, 177 to 179 | agree | insert after `mesh_check` with `mesh.nx 14` (144 to 146); delete refused with `depend`; value 14; `finer` done, `mesh_check` stale; after `report` and `finer` without prerequisites `report` and `solve` stale, `finer` done |
| H23 | 0706:90 | SW 225, 226, 231, 236, 245, 251 | agree | format `rcfd-workflow`, 6 tasks written and 6 read, the two paths refused with `sandbox`, the cycle with `cycle`, `results/solution.vtu` exists after the task `export_results` |
| W6 | 0707:41 | CH 161 | agree | `(u1 / want1 - 1).abs() < 5e-3`, `want1 = wc.u_plus(yplus1)`: relative, upper bound |
| W7 | 0707:42 | CH 164 to 188; F-040 note | agree | cells with `p[1] < 1.0` and wall distance above `y1 * 1.001`; `l = 0.41 * y`, `tau = 1 - y` (h = 1); Simpson with `n = 4000` from `y1`; `worst < 0.03` relative. E: the F-040 note has "max deviation 2.1% / 2.3% / 2.4%, test tolerance 3%" for "ny = 8/12/16" and the sentence on the offset; the comment at CH 186 and 187 says what the row says it says |
| W8 | 0707:43 | CH 190 to 193 | agree | `col0 = (c / 4) * 4`; the difference of the x velocity of cell `c` and cell `col0` is below 1e-7, absolute, for every cell. Remark (no clause fails): the group the code calls a column is the 4 cells of one height along the flow (cell index `i + nx j`, nx = 4, `gen.rs` 321), which is what streamwise uniformity needs |
| C3 | 0708:25 | PC 63 to 65 | agree | exit code `Some(0)` and `solution.vtu` is a file; nothing reads the end time. The case has `end_time = 3.0` and exit 0 is "a transient run reached its end time" in `docs/run-control.md` line 73 |
| C6 | 0708:28 | PC 99 to 106 | agree | `matches!(code, Some(0) | Some(5))` and `solution.vtu` is a file; the comment at PC 100 says what the bracket of the row says |
| T5 | 0710:34 | TE 55 to 70 | agree | the four calls and the four message fragments, each as the row quotes it; `L` is a length (54); the bare table's input is `s` (63) |

Clause A holds for all 18: each named function is a `#[test]` of the section's file, spelled as in the row, none ignored.

**K2: 18 of 18 agree. No disagreement, no reservation, no row that could not be judged.** K1: the 18 rows of the acceptance, each
read once. K3: no disagreement to list.

### N1 (literal search)

`f795-scratch/n1_literals.py` searched each numeric literal of the limit cell of the 18 rows as written in the test file. Found as
written: all but seven. The seven, each explained, none changing a reading:

| row | literal | explanation |
|---|---|---|
| H5 | `50` | in the file as the range `0..50` (PW 121, 131); the script's pattern does not match a number that follows a dot |
| H5 | `1e-9` (of "1e-9.8") | a value the row derives and marks "about": 10^(-49/5), clause D above |
| H11 | `1.15` | in the file as `1.15f64` (AW 35): the type suffix |
| W7 | `2.1`, `2.3`, `2.4` | recorded values, attributed to the F-040 note and found there (clause E) |
| W7 | `8` | in the file as `8usize` (CH 132): the type suffix |

### H22 and W1 (not among the 18, read before their corrections)

| row | ADR line | code | finding |
|---|---|---|---|
| H22 | 0706:89 | SW 183 to 213 | The first half holds: `max_steps 5` (190), `ok == false`, the last task of `ran` is `solve` (195) with a message containing `step_limit` (196), `solve` is `failed` and `report` `not_run` (198 to 201). The second half does not: the row says the invalid boundary name "fails the validate task". SW 212 takes the first task of `ran` whose `ok` is false and SW 213 asserts only that its message contains `nonexistent`; no assert reads its id or kind. The words come from the comment at SW 202. (For information: the template has no task with the id `validate`; the task of kind `validate` has the id `setup`, `crates/rcfd-api/src/workflow.rs` 144.) The source supports the acceptance; the correction is written |
| W1 | 0707:36 | CH 95 to 126, 33 | The row says "the 12-cell uniform channel mesh". `run_channel(12, 1.0)` builds `box_2d_graded(4, ny, 1.0, 2.0, ...)` (CH 33): 4 x 12 = 48 cells, uniform, 12 across the height (clause D). The row's "20-cell mesh" is 3 x 20 = 60 cells (CH 107 to 115), which the row's own bracket "(3 x 20 cells, height 2)" already says. The limits hold: 1e-13 at CH 101, 119 and 124, `nope` at 126; the `xmin` and `nope` asserts are on the graded mesh. E holds: the F-040 note has "wall distance equals min(y, 2h-y) to 1e-13 on uniform and graded channel meshes and equals x for a side wall". The source supports the acceptance; the correction is written |

### Observations (not counted, no correction by the registered rule)

* The code comment at CH 189 announces "symmetry about the centre line"; no assert checks it. Row W8 does not claim it.
* Row W8's word "column" (see its remark) follows the code's variable name; a reader who takes it geometrically reads it as the
  cells across the height. Remaining question 1 of Addendum 2.
