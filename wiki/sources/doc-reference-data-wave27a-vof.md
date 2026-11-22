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

# Reference data: Wave 27A volume of fluid and surface tension (F-055, ADR 0049)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Wave 27A volume of fluid and surface tension (F-055, ADR 0049)" (lines 1099-1113) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Wave 27A volume of fluid and surface tension references (ADR 0049), including Martin-Moyce and Hysing.

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


| Item | Status | Note |
|------|--------|------|
| Martin and Moyce dam-break non-dimensional average front velocity `v / sqrt(g H)` = 1.48 (H = 57 mm), 1.69 (H = 114 mm), averaging window t* > 1 | verified, secondary | Table 1 of a 2014 journal paper (arXiv:1308.0115), page read; its extraction method from the original is not stated; the primary Martin-Moyce 1952 tables were not obtained and the original's time normalisation was not resolved, so no pointwise front table is used |
| Ritter front velocity `2 sqrt(g H)` (upper bound) | derived | frictionless dam-break characteristic solution |
| Laplace jump `dp = sigma / R` (2-D, cylinder) | derived | |
| Zalesak slotted-disk set-up (disk R = 15 at (50,75), slot width 5, 628 steps) | unverified | recalled from the literature; the test uses the registered numbers, not a published table |
| Zalesak flux-corrected-transport limiter (1979), Boris-Book FCT (1973) | unverified | implemented from memory of the published algorithm and validated by boundedness and exact-conservation tests |
| Interface-compression flux (Rusche 2002 thesis, Weller 2008 report form) | unverified | structure recalled, not read |
| CSF surface tension (Brackbill, Kothe, Zemach 1992) | unverified | the method is standard; the formula is validated by the Laplace-jump test |
| Capillary time-step constant `dt_cap = sqrt((rho_1 + rho_2) dx^3 / (8 pi sigma))` | unverified | from memory; the most restrictive reading is used |
| LeVeque (1996) single-vortex deformation field | unverified | recalled; used with the registered period T = 2 |
| Hysing et al. (2009) rising bubble case 1: `y_c(3)` = 1.0813, `v_max` = 0.2417 at t = 0.9213 | verified (wave 39E, ADR 0096: Int. J. Numer. Meth. Fluids 60 (2009) 1259-1288, Table V, TP2D 1/h = 320; Table XI MooNMD 0.2417 / 1.0817; the earlier time 0.9317 was a misrecall) | originally quoted from memory; the registered limits are loose (0.12 in height, 25 percent in velocity) because the 40 x 80 algebraic scheme is far from the references |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
