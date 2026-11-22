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

# Reference data: Wave 30B: incompressible validation data actually read (ADR 0060, retrieved 2026-10-17)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Wave 30B: incompressible validation data actually read (ADR 0060, retrieved 2026-10-17)" (lines 1256-1273) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Wave 30B: incompressible validation data actually read on 2026-10-17 (ADR 0060).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


Wave 30B read every row below as raw text (`curl` plus `pdftotext -layout`, or the raw data file). The numbers are stored in `validation-data/reference/` and registered in `validation-data/manifest.json`. Nothing here is typed from memory.

The measured results of wave 30B against the registered limits are in the addendum of ADR 0060. Known misses are the ignored `*_known_miss` tests in `crates/rcfd-api/tests/validation_*.rs`.

| Value | Source and what was read | Result |
|---|---|---|
| Cavity Re 1000, u along x = 0.5, 15 rows, Ghia next to Botella and Peyret spectral values (largest difference 0.0063) | arXiv:1807.04896 Table A-1 ("Ghia, Table 1, p. 398" and "Botella, Table 9, p. 426") | verified (secondary copy of both); used in `validation_cavity_ghia.rs` |
| Cavity Re 100 and 400 u, 16 rows each | raw CSV in github.com/PN-CourseWork/02689-AdvancedNumericalAlgorithmP3, equal to the in-repo table to the last digit | verified (secondary, student repository); the Ghia 1982 original, and Ghia v at Re 400 and 1000, were NOT obtained |
| Backward step ER 2, Re_D = 800: ten published x_r/h values 11.48 to 12.20 | arXiv:2507.16509 Table 2 re-read in full (Rogers and Kwak, Barton, Erturk, Kim and Moin, Lee and Mateescu, Guj and Stella, Grigoriev and Dargush, Keskar and Lyn, Gartling, Gresho et al.) | verified as a quotation in a 2025 paper; the originals including Gartling (1990) and Erturk (2008) were NOT read. Armaly et al. (1983): data not obtained (host timed out, Cambridge paywalled), no check |
| Blasius f''(0) = 0.33205733621519629893718, f'(eta) at 1..5, delta* = 1.72078765752 sqrt(nu x/U) | arXiv:2204.02215 (Lal and Martin 2022) Table 1 and benchmark tables, 32-digit Taylor series | verified; Cf = 0.664115/sqrt(Re_x) derived from f''(0) |
| Channel DNS Re_tau = 178.12: U_c+ 18.301, U_b+ 15.6787, U+ at y+ = 5, 10, 20, 30, 50, 100, 150 | `chan180.means` of Moser, Kim and Mansour at torroja.dmt.upm.es/turbdata/agard/chapter5/PCH10/chan180/profiles/ (65 points, read and integrated); only the sampled numbers are stored (redistribution terms not stated) | verified (file read) |
| Cylinder Re 100: C_D 1.314 to 1.370, St 0.164 to 0.167, experiment St 0.165 | arXiv:2303.09262 Table 1 (Newtonian rows) and arXiv:1401.6513 Table 3 | verified as quotations; the original papers were NOT read |
| Moody, Colebrook, Dean friction correlations; Gartling and Ghia originals; Armaly | not obtained | still unverified or data not obtained; no wave 30B check uses them |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
- [[lid-driven-cavity]]
- [[backward-facing-step]]
- [[flat-plate]]
- [[cylinder-re100]]
