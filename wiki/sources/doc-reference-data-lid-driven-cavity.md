---
type: source
tags: [reference-data, benchmark]
sources: []
origin: docs/reference-data.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Reference data: Lid-driven cavity (Ghia, Ghia and Shin, J. Comput. Phys. 48 (1982) 387-411)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Lid-driven cavity (Ghia, Ghia and Shin, J. Comput. Phys. 48 (1982) 387-411)" (lines 43-56) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Ghia, Ghia and Shin (1982) centreline velocities used for the lid-driven cavity, verified against secondary copies, with a recorded misprint in the Re 400 v table.

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


Used in `cases/cavity/reference/*.csv`, `crates/rcfd-fv/tests/cavity.rs`, `rcfd-io` cases test.

| Value | Source | Result |
|---|---|---|
| u centreline, Re 100/400/1000, 16 rows y = 0.0547 ... 0.9688 (48 numbers) | (a) a student course repository, github.com/PN-CourseWork/02689-AdvancedNumericalAlgorithmP3 `data/validation/ghia/ghia_Re{100,400,1000}_u_centerline.csv` (this file omits y = 0.9766); (b) Uppsala bachelor thesis diva2:1668016, Table 2, which prints all 17 rows for Re 100/400/1000 (its y labels are shifted by one row by the PDF text extraction, but the value columns match our CSV row for row in order); (c) for Re 1000 also arXiv:1807.04896 Table A-1, "Ghia, Table 1, p. 398" | verified, two independent secondary copies, exact match of every number |
| u at y = 0.9766: 0.84123 / 0.75837 / 0.65928 | thesis (b) and, for Re 1000, arXiv:1807.04896 | verified (Re 1000 twice, Re 100/400 once; wave 31C: a third copy, gist ivan-pi/3e9326d18a366ffe6a8e5bfda6353219 (Table I typed by a third party), has all three, so Re 1000 three times and Re 100/400 twice) |
| v centreline Re 100, 17 rows | course repository `ghia_Re100_v_centerline.csv`; thesis (b) v-table (same shifted-label extraction; values in order -0.05906 ... 0.09233 match, including 0.05454, 0.17527, -0.24533) | verified, two secondary copies, exact match |
| Only Re 100 v is stored; Re 400/1000 v are not stored | n/a | not applicable. Wave 31C: the Re 400 and 1000 v tables (and u for all three Re, including y = 0.9766) were read from three secondary copies and stored in `validation-data/reference/w31c_ghia1982_{u,v}_centerline_read.csv`; see the wave 31C section and its discrepancy list |

Both secondary copies carry the known printed misprint of the Re=400 v table at x = 0.9063 (-0.23827; the thesis marks it in red). rcfd does not use Re 400 v.


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
- [[lid-driven-cavity]]
