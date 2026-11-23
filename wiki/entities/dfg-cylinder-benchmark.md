---
type: entity
tags: [validation, benchmark, immersed-boundary]
sources: [doc-reference-data-f219-immersed-solid, doc-reference-data-f355-immersed-piso, doc-reference-data-f354-dfg-2d2-extrema]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# DFG flow around a cylinder (2D-1, 2D-2)

**What it is:** the DFG benchmark of flow around a cylinder in a channel (FeatFlow pages, read 2026-11-07), used to check rcfd's immersed solid (F-219 and its follow-ups) ([[doc-reference-data-f219-immersed-solid]]).
**Where:** immersed-solid tests of ADRs 0218, 0234, 0239, 0249, 0254, 0268.

## Details

- 2D-1 set-up: channel `[0, 2.2] x [0, 0.41]`, cylinder centre `(0.2, 0.2)`, radius 0.05, rho 1, nu 1e-3, parabolic inflow with U 0.3, U_mean 0.2, Re 20. Reference values "computed with high order spectral methods": C_D = 5.57953523384, C_L = 0.010618948146, p_diff = 0.11752016697 ([[doc-reference-data-f219-immersed-solid]]).
- 2D-2 (Re 100, U 1.5, U_mean 1), level-6 values at k = 1/200: St 0.30188, 1/f 0.33125, max/min/mean C_D 3.2200 / 3.1569 / 3.1884, max/min C_L 0.9859 / -1.0206 ([[doc-reference-data-f219-immersed-solid]]).
- The Schaefer and Turek (1996) bounds were not obtained and are not used ([[doc-reference-data-f219-immersed-solid]]).
- Measured with the bounded second-order forcing (ADR 0254): St 0.30112, max C_D 3.20930, max C_L 0.97593 ([[doc-reference-data-f355-immersed-piso]]). On D/80 meshes (ADR 0268), e.g. M3 bounded 3.22024, 0.98167, 0.30168 ([[doc-reference-data-f354-dfg-2d2-extrema]]).

## Related

[[cylinder-re100]], [[doc-reference-data]]

## Open questions

- None recorded in these sections.
