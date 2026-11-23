---
type: concept
tags: [foundation, pressure-velocity]
sources: [ref-jasak-1996-phd-thesis, ref-klaij-vuik-2013-simple-preconditioners]
builds_on: [collocated-grid-and-checkerboarding, momentum-matrix-and-h-operator]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Rhie-Chow interpolation (momentum-weighted face flux)

## In short

On a collocated grid the face mass flux is not taken from linearly interpolated cell velocities. Instead the cell momentum equation $U = H/a_P - \nabla p/a_P$ is interpolated to the face, and the pressure-gradient part is replaced by the pressure gradient computed directly across the face. The difference between the two pressure gradients acts as a small pressure-dissipation term that suppresses the checkerboard. Because $a_P$ contains the time-step and relaxation terms, the converged face flux can depend on them unless care is taken ([[ref-jasak-1996-phd-thesis]] 3.8.1; [[ref-klaij-vuik-2013-simple-preconditioners]] 2.5).

## Explanation

**Face flux.** "In the spirit of the Rhie and Chow procedure" the pressure gradient is not discretised in the semi-discrete momentum equation $a_P U_P = H(U) - \nabla p$ ([[ref-jasak-1996-phd-thesis]] Eqn. 3.136). The face velocity is the face interpolate $U_f = (H/a_P)_f - (1/a_P)_f (\nabla p)_f$ (Eqn. 3.140), and the flux $F = S \cdot (H/a_P)_f - S \cdot (1/a_P)_f (\nabla p)_f$ (Eqn. 3.144) takes the face-normal pressure gradient from the two cell pressures across the face, the same one used in the pressure Laplacian ([[ref-jasak-1996-phd-thesis]] 3.8.1).

**As a stabilisation term.** Klaij and Vuik write the same idea as the pressure-weighted interpolation method (PWIM): the face velocity gets an added $h^2$ times a second derivative of the pressure gradient, which vanishes as the mesh is refined ([[ref-klaij-vuik-2013-simple-preconditioners]] 2.1, Eq. 2). Its coefficients are, on each side, an interpolation weight times the cell volume over $\operatorname{diag}(Q)$ ([[ref-klaij-vuik-2013-simple-preconditioners]] Eq. 9). They derive it from SIMPLE's approximation $Q^{-1} \approx \operatorname{diag}(Q)^{-1}$: "interchanging" a cell velocity with its pressure gradient in the part of the face velocity interpolated from the cells gives exactly the PWIM term ([[ref-klaij-vuik-2013-simple-preconditioners]] 2.5).

**Dependence on time step and relaxation.** Implicit time discretisation and implicit relaxation add diagonal terms to $Q$; through the PWIM coefficients this makes "the converged steady-state solution depends on the user's choice of time-step and/or implicit relaxation factor (although the effect vanishes with grid refinement)". Klaij and Vuik therefore leave these contributions out of $\operatorname{diag}(Q)$ in the PWIM only ([[ref-klaij-vuik-2013-simple-preconditioners]] 2.5). Defect correction keeps the PWIM coefficients independent of the convection scheme ([[ref-klaij-vuik-2013-simple-preconditioners]] 2.5). Because the PWIM needs $\operatorname{diag}(Q)$ of the previous iteration, a simulation "must start with a constant pressure field" ([[ref-klaij-vuik-2013-simple-preconditioners]] 3.4).

## Builds on

- [[collocated-grid-and-checkerboarding]]: the problem it fixes.
- [[momentum-matrix-and-h-operator]]: $a_P$, $H(U)$ and `HbyA`.

## Leads to

- [[pressure-correction-equation]], [[under-relaxation]]
- rcfd pages: [[simple]], [[piso]], [[adr-thread-bdf2-step-consistent-face-flux]], [[doc-case-schema-rhie-chow]]

## How rcfd uses it

- rcfd's SIMPLE and PISO kernels are collocated with Rhie-Chow ([[simple]]; [[piso]]).
- **Step dependence:** the default transient face flux depends on the step size where the momentum diagonal differs across a face, $3.7 \times 10^{-4}$ of the lid velocity in ADR 0245's cavity between $\Delta t = 0.04$ and `0.01` ([[doc-case-schema-rhie-chow]]). This is the time-step dependence Klaij and Vuik describe ([[ref-klaij-vuik-2013-simple-preconditioners]] 2.5).
- **Opt-in fix:** `[solver.rhie_chow] step_consistent = true` uses a harmonic face value of `rAU` $= V/a_P$ plus the commutator of the lagged pressure gradient on internal faces ([[doc-case-schema-rhie-chow]]); in `PimpleSolver` it removes the step-size dependence on internal faces (F-344, F-443), after which BDF2 meets its order at rest but not on the moving overset TG_2 (`2.454` against 4) ([[piso]]; [[adr-thread-bdf2-step-consistent-face-flux]]).
- `solver.pressure_interpolation` offers `linear` (default), `skew_corrected` and `body_force_weighted` in the steady kernel (F-246, partial) ([[simple]]).
