# ADR 0715: F-794 registration: a second reader for the limit rows of ADRs 0706 to 0710

Status: **REGISTERED 2026-11-17, written before the sample of F-794 was drawn and before any row was read against a file.** Written by
the reading agent on branch `f794-work` (from 68f66132, which holds F-769 / ADRs 0705 to 0710), worktree `w49i-work/f794`, scratch
`w49i-work/f794-scratch`. Not yet accepted by the maintainer. Every choice below that the maintainer would normally be asked about
is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation) and is marked "(standing instruction)".

Roadmap item F-794 (deps F-769), a follow-up proposed by ADR 0705. No code is written; this ADR is the only file changed.

House rules (`w49i-work/AGENT_RULES.md`) apply: limits fixed here and not changed after the reading, the reading made once, existing
tests not edited, no golden or hash table edit, clean-room. ADRs 0705 to 0710 are not edited.

## What F-794 asks

> Every limit not stated here is registered in the ADR before the run. A reader other than the author of the rows reads a registered sample of at least 15 rows of ADRs 0706 to 0710, including 10 of the 22 rows of 0706 not read in ADR 0705, against the files and the roadmap notes they cite, with at least 90 percent agreement, and the disagreements are listed with the line.

## What was read beforehand (disclosed)

* `AGENT_RULES.md` and the brief.
* The entries of F-794, F-769 and F-786 in `docs/roadmap.json`. The note of F-769 summarises some limits of the five ADRs (for
  example the four numbers of 0707) and says that the first reading found "10 of 10 agree (one imprecise count)". I therefore know
  the first reader's total. I do not know which row carried the reservation or why.
* The top of ADR 0704 and its "For the lead" section, and the "For the lead" section of ADR 0572, for the form.
* Of ADR 0705: the headings (`grep '^#'`), the lines that contain the word "sample" (cut to 260 characters; among them the result
  row "10 of 10 agree, one with a reservation" and the heading of open question 3), the registered sample rule (lines 82 to 90) and
  the first two lines of the sample paragraph of the addendum (line 150 and 151), which give the list of rows read: **H1, H6; W1,
  W2; C1, C2; M1, M4; T1, T2**. The readings and reasons that follow that list were not opened.
* Of ADRs 0706 to 0710: the headings, the header lines of the row tables, and the first 110 characters of each row line (the id and
  the beginning of the test name), to count the rows and to see the shape of the tables. Row counts: 0706 24 (H1 to H24), 0707 8
  (W1 to W8), 0708 7 (C1 to C7), 0709 4 (M1 to M4), 0710 5 (T1 to T5); 48 rows, 10 read by ADR 0705, 38 not.
* No test file, no source file and no roadmap note of F-040, F-043, F-105, F-120, F-121 or F-148 was opened.

I did not write the rows, ADR 0705 or the tests.

## The sample (fixed now, drawn after this file is committed)

A row is a numbered limit row of a group ADR, identified by the id in its first column. The pool of an ADR is its rows **not read
by ADR 0705**, in the order they appear in its tables:

| ADR | pool (in table order) | size | drawn |
|---|---|---|---|
| 0706 | H2, H3, H4, H5, H7, H8, ..., H24 | 22 | 10 |
| 0707 | W3, W4, W5, W6, W7, W8 | 6 | 3 |
| 0708 | C3, C4, C5, C6, C7 | 5 | 3 |
| 0709 | M2, M3 | 2 | 2 (all) |
| 0710 | T3, T4, T5 | 3 | 2 |

For each ADR the draw is `random.Random(794).sample(pool, k)` (Python 3, a fresh generator seeded 794 for each ADR, the pool a list
of the id strings in the order above). The sample is **20 rows**, none of them read by ADR 0705, 10 of them from the 22 unread rows
of 0706. The command and its output are recorded in the addendum. No row is replaced: a row that cannot be judged counts as a
disagreement. The sample is read once.

The seed is the item number. The sizes: the acceptance fixes 10 for 0706 and at least 15 in all; the other four ADRs get about half
of their unread rows each so that every ADR is read (standing instruction).

## What "a row agrees" means

A row is read against (1) the test file its section names, at 68f66132, (2) any source file the row cites, (3) the note in
`docs/roadmap.json` of any roadmap item the row cites. The row **agrees** if every clause below that applies holds, and
**disagrees** if one fails. A row that makes several claims agrees only if all of them hold.

* **A. The test exists.** The test function the row names is a `#[test]` function of the file the section names, spelled as in the
  row. "same" means the function of the nearest row above that names one. A table without a test column (H6 to H10): the assert
  must be in a `#[test]` function of the section's file; if the row's text names a function, that one. If the test is `#[ignore]`d
  and the row does not say so, the row disagrees.
* **B. It checks what the row says.** An assert of that test (or of a helper the test calls) checks the quantity or behaviour the
  row describes. An assert that the test can skip (inside a branch that the test's own set-up does not always take) does not count
  unless the row says it is conditional. A row is not judged on what it leaves out: other asserts of the test that the row does
  not mention are no disagreement, unless the row says "only", "all" or gives a count, in which case the count or the claim is
  checked.
* **C. With the limit the row states.**
  * Numbers are compared after normalisation: `10 %` equals `0.10` equals `1e-1`; `1e-13` equals `1.0e-13`; a limit the row gives as
    a relative band of a reference (`within 10 percent of 17.4`) equals a code literal for the same band (`0.1 * 17.4`, or `1.74`)
    if the two are arithmetically equal to three significant figures.
  * Rounding: a row that states a number different from the code's literal disagrees, however close (0.03 against 0.031), unless
    the row marks it with "about", "roughly" or "~", in which case it agrees within 5 percent relative.
  * Sense: an upper bound must be an upper bound, a lower a lower, a two-sided band two-sided; absolute against relative must match.
    `<` and `<=` are not distinguished.
  * Units: the unit the row gives must be the unit of the quantity in the code (as the code's own units, comments or the set-up
    make it). A row that gives no unit for a dimensionless or SI number agrees. A wrong unit disagrees.
  * Kind "exact": the test asserts equality (`assert_eq!`, `==`, a match on a variant or a string) with the value the row states.
    Kind "structural": the test asserts the presence, shape, count or order the row states.
* **D. Mesh and set-up descriptions.** A number the row gives for the mesh or set-up (cells, lengths, Reynolds number, a wall
  temperature) must equal the one the test builds or loads, with the normalisation and the "about" rule of C. A description with no
  number ("the channel mesh") is not judged.
* **E. The source says what the row says it says.** If the row's source column, or its text, attributes a number or a statement to
  a roadmap note, the `note` (or `acceptance`) of that item in `docs/roadmap.json` at 68f66132 contains it, with the normalisation
  of C. If the source is the code or the file, A to C cover it. A table without a source column (H11 to H24) has no clause E unless
  the row's text cites a note.
* **Recorded values.** A row may quote a measured value ("2.1 to 2.4 percent recorded"). If it is attributed to a note or a code
  comment, it is checked there under E. If it depends on runtime output and nothing written carries it, the named test is run
  once (release, that test binary only) and the value compared if the test prints it; if it cannot be checked, the row gets a
  **reservation** and is judged on its other clauses.

Each row is recorded with: the ADR line of the row, the file and line of the assert, agree or disagree, the failing clause and
a reason, and any reservation.

## Checks

| # | check | limit | estimate behind it |
|---|---|---|---|
| R1 | the sample is the one registered: 20 rows, none of ADR 0705's ten, 10 of 0706's unread 22 | exact; sample size at least 15 (acceptance), 20 registered | deterministic draw |
| R2 | agreement = rows that agree / rows sampled | **at least 90 percent: at least 18 of 20** | the acceptance; the first reader found 10 of 10 on other rows |
| R3 | every disagreement is listed with the ADR file and line of the row, the file and line of the code or note, and the failing clause | all of them | by construction |
| R4 | reported, no limit: agreement of the 10 rows of 0706 alone; agreement per ADR; agreement if every reservation is counted as a disagreement | none | information |
| R5 | `cargo test -q --release -p rcfd-cli --test docs_site` after the ADR edits | passes | this ADR adds one file under `docs/adr` |

Counting: a row is one unit whatever the number of claims in it. A reservation is not a disagreement in R2 and is counted in R4.

## Negative controls

* N1 (a control of the reader's leniency). After the reading, for every row judged "agree", each numeric literal of the row's
  limit cell is searched as written in the cited test file by a script (`f794-scratch`). A literal that is not found as written
  must be explained in the table by a named normalisation of clause C (percent, band, exponent form) or the row is changed to
  "disagree". The script cannot change a "disagree" to "agree". This is the one permitted revision of a reading and it goes one way.
* N2. The draw is reproduced by the command in the addendum; anyone can re-run it.
* N3. There is no planted wrong row: a reader cannot be blind to a row he plants. This is a limit of the method.

No code is changed, so there are no mutations.

## Order of work

1. This file is committed alone.
2. The sample is drawn. Each row is read against the files and notes and recorded in a table in Addendum 1. That is committed.
3. Only then are the readings of ADR 0705's addendum opened. By the sample rule no row overlaps, so there is no row-by-row
   comparison of the two readers; what the first reading says is reported next to the second for information.

## Decision rule

* **done**: R1, R3 and R5 hold and R2 is at least 18 of 20.
* **partial**: the sample was read and listed but R2 is below 18 of 20, or R5 fails for a reason outside this ADR. The rows are
  not corrected here (ADRs 0706 to 0710 are not edited); the corrections are proposed as a follow-up.
* **todo**: the sample could not be read.

Whatever the result, each disagreement is proposed for correction in a follow-up, because a wrong row stays wrong at 95 percent.

## Limits (what is not covered)

* 20 of the 38 unread rows; 18 rows of the five ADRs remain with no reading or one.
* The reading says whether a row matches the code and the notes. It does not say whether the test's limit is a good one, whether
  the test passes, or whether the ADR's prose outside the row tables ("does not check" sections) is right.
* A row is not judged on what it leaves out (clause B), so a table that omits an assert of a test is not caught here.
* One second reader, who knows the first reader's total (disclosed above).
* No overlap with the first reader's rows, so this measures the rows, not the agreement of two readers on the same row.

## Addendum 1: results (2026-11-17)

Written after the registration was committed alone (d0b55306). No row of ADRs 0706 to 0710 was read against a file before that
commit. No Rust file, script, golden or hash table was edited; ADRs 0705 to 0710 are unchanged. No test was run for the reading:
no sampled row needed runtime output (the one recorded value, 18.17 in C5, is in the test's comment and in the F-043 note).

**The draw** (Python 3.9.13, once, output kept in `f794-scratch/draw.txt`):

```
pools = {'0706': ['H%d' % i for i in range(2, 25) if i != 6], '0707': ['W%d' % i for i in range(3, 9)],
         '0708': ['C%d' % i for i in range(3, 8)], '0709': ['M2', 'M3'], '0710': ['T3', 'T4', 'T5']}
k = {'0706': 10, '0707': 3, '0708': 3, '0709': 2, '0710': 2}
for a in pools: print(a, random.Random(794).sample(pools[a], k[a]))

0706 ['H12', 'H8', 'H19', 'H4', 'H24', 'H16', 'H22', 'H3', 'H2', 'H18']
0707 ['W5', 'W4', 'W3']
0708 ['C5', 'C4', 'C7']
0709 ['M3', 'M2']
0710 ['T4', 'T3']
```

**The readings.** "ADR line" is the line of the row in its group ADR at 68f66132. Test files are under `crates/`. The roadmap
notes are those of `docs/roadmap.json` at 68f66132.

| row | ADR line | test and lines read | reading | reason |
|---|---|---|---|---|
| H2 | 0706:43 | `rcfd-assistant/tests/physics_workflow.rs` 41 to 55 | agree | the three `k_epsilon` inlet paths (45 to 47), `boundaries.inlet.velocity` (49), `turbulence_length` for `k_omega_sst` (53) and no wall path containing `turbulence` (54) are asserted as the row says |
| H3 | 0706:44 | same file 58 to 67 | agree | inlet has `compressible_bc` and `pressure_value`, not `pressure` (62 to 64); outlet and walls by `assert_eq!` on the whole list (65, 66) |
| H4 | 0706:45 | same file 70 to 117 | agree | every claim is an assert: "laminar flow only" (84), `y+` and not laminar (86), `ly = 0.1`, `ny = 20` (90), warning at `turbulence.model` (96), `yp > 500.0 && yp < 1500.0` (100), `SST` (101), `wall_layers = 10`, `first_layer_height = 1e-5` (104), `yf < 3.0` (114), `cfl = 0.95` and `compressible.cfl` (115, 116). The source cell quotes the comment of line 99 correctly (Re_x 1e7, u_tau about 0.36, 0.005, about 900) |
| H8 | 0706:60 | `rcfd-assistant/tests/workflow_engine.rs` 23, 57, 65, 141 to 149 | agree | the inserted task is `export`, kind `export_vtk`, dir `results` (57); `workflow.complete == true` and `results/solution.vtu` (142, 143); `physics.nu = 0.02` (65) and all tasks `stale` (145 to 148) |
| H12 | 0706:72 | `rcfd-assistant/tests/workflow.rs` 54 to 61 | agree | `decay(30, ..)`, `y[20] = NAN`, `non_finite`, verdict contains `NaN`, evidence contains "sample 20" |
| H16 | 0706:76 | same file 100 to 118 | agree | the three inputs carry the numbers of the row (102, 106, 114); verdicts `good`, `usable with care` with the advice, `not usable` with 4 findings (104, 108 to 112, 116, 117) |
| H18 | 0706:78 | same file 150 to 172 | agree | `ok == false` and the first error contains `nowhere` (157, 158); `'walls'` with "no boundary condition" (165 to 169); `Re =` (170); none with `frontAndBack` (171) |
| H19 | 0706:79 | same file 175 to 207 | agree | `good` (189), ok (191), 3 suggestions (194), "not enough data" (198), hashes equal (203), error without a session (206) |
| H22 | 0706:89 | `rcfd-api/tests/workflow.rs` 183 to 214 | **disagree (clause B)** | the first half is as the row says: `max_steps = 5` (190), the last task run is `solve` (195) with `step_limit` in its message (196), `report` is `not_run` (198 to 201; the test also asserts `solve` is `failed`). The second half says the invalid boundary name "fails the validate task". The test takes the first task of `ran` with `ok == false` (212) and asserts only that its message contains `nonexistent` (213); it does not assert which task that is. The words "the validate task" come from the comment of line 202, not from an assert. Also no task has the id `validate`: in the template the task of kind `validate` has the id `setup` (`rcfd-api/src/workflow.rs` 144) |
| H24 | 0706:91 | same file 256 to 313 | agree | ok and `exit_code` 0 (261); `max_steps` 25, checkpoint, status file, `status_every` 10 (266, 267); `step_limit`, `ready`, the checkpoint file (270 to 272); `status.read` gives `step_limit`, complete, 1 checkpoint (274 to 281); stop file gives 7 and `stopped` (287, 288); `converged`, more than 25 iterations, `res/solution.vtu` (297 to 299); the five keys with `../x` and "sandbox" (301 to 306); the four methods (309 to 311) |
| W3 | 0707:38 | `rcfd-turb/tests/channel_wall_function.rs` 130 to 142 | agree | `y1 = 1.0 / ny`, `yplus1 = y1 * RE_TAU` with `RE_TAU = 395.0` (16, 139, 140), `assert!(yplus1 > 11.6)` (142). 395/8 = 49.375, 395/12 = 32.917, 395/16 = 24.688: the row's 49.4, 32.9, 24.7 are these to one decimal, and the F-040 note has "first-cell y+ = 49.4/32.9/24.7" |
| W4 | 0707:39 | same file 143 to 146 | agree | `(yp / yplus1 - 1.0).abs() < 5e-3` for every value of `wall_y_plus` on the two wall patches; the F-040 note has "within 0.5%" (0.5 % = 5e-3) |
| W5 | 0707:40 | same file 147 to 153 | agree | `tau = (nu + nutw) * u / y`, `(tau - 1.0).abs() < 2e-3`, for every wall face; the F-040 note has "equals the imposed 1 within 2e-3 (momentum balance)" |
| C4 | 0708:26 | `rcfd-cli/tests/physics_cases.rs` 69 to 78 | agree | exit code 0 (71); `k`, `epsilon`, `nut`, `y_plus` in `solution.vtu` (73 to 75); first line of `monitors.csv` contains `yplus_lower` (77) |
| C5 | 0708:27 | same file 81 to 95; `cases/channel/sst_channel.toml` 5 | agree | exit 0 (83); the four fields (85); last row, `(yplus_lower - 1.0).abs() < 0.01` (92); `(u_mean / 17.4 - 1.0).abs() < 0.10` (94). The comment of line 93 is "18.17 (DNS 17.4, +4.4 percent, see the F-043 roadmap note)"; the case file cites Moser/Kim/Mansour 1999 with U_b+ = 17.4; the F-043 note has "U_b+ 18.167 (+4.4% vs 17.4)" |
| C7 | 0708:29 | same file 110 to 138; `cases/sod/sod.toml` 22; `cases/channel/kepsilon_inlet.toml` 32, 33 | agree | three `validate` calls each asserted to exit 3 (117, 129, 135); (a) `rk_order = 3` to `1`, `compressible.rk_order` and `line {line}` with the line found in the file (115 to 123); (b) `simplec` to `piso`, `turbulence.model` (127, 130); (c) `p_ref_cell = 99999` under `[solver]`, `solver.p_ref_cell` and `line ` (133, 137). The strings replaced exist in the case files |
| M2 | 0709:27 | `rcfd-api/tests/mesh_params.rs` 21 to 32 | agree | `nx = 8`, `ny = 4`, `lx = 2.0`, `ly = 0.5` (22); extents minus 2.0 and 0.5 below `1e-12` (30); 32 cells (31) |
| M3 | 0709:28 | same file 35 to 38 | agree | `nx = 12`, `ny = 12`, `assert_eq!(m.n_cells(), 144)` |
| T3 | 0710:32 | `rcfd-core/tests/table_expr.rs` 16, 41, 42 | agree | `slots[temp] = 400.0`, `is_nan()`; the table rows are 280, 300, 320 K (16) |
| T4 | 0710:33 | same file 44 to 46 | agree | `nu_t(300 [K])`, `is_constant()`, `assert_eq!(k.constant_value(), Some(0.85e-6))` |

**Control N1** (`f794-scratch/n1.py`, output `n1.txt`): for the 19 rows read as "agree", every number in the limit cell was
searched as written in the cited test file. Not found as written: `49.4`, `32.9`, `24.7` of W3 (computed values, 395/ny to one
decimal, and in the F-040 note: clause C, a derived value, and clause E); `10` and `1999` of C5 (`10 %` is the code's `0.10`, the
percent normalisation of clause C; 1999 is the year of the reference, in the case file the row names). All other literals are in
the files as written. No reading was changed by N1.

**Checks.**

| # | check | limit | measured | met |
|---|---|---|---|---|
| R1 | the registered sample | 20 rows, none of ADR 0705's ten, 10 of 0706's unread 22 | 20 rows as drawn above; 10 of 0706 | yes |
| R2 | agreement | at least 18 of 20 (90 percent) | **19 of 20 (95 percent)** | yes |
| R3 | disagreements listed with the line | all | 1: H22, ADR 0706 line 89, against `rcfd-api/tests/workflow.rs` lines 212 and 213 | yes |
| R4 | reported | none | 0706: 9 of 10. 0707: 3 of 3. 0708: 3 of 3. 0709: 2 of 2. 0710: 2 of 2. Reservations: none, so 19 of 20 either way | - |
| R5 | `docs_site` | passes | recorded in Addendum 2 | - |

**The disagreement.** H22 (ADR 0706 line 89) says the test checks that an invalid boundary name "fails the validate task with a
message containing `nonexistent`". The test checks that some task failed with that message, not which one. It is the smallest kind
of disagreement: the row repeats the test's own comment, and by the template the task that validates is `setup`, which runs second,
so the statement is probably true of the program (not checked by a run). It is still not what the assert pins, and the first half
of the same row shows the difference (there the id `solve` is asserted). A corrected row would read "an invalid boundary name makes
the run fail (`ok = false`) and the first failed task has a message containing `nonexistent`; the test does not assert which task
it is".

**Deviations.** None from the registration. The reading did not need a test run.

**Not run, not covered.** The eight test files were not run. The 18 unread rows (H5, H7, H9, H10, H11, H13, H14, H15, H17, H20,
H21, H23, W6, W7, W8, C3, C6, T5) have no reading; the 10 rows of ADR 0705 have the author's only.

## Addendum 2: the first reading, `docs_site`, decision (2026-11-17)

Written after Addendum 1 was committed (cb0cdb9a). Only then were the readings of ADR 0705 (its lines 150 to 186) opened.

**The first reading next to the second.** The first reader read H1, H6, W1, W2, C1, C2, M1, M4, T1, T2 and found 10 of 10, with a
reservation on W1 (the row says "12-cell uniform channel mesh", the mesh is 4 x 12 cells). The two samples share no row, as
registered, so there is no row on which the two readers can agree or differ. Taken together 30 of the 48 rows have been read once
(10 by the author, 20 here), with 29 read as agreeing and one disagreement (H22).

One remark for information, outside the sample and not counted: I then read row W1 (ADR 0707 line 36). Under clause D of this ADR
(a number given for the mesh must equal the one the test builds) "12-cell" against a 48-cell mesh would be a disagreement, not a
reservation; the same row says "20-cell mesh ... (3 x 20 cells)" for the graded one, so the author's "N-cell" means N cells across
the channel, and the row is unclear, the limit 1e-13 right. The first reader's criterion had no mesh clause. This does not change
R2: W1 is not in the sample.

**R5.** `cargo test -q --release -p rcfd-cli --test docs_site` (target directory `f794-scratch/target`, 4 jobs), run once after
Addendum 1: 2 passed, 0 failed. Met. Nothing else was run; no fmt or clippy because no crate was touched.

**Decision by the registered rule: done.** R1, R3 and R5 hold and R2 is 19 of 20 (95 percent) against at least 18 of 20.

**Remaining questions.**

1. Row H22 of ADR 0706 (line 89) says more than the test asserts. Options: (a) an addendum to ADR 0706 that corrects the row to
   the wording given above (the ADR's text above the addendum stays); (b) add the assert to the test (`bad["id"] == "setup"`), which
   edits an existing test and is against the house rule; (c) leave it, recorded here. Recommendation: (a). Taken on the
   maintainer's standing instruction of 2026-11-11 (take the recommendation) as the follow-up proposed below; not done here because
   this item may not edit ADR 0706.
2. Row W1 of ADR 0707 (line 36): "12-cell" for a 4 x 12 mesh, noted by both readers. Options: (a) correct it in the same addendum
   pass as H22 ("a uniform 4 x 12 channel mesh"); (b) leave. Recommendation: (a).
3. The 18 rows with no reading. Options: (a) read them all in the same follow-up (a census then, 48 of 48 read at least once); (b)
   leave: 95 percent on 20 rows is the evidence asked for. Recommendation: (a), because it is small (eight files of 71 to 313
   lines) and the one disagreement found is of a kind (a comment taken for an assert) that a sample cannot bound well: with 1 of 20
   the 95 percent upper bound on the share of such rows is about 22 percent.

## For the lead: roadmap entries

Proposed status and note of F-794:

> Proposed status: done.

> Follow-up of F-769 (ADR 0705, 2026-11-16). DONE. ADR 0715, 2026-11-17: a reader who wrote neither the rows nor the tests read 20 limit rows of ADRs 0706 to 0710 against the test files and the roadmap notes of F-040 and F-043, by a criterion and a sample registered and committed before the draw (a row agrees if the test named exists, an assert checks what the row says, the number equals the code's after normalisation of percent, exponent and band forms, mesh and set-up numbers equal those the test builds, and a cited note contains what the row says it contains; rounding without "about" disagrees). Sample: `random.Random(794).sample` of the rows not read by ADR 0705, 10 of the 22 of 0706 (H2, H3, H4, H8, H12, H16, H18, H19, H22, H24), W3, W4, W5, C4, C5, C7, M2, M3, T3, T4; none of ADR 0705's ten. Result: 19 of 20 agree (95 percent, limit 90 percent met); by ADR 9 of 10, 3 of 3, 3 of 3, 2 of 2, 2 of 2; no reservation. The one disagreement: row H22 (ADR 0706 line 89) says an invalid boundary name "fails the validate task", but `rcfd-api/tests/workflow.rs` lines 212 and 213 assert only that the first failed task has a message containing `nonexistent`, not which task it is (the words come from the test's comment; the task of kind `validate` has the id `setup`). A literal search of every number of the 19 agreeing rows in the test files found all but the three derived y+ values of W3 (395/ny, in the F-040 note), `10 %` for `0.10` and a year. No test was needed for the reading; docs_site passes; no Rust file, script, golden or hash table and none of ADRs 0705 to 0710 edited. The samples of ADR 0705 and ADR 0715 share no row, so this is a reading of other rows, not a measure of two readers on the same row: 30 of 48 rows are read once, 18 are unread. Outside the sample, row W1 (ADR 0707 line 36) says "12-cell" for a 4 x 12 mesh (the first reader's reservation; a disagreement under the mesh clause of ADR 0715). Follow-up proposed below (correct H22 and W1 by addendum, read the 18 remaining rows).

> Follow-up proposed: Correct rows H22 and W1 and read the 18 unread limit rows of ADRs 0706 to 0710. Deps F-794. Acceptance: Every limit not stated here is registered in the ADR before the run. An addendum to ADR 0706 corrects row H22 (the test asserts that the first failed task has a message containing `nonexistent`, not which task it is) and an addendum to ADR 0707 corrects row W1 (a uniform 4 x 12 mesh), with the text above each addendum unchanged; the 18 rows not read in ADR 0705 or ADR 0715 (H5, H7, H9, H10, H11, H13, H14, H15, H17, H20, H21, H23, W6, W7, W8, C3, C6, T5) are read by a reader other than the author of the rows against the criterion of ADR 0715, every disagreement is listed with the line and corrected by the same addenda, and at least 16 of the 18 agree; no Rust file is edited and docs_site passes.
