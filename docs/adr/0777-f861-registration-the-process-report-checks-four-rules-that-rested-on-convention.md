# ADR 0777: F-861 registration: the process report checks four rules that rested on convention

Status: **REGISTERED 2026-11-23, written before any code of F-861 and before any run of its checks.** Before this text no
line of `scripts/process_report.py`, of its tests, of an exceptions list or of a test file was changed. What was done:
reading (below), and counting scripts over the repository as it stands at main d10ea0bd (`briefs.py`, `notes.py`,
`cites.py`, `hdr.py`, `records.py`, `blob.py` in the scratch folder; one run of the unchanged
`scripts/adr_test_index.py --out`, written to the scratch folder; one run of the unchanged
`python scripts/process_report.py --check`, which gave 0 failures). Their numbers are below. Results go in addenda at
the end; nothing above them is changed after a run.

It answers the brief `docs/adr/briefs/f861-process-report-convention-checks.md` and follows
`docs/process/agent-rules.md`, version 5. Branch `f861-process-report-convention-checks` from main d10ea0bd, scratch
`w49i-work/f861-process-report-convention-checks-scratch`.

## What is asked

Roadmap item F-861, the binding acceptance (read with a script): `scripts/process_report.py --check` fails, each shown
by a unit test that fails before the change, on (1) a roadmap note entry added after the registered date that is not one
line in the form the agent rules give; (2) a citation in an ADR registered after that date that names a source without a
place in it; (3) a brief whose reading list exceeds the registered limit or whose stated total is not the sum of its
files; (4) a test root file under `crates/*/tests` or a Studio spec without its record in the leading comment. What
exists before the date goes on a dated exceptions list that may only shrink; the 56 Rust and 12 Studio files without a
header get one where the record can be found from git history or the ADR index, and the rest are listed with the reason.
No test's code changes and no default output changes. Every limit not stated there is registered here.

## What was read

`docs/process/agent-rules.md` (version 5, whole); the brief; `scripts/process_report.py` and
`scripts/test_process_report.py` (each whole); `docs/adr/briefs/README.md`; the lists
`docs/process/exceptions/test-root-without-adr.txt`, `studio-spec-without-record.txt`, `registration-kind.txt` and
the count file of section e; the item F-861 of `docs/roadmap.json` (by a script); `scripts/roadmap_proposals.py` (its
head, `check_shape` for the note, and how `apply` joins a note and enters a follow-up); `docs/process/lead-process.md`
(Issuing); `docs/process/documentation-process-review.md` (the lines of round 2 on one-line note entries, on locators and
on the limit on reading, and the line that lists the rules resting on convention); the head of
`scripts/adr_test_index.py` (its rules of evidence and of the primary ADR); ADR 0776 for the form of a registration of a
script; `scripts/test_process_report.measures.mutation.patch` for the form of a committed mutation; the first lines of
the 12 Studio specs and of seven of the 56 Rust files; `git log --follow --diff-filter=A` of the 68 files.

## Readings (before any code)

* **Check (4) exists in part.** Section l of the script (since 2026-11-18) already fails on a Rust test root file whose
  leading comment names no existing `ADR NNNN` and on a Studio spec that names no existing item and no existing ADR, less
  the two lists. A unit test of the Rust half exists and passes. What it does not do: with no list file the section finds
  nothing new (`less_exceptions` returns nothing when the file is absent, "the rule has not started"), so deleting a
  list, or removing it when it is empty, stops the check; a list that grows is accepted (a stale entry is reported,
  an added one is not seen); the Studio half has no unit test. The unit test of (4) that fails before the change is
  therefore on these, not on the plain case, which a test that passes before and after covers.
* **Note entries.** No note of the 792 items holds a line break. `scripts/roadmap_proposals.py` refuses a proposal whose
  note is not one line or does not start `ADR <the proposal's number>, <YYYY-MM-DD>, ` followed by a non-blank; it joins
  the entry to the note with one space. A follow-up it enters has no note. Since 2026-11-18 the notes hold 25 entries
  that start `ADR NNNN, <date>, `, 108 that start `Lead (<date>): ` and 98 that start `Maintainer (<date>): `. The word
  after the date of the 25 is `done` or `partial` in 24 and `DONE` in one. Lengths in characters: the 25, median 662,
  largest 1186; the lead's 108, median 288, two above 1200 (1290 and 2671); the maintainer's 98, largest 701. The
  rules state no length. The highest item number is 887.
* **Citations.** Recent registrations name sources in several forms: `Surname (year)`, `Surname et al. (year)`,
  `Surname and Surname (year)`, a full reference with authors' initials, title and `arXiv:NNNN.NNNNN`, a report
  number. Only the first three and the arXiv number have a shape a script can recognise without reading many other
  things as citations. ADRs are append-only, so a finding in a new ADR can only be closed by text added to it.
* **Briefs.** All 41 briefs have a `## Reading list` whose entries are lines `- <path> <bytes>`, some followed by a
  remark in round brackets; lines of prose after them name further reading without a size. In all 41 the fifth value
  of `Size (lead):` equals the sum of the listed sizes. `.gitattributes` fixes line ends (`eol=lf`), so the size of a
  file in a commit is its size on disk. Of the 207 listed entries, 201 state the size the file has in the commit that
  added the brief; 5 differ and one names a file that commit does not hold (in 4 briefs).
* **The 68 files.** The index gives every test of 31 of the 56 Rust files one primary ADR (the same for all tests of
  the file) and gives the tests of 25 none. Seven of the 31 name their ADR in the leading comment as a path
  (`docs/adr/0008-mrf.md`), which the index reads and section l does not. For each of the 12 Studio specs the subject
  of the commit that first added it names an item (ten) or an ADR (two, ADR 0801).

## Arithmetic (before any code)

* **The reading limit.** The review sets it as "the 90th percentile observed" of the bytes to read "after two weeks of
  the size data". The data are the 41 stated totals of the briefs, 2026-11-18 to 2026-11-23 (six days, not two weeks).
  Sorted, the 37th of 41 (nearest rank, 0.9 x 41 = 36.9) is 194023; linear interpolation between the order statistics
  (position 0.9 x 40 = 36) gives the same value. The limit is **194023 bytes**. Four briefs are above it: 214687, 280627,
  346865, 473792.
* **The length of a note entry.** The largest of the 25 entries written in the form of rule 9 is 1186 characters. The
  limit is **1200 characters**: the next round number above what the form has produced so far, so that the check
  holds the practice that exists and refuses an entry that carries the history of the work.
* **The lists after the headers.** By rule H below: Rust 56 - 31 = **25** entries stay; Studio 12 - 12 = **0** stay.

## Design (fixed now)

**The date.** Each new check applies from **2026-11-24** (the day after this registration), written `D` below.

**(1) Note entries, section n.** An entry of a note begins at a head, at the start of the note or after a space: the
agent's head `ADR NNNN, YYYY-MM-DD, ` (rule 9: "ADR number, date, outcome, one sentence", and the start
`scripts/roadmap_proposals.py` requires), or the lead's heads `Lead (YYYY-MM-DD): ` and `Maintainer (YYYY-MM-DD): `
(rule 9: the lead "adds his own entry after it"; the rules give that entry no form, these two are the forms in use).
It ends before the next head or at the end of the note. An entry whose date is `D` or later is read; it fails when

* it has the agent's head and the text after the head does not begin with one of the words `done`, `partial`, `todo`
  (the outcome: the three status words of the roadmap, which rule 9 pairs with the entry, in lower case as the proposal
  file writes them);
* it is longer than 1200 characters, head included, whichever head it has;
* it holds a line break.

An item numbered above 887 was entered after this registration, so all of its note is new: it fails when its note is
not empty and does not begin with a head. Why this form: the rule's four parts are checked as far as a script can tell
them (number and date by the head, outcome by the word, "one line" by the line break, "one sentence" by the length; a
count of sentence ends would misread decimals and abbreviations). The lead's entries are held to the same length and to
nothing else.

**(2) Places of citations, section s.** An ADR is read when the date of its status line is `D` or later, or when the
line has no date and the ADR's number is above 0779 (the last number issued when this was written). Its text is cut
into blocks: paragraphs at blank lines, with every table row and every list item a block of its own. A block names a
source when it holds

* `Surname (year)`, `Surname et al. (year)`, `Surname and Surname (year)` or `Surname & Surname (year)`;
* `(Surname year)`, `(Surname, year)`, the same with `et al.` or a second surname;
* `arXiv:NNNN.NNNNN` (four digits, a point, four or five digits).

A surname is a capital letter and at least two more letters (accented letters, hyphen and apostrophe allowed) and is
none of: a month's name, Lead, Maintainer, Version, Addendum, Amendment, Table, Figure, Section, Equation, Chapter,
Appendix, Rule, Wave, Phase. A year is 1500 to 2099 with an optional letter. The source is the pair of the first
surname and the year, or the arXiv number. A block gives a place when it holds one of the words equation, eq., eqs.,
eqn., table, tab., page, p., pp., figure, fig., section, sec., chapter, appendix or the sign §, followed (after an
optional opening bracket) by a digit, a single capital letter or a Roman numeral; or the word DOI; or a DOI itself
(`10.` and four to nine digits and `/`). A block says the source was not read when it holds the phrase rule 4
prescribes for a citation that could not be read, or "not read", "not obtained", "not fetched", or "could not be read",
"could not be obtained", "could not be fetched". A source fails when no block that names it gives a place or says it
was not read. Why this form: rule 4 asks for "its place in it: an equation, table or page number, or a DOI"; figure,
section, chapter and appendix are added because values are read off figures and methods are described in sections, and
a check that refused them would make an agent leave the true place out (question 2). One place per source and ADR is
enough, in the same block, because a sentence cannot be cut reliably in wrapped text with abbreviations; a finding is
closed by a later addendum that names the source with its place.

**(3) The reading of a brief, section u.** Every file of `docs/adr/briefs` except `README.md` is read. Its entries are
the lines that start `- ` in the section `## Reading list`; the form is `- <path> <bytes>`, the path without a space,
the bytes a whole number, anything after them free (the brief README: "one path per line with its size in bytes"). The
stated total is the fifth value of the line `Size (lead):`, the values separated by `;` (the README). A brief fails when
it has no entry; when an entry has no size; when the stated total is not a whole number or is not the sum of the listed
sizes; when the sum is above 194023. With `--history` (the daily run, full history) it also fails when a listed size is
not the size the file has in the commit that first added the brief, or that commit holds no such file: this is "the sum
of its files" taken at the moment of issue, which later changes of the files do not disturb.

**(4) Test headers, section l.** The forms stay as section l has them (rule 12): a Rust test root file names an
existing `ADR NNNN` in its leading comment; a Studio spec names an existing item `F-NNN` or an existing `ADR NNNN`. New:
for the two lists of this section an absent file is an empty list (the rule started on 2026-11-18 and does not stop);
and each list that "only shrinks" has a registered largest number of entries in the script, above which the check
fails: `test-root-without-adr` 25, `studio-spec-without-record` 0, and the two lists of (3) below. When a list has
fewer entries than its number the report says so, so that the lead lowers the number.

**Exceptions lists (dated 2026-11-23, they only shrink).** `brief-over-reading-limit.txt`: the 4 briefs above the
limit, largest number 4. `brief-size-differs.txt`: the 6 entries of 4 briefs, as `<brief file>: <path>`, largest number
6. `test-root-without-adr.txt` shrinks to 25, each entry under a comment line with the reason;
`studio-spec-without-record.txt` shrinks to no entry and keeps its head. For (1) and (2) nothing is listed: the entry
and the ADR carry their own date, nothing dated before `D` is read, and no entry or ADR dated `D` or later exists
today, so the list would be empty; no file is made for them.

**Rule H, the headers.** Rust: the record of a file is the `primary_adr` that `scripts/adr_test_index.py` (default rule,
`--primary git`) gives its tests on main d10ea0bd, where every test of the file has the same one. The file gets, as
its new first line, `// ADR NNNN: the record of this file, the primary ADR of its tests by scripts/adr_test_index.py
(evidence <kind>).`, with the items of the ADR's file name after the number where it has any (`// ADR NNNN, F-MMM:
...`). A file whose tests have no primary ADR stays on the list with the reason: the index's finding, and the commit
that first added the file. Studio: the record is the first existing item `F-NNN` in the subject of the commit that first
added the file (`F-817b` names item F-817), else the first existing `ADR NNNN` there; the spec gets, as its new first
line, `// F-NNN: the item of this spec, by the commit that added it (<commit>, <date>).` or the same with `ADR NNNN:
the record`. A header is one added comment line; nothing else in the file changes. No other search for a record is
made: a record the index's rule does not give would be a guess written into the file as a fact.

**Other.** The docstring of the script names the new sections and the new lists. `--seed-exceptions` is unchanged
(it would overwrite the two lists of section l; it is the lead's, run once). No other script is changed.

## Table of checks

Clauses of the acceptance first. "Fails before" means: run against `scripts/process_report.py` of main d10ea0bd, the
test fails by an assertion on what `--check` printed or returned, not by a missing name.

| Id | Check | Limit | Source of the limit | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|
| T1 | acceptance (1), unit test `test_a_note_entry_from_the_date_is_one_line_in_the_form`: in the temporary repository `--check` passes with an entry `ADR 0002, 2026-11-24, done: ...`, with a lead's entry of 1200 characters of that date, and with an entry of 2026-11-23 of 3000 characters without an outcome; it fails, naming the item, on an agent's entry of 2026-11-24 without an outcome word, on one of 1201 characters, on a lead's entry of 1201 characters, on an entry with a line break, and on item F-888 whose note begins with no head | fails before, passes after | acceptance of F-861, clause 1; this registration for the date, the words and 1200 | fails before, passes after | n/a | code verification |
| T2 | acceptance (2), unit test `test_a_citation_from_the_date_names_a_place`: an ADR registered 2026-11-24 with "Menter (1994) gives the constants" fails naming Menter 1994; it passes with "Menter (1994), Table 2", with the place in a later addendum, and with "Menter (1994) was not read"; a bare `arXiv:2303.09262` fails and passes with "Table 1" in its block; "Lead (2026)" and "March (2023)" name no source; the same bare citation in an ADR registered 2026-11-23 passes; an ADR numbered 0780 without a date is read | fails before, passes after | acceptance of F-861, clause 2; agent rules, rule 4, for the place | fails before, passes after | n/a | code verification |
| T3 | acceptance (3), unit test `test_a_brief_reading_list_has_its_sum_and_its_limit`: a brief whose three entries sum to its stated total of 194023 passes; it fails with the total stated one byte off, with the sum 194024, with an entry without a size, with no entry, and with a total that is no number; a brief on `brief-over-reading-limit.txt` passes; with `--history` a listed size that is not the committed file's size fails and an entry on `brief-size-differs.txt` passes | fails before, passes after | acceptance of F-861, clause 3; the review's 90th percentile for 194023 (Arithmetic) | fails before, passes after | n/a | code verification |
| T4 | acceptance (4), unit test `test_a_test_file_without_its_record_fails_without_a_list_and_a_list_cannot_grow`: with no list file, a Rust test root without an ADR and a Studio spec without item or ADR each fail `--check`; a spec that names an existing item passes; `test-root-without-adr.txt` with 26 entries fails as above its number, with 25 not | fails before, passes after | acceptance of F-861, clause 4 and "may only shrink"; rule H for 25 | fails before, passes after | n/a | code verification |
| X | acceptance, "what exists before the date goes on a dated exceptions list": on the branch tip `python scripts/process_report.py --check` and `--check --history` name no failure of sections n, s, u, l; the lists hold 4, 6, 25 and 0 entries, each list dated in its head | 0 failures of these sections; 4, 6, 25, 0 | acceptance of F-861; Arithmetic and rule H | 0; 4, 6, 25, 0 | 1 | code verification |
| H | acceptance, the headers: 31 Rust files and 12 Studio specs get a header by rule H; the 25 others are listed with the reason | 31 and 12 with a header; 25 listed | acceptance of F-861; rule H on main d10ea0bd | 31, 12, 25 | 1 | code verification |
| U | acceptance, "no test's code changes and no default output changes": for every file under `crates/` or `apps/` in the diff of this work against main d10ea0bd, the diff is one added first line that is a `//` comment and no removed line (a script over `git diff`); the diff touches no file under a `src` folder, no golden and no hash table | 43 files, each one added comment line, 0 removed lines; 0 other files under `crates/` or `apps/` | acceptance of F-861 and agent rules, rule 3 | 43, 1, 0, 0 | 1 | unchanged default output |
| C | every changed Rust test target compiles: `cargo test --release --no-run -p <crate> --test <name>` for the 31; `rustfmt --check` of the 31 files | all compile; no difference | the brief, task 3; agent rules, rule 7 | all compile | n/a | unchanged default output |
| E | every existing test of `scripts/` passes, unedited: `python -m unittest discover -s scripts -p "test_*.py"` | all pass; no existing test edited | the brief, task 3 | all pass | n/a | unchanged default output |
| L | `cargo test -p rcfd-cli --release --test docs_site` (the links of this ADR), before its commit and at the tip | passes | the brief | passes | n/a | code verification |
| P | `python scripts/process_report.py --check` at the tip | 0 failures, except a generated file that is only not current (rule 7) | agent rules, rule 7 | 0 such failures | n/a | code verification |
| M | the mutations and the negative control below | each caught by the named row | agent rules, rule 16 | each caught | n/a | code verification |

## Negative controls and mutations

Negative control N: the changed script on the tree of main d10ea0bd (no new list, no header; a scratch copy) must fail
`--check` on exactly the 4 briefs above the limit and on the two lists of section l above their numbers (56 above 25, 12
above 0), and with `--history` also on the 6 sizes.

Mutations, each on a scratch copy of the changed script, the unit tests run against it:

| Id | Mutation | Must be caught by | Committed |
|---|---|---|---|
| M1 | (1): an entry is read when its date is after `D`, not on it | T1 | no |
| M2 | (1): the length is not compared | T1 | `test_process_report.convention.mutation.patch` |
| M3 | (1): the outcome word is not asked for | T1 | no |
| M4 | (2): every block counts as giving a place | T2 | no |
| M5 | (2): the ADR's date is not compared (every ADR is read) | T2 | no |
| M6 | (3): the stated total is not compared with the sum | T3 | no |
| M7 | (3): the limit is compared with "at least" in place of "above" | T3 | no |
| M8 | (4): an absent list means the rule has not started | T4 | no |
| M9 | (4): the largest number of a list is not compared | T4 | no |

## Size

Size (agent): done; 0; 0; 4; 180000

## Decision rule

Done if T1 to T4 fail before the change and pass after it, X, H and U hold with their numbers, and C, E, L, P and M
hold. Partial if any of T1 to T4 does not fail before or does not pass after, if `--check` at the tip names a failure of
sections n, s, u or l, if a number of X, H or U is missed (recorded as a miss with its number; the registered numbers of
the lists stay), or if a header changes anything but one added comment line. Todo if no check of the four is in the
script.

## What is not covered

* (1): text added to an old item's note without a head joins the entry before it and is read only if that entry is
  dated `D` or later. A sentence that happens to hold `ADR NNNN, <date>, ` inside it is read as a head. "One sentence"
  is held by the length only.
* (2): a source named in another form (authors with initials and a title, a report or standard number, `Surname year`
  without brackets) is not seen. The place is looked for in the block, not in the sentence. Whether the place is the
  right one is not checked. Addenda written from `D` on into an ADR registered before `D` are not read.
* (3): the listed sizes are compared with the files only with `--history`; the per-merge `--check` compares the sum,
  the stated total and the limit. The prose lines of further reading carry no size and count for nothing.
* (4): whether the ADR a header names is the right one is not checked. Rule 12's "its item as `F-NNN` where it has
  one" stays unchecked for Rust files (31 files name an ADR whose file name carries an item and do not name that item,
  27 of them no item at all; counted, not changed). The 25 files without a record stay without one.
* The weekly audit and the validation rule (the other two of the review's line) are not part of this item.
* `npm test` of `apps/studio-web` and clippy of whole crates are not planned: the change to their files is one comment
  line each; C compiles the Rust targets. If the Studio's packages are installed in the worktree its type check is run;
  otherwise it is named as not run.

## Open questions for the maintainer

The registered form of each is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation),
so that the checks can be built; each is a form the wording of a rule does not fix, and the answer may change a constant
of the script, not its design.

1. The length of a note entry. Options: (a) 1200 characters, the practice so far (registered); (b) 500, as for a `gap`,
   which 18 of the 25 entries so far would miss; (c) no length. Recommendation: (a) now, (b) when the entries are meant
   to shrink, said to the agents in rule 9 first. Whether the lead's own entries are held to it: registered yes.
2. The places a citation may give. Options: (a) rule 4's four (equation, table, page, DOI); (b) those and figure,
   section, chapter, appendix (registered); (c) as (b), and an arXiv number counts as a place as a DOI does.
   Recommendation: (b), and a word in rule 4 to match.
3. The reading limit. Options: (a) 194023 bytes, the 90th percentile of six days of briefs (registered); (b) the same
   percentile taken again after two weeks of data, as the review says; (c) a round figure the maintainer sets.
   Recommendation: (a) now and (b) on 2026-12-02, by changing one constant.
4. Rule 12's item for a Rust test file. Options: (a) leave unchecked (registered); (b) a further check with its own
   list of the 31 files that miss it today. Recommendation: (a) until the headers are used for something that needs the
   item.

## Addendum 1: results (2026-11-23)

Commits on the branch, after the registration 5f47774d: 4cf6b4f6 (the unit tests, alone, so that the history holds the
state in which they fail), 90f92513 (the 43 headers and the two lists of section l), ad37c250 (the script and the two
lists of section u), and the commit of this addendum with the committed mutation and the proposal file. Python checks
with the system's Python 3; cargo in the release profile with a target folder of its own under the scratch folder.

### Checks

| Id | Measured | Limit met |
|---|---|---|
| T1 | on the script of main d10ea0bd: fails by assertion (`(0, 0) != (1, 1)`: `--check` named no failure for an entry without an outcome word); on the changed script: passes | yes |
| T2 | before: fails by assertion (no failure named for "Menter (1994) gives the constants"); after: passes | yes |
| T3 | before: fails by assertion (no failure named for a total one byte off); after: passes. The half with `--history` is a test of its own, `test_a_listed_size_is_the_size_of_the_file_at_issue`: before, fails by assertion; after, passes | yes |
| T4 | before: fails by assertion (no failure named for a Rust test root without a record and without a list); after: passes | yes |
| X | at the tip, `--check`: no failure of sections n, s, u, l. `--check --history`: none of those sections either; 207 listed sizes compared, 6 differ, all listed. Lists: `brief-over-reading-limit.txt` 4, `brief-size-differs.txt` 6, `test-root-without-adr.txt` 25, `studio-spec-without-record.txt` 0, each dated in its head | yes: 0; 4, 6, 25, 0 |
| H | 31 Rust files with a header (evidence E0 7, E1 1, E2 23; nine of them name the item of the ADR's file name), 12 Studio specs (ten by an item, two by ADR 0801); 25 Rust files listed, each under a line with the reason | yes: 31, 12, 25 |
| U | `check_u.py` on the tip against d10ea0bd: 43 files under `crates/` or `apps/` (31 Rust, 12 Studio), each the old file with one `//` line put first, 0 otherwise; no file under a `src` folder, no golden, no hash table. The other files of the diff: this ADR, the four lists, the script, its test file, the mutation patch, the proposal file | yes: 43, 1, 0, 0 |
| C | `cargo test --release --no-run` for the 31 targets in seven crates: 31 executables, every exit 0, no warning; `rustfmt --check --edition 2021` of the 31 files: no difference | yes |
| E | `python -m unittest discover -s scripts -p "test_*.py"`: 382 tests, OK (5 skipped, as on main); `scripts/test_process_report.py` 44 tests, the 39 existing ones unedited | yes |
| L | `docs_site` before the registration's commit: 2 passed; at the tip: see "Closing checks" | yes |
| P | see "Closing checks" | see below |
| M | negative control as predicted; 9 of 9 mutations caught (below) | yes |

### Negative control and mutations

N: the changed script with the lists and files as on main d10ea0bd gave, with `--check`, exactly: the two lists of section
l above their numbers (56 above 25, 12 above 0) and the 4 briefs above the limit (473792, 280627, 214687, 346865); with
`--history` also the 6 sizes (5 that differ, 1 file the commit does not hold). As predicted.

Mutations on a scratch copy of `scripts/` (`mutate.py`), the five tests of the class `Convention` run against each; the
unmutated copy passes; no other test of the file fails under any of them:

| Id | Caught by |
|---|---|
| M1 | T1 |
| M2 | T1 (committed: `scripts/test_process_report.convention.mutation.patch`; applied with `git apply`, `python scripts/test_process_report.py` fails 1 of 44, removed with `git apply -R`) |
| M3 | T1 |
| M4 | T2 |
| M5 | T2 |
| M6 | T3 |
| M7 | T3 |
| M8 | T4 |
| M9 | T4 |

### Deviations and things to know

* T3 is two test functions, not one: the half that needs a commit history is separate. Five tests were written for four
  rows; all five fail before the change.
* The place of a citation, as built: the words of the registration, their plurals, the short forms only with their
  point (`eq.`, `fig.`, `sec.`, `tab.`, `p.`, `pp.`), then a digit or a capital letter that does not begin a lower-case
  word. That takes "Table A1", "equation (A1)" and "Table IV" and leaves "Table Of" out; a Roman numeral in lower case
  is not taken. A heading line is a block of its own, as a list item is.
* The checks of sections n and s find nothing today: no entry and no ADR is dated 2026-11-24 or later. They first act on
  what is written from that day. An agent briefed before this merge has not been told of the 1200 characters or of the
  outcome word; a proposal of such an agent that the lead applies from 2026-11-24 can fail `--check` on main, and is then
  shortened by the lead in the note (the ADR keeps the long text).
* The lead's own entries are held to 1200 characters from that day (two of the 108 since 2026-11-18 were longer).
* `--check --history` names 16 failures of section c ("registration after code" 14, "not committed alone" 2) on this
  branch; the unchanged script of main names the same 16 on the same tree. They are not of this work and not touched.
* `LIST_MAX` in the script holds 25, 0, 4 and 6. When a list shrinks the report says so and the lead lowers the number;
  until then the list could grow back to it.
* The headers change what the test index reads: the first `ADR NNNN` of a leading comment is its evidence E0, so the
  31 files now have that ADR by E0 where 24 had it by E1 or E2. The ADR is the same one in every file.
* `--seed-exceptions` would overwrite `test-root-without-adr.txt` and lose the reasons; it was not run and should not
  be for these lists.

### What was not run

`npm test` and the type check of `apps/studio-web` (its packages are not installed in the worktree; the 12 specs each
gained one comment line). Clippy. No Rust test was executed: the 31 targets were compiled. No whole-crate or workspace
run. No solve.

### Size

Size (agent, actual): done; 0; 0; 4; 200000

### Decision

**Done** by the registered rule: T1 to T4 fail before the change and pass after it; X, H and U hold with their numbers
(0 failures; 4, 6, 25, 0; 31, 12, 25; 43, 1, 0, 0); C, E, L, P and M hold.

### Open questions for the maintainer

The four of the registration stand as written there, each with its options and recommendation: (1) the length of a note
entry (1200 registered; 500 as for a `gap`; none) and whether the lead's entries are held to it; (2) the places a
citation may give (rule 4's four; with figure, section, chapter, appendix, registered; an arXiv number as a place);
(3) the reading limit (194023 from six days of briefs, registered; the percentile again after two weeks; a round
figure); (4) rule 12's item for a Rust test file (unchecked, registered; a further check with a list of 31). An answer
changes a constant at the head of `scripts/process_report.py`.

### For the lead: roadmap entries

Status proposed for F-861: done.

> ADR 0777, 2026-11-23, done: `process_report.py --check` now fails, each shown by a unit test that fails on the script of main d10ea0bd, on a note entry dated 2026-11-24 or later that lacks the outcome word, exceeds 1200 characters or holds a line break (section n), on a source named as Surname (year) or by its arXiv number without a place in an ADR registered from that day (section s), on a brief whose stated total is not the sum of its reading list or whose list exceeds 194023 bytes, the 90th percentile of the 41 briefs so far, and with --history on a listed size that is not the file's size at issue (section u), and on a test file without its record even with no list file, the lists being held to 25, 0, 4 and 6 entries; 31 Rust files and 12 Studio specs got their record as one comment line, 25 Rust files stay listed with the reason; 9 of 9 mutations caught; four forms are open for the maintainer.

No follow-up item is proposed. The same entries are in `docs/roadmap/proposals/0777-F-861.json`.

### Closing checks

On the tree of this addendum's commit: `cargo test -p rcfd-cli --release --test docs_site`, 2 passed;
`python scripts/roadmap_proposals.py check docs/roadmap/proposals/0777-F-861.json`, exit 0 ("F-861: todo -> done; note +
911 characters"); `python scripts/process_report.py --check`, 1 failure, the generated file `docs/adr/index.md` that is
only not current (it lacks the row of this ADR) and is left to the lead by rule 7. No other failure.

## The lead's reading (2026-11-23), at the merge

The first merge (4e3922cd) failed `npm test` in each of three push runs on Studio tests outside this item (this item
adds one header line to each of 12 Studio test files and changes nothing they run), and in a fourth run after the
leftover `rcfd serve` processes were ended. It was parked until the fixes of F-890 to F-892 were on main, merged again
at f8e433bb and pushed at 1e6dcaa0 (the report read at 03:58 UTC, 8 minutes of reading, proposed done, written done);
`npm test` passed all 873 tests in the first run. 734 binaries were left out for a left-out run.

Status: **done**, as proposed. The four open questions were taken on their recommendations under the maintainer's
standing instruction of 2026-11-11, as the ADR says; each changes a constant of the script, not its design.
