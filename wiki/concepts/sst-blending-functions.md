---
type: concept
tags: [foundation, turbulence, rans, sst]
sources: [ref-menter-1992-nasa-tm-103975, ref-nasa-tmr, ref-davidson-turbulence-notes]
builds_on: [k-omega-wilcox, k-epsilon, wall-distance]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# SST blending functions (F1, F2) and cross-diffusion

## In short

Menter's blending function `F1` is about 1 in the inner half of a boundary layer and goes to 0 towards its edge and in the free stream. Multiplying the k-omega equations by `F1` and a k-epsilon model rewritten in k-omega form by `1 - F1` gives one model that is Wilcox k-omega near walls and k-epsilon outside, keeping k-omega's near-wall strengths without its free-stream sensitivity. `F2`, reaching further out, confines SST's eddy-viscosity limiter to boundary layers ([[ref-menter-1992-nasa-tm-103975]]).

## Explanation

**The problem it solves.** [[k-omega-wilcox]] is accurate and robust near walls but its results depend strongly on the free-stream `omega`; [[k-epsilon]] is free-stream independent but weak near walls and in adverse pressure gradients. The Baseline (BSL) idea is "to retain the robust and accurate formulation of the Wilcox k-omega model in the near wall region, and to take advantage of the freestream independence of the k-epsilon model in the outer part of the boundary-layer" ([[ref-menter-1992-nasa-tm-103975]] p. 3).

**Step 1: k-epsilon in k-omega variables.** Substituting `epsilon = beta* omega k` into the Jones-Launder k-epsilon model gives equations of the same form as Wilcox's, with different constants and one extra term in the omega equation, the cross-diffusion `2 rho sigma_w2 (1/omega) dk/dx_j d omega/dx_j` ([[ref-menter-1992-nasa-tm-103975]] eqs. 3-4; [[ref-davidson-turbulence-notes]] sec. 16, eq. 16.1).

**Step 2: blend.** k-omega times `F1` plus transformed k-epsilon times `(1 - F1)` gives (TMR conservation form) ([[ref-nasa-tmr]] `sst.html`; [[ref-menter-1992-nasa-tm-103975]] eqs. 7-9):
- `d(rho omega)/dt + d(rho U_j omega)/dx_j = (gamma/nu_t) P - beta rho omega^2 + d/dx_j[(mu + sigma_w mu_t) d omega/dx_j] + 2(1 - F1)(rho sigma_w2/omega) dk/dx_j d omega/dx_j`
- every constant `phi = F1 phi1 + (1 - F1) phi2`, set 1 the k-omega values, set 2 the transformed k-epsilon values (TMR SST: `sigma_k1 = 0.85`, `sigma_w1 = 0.5`, `beta1 = 0.075`; `sigma_k2 = 1.0`, `sigma_w2 = 0.856`, `beta2 = 0.0828`; `beta* = 0.09`). Set 2 is "the exact transformation of the Jones-Launder model (c1e = 1.44, c2e = 1.92)" (Menter p. 5).

**F1** (TMR form of the 1994 model) ([[ref-nasa-tmr]] `sst.html`):
- `F1 = tanh(arg1^4)`
- `arg1 = min[ max( sqrt(k)/(beta* omega d), 500 nu/(d^2 omega) ), 4 rho sigma_w2 k/(CD_kw d^2) ]`
- `CD_kw = max(2 rho sigma_w2 (1/omega) dk/dx_j d omega/dx_j, 1e-20)` (`1e-10` in the 2003 version)

Reading the arguments ([[ref-menter-1992-nasa-tm-103975]] p. 5, which uses `400 nu/(y^2 omega)` and a different second argument): `sqrt(k)/(beta* omega d)` is the turbulent length scale `k^(3/2)/epsilon` over the [[wall-distance]]; it is 2.5 in the log layer and falls to zero at the boundary-layer edge. `500 nu/(d^2 omega)` keeps `F1 = 1` in the viscous sublayer. The third argument guards against the "degenerate" solution with small free-stream `omega`. `F1` "is equal to one over about 50% of the boundary-layer and then gradually goes to zero" (Fig. 1); since most production of `k` and `omega` happens in that inner half, BSL behaves like k-omega in boundary layers and like k-epsilon in free shear layers.

**F2** ([[ref-nasa-tmr]] `sst.html`; [[ref-menter-1992-nasa-tm-103975]] eqs. 21-22): `F2 = tanh(arg2^2)`, `arg2 = max(2 sqrt(k)/(beta* omega d), 500 nu/(d^2 omega))`. It is used only in the eddy-viscosity limiter `nu_t = a1 k / max(a1 omega, S F2)` ([[bradshaw-assumption-and-sst-limiter]]), and "it is imperative that F2 extends further out into the boundary-layer than F1" because the limiter acts mainly in the wake region; `F1` must fall to zero "well within the boundary-layer in order to prevent the freestream dependence" (Menter p. 8).

**Evidence.** On a flat plate with the free-stream `omega` reduced by four orders of magnitude, the original k-omega eddy viscosity "changes by almost 100%", BSL "gives the same results for both cases", and SST is also independent of `omega_f` ([[ref-menter-1992-nasa-tm-103975]] pp. 10-11).

**What it adds to its prerequisites.** From [[k-omega-wilcox]] it keeps the near-wall equations; from [[k-epsilon]] the outer behaviour; from [[wall-distance]] the coordinate that tells the two regions apart. What is new is the smooth, solution-dependent switch: a zonal model whose zones "do not require an a priori knowledge of the flowfield" (Menter p. 8).

## Builds on

- [[k-omega-wilcox]]: the inner model (set 1).
- [[k-epsilon]]: the outer model, transformed to k-omega form (set 2).
- [[wall-distance]]: `d` in `arg1` and `arg2`.

## Leads to

- [[bradshaw-assumption-and-sst-limiter]] (uses `F2`), [[k-omega-sst]], [[reynolds-stress-models]] (SSG/LRR uses the same `F1` idea), [[les-and-des]] (DDES uses `F1` or `F2` as shielding)

## How rcfd uses it

- rcfd's SST: `F1 = tanh(arg1^4)`, `F2 = tanh(arg2^2)`, `CDkw = max(2 rho sigma_w2/omega dk.dw, 1e-10)`, constants `sigma_k1 0.85`, `sigma_w1 0.5`, `beta1 0.075`, `gamma1 5/9`, `sigma_k2 1.0`, `sigma_w2 0.856`, `beta2 0.0828`, `gamma2 0.44`; "verified against the TMR page" ([[doc-reference-data-sst-wall]]).
- ADR 0070 audited `sst.rs` term by term against the TMR page and found no defect ([[adr-thread-early-turbulence]]). BSL also exists (ADR 0015).
