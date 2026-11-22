---
type: concept
tags: [foundation, discretisation]
sources: [ref-mit-ocw-2-29-numerics, ref-jasak-1996-phd-thesis, ref-van-leer-1992-icase-92-43, ref-venkatakrishnan-1995-icase-95-3]
builds_on: [conservation-law, divergence-theorem]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Finite-volume method (control volumes, face fluxes)

## In short

Cut the flow domain into small cells (control volumes) and write the conservation law for each cell: the amount of a quantity inside changes only by what flows through the cell's faces and what is produced inside. The unknowns are cell averages, the work is computing one flux per face, and because the flux leaving one cell enters its neighbour, the method conserves mass and momentum exactly at the discrete level ([[ref-mit-ocw-2-29-numerics]]).

## Explanation

**From differential to integral form.** Finite differences discretise the differential form of the equations; finite volumes discretise the integral form over control volumes that do not overlap ([[ref-mit-ocw-2-29-numerics]] L15 s.6). For a scalar `phi` with velocity `U`, diffusivity `Gamma` and source `S`, the integral balance over a cell `V_P` with boundary `dV_P` is `d/dt int_V rho phi dV + int_dV rho phi U . n dA - int_dV rho Gamma grad phi . n dA = int_V S dV` ([[ref-mit-ocw-2-29-numerics]] L15 s.4, s.7; [[ref-jasak-1996-phd-thesis]] Eqns. 3.2, 3.8). The [[divergence-theorem]] turns each volume integral of a divergence into a sum over the cell's flat faces.

**The mesh.** Cells fill the domain without overlap; the computational point P sits at the cell centroid; each internal face is shared by exactly two cells, an owner and a neighbour, and carries an area vector `S_f` normal to the face with magnitude equal to its area, pointing from owner to neighbour ([[ref-jasak-1996-phd-thesis]] 3.2, Eqn. 3.1). Cells can be general polyhedra, so the method applies directly to unstructured meshes without a coordinate transformation ([[ref-mit-ocw-2-29-numerics]] L15 s.7; [[ref-jasak-1996-phd-thesis]] 3.1). Grids can be cell-centred (node at the centroid) or cell-vertex (control volumes built around mesh vertices) ([[ref-mit-ocw-2-29-numerics]] L15 s.11-12; [[ref-venkatakrishnan-1995-icase-95-3]] 3).

**Second order by midpoint rules.** If `phi` is assumed to vary linearly about P, two midpoint rules are second-order accurate: the volume integral is the centre value times the volume, `int_V phi dV = phi_P V_P`, and a face integral is the face-centre value times the face area. Together they give the discrete Gauss theorem `(div a)_P V_P = sum_f S_f . a_f` ([[ref-jasak-1996-phd-thesis]] Eqns. 3.12-3.15; [[ref-mit-ocw-2-29-numerics]] L15 s.13-20). The cell average and the centre value differ only by a second-order term ([[ref-mit-ocw-2-29-numerics]] L15 s.23).

**Face flux.** The convection term becomes `sum_f F_f phi_f` with the face mass flux `F_f = S_f . (rho U)_f` ([[ref-jasak-1996-phd-thesis]] Eqns. 3.17-3.18). The face value `phi_f` is not stored; it must be interpolated from the cell values, which is the job of a convection scheme ([[upwind-and-central-differencing]]). The diffusion term needs the face-normal gradient ([[non-orthogonal-and-skewness-correction]]). In explicit form, cell averages change by the difference of the face fluxes, `U_i^{n+1} = U_i^n - (dt/dx)(F_{i+1/2} - F_{i-1/2})` in 1D ([[ref-van-leer-1992-icase-92-43]] Eq. 13).

**Steps of a scheme.** Reconstruct the variable in each cell from its average; resolve the two face values (one from each side) into a single flux; integrate the fluxes over the surface and the sources over the volume; advance in time ([[ref-mit-ocw-2-29-numerics]] L15 s.10, s.21).

**What it adds** to the bare [[conservation-law]]: a discrete form in which conservation holds cell by cell and globally, because summing over all cells cancels every internal face ([[ref-mit-ocw-2-29-numerics]] L15 s.7).

## Builds on

- [[conservation-law]]: the integral balance of a quantity over a control volume is the equation being discretised.
- [[divergence-theorem]]: turns volume integrals of fluxes into sums over the faces.

## Leads to

- [[convection-diffusion-equation]], [[upwind-and-central-differencing]], [[gradient-reconstruction]], [[deferred-correction]], [[non-orthogonal-and-skewness-correction]], [[mesh-generation-basics]]
- [[pressure-velocity-coupling]], [[momentum-matrix-and-h-operator]], [[collocated-grid-and-checkerboarding]]
- rcfd pages: [[simple]], [[piso]], [[mesh-quality]]

## How rcfd uses it

- rcfd's segregated pressure-based kernels are cell-centred finite-volume solvers, collocated with Rhie-Chow: `SimpleSolver` for SIMPLE/SIMPLEC ([[simple]]) and `PimpleSolver` for PISO/PIMPLE ([[piso]]), in the [[rcfd-fv]] crate.
- rcfd judges a mesh before a run by non-orthogonality, skewness, aspect ratio and volume ratio, among other measures ([[mesh-quality]]); the first two are the geometric errors described on [[non-orthogonal-and-skewness-correction]].
- Adapted meshes are hanging-node polyhedra, which the polyhedral face-based formulation accepts as ordinary cells ([[mesh-adaptation]]).
