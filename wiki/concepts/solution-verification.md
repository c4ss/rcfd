---
type: concept
tags: [foundation, verification-validation]
sources: [ref-nparc-vv-tutorial, ref-celik-2008-jfe-discretisation-uncertainty, ref-oberkampf-trucano-2002-sand2002-0529, ref-mfix-vv-manual]
builds_on: [truncation-error-and-order-of-accuracy, sparse-linear-systems]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Solution verification (numerical error of one calculation)

## In short

Solution verification estimates how far one particular computed result is from the exact solution of the equations being solved, without knowing that solution. The numerical error has three main parts: round-off, incomplete iterative convergence and discretisation (mesh and time step). The last is estimated from the same case on several systematically refined meshes ([[ref-nparc-vv-tutorial]] `errors.html`; [[ref-celik-2008-jfe-discretisation-uncertainty]]).

## Explanation

**Place in V&V.** Code verification asks whether the software is right; solution verification asks how accurate "a particular numerical solution" is ([[ref-mfix-vv-manual]] sec. 1.1). Roache's "verification of a calculation" is error *estimation*: an error band on one result, usually from a grid convergence study ([[ref-nparc-vv-tutorial]] `verassess.html`). It may not be fully attainable, since grid convergence may not be reachable for a complex case ([[ref-oberkampf-trucano-2002-sand2002-0529]] sec. 3.2.3).

**Error sources and checks** ([[ref-nparc-vv-tutorial]] `errors.html`):
- *Round-off:* finite-precision arithmetic, usually insignificant; checked by running at higher precision.
- *Iterative convergence:* the solver stopped before the discrete equations were solved exactly. Residuals should drop and level off, usually by 3 to 4 orders of magnitude, and monitored quantities (lift, drag) should stop changing ([[ref-nparc-vv-tutorial]] `iterconv.html`). Celik et al. require "at least three (preferably four) orders of magnitude decrease in the normalized residuals for each equation" before any discretisation estimate, and an iteration error at least one order of magnitude below the discretisation error (App. A) ([[ref-celik-2008-jfe-discretisation-uncertainty]]). The residual is the misfit of the discrete equations at the current iterate ([[sparse-linear-systems]]).
- *Discretisation:* the difference between the discrete and exact solutions of the PDEs; for a consistent scheme it vanishes as the spacing goes to zero ([[ref-nparc-vv-tutorial]]; [[ref-oberkampf-trucano-2002-sand2002-0529]] sec. 3.3.1).

**Grid convergence study.** With the error model `E = C h^p` ([[ref-nparc-vv-tutorial]] `spatconv.html`):
- Solve on three grids, finest `h1`, refined systematically (Celik: "desirable" ratio `r = h_coarse/h_fine > 1.3`; NPARC: `r >= 1.1`); for unstructured meshes a representative size `h = [(1/N) sum dV_i]^(1/3)` (Celik Eq. 1).
- Observed order: `p = ln((f3 - f2)/(f2 - f1)) / ln(r)` for constant `r` ([[ref-nparc-vv-tutorial]]); for unequal ratios Celik's Eq. 3 solves for `p` by fixed-point iteration, and a negative ratio `eps32/eps21` signals oscillatory convergence.
- Richardson extrapolation `f_exact ~ f1 + (f1 - f2)/(r^p - 1)` estimates the zero-spacing value (NPARC; Celik Eq. 4).
- The error band: Roache's grid convergence index `GCI = Fs |eps| / (r^p - 1)` with `Fs = 1.25` for three or more grids and `3` for two ([[ref-nparc-vv-tutorial]]; Celik Eq. 7). The full procedure and rcfd's use are on [[gci-richardson]].
- Asymptotic range: `GCI_23 ~ r^p GCI_12`; agreement of the observed order with the formal order indicates it, but the converse "should not necessarily be taken as a sign of unsatisfactory calculations" ([[ref-nparc-vv-tutorial]]; [[ref-celik-2008-jfe-discretisation-uncertainty]] Step 3).

> [!note] The two procedures differ on the minimum refinement ratio
> NPARC recommends `r >= 1.1`, so that discretisation error can be separated from iterative and round-off error ([[ref-nparc-vv-tutorial]]); Celik et al. call `r > 1.3` "desirable", "based on experience, and not on formal derivation" ([[ref-celik-2008-jfe-discretisation-uncertainty]]). Both are recommendations, not limits.

**Worked example** (NPARC, supersonic inlet pressure recovery on grids at spacing 1, 2, 4): 0.97050 / 0.96854 / 0.96178, `p ~ 1.786` against a formal 2.0, extrapolated 0.97130, GCI_12 = 0.103 %, asymptotic ratio about 1.002 ([[ref-nparc-vv-tutorial]]).

**What it does not cover.** The band covers numerical error only: Celik's sample result "does not account for modeling errors" ([[ref-celik-2008-jfe-discretisation-uncertainty]]). Comparing with a measurement is [[validation-comparison]].

**What it adds.** [[truncation-error-and-order-of-accuracy]] predicts how error scales with spacing; solution verification turns that scaling, applied to actual results on several meshes, into an error estimate for the finest one.

## Builds on

- [[truncation-error-and-order-of-accuracy]]: the `C h^p` error model and the formal order.
- [[sparse-linear-systems]]: residuals, which measure iterative convergence.

## Leads to

- [[gci-richardson]] (the procedure in detail and its rcfd uses), [[validation-comparison]], [[verification-vs-validation]], [[a-posteriori-error-indicators]].

## How rcfd uses it

- The FAN-01 mesh study of ADR 0086 is called "first solution-verification study"; its bands are "NOT shown credible" because the empty-passage check failed ([[verification-vs-validation]], [[gci-richardson]]).
- The F-870 numerical band uses the three-mesh GCI with factor 1.25 and the order capped at 1 ([[gci-richardson]]).
- For the cylinder, rcfd counts an extrapolation only if the observed order is in [1.5, 2.5] (ADR 0763) ([[gci-richardson]]).
