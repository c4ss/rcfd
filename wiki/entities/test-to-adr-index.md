---
type: entity
tags: [process, script, gate, testing, attribution]
sources: [adr-thread-test-to-adr-attribution, adr-thread-daily-gate-and-edit-lock]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Test-to-ADR index (scripts/adr_test_index.py)

**What it is:** the stdlib-only script that finds every test statically and maps it to the ADRs and items that describe it, with one **primary ADR** per test (the registering ADR, the one the gate prints first when the test fails) ([[adr-thread-test-to-adr-attribution]]).
**Where:** `scripts/adr_test_index.py`, tests `scripts/test_adr_test_index*.py`; read by `scripts/gate_attribute.py` in the daily gate and by `push_main.py --adr` ([[adr-thread-daily-gate-and-edit-lock]]).

## Details

- Built with the daily gate (F-723, ADR 0653): 501 of 501 binaries found; 1053 of 1134 integration-test binaries (92.9 %) and 6986 of 7472 tests (93.5 %) mapped at that time ([[adr-thread-daily-gate-and-edit-lock]]).
- Mapping evidence: file-name item, ADR and roadmap citations, wave tags (`w34a`, `wave17a`; bare `w31` ambiguous, F-737), the first `ADR NNNN` of a file's header, "Test files" notes appended to old ADRs (F-760); after F-760 no integration binary with tests is unmapped ([[adr-thread-test-to-adr-attribution]]).
- Primary tiers: E0 (header ADR, or an ADR path in the leading comment, ADR 0701) > E1 > E6 > E8 (ADR or item named by the commit that first adds the file, ADR 0720) > E2 (file citation, resolved by the first commit that adds the file, `--primary git`, ADR 0690). Retro-registration units H1 and H2 give a primary to file-citation-only tests; H3 was removed (ADR 0703) and its code deleted (ADR 0712) ([[adr-thread-test-to-adr-attribution]]).
- On a shallow clone or a `git archive` copy the git-based primaries are null (history `shallow` or `none` in `coverage.primary.git`) ([[adr-thread-test-to-adr-attribution]]).
- Definition (maintainer, 2026-11-18, ADR 0704 Q1 (a)): "The primary is the ADR that decided the file's checks, and where there is none the file has no primary" ([[adr-thread-test-to-adr-attribution]]).
- Reading agreement: second reader 21 of 28 (75.0 %, limit 85, F-786); later readings against a written criterion 19 of 20, 18 of 18, 22 of 22 ([[adr-thread-test-to-adr-attribution]]).

> [!warning] Contradiction
> The roadmap gap of F-773 (in [[roadmap-quality-docs-and-validation]]) says "The maintainer's question on F-787 and F-793 is open", while ADR 0704's "Answers of the maintainer (2026-11-18)" decide the definition they rest on and say "F-787 and F-793, which rest on this definition, may be issued" ([[adr-thread-test-to-adr-attribution]]). Within the repo docs (the gap was written on 2026-11-17, before the answer).

## Related

[[gate]], [[adr]], [[registration]], [[process-report]], [[push-main]], [[adr-thread-test-to-adr-attribution]]

## Open questions

- A third reading against one written criterion (F-788), the retro unit "no ADR describes the file" giving no primary (F-787), the primary of files a retro ADR describes (F-793), items named only in roadmap notes as E8 evidence (F-840): all todo ([[adr-thread-test-to-adr-attribution]]).
