---
type: source
tags: [interface, case, numerics, fan]
sources: [doc-case-schema]
origin: docs/case-schema.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Case schema: least-squares velocity gradient on periodic faces

**Origin:** [docs/case-schema.md](../../docs/case-schema.md), section "Least-squares velocity gradient on periodic faces (`solver.periodic_gradient`)", at `origin/main` 2e94e5ef. Hub: [[doc-case-schema]]
**Kind:** reference section (with the subsection on high-aspect wall cells)

## Summary

`solver.periodic_gradient` (`face_value` default, `partner_cell`) for the steady momentum gradient (ADR 0077 F-H0), and why `linear_upwind` diverges on high-aspect, strongly non-orthogonal wall cells (ADR 0077 part B, ADR 0171).

## Key points

- `face_value` on a skewed blade-passage seam is wrong by O(1) of the gradient even for a linear field ("0.17 of it on the FAN-01 sector mesh"); `partner_cell` is exact for linear fields. Opt-in.
- FAN-01 H-slit sector (13 332 cells, aspect up to 944, non-orthogonality up to 79 degrees): `linear_upwind` and `tvd_van_leer` diverged; `upwind` and `tvd_minmod` were stable.
- NACA 0012 O-grid at 6 degrees stops in 15 iterations (SA; 16 with SST) at trailing-edge cells with aspect x tan about 21 000; `barth_jespersen` removes it at about 3 % of `cd_eff` (ADR 0171 LL4).

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[fan-01]], [[setup-rules]]
