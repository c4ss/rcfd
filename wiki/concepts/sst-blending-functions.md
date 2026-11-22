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

Menter's blending function $F_1$ is about 1 in the inner half of a boundary layer and goes to 0 towards its edge and in the free stream. Multiplying the k-omega equations by $F_1$ and a k-epsilon model rewritten in k-omega form by $1 - F_1$ gives one model that is Wilcox k-omega near walls and k-epsilon outside, keeping k-omega's near-wall strengths without its free-stream sensitivity. $F_2$, reaching further out, confines SST's eddy-viscosity limiter to boundary layers ([[ref-menter-1992-nasa-tm-103975]]).

## Explanation

**The problem it solves.** [[k-omega-wilcox]] is accurate and robust near walls but its results depend strongly on the free-stream $\omega$; [[k-epsilon]] is free-stream independent but weak near walls and in adverse pressure gradients. The Baseline (BSL) idea is "to retain the robust and accurate formulation of the Wilcox k-omega model in the near wall region, and to take advantage of the freestream independence of the k-epsilon model in the outer part of the boundary-layer" ([[ref-menter-1992-nasa-tm-103975]] p. 3).

**Step 1: k-epsilon in k-omega variables.** Substituting $\varepsilon = \beta^* \omega k$ into the Jones-Launder k-epsilon model gives equations of the same form as Wilcox's, with different constants and one extra term in the $\omega$ equation, the cross-diffusion $2 \rho \sigma_{\omega 2} (1/\omega)\, (\partial k/\partial x_j)(\partial \omega/\partial x_j)$ ([[ref-menter-1992-nasa-tm-103975]] eqs. 3-4; [[ref-davidson-turbulence-notes]] sec. 16, eq. 16.1).

**Step 2: blend.** k-omega times $F_1$ plus transformed k-epsilon times $(1 - F_1)$ gives (TMR conservation form) ([[ref-nasa-tmr]] `sst.html`; [[ref-menter-1992-nasa-tm-103975]] eqs. 7-9):
- $\dfrac{\partial (\rho \omega)}{\partial t} + \dfrac{\partial (\rho U_j \omega)}{\partial x_j} = \dfrac{\gamma}{\nu_t} P - \beta \rho \omega^2 + \dfrac{\partial}{\partial x_j}\left[(\mu + \sigma_\omega \mu_t) \dfrac{\partial \omega}{\partial x_j}\right] + 2(1 - F_1) \dfrac{\rho \sigma_{\omega 2}}{\omega} \dfrac{\partial k}{\partial x_j} \dfrac{\partial \omega}{\partial x_j}$
- every constant $\phi = F_1 \phi_1 + (1 - F_1)\, \phi_2$, set 1 the k-omega values, set 2 the transformed k-epsilon values (TMR SST: $\sigma_{k1} = 0.85$, $\sigma_{\omega 1} = 0.5$, $\beta_1 = 0.075$; $\sigma_{k2} = 1.0$, $\sigma_{\omega 2} = 0.856$, $\beta_2 = 0.0828$; $\beta^* = 0.09$). Set 2 is "the exact transformation of the Jones-Launder model ($c_{1\varepsilon} = 1.44$, $c_{2\varepsilon} = 1.92$)" (Menter p. 5).

**F1** (TMR form of the 1994 model) ([[ref-nasa-tmr]] `sst.html`):
- $F_1 = \tanh(\mathrm{arg}_1^4)$
- $\mathrm{arg}_1 = \min\left[ \max\left( \dfrac{\sqrt{k}}{\beta^* \omega d}, \dfrac{500 \nu}{d^2 \omega} \right), \dfrac{4 \rho \sigma_{\omega 2} k}{CD_{k\omega} d^2} \right]$
- $CD_{k\omega} = \max\left(2 \rho \sigma_{\omega 2} \dfrac{1}{\omega} \dfrac{\partial k}{\partial x_j} \dfrac{\partial \omega}{\partial x_j},\, 10^{-20}\right)$ ($10^{-10}$ in the 2003 version)

Reading the arguments ([[ref-menter-1992-nasa-tm-103975]] p. 5, which uses $400 \nu/(y^2 \omega)$ and a different second argument): $\sqrt{k}/(\beta^* \omega d)$ is the turbulent length scale $k^{3/2}/\varepsilon$ over the [[wall-distance]]; it is 2.5 in the log layer and falls to zero at the boundary-layer edge. $500 \nu/(d^2 \omega)$ keeps $F_1 = 1$ in the viscous sublayer. The third argument guards against the "degenerate" solution with small free-stream $\omega$. $F_1$ "is equal to one over about 50% of the boundary-layer and then gradually goes to zero" (Fig. 1); since most production of $k$ and $\omega$ happens in that inner half, BSL behaves like k-omega in boundary layers and like k-epsilon in free shear layers.

**F2** ([[ref-nasa-tmr]] `sst.html`; [[ref-menter-1992-nasa-tm-103975]] eqs. 21-22): $F_2 = \tanh(\mathrm{arg}_2^2)$, $\mathrm{arg}_2 = \max\left(\dfrac{2 \sqrt{k}}{\beta^* \omega d}, \dfrac{500 \nu}{d^2 \omega}\right)$. It is used only in the eddy-viscosity limiter $\nu_t = \dfrac{a_1 k}{\max(a_1 \omega,\, S F_2)}$ ([[bradshaw-assumption-and-sst-limiter]]), and "it is imperative that $F_2$ extends further out into the boundary-layer than $F_1$" because the limiter acts mainly in the wake region; $F_1$ must fall to zero "well within the boundary-layer in order to prevent the freestream dependence" (Menter p. 8).

**Evidence.** On a flat plate with the free-stream $\omega$ reduced by four orders of magnitude, the original k-omega eddy viscosity "changes by almost 100%", BSL "gives the same results for both cases", and SST is also independent of $\omega_f$ ([[ref-menter-1992-nasa-tm-103975]] pp. 10-11).

**What it adds to its prerequisites.** From [[k-omega-wilcox]] it keeps the near-wall equations; from [[k-epsilon]] the outer behaviour; from [[wall-distance]] the coordinate that tells the two regions apart. What is new is the smooth, solution-dependent switch: a zonal model whose zones "do not require an a priori knowledge of the flowfield" (Menter p. 8).

## Builds on

- [[k-omega-wilcox]]: the inner model (set 1).
- [[k-epsilon]]: the outer model, transformed to k-omega form (set 2).
- [[wall-distance]]: $d$ in $\mathrm{arg}_1$ and $\mathrm{arg}_2$.

## Leads to

- [[bradshaw-assumption-and-sst-limiter]] (uses $F_2$), [[k-omega-sst]], [[reynolds-stress-models]] (SSG/LRR uses the same $F_1$ idea), [[les-and-des]] (DDES uses $F_1$ or $F_2$ as shielding)

## How rcfd uses it

- rcfd's SST: $F_1 = \tanh(\mathrm{arg}_1^4)$, $F_2 = \tanh(\mathrm{arg}_2^2)$, $CD_{k\omega} = \max(2 \rho \sigma_{\omega 2}/\omega\; \nabla k \cdot \nabla \omega,\, 10^{-10})$, constants $\sigma_{k1}$ 0.85, $\sigma_{\omega 1}$ 0.5, $\beta_1$ 0.075, $\gamma_1$ 5/9, $\sigma_{k2}$ 1.0, $\sigma_{\omega 2}$ 0.856, $\beta_2$ 0.0828, $\gamma_2$ 0.44; "verified against the TMR page" ([[doc-reference-data-sst-wall]]).
- ADR 0070 audited `sst.rs` term by term against the TMR page and found no defect ([[adr-thread-early-turbulence]]). BSL also exists (ADR 0015).
