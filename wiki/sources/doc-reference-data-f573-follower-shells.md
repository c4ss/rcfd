---
type: source
tags: [reference-data, structural]
sources: []
origin: docs/reference-data.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Reference data: F-573 follower pressure on shells (ADR 0487)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-573 follower pressure on shells (ADR 0487)" (lines 2015-2025) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-573 follower pressure on shells (ADR 0487).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read for F-573. Every formula used is derived in ADR 0487 (arithmetic 1 to 8) or re-used from ADR 0480 (cited there from memory, NOT read); the references of the checks are closed forms and the dead `ShellPressure` of the repository.

| Item | Status | Source |
|---|---|---|
| Follower shell pressure on the interpolated current mid-surface $f_a = +p \int N_a (x_u \times x_v) du dv$ (translations, no moments) and its load stiffness $K_{(ai)(bj)} = p \int N_a [N_{b,u} (e_j \times x_v) + N_{b,v} (x_u \times e_j)]_i$; the solid face arithmetic of ADR 0480 with the pressure negated | derived | virtual work of the pressure on the current surface, ADR 0487 arithmetic 1 and 2 (the linearisation as in ADR 0480 arithmetic 1) |
| Vector area of a bilinear quad $(1/2)(x_2 - x_0) \times (x_3 - x_1)$, equal to that of its projected facet: the resultants of the follower and the dead shell pressure agree on a warped Shell4 | derived | ADR 0487 arithmetic 5 |
| Cylinder of $N$ facets at the stretch $\lambda$: $\sum f \cdot e_r = p N \lambda R L \sin(2 \pi/N)$ (follower), $p N R L \sin(2 \pi/N)$ (dead); the hoop tension $p \lambda R \cos(\pi/N)$ and, with an SVK membrane of $\nu = 0$, $p = E t (\lambda^2 - 1)/(2 R \cos(\pi/N))$ | derived | node balance of the facet polygon, ADR 0487 arithmetic 6 |
| Ring buckling of shells under external pressure: $3 E I/R^3$ with the load stiffness (hydrostatic), $4 E I/R^3$ without (fixed-direction forces), $n = 2$ | derived | ADR 0480 arithmetic 3 re-used (ADR 0487 arithmetic 7) |

## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
