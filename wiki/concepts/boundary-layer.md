---
type: concept
tags: [foundation, turbulence, boundary-layer]
sources: [ref-davidson-turbulence-notes, ref-nasa-tmr, ref-menter-1992-nasa-tm-103975]
builds_on: [viscosity, shear-stress, dimensionless-numbers]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Boundary layer

## In short

A boundary layer is the thin layer next to a wall in which the velocity rises from zero at the wall (no slip) to the outer-flow value. Viscous and turbulent shear stresses are large there and nowhere else, so the wall friction, heat transfer and separation are all decided inside it. It grows downstream, and its thickness is measured by `delta_99`, the displacement thickness `delta*` and the momentum thickness `theta` ([[ref-davidson-turbulence-notes]], [[ref-nasa-tmr]]).

## Explanation

**The thin-layer equations.** For steady, 2D, incompressible flow along a wall (`x` along, `y` normal), with the layer thin so that `V << U` and `dU/dx << dU/dy`, the equations reduce to ([[ref-davidson-turbulence-notes]] eqs. 3.42, 6.12-6.14):

`U dU/dx + V dU/dy = -(1/rho) dp/dx + d/dy[ nu dU/dy - mean(u'v') ]`,  `dp/dy = 0`,  `dU/dx + dV/dy = 0`

with `U = V = 0` at the wall and `U -> U_e` far away. The pressure is imposed by the outer flow (`dp/dy = 0` across the layer). For laminar flow the Reynolds-stress term `-mean(u'v')` is absent. The bracket is the total shear stress per unit density ([[shear-stress]] plus [[reynolds-stress]]).

**Thicknesses** ([[ref-davidson-turbulence-notes]] App. E.2; [[ref-nasa-tmr]] `flatplate_val.html`):
- `delta_99`: the distance where `U = 0.99 U_e`; it depends on the edge of the profile and is less robust.
- displacement thickness `delta* = int_0^inf (1 - U/U_e) dy`: how far the wall is effectively pushed into the outer flow by the velocity deficit.
- momentum thickness `theta = int_0^inf (U/U_e)(1 - U/U_e) dy` (the TMR adds the density ratio for compressible flow); `Re_theta = U_e theta/nu` is the usual Reynolds number of a turbulent boundary layer.

**Momentum balance.** On a flat plate there is no pressure gradient; the wall shear stress is balanced by a loss of momentum flux in the convective terms, which is why the layer thickens downstream; channel flow, by contrast, is driven by a pressure drop ([[ref-davidson-turbulence-notes]] sec. 3.3.1, p. 51). Combining that balance with mass conservation gives the momentum thickness (App. E).

**Laminar and turbulent.** A laminar flat-plate layer has the exact similarity solution of Blasius ([[blasius-solution]]); transition happens at about `Re_x = 500 000` ([[ref-davidson-turbulence-notes]] p. 77). A turbulent boundary layer has an inner region next to the wall that is "principally the same" as in channel flow (viscous sublayer, buffer layer, log layer, see [[law-of-the-wall]]), but the log law holds only to about `y/delta = 0.1`, and the outer part is "highly intermittent" ([[ref-davidson-turbulence-notes]] sec. 6.4, pp. 96-97). On the TMR flat plate at `Re = 5e6` per unit length the maximum thickness is "about 0.03 L" ([[ref-nasa-tmr]] `flatplate.html`).

**Pressure gradients.** On an aerofoil's suction side the pressure falls to a minimum and then rises downstream as the velocity drops: the rising part is the adverse-pressure-gradient (APG) region ([[ref-davidson-turbulence-notes]] sec. 16, p. 180). An APG decelerates the near-wall fluid and can lead to separation ([[flow-separation-and-reattachment]]). This is the regime where turbulence models differ most: k-epsilon predicts "significantly too high shear-stress levels" there ([[ref-menter-1992-nasa-tm-103975]] p. 2), which motivated the SST limiter ([[bradshaw-assumption-and-sst-limiter]]).

**What it adds to its prerequisites.** [[viscosity]] and [[shear-stress]] give the stress `mu dU/dy`; the Reynolds number of [[dimensionless-numbers]] says when viscous effects are confined to a thin layer. The boundary layer is the region where these effects concentrate, with its own simplified equations, thickness measures and the pressure-gradient dependence that decides separation.

## Builds on

- [[viscosity]]: the viscous stress that brings the velocity to zero at the wall.
- [[shear-stress]]: the wall and internal shear stress `tau = mu dU/dy`.
- [[dimensionless-numbers]]: Reynolds numbers `Re_x`, `Re_theta`.

## Leads to

- [[blasius-solution]], [[skin-friction-coefficient]], [[friction-velocity-and-y-plus]], [[flow-separation-and-reattachment]], [[bradshaw-assumption-and-sst-limiter]]
- [[flat-plate]], [[k-omega-sst]]

## How rcfd uses it

- The zero-pressure-gradient [[flat-plate]] is rcfd's boundary-layer benchmark: Blasius, skin-friction correlations and the NASA TMR 2DZP turbulent case ([[flat-plate]]).
- The fan passage meshes size a boundary-layer stack from a target `y+` and a friction estimate, `h1 = 2 y+ nu / u_tau` with `Cf = 0.0576 Re_c^(-1/5)` at mid-span ([[fan-passage-mesh]]).
