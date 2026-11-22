---
type: concept
tags: [foundation, heat-transfer, cht]
sources: [ref-monge-birken-2017-dirichlet-neumann-cht, ref-lienhard-ahtt-6e]
builds_on: [heat-conduction, convective-heat-transfer, boundary-conditions]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Conjugate heat transfer (fluid-solid thermal coupling)

## In short

In conjugate heat transfer the temperature is solved in a fluid and in a neighbouring solid together: convection and conduction in the fluid, conduction alone in the solid. At the shared interface the two must agree on the temperature and on the heat flux, so neither side can be given a fixed boundary condition in advance ([[ref-monge-birken-2017-dirichlet-neumann-cht]] sec. 2).

## Explanation

**The two problems.** The fluid obeys the flow equations with the [[energy-equation]]; the solid only the [[heat-conduction]] equation, $\rho c_p(\Theta)\,d\Theta/dt = -\nabla\cdot q_s$ with $q_s = -\lambda(\Theta)\nabla\Theta$ (Monge and Birken Eq. 2), where for alloys the heat capacity and conductivity can depend strongly on temperature ([[ref-monge-birken-2017-dirichlet-neumann-cht]] sec. 2.2). Monge and Birken call this thermal fluid-structure interaction, "also called conjugate heat transfer"; applications include gas-turbine blade cooling, thermal anti-icing and re-entry vehicles (sec. 1).

**Interface conditions.** On the interface $\Gamma$ (fluid temperature $T$, solid temperature $\Theta$):

$$T(x,t) = \Theta(x,t), \qquad q_f\cdot n = q_s\cdot n, \qquad x \in \Gamma$$ (Monge and Birken, Eqs. 7, 8)

continuity of temperature and of the normal heat flux ([[ref-monge-birken-2017-dirichlet-neumann-cht]] sec. 2.3). They combine the first-kind (temperature) and second-kind (flux) [[boundary-conditions]] of a single conduction problem ([[ref-lienhard-ahtt-6e]] sec. 4.1).

**Contact resistance.** Two solids pressed together never touch perfectly, and the gaps add a temperature drop across the joint: $Q = Ah_c\Delta T$ with an interfacial conductance $h_c$, or a contact resistance $R_t = 1/(h_cA)$; typical conductances are 10 000-25 000 W/m$^2$ K for copper on copper and 2 000-3 700 for stainless steel on stainless steel ([[ref-lienhard-ahtt-6e]] sec. 2.3, Table 2.1). With such a resistance the temperature is no longer continuous; the flux still is.

**Monolithic and partitioned solution.** A monolithic method solves the coupled equations in one code; a partitioned method keeps one solver per domain and exchanges interface data ([[ref-monge-birken-2017-dirichlet-neumann-cht]] sec. 1). In the Dirichlet-Neumann iteration the fluid gets the interface temperature as Dirichlet data, the solid gets the fluid's interface heat flux as Neumann data, and the loop repeats until the interface temperature changes less than a tolerance (Eqs. 13-15) (sec. 2.6). For large time steps its convergence rate is "the aspect ratio times the quotient of heat conductivities"; strong jumps in the material coefficients give fast convergence, and smaller time steps improve it (abstract, sec. 8).

## Builds on

- [[heat-conduction]]: the solid's equation and Fourier's law at the interface.
- [[convective-heat-transfer]]: the fluid side of the interface heat flux.
- [[boundary-conditions]]: Dirichlet and Neumann data, here exchanged between the domains.

## Leads to

- [[thermal-radiation]] (radiation as an additional wall flux), rcfd's solid zones and thermal interfaces

## How rcfd uses it

- Solid cell zones `solids.<name>` (need `energy.enabled` and either `solver.frozen_flow = true` or the coupled algorithm, ADR 0029); `thermal_interfaces.<name>` between two solids with a contact `resistance` in m$^2$K/W; a `temperature = "convection"` wall with `heat_transfer_coefficient`, `free_stream_temperature`, thin walls and `wall_emissivity` (F-051, ADR 0024) ([[doc-case-schema-conjugate-heat-transfer]]).
- Conjugate heat transfer came first with frozen flow (ADR 0024), then with solved flow ([[roadmap-heat-materials-and-chemistry]], [[adr-thread-early-physics-models]]).
- Roadmap area: [[roadmap-heat-materials-and-chemistry]].
