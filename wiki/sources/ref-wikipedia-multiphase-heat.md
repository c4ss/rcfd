---
type: source
tags: [reference, foundation, multiphase, heat-transfer, wikipedia, tertiary]
sources: []
origin: https://en.wikipedia.org/wiki/Stokes%27_law
licence: CC BY-SA 4.0 (page footers, read 2026-11-22); never copied into raw/
read: 2026-11-22
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Wikipedia articles for the multiphase and heat-transfer foundations

**Origin:** English Wikipedia, read 2026-11-22: "Stokes' law" (https://en.wikipedia.org/wiki/Stokes%27_law), "Young-Laplace equation" (https://en.wikipedia.org/wiki/Young%E2%80%93Laplace_equation), "Volume of fluid method" (https://en.wikipedia.org/wiki/Volume_of_fluid_method), "Arrhenius equation" (https://en.wikipedia.org/wiki/Arrhenius_equation).
**Kind:** encyclopedia (tertiary).
**Licence:** CC BY-SA 4.0. Used only for elementary concepts and as a cross-check (maintainer decision 6, [[ingest-plan]]); never the sole source of a model-level claim; never copied.
**Read:** 2026-11-22 by job p6-multiphase-heat.

## Key points by article

### "Stokes' law" (sections "Statement of the law", "Applications")
- Drag on a sphere of radius $R$ moving at speed $v$ through a fluid of viscosity $\mu$: $F_d = 6\pi\mu R v$, opposing the motion.
- Assumptions: laminar flow, no inertial effects (zero Reynolds number), spherical, homogeneous, smooth particles that do not interfere; "To 10% error, for instance, velocities need be limited to those giving Re < 1."
- Terminal velocity: weight minus buoyancy $(\rho_p - \rho_f)g\frac43\pi R^3$ balances drag, $v = \frac29\frac{(\rho_p-\rho_f)gR^2}{\mu}$; the particle falls if $\rho_p > \rho_f$, rises otherwise; $v \propto R^2$.

### "Young-Laplace equation" (lead, "Capillary pressure in a tube")
- Pressure jump across a curved fluid interface at rest: $\Delta p = -\gamma\nabla\cdot\hat n = -2\gamma H_f = -\gamma(1/R_1 + 1/R_2)$, with surface tension $\gamma$, mean curvature $H_f$, principal radii $R_1$, $R_2$ (the sign follows the page's convention of exterior minus interior pressure); for a sphere $\Delta p = 2\gamma/R$.

### "Volume of fluid method" (sections "History", "Overview", "Discretization", "Interface capture issues")
- VOF descends from the Marker-and-Cell method; Hirt and Nichols named and formalised it in 1981 (Nichols and Hirt 1975 earlier); SLIC (Noh and Woodward 1976) and PLIC (Youngs 1982) reconstruct the interface.
- The fraction $C$ is 0 in an empty cell, 1 in a full cell, between 0 and 1 in an interface cell; advection $\partial C/\partial t + \vec v\cdot\nabla C = 0$; properties are volume-fraction averages and all fluids share one momentum equation.
- Main difficulty: numerical diffusion smears the free surface; low-order upwind schemes smear, high-order schemes can oscillate; compressive differencing schemes (CICSAM, HRIC) and geometric reconstruction are the remedies named. Surface-tension errors with very different phase properties cause spurious currents in VOF and level-set methods.

### "Arrhenius equation" (sections "Formulation", "Modified Arrhenius equation")
- $k = A\exp(-E_a/(RT))$, per-molecule form $k = A\exp(-E_a/(k_BT))$; $A$ the pre-exponential (frequency) factor, $E_a$ the activation energy; modified form $k = AT^n\exp(-E_a/(RT))$, $n = 0$ the original. For common activation energies the rate roughly doubles or triples per 10 °C.

## Contradictions with the wiki

None.

## Pages this source informs

[[particle-equation-of-motion]], [[surface-tension-and-curvature]], [[two-phase-flow-basics]], [[volume-fraction-transport]], [[chemical-kinetics-arrhenius]]
