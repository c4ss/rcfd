---
type: concept
tags: [foundation, compressible, conservation-law]
sources: [ref-van-leer-1992-icase-92-43, ref-hubbard-1991-nasa-rp-1258-vol2, ref-nasa-glenn-beginners-guide, ref-naca-1953-report-1135, ref-liou-2001-nasa-tm-210977, ref-hu-1995-icase-95-70]
builds_on: [conservation-law, ideal-gas-and-thermodynamics-basics]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Euler equations

## In short

The Euler equations are the conservation laws of mass, momentum and energy for a compressible fluid with no viscosity and no heat conduction. They are the Navier-Stokes equations without the viscous terms, closed by the equation of state of the gas. Written in conservation form they say that each cell's mass, momentum and energy change only by what flows through its faces ([[ref-van-leer-1992-icase-92-43]] sec. 2.1; [[ref-nasa-glenn-beginners-guide]] "Navier-Stokes Equations").

## Explanation

**One dimension, conservation form.** In 1D the system is

`U_t + F(U)_x = 0`, with `U = (rho, rho u, rho E)^T` and `F(U) = (rho u, rho u^2 + p, rho u H)^T`

([[ref-van-leer-1992-icase-92-43]] sec. 2.1, Eq. 4). `U` holds the conserved quantities: density, momentum per unit volume and total energy per unit volume (`E` total energy per unit mass, `H = E + p/rho` total enthalpy). `F` holds their fluxes: mass flux, momentum flux plus pressure, and energy flux. "Any local imbalance of the fluxes causes the local solution to change in time" ([[ref-van-leer-1992-icase-92-43]] sec. 2.1).

**Closure.** Three equations have four unknowns (`rho, u, p, E`), so a state equation is needed: for a perfect gas `p = rho R T` and `e = c_v T`, giving `p = (gamma - 1) rho (E - u^2/2)` from the relations of [[ideal-gas-and-thermodynamics-basics]] ([[ref-naca-1953-report-1135]] pp. 614-615, Eqs. 2, 9, 17).

**Several dimensions, primitive form.** Eversman's duct chapter starts from the same inviscid, non-heat-conducting perfect gas: continuity `drho/dt + div(rho V) = 0`, momentum `dV/dt + (V . grad) V = -(1/rho) grad p`, an isentropic equation of state `p = rho^gamma/gamma` (nondimensional), and an energy equation `dp/dt + V . grad p + gamma p div V = 0` ([[ref-hubbard-1991-nasa-rp-1258-vol2]] ch. 13, pp. 103-104, Eqs. 1-4).

**Relation to Navier-Stokes.** NASA Glenn describes the Euler equations as the Navier-Stokes equations without the viscous terms; they cannot model boundary layers ([[ref-nasa-glenn-beginners-guide]] "Navier-Stokes Equations"). At high Reynolds number the viscous forces are small away from walls and the Euler equations can model the flow ([[ref-nasa-glenn-beginners-guide]] "Reynolds Number").

**Waves.** The Jacobian `A(U) = dF/dU` has real eigenvalues `lambda_k` (wave speeds) and eigenvectors `R_k`; a jump between two states splits into waves of strength `alpha_k`, `Delta U = sum_k alpha_k R_k` ([[ref-van-leer-1992-icase-92-43]] sec. 2.1, Eqs. 5-11). Linearised about a uniform mean flow, the equations carry acoustic waves at the speed of sound relative to the flow, and vorticity and entropy waves that move with the flow ([[ref-hu-1995-icase-95-70]] sec. 1). This wave structure is the subject of [[characteristics-and-hyperbolic-systems]].

**Discontinuities.** Because the equations are in conservation form they still make sense when the solution jumps: integrated across a jump they give the shock relations of [[shock-waves]] (mass, momentum and energy fluxes equal on both sides) ([[ref-naca-1953-report-1135]] pp. 618-619, Eqs. 84-86).

**Convective and pressure parts.** The flux can be split into a convective part, a mass flux carrying `(1, u, v, w, H)`, and a pressure part `(0, p n, 0)`; carrying `H` guarantees conservation of total enthalpy. This split is the starting point of the AUSM schemes ([[ref-liou-2001-nasa-tm-210977]] Eqs. 3-4).

**What it adds.** A [[conservation-law]] says a quantity changes only by its flux; the Euler equations are three (in 1D) coupled conservation laws whose fluxes depend on the state through the gas model. The coupling through pressure makes the system hyperbolic, with finite wave speeds, and allows shocks.

## Builds on

- [[conservation-law]]: the integral and differential form "rate of change = minus net flux".
- [[ideal-gas-and-thermodynamics-basics]]: the state equation that closes the system.

## Leads to

- [[characteristics-and-hyperbolic-systems]], [[shock-waves]], [[riemann-problem-and-godunov-method]], [[acoustic-wave-equation]]
- [[doc-case-schema-compressible]], [[dynamic-mesh]]

## How rcfd uses it

- `solver.algorithm = "compressible"` selects "the explicit density-based Euler solver (no viscosity, no heat conduction)", with fluxes `rusanov`, `roe`, `hllc`, `ausm_plus`, first-order or TVD-limited reconstruction and Runge-Kutta order 1 to 3 ([[doc-case-schema-compressible]]).
- The explicit compressible solver also runs on moving meshes, with prescribed motion or a layered piston (`CompressibleSolver::step_ale`) ([[dynamic-mesh]]).
- The linear acoustic wave solver is "the small-amplitude limit of the Euler equations of `CompressibleSolver`", two unknowns instead of five ([[adr-thread-acoustic-wave-solver]]).
