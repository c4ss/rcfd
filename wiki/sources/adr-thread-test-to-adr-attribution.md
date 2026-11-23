---
type: source
tags: [adr-thread, gate, testing, attribution, process, second-reader]
sources: [doc-test-process]
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: the test-to-ADR index and the primary ADR of a test (F-737 to F-795, F-839)

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread (registrations, retro-registrations, second and third readings), 19 ADRs, all 2026-11-17.

`scripts/adr_test_index.py` (ADR 0653) maps every test to the ADRs that describe it, so that the daily gate can name the ADR to open when a test fails. This thread is the chain of items that followed: wave tags and a **primary ADR** per test (F-737), a primary only from registering evidence (F-759), ADR text for unmapped binaries (F-760), the primary from the commit that adds a file (F-768), retro-registration rules H1 to H3 (F-772, F-785, F-790, F-792), new ADRs for five feature groups no ADR described (F-769, ADRs 0705 to 0710), commit-subject evidence E8 (F-773), and independent second readers of every sample (F-786, F-794, F-795, F-839). See [[test-to-adr-index]].

## Current state

- **Mapping coverage is near complete**: after F-760 (ADR 0685) "0 unmapped integration binaries with tests (baseline 25, limit 5)", all tests mapped 98.2 % to 99.7 %.
- **The primary ADR rule** as it stands: tiers E0 (header ADR or an ADR path in the leading comment block) > E1 > E6 > E8 (ADR or item named by the commit that first adds the file, ADR 0720) > E2 (file citation), with retro-registration units H1 (a `## Test files (added <date>, ADR dddd)` note) and H2 (a unit saying `without an ADR`, `ADR-less` or `no ADR describes`) for file-citation-only tests; H3 (table rows) was removed (ADR 0703) and its dormant code deleted (ADR 0712). On a shallow clone or a `git archive` copy the git-based primaries are null.
- **Definition of "primary" decided by the maintainer** (ADR 0704, answers of 2026-11-18, Q1 option (a)): "The primary is the ADR that decided the file's checks, and where there is none the file has no primary; a record written afterwards to describe the file does not become its primary." F-787 and F-793, which rest on it, "may be issued" (both todo).
- **Agreement of readers is the weak point.** Second reading of 28 files (ADR 0704): 21 of 28 (75.0 per cent) against the limit 85; five of the seven differences come from different criteria. Later readings against a written criterion agree: ADR 0715 19 of 20 limit rows, ADR 0717 18 of 18, ADR 0721 22 of 22 (but chance agreement on that one-sided sample is 90.5 %).
- **Partial items and why** (roadmap gaps): F-737 (tests that gained an ADR 15.8 % against 12; "binary named in an ADR" evidence 1 of 4); F-759 (file-citation-only sample 3 of 20 against 16); F-768 (fresh sample 16 of 25 against 20; tests with an ADR and no primary 456 limit missed); F-772 (9 of 12, 75 per cent against 80); F-785 (mutations M2 and M5 caught only after tests A6 and A7, waits for the maintainer to call it done); F-786 (75.0 against 85); F-769 (32 tests outside the eight files gain an ADR against the limit 10); F-773 (the sample of 20 E8-primary files cannot be drawn: the population is 2).
- **Todo follow-ups:** F-787, F-788 (a third reading against one written criterion), F-793, F-840.

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0679](../../docs/adr/0679-f737-registration-wave-tag-mapping-and-a-primary-adr-in-the-test-to-adr-index.md) | 2026-11-17 | registration | F-737 | Wave-tag mapping and a primary ADR per test | partial; unmapped binaries 79 to 25; 17 of 20 sample (amended rule); 15.8 % gained against 12 | 0684, 0685 |
| [0684](../../docs/adr/0684-f759-registration-primary-adr-only-from-registering-evidence.md) | 2026-11-17 | registration | F-759 | Primary only from registering evidence | partial; uniform files 39 of 40; file-citation-only 3 of 20 against 16 | 0690 |
| [0685](../../docs/adr/0685-f760-registration-adr-text-for-the-25-unmapped-integration-test-binaries.md) | 2026-11-17 | registration (ADR text) | F-760 | ADR text for the 25 unmapped binaries | done; "Test files" notes appended to 12 ADRs; 0 unmapped; sample 7 of 8 | |
| [0690](../../docs/adr/0690-f768-registration-primary-adr-from-the-first-commit-that-adds-a-cited-file.md) | 2026-11-17 | registration | F-768 | Primary of a file-citation-only test from the commit that adds the file | partial; sample D 16 of 20, fresh 16 of 25 against 20 | 0701, 0720 |
| [0701](../../docs/adr/0701-f772-registration-retro-registration-adrs-and-header-adr-paths-as-primary-evidence.md) | 2026-11-17 | registration | F-772 | Retro-registration units and header ADR paths as primary evidence | partial; 9 of 12 (75 %) against 80; H3 read 1 of 5 | 0703 |
| [0703](../../docs/adr/0703-f785-registration-restrict-or-remove-rule-h3-of-the-retro-registration-rule.md) | 2026-11-17 | registration | F-785 | Restrict or remove rule H3 | proposed done, item partial; H3 removed (`RETRO_H3 = False`); 20 of 20 by the rule's author | 0712 |
| [0704](../../docs/adr/0704-f786-registration-a-second-reader-for-the-primary-adr-samples.md) | 2026-11-17 | reading | F-786 | A second reader for the primary-ADR samples | partial; 21 of 28 (75.0 %) against 85; Q1 answered 2026-11-18 | |
| [0705](../../docs/adr/0705-f769-registration-adrs-for-the-five-feature-groups-that-no-adr-describes.md) | 2026-11-17 | registration (index of retro ADRs) | F-769 | ADRs for five undescribed feature groups | partial; 35 of 35 tests named, 24 of 25 tolerance literals; 32 outside tests gain an ADR against 10 | |
| [0706](../../docs/adr/0706-f769-registration-assistant-setup-helpers-and-the-workflow-engine.md) | 2026-11-17 | retro-registration | F-769 | Assistant setup helpers and the workflow engine (24 limit rows) | record; row H22 corrected by ADR 0717 | |
| [0707](../../docs/adr/0707-f769-registration-the-f040-wall-function-channel.md) | 2026-11-17 | retro-registration | F-769 | The F-040 wall-function channel | record; row W1 corrected by ADR 0717 | |
| [0708](../../docs/adr/0708-f769-registration-the-shipped-turbulence-and-compressible-examples-through-the-binary.md) | 2026-11-17 | retro-registration | F-769 | Shipped turbulence and compressible examples through the binary | record | |
| [0709](../../docs/adr/0709-f769-registration-the-mesh-generator-parameters.md) | 2026-11-17 | retro-registration | F-769 | The mesh generator parameters | record | |
| [0710](../../docs/adr/0710-f769-registration-tables-called-from-expressions.md) | 2026-11-17 | retro-registration | F-769 | Tables called from expressions | record; "the test named 'with a span' does not check the span" | |
| [0712](../../docs/adr/0712-f790-registration-delete-the-dormant-rule-h3-code-and-its-record-tests.md) | 2026-11-17 | registration | F-790 | Delete the dormant H3 code and its record tests | done; index byte-identical before and after | 0714 |
| [0714](../../docs/adr/0714-f792-registration-a-test-of-the-per-file-tier-list-of-the-retro-counts-for-rules-h1-and-h2.md) | 2026-11-17 | registration | F-792 | A test of the per-file tier list of the H1/H2 retro counts | done; 5 of 5 planted changes caught | |
| [0715](../../docs/adr/0715-f794-registration-a-second-reader-for-the-limit-rows-of-adrs-0706-to-0710.md) | 2026-11-17 | reading | F-794 | A second reader for 20 limit rows of ADRs 0706 to 0710 | done; 19 of 20 (95 %) against 90; H22 disagrees | 0717 |
| [0717](../../docs/adr/0717-f795-registration-corrections-and-a-reading-of-the-18-unread-limit-rows-of-adrs-0706-to-0710.md) | 2026-11-17 | reading + corrections | F-795 | Correct H22 and W1, read the 18 unread rows | done; 18 of 18 against 16 | |
| [0720](../../docs/adr/0720-f773-registration-a-git-commit-that-names-an-adr-or-an-item-as-evidence-of-the-files-it-adds.md) | 2026-11-17 | registration | F-773 | The ADR or item named by the commit that adds a test file, as evidence E8 | partial; the sample of 20 cannot be drawn (population 2) | 0721 |
| [0721](../../docs/adr/0721-f839-registration-a-second-reader-for-the-e8-primary-files-and-the-supplementary-sample-of-adr-0720.md) | 2026-11-17 | reading | F-839 | Second reader for the E8 files and the supplementary sample | done; 22 of 22 against 19 of 22 | |

## Key numbers and limits

- **F-737** (ADR 0679): "Integration binaries unmapped with tests fell from 79 to 25 (limit 65); all tests mapped 93.7 % to 98.2 %; no test lost an ADR". First rule 11 of 20, amended rule 17 of 20 on a fresh draw (limit 17).
- **F-759** (ADR 0684): 43 of 7597 mapped tests have no primary; the file-citation stratum fails "because most of those files come from the initial import and the earliest ADR that names one only edits or mentions it".
- **F-760** (ADR 0685): 20 binaries in "Test files" notes appended to ADRs 0004, 0008, 0011, 0024, 0025, 0027, 0031, 0036, 0060, 0067, 0088, 0123; 5 in ADR 0685 itself (`mesh_params`, `channel_wall_function`, `physics_cases`, `physics_workflow`, `table_expr`). "Most were committed on 2026-10-13 or 2026-10-14 without an ADR". 109 tests gained an ADR.
- **F-768** (ADR 0690): one batched `git log -M --diff-filter=AR --name-status` pass (0.44 s, 2 git processes); "522 of 522 E2-only tests null" on a shallow clone and an archive copy.
- **F-772 / F-785** (ADRs 0701, 0703): tests with an ADR and no primary 559 to 351 (H1 to H3), then 351 to 405 when H3 was removed. "4 readings are borderline and would give 80 % if read the other way" (ADR 0703).
- **F-786** (ADR 0704): the second reader's own reading would give "S45 4 of 12 (33 %), S25 13 of 16 (81 %) and together 17 of 28 (61 %)".
- **F-769** (ADRs 0705 to 0710): 0707 records "1e-13 wall distance, y+ 5e-3, wall shear 2e-3, interior 0.03 against 2.1 to 2.4 percent recorded"; 0708 "bulk velocity within 10 percent of 17.4 against 18.17 recorded"; 0710 "1e-15 absolute, NaN outside the range".
- **F-773** (ADR 0720): 5695 tests in 950 files, 6089 links, 271 links in 216 tests naming an ADR the test did not have; "the population is 2 files (948 of the 950 files with E8 already have an E0 primary)".
- **F-839** (ADR 0721): "Agreement expected by chance on this near one-sided sample is 90.5 %, so the result says little about the criterion's power to separate readers; the readers are agents of the same kind".

## Open questions

- F-773 gap names "The maintainer's question on F-787 and F-793" as open; ADR 0704 records the maintainer's answer of 2026-11-18 to the definition those items rest on (see the contradiction note on [[test-to-adr-index]]).
- ADR 0704 Q2 (ADR 0685 as a source: recommendation (b), exclude its own units), Q3 ("Test files" notes whose host only names the item: recommendation (c) where F-769 covers the file, else (b)) and Q4 (a common criterion before a third reading, F-788) carry recommendations and are not answered in the ADR one by one.
- ADR 0720 remaining questions 4 and 5 (items named only in a roadmap note as E8 evidence, F-840; a second reader, done as F-839) and ADR 0721 remaining question 3 (criterion (b) "by name" against "states the limits": recommendation (b) in the next registration that adopts the criterion).
- The registrations of ADRs 0679, 0684, 0685, 0690, 0701, 0703 and 0704 each say "Not yet accepted by the maintainer".

## Related

- [[test-to-adr-index]], [[gate]], [[adr]], [[registration]], [[process-report]]
- [[adr-thread-daily-gate-and-edit-lock]]
- [[roadmap-quality-docs-and-validation]]
