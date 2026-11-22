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

# Reference data: F-426 tied non-matching meshes, beam and shell joints to solids, cyclic extensions (ADR 0456)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-426 tied non-matching meshes, beam and shell joints to solids, cyclic extensions (ADR 0456)" (lines 1687-1700) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-426 tied non-matching meshes and joints (ADR 0456).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


| Item | Status | Source |
|---|---|---|
| One-pass node-to-surface tie fails the patch test on non-matching meshes; at 2:3 the slave trapezoid weights give a master corner `5/18` of the face width instead of `1/4`, the middle node `4/9` instead of `1/2` | derived | ADR 0456 arithmetic 1; Taylor and Papadopoulos 1991 (patch test for contact): NOT read, from memory; El-Abbasi and Bathe 2001: NOT read |
| NTS bar of two Hex8 parts (2:3, 4:6, 3:5, 6:10), far-end error and interface stress errors (`2.8797762239e-3` ... `5.6804678473e-2`) | computed | independent numpy prototype `f426-scratch/proto_tie.py`, ADR 0456 P; the Rust tie agrees within `4e-11` |
| Mortar tie `D u_s = M u_m` with the slave face functions as multipliers, segment integration on the clipped slave faces; passes the flat patch test exactly | derived | ADR 0456 arithmetic 2, 3; Puso and Laursen 2004 (segment-to-segment mortar), Wohlmuth 2000 (mortar finite elements): NOT read, from memory |
| Sutherland-Hodgman polygon clipping | from memory | Sutherland and Hodgman 1974: NOT read; the algorithm restated in `tie::clip_polygon` |
| Distributing coupling of a beam or shell node: the least-squares rigid motion of a node set, `A (t, theta) = sum B_i^T u_i` | derived | first principles, ADR 0456 arithmetic 4 |
| Cyclic sector, node on the axis: held unless `k = 0` (axial free) or `k = 1` doubled (`b = n x a` in-plane free) or `N = 2`, `2k = N` | derived | ADR 0456 arithmetic 5; Thomas 1979 (dynamics of rotationally periodic structures): NOT read, from memory |
| Rotating solid disc in plane stress, centre `(3 + nu) rho Omega^2 b^2 / 8`, field `sigma_r = (3 + nu) rho Omega^2 (b^2 - r^2)/8`, `sigma_theta = rho Omega^2 ((3 + nu) b^2 - (1 + 3 nu) r^2)/8` | derived, from memory | Timoshenko and Goodier, Theory of Elasticity: NOT read; equilibrium and compatibility checked, ADR 0456 arithmetic 6 |
| Regular polygon section `A = (M/2) a^2 sin(2 pi/M)`, `I = A a^2 (2 + cos(2 pi/M))/12` | derived | ADR 0456 arithmetic 7 |
| Timoshenko cantilever with `kappa = 10 (1 + nu)/(12 + 11 nu)` (rectangle), `6 (1 + nu)/(7 + 6 nu)` (circle) | from memory | Cowper 1966: NOT read |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
