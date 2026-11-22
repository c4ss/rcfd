---
type: concept
tags: [foundation, multiphase, vof]
sources: [ref-barreiro-villaverde-2021-jet-wiping-vof, ref-sharifi-2024-mules-interface-advection]
builds_on: [volume-fraction-transport]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Interface compression (artificial compression flux in algebraic VOF)

## In short

Algebraic VOF smears the interface over more and more cells as the flow moves it. Interface compression adds an artificial flux that pushes $\alpha$ back towards the interface, normal to it, so the interface stays a few cells thick ([[ref-barreiro-villaverde-2021-jet-wiping-vof]] sec. II.B; [[ref-sharifi-2024-mules-interface-advection]] sec. 1).

## Explanation

**The problem.** The volume fraction equation of [[volume-fraction-transport]] has no physical diffusion, but any algebraic scheme adds numerical diffusion, which spreads the 0-to-1 jump.

**The fix.** Define an artificial relative velocity $\vec U_r = \vec U_l - \vec U_g$ between liquid and gas at the interface; with $\vec U = \alpha\vec U_l + (1-\alpha)\vec U_g$ the volume fraction equation gets an extra term ([[ref-barreiro-villaverde-2021-jet-wiping-vof]] sec. II.B):

$$\frac{\partial\alpha}{\partial t} + \nabla\cdot(\vec U\alpha) + \nabla\cdot\big(\vec U_r\,\alpha(1-\alpha)\big) = 0$$ (Barreiro-Villaverde et al., Eq. 4)

The factor $\alpha(1-\alpha)$ is zero in pure fluid, so the term only acts in interface cells; the compressive flux acts in the direction normal to the interface and counters numerical diffusion ([[ref-sharifi-2024-mules-interface-advection]] sec. 1). Its advantages are low cost and simple implementation (Barreiro-Villaverde et al.). The strength is set by a coefficient ($C_\alpha$, "IC coefficient"); with $C_\alpha = 1$ "there is no additional relaxation of the interface compression" ([[ref-barreiro-villaverde-2021-jet-wiping-vof]] sec. II.B). A limiter such as MULES keeps the result bounded ([[volume-fraction-transport]]).

**The price.** In a droplet test with the IC coefficient from 0 to 2, the interface advection error fell from 4.8 % to 3.95 % but the mass conservation error "rose steeply from 0% to 23.19%, driven by parasitic currents and numerical instabilities"; above 1.4 the advection error no longer improved ([[ref-sharifi-2024-mules-interface-advection]] abstract). Compression causes parasitic currents in surface-tension-dominated flows (Deshpande et al.), and Hoang et al. found the best balance between parasitic flow and numerical diffusion at a coefficient of 1 (same, sec. 1).

**What it adds.** Plain algebraic advection keeps $\alpha$ bounded but lets the interface thicken; compression keeps it sharp without a geometric reconstruction.

## Builds on

- [[volume-fraction-transport]]: the equation the compression term is added to, and the limiter that keeps it bounded.

## Leads to

- [[surface-tension-and-curvature]] (the curvature is computed from the compressed $\alpha$; compression and curvature errors together drive parasitic currents)
- [[vof]]

## How rcfd uses it

- rcfd's VOF uses interface compression (F-055, ADR 0049); the case key `compression` defaults to 1 ([[vof]], [[doc-case-schema-multiphase]]).
- Verified with the Zalesak slotted disk, hydrostatics, the dam break and the Hysing rising bubble ([[adr-thread-vof-multiphase-particles]]).
