---
type: concept
tags: [foundation, shared-roots, fluid-mechanics]
sources: [ref-bar-meir-fluid-mechanics, ref-openstax-university-physics-1, ref-nasa-glenn-beginners-guide, ref-wikipedia-shared-roots]
builds_on: [viscosity]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Dimensionless numbers (Reynolds, Mach, Prandtl, Strouhal, Courant, ...)

## In short

A dimensionless number is a ratio of two effects in a flow, for example inertia over viscosity (Reynolds) or flow speed over sound speed (Mach). Two flows with the same dimensionless numbers behave alike, which is why experiments are scaled by them and why a case is described by "Re 100" rather than by its speed ([[ref-nasa-glenn-beginners-guide]] "Reynolds Number"; [[ref-bar-meir-fluid-mechanics]] Table 9.8).

## Explanation

**Similarity.** If two experiments have the same values of the similarity parameters, the relative importance of the forces is modelled correctly ([[ref-nasa-glenn-beginners-guide]] "Reynolds Number"). Bar-Meir's chapter 9 builds such groups from dimensional analysis (Buckingham pi theorem, sec. 9.2) and lists them in Table 9.8 ([[ref-bar-meir-fluid-mechanics]]).

**The numbers rcfd pages meet** (definitions from Table 9.8 unless noted; `U` a velocity, `l` a length):
- **Reynolds** `Re = rho U l / mu = U l / nu`: inertia forces to viscous forces ([[ref-bar-meir-fluid-mechanics]]; [[ref-nasa-glenn-beginners-guide]]). Around `Re = 100` viscous forces must be considered, around `10 million` they are small ([[ref-nasa-glenn-beginners-guide]]). For pipe flow `N_R = 2 rho v r / eta` (Eq. 14.20): below about 2000 laminar, above about 3000 turbulent, unstable in between ([[ref-openstax-university-physics-1]] sec. 14.7).
- **Mach** `M = U / c`: flow speed to speed of sound; it indicates how strongly compressibility matters; subsonic `M < 1`, transonic near 1, supersonic `1 < M < 3`, hypersonic `M > 5` ([[ref-nasa-glenn-beginners-guide]] "Mach Number"). See [[incompressible-flow]] for the low-Mach approximation.
- **Prandtl** `Pr = nu / alpha`: viscous diffusion to thermal diffusion.
- **Strouhal** `St = omega l / U`: unsteady effects to inertia (shedding frequency made dimensionless).
- **Froude** `Fr = U / sqrt(g l)`: inertia to gravity. **Euler** `Eu = (P0 - P_inf)/(rho U^2/2)`: pressure to inertia. **Weber** `We = rho U^2 l / sigma`: inertia to surface tension. **Knudsen** `Kn = lambda / l`: mean free path to length ([[continuum-hypothesis]]).
- **Stokes number:** Table 9.8 describes it as particle relaxation time to Kolmogorov time, but its printed formula is the Galileo number's, so the formula is not used here ([[ref-bar-meir-fluid-mechanics]]); see `particle-equation-of-motion`.
- **Peclet number:** not in the sources read in this job; see [[convection-diffusion-equation]].

**Numerical (grid) numbers.** The Courant number `C = u dt / dx` compares the distance a wave travels in one step with the cell size; explicit schemes need `C <= C_max`, typically 1, while implicit schemes tolerate more; in n-D `C = dt sum(u_i / dx_i)` ([[ref-wikipedia-shared-roots]] "Courant-Friedrichs-Lewy condition"). By analogy, the diffusion number `nu dt / dx^2` compares viscous diffusion in one step with the cell size (used on rcfd pages; no general source for its limits was read in this job; see [[stability-and-cfl]]).

**What it adds.** [[viscosity]] gives `mu` and `nu`; combined with a speed and a length they say which terms of the [[navier-stokes-equations]] dominate, without solving them.

## Builds on

- [[viscosity]]: `mu` and `nu` appear in Reynolds and Prandtl numbers.
- Floor: units and dimensions, speed of sound, frequency.

## Leads to

- [[turbulence]] (Reynolds number sets transition), [[incompressible-flow]] (Mach), [[stability-and-cfl]] (Courant), [[boundary-layer]], `particle-equation-of-motion` (Stokes number), `boussinesq-buoyancy-approximation`.

## How rcfd uses it

- [[cylinder-re100]] is laminar flow past a cylinder at Re 100, compared by Strouhal number and mean drag coefficient ([[cylinder-re100]]).
- [[lid-driven-cavity]] reference velocities are at Re 100, 400 and 1000 ([[lid-driven-cavity]]); [[backward-facing-step]] is another Re-scaled benchmark.
- PISO stability on fine wall cells is stated as a diffusion number: unstable above `nu dt/dx^2` of about 3 to 4 in narrow gaps (F-244, ADR 0211) ([[piso]]); wall onset between 6.5 and 11.4 on the refined cylinder (ADR 0154) ([[cylinder-re100]]).
- Particles use the response time `tau_p` from Stokes drag ([[lagrangian-particles]]).
