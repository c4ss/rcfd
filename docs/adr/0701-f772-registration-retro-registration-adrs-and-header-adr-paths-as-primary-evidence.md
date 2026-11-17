# ADR 0701: F-772 registration: retro-registration ADRs and ADR paths in file headers as primary evidence for file-citation-only tests

Status: **REGISTERED 2026-11-17, written before any code of F-772 and before any run of its checks.** Not yet accepted by the
maintainer. Results go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-772 (deps F-768), proposed by [ADR 0690](0690-f768-registration-primary-adr-from-the-first-commit-that-adds-a-cited-file.md)
addendum 1 (open question 3: 9 of the 25 files of its sample E and 4 of the 20 of sample D are described by an ADR that rule G
cannot see). Number 0701 given by the lead. Branch `f772-work` from 5a92bcd7 (F-737 / 0679, F-759 / 0684, F-760 / 0685 and F-768 / 0690
merged), worktree `w49i-work/f772`, scratch `w49i-work/f772-scratch`.

House rules (`w49i-work/AGENT_RULES.md`) apply: limits fixed here; each registered run made once; existing tests and ignored records
not edited; no Rust file, hash table or golden touched; stdlib Python only; clean-room; the roadmap files are not edited. Choices the
maintainer would normally be asked about are taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation),
except where listed as open.

## What F-772 asks

> Every limit not stated here is registered in the ADR before the run. A header comment that names an ADR as a path
> (`docs/adr/NNNN-...md`) counts as E0 evidence, and an ADR that says of a file that it was committed without an ADR or states the
> checks of a file it first registers (ADR 0024, 0060 and 0685 are the cases found by ADR 0690) is read by a rule registered before the
> run; a fresh registered sample of at least 25 file-citation-only files drawn from the files that rule G leaves without a primary, and
> the 45 files read in ADR 0690, are read with at least 80 % agreement counting only the files with a primary, and the number of files
> with a primary is reported next to the agreement; the attribution data of `gate_attribute.py` is unchanged for the existing
> consumers.

## What was read, and what was done beforehand (disclosed)

- `scripts/adr_test_index.py` at 5a92bcd7 (`header_adr`, `primary_adr`, rule G in `build()`, `git_history`), the tests
  `scripts/test_adr_test_index_git.py` (fake git workspace helpers `make_repo`, `meta`), `scripts/gate_attribute.py` (reads only the
  record field `primary_adr`, line 297, and prints it in `adr_cell`), ADR 0690 in full (rule G, its samples D and E and their readings,
  open question 3), ADR 0685 (the plan table and the notes it appended), ADR 0024 and ADR 0060 at the passages of ADR 0690's readings.
- `cargo metadata` saved to scratch and one run of the unchanged index on this tree, default flag (rule G), saved as `idx_base.json`:
  **37.1 s** wall (the "before" of the run-time check); `coverage.primary`: 7237 tests with a primary (E0 7231, E2 6), 522 E2 tests,
  `from_git` 6, `no_commit_rule` 514, `not_in_adrs` 2.
- By `grep`: 12 old ADRs end with a section headed `## Test files (added 2026-11-17, ADR 0685)` (the note ADR 0685 appended; ADR 0024
  and 0060 are two of them and their note says the file was committed without an ADR); 56 Rust files have a line comment that
  contains `docs/adr/dddd-`, not counting whether the line is in the leading comment block. I know from ADR 0690's readings how the
  45 old files read; **the patterns below were chosen with that knowledge** (the three cases the acceptance names). That is why a fresh
  sample of files I have not looked at is part of the acceptance, and why it is drawn after the code is final. Nothing about the
  other files was computed before this text.

## Design

**Rule H (retro-registration), `--primary git` only.** The command-line default `git` gets two additions; `--primary registered` and
`--primary p0679` are unchanged. The library call `build(root, meta, "git")` gets a keyword `retro` (default True for `git`; the keyword
is there so that rule G alone can still be run, for the pool of the samples and for the controls).

**H0, a header names its ADR as a path (E0).** `header_adr` today takes the first `ADR dddd` of the leading comment block (lines
before the first line of code). New: when the block has no `ADR dddd`, the first `docs/adr/dddd-` of the block (regex
`\bdocs/adr/(\d{4})-[\w.-]+\.md`, any line of the block) gives the ADR, with the same conditions as before (the ADR exists, it is not
one of 0653 and 0679). A file whose leading block already has an `ADR dddd` keeps it, bit for bit; a path in a comment after the first
line of code does not count. The evidence is E0 like the others (kind `E0`, `adr`), so it enters `adrs`, `items`, `evidence` and,
being the first tier, the primary. The path is the author's own statement of which ADR fixed the file's limits, as the `ADR dddd`
form is.

**H1/H2/H3 (an ADR that registers a file after the fact).** For a test whose rule-R primary evidence is E2 (file-citation-only) and
that rule G left without a primary (`no_commit_rule`, `not_in_adrs`, `no_origin`, or a shallow or absent history), the primary is the
first ADR, by tier then by number, that has a *registering unit* for the test's file. The ADR texts are cut into units: a line that
starts with `|` is a unit of its own (a table row); other consecutive non-blank lines up to a blank line, a heading line or a table row
are one unit (a paragraph); heading lines are not units. A unit **cites the file** when its `.rs` tokens give one of the file keys the
index already uses for E2 (crate-relative path, or the stem when it is unique). The ADRs excluded as sources are `LISTING_ADRS`
(0653, 0679, 0684) and, because their tables quote the sample files and their readings, 0690 and 0701 (`RETRO_EXCLUDED`). Tiers:

- **H1**: a unit that cites the file inside a section whose heading line matches `^#{2,3}\s+Test files \(added \d{4}-\d{2}-\d{2}, ADR \d{4}\)\s*$`
  (the section runs to the next line that starts with `# ` or `## `, or to the end of the text). The primary is the ADR that holds the
  section, not the ADR named in its heading. This is the note ADR 0685 appended to old ADRs.
- **H2**: a unit that cites the file and matches (case-insensitive) `without an ADR|ADR-less|had no ADR|no ADR (describes|registers|registered|states|names|covers)`:
  the ADR says of the file that it had no ADR, so it takes on the file's description.
- **H3**: a table row that cites the file in one cell and has another cell with a limit: `\d+(\.\d+)?\s*%`, a decimal `\d\.\d`, or a
  comparison `[<>]=?\s*\d`. The ADR states the checks of the file in a table; the lowest number is the first to do so.

Within a tier the lowest ADR number wins (the order of ADR numbers equals the order of the git files, ADR 0684). The primary has
`primary_evidence` `E2`, as in ADR 0690, so the record keeps its keys. A test with any other primary evidence, or with a primary from
rule G, is not touched. A mere mention is not a registration: a unit that only cites the file, without a marker, a section or a limit
cell, gives nothing (negative control N1).

**Output.** `coverage.primary.retro`: `{"h1": n, "h2": n, "h3": n, "none": n, "files": {file: [tier, adr]}}` counting tests and the
files with a retro primary; the existing `with_primary` and `by_evidence` are recomputed from the final primaries. Test records keep
exactly their keys; the E0 tests of H0 have the extra `evidence` entry and ADR in `adrs` like any E0. `gate_attribute.py`, `gate.yml`
and every existing test are not edited.

**Reading by sample.** As in ADR 0690: the reader sees the file's header, the ADR named by the primary (title, the unit that
registered, and whether the ADR registered the file's item or first described it) and the first commit that adds the file. A primary
**agrees** when that ADR registered the file's item or first described the file (its header, a section on the file, an ADR note that
says the file had none); it **disagrees** when it only lists, edits, runs or mentions the file, or an earlier ADR described it better.
A file with no primary is `agree-null` or `disagree-null` as in ADR 0690. The acceptance's *agreement* here counts **only the files with
a primary** (agree over files with a primary); the number of files with a primary is reported next to it, and the null readings are
reported separately. The reader is the implementing agent (disclosed; a single reader).

## Samples

- **S45**: the 45 files read in ADR 0690 (sample D, 20 files, and sample E, 25 files; `f768-scratch/samples.json`), re-read with their
  new primary or null. The ones that had an agreeing or null reading before are re-read, not carried over.
- **S25**: a fresh sample. Pool: the distinct files that have an E2-only test and that rule G alone (code of this branch,
  `retro=False`) leaves without a primary, at the final tree (including files that H0 or H1 to H3 will give one), minus the 45 files of
  S45. **25** are drawn once with `random.Random(7720).sample` from the sorted pool (7720 = F-772). The overlap with samples A and A'
  of ADR 0679 and C of ADR 0684 is not removed and not checked.

## Checks (limits fixed now)

| id | check | limit | estimate behind it |
|---|---|---|---|
| R1 | unit tests, all in a new file `scripts/test_adr_test_index_retro.py` on a fake workspace (no git needed): N1 an ADR that only cites the file (no marker, no limit cell) gives no primary; N2 a heading `## Tests` (not the marker) with the file gives none; N3 the phrase in a unit that does not cite the file gives none; N4 a listing ADR (0684, 0690, 0701) with the phrase gives none; N5 a header path in a comment after the first line of code, or to an ADR file that does not exist, or to 0679, gives no E0; N6 a leading block with `ADR dddd` and a path keeps the `ADR dddd` | all pass | by construction |
| R2 | positive tests: P1 H0 path-only header gives E0 and primary; P2 H1 section of an old ADR; P3 H2 phrase; P4 H3 table row with a limit; P5 tier order H1 over H2 over H3 over a lower-numbered ADR of a lower tier; P6 within a tier the lowest number; P7 a test with a rule-G primary is not changed; P8 a non-E2 primary is not changed | all pass | by construction |
| R3 | `--primary registered` and `--primary p0679` give the same index as the unchanged script (control: `idx_base2.json` below, the unchanged script of 5a92bcd7 on the final tree) | 0 differences | the new code is behind `retro` |
| R4 | data unchanged: index of this branch (`--primary git`) against the unchanged script's `--primary git` on the final tree (`idx_base2.json`, one run, so ADR 0701's own citations are in both): every field of every test equal except `primary_adr`, `primary_evidence`, and for tests whose file got an H0 evidence `adrs`, `items`, `evidence`; `binaries`, `crates`, `items`, `adrs[n].files` equal; every test with a rule-G primary or a non-E2 primary evidence keeps it, except the tests whose file got an H0 path (counted and listed); `coverage` equal except `primary` | 0 other differences; count of H0 files recorded | by construction |
| R5 | the unit tests of the existing consumers pass with no edit: `scripts/test_gate_attribute.py`, `test_gate_hooks_f761.py`, `test_adr_test_index.py`, `test_adr_test_index_wave.py`, `test_adr_test_index_primary.py`, `test_adr_test_index_git.py`; `gate_attribute.py` and `gate.yml` byte-identical | pass; 0 bytes | |
| R6 | shallow clone and git archive copy (the controls of ADR 0690, same saved metadata): the indexes differ from the full run only in the tests whose primary came from git (rule G, 6 tests in 2 files at 5a92bcd7) | at most the `from_git` tests differ | H needs no git; G needs a full history |
| R7 | git processes per run (counting `subprocess.run`) | at most 2 | H reads only the ADR texts |
| R8 | run time of the index, default flag, saved metadata, one run after the code is final, against the 37.1 s measured before | at most before + 10 s | the unit split of about 1800 ADR files is linear text work (below 1 s) |
| R9 | coverage record: tests with an ADR and no primary before (559) and after, and the split into H0, H1, H2, H3, none | recorded; no limit | the 45 files read in ADR 0690 suggest 10 to 40 of the 100 files get one |
| R10 | S45 re-read | agreement (agree over files with a primary) **at least 80 %**; the number of files with a primary reported next to it; **at least 5** of the 45 files with a primary | the three named cases (0024, 0060, 0685's own table) and `mrf.rs` give about 8 files, of which I expect most to agree; 5 is a floor under which the rule says nothing |
| R11 | S25 read | agreement over the files with a primary at least 80 %; number with a primary reported (no floor: a pool of files that rule G left null may have few retro-registered ones) | a precision estimate on files the patterns were not chosen from; H3 is the weakest tier and is reported apart |
| R12 | S45 and S25 together | agreement over files with a primary at least 80 % (the acceptance); per tier counts (H0, H1, H2, H3) reported | |
| R13 | `python -m unittest discover -s scripts` from the clean worktree (not the main checkout); `docs_site` (this ADR is a file under docs/) | pass (the 4 skipped E2E tests ignored); 2 passed | |

Samples are drawn once, from the index built after the code is final, and the S25 sample is read once.

## Negative controls and planned mutations

R1 are the negative controls: an ADR that merely mentions a file is not a registration (N1); only the registered heading makes a
section (N2); the unit has to cite the file (N3); the sample-listing ADRs are not sources (N4); a path outside the leading block, to a
missing ADR, or to a listing ADR is not E0 (N5).

Mutations on a scratch copy of `scripts/` (each must be caught by a test, or recorded as not caught): M1 the H0 path read in the whole
file, not the leading block (N5); M2 the H0 path used before `ADR dddd` (N6); M3 H1 without the heading (any section) (N2); M4 H2 without
the phrase (any unit that cites the file) (N1, N3); M5 H3 without the limit cell (N1); M6 highest number in a tier instead of lowest
(P6); M7 tiers reversed (P5); M8 the listing exclusion dropped (N4); M9 H applied over a rule-G primary (P7); M10 H applied to non-E2
tests (P8); M11 `retro` on for `registered` (R3: control run).

## Decision rule

**done** if R1 to R13 are all met and all mutations are caught; **partial** if any limit is missed (the number is recorded and the limit
stays), in particular R10, R11 or R12 (the acceptance's 80 %); **todo** if R3, R4, R5 or R6 fails (a wrong or changed result).

## Limits (what is not covered)

- The patterns are textual: an ADR that says the same in other words (for example "this file had no registration") is not read. The
  phrase list was chosen with the cases ADR 0690 found; a fresh sample tests it, but the list is not claimed to be complete.
- The units are paragraphs and rows, not sentences; a paragraph that cites several files and has the phrase for one of them gives all
  of them the primary (a false positive the sample may or may not show).
- H3 reads a table row as "states the checks of the file", which a row of a coverage table also is; the lowest number is taken as the
  first, not verified by git.
- A file whose only describing ADR cites it by a name the index does not map to its file keys (a bare binary name `rcfd-api/mesh_params`
  with no `.rs` token) is not seen, as before.
- Files read only by the implementing agent (single reader); the S45 readings repeat the ones of ADR 0690 by the same reader.
- No git evidence is added (that is the second follow-up of ADR 0690, a different item).
