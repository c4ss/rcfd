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

# Reference data: Cylinder wake (`crates/rcfd-fv/tests/cylinder.rs`)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Cylinder wake (`crates/rcfd-fv/tests/cylinder.rs`)" (lines 200-208) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Cylinder wake at Re 100: Strouhal reference values, the Williamson 1989 fit and the recorded run.

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


| Value | Source | Result |
|---|---|---|
| St = 0.165 at Re = 100 | Williamson, Annu. Rev. Fluid Mech. 28 (1996), reported as 0.165 in the Re = 100 table of arXiv:2303.09262; same table: Kravchenko et al. 0.164, Posdziech and Grundmann 0.164, Sivakumar et al. 0.164, Oliveira 0.167; a vendor validation manual quotes the same 0.165 (not used as a source) | verified within 1 percent |
| "Williamson 1989 about 0.164" and "Henderson 1995 about 0.165" | wave 31C: Williamson, J. Fluid Mech. 206 (1989) 579-627, eq. (6), read in the author copy https://authors.library.caltech.edu/records/wavw6-f7k56/files/WILjfm89a.pdf: S = A/Re + B + C Re with A = -3.3265, B = 0.1816, C = 1.600e-4 (parallel shedding, study range Re 49 to 178), which gives St(100) = 0.164335 | Williamson 1989 about 0.164: verified (primary, computed from the printed fit; the paper prints no value at exactly Re = 100). Henderson 1995 about 0.165: still unverifiable. The CSV row "Williamson (experiment) 0.165" is the 1996 review value as quoted in arXiv:2303.09262; it differs from the 1989 fit by 0.4 % |

The 5 percent tolerance is unchanged; the recorded run of 0.1588 is -3.8 percent from 0.165 (-3.2 percent from 0.164).


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
- [[cylinder-re100]]
