---
type: concept
tags: [foundation, rotating, boundary-conditions, periodic]
sources: [ref-thompkins-1982-nasa-cr-3560, ref-chima-1998-nasa-tm-206613]
builds_on: [boundary-conditions]
created: 2026-11-22
updated: 2026-11-22
status: stub
---

# Periodic and cyclic boundaries

## In short

A fan with N identical blades has a flow that repeats every blade pitch (when it is steady in the blade frame), so only one passage needs to be meshed. The two cut faces of the passage are tied together: whatever leaves one side enters the other, one pitch further round.

## Explanation

- **Blade-to-blade periodicity.** For a single blade row the flow is assumed periodic from passage to passage, and the computational domain is one passage ([[ref-thompkins-1982-nasa-cr-3560]], grid section and "Periodic Boundaries").
- **Implementation in index space.** On a grid whose boundaries lie midway between grid lines, periodicity is imposed by copying: the values at the ghost layer `k = 1` are those of `k = NTH - 1`, and at `k = NTH` those of `k = 2` ([[ref-thompkins-1982-nasa-cr-3560]], "Periodic Boundaries", p. 35). The code works in cylindrical coordinates `(R, theta, X)` with blade-to-blade grid lines at constant `(r, z)`, so the two seam faces match point for point under the rotation by one pitch (same report, grid section).
- **Use in averaging.** Periodicity across the pitch makes the integral of the circumferential fluxes zero, which is what lets a mixed-out average represent the fully mixed downstream state ([[ref-chima-1998-nasa-tm-206613]], averaging techniques).
- **Vector quantities.** How velocity components are transformed across a rotational seam is not stated in the sources read; see Open questions.

## Builds on

- [[boundary-conditions]]: periodic conditions as a pair of coupled boundaries rather than prescribed values.
- Floor: rotation matrices.

## Leads to

- [[mrf]] (cyclic seams), [[sliding-mesh]] (periodic sector), [[cyclic-symmetry]] (the structural analogue), [[adr-thread-coupled-periodic-faces]], [[frozen-rotor-and-mixing-plane]]

## How rcfd uses it

- **Cyclic seams in MRF:** `sources.<name>.cyclic_seams` (default `false`) "gives the frame flux to both faces of a seam pair when both owner cells are in the zone" (ADR 0113) ([[mrf]]).
- **Periodic sector slider:** `rcfd_mesh::sliding_periodic`, wrap patches `sliding_wrap_a` / `sliding_wrap_b`; the sector agrees with the full annulus within `2.0e-2 U_in` over 400 steps, mostly the lagged partner value; opt-ins `rotational_periodic_implicit` (F-743), `coupled_non_orth` (F-744) bring it to `1.1e-4 U_in` with one outer iteration ([[sliding-mesh]], [[adr-thread-coupled-periodic-faces]]).
- **Structures:** cyclic symmetry ties high-side unknowns to the low side rotated by `2 pi / N`, per harmonic index ([[cyclic-symmetry]]).
- **Fan passage:** the H-slit and C-grid generators build one blade passage as a periodic-sector hex mesh ([[fan-passage-mesh]]).

## Open questions

- The transformation of vector and tensor quantities across a rotational seam (Cartesian rotation by the pitch angle) is in the map's scope for this node but was not found in a source read in this job; a source is needed before the page leaves stub.
