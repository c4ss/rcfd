---
type: concept
tags: [foundation, mesh, verification]
sources: [ref-jasak-1996-phd-thesis, ref-venkatakrishnan-1995-icase-95-3, ref-mit-ocw-2-29-numerics]
builds_on: [gradient-reconstruction, truncation-error-and-order-of-accuracy]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# A-posteriori error indicators and estimates

## In short

After a solution has been computed, look at it and guess where the discretisation error is large, so the mesh can be refined there (and coarsened elsewhere). Cheap indicators just look at large gradients or curvatures; error estimates try to give the size of the error as well as where it is ([[ref-jasak-1996-phd-thesis]] 4.1).

## Explanation

**Indicators against estimates.** Error indicators are "Combinations of gradients of flow variables": cheap, they mark regions that need resolution, but give no absolute error level ([[ref-jasak-1996-phd-thesis]] 4.1.1). An error estimate should give both the level and the distribution of the error, be cheap and local (cell by cell), be asymptotically exact as the mesh is refined, and over-estimate only modestly ([[ref-jasak-1996-phd-thesis]] 4.1).

**Common choices.**
- Heuristic criteria such as undivided density gradients, or an estimate of the truncation error, are the usual way to pick cells for h-refinement in compressible flow ([[ref-venkatakrishnan-1995-icase-95-3]] 7).
- Richardson extrapolation uses solutions on two (or more) meshes and the known order of the scheme ([[ref-jasak-1996-phd-thesis]] 4.2; [[ref-mit-ocw-2-29-numerics]] L12 s.5-8; see [[truncation-error-and-order-of-accuracy]]).
- A residual estimate measures how well each cell's solution, reconstructed with its gradient, satisfies the original differential equation ([[ref-jasak-1996-phd-thesis]] 4.3.1).

**Pitfall.** Refining purely to flow features can over-resolve some regions, "resulting in seemingly sharp but incorrect solutions"; Warren et al. modified the indicators against this ([[ref-venkatakrishnan-1995-icase-95-3]] 7).

**Marking cells.** With the error scaling as $h^2$, a desired local cell size follows from a target error. Alternatively each cell's error is compared with the domain mean, using a refinement factor above and an unrefinement factor below it; refinement normal to the solution gradient is preferred where the solution varies mainly in one direction ([[ref-jasak-1996-phd-thesis]] 5.2).

## Builds on

- [[gradient-reconstruction]]: the gradients most indicators are built from.
- [[truncation-error-and-order-of-accuracy]]: why the error scales with a power of $h$.

## Leads to

- rcfd pages: [[mesh-adaptation]], [[gci-richardson]]

## How rcfd uses it

- rcfd's `AmrMesh` offers `gradient_indicator`, `hessian_indicator`, `zz_indicator` ($\eta = \lvert g^* - g \rvert \sqrt{V}$) and `region_indicator`, and marks cells with a dead band (`coarsen_below < refine_above`) and level limits ([[mesh-adaptation]]).
- Directional refinement (`AnisoMesh`, one level per axis) on the suction layer reached error 0.00298 with 1547 cells against 0.01159 with 3841 isotropic cells ([[mesh-adaptation]]).

## Open questions

- The Hessian and Zienkiewicz-Zhu recovery indicators used by rcfd are not described in the sources read for this page.
