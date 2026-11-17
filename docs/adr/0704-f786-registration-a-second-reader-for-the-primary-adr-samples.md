# ADR 0704: F-786 registration: a second reader for the primary-ADR samples of the test-to-ADR attribution

Status: **REGISTERED 2026-11-17, written before any reading of F-786 and before the list of the 28 files was obtained.** Written by
the reading agent on branch `f786-work` (from 55831919, which holds F-772 / ADR 0701), worktree `w49i-work/f786`, scratch
`w49i-work/f786-scratch`. Not yet accepted by the maintainer. Every choice below that the maintainer would normally be asked about
is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation) and is marked "(standing instruction)".

Roadmap item F-786 (deps F-772), a follow-up proposed by ADR 0701. No code is written; this ADR is the only file changed.

House rules (`w49i-work/AGENT_RULES.md`) apply: limits fixed here and not changed after the reading, the reading made once, existing
tests not edited, no golden or hash table edit, clean-room. ADRs 0684, 0690 and 0701 and every script are not edited.

## What F-786 asks

> Every limit not stated here is registered in the ADR before the run. The 28 files with a primary read in ADR 0701, with the five borderline readings (`cylinder.rs`, `run_control.rs`, `species.rs`, `properties.rs`, `cavity.rs`), are read by a reader who did not write the rule, against a written criterion registered before the reading, and agree with the first reading on at least 85 % of the files; the disagreements are listed with a reason; no limit of ADR 0684, 0690 or 0701 is changed by the second reading.

## What was read beforehand (disclosed)

`AGENT_RULES.md`; the brief; the `--help` of `scripts/adr_test_index.py`; the headings of ADR 0701 (by `grep '^#'`) and its
section "Samples" (lines 90 to 98: S45 is the 45 files of ADR 0690 in `f768-scratch/samples.json`, S25 is 25 files drawn with seed
7720); the status paragraph and "What was read" of ADR 0698 and the "For the lead" section of ADR 0572, for the form; the
roadmap entries of F-785 and F-786; the names of the files in `f768-scratch` and `f772-scratch` (not their contents).

Not read: the "Design", "Checks" and "Addendum 1" sections of ADR 0701, ADRs 0684 and 0690, the script's source,
`f772-scratch/reading.txt`, `f772-scratch/addendum.md`.

Two things I did see that a blind reader would not, and which I cannot unsee:

1. While looking for the roadmap entry of F-786, `grep` printed the note of F-772. It gives the totals of the first reading (of the 45
   files 12 have a primary and 9 agree; of the fresh 25, 16 have a primary and 15 agree; together 24 of 28) and by rule ("H0, H1 and H2
   read 22 of 22, H3 1 of 5 (audit and test-list rows)"), and names the rules: H0 an ADR path in a file header, H1 a unit under a
   heading `## Test files (added <date>, ADR dddd)` (the notes of ADR 0685), H2 a unit that says the file had no ADR, H3 a table row
   with the file and a limit. So I know that the first reader disagreed with the assigned primary on 4 files, and that these are among
   the H3 files. I do not know which files.
2. The acceptance and the brief name five files whose first reading was borderline.

To limit the effect of item 1, I do not look up which rule (H0 to H3) gave a file its primary until my readings are committed; I
judge each file from the test file, the ADR text and the git history only.

No arithmetic beforehand except the limit below.

## The criterion (registered now)

**Question asked of each file.** The index assigns a test file one primary ADR. The assignment is *right* when that ADR is the one
a maintainer should open first when a test of the file fails: the ADR that decided what the file checks and states the limits the
file asserts.

**Evidence that counts**, each read by me from the tree at 55831919:

- (a) *The test file itself*: its leading comment and the comments on its tests. An ADR named there as the origin of the file
  ("ADR dddd", "registered in", a `docs/adr/dddd-...` path) counts; an ADR named only as background for one constant or one test
  does not make it the primary of the file.
- (b) *The text of each ADR that cites the file* (found by `grep` of the file's base name under `docs/adr`). An ADR *registers* the
  file when it states, for that file by name, what its tests check and the limits or expected values, either before the tests were
  written (a registration, a checks table) or afterwards with the stated purpose of supplying the missing registration (a
  retro-registration that says the file had no ADR). An ADR only *mentions* the file when it lists it in an inventory, an audit, a
  list of tests run or re-run, a list of files changed, or as a neighbour or precedent.
- (c) *The git history*: the commit that added the file (`git log --diff-filter=A --follow`), its message, and whether an ADR was
  added in the same commit or named in the message.

**Verdict.** For each file one of two, with a one-line reason:

- **agree**: the assigned ADR registers the file in the sense of (b), or is named by the file as its origin (a), and no other ADR
  has a clearly better claim under the rules for several ADRs below.
- **disagree**: the assigned ADR only mentions the file; or another ADR has a clearly better claim; or the assigned ADR is about a
  different subject from what the tests check. Where I can name the right ADR I name it; "none" is a permitted answer (the file
  should have no primary).

**A file that several ADRs describe.**

1. The ADR for which the file was first written wins (the file names it, or the commit that added the file names it or adds it).
   Later ADRs that add tests to the file, re-run it or tighten a limit are secondary, even if they describe it at more length.
2. If the file was written with no ADR, a later ADR that registers it in the sense of (b) is right. Among several such, the earliest
   one that states the limits wins; a later one wins only if it says it replaces the earlier registration.
3. If the file was written with no ADR and a later ADR decided the behaviour and added tests to the file for that decision, while
   another ADR retro-registers the whole file, the retro-registration is right when it covers the majority of the file's tests;
   otherwise the ADR that covers the most tests.
4. ADRs about the attribution itself (0653, 0679, 0684, 0690, 0701, this ADR) and pure inventories are never the right primary.
5. A doubt that these rules do not settle is resolved in favour of the assignment (verdict agree) and the row is marked
   "borderline" with the doubt stated. Borderline rows count by their verdict; their number is reported.

**What I read per file** (the same for all 28, fixed now so that effort does not follow suspicion): the first 80 lines of the test
file and the names of its test functions; the commit that added it; for every ADR that cites the base name, the citing lines with
five lines of context each side, and the title and status line of that ADR; more of an ADR only where those lines do not show
whether it registers or mentions.

## Procedure and blindness

1. This file is committed alone.
2. The list: `python scripts/adr_test_index.py --primary git` in the worktree gives each file's primary. The sample membership
   (S45, S25) comes from `f768-scratch/samples.json` and `f772-scratch/samples.json`, from which only file paths are extracted by a
   short script that prints paths and nothing else; if a samples file turns out to hold readings, they are not printed. The 28 are
   the files of S45 and S25 with a non-null primary. `reading.txt` and `addendum.md` of `f772-scratch` and ADR 0701 from line 99
   on stay closed.
3. The 28 readings are written as a table in "Addendum 1" of this ADR (file, assigned primary, verdict, right ADR if different,
   reason) and committed.
4. Only then is the first reading opened (ADR 0701 addendum 1; `f772-scratch/reading.txt` if the addendum lacks a per-file verdict),
   the comparison written as "Addendum 2" and committed. Addendum 1 is not edited after step 4; a second thought about a row goes
   into Addendum 2 as a remark and does not change the count.

Blindness rests on this order and on my statement; the commit order records it, it does not prove it.

## Checks (limits fixed now)

| # | Check | Limit | Estimate behind it |
|---|---|---|---|
| C1 | Number of files of S45 and S25 with a non-null primary under `--primary git` | exactly 28 (12 of S45, 16 of S25) | the note of F-772 |
| C2 | Files read against the criterion, each with a verdict and a reason | 28 of 28 | the acceptance |
| C3 | **Agreement of the two readings**: files on which my verdict (agree / disagree with the assigned primary) equals the first reader's counted verdict, over 28 | at least 85 %, i.e. at least 24 of 28 (23 of 28 is 82.1 %) | the acceptance; I have no estimate of my own |
| C4 | Every file on which the readings differ listed with both verdicts and a reason | all | the acceptance |
| C5 | `git diff 55831919 -- docs/adr/0684-* docs/adr/0690-* docs/adr/0701-* scripts/` | empty | the acceptance: no limit changed |
| C6 | `cargo test -q --release -p rcfd-cli --test docs_site` | passes | docs touched |

How C3 is counted, in full:

- The unit is the file; the denominator is 28 whatever happens. If C1 gives another number, the files found are read, the number is
  reported, C1 is a miss and the denominator is the number found.
- The first reader's verdict for a file is the one it was *counted* as in ADR 0701 (a borderline reading counted as agreeing is
  "agree"). If a file's first verdict cannot be determined from ADR 0701 or `reading.txt`, the file counts as differing.
- Two "disagree" verdicts count as agreeing readings even if the two readers name different right ADRs; such files are listed
  separately.
- The five borderline files of the acceptance are in the 28 and count like the others; their five rows are also reported as a
  group. If one of the five is not among the 28, that is reported and it is read in addition, outside the count.

Reported next to C3, without a limit: my rate of agreement with the assigned primary (the first reader's was 24 of 28), the number
of my borderline rows, and Cohen's kappa of the two readings (with 4 disagreeing verdicts of 28 on one side, raw agreement can be
high by chance: two readers who each say "agree" on 24 random files agree on about 76 % of them).

## Negative controls and planned mutations

No code, so no mutations. One control on the reader: the criterion must be able to say "disagree". If my 28 verdicts are all
"agree", that is reported as a sign that the criterion or the reading did not discriminate, whatever C3 gives (all-agree would give
24 of 28 and meet the limit without any discrimination).

## Decision rule

- **done**: C1 to C6 met.
- **partial**: C2, C4, C5, C6 met and C3 or C1 missed; or all met but all 28 of my verdicts are "agree".
- **todo**: the reading was not made, or was not blind in a way beyond the two disclosures above.

A miss of C3 is a finding about the first reading or about the criterion, not a defect to repair: no row is re-read to change it.

## Limits (what is not covered)

- One second reader, an agent of the same kind as the first; not a person, and not independent of the first reader's training.
- The disclosures above: I know the first reader's totals and that its disagreements are H3 files.
- The files without a primary (17 of the 45 and 9 of the 25, by the note of F-772) are not read: whether a file *should* have had a
  primary is not asked.
- My criterion is not the first reader's criterion (which I have not read); a difference of verdicts may come from the criterion
  and not from the file. Addendum 2 says which, where I can tell.
- No limit of ADR 0684, 0690 or 0701 is changed, whatever is found; a proposal goes to the lead as a follow-up.
