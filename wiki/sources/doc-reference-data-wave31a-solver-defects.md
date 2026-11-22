---
type: source
tags: [reference-data, wave-section]
sources: []
origin: docs/reference-data.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Reference data: Wave 31A: solver defects found by the wave 30 validation (ADR 0062)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Wave 31A: solver defects found by the wave 30 validation (ADR 0062)" (lines 1325-1340) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Wave 31A: solver defects found by the wave 30 validation (ADR 0062).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No new external source was read in this wave. All limits were taken unchanged from ADR 0060 and ADR 0061. The reference values used are the ones already in the repository from waves 30B and 30C:

- de Vahl Davis 1983 Table V, from the page scan.
- The cylinder literature rows of `validation-data/reference/cylinder_re100_literature.csv`, St 0.164 to 0.167 and C_D 1.314 to 1.370.
- The Sod exact values of Marzouk.

Results of this wave computed by rcfd (not literature, not validation data):

- The Ra 1e6 cavity diagnosis: converged 32 x 32 value v_max 220.48, Nu 8.873 at alpha_u 0.5.
- The cylinder time-step and mesh study: St extrapolated to zero time step and mesh 0.1669, C_D 1.357 by a fitted model with first-order time and second-order space terms, versus the unchanged literature windows.


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
- [[cylinder-re100]]
- [[differentially-heated-cavity]]
