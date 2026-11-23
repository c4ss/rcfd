---
type: concept
tags: [foundation, mesh, moving-mesh, sliding-mesh]
sources: [ref-liu-hasegawa-2026-vpm-mrf-sliding, ref-yao-2026-ale-chalmers, ref-spenke-2020-interface-quasi-newton, ref-chima-1998-nasa-tm-206613]
builds_on: [finite-volume-method]
created: 2026-11-22
updated: 2026-11-22
status: stub
---

# Non-conformal interfaces

## In short

Two mesh blocks can meet at a surface where their faces do not line up: a rotor block that has turned against a stator block, or two meshes generated separately. Fluxes (or loads) must then be passed across faces that only partly overlap, and the transfer should conserve what it carries.

## Explanation

- **Sliding interfaces.** In a sliding mesh the grid of the rotating region turns at the prescribed speed; the interface nodes of the rotating and stationary regions are remapped each step, and the regions, "geometrically separated but numerically coupled", are joined by continuity of velocity and stress across the interface ([[ref-liu-hasegawa-2026-vpm-mrf-sliding]], sec. 2.2, eq. 7). In OpenFOAM's arbitrary mesh interface (AMI) the coupling works by "projecting grid nodes of the rotating region into the stationary region across the sliding interface" (sec. 2.6). A pressure jump can appear across the interface from an inconsistency inside the PISO loop, treated there by an extra pressure correction (sec. 2.5).
- **Conservation at the interface.** Interface transfers are not automatically conservative. A mixing-plane interface "does not guarantee conservation between blade rows"; the error depends on the averaging ([[ref-chima-1998-nasa-tm-206613]], interface boundary condition). For fluid-structure interfaces with non-matching meshes "a conservative projection is required to transfer relevant data between the two solvers" ([[ref-spenke-2020-interface-quasi-newton]], sec. 3.3), and the load- and velocity-transfer operators should preserve the interface work ([[ref-yao-2026-ale-chalmers]], sec. 7.6).
- **Intersecting faces.** The map's scope (intersecting the two face sets and weighting each partial overlap) was not described in detail in the sources read; see Open questions.

## Builds on

- [[finite-volume-method]]: face fluxes, which here are split over partly overlapping face pairs.
- Floor: plane geometry (polygon intersection).

## Leads to

- [[sliding-mesh]], [[lagrangian-particles]] (`join_non_conformal`), [[partitioned-fluid-structure-interaction]] (non-matching interface meshes)

## How rcfd uses it

- **Sliding mesh:** a rotor block turns against a stator block and "the non-conformal interface between them is rebuilt every step" (`rcfd_fv::sliding`, `PimpleSolver::step_sliding`; F-250, ADR 0216); a step whose interface non-orthogonality exceeds 70 deg stops the run (ADR 0637 D3); monitors include `sliding_interface_flux` and `sliding_gcl` ([[sliding-mesh]]).
- **Particles:** non-conformal interfaces are tracked only after `join_non_conformal` (not tested) ([[lagrangian-particles]]).
- **Co-simulation:** least-squares coupling converges better than Aitken on a 3:2 non-matching wall (F-401, ADR 0434) ([[cosimulation-protocol]]).

## Open questions

- A source that describes face intersection (supermesh or polygon clipping) and conservative weighting of the partial face fluxes is needed; the search found journal papers (e.g. supermesh methods in J. Comput. Phys.) but no open copy was read in this job.
