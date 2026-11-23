---
type: concept
tags: [foundation, heat-transfer]
sources: [ref-lienhard-ahtt-6e, ref-openstax-college-physics-ch14]
builds_on: [energy-equation, boundary-layer]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Convective heat transfer (heat transfer coefficient, Nusselt and Prandtl numbers)

## In short

Convection is heat carried between a surface and a moving fluid. Its rate is written with a heat transfer coefficient $h$ times the temperature difference (Newton's law of cooling); $h$ is set by the thermal boundary layer near the wall and is reported as a Nusselt number that depends on the Reynolds and Prandtl numbers ([[ref-lienhard-ahtt-6e]] sec. 1.3, 6.3-6.5; [[ref-openstax-college-physics-ch14]] 14.6).

## Explanation

**Mechanism.** Convection is heat transfer by the macroscopic movement of a fluid; it can be natural (driven by buoyancy, [[boussinesq-buoyancy-approximation]]) or forced (driven by a fan or pump) ([[ref-openstax-college-physics-ch14]] 14.6).

**Newton's law of cooling.** The heat flux from a body at $T_{body}$ to a fluid at $T_\infty$:

$$q = \bar h\,(T_{body} - T_\infty)$$ (Lienhard eq. 1.17)

with the film or heat transfer coefficient $h$ (W/m$^2$ K). Newton "never wrote such an expression"; the "law" is a definition of $h$. In forced convection with modest temperature differences $h$ does not depend on $\Delta T$; in natural convection it varies as a weak power of $\Delta T$ ($\Delta T^{1/4}$ or $\Delta T^{1/3}$), in boiling about as $\Delta T^2$; illustrative values span about six orders of magnitude ([[ref-lienhard-ahtt-6e]] sec. 1.3, Table 1.1).

**$h$ from the temperature field.** At the wall the fluid is at rest, so heat leaves by conduction through the fluid:

$$h = \frac{q_w}{T_w - T_\infty} = -\frac{k}{T_w - T_\infty}\left.\frac{\partial T}{\partial y}\right|_{y=0}$$ (Lienhard eq. 6.35)

which is why $h$ depends on the temperature profile in the thermal [[boundary-layer]], found from the [[energy-equation]] ([[ref-lienhard-ahtt-6e]] sec. 6.3).

**Dimensionless groups.** Dimensional analysis gives $Nu_x = fn(Re_x, Pr)$ (Lienhard eq. 6.44), with

$$Nu_x = \frac{hx}{k}, \qquad Re_x = \frac{u_\infty x}{\nu}, \qquad Pr = \frac{\nu}{\alpha} = \frac{\mu c_p}{k}$$ (Lienhard sec. 6.4)

The Nusselt number uses the fluid conductivity and "relates only to the fluid"; the similar-looking Biot number $Bi = hL/k_b$ uses the body's conductivity and decides whether a body can be treated as having one uniform temperature (sec. 1.3). The Prandtl number compares momentum and heat diffusion: about 2/3 for monatomic gases, 5/7 for diatomic gases, 1-10 for simple liquids, $10^{-2}$ or less for liquid metals; for $Pr > 1$ the velocity boundary layer is thicker than the thermal one, for $Pr < 1$ thinner (sec. 6.4, eq. 6.46).

**Laminar flat plate.** For $\nu = \alpha$ ($Pr = 1$) the temperature profile equals the Blasius velocity profile and $Nu_x = 0.33206\,Re_x^{1/2}$ (eq. 6.43). For other Prandtl numbers $\delta_t/\delta \approx Pr^{-1/3}$ and

$$Nu_x = 0.332\,Re_x^{1/2}Pr^{1/3}, \quad Pr \ge 0.6$$ (Lienhard eq. 6.58)

within 2 % of the exact solution; the plate average is $\overline{Nu}_L = 0.664\,Re_L^{1/2}Pr^{1/3}$ (eq. 6.68); properties are taken at the film temperature ([[ref-lienhard-ahtt-6e]] sec. 6.3, 6.5).

## Builds on

- [[energy-equation]]: the temperature field whose wall gradient gives $h$.
- [[boundary-layer]]: the thin near-wall layer where velocity and temperature change.

## Leads to

- [[conjugate-heat-transfer]], [[heat-exchanger-effectiveness-ntu]], [[droplet-evaporation-and-breakup]] (Nusselt and Sherwood correlations for droplets), [[boussinesq-buoyancy-approximation]] (natural convection)

## How rcfd uses it

- A convection wall condition with `heat_transfer_coefficient` and an ambient temperature exists in the conjugate-heat-transfer case schema ([[doc-case-schema-conjugate-heat-transfer]]).
- The [[differentially-heated-cavity]] validation reports average Nusselt numbers against the de Vahl Davis benchmark.
