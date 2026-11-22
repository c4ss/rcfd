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

# Reference data: Wave 13 numerics and wall motion (F-211, F-208, F-204, ADR 0016)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Wave 13 numerics and wall motion (F-211, F-208, F-204, ADR 0016)" (lines 639-656) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Wave 13 numerics and wall motion references (ADR 0016), all from memory.

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


Every literature statement below was written from memory while building; none was checked against a source in this wave (no web access to the papers). They are marked unverified and none of the registered checks depends on them: the checks use exact solutions (manufactured, linear profiles, the Taylor-Couette solution derived in the ADR) or equivalences between options.

| Item | Source | Status |
| --- | --- | --- |
| Gamma scheme (normalised-variable blend, beta_m range 0.1 to 0.5) | Jasak, Weller and Gosman (1999), Int. J. Numer. Meth. Fluids 31:431 | unverified (memory); bounds measured in W1, not assumed |
| Compressive family psi_k(r) = max(0, min(k r, 1), min(r, k)); k = 1 minmod, k = 2 superbee | Roe (1985), Sweby (1984) | unverified (memory); the extension to k in [0, 1) is project-defined |
| Convection boundedness criterion | Gaskell and Lau (1988) | unverified (memory); not used as a pass criterion |
| Bounded central differencing equals minmod in normalised variables | Jasak (1996) thesis | unverified (memory); equivalence to `Tvd(Minmod)` is tested bitwise (B3), the literature identification is not claimed |
| `Bdf2Bounded` time scheme | project-defined | no source; accuracy measured (T1) and boundedness measured (T2) |
| Face-gradient correction (compact difference replaces the component along the cell-centre line) | Jasak (1996) thesis; Mathur and Murthy (1997) | unverified (memory); effect measured in M1 to M4 |
| Maxwell slip law and the slip length l = lambda (1 + s)/(1 - s) | Maxwell (1879) | unverified (memory); D3 checks the discrete law against its own closed form, not the slip law |
| Thermocapillary layer u = (tau h/(4 mu)) (3 xi^2 - 2 xi) | derived in ADR 0016 (parallel flow, zero net flux) | derived, checked by hand in the ADR; no external source |
| Taylor-Couette u_theta = A r + B/r and torque 4 pi mu B | derived in ADR 0016 (standard solution) | derived; D5b (rigid rotation) separately confirms the torque evaluation has no spurious transpose term |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
