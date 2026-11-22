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

# Reference data: F-419 three-node flat shell Shell3 (ADR 0454)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-419 three-node flat shell Shell3 (ADR 0454)" (lines 1671-1686) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-419 three-node flat shell Shell3 (ADR 0454).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read for F-419; every item is cited from memory and re-derived in ADR 0454 (arithmetic 1 to 8, the prototype `f419-scratch/proto.py`). The one number taken from a source is the stabilisation factor `alpha = 0.1`, taken once and not varied.

| Item | Status | Source |
|---|---|---|
| Constant-strain membrane of a triangle, `B` from the shape gradients `(b_i, c_i)/(2A)`; exact for a linear displacement | derived | first principles, ADR 0454 arithmetic 2 |
| MITC3 transverse shear: the assumed shear in the lowest-order edge space `gamma = (a1 - b Y, a2 + b X)`, tied to the tangential shear at the edge mid-points, `g_e = (w_j - w_i)/l_e + (beta_i + beta_j)/2 . t_e` | derived | P.-S. Lee and K.-J. Bathe, Comput. Struct. 82 (2004) 945-962; J. C. Nedelec, Numer. Math. 35 (1980) 315: NOT read; the tying re-derived in ADR 0454 arithmetic 4 |
| Shear stabilisation `H t^2/(t^2 + alpha h^2)`, `alpha = 0.1`, `h` the longest edge | from memory | M. Lyly, R. Stenberg and T. Vihinen, Comput. Methods Appl. Mech. Engrg. 110 (1993) 343-357; K.-U. Bletzinger, M. Bischoff and E. Ramm (DSG3), Comput. Struct. 75 (2000) 321-334: NOT read; plain MITC3 measured 0.8988 of Navier at `t/a = 0.001` (N4) |
| Drilling penalty tying `theta_z` to the membrane rotation `omega = (dv/dx - du/dy)/2` | from memory | Hughes and Brezzi, as ADR 0432: NOT read; Shell4's factor `DRILLING_FACTOR` reused |
| Navier series of the simply supported square plate under uniform pressure, `w_c = c p a^4/D`, `c = 0.004062` | derived | Timoshenko and Woinowsky-Krieger, Theory of Plates and Shells, as ADR 0432: NOT read; the series summed in ADR 0454 (SS4) |
| Consistent triangle mass `rho t A (1 + delta_ij)/12`, rotary inertia `rho t^3/12` on the three rotations | derived | the integral of `N_i N_j` over the triangle, ADR 0454 arithmetic 6 |
| Thin cylinder `u_r = p R^2/(E t)`, cantilever tip `P L^3/(3 E I)`, pinched ring diameter change | derived | membrane equilibrium and beam theory, as ADR 0432 |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
