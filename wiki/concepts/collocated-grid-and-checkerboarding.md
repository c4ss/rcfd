---
type: concept
tags: [foundation, pressure-velocity]
sources: [ref-mit-ocw-2-29-numerics, ref-jasak-1996-phd-thesis, ref-klaij-vuik-2013-simple-preconditioners]
builds_on: [finite-volume-method, pressure-velocity-coupling]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Collocated grids and pressure checkerboarding

## In short

A collocated (non-staggered) arrangement stores pressure and all velocity components at the same point, the cell centre. It is simple and works on any mesh, but with plain linear interpolation the pressure can develop a spurious oscillating pattern ("checkerboard") that the discrete momentum and continuity equations do not see. A staggered grid, with velocities on the faces, avoids this but is awkward on non-orthogonal meshes. Collocated codes add a stabilisation, usually Rhie-Chow interpolation ([[ref-mit-ocw-2-29-numerics]] L17 s.17-20; [[ref-klaij-vuik-2013-simple-preconditioners]] 2.1).

## Explanation

**Collocated arrangement.** All variables share the same control volumes, "usually called the colocated or non-staggered variable arrangement (Rhie and Chow, Peric)" ([[ref-jasak-1996-phd-thesis]] 3.1). Advantages: all geometric coefficients are evaluated at the same points, and multigrid is easy ([[ref-mit-ocw-2-29-numerics]] L17 s.17).

**The difficulty.** Collocated grids were out of favour until the 1980s because of "Occurrence of oscillations in the pressure" and "Difficulties with pressure-velocity coupling", and they require more interpolation ([[ref-mit-ocw-2-29-numerics]] L17 s.18). The pressure-weighted interpolation method is "used to avoid checkerboard oscillations on grids with colocated variables", and without such a stabilisation term the system admits "spurious pressure oscillations" ([[ref-klaij-vuik-2013-simple-preconditioners]] 1, 2.1).

**Staggered alternative.** On staggered grids pressure sits at cell centres and velocity components on the faces; the arrangement "couples p and v", and the pressure and diffusion terms need no interpolation ([[ref-mit-ocw-2-29-numerics]] L17 s.19-20). On non-orthogonal grids for complex geometry, however, it needs grid-oriented velocity components and curvature terms, which are harder to treat and can create non-conservative errors, so collocated grids "became more popular with complex geometries" ([[ref-mit-ocw-2-29-numerics]] L17 s.18).

**Stabilisation.** The pressure-weighted interpolation adds to the face velocity a term `h^2` times a second derivative of the pressure gradient (a third-derivative pressure term per face, fourth order in the continuity equation) that vanishes as `h -> 0` and is scaled not to spoil second-order accuracy; in matrix form it is the block `C`, whose stencil reaches "not only the neighbors of the cell but also the neighbors of the neighbors" ([[ref-klaij-vuik-2013-simple-preconditioners]] 2.1-2.3, Eqs. 2, 7-9). See [[rhie-chow-interpolation]].

## Builds on

- [[finite-volume-method]]: cell-centred unknowns and face interpolation.
- [[pressure-velocity-coupling]]: the constraint that must couple pressure to velocity.

## Leads to

- [[rhie-chow-interpolation]]
- rcfd pages: [[simple]], [[piso]]

## How rcfd uses it

- rcfd's segregated kernels are collocated with Rhie-Chow ([[simple]]).
- The block-coupled solver has a known limit: "a decaying odd-even pressure mode next to a prescribed-flux inlet" (ADR 0029 addendum A3) ([[doc-case-schema-coupled]]).

## Open questions

- None of the sources read for this page spells out the stencil argument for why linear interpolation decouples odd and even cells; a source that does would make the explanation above complete.
