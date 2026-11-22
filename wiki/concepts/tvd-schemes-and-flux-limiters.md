---
type: concept
tags: [foundation, discretisation, convection-scheme, limiter]
sources: [ref-jasak-1996-phd-thesis, ref-berger-aftosmis-murman-2005-slope-limiters, ref-van-leer-1992-icase-92-43]
builds_on: [upwind-and-central-differencing, linear-upwind-scheme]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# TVD schemes and flux limiters

## In short

High-order convection schemes wiggle near steep gradients; first-order upwind does not but smears. A flux limiter switches smoothly between them: where the solution is smooth it keeps the high-order flux, near a jump or an extremum it falls back to upwind. "Total variation diminishing" (TVD) is the condition that guarantees no new wiggles in 1D ([[ref-jasak-1996-phd-thesis]]; [[ref-berger-aftosmis-murman-2005-slope-limiters]]).

## Explanation

**Total variation.** The total variation of a discrete solution is the sum over faces of the absolute difference between the two cell values around the face; a scheme is TVD if it does not grow from one time step to the next (Sweby's condition), so no new extrema appear ([[ref-jasak-1996-phd-thesis]] 3.4.1, Eqns. 3.64-3.65). Harten introduced the notion to characterise oscillation-free flux-limited schemes, and flux limiting gives schemes of higher than first order without the oscillations of classical second-order schemes ([[ref-jasak-1996-phd-thesis]] 3.4.1).

**Sweby's form.** Write the scheme as the upwind flux plus a limited anti-diffusive correction towards a second-order flux; the correction is multiplied by $\psi(r)$, a function of the ratio $r$ of consecutive gradients of the solution ([[ref-jasak-1996-phd-thesis]] Eqns. 3.66-3.69; [[ref-berger-aftosmis-murman-2005-slope-limiters]] II.A, Eqns. 1-2). Choosing $\psi$ inside Sweby's shaded region gives a second-order TVD scheme ([[ref-berger-aftosmis-murman-2005-slope-limiters]] Fig. 1). Second order away from extrema needs $\psi(1) = 1$ ("the method should not do any limiting when the solution is a linear function"), and $\psi$ must be symmetric ([[ref-berger-aftosmis-murman-2005-slope-limiters]] II.A).

**Common limiters** (for $R \ge 0$; all set $\psi = 0$ for $R \le 0$, an extremum), in order of increasing dissipation ([[ref-berger-aftosmis-murman-2005-slope-limiters]] Eq. 3):
- superbee $\min[\max(1, R), 2, 2R]$ (follows the boundary of the TVD region);
- Barth-Jespersen (see [[slope-limiters-unstructured]]);
- van Leer $2R/(R + 1)$;
- van Albada $(R^2 + R)/(R^2 + 1)$;
- min (minmod) $\min(1, R)$.

Superbee and Barth-Jespersen "are the most compressive, and are known to turn smooth waves into square waves" ([[ref-berger-aftosmis-murman-2005-slope-limiters]] II.A). Jasak's summary is that TVD schemes are "either too diffusive" (MINMOD) "or too compressive" (SUPERBEE) ([[ref-jasak-1996-phd-thesis]] 3.4.1).

**Normalised-variable view.** With the upwind (U), central (C) and downwind (D) values around a face, Leonard's normalised variable maps every scheme to a curve of the normalised face value against the normalised central value; Gaskell and Lau's convection boundedness criterion (CBC) marks the bounded region of that diagram. Only upwind differencing satisfies it among the basic schemes; CD, LUD and QUICK do not ([[ref-jasak-1996-phd-thesis]] 3.4.1, Fig. 3.11). The Gamma scheme blends UD and CD smoothly inside that region; its face value does not depend on the Courant number, whereas the TVD criterion does, which Jasak finds unacceptable for steady solutions ([[ref-jasak-1996-phd-thesis]] 3.4.1, 3.4.3).

**Convergence.** Any bounded scheme above first order must be nonlinear (the face weights depend on the solution). NVD schemes may not have a unique steady solution, and TVD convergence is proven only for explicit calculations ([[ref-jasak-1996-phd-thesis]] 3.4.1.3). Non-smooth limiter formulas lead to "limiter chatter, even in near constant portions of the flow field, making it difficult to achieve steady state either with time marching or Newton's method" ([[ref-berger-aftosmis-murman-2005-slope-limiters]] I).

**Origin in upwind schemes.** The limiters grew out of upwind finite-volume schemes for gas dynamics (van Leer's MUSCL line); the first-order upwind scheme they extend is the Godunov update ([[ref-van-leer-1992-icase-92-43]] 2).

## Builds on

- [[upwind-and-central-differencing]]: the bounded first-order scheme and the unbounded second-order one being blended.
- [[linear-upwind-scheme]]: the second-order upwind flux that slope limiting restricts.

## Leads to

- [[slope-limiters-unstructured]]
- rcfd pages: [[simple]], [[vof]]

## How rcfd uses it

- F-022 (done): rcfd's convection schemes include QUICK/MUSCL and TVD limiters ([[roadmap-numerics-and-solvers]]).
- **Limit cycle at $Re_h = 400$:** on the backward-facing step a TVD limiter (vanLeer) with SIMPLE 0.7/0.3 limit-cycles; "the working recipe is vanLeer + SIMPLEC alpha_u 0.9" ([[simple]]). This matches the convergence difficulty of nonlinear limited schemes described above ([[ref-berger-aftosmis-murman-2005-slope-limiters]] I).
- VOF keeps the volume-fraction interface sharp with interface compression ([[vof]]); which bounded scheme carries $\alpha$ is not recorded on that page.
