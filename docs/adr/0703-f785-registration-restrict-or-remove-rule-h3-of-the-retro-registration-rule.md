# ADR 0703: F-785 registration: restrict or remove rule H3 of the retro-registration rule

Status: **REGISTERED 2026-11-17, written before any code of F-785 and before any run of its checks.** Not yet accepted by the
maintainer. Results go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-785 (deps F-772), proposed by [ADR 0701](0701-f772-registration-retro-registration-adrs-and-header-adr-paths-as-primary-evidence.md)
addendum 1 (open question 2: H3 read 1 of 5, all four disagreements were H3 rows). Number 0703 given by the lead. Branch `f785-work`
from 55831919 (F-768 / 0690 and F-772 / 0701 merged), worktree `w49i-work/f785`, scratch `w49i-work/f785-scratch`.

House rules (`w49i-work/AGENT_RULES.md`) apply: limits fixed here; each registered run made once; stdlib Python only; no Rust file,
hash table or golden touched; ADR 0701 and its readings are not edited (a separate item, F-786, re-reads its 28 files); the roadmap
files are not edited. Choices the maintainer would normally be asked about are taken on the maintainer's standing instruction of
2026-11-11 (take the recommendation).

## What F-785 asks

> Every limit not stated here is registered in the ADR before the run. A table row that cites a file and has a limit in another cell
> takes the primary only where its ADR registers the file (the ADR file name carries `registration`, or the row is in a section the
> ADR names as the registration of the file's item), or rule H3 is removed; the 8 files of rule H3 of ADR 0701 and a fresh registered
> sample of at least 20 files with a primary under rules H0 to H3 as they stand are read with at least 90 % agreement counting only
> the files with a primary, and the number of files with a primary is reported next to the agreement; the attribution data of
> `gate_attribute.py` is unchanged for the existing consumers.

## What was read, and what was done beforehand (disclosed)

- ADR 0701 in full (rules H0 to H3, its readings and its open questions), `scripts/adr_test_index.py` (`retro_units`,
  `retro_primaries`, `build`, `load_docs`) and `scripts/test_adr_test_index_retro.py` at 55831919.
- One exploration run of the unchanged code on this tree with the saved metadata of F-772 (`f772-scratch/meta.json`; the crates have
  not changed): files with a retro primary by tier, with the file name of the ADR: **H0 7 files, H1 19, H2 5, H3 8** (the numbers of
  ADR 0701). The 8 H3 files and their ADRs: `rcfd-fv/src/bc.rs` 0650, `rcfd-fv/src/pimple.rs` 0245, `rcfd-fv/tests/cavity.rs` 0060,
  `cylinder.rs` 0060, `step_laminar.rs` 0060, `rcfd-cli/tests/run_control.rs` 0123, `rcfd-api/src/schema.rs` 0136,
  `rcfd-api/src/sim.rs` 0110. **Of these, only 0245 (`pimple.rs`) and 0650 (`bc.rs`) have `registration` in the ADR file name**; the
  other six would lose the primary under the first option of the acceptance.
- What I knew of their readings: ADR 0701 has my readings of `pimple.rs`, `run_control.rs`, `cavity.rs`, `cylinder.rs` and
  `step_laminar.rs` (agree only for `cylinder.rs`, borderline); I have not read `bc.rs`, `schema.rs` and `sim.rs`. That knowledge is
  why the choice below is registered before the readings and why the sample is drawn by a seed.
- Nothing about the files of the sample (below) was read for this item.

## Design: the choice, and its reason

Options: (R) restrict H3 to ADRs that register the file (file name with `registration`, or a section the ADR names as the
registration of the file's item); (D) remove H3.

**Taken: (D), remove H3.** Recommendation and reason:

1. The ADR file name `registration` says the ADR registers the *item* (a roadmap item's checks); it does not say that a table row
   that cites a file registers *that file*. ADR 0245 is such an ADR and its row for `pimple.rs`, a file of the initial import, is a
   known false positive; (R) by file name keeps it, so (R) keeps one of the four readings that disagreed. A restriction that drops it
   would need a rule written to the readings already seen (tuning to the data). The second clause of (R), a section the ADR names as
   the registration of the file's item, has no textual marker that I can state before reading the 8 ADR texts, so it would be a new
   heuristic with no limit estimated.
2. H3 gave 54 tests in 8 files; H0 to H2 read 22 of 22 (ADR 0701). What is lost is a primary for the 8 files: the attribution falls
   back to "no primary" (the consumers print the whole ADR list next to the primary, as before), the safe value.
3. Removal is deterministic and has nothing to tune; a later item can add an evidence-based rule (for example git evidence, the second
   follow-up of ADR 0690) for these files.

**Implementation.** Rule H3 is switched off in every command and every default call: the module constant `RETRO_H3 = False`; the units
of tier 3 are produced by `retro_units` only when it is true. The code stays behind the constant only so that the 15 tests of ADR 0701
remain as records (their set-up sets the constant, see C2); no command line or keyword turns it on. `coverage.primary.retro` keeps
its keys (`h3` is 0), so no consumer sees a different shape. Docstrings and the `--report` text are updated. H0, H1 and H2 are not
touched. `--primary registered` and `--primary p0679` are not touched. `gate_attribute.py` and `gate.yml` are not edited.

## Sample (registered before the run)

- **Pool P**: the distinct files that get a primary under rules H0, H1 or H2 of ADR 0701 as they stand (the unchanged code, `--primary
  git`, saved metadata, this tree): the files with a retro primary of tier 1 or 2, plus the H0 files (files whose `header_adr` is
  found only with `by_path`), minus the 8 H3 files. Expected size 31 (7 + 19 + 5). A pool of at least 20 files that ADR 0701 did not
  read does not exist (it read most of P), so the sample is **drawn from all of P, whether or not ADR 0701 read the file; the overlap
  is counted and reported**, and each file is read anew without carrying a reading over.
- **S20**: 20 files drawn once with `random.Random(7850).sample(sorted(P), 20)` (7850 = F-785). If P has fewer than 20 files, the
  whole of P is the sample and the number is reported.
- **T8**: the 8 H3 files of ADR 0701, read under the final rule (they have no primary after the change; each is read as `agree-null`
  when no ADR describes the file better than "no primary" and `disagree-null` when an ADR registers the file or first describes it).
- Reading, as in ADR 0701: the reader sees the file's header, the ADR named by the primary (title, the unit that registered, whether
  the ADR registered the file's item or first described it) and the first commit that adds the file. Agree: the ADR registered the
  file's item or first described the file (header, a section on the file, an ADR note that says the file had none). Disagree: the ADR
  only lists, edits, runs or mentions the file, or an earlier ADR described it better. The reader is the implementing agent (single
  reader, disclosed; F-786 reads ADR 0701's files independently, not these).

## Checks (limits fixed now)

| id | check | limit | estimate behind it |
|---|---|---|---|
| C1 | new tests in `scripts/test_adr_test_index_h3.py` (fake workspace, no git): A1 a table row with a limit gives no primary in the default build, also when the ADR file name has `registration`; A2 the `h3` key of `coverage.primary.retro` is present and 0; A3 H1 and H2 give the same primaries as before in the same workspace; A4 H0 unchanged; A5 a file with an H3 row and an H2 phrase in another ADR takes the H2 ADR | all pass | by construction |
| C2 | the 15 tests of `scripts/test_adr_test_index_retro.py` (ADR 0701): one line in `setUpClass` sets `ati.RETRO_H3 = True` (restored in `tearDownClass`); no assertion edited | 15 pass | the old behaviour is behind the constant |
| C3 | data: index of this branch (`--primary git`) against the index of the unchanged code (55831919) on the same tree and metadata, one run each | the only differing fields are `primary_adr` and `primary_evidence` of the tests of the 8 H3 files (54 tests; both become null); `coverage` equal except `primary` (`h1` 72, `h2` 20 as before, `h3` 0, `none` 310 + 54 = 364, `retro.files` the 24 H1 and H2 files); tests with an ADR and no primary 351 + 54 = 405 | by construction |
| C4 | `--primary registered` and `--primary p0679` equal to the unchanged code; `gate_attribute.py` and `gate.yml` byte-identical; the six consumer test files of ADR 0701 R5 pass with no edit | 0 differences; 0 bytes; pass | |
| C5 | git processes per run and run time (default flag, saved metadata, one run) | at most 2; at most 37.1 s + 10 s | unchanged work |
| C6 | S20 read | agreement (agree over files with a primary) **at least 90 %**; the number of files with a primary reported next to it (20 expected) | ADR 0701 read H0 5/5, H1 13/13, H2 4/4 on the files it read; 18 of 20 is the floor |
| C7 | T8 read | the 8 files have no primary (8 of 8); the null readings reported (`agree-null` / `disagree-null`), no limit; the agreement over files with a primary is over 0 files and is not defined | |
| C8 | S20 and T8 together | agreement over the files with a primary (all in S20, T8 has none) at least 90 %, the number of files with a primary next to it; per tier counts (H0, H1, H2) of the sample | the acceptance |
| C9 | `python -m unittest discover -s scripts` from the clean worktree; `docs_site` (this ADR is under docs/) | pass (the skipped E2E tests ignored); 2 passed | |

Information only, no limit: the 2 of the 8 H3 files that the first option of the acceptance would have kept by file name (`pimple.rs`,
`bc.rs`) and their readings, reported in the addendum.

## Negative controls and planned mutations

A1 and A5 of C1 are the negative controls. Mutations on a scratch copy of `scripts/` (each must be caught, or recorded as not
caught): M1 `RETRO_H3 = True` (A1, A2, C3); M2 `retro_units` drops the tier-2 unit of table rows (`p3_h2` of ADR 0701's tests, and A3);
M3 the `h3` key removed from the retro statistics (A2); M4 `retro_units` ignores the constant, tier 3 always produced (A1); M5 H1 off
when H3 is off (A3, P2).

## Decision rule

**done** if C1 to C9 are all met and all mutations are caught; **partial** if any limit is missed (the number recorded, the limit
stays), in particular C6 or C8 (the 90 %); **todo** if C3 or C4 fails (a wrong or changed result).

## Limits (what is not covered)

- Single reader, the author of the choice; the sample overlaps the files that ADR 0701 read (counted), so it is a re-reading of mostly
  the same files by the same reader and says little about files neither of us looked at; F-786 is the independent reading.
- The 8 files lose their primary; whether a better rule (git evidence, a registered item map) finds the right ADR for them is not
  tested here.
- Rules H0 to H2 are not re-tested for recall; only the precision of their primaries is read.
- The textual patterns of H1 and H2 are as in ADR 0701.
