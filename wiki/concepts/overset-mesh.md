---
type: concept
tags: [mesh, moving-mesh, overset, chimera]
sources: [doc-dynamic-mesh, doc-readme]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Overset (chimera) mesh

A component mesh moving over a background mesh, the two coupled through interpolation in fringe cells, with hole cells cut out of the background. In rcfd: library only (F-134, ADR 0219), one rigidly moving component over a static background, `rcfd_mesh::overset`, `rcfd_fv::overset::OversetSolver` ([[doc-dynamic-mesh]]). Not available from a case or the app ([[doc-readme]]).

## How rcfd uses it

- **Composite mesh** by `rcfd_mesh::overset::compose`; each step `Overset::connectivity` sorts cells into calculated, fringe and hole (user hole function) ([[doc-dynamic-mesh]]).
- **Implicit coupling:** fringe rows of momentum and pressure are the interpolations (one system for both meshes); weights reproduce linear fields ([[doc-dynamic-mesh]]).
- **Donor search** on warped and non-convex cells (F-352, ADR 0253; F-356, ADR 0260): every one of 100000 random points in exactly one cell ([[doc-dynamic-mesh]]).
- **Mass:** `MassMode::Conservative` (one offset of the background fringe pressures makes the net flux into the hole region zero, so the domain keeps its mass) vs `MassMode::Interpolated` (overlap non-conservative; without the offset the cylinder case loses `2.7e-3` of `U D L`) ([[doc-dynamic-mesh]]).
- **Checks:** cylinder Re 40 moving 7.5 D: net outflow `7.5e-15`, drag within 0.62 percent of body-fitted (limit 5 percent); Taylor-Green interpolated-cell order 1.62 (registered 1.7, missed); background order 1.98 ([[doc-dynamic-mesh]]).
- **Opt-ins** (each bit for bit off by default): outer iterations (F-335, ADR 0238), prescribed fringe flux (F-445, ADR 0347), second-order ring gradient (F-479, ADR 0409), pose-continuous donors (F-468, ADR 0396), geometry corrections (F-368, ADR 0367; F-472, ADR 0382; F-487, ADR 0387), BDF2 (F-338, ADR 0241) ([[doc-dynamic-mesh]]).
- **BDF2 remainder:** time error does not fall with the square of the step (ratio 2.46 vs 5); the same on a single mesh (ratio 2.30) ([[doc-dynamic-mesh]]).
- **Not available:** a second component, moving background, hole cutting from wall geometry, non-orthogonal correctors, linear-upwind convection, turbulence, energy, the case tree ([[doc-dynamic-mesh]]).

## Related

- [[dynamic-mesh]]
- [[sliding-mesh]]
- [[rcfd-fv]]
- [[rcfd-mesh]]
- [[known-miss]]

## Open questions

- Tree use is ADR 0219 Q1 ("The tree does not use it yet") ([[doc-dynamic-mesh]]).
