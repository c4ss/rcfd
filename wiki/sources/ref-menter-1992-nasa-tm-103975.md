---
type: source
tags: [reference, foundation, turbulence, sst, public-domain]
sources: []
origin: https://ntrs.nasa.gov/citations/19930013620
licence: "Work of the US Gov. Public Use Permitted." (NTRS record, copyright determination GOV_PUBLIC_USE_PERMITTED, checked 2026-11-22)
read: 2026-11-22
raw: raw/menter-1992-nasa-tm-103975.pdf
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Menter (1992), Improved two-equation k-omega turbulence models for aerodynamic flows, NASA TM-103975

**Origin:** F. R. Menter, "Improved Two-Equation k-omega Turbulence Models for Aerodynamic Flows", NASA Technical Memorandum 103975, NASA Ames Research Center, October 1992 (NTRS 19930013620, N93-22809). PDF: https://ntrs.nasa.gov/api/citations/19930013620/downloads/19930013620.pdf, copied to [raw/menter-1992-nasa-tm-103975.pdf](../raw/menter-1992-nasa-tm-103975.pdf).
**Kind:** NASA technical report (scanned, 38 PDF pages; report page n is PDF page n + 4).
**Licence:** "Work of the US Gov. Public Use Permitted." (NTRS).
**Read:** 2026-11-22 by job p6-turbulence: Summary, Introduction (pp. 1-3), "The Turbulence Model" (BSL and SST, pp. 3-9), Boundary Conditions (p. 9), Numerical Method (p. 9), the flat-plate result (pp. 10-11) and Appendix (pp. 28-29). The equations were read from page images, the text from the OCR layer. The results chapters for other flows were not read.

## Summary

The report introduces two k-omega models. The Baseline (BSL) model keeps Wilcox's k-omega in the inner part of a boundary layer and blends to the Jones-Launder k-epsilon (rewritten in k-omega form) towards the edge and in free shear layers, removing the free-stream dependence of Wilcox's model. The Shear-Stress Transport (SST) model adds a limit on the eddy viscosity based on Bradshaw's assumption that the principal shear stress is proportional to k, so that the shear stress is "transported" as in the Johnson-King model and adverse-pressure-gradient flows are predicted better.

## Key points

### Introduction (pp. 1-3): what was wrong with the earlier models
- Algebraic models (Cebeci-Smith, Baldwin-Lomax, Johnson-King) become "very complicated and often ambiguous" in complex flows "mainly because of the difficulty to define an algebraic length-scale" (p. 1).
- Two-equation eddy-viscosity models "solve two transport equations, generally one for the turbulent kinetic energy and another one related to the turbulent length- (or time-) scale" (p. 2).
- k-epsilon: "the most disturbing" shortcoming "is the lack of sensitivity to adverse pressure-gradients": it "predicts significantly too high shear-stress levels and thereby delays (or completely prevents) separation" (p. 2). Low-Re k-epsilon models need damping functions in the sublayer and $\varepsilon$ is not zero at a wall, both sources of numerical stiffness (p. 2).
- Wilcox k-omega: one equation for k and one for "the specific turbulent dissipation rate (or turbulence frequency) $\omega$"; better under adverse pressure gradients than k-epsilon; no damping functions and "straightforward Dirichlet boundary conditions" (pp. 2-3). Its "major shortcoming": results "depend strongly on the freestream values, $\omega_f$"; the eddy viscosity "can be changed by more than 100% just by using different values for $\omega_f$" (p. 3).
- Both k-omega forms "still underpredict the amount of separation for severe adverse pressure gradient flows" (p. 3).

### BSL model (pp. 3-5, eqs. 1-13)
- Wilcox k-omega, eqs. (1)-(2): $\dfrac{\partial(\rho k)}{\partial t} + \dfrac{\partial(\rho u_j k)}{\partial x_j} = P_k - \beta^* \rho \omega k + \dfrac{\partial}{\partial x_j}\left[(\mu + \sigma_{k1} \mu_t) \dfrac{\partial k}{\partial x_j}\right]$; $\dfrac{\partial(\rho \omega)}{\partial t} + \dfrac{\partial(\rho u_j \omega)}{\partial x_j} = \gamma_1 P_\omega - \beta_1 \rho \omega^2 + \dfrac{\partial}{\partial x_j}\left[(\mu + \sigma_{\omega 1} \mu_t) \dfrac{\partial \omega}{\partial x_j}\right]$.
- Transformed k-epsilon, eqs. (3)-(4): same form with set-2 constants plus the cross-diffusion term $2 \rho \sigma_{\omega 2} \dfrac{1}{\omega} \dfrac{\partial k}{\partial x_j} \dfrac{\partial \omega}{\partial x_j}$ in the $\omega$ equation.
- Production, eqs. (5)-(6): $P_k = \mu_t \dfrac{\partial u_i}{\partial x_j} \left(\dfrac{\partial u_i}{\partial x_j} + \dfrac{\partial u_j}{\partial x_i}\right) - \tfrac{2}{3} \rho k \delta_{ij} \dfrac{\partial u_i}{\partial x_j}$; $P_\omega$ the same with $\rho$ in place of $\mu_t$ and $\omega$ in place of $k$.
- Blend, eqs. (7)-(9): eqs. (1)-(2) times $F_1$ plus eqs. (3)-(4) times $(1 - F_1)$; the cross-diffusion term carries $(1 - F_1)$; every constant $\phi = F_1 \phi_1 + (1 - F_1)\, \phi_2$.
- Set 1 (Wilcox): $\sigma_{k1} = 0.5$, $\sigma_{\omega 1} = 0.5$, $\beta_1 = 0.0750$, $\beta^* = 0.09$, $\kappa = 0.41$, $\gamma_1 = \beta_1/\beta^* - \sigma_{\omega 1} \kappa^2/\sqrt{\beta^*}$. Set 2 (Jones-Launder): $\sigma_{k2} = 1.0$, $\sigma_{\omega 2} = 0.856$, $\beta_2 = 0.0828$, same $\beta^*$, $\kappa$; set 2 is "the exact transformation of the Jones-Launder model ($c_{1\varepsilon} = 1.44$, $c_{2\varepsilon} = 1.92$)" (p. 5).
- Eddy viscosity eq. (10) $\nu_t = k/\omega$; turbulent stress eq. (11) $\tau_{ij} = -\overline{u_i' u_j'} = \nu_t \left(\dfrac{\partial u_i}{\partial x_j} + \dfrac{\partial u_j}{\partial x_i}\right) - \tfrac{2}{3} k \delta_{ij}$.
- Blending function eqs. (12)-(13): $\arg_1 = \max\left(\min\left(\dfrac{\sqrt{k}}{0.09\, \omega y};\ \dfrac{0.45\, \omega}{\Omega}\right);\ \dfrac{400\, \nu}{y^2 \omega}\right)$, $F_1 = \tanh(\arg_1^4)$, $y$ the distance to the next surface, $\Omega$ the vorticity magnitude. The first argument is the turbulent length scale $L_t = \sqrt{k}/(0.09\, \omega)$ ($= k^{3/2}/\varepsilon$) over $y$, "equal to 2.5 in the logarithmic region" and going to zero at the boundary-layer edge; the second guards against the "degenerate" solution with small free-stream $\omega$; the third keeps $F_1$ from going to zero in the viscous sublayer. $F_1$ "is equal to one over about 50% of the boundary-layer and then gradually goes to zero" (p. 5, Fig. 1).

### SST model (pp. 6-8, eqs. 14-22)
- A full Reynolds-stress model accounts for "the transport of the principal turbulent shear-stress $\tau = -\overline{u'v'}$" through $D\tau/Dt$ (eq. 14); an eddy-viscosity model does not (p. 6).
- Bradshaw's assumption, eq. (15): $\tau = \rho a_1 k$, $a_1 = 0.3$, the basis of the Johnson-King model. Two-equation models compute $\tau = \mu_t \Omega$ (eq. 16), which can be rewritten $\tau = \rho \sqrt{\text{Production}_k/\text{Dissipation}_k}\; a_1 k$ (eq. 17); in adverse-pressure-gradient flows production/dissipation "can be significantly larger than one", so eq. (17) "leads to an overprediction of $\tau$" (pp. 6-7).
- To satisfy eq. (15) within an eddy-viscosity model: $\nu_t = a_1 k/\Omega$ (eq. 18). In conventional two-equation models "the turbulent shear-stress responds instantaneously to changes in the shear-strain rate", whereas eq. (18) "guarantees that $\tau$ does not change more rapidly than $\rho a_1 k$". Eq. (18) alone gives infinite $\nu_t$ where $\Omega \to 0$, so eq. (19): $\nu_t = a_1 k / \max(a_1 \omega;\ \Omega)$ (p. 7, Fig. 2).
- Restricted to boundary layers by $F_2$ (free shear layers keep the original form "where Bradshaw's assumption ... does not necessarily hold"): eq. (20) $\nu_t = a_1 k / \max(a_1 \omega;\ \Omega F_2)$, eqs. (21)-(22) $\arg_2 = \max\left(\dfrac{2 \sqrt{k}}{0.09\, \omega y};\ \dfrac{400\, \nu}{y^2 \omega}\right)$, $F_2 = \tanh(\arg_2^2)$; $F_2$ "extends further out into the boundary-layer than $F_1$" (p. 8, Fig. 3).
- SST inner constants (set 1): $\sigma_{k1} = 0.85$, $\sigma_{\omega 1} = 0.65$, $\beta_1 = 0.0750$, $\beta^* = 0.09$, $\kappa = 0.41$; set 2 unchanged; for general flows $\Omega$ is the absolute vorticity (p. 8).
- Appendix (pp. 28-29, eqs. A-1 to A-13): the full BSL and SST models; $a_1 = 0.3$; the $\omega$ production must be eq. (A-4), not $P_\omega = (\gamma \omega/k)\, \tau_{ij}\, \partial u_i/\partial x_j$ (A-13), because with eq. (A-10) $\nu_t \omega/k$ is not 1 and "the SST model has been calibrated with respect to equation A-4".

### Boundary conditions (p. 9, eqs. 23-24)
- At a no-slip wall all turbulence quantities except $\omega$ are zero; near a wall $\omega \to 6 \nu/(\beta_1 y^2)$ as $y \to 0$ (eq. 23). Menter uses instead $\omega = 10 \cdot 6 \nu/(\beta_1 (\Delta y)^2)$ at $y = 0$ (eq. 24), $\Delta y$ the distance to the next point; "The results are not sensitive to the factor (10)".
- Inflow: turbulence quantities specified; outflow: zero gradient.

### Numerics and flat plate (pp. 9-11)
- Turbulence equations solved decoupled from the mean flow, second-order TVD convection, implicit source linearisation that "increase[s] the diagonal dominance" (p. 9).
- Flat plate: with the free-stream $\omega$ reduced by four orders of magnitude, the eddy viscosity of the original k-omega model "changes by almost 100%", while BSL "gives the same results for both cases"; SST results "were also found to be independent of $\omega_f$" (pp. 10-11, Fig. 4). Friction velocity $u_\tau = \sqrt{\tau_w/\rho}$ (p. 10).

## Contradictions with the wiki

- None with the repo docs; rcfd uses the later SST-2003 form via TMR ([[doc-reference-data-sst-wall]]).
- Version difference with [[ref-nasa-tmr]] ($a_1$, $\sigma_{\omega 1}$, the $\arg_1$ constant and the $CD_{k\omega}$ term), recorded there; not a contradiction.

## Pages this source informs

- [[k-omega-sst]], [[k-epsilon]]
- [[k-omega-wilcox]], [[sst-blending-functions]], [[bradshaw-assumption-and-sst-limiter]], [[two-equation-turbulence-models]], [[boussinesq-eddy-viscosity]], [[wall-distance]], [[flow-separation-and-reattachment]]
