---
type: concept
tags: [foundation, heat-transfer]
sources: [ref-lienhard-ahtt-6e, ref-openstax-college-physics-ch14]
builds_on: []
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Heat conduction (Fourier's law, conductivity, diffusivity, transient conduction)

## In short

Heat flows through a material from hot to cold, at a rate proportional to the temperature gradient: Fourier's law. Combined with conservation of energy it gives the heat (diffusion) equation for the temperature field, whose speed of response is set by the thermal diffusivity ([[ref-lienhard-ahtt-6e]] sec. 1.3, 2.1; [[ref-openstax-college-physics-ch14]] 14.5).

## Explanation

**Mechanism.** Conduction is "heat transfer through stationary matter by physical contact": molecular collisions pass kinetic energy from hotter to cooler regions ([[ref-openstax-college-physics-ch14]] 14.4, 14.5); in solids lattice vibrations and, in metals, the electron gas carry it, which is why most solids conduct better than liquids and liquids better than gases ([[ref-lienhard-ahtt-6e]] sec. 1.3).

**Fourier's law.** The heat flux $\vec q$ (W/m$^2$) is

$$\vec q = -k\nabla T$$ (Lienhard eq. 2.2; 1-D form $q = -k\,dT/dx$, eq. 1.8)

with thermal conductivity $k$ (W/m K). For a slab of thickness $d$ and area $A$ the rate is $Q/t = kA(T_2 - T_1)/d$ ([[ref-openstax-college-physics-ch14]] 14.5). Values span about five orders of magnitude between gases and diamond ([[ref-lienhard-ahtt-6e]] Fig. 1.6): near 0 °C air 0.023, water 0.6, stainless steel 14, aluminium 220, copper 390 W/m K ([[ref-openstax-college-physics-ch14]] 14.5). In general $k$ depends on temperature, rising with $T$ in low-pressure gases ([[ref-lienhard-ahtt-6e]] sec. 2.1).

**The heat conduction equation.** Energy balance on a control volume with a volumetric heat source $\dot q$, Gauss's theorem ([[divergence-theorem]]) and an arbitrary region give

$$\nabla\cdot(k\nabla T) + \dot q = \rho c\frac{\partial T}{\partial t}$$ (Lienhard eq. 2.10)

for an incompressible, motionless medium; with constant $k$, $\nabla^2T + \dot q/k = \frac1\alpha\partial T/\partial t$ (eq. 2.11) ([[ref-lienhard-ahtt-6e]] sec. 2.1). The thermal diffusivity $\alpha = k/(\rho c)$ (m$^2$/s) measures "how quickly a material can carry heat away from a hot source": a material must be warmed as well as conduct (sec. 1.3).

**Boundary conditions.** Each coordinate needs two: temperature given (first kind, Dirichlet), heat flux given (second kind, Neumann, zero for insulation), or convection $-k\,\partial T/\partial n = h(T - T_\infty)$ (third kind); plus one initial condition for a transient problem ([[ref-lienhard-ahtt-6e]] sec. 4.1). See [[boundary-conditions]].

**Transient conduction (erf solution).** A semi-infinite body at $T_i$ whose surface is suddenly held at $T_\infty$: position and time combine into one variable $x/\sqrt{\alpha t}$, and

$$\frac{T - T_\infty}{T_i - T_\infty} = \mathrm{erf}\left(\frac{x}{2\sqrt{\alpha t}}\right)$$ (Lienhard eq. 5.50)

with $\mathrm{erfc} = 1 - \mathrm{erf}$; the disturbance has reached 99 % depth at $\delta_{99} = 3.64\sqrt{\alpha t}$ (eq. 5.51), so the heated layer grows like $\sqrt t$ ([[ref-lienhard-ahtt-6e]] sec. 5.6).

## Builds on

- Floor: heat and temperature, energy conservation (first-year physics); the divergence theorem ([[divergence-theorem]]).

## Leads to

- [[energy-equation]] (adds fluid motion), [[conjugate-heat-transfer]] (conduction in solids coupled to fluids), [[convective-heat-transfer]] (the third-kind boundary condition)

## How rcfd uses it

- Solid cell zones `solids.<name>` with `conductivity`, `density`, `specific_heat` solve conduction in conjugate heat transfer (F-051, ADR 0024) ([[doc-case-schema-conjugate-heat-transfer]]).
- Temperature-dependent conductivity laws (`constant`, `polynomial`, `sutherland`, `piecewise_linear`, ...) under `properties.conductivity` ([[doc-case-schema-scalars-species-properties]]).
- Roadmap area: [[roadmap-heat-materials-and-chemistry]].
