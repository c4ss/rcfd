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

# Reference data: Wave 18C particle tracking (F-056, ADR 0028)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Wave 18C particle tracking (F-056, ADR 0028)" (lines 763-775) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Wave 18C particle tracking references (ADR 0028): Schiller-Naumann drag and its switch.

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


Every reference value of the registered checks is a closed form derived in the ADR (terminal velocity, exponential relaxation, Couette and solid-body-rotation particle motion, ballistic bounce and landing). Published correlations used by the code:

| Quantity | Source | Status |
| --- | --- | --- |
| Stokes drag $C_D = 24/Re$, $\tau_p = \rho_p d^2/(18\mu)$, terminal speed $d^2 (\rho_p - \rho_f)\, g/(18\mu)$ | standard | derived |
| Schiller-Naumann drag factor $1 + 0.15\, Re^{0.687}$ (and its Stokes limit) | arXiv:1506.00478 and a published molecular-dynamics code's documentation, found by web search during wave 18C; original Schiller and Naumann (1933/1935) not accessed | coefficients verified (secondary); validity limit quoted as Re 800 by some sources and 1000 by others, the switch to $C_D = 0.44$ at 1000 is UNVERIFIED |
| Rosin-Rammler oversize mass fraction $Y = \exp\left(-(d/\bar{d})^n\right)$, $\bar{d}$ at $Y = 1/e$ | two independent published references (a CFD manual and an engineering reference page) found by web search; Rosin and Rammler (1933) not accessed | verified (secondary) |
| Inversion sampler $d = \bar{d}\, (-\ln u)^{1/n}$ | derived from $Y(d) = u$ | derived |
| SplitMix64 constants | Steele, Lea and Flood 2014, from memory | unverified; irrelevant to correctness (any 64-bit generator with the same interface works), only reproducibility is claimed |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
