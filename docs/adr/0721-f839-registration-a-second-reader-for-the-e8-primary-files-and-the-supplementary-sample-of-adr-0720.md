# ADR 0721: F-839 registration: a second reader for the E8-primary files and the supplementary sample of ADR 0720

Status: **REGISTERED 2026-11-17, written before any reading of F-839 and before the list of the 22 files was obtained.** Written by
the reading agent on branch `f839-work` (from 1b26b8a9, which holds F-773 / ADR 0720), worktree `w49i-work/f839`, scratch
`w49i-work/f839-scratch`. Not yet accepted by the maintainer. Every choice below that the maintainer would normally be asked about
is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation) and is marked "(standing instruction)".

Roadmap item F-839 (deps F-773), a follow-up proposed by
[ADR 0720](0720-f773-registration-a-git-commit-that-names-an-adr-or-an-item-as-evidence-of-the-files-it-adds.md). Number 0721 given
by the lead. No code is written; this ADR is the only file changed.

House rules (`w49i-work/AGENT_RULES.md`) apply: limits fixed here and not changed after the reading, the reading made once, existing
tests not edited, no golden or hash table edit, clean-room. ADR 0720, ADR 0704 and every script are not edited.

## What F-839 asks

> Every limit not stated here is registered in the ADR before the reading. The 2 files with an E8 primary and the 20 files of the supplementary sample of ADR 0720 (`random.Random(7730)`, listed in its addendum) are read by a reader who did not write the rule, against the criterion of ADR 0704 registered before the reading, and agree with the first reading on at least 85 % of the files; the disagreements are listed with a reason; no limit of ADR 0720 is changed by the second reading.

## What was read beforehand (disclosed)

`AGENT_RULES.md`; the brief; the `--help` of `scripts/adr_test_index.py`; ADR 0704 lines 1 to 152 (its registration: the criterion,
the procedure, the checks, the decision rule, the limits), not its addenda; ADR 0720 lines 1 to 144 (its registration: what F-773
asks, the count beforehand, the design of rule E8, the checks E1 to E11, the mutations, the decision rule, the limits), not its
addendum; the headings of both ADRs (by `grep '^#'`); the "For the lead" section of ADR 0572, for the form; the roadmap entry of
F-839; the names of the files in `f773-scratch` (not their contents); `git log --oneline -8`.

Not read: ADR 0720 from line 145 on (addendum 1: the readings of E7 and E8, "What the numbers say", the remaining questions, the
roadmap entries), `f773-scratch/addendum.md`, `samples_e8.json`, `sup_list.txt`, `e8_files.json` and every other file of that
folder, the script's source, the tests of the script.

Things I did see that a blind reader would not, and which I cannot unsee:

1. The registration of ADR 0720 names the two E8-primary files and their ADRs (`rcfd-claude/src/discover.rs`, ADR 0126;
   `rcfd-project/src/time.rs`, ADR 0122) and states, as the estimate behind check E7, what its author expected: "0126 is cited by
   the commit and registers the session discovery; 0122 is the project folder library and `time.rs` is one of its modules, which ADR
   0690 read as `disagree-null` because 0122 does not cite it by name". This is an expectation written before the first reading, not
   the reading, but it tells me which way the first reader leaned on both files.
2. The same registration states the author's expectation for the supplementary sample ("header ADRs are written by the author of
   the file, so I expect E0 right in most of them") and the decision aid (E8 right and E0 not in at least 16 of 20 would support
   moving E8 above E0).
3. The subject of commit 1b26b8a9 and the roadmap say F-773 is partial. By the registration of ADR 0720 that status follows by
   construction from the population of 2 files, so it says nothing about the readings.
4. While looking for the roadmap entry of F-839, `grep` printed the last three lines of the note of F-773 (the names of the
   follow-ups F-839 and F-840 and "open: F-787, F-793"). The title of F-840 ("Items named in a commit subject whose ADRs are named
   only in the roadmap note as evidence of the files the commit adds") hints that the first reading met files whose commit names
   an item with no ADR in the subject. No count and no verdict was printed.

I do not know any verdict of the first reading, nor its totals.

No arithmetic beforehand except the limit below.

## The criterion (ADR 0704, adopted by quotation)

The criterion is that of ADR 0704, section "The criterion (registered now)", quoted here; where it says "the tree at 55831919" I read
the tree at 1b26b8a9, and its rule 4 is extended by the ADRs about the attribution written since (0703, 0712, 0714, 0720, this ADR).

> **Question asked of each file.** The index assigns a test file one primary ADR. The assignment is *right* when that ADR is the one a maintainer should open first when a test of the file fails: the ADR that decided what the file checks and states the limits the file asserts.

> **Evidence that counts**, each read by me from the tree [...]:
>
> - (a) *The test file itself*: its leading comment and the comments on its tests. An ADR named there as the origin of the file ("ADR dddd", "registered in", a `docs/adr/dddd-...` path) counts; an ADR named only as background for one constant or one test does not make it the primary of the file.
> - (b) *The text of each ADR that cites the file* (found by `grep` of the file's base name under `docs/adr`). An ADR *registers* the file when it states, for that file by name, what its tests check and the limits or expected values, either before the tests were written (a registration, a checks table) or afterwards with the stated purpose of supplying the missing registration (a retro-registration that says the file had no ADR). An ADR only *mentions* the file when it lists it in an inventory, an audit, a list of tests run or re-run, a list of files changed, or as a neighbour or precedent.
> - (c) *The git history*: the commit that added the file (`git log --diff-filter=A --follow`), its message, and whether an ADR was added in the same commit or named in the message.

> **Verdict.** For each file one of two, with a one-line reason:
>
> - **agree**: the assigned ADR registers the file in the sense of (b), or is named by the file as its origin (a), and no other ADR has a clearly better claim under the rules for several ADRs below.
> - **disagree**: the assigned ADR only mentions the file; or another ADR has a clearly better claim; or the assigned ADR is about a different subject from what the tests check. Where I can name the right ADR I name it; "none" is a permitted answer (the file should have no primary).

> **A file that several ADRs describe.**
>
> 1. The ADR for which the file was first written wins (the file names it, or the commit that added the file names it or adds it). Later ADRs that add tests to the file, re-run it or tighten a limit are secondary, even if they describe it at more length.
> 2. If the file was written with no ADR, a later ADR that registers it in the sense of (b) is right. Among several such, the earliest one that states the limits wins; a later one wins only if it says it replaces the earlier registration.
> 3. If the file was written with no ADR and a later ADR decided the behaviour and added tests to the file for that decision, while another ADR retro-registers the whole file, the retro-registration is right when it covers the majority of the file's tests; otherwise the ADR that covers the most tests.
> 4. ADRs about the attribution itself (0653, 0679, 0684, 0690, 0701, this ADR) and pure inventories are never the right primary.
> 5. A doubt that these rules do not settle is resolved in favour of the assignment (verdict agree) and the row is marked "borderline" with the doubt stated. Borderline rows count by their verdict; their number is reported.

The two differences that ADR 0720 registered for its own reading (its section "Reading the E8-primary files") apply here too, so
that the second reading asks the question the first one asked:

- the commit subject is the source of rule E8, so a verdict of agree with an E8 ADR needs more than the subject: the ADR must also
  register or first describe the file in the sense of (b), or the file's own header must name it. "First describe" I read as: the
  ADR decided the module or behaviour the file's tests check and states what is expected of it, though it need not cite the file's
  base name (an ADR that is merely about the same crate does not qualify);
- a file with an E8 primary is never "agree-null": its verdict is agree or disagree.

### What is judged for each file

**The 2 E8-primary files** (`discover.rs` with ADR 0126, `time.rs` with ADR 0122, if the index of this tree confirms them): one
verdict each, **agree** or **disagree** with the E8 primary, with a reason, and the right ADR (or "none") on a disagree.

**The 20 supplementary files** (the first E8 ADR differs from the E0 primary). The criterion is applied twice to each file, once
with the E0 primary as "the assigned ADR" and once with the first E8 ADR, giving two verdicts (E0: agree / disagree; E8: agree /
disagree), from which the file gets one **category**:

| Category | E0 verdict | E8 verdict | Meaning |
|---|---|---|---|
| **E0** | agree | disagree | the header ADR is right, the commit's is not |
| **E8** | disagree | agree | the commit's ADR is right, the header's is not |
| **both** | agree | agree | neither has a clearly better claim than the other (for example a registration and its amendment, or two ADRs of one decision) |
| **neither** | disagree | disagree | a third ADR, or none, is right; named in the row |

Rule 5 (a doubt resolved in favour of the assignment) applied to two candidates at once would make every doubtful file "both". So
for the supplementary files it is applied in this order: a doubt *between the two candidates* is resolved by rule 1 (the ADR for
which the file was first written: named by the file's header as its origin at the origin commit, or added or named by the origin
commit), and only if rule 1 does not separate them is the row "both"; either way the row is marked "borderline" with the doubt
stated.

**What I read per file** (the same for all 22, fixed now so that effort does not follow suspicion): the first 80 lines of the test
file and the names of its test functions; the commit that added it (`git log --diff-filter=A --follow --format='%h %s'`, the oldest
line; its subject, body and the list of files it adds under `docs/adr`); the first 80 lines of the file as that commit added it
when the header of the present file names an ADR newer than the origin commit; for the E0 primary and the first E8 ADR (and the
E8 primary of the first two), the title, the status line, and every line citing the file's base name with five lines of context
each side; for every other ADR that cites the base name, the title and the citing lines, up to 12 such ADRs (the lowest numbers
first; a base name such as `mod.rs` or `lib.rs` is searched with its parent directory); more of an ADR only where those lines do
not show whether it registers or mentions.

## Procedure and blindness

1. This file is committed alone.
2. The list. `python scripts/adr_test_index.py --primary git` (E8 on) and `--primary git --no-e8` in the worktree, outputs in the
   scratch folder. The E8-primary files are the files with a test whose `primary_evidence` is `E8`. The supplementary population is
   the files with E8 evidence and an E0 primary whose first E8 ADR differs from that primary; the sample is
   `random.Random(7730).sample(sorted(population), 20)`. To confirm that this is the sample of ADR 0720 without reading its
   readings, a script counts how many of the 20 paths (and of the 2) occur as strings in ADR 0720 from line 145 on and prints the
   counts and the missing paths only. If fewer than 20 occur (another sort key or population), the paths of the sample are
   extracted from ADR 0720's addendum, or from `f773-scratch/sup_list.txt` / `samples_e8.json`, by a script that prints file paths
   and nothing else, and that list is used; this is then reported as a deviation.
3. The 22 readings are written as two tables in "Addendum 1" of this ADR and committed.
4. Only then is the first reading opened (ADR 0720 addendum 1; files of `f773-scratch` if the addendum lacks a per-file verdict),
   the comparison written as "Addendum 2" and committed. Addendum 1 is not edited after step 4; a second thought about a row goes
   into Addendum 2 as a remark and does not change the count.

Blindness rests on this order and on my statement; the commit order records it, it does not prove it.

## Checks (limits fixed now)

| # | Check | Limit | Estimate behind it |
|---|---|---|---|
| C1 | The list: files with an E8 primary; the supplementary population; the sample; paths of the sample found in ADR 0720's addendum | exactly 2 (`discover.rs` 0126, `time.rs` 0122); 29; 20; 20 of 20 and 2 of 2 | the registration of ADR 0720 |
| C2 | Files read against the criterion, each with its verdicts and a reason | 22 of 22 | the acceptance |
| C3 | **Agreement of the two readings**: files on which my reading equals the first reader's, over 22 (counting below) | at least 85 %, i.e. at least 19 of 22 (18 of 22 is 81.8 %, 19 of 22 is 86.4 %) | the acceptance; I have no estimate of my own |
| C4 | Every file on which the readings differ listed with both readings and a reason | all | the acceptance |
| C5 | `git diff 1b26b8a9 -- docs/adr/0720-* docs/adr/0704-* scripts/` | empty | the acceptance: no limit changed |
| C6 | `cargo test -q --release -p rcfd-cli --test docs_site` | passes | docs touched |

How C3 is counted, in full:

- The unit is the file; the denominator is 22 whatever happens (2 + 20, pooled, as the acceptance says "of the files"). If C1 gives
  another number of files, the files found are read, the number is reported, C1 is a miss and the denominator is the number found.
- For an E8-primary file the readings are equal when both say agree or both say disagree. Two "disagree" count as equal even if the
  two readers name different right ADRs; such files are listed separately.
- For a supplementary file the readings are equal when the **category** (E0, E8, both, neither) is the same. The first reader's
  category is derived from what ADR 0720 records for the file: a named right ADR equal to the E0 primary is "E0", equal to the first
  E8 ADR is "E8"; both accepted is "both"; a third ADR or none is "neither". Two "neither" count as equal even if the readers name
  different third ADRs; listed separately. A borderline row counts by the category it was *counted* as.
- If the first reading records something coarser for a file than a category (for example only "E8 right and E0 not: yes / no"), my
  category is mapped to that record (E8 is "yes", the three others "no") for that file, and the number of files compared this way
  is reported as a deviation.
- If a file's first reading cannot be determined from ADR 0720 or its scratch files, the file counts as differing.

Reported next to C3, without a limit: the agreement over the 2 and over the 20 separately; my counts by category against the first
reader's; the number of files where I find "E8 right and E0 not" (the decision aid of ADR 0720's check E8, limit 16 of 20, which
stays ADR 0720's and is not re-decided here); the number of my borderline rows; Cohen's kappa of the two readings over the 20
(four categories), because raw agreement is high by chance when one category dominates.

## Negative controls and planned mutations

No code, so no mutations. One control on the reader: the reading must discriminate. If all 20 supplementary files get the same
category from me, or both E8-primary files get the verdict the registration of ADR 0720 predicted without my having found the
evidence for it in the ADR text, that is reported as a sign that the criterion or the reading did not discriminate, whatever C3
gives.

## Decision rule

- **done**: C1 to C6 met.
- **partial**: C2, C4, C5, C6 met and C3 or C1 missed; or all met but all 20 of my supplementary categories are the same.
- **todo**: the reading was not made, or was not blind in a way beyond the disclosures above.

A miss of C3 is a finding about the first reading or about the criterion, not a defect to repair: no row is re-read to change it.

## Limits (what is not covered)

- One second reader, an agent of the same kind as the first; not a person, and not independent of the first reader's training.
- The disclosures above: I know the first reader's expectation for both E8-primary files and for the sample as a whole.
- The supplementary category scheme is mine, fixed here from the brief ("which of the E0 primary and the first E8 ADR is right, or
  neither") and from ADR 0720's "each read as above for both ADRs"; I have not seen how the first reader recorded its readings, and
  a difference may come from the recording and not from the file. Addendum 2 says which, where I can tell.
- The 9 files of the population of 29 that are not in the sample, and the 919 files where the first E8 ADR equals the E0 primary,
  are not read.
- Whether E8 should rank above E0, and what the primary of an unregistered file should be (F-787, F-793), are not decided here.
- No limit of ADR 0720 is changed, whatever is found; a proposal goes to the lead as a follow-up.

## Addendum 1: the second reading (2026-11-17, written and committed before the first reading was opened)

Registration commit 6ff7b814 (this file alone). ADR 0720 from line 145 on and the contents of `f773-scratch` were still closed
when this addendum was committed.

### The list (C1)

`python scripts/adr_test_index.py --primary git --out f839-scratch/idx_on.json` (44 s) and `--no-e8` (15 s), then
`f839-scratch/list.py`: the files with an E8 primary are exactly `crates/rcfd-claude/src/discover.rs` (0126) and
`crates/rcfd-project/src/time.rs` (0122); the supplementary population is 29 files; `random.Random(7730).sample(sorted(population),
20)` gives the 20 files of the table below (in the order drawn). The script counted the paths that occur as strings in ADR 0720
from line 145 on and printed counts only: 20 of 20 of the sample, 2 of 2 of the E8-primary files (and 1 of the 9 files not drawn,
which says the addendum lists the sample and not the population). **C1 met**; the fallback of the procedure was not needed.

The index of this tree reports `e8`: tests 5704, files 951, links 6098 (ADR 0720 counted 5695, 950, 6089 at its base); the
population (29) and the 2 E8-primary files are as ADR 0720 registered them.

### Deviations and further disclosures

1. **`--follow`.** The registered command `git log --diff-filter=A --follow` gave, for 2 of the 22 files
   (`sa_gyro_sector_split_schur_default_bits.rs`, `si_eas_hg3.rs`), the commit that added *another* file (git's rename detection
   took the new file for a rename of a similar earlier one: 6523d01d, an F-569 record, and 490d2a86, `si_eas_hg2.rs`). For these
   two I used the oldest line of `git log --diff-filter=A` without `--follow` (a71861ee and 9c075cc2), which is the commit the
   index records in the E8 evidence. For the other 20 the two commands give the same commit.
2. **ADRs about the attribution left out of the search.** The search for ADRs citing a base name excluded 0653, 0679, 0684,
   0690, 0701, 0703, 0704, 0712, 0714, 0720 and this ADR (rule 4: never a primary), so that the grep could not print a line of ADR
   0720's addendum or of an earlier reading of the same file. No candidate ADR is among them.
3. **More of an ADR read where the citing lines did not settle it** (allowed by the registration): ADR 0126 (lines matching
   `discover`, `project_dir_name`, `agents --json`, the transcript directory: D1, D6, check D7), ADR 0122 (lines matching the
   time format: decisions 2 and 4, check T1), ADR 0155 (the rows S1 to S3), ADR 0173 (lines matching `hook`: D5 (b)), ADR 0195
   (lines matching P6, C5-K, C5-R), ADR 0051 (R1, R2).
4. The evidence per file is in `f839-scratch/ev_01.txt` to `ev_22.txt` (written by `gather.py`).

### The 2 E8-primary files

| # | File | E8 primary | Verdict | Reason |
|---|---|---|---|---|
| 1 | `crates/rcfd-claude/src/discover.rs` | 0126 | **agree** (borderline) | The origin commit 975f340d adds the crate and names ADR 0126 ("session discovery ... (ADR 0126)"). ADR 0126 decides the module (D1 lists `discover`) and the behaviour its three unit tests check (D6: the transcript directory named by replacing non-alphanumerics with `-`, the cwd compared with separators and Windows case ignored, the listing of `claude agents --json` with live pids) and its file list names `src/discover.rs`. The only other ADR citing the file (0128) is a later audit fix of its spawns. The doubt: the ADR's check table registers the integration tests `d7_*`, not these three unit tests by name, so it "first describes" the file but does not state a limit for each test; resolved by rule 5. |
| 2 | `crates/rcfd-project/src/time.rs` | 0122 | **agree** | The origin commit 46226203 adds the crate, names ADR 0122 and lists the check T1. ADR 0122 never cites `time.rs` by name, but its decision 4 fixes the UTC formats and the civil-from-days algorithm of Hinnant, and its check T1 is the file's two tests exactly ("known dates (1970-01-01, 2000-02-29, 2000-03-01, 2024-02-29, 1 700 000 000 s) and 100 000 consecutive days", limit exact; `known_dates` and `days_step_one_calendar_day_at_a_time`). That is a registration of the file's tests in everything but the file name. ADR 0128 only lists the file as a duplicate of another crate's code. |

### The 20 supplementary files

E0 is the E0 primary (the file's header), E8 the first E8 ADR (the first ADR of the origin commit's subject, or the registration
ADR of its item).

| # | File | E0 | E8 | E0 verdict | E8 verdict | Category | Reason |
|---|---|---|---|---|---|---|---|
| 3 | `rcfd-fem/tests/sa_cms_pattern_default_bits.rs` | 0511 | 0474 | agree | disagree | **E0** | Header "F-559 (ADR 0511) D4"; 0511 names the file as new and registers D4 (the digests it prints). The subject says "digests of the ADR 0474 option paths ... (ADR 0511 D4)": 0474 is the feature whose bits are recorded, it does not cite the file. |
| 4 | `rcfd-fem/tests/sa_rotor.rs` | 0476 | 0450 | agree | disagree | **E0** | Header "F-416 (ADR 0476)"; 0476 names the file as new with its checks Z, J, X, R, O, A, U, L, G. The subject names 0450 as the solver the rotor uses ("whirl by the quadratic solver of ADR 0450") before "(ADR 0476)"; 0450 does not cite the file. |
| 5 | `rcfd-fem/tests/sb_modal.rs` | 0284 | 0273 | agree | disagree | **E0** | Header "F-383 (ADR 0284)", seed `0xF383_0284`; 0284 names the file as new and holds its set-ups and checks. The subject names 0273 for the mass rules reused; 0273 (element library) does not cite the file. |
| 6 | `rcfd-fem/tests/si_rotating_whirl_sparse.rs` | 0427 | 0411 | agree | disagree | **E0** | Header "F-506 (ADR 0427)"; the subject ends "(ADR 0427), before their run" and names 0411 only as the state form the solver works on. 0411 does not cite the file. |
| 7 | `rcfd-fem/tests/sa_prestress_mixed.rs` | 0486 | 0473 | agree | disagree | **E0** | Header "F-557 (ADR 0486)"; 0486 names the test file above its checks table. The subject names 0473 as the ADR whose refusal RF1 is lifted, then "the checks of ADR 0486". |
| 8 | `rcfd-fem/tests/sa_cms_enrich_default_bits.rs` | 0532 | 0511 | agree | disagree | **E0** | Header "F-603 (ADR 0532) D6"; 0532 names the file as new and registers D6. The subject: "D6 digests of the ADR 0511 paths ... (ADR 0532)"; 0511 is the feature recorded and does not cite the file. |
| 9 | `rcfd-post/tests/sa_solid_rotor.rs` | 0476 | 0450 | agree | disagree | **E0** | Same commit as row 4. Header "F-416 (ADR 0476)"; 0476 names the file as new with checks K and S. 0450 is the eigensolver, not cited. |
| 10 | `rcfd-studio/tests/mesh_transform_u.rs` | 0155 | 0133 | agree | disagree | **E0** | Header "ADR 0155 (job 48U) S1-S3"; 0155 does not cite the file name but registers S1, S2, S3 (rows of its checks table and of its results), which are the file's tests. The subject names 0133 and 0142 for "dated notes in ADRs 0133 and 0142", then "(ADR 0155)". 0133 is run control in the Studio, another subject. |
| 11 | `rcfd-fem/src/rotating/whirl/state/arnoldi.rs` | 0428 | 0416 | agree | disagree | **E0** | Module header "(ADR 0428, F-519)"; 0428 names the file as new, defines its interfaces (D5) and its one unit test is "the development unit test of amendment 1" of 0428. The subject names 0416 for the Francis QR reused. |
| 12 | `rcfd-fem/tests/si_knife_edge_closed_form.rs` | 0394 | 0390 | agree | disagree | **E0** | Header "F-492 (ADR 0394)"; 0394 is test-only and names this file and its two test functions (D3). The subject: "the knife-edge buckling factors of ADR 0390's beam ... checks of ADR 0394". 0390 owns the beam and the A9 miss that led to F-492 but does not cite the file. |
| 13 | `rcfd-mcp/src/edit_hook.rs` | 0173 | 0195 | disagree (right: 0195) | agree | **E8** (borderline) | Header "(ADR 0173 D5 (b), ADR 0195 P6)". ADR 0173 is a scoping proposal whose D5 (b) says a plugin hook should refuse Edit and Write under `cases/`, `runs/`, `designs/` and leaves the contract "to be read from the Claude Code hooks documentation"; it does not cite the file and states no check. ADR 0195 ("ADR 0173 amendment 6, F-275 registration") names `crates/rcfd-mcp/src/edit_hook.rs` as the new module, fixes its steps and exit codes (P6), registers C5-K and C5-R and reports "one unit test in `edit_hook.rs`"; the origin commit names only 0195. The doubt: 0195 calls itself an amendment of 0173 and the header names 0173 first; by rule 1 (the origin commit names 0195) the row is E8, not "both". |
| 14 | `rcfd-fem/tests/sa_gyro.rs` | 0472 | 0450 | agree | disagree | **E0** | Header "F-415 (ADR 0472)"; 0472 names the file as new and holds the checks and the amendment about its run. 0450 is the quadratic eigensolver used. |
| 15 | `rcfd-fem/tests/sb_loads.rs` | 0276 | 0281 | agree | disagree | **E0** | Header "F-375 (ADR 0276)", seed `0xF375_0276`; 0276 names the file as new (D3, checks table). The subject names 0281 only for "the thermal load still refused (ADR 0281)", then "tests sb_loads (ADR 0276)". 0281 cites the file later, to retire its X1 block: a later ADR touching one check (rule 1, secondary). |
| 16 | `rcfd-fem/tests/sa_gyro_sector_split_schur_default_bits.rs` | 0527 | 0492 | agree | disagree | **E0** | Header "F-584 (ADR 0527) G11"; 0527 names the file and registers G11. The subject: "default-bits record of the ADR 0492 sector split paths ... (ADR 0527 G11)"; 0492 is the feature recorded. |
| 17 | `rcfd-fem/tests/sa_cms_residual.rs` | 0474 | 0469 | agree | disagree | **E0** | Header "F-552 (ADR 0474)"; 0474 names the file as new for all checks but D2. The subject names 0469 (the reduction extended) and 0464 (a routine shared) before "checks of ADR 0474". |
| 18 | `rcfd-io/tests/w27c_registry.rs` | 0051 | 0244 | agree | disagree | **E0** | Header "Wave 27C (ADR 0051) registered checks R1 and R2"; 0051 registers R1 (registry) and R2 (deck round trip) and lists the file in its addendum. The E8 ADR comes from the item: the subject is "F-237: format registry, Nastran-style load deck" (2026-10-17) and 0244 is the registration ADR F-237 received on 2026-11-12 for a later extension (heat-rate export); 0244 cites the file only as an existing test it must not force an edit of. |
| 19 | `rcfd-fem/tests/si_eas_hg3.rs` | 0630 | 0614 | agree | disagree | **E0** | Header "F-703 (ADR 0630)"; 0630 D5 names the file and what it holds. The subject names 0614 for the existing coefficient ("RestStress the ADR 0614 coefficient") before "the checks of ADR 0630". |
| 20 | `rcfd-fem/tests/si_rotating_whirl_state_sparse.rs` | 0428 | 0416 | agree | disagree | **E0** | Same commit as row 11. Header "F-519 (ADR 0428)"; 0428 names the file as the new test file. 0416 is the state matrix and QR reused. |
| 21 | `rcfd-fv/tests/coupled_ddt_fractional_bdf2_f757.rs` | 0687 | 0675 | agree | disagree | **E0** | Header "F-757 (ADR 0687)"; the subject lists the file with "(ADR 0687: P1f, ...)" and names 0675 for the defect routine shared. 0675 does not cite the file. |
| 22 | `rcfd-fem/tests/si_modal_mpc.rs` | 0384 | 0296 | agree | disagree | **E0** | Header "F-486 (ADR 0384)"; 0384 names the file as new and registers all its checks. The subject: "modal analysis on the transformation of ADR 0296 ... the checks of ADR 0384"; 0296 is the transformation reused. |

### Counts of the second reading alone

- E8-primary files: 2 of 2 agree (1 borderline). Both verdicts are the ones the registration of ADR 0720 predicted; for each
  the evidence was found in the ADR text (D1 and D6 of ADR 0126; decision 4 and check T1 of ADR 0122), so the control of the
  registration is not triggered, but with 2 files it has little force.
- Supplementary files: **E0 19, E8 1, both 0, neither 0**; 1 borderline (row 13). Not all in one category, so the control is not
  triggered. "E8 right and E0 not": 1 of 20 (the decision aid of ADR 0720's check E8 asks for at least 16).
- The pattern in the 19: the subject of the origin commit names an earlier ADR as the thing the new work builds on, lifts or
  records ("the quadratic solver of ADR 0450", "RF1 of ADR 0473 lifted", "digests of the ADR 0474 option paths") and names the
  file's own ADR last, usually in brackets. The first ADR of the subject is then the background, the last one the origin. In 18 of
  the 20 rows the E0 primary is also among the E8 ADRs of the file, as the last ADR of the subject; the 2 exceptions are rows 13
  (subject names only 0195) and 18 (the E8 ADR comes from the item).
- C2: 22 of 22 read, each with a verdict and a reason. **Met.**
