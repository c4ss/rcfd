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

# Reference data: Wave 32C: design-loop infrastructure (ADR 0067, docs/design-loop.md)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Wave 32C: design-loop infrastructure (ADR 0067, docs/design-loop.md)" (lines 1358-1372) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Wave 32C: design-loop infrastructure references and measured optimiser results (ADR 0067).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No measured data and no vendor material were read or used in this wave.

| Item | What was read (source) | Result |
|---|---|---|
| Nelder-Mead simplex | Nelder and Mead (1965) NOT read; standard coefficients 1, 2, 0.5, 0.5 and the 5 % initial simplex from general knowledge | implemented and checked on Rosenbrock (20 of 20 seeds, f < 1e-18); no claim about the original text |
| Differential evolution | Storn and Price (1997) NOT read; DE/rand/1/bin with F = 0.5, CR = 0.9 from general knowledge | checked on Rastrigin d = 4 (20 of 20 seeds, f = 0) |
| Deb's feasibility rules and fast non-dominated sorting | Deb (2000) and Deb et al. (2002) NOT read; rules as commonly stated | checked on analytic fronts (ZDT1-like, quarter circle, plane) |
| Latin hypercube | McKay, Beckman, Conover (1979) NOT read | stratification property tested exactly |
| Sobol sequence, Joe-Kuo direction numbers | Sobol (1967), Antonov and Saleev (1979), Joe and Kuo (2008) NOT read; the direction numbers for dimensions 2 to 10 were recalled from memory | UNVERIFIED as a transcription; verified only by the net properties (first 8 points of dimensions 1 and 2 exact, power-of-two stratification in all 10 dimensions, (0, m, 2)-net of the first two dimensions, L2-star discrepancy 0.0051 against a random mean of 0.0131) |
| Warnock's L2-star discrepancy formula | from memory | used only as a relative comparison between Sobol and random points |
| xoshiro256** and SplitMix64 | Blackman and Vigna generators recalled from memory | used for seeded reproducibility only; no statistical quality claim beyond the stratification and determinism tests |
| Contraction channel (E1) | no reference data; the 40 % claim of the ADR is an estimate from a lubrication argument | computed by rcfd: pressure drop 9.94 to 5.07 (49 %) with the outlet velocity ratio at the constraint 1.3 |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
- [[design-loop]]
