---
type: concept
tags: [physics, rotating]
sources: [doc-glossary, doc-readme, doc-meshing, doc-component-diagonal-default, adr-thread-fan01-rotating-solve, adr-thread-sliding-mesh-rotor-stator, adr-thread-fan01-design-point]
builds_on: [relative-and-absolute-velocity-formulations, rotating-reference-frame, frozen-rotor-and-mixing-plane, periodic-and-cyclic-boundaries, simple]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# MRF (multiple reference frame / frozen rotor)

A steady solve in which a rotating zone is computed in its own rotating frame (ADR 0008) ([[doc-glossary]]).

## How rcfd uses it

- The steady rotating-frame solve of [[fan-01]] does not reach its expected operating point ([[doc-readme]]).
- The absolute-formulation `mrf` source is refused on the bladed fan sector mesh (blade and seam faces are not surfaces of revolution; located diagnostic on `sources.<name>.zone`); smoke solves there use the relative-frame `rotating_frame` source ([[doc-meshing]]). See [[fan-passage-mesh]].
- The exact annular sector `annulus_3d_sector` (ADR 0116) has planar faces, so an MRF zone with cyclic seams is exact per cell; it has no blade, so "it does not open MRF on the fan passage" ([[doc-meshing]]).
- MRF zones always use the corrected component diagonal, whatever `solver.component_diagonal` says ([[doc-component-diagonal-default]]). See [[component-diagonal]].
- F-126 (frozen-rotor and stage interfaces with sector periodicity) is partial; its gap names "MRF on the bladed FAN-01 passage (blade wall refused by the revolution check)" and "transient and coupled solvers" among what is missing ([[adr-thread-fan01-rotating-solve]]).
- The transient alternative, a sliding rotor-stator interface, runs from a case file on the bladeless cylindrical-interface annulus and sector (F-303, F-711), laminar only ([[adr-thread-sliding-mesh-rotor-stator]]). See [[sliding-mesh]].

- **Formulation (ADR 0008, added by p3-records-a):** an MRF zone in the steady `simple` / `simplec` kernel "solves for the absolute velocity $\mathbf{u}$, with the relative flux $\phi_{rel}$ in momentum convection only and a lagged $\boldsymbol{\Omega}\times\mathbf{u}$ source"; the checks are analytic flows (solid body, Taylor-Couette, axial through-flow), "no pump or turbine benchmark is claimed"; the registered torque limits of M2 to M4 stay missed (-3.41 %, -3.41 %, -4.12 % against 3 %) ([[adr-thread-fan01-rotating-solve]]).
- **Rotating frame without a fan (ADR 0071):** the solver is second order in the `rotating_frame` and `mrf` formulations; a seam imbalance was a defect of `SimpleSolver::iterate`, fixed opt-in by `coupled_body_pair_mean` ([[adr-thread-fan01-rotating-solve]]).
- **Cyclic seams (ADR 0113):** `sources.<name>.cyclic_seams` (default `false`) "gives the frame flux to both faces of a seam pair when both owner cells are in the zone", with the solver flag `coupled_offdiag_in_h1`; verified at library level only ([[adr-thread-fan01-rotating-solve]]).
- **On FAN-01:** the steady relative-frame solve with first-order upwind manufactures swirl that the walls do not supply (ADRs 0078, 0082), and the steady run stalls in a limiter cycle (ADRs 0110, 0111) ([[adr-thread-fan01-rotating-solve]]). Hub item: [[f-126]].
- **Absolute velocity from the relative-frame solve (ADR 0765):** to compare the rotating-frame FAN-01 solve with LDA profiles the value at a point is the cell's absolute velocity $\mathbf{c} = \mathbf{w} + \omega\,\mathbf{e}_z \times \mathbf{x}_c$ at the cell centre, $c_t = w_t + \omega r_c$. Amendment 1 replaced the registered $w_t + \omega r$ of the point, which put a sawtooth of $\omega$ times the cell's radial extent on the profile (0.78 m/s per 5 mm on B3) ([[adr-thread-fan01-design-point]]).

## Builds on

- [[relative-and-absolute-velocity-formulations]]: the `mrf` source is the absolute form (absolute $\mathbf{u}$, relative flux $\phi_{rel}$), the `rotating_frame` source the relative form.
- [[rotating-reference-frame]]: the Coriolis and centrifugal terms (here the lagged $\boldsymbol{\Omega}\times\mathbf{u}$ source).
- [[frozen-rotor-and-mixing-plane]]: MRF as a frozen-rotor model and its known limits (no rotor-stator unsteadiness).
- [[periodic-and-cyclic-boundaries]]: the cyclic seams of a sector zone (ADR 0113).
- [[simple]]: the steady `simple` / `simplec` kernel the MRF zone runs in.

## Related

- [[adr-thread-fan01-rotating-solve]]
- [[fan-01]]
- [[sliding-mesh]]
- [[fan-passage-mesh]]
