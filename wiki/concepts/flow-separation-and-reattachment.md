---
type: concept
tags: [foundation, turbulence, boundary-layer, validation]
sources: [ref-nasa-tmr, ref-menter-1992-nasa-tm-103975, ref-davidson-turbulence-notes]
builds_on: [boundary-layer]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Flow separation and reattachment

## In short

A boundary layer separates when the near-wall fluid, slowed by friction and pushed back by a rising pressure (adverse pressure gradient) or a sudden geometry change, can no longer follow the wall: the wall shear stress drops to zero and a region of reversed flow forms. Downstream the flow may reattach, where the wall shear crosses zero again. Predicting where this happens is one of the hardest tests for a RANS turbulence model ([[ref-nasa-tmr]], [[ref-menter-1992-nasa-tm-103975]]).

## Explanation

**Mechanism.** In the [[boundary-layer]] the pressure is imposed by the outer flow. On an aerofoil's suction side the pressure reaches a minimum and then rises as the velocity drops: the adverse-pressure-gradient (APG) region ([[ref-davidson-turbulence-notes]] sec. 16, p. 180). The rising pressure decelerates the flow, and the slow near-wall fluid most; momentum exchange with the faster outer fluid (molecular and turbulent shear stress) works against this, which is why turbulence's increased diffusivity "reduces or delays" separation ([[ref-davidson-turbulence-notes]] p. 77). Separation is where the wall shear stress, and so `Cf`, passes through zero ([[skin-friction-coefficient]]). At a sudden step the geometry alone causes it: "a turbulent boundary layer encounters a sudden back step, causing flow separation. The flow then reattaches and recovers downstream of the step" ([[ref-nasa-tmr]] `backstep_val.html`).

**Reattachment length as a test.** For the Driver and Seegmiller backward-facing step (`Re_H` about 36 000, boundary layer about 1.5 H before the step), "One of the key measures of success for this flowfield is the prediction of reattachment point downstream of the step"; the measured value is `x/H = 6.26 +- 0.10`, from the zero skin-friction location found by oil-flow interferometry ([[ref-nasa-tmr]] `backstep_val.html`).

**Why models differ here.** The amount of turbulent shear stress decides how well the layer resists the APG:
- k-epsilon "predicts significantly too high shear-stress levels and thereby delays (or completely prevents) separation" ([[ref-menter-1992-nasa-tm-103975]] p. 2).
- Wilcox k-omega does better, but both k-omega forms "still underpredict the amount of separation for severe adverse pressure gradient flows" (p. 3).
- SST's limiter caps the shear stress at `rho a1 k` in boundary layers, aimed at exactly these flows ([[bradshaw-assumption-and-sst-limiter]]).
- Menter reports that Spalart-Allmaras "gives very high backflow velocities in the recirculation region of a backward-facing step flow" (p. 2).

**What it adds to its prerequisite.** The [[boundary-layer]] page gives the thin-layer picture and the pressure-gradient dependence; separation is where that picture breaks down (the layer leaves the wall, the thin-layer assumptions fail), and it turns a model's shear-stress level into a measurable position: the separation and reattachment points.

## Builds on

- [[boundary-layer]]: wall shear, pressure gradient, the near-wall momentum deficit.

## Leads to

- [[backward-facing-step]], [[naca0012]] (stall), [[bradshaw-assumption-and-sst-limiter]]

## How rcfd uses it

- rcfd's turbulent [[backward-facing-step]] uses the Driver and Seegmiller reattachment `x/H = 6.26 +/- 0.10`; on the corrected step BFS-C (window 6.16 to 6.36) k-epsilon Standard gives 5.350 and Realizable 6.413, RNG passes marginally; wall-resolved SST 6.554 and BSL 5.956 miss ([[backward-facing-step]], [[roadmap-turbulence]]).
- NACA 0012 lift: SA peaks about 3 degrees late (1.5278 at 20 degrees) and SST about 3 degrees early (1.3973 at 14 degrees) in job 48Y ([[naca0012]]).
- DES in rcfd is "not validated on a separated flow"; F-207 has no separated-flow validation ([[doc-case-schema-turbulence]], [[roadmap-turbulence]]).
