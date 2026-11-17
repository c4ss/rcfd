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

## Addendum 1: the second reading (2026-11-17, written and committed before the first reading was opened)

Registration commit: befc895d. At the time of writing I have not opened ADR 0701 from line 99 on, `f772-scratch/reading.txt`,
`f772-scratch/addendum.md`, ADR 0684 or ADR 0690.

### The list (C1)

`python scripts/adr_test_index.py --primary git --out f786-scratch/idx.json` at befc895d (28.9 s). Sample membership from the keys
`S45` and `S25` of `f772-scratch/samples.json` (lists of paths only; `S45` equals `D` + `E` of `f768-scratch/samples.json`). Files
with a non-null primary: **12 of S45 and 16 of S25, 28 in all**: C1 met. Every test of each of the 28 files has the same primary
and none has a null one; the primaries are identical to those in `f772-scratch/idx_new_git.json` (so this ADR, which names five of
the files, changed none). The five borderline files of the acceptance are all among the 28.

### Deviations and further disclosures

1. **A third reading seen.** The evidence script printed, for some files, lines 131 to 143 of ADR 0685 (addendum 1 of F-760): its
   author's reading of an 8-file sample, with verdicts for `interchange.rs` (agree), `species.rs` and `full_loop.rs` ("agree, weak"),
   `non_conformal_case.rs` (agree), `workflow_engine.rs` (disagree, not in my 28), and a sentence that calls the notes for
   `properties.rs`, `data.rs`, `datapack.rs` and the assistant's `workflow.rs` weak. That is not the first reading of ADR 0701, but
   it is an earlier opinion on 8 of my rows, and my verdicts on them cannot be shown to be independent of it. After seeing it I
   left ADR 0685's addendum out of what I printed. ADR 0685 was not on my list of closed files; it should have been.
2. **What was read per file.** The script extracted the first 80 lines of each file; I read the leading comment block (to the
   first `use`) and the test names, not all 80 lines. For `pimple.rs` (a source file cited by 68 ADRs) I read the test names, the
   doc comments of the two newest tests, the ADR numbers the file names, and the citing lines of the assigned ADR 0245 only.
3. **An interpretation made while reading**, not in the registration: criterion (b) asks for "the limits or expected values". The
   "Test files" notes that ADR 0685 appended to older ADRs say what a file checks, without numbers; the numbers are in the file
   header ("checks fixed before running"). I counted such a note as registering when it says the file had no ADR and the host
   ADR's own decisions are about the feature the file tests (the "different subject" clause of the verdict decides the rest).
   Rows that depend on this are marked "note".
4. Evidence files: `f786-scratch/ev/*.txt`, script `f786-scratch/gather.py`. ADRs 0653, 0679, 0684, 0690, 0701 and 0704 were
   excluded from the search for citing ADRs.

### The 28 readings

"Right" is the ADR I would assign; "none" means the file should have no primary. B = borderline (counted by its verdict).

| # | Sample | File | Assigned | Verdict | Right | Reason |
|---|---|---|---|---|---|---|
| 1 | S45 | `crates/rcfd-claude/src/discover.rs` | 0126 | agree | | Added by 975f340d "session discovery ... (ADR 0126)"; 0126 is the bridge library the module belongs to. One later test names ADR 0128 (finding 17); the file was written for 0126. |
| 2 | S45 | `crates/rcfd-fv/src/pimple.rs` | 0245 | **disagree** | none (0664, 0667 for one test each) | 0245 (BDF2 step remainder) read the file, added 20 lines and re-ran the crate's unit tests as a defaults check (B1). The 7 unit tests are five from the first import (Taylor-Green decay and order, PIMPLE, PISO conservation) and two whose doc comments name ADR 0664 D1 and ADR 0667 U1. 0245 registers none of them. |
| 3 | S45 | `crates/rcfd-fv/tests/cylinder.rs` | 0060 | **disagree** | none | First import, F-029, no ADR. 0060 cites it in "Audit of what was validated before this wave" (one row: reference, one mesh, 5 %, recorded St) and as the model of the far field for its own `validation_cylinder_re100.rs`. An audit row and a precedent: a mention. 0060's own cylinder checks are in another file. |
| 4 | S45 | `crates/rcfd-cli/tests/run_control.rs` | 0123 | **disagree** | none | Added by 643caf54 (F-120, F-121) with no ADR; criteria in the header. 0123 (the run service `rcfd-runner`) cites it once, in the estimate cell of its check S6, as the reason a bitwise limit can be registered. A precedent, not a registration; no "Test files" note names this file. |
| 5 | S45 | `crates/rcfd-api/tests/mesh_params.rs` | 0685 | **disagree** | none | 0685 is the ADR that added ADR text for unmapped binaries of the index. It names this file only to say that no ADR describes the generator parameters, that it "is not guessed", and in a proposed follow-up that asks for a registration ADR to be written. It declines to register the file. |
| 6 | S45 | `crates/rcfd-api/tests/mrf.rs` | 0008 | agree | | Header: "the checks M1-M7 of docs/adr/0008-mrf.md ... fixed in that ADR before any MRF code ran". Later ADRs (0045, 0050, 0077, 0170, 0183) only re-run it. |
| 7 | S45 | `crates/rcfd-api/tests/non_conformal_case.rs` | 0024 | agree (B, note) | | F-114 commit e79f7db0, no ADR. The note in 0024 says so and what the file checks; 0024's section C registers checks of the same generator (`channel_2d_interface`, C2). Doubt: 0024 is the conjugate-heat ADR and "uses" F-114; C2 is a composite wall, the file a Poiseuille mass balance. |
| 8 | S45 | `crates/rcfd-turb/tests/channel_wall_function.rs` | 0685 | **disagree** | none | As row 5: 0685 says "ADR 0014 and 0015 describe the turbulence models, not this test" and proposes that an ADR be written. |
| 9 | S45 | `crates/rcfd-assistant/tests/physics_workflow.rs` | 0685 | **disagree** | none | As row 5: "no ADR describes them". |
| 10 | S45 | `crates/rcfd-cli/tests/physics_cases.rs` | 0685 | **disagree** | none | As row 5: "ADR 0005 only lists the cases in a hash list". |
| 11 | S45 | `crates/rcfd-cli/tests/data.rs` | 0060 | agree (note) | | F-125 commit f67e3c16, no ADR. 0060 carries F-125 in its title and has a section "Data pack" that names this test binary; the note says what the file checks. |
| 12 | S45 | `crates/rcfd-fv/tests/cavity.rs` | 0060 | **disagree** | none | As row 3: first import, F-028; 0060 has it in the audit of earlier validation and as the model of the interpolation for its own `validation_cavity_ghia.rs`. 0047, 0096 and 0242 use its set-up as a precedent. |
| 13 | S25 | `crates/rcfd-assistant/tests/workflow.rs` | 0067 | **disagree** | none | First import, F-148. 0067 is the design loop; its only link is one line "tool layer: F-090, F-147, F-148". The note itself says "No earlier ADR describes it; this ADR only names the item". A different subject. |
| 14 | S25 | `crates/rcfd-api/tests/properties.rs` | 0036 | agree (B, note) | | F-053 commit e75d6470, no ADR. 0036 (tabulated properties) registers P1 and P4 on this file's two cases, describing their set-up, and P6 "passes unmodified"; same feature (property laws). Doubt: the file's own limits (1 % of U, the Kirchhoff transform) are in its header, not in 0036. |
| 15 | S25 | `crates/rcfd-api/tests/udf.rs` | 0088 | agree (note) | | F-122 commit f05fd7dc, no ADR; 0088 (hook catalogue v2 of the same mechanism) calls F-122 "ADR-less", takes its 1e-9 limit from it and registers "the F-122 tests `tests/udf.rs` pass unchanged". |
| 16 | S25 | `crates/rcfd-cli/tests/interpolate.rs` | 0031 | agree (note) | | F-115 commit b22ae2b4, no ADR. 0031 extends F-115, keeps the `--interpolate-*` flags the file tests and re-registers limits "from the first F-115 slice". 0099 concerns the file of the same name in `rcfd-api`. |
| 17 | S25 | `crates/rcfd-assistant/tests/keychain.rs` | 0004 | agree | | 0004 (added the day before the file) decides exactly what the 7 tests check: environment, keychain, `keys.json`; read-back of a write; keys scrubbed from error text; an in-memory backend in tests. 0025 only audits it. |
| 18 | S25 | `crates/rcfd-assistant/tests/full_loop.rs` | 0004 | agree (note) | | First import, F-147. 0004 decides that tools run in process, that the approval card is the hook and that "the gate is enforced below the UI", which the tests on approval, denial, read-only mode and the gate check; no other ADR cites the file. |
| 19 | S25 | `crates/rcfd-fv/tests/compressible_bc.rs` | 0006 | agree | | Header: "checks registered in docs/adr/0006-compressible-boundary-conditions.md before any run"; test names A5, B, C follow the ADR. |
| 20 | S25 | `crates/rcfd-fv/tests/non_conformal.rs` | 0024 | agree (B, note) | | As row 7. Doubt, stated by the note itself: 0024 "describes the join and a conduction check, not the Poiseuille check, which is registered in the file's own header"; one of the two tests (linear temperature across the interface) is 0024's subject. |
| 21 | S25 | `crates/rcfd-api/tests/fan_sector.rs` | 0065 | agree | | Header: checks M3, S1 to S4 of ADR 0065, "registered in the ADR before any of this code ran"; added by the wave 32A commit. |
| 22 | S25 | `crates/rcfd-fv/tests/step_laminar.rs` | 0060 | **disagree** | none | As rows 3 and 12: first import (F-106, F-042), assertions registered in commit 97fd3e4c with no ADR; 0060 has it in the audit table and as a precedent for `validation_bfs_step.rs`; 0023 P4 borrows its set-up. |
| 23 | S25 | `crates/rcfd-fv/tests/species.rs` | 0027 | agree (B, note) | | F-053 commit e75d6470, no ADR. 0027 adds chemistry to the species transport of F-053 and describes how the species fields are carried (D3). Doubt: 0027 decides nothing about the mixing-layer check; it is the nearest ADR, not the deciding one. |
| 24 | S25 | `crates/rcfd-api/tests/workflow.rs` | 0123 | **disagree** | none | 643caf54 (F-120, F-121), no ADR. 0123 is the run service; it names F-120 and F-121 as parents. Four of the five tests are the task-based workflow engine, which 0123 does not describe; one is the headless run contract the service rests on. A different subject for the majority of the file. |
| 25 | S25 | `crates/rcfd-post/tests/gltf.rs` | 0011 | agree (note) | | F-117 commit 49735b69, no ADR. 0011 carries F-117 in its title; its section 4 decides the GLB structural check (`check_glb`) that the file's validity and corruption tests exercise. |
| 26 | S25 | `crates/rcfd-io/tests/interchange.rs` | 0011 | agree | | Header: "checks registered in docs/adr/0011-formats-and-registers.md before this file was run"; section 2 of 0011. |
| 27 | S25 | `crates/rcfd-io/tests/datapack.rs` | 0060 | agree (note) | | As row 11. |
| 28 | S25 | `crates/rcfd-api/tests/fan_cgrid_case.rs` | 0068 | agree | | Header: checks Q6(d), R1, S1' and S3' of ADR 0068, registered before the code ran; the only citing ADR. |

### Counts of the second reading alone

- Read: 28 of 28, each with a verdict and a reason: C2 met.
- **Agree with the assigned primary: 17 of 28 (60.7 %); disagree: 11.** S45: 4 of 12 agree. S25: 13 of 16 agree.
- Borderline rows: 4 (rows 7, 14, 20, 23), all counted "agree" by rule 5 of the criterion. Without them, 13 clear agreements.
- The control on the reader: the criterion did say "disagree", 11 times.
- The 11 disagreements fall in four groups: the assigned ADR is 0685 naming a file as undescribed (rows 5, 8, 9, 10); an audit
  row or precedent in ADR 0060 (rows 3, 12, 22); a precedent or a re-run in an ADR on another subject (rows 2, 4); a "Test files"
  note hosted by an ADR that only names the item (rows 13, 24).
- The five borderline files of the acceptance: `cylinder.rs` disagree, `run_control.rs` disagree, `cavity.rs` disagree,
  `species.rs` agree (B), `properties.rs` agree (B).

The note of F-772 (disclosed in the registration) says the first reader counted 24 of 28 as agreeing; I count 17. The two readings
therefore differ on at least 7 files and C3 can be at most 21 of 28 (75 %): a miss of C3 is certain before the comparison. No row
above is changed for that.
