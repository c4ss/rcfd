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
