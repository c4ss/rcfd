---
type: concept
tags: [foundation, multiphase]
sources: [ref-barreiro-villaverde-2021-jet-wiping-vof, ref-sharifi-2024-mules-interface-advection, ref-folden-aschmoneit-2023-cavitation-models, ref-wikipedia-multiphase-heat]
builds_on: [navier-stokes-equations]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Two-phase flow basics (immiscible fluids, interface capturing vs tracking)

## In short

A two-phase flow has two fluids, such as water and air, that do not mix and are separated by a moving interface. The common "one-fluid" approach solves a single set of [[navier-stokes-equations]] for the whole domain, with density and viscosity that jump across the interface, and marks where each fluid is with an extra field ([[ref-barreiro-villaverde-2021-jet-wiping-vof]] sec. II.B; [[ref-wikipedia-multiphase-heat]] "Volume of fluid method").

## Explanation

**The interface.** Between two immiscible fluids the density and viscosity change abruptly, and surface tension acts on the interface ([[surface-tension-and-curvature]]). The numerical difficulties named in the literature are conservation of mass, momentum and energy, large jumps of properties across the interface, changes of topology (break-up, merging), stability and an accurate surface-tension force ([[ref-sharifi-2024-mules-interface-advection]] sec. 1).

**One-fluid formulation.** A volume fraction (colour function) $\alpha$ is 1 in cells full of liquid, 0 in cells full of gas and between 0 and 1 where the interface passes. Local properties are volume-weighted averages ([[ref-barreiro-villaverde-2021-jet-wiping-vof]] Eq. 1):

$$\rho = \alpha\rho_l + (1-\alpha)\rho_g, \qquad \mu = \alpha\mu_l + (1-\alpha)\mu_g$$ (Barreiro-Villaverde et al., Eq. 1)

These properties go into one incompressible momentum equation with gravity and a surface-tension force $F_\sigma$ (Eqs. 2a, 2b), and $\alpha$ is advected by the flow (Eq. 3; see [[volume-fraction-transport]]). All fluids share the one momentum equation, and the constraint that the fractions sum to one keeps the fluid volume constant ([[ref-wikipedia-multiphase-heat]] "Volume of fluid method", Overview). The same mixture idea, with the vapour fraction $\alpha_v$, is the "homogeneous mixture" basis of most cavitation models: both phases share velocity and pressure and are in mechanical and thermal equilibrium ([[ref-folden-aschmoneit-2023-cavitation-models]] sec. II.1, Eq. 2).

**Capturing versus tracking.** Interface-tracking methods follow the interface explicitly, with marker points or a mesh fitted to it (front tracking); interface-capturing methods represent it implicitly by a field on a fixed mesh: VOF, level set, phase field ([[ref-sharifi-2024-mules-interface-advection]] sec. 1, Fig. 1). In VOF "the interface delineation occurs implicitly through the allocation of volume fractions" (same). VOF replaced the costly Lagrangian markers of the earlier Marker-and-Cell method with one scalar per cell, needs little storage, handles sharp topology changes and avoids the mesh deformation of surface-tracking methods; its main difficulty is numerical diffusion that smears the interface ([[ref-wikipedia-multiphase-heat]] "Volume of fluid method", History and Overview).

**What it adds.** The single-phase [[navier-stokes-equations]] assume one fluid with constant properties; two-phase flow adds a field that says which fluid is where, property jumps, and an interface force.

## Builds on

- [[navier-stokes-equations]]: the one momentum and continuity system, now with variable $\rho$ and $\mu$ and an extra surface force.

## Leads to

- [[volume-fraction-transport]], [[interface-compression]], [[surface-tension-and-curvature]], [[cavitation-and-mass-transfer-models]], [[nucleation-and-condensation]]
- [[vof]] (rcfd's implementation)

## How rcfd uses it

- rcfd's VOF solves two incompressible immiscible fluids with one volume fraction; mixture density and viscosity follow from $\alpha$ ([[vof]]; [[doc-case-schema-multiphase]]).
- The density jump made AMG break down in the pressure solve until ADR 0052 ([[vof]], [[adr-thread-vof-multiphase-particles]]).
- The same VOF host carries the algebraic-slip mixture model and Schnerr-Sauer cavitation; the mixture model "is NOT a multi-fluid (per-phase momentum) Euler-Euler model" ([[doc-case-schema-multiphase-mixture-cavitation]]).
- Roadmap area: [[roadmap-multiphase-and-particles]].
