---
type: source
tags: [reference, foundation, heat-transfer, cht, coupling, cite-only]
sources: []
origin: https://arxiv.org/abs/1705.05201
licence: arXiv non-exclusive distribution licence 1.0 (arXiv licence link, read 2026-11-22). Cite only.
read: 2026-11-22
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Monge and Birken (2017), convergence of the Dirichlet-Neumann iteration for thermal FSI (conjugate heat transfer)

**Origin:** A. Monge, P. Birken, "On the convergence rate of the Dirichlet-Neumann iteration for unsteady thermal fluid structure interaction", arXiv:1705.05201 [math.NA], 15 May 2017. PDF: https://arxiv.org/pdf/1705.05201
**Kind:** research preprint (Lund University).
**Licence:** arXiv non-exclusive distribution licence; cite only, no copy in `raw/`.
**Read:** 2026-11-22 by job p6-multiphase-heat: abstract, sec. 1, sec. 2 (models, coupling conditions, discretisation, Dirichlet-Neumann method; Eqs. 1-15), sec. 8 "Summary and Conclusions". The 1D analysis (sec. 4-6) was not read in detail.

## Summary

Thermal fluid-structure interaction, "also called conjugate heat transfer": a fluid domain and a solid domain coupled at an interface by continuity of temperature and of the normal heat flux, solved in a partitioned way by the Dirichlet-Neumann iteration. The paper derives the iteration's convergence rate for coupled 1D heat equations.

## Key points by section

### Sec. 1 "Introduction"
- Partitioned approach: separate codes per sub-problem, coupled by a master program through interface functions; the sub-solvers exchange Dirichlet and Neumann data in turn, hence the name. Monolithic approach: one code for the coupled equations.
- Applications: cooling of gas-turbine blades, thermal anti-icing of aircraft, supersonic re-entry of space vehicles.

### Sec. 2 "Thermal FSI Methodology"
- Fluid: compressible Navier-Stokes (Eq. 1). Solid: heat conduction only, $\rho c_p(\Theta)\,d\Theta/dt = -\nabla\cdot q_s$, $q_s = -\lambda(\Theta)\nabla\Theta$ (Eq. 2); for alloys $c_p$ and $\lambda$ depend strongly on temperature (steel 51CrV4 fits, Eqs. 3-6).
- Coupling conditions on the interface $\Gamma$ (sec. 2.3): temperature continuity $T(x,t) = \Theta(x,t)$ (Eq. 7) and normal heat flux continuity $q_f\cdot n = q_s\cdot n$ (Eq. 8).
- Fluid by finite volumes (Eq. 9), solid by quadratic finite elements with heat capacity matrix $M$ and conductivity matrix $A$ (Eq. 10), implicit Euler in time (Eqs. 11, 12).
- Dirichlet-Neumann (sec. 2.6): solve the fluid with the interface temperature as Dirichlet data, then the solid with the fluid's interface heat flux as Neumann data, repeat until the interface temperature change is below a tolerance (Eqs. 13-15).

### Abstract and sec. 8 "Summary and Conclusions"
- For large time steps the convergence rate is "the aspect ratio times the quotient of heat conductivities"; decreasing the time step improves it. Strong jumps in the material coefficients give fast convergence; for the nonlinear cooling problems the rate was still about 0.1 for large time steps.

## Contradictions with the wiki

None.

## Pages this source informs

[[conjugate-heat-transfer]]
