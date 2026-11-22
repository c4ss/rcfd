---
type: concept
tags: [foundation, turbulence, mesh]
sources: [ref-nasa-tmr, ref-menter-1992-nasa-tm-103975, ref-davidson-turbulence-notes]
builds_on: [finite-volume-method]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Wall distance

## In short

Many turbulence models need, at every cell, the distance `d` to the nearest wall: SST uses it in its blending functions, Spalart-Allmaras in its destruction term, DES compares it with the cell size. It must be the true minimum distance to the wall surface, not the distance along a grid line or to the nearest wall node ([[ref-nasa-tmr]], [[ref-menter-1992-nasa-tm-103975]]).

## Explanation

**Where it enters.**
- SST/BSL: `arg1` and `arg2` of the blending functions contain `sqrt(k)/(beta* omega d)` (turbulent length scale over wall distance) and `500 nu/(d^2 omega)` (a sublayer term); `d` is "the distance from the field point to the nearest wall" ([[ref-nasa-tmr]] `sst.html`; [[ref-menter-1992-nasa-tm-103975]] eq. 12, "the distance to the next surface").
- Spalart-Allmaras: the destruction term scales with `(nu_tilde/d)^2`; the source terms are non-zero even in the free stream, though small, "proportional to 1/d2" ([[ref-nasa-tmr]] `spalart.html`; [[ref-davidson-turbulence-notes]] eq. 20.2).
- DES: `d_tilde = min(d, C_des Delta)` switches between RANS and LES ([[ref-davidson-turbulence-notes]] eq. 20.3; [[les-and-des]]).
- Reynolds-stress models: the SSG/LRR blend uses `d` in an SST-like `F1` ([[ref-nasa-tmr]] `rsm-ssglrr.html`).
- The wall-cell value of `omega` uses the first-cell distance `Delta d1` ([[ref-nasa-tmr]] `sst.html`).

**What "nearest wall distance" must mean.** The TMR is explicit: computing `d` "by searching along grid lines or by finding the nearest wall gridpoint (or cell center) are incorrect, and are not the same as computing actual minimum distance to the nearest wall (in general)"; such shortcuts "will yield grid-dependent differences in the results", especially where grid lines are not normal to the body or the nearest body is in another grid zone. Near a sharp convex corner or edge (a trailing edge) the correct distance is to that corner or edge, "which is not a wall normal" ([[ref-nasa-tmr]] `spalart.html`). In other words `d(x) = min over all wall surface points x_w of |x - x_w|`, a purely geometric quantity of the mesh and the wall patches.

> [!warning] Contradiction
> [[ref-davidson-turbulence-notes]] (eq. 20.5, p. 217) describes `d` in its SST summary as "the distance to the closest wall node", while [[ref-nasa-tmr]] (`spalart.html`) calls the nearest wall gridpoint or cell centre "incorrect" for the minimum distance. Both are external sources; the TMR statement is the more specific one and is the one this page follows.

**What it adds to its prerequisite.** A [[finite-volume-method]] stores values at cell centres and faces; wall distance is an extra geometric field computed once per mesh (again when the mesh moves) from the cell centres and the wall faces. It turns "how close is this cell to a wall" into a number the turbulence model can use to tell the inner boundary layer from the outer flow.

## Builds on

- [[finite-volume-method]]: cell centres, wall faces and the mesh geometry the distance is measured on.
- floor: Euclidean distance, minimisation.

## Leads to

- [[sst-blending-functions]], [[spalart-allmaras]], [[les-and-des]], [[friction-velocity-and-y-plus]] (`y` in `y+`)
- [[k-omega-sst]]

## How rcfd uses it

- `wall_distance` lives in [[rcfd-turb]] next to the wall functions ([[rcfd-turb]]).
- F-040 "Wall distance calculation and standard wall functions" is done (milestones fan, aerofoil-lift) ([[roadmap-turbulence]]).
- RANS models need a wall patch ([[doc-case-schema-turbulence]]).

## Open questions

- How rcfd computes `d` (exact search or a PDE-based approximation) is not stated in the wiki pages read for this job; the method sources (Poisson or eikonal equation) were not read either. See the handoff of job p6-turbulence.
