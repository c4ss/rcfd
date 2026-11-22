---
type: source
tags: [reference, foundation, overset, chimera, moving-mesh, public-domain]
sources: []
origin: https://ntrs.nasa.gov/citations/19960050498
licence: "Work of the US Gov. Public Use Permitted." (NTRS record, verified 2026-11-22)
read: 2026-11-22
raw: raw/meakin-1996-nasa-cr-200828.pdf
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Meakin (1996), The Chimera Method of Simulation for Unsteady Three-Dimensional Viscous Flow, NASA CR-200828

**Origin:** R. L. Meakin, "The Chimera Method of Simulation for Unsteady Three-Dimensional Viscous Flow", NASA-CR-200828 (OM 01-92), Overset Methods, Inc. at NASA Ames Research Center, 31 March 1996 (reprint of the journal version), NTRS 19960050498. PDF: https://ntrs.nasa.gov/api/citations/19960050498/downloads/19960050498.pdf, copied to [raw/meakin-1996-nasa-cr-200828.pdf](../raw/meakin-1996-nasa-cr-200828.pdf).
**Kind:** review paper, 18 PDF pages.
**Licence:** "Work of the US Gov. Public Use Permitted." (NTRS).
**Read:** 2026-11-22 by job p6-linear-rotating: secs. 1, 2.1-2.3.2 in full, 2.4 skimmed, conclusions.

## Summary

A review of the Chimera overset-grid approach for unsteady 3-D viscous flow with moving bodies: decomposing the geometry into simple overlapping body-fitted component grids over a background grid, establishing domain connectivity by hole cutting and interpolation, the state of grid-generation and connectivity software, conservation and accuracy at intergrid boundaries, adaptive refinement in overset systems, and four aerodynamic applications.

## Key points

- **Sec. 1, introduction:** the geometry is decomposed into "geometrically simple overlapping component grids"; components associated with moving bodies "move with the bodies without stretching or distorting the grid system"; body-fitted component grids suit thin shear layers; structured solvers and parallelism carry over.
- **Sec. 2.1, background (fig. 1):** domain connectivity "is achieved through interpolation of necessary intergrid boundary information from solutions in the overlap region of neighboring grid systems". Airfoil example: a near-body grid overlapped by a background Cartesian grid; the near-body outer boundary is interpolated from the background solution; a "hole boundary" is created in the background by excluding the region overlapped by the airfoil, and the hole-boundary values are interpolated from the near-body grid. Hole points are excluded from the solution.
- **Sec. 2.2.4, domain connectivity:** active research areas are hole cutting, donor search (with quality optimisation) and automation; general codes PEGSUS, CMPGRD; DCF3D and others integrated with flow solvers for moving bodies, which need connectivity every time step.
- **Sec. 2.3, flow solver issues:** existing single-grid structured solvers adapt easily: an array `ib` is 0 at hole points and 1 at field points, so hole points are not updated and their values come from interpolation, without special branching logic.
- **Sec. 2.3.1, solution accuracy and conservation:** simple interpolation "implies that conservation is not strictly enforced", but with a conservative solver conservation holds everywhere except at a few intergrid boundary points. Formal accuracy is kept with simple interpolation: with fixed component boundaries in a refinement study, a 2nd-order solver stays 2nd order with a 2nd-order interpolation (e.g. tri-linear). The primary issue is resolution, not conservation: if the flow is smooth in donor and recipient grids, simple interpolation suffices; a conservative interpolation cannot ensure accuracy without resolution. Options for conservation: unstructured patches near intergrid boundaries; for incompressible flow, "non-conservative interpolation of intergrid boundary conditions can be made conservative by local redistribution of fluxes such that global conservation is ensured" (refs. 39, 40).
- **Sec. 2.3.2:** adaptive refinement in overset systems by cell subdivision, nested overset fine grids, or off-body uniform Cartesian grids of several levels (fig. 6).
- **Sec. 2.4:** applications (space shuttle ascent, SOFIA, store separation, tiltrotor download).

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[overset-interpolation]], [[overset-mesh]]
