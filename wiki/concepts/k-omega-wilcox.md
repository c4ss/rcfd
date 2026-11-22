---
type: concept
tags: [foundation, turbulence, rans, two-equation]
sources: [ref-nasa-tmr, ref-menter-1992-nasa-tm-103975, ref-davidson-turbulence-notes]
builds_on: [two-equation-turbulence-models]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# k-omega model (Wilcox)

## In short

Wilcox's k-omega model transports the turbulent kinetic energy $k$ and the specific dissipation rate $\omega$ (dissipation per unit $k$, a turbulence frequency), and sets $\nu_t = k/\omega$. It behaves well near walls without damping functions and handles adverse pressure gradients better than k-epsilon, but its results depend strongly on the $\omega$ value put in the free stream ([[ref-menter-1992-nasa-tm-103975]], [[ref-nasa-tmr]]).

## Explanation

**Equations (Wilcox 1988 form)** ([[ref-nasa-tmr]] `wilcox.html`; [[ref-menter-1992-nasa-tm-103975]] eqs. 1-2):

- $\dfrac{\partial (\rho k)}{\partial t} + \dfrac{\partial (\rho U_j k)}{\partial x_j} = P - \beta^* \rho \omega k + \dfrac{\partial}{\partial x_j}\left[\left(\mu + \sigma_k \dfrac{\rho k}{\omega}\right) \dfrac{\partial k}{\partial x_j}\right]$
- $\dfrac{\partial (\rho \omega)}{\partial t} + \dfrac{\partial (\rho U_j \omega)}{\partial x_j} = \dfrac{\gamma \omega}{k} P - \beta \rho \omega^2 + \dfrac{\partial}{\partial x_j}\left[\left(\mu + \sigma_w \dfrac{\rho k}{\omega}\right) \dfrac{\partial \omega}{\partial x_j}\right]$
- $\mu_t = \rho k/\omega$

$P = \tau_{ij}\, \partial U_i/\partial x_j$ is the production of $k$ by the mean strain. Constants (1988): $\sigma_k = \sigma_w = 0.5$, $\beta^* = 0.09$, $\beta = 3/40$, $\gamma = 5/9$; $\gamma$ follows from $\gamma = \beta/\beta^* - \sigma_w \kappa^2/\sqrt{\beta^*}$ so that the log layer has the right Karman constant ([[ref-nasa-tmr]] `wilcox.html`). The dissipation is $\varepsilon = \beta^* \omega k$, so $\omega$ is a rate $[\mathrm{1/s}]$ ([[lagrangian-particles]] uses this relation; [[ref-menter-1992-nasa-tm-103975]] p. 5 relates the length scale $\sqrt{k}/(0.09\, \omega) = k^{3/2}/\varepsilon$).

**Later versions.** Wilcox 2006 adds a cross-diffusion term $(\rho \sigma_d/\omega)\, (\partial k/\partial x_j)(\partial \omega/\partial x_j)$ (with $\sigma_d = 1/8$ only when $(\partial k/\partial x_j)(\partial \omega/\partial x_j) > 0$), a stress limiter $\mu_t = \rho k/\hat{\omega}$, $\hat{\omega} = \max\left[\omega,\, C_\text{lim} \sqrt{2 \bar{S}_{ij} \bar{S}_{ij}/\beta^*}\right]$, $C_\text{lim} = 7/8$, and constants $\sigma_k = 0.6$, $\sigma_w = 0.5$, $\gamma = 13/25$, $\beta_0 = 0.0708$ ([[ref-nasa-tmr]] `wilcox.html`).

**Near the wall.** $k = 0$ at a wall. $\omega$ becomes singular: $\omega \to 6 \nu/(\beta y^2)$ as $y \to 0$ ([[ref-menter-1992-nasa-tm-103975]] eq. 23; [[ref-nasa-tmr]] `wilcox.html`). Menter's practical wall value is $\omega_\text{wall} = 10 \cdot 6 \nu/(\beta_1 (\Delta y)^2)$, $\Delta y$ the first-point distance, and "The results are not sensitive to the factor (10)" ([[ref-menter-1992-nasa-tm-103975]] eq. 24); the TMR adds that using the asymptotic value itself as a wall condition is "not appropriate" according to Menter (1994) ([[ref-nasa-tmr]] `wilcox.html`). There are no damping functions and the wall conditions are "straightforward Dirichlet boundary conditions", which "leads to significant advantages in numerical stability" ([[ref-menter-1992-nasa-tm-103975]] pp. 2-3).

**What it adds over k-epsilon.** The $\varepsilon$ equation of [[k-epsilon]] needs damping functions in the viscous sublayer and a non-zero wall value of $\varepsilon$, sources of stiffness; and k-epsilon is insensitive to adverse pressure gradients. k-omega "performs significantly better under adverse pressure-gradient conditions" and is simple in the sublayer ([[ref-menter-1992-nasa-tm-103975]] p. 2; [[ref-davidson-turbulence-notes]] sec. 16, p. 180).

**Its own problem: free-stream sensitivity.** The results "depend strongly on the freestream values, $\omega_f$, that are specified outside the shear-layer"; the eddy viscosity "can be changed by more than 100% just by using different values for $\omega_f$" ([[ref-menter-1992-nasa-tm-103975]] p. 3, Fig. 4). The TMR still notes "some sensitivity to freestream boundary conditions on $\omega$" for all Wilcox versions ([[ref-nasa-tmr]] `wilcox.html`). It also gives too low spreading rates for free shear layers (Menter p. 3). That is the problem the SST blending fixes ([[sst-blending-functions]]).

**Grid sensitivity.** On the TMR flat plate, $\omega$-based models are more sensitive to the first-cell $y^+$ than SA: an average minimum $y^+$ of 0.8 gave a drag "between 2-3% low" compared with $y^+$ 0.1, attributed to the $\omega$ equation and/or its wall condition ([[ref-nasa-tmr]] `flatplate_val_ypluseffect.html`).

## Builds on

- [[two-equation-turbulence-models]]: the generic $k$ plus length-scale equation structure; here the second variable is $\omega$.

## Leads to

- [[sst-blending-functions]] (k-omega kept near the wall), [[k-omega-sst]]

## How rcfd uses it

- rcfd has no stand-alone Wilcox model; its k-omega equations are the inner part of SST and BSL (ADR 0015) ([[adr-thread-early-turbulence]], [[k-omega-sst]]).
- rcfd's SST wall value is the Menter form $\omega_\text{wall} = 10 \cdot 6 \nu/(\beta_1 d_1^2)$; ADR 0063 found a "slowly (first order) decaying sensitivity to the published wall-face $\omega$ factor 10, not a defect" ([[doc-reference-data-sst-wall]], [[adr-thread-early-turbulence]]).
- "generalised k-omega/EARSM" is listed as not done in the F-043 gap ([[roadmap-turbulence]]).
