---
type: concept
tags: [foundation, compressible, thermodynamics]
sources: [ref-naca-1953-report-1135]
builds_on: []
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Ideal gas and thermodynamics basics

## In short

A perfect (ideal) gas obeys $p = \rho R T$, and its internal energy and enthalpy depend only on temperature. With constant specific heats, two numbers fix its thermodynamics: the gas constant $R$ and the ratio of specific heats $\gamma = c_p/c_v$ ($7/5$ for air). In a flow without shocks or heat transfer the entropy stays constant, and then pressure, density and temperature are tied together by the isentropic relations ([[ref-naca-1953-report-1135]] pp. 614-616).

## Explanation

**Equation of state.** A thermal equation of state relates pressure $p$, specific volume $v = 1/\rho$ and temperature $T$. For a thermally perfect gas ([[ref-naca-1953-report-1135]] p. 614):

$$p = \frac{RT}{v} = \rho R T$$ (NACA 1135, eq. 2)

or in differential form $\frac{dp}{p} - \frac{d\rho}{\rho} - \frac{dT}{T} = 0$ (eq. 3). Real gases deviate; NACA 1135 lists the van der Waals, Berthelot and Beattie-Bridgeman forms (eqs. 4-6).

**Energy.** The first law reads ([[ref-naca-1953-report-1135]] pp. 614-615):

$$dq = du + p\,dv = dh - v\,dp$$ (NACA 1135, eq. 10a)

with $u$ the internal energy and $h = u + pv$ the enthalpy per unit mass (eq. 18). For a thermally perfect gas $du = c_v\,dT$ and $dh = c_p\,dT$ (eqs. 8b, 19b). "Calorically perfect" adds that $c_v$ and $c_p$ are constant, so $u = c_v T + u_r$ (eq. 9).

**Specific heats.** $c_p - c_v = R$ (eq. 13b) and $\gamma = c_p/c_v$ (eq. 14), so

$$c_p = \frac{\gamma R}{\gamma - 1}, \qquad c_v = \frac{R}{\gamma - 1}$$ (NACA 1135, eqs. 16-17)

Kinetic theory gives $\gamma = (n+2)/n$ for a molecule with $n$ effective degrees of freedom (eq. 15); the tables of the report use $\gamma = 7/5$ ([[ref-naca-1953-report-1135]] p. 615).

**Entropy.** $ds = (dq/T)_{rev}$ (eq. 21). For a perfect gas ([[ref-naca-1953-report-1135]] p. 615):

$$s - s_r = c_v \ln \frac{T/T_r}{(\rho/\rho_r)^{\gamma-1}}$$ (NACA 1135, eq. 23b)

equivalently $p/\rho^\gamma = (p_r/\rho_r^\gamma)\, e^{(s-s_r)/c_v}$ (eq. 24). The second law requires $s - s_r \ge 0$ in an adiabatic process (eq. 25).

**Isentropic relations.** If the entropy is constant, $p/\rho^\gamma$ is constant (eq. 34) and

$$\frac{p}{p_t} = \left(\frac{\rho}{\rho_t}\right)^{\gamma} = \left(\frac{T}{T_t}\right)^{\gamma/(\gamma-1)}$$ (NACA 1135, eq. 35)

where the subscript $t$ denotes total (stagnation) conditions, the state the gas would reach if brought to rest isentropically ([[ref-naca-1953-report-1135]] pp. 614, 616). The report tags such equations `[isen]` and warns they "may not be applied to the flow across a shock wave"; equations tagged `[adiab]` need only no heat transfer and do hold across a shock ([[ref-naca-1953-report-1135]] p. 614).

**Energy along a streamline.** For adiabatic flow the total enthalpy is conserved ([[ref-naca-1953-report-1135]] p. 616):

$$h + \frac{V^2}{2} = h_t, \qquad c_p T + \frac{V^2}{2} = c_p T_t$$ (NACA 1135, eqs. 32a-32b)

This links speed to temperature: a gas that speeds up cools.

**What it adds.** First-year physics gives heat, work and temperature; this page adds the specific model of a gas that compressible flow needs: a state equation that closes the flow equations, the two specific heats, entropy as the quantity that decides which changes are possible, and the isentropic shortcuts that hold away from shocks.

## Builds on

- First-year physics: heat, work, temperature, the first law (the "floor" of [[foundations-map]]).

## Leads to

- [[speed-of-sound-and-mach-number]]: the speed of sound is a thermodynamic derivative of this gas model.
- [[euler-equations]]: the state equation closes the inviscid conservation laws.
- [[shock-waves]]: the entropy condition picks the physical shock.
- `energy-equation`, `cavitation-and-mass-transfer-models`, `nucleation-and-condensation` (other branches of [[foundations-map]]).
- [[adr-thread-wet-steam]]

## How rcfd uses it

- The pressure-based variable-density kernel selects an ideal-gas density with `fluid.density = "ideal_gas"` (ADR 0005) or a real gas (Peng-Robinson or Redlich-Kwong, compressible mode only, ADR 0007) ([[adr-thread-early-physics-models]]).
- The wet-steam work (F-221) meets its nozzle comparison "inviscid with a perfect-gas vapour"; a real-gas vapour is a todo item ([[roadmap-compressible-flow-and-acoustics]]).
- The isentropic and area-Mach relations used for the compressible nozzle check are cited to a textbook (Anderson) as "standard textbook formulae" ([[doc-reference-data-compressible-bcs]]).
