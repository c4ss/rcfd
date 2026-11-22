---
type: source
tags: [reference, foundation, turbulence, open-licence]
sources: []
origin: https://tmbwg.github.io/turbmodels/
licence: CC0 1.0 Universal (LICENSE file of the source repository github.com/TMBWG/turbmodels; the HTML pages themselves state no licence)
read: 2026-11-22
raw: raw/tmr-turbmodels/
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# NASA Turbulence Modeling Resource (TMR), TMBWG mirror

**Origin:** https://tmbwg.github.io/turbmodels/ (GitHub Pages mirror of the NASA Langley Turbulence Modeling Resource, maintained by the AIAA Turbulence Model Benchmarking Working Group; page curators Ethan Vogel and Clark Pederson). The original host `turbmodels.larc.nasa.gov` failed its TLS check on 2026-11-22 ([[foundations-sources]]).
**Kind:** web reference (model equations, verification and validation cases).
**Licence:** the repository github.com/TMBWG/turbmodels carries a `LICENSE` file "CC0 1.0 Universal" (fetched 2026-11-22). The pages read below are copied to [raw/tmr-turbmodels/](../raw/tmr-turbmodels/) with that licence file. The equation images are not copied; the page HTML carries each equation as LaTeX in the image `alt` text, which is what was read.
**Read:** 2026-11-22, by job p6-turbulence. Citation form: NASA Langley Research Center, *Turbulence Modeling Resource*, page name, "Last Updated" date as printed.

## Summary

The TMR is the reference rcfd already uses for its SST and Spalart-Allmaras constants and for the flat-plate and backward-step data ([[doc-reference-data-sst-wall]], [[doc-validation-data]]). For the foundation pages it gives the exact model equations (SST, BSL, Wilcox k-omega, SSG/LRR Reynolds stress model), the definition of the Reynolds stress and the Boussinesq relation in the compressible RANS equations, the wall-unit definitions, the flat-plate correlations, the minimum-wall-distance rule and the backward-step reattachment value.

## Key points by page

### `sst.html` — "The Menter Shear Stress Transport Turbulence Model" (Last Updated 10/04/2024)
- All forms on the page are "linear eddy viscosity models" using the Boussinesq relation $\tau_{ij} = \mu_t \left(2 S_{ij} - \tfrac{2}{3} \dfrac{\partial u_k}{\partial x_k} \delta_{ij}\right) - \tfrac{2}{3} \rho k \delta_{ij}$.
- Standard SST (reference Menter 1994, AIAA J. 32(8)): conservation-form $k$ and $\omega$ equations; $P = \tau_{ij}\, \partial u_i/\partial x_j$; $\mu_t = \rho a_1 k / \max(a_1 \omega, \Omega F_2)$; constants blended by $\phi = F_1 \phi_1 + (1 - F_1)\, \phi_2$; $F_1 = \tanh(\arg_1^4)$, $\arg_1 = \min\left[\max\left(\dfrac{\sqrt{k}}{\beta^* \omega d}, \dfrac{500\, \nu}{d^2 \omega}\right), \dfrac{4 \rho \sigma_{\omega 2} k}{CD_{k\omega} d^2}\right]$, $CD_{k\omega} = \max\left(2 \rho \sigma_{\omega 2} \dfrac{1}{\omega} \dfrac{\partial k}{\partial x_j} \dfrac{\partial \omega}{\partial x_j},\ 10^{-20}\right)$; $F_2 = \tanh(\arg_2^2)$, $\arg_2 = \max\left(\dfrac{2 \sqrt{k}}{\beta^* \omega d}, \dfrac{500\, \nu}{d^2 \omega}\right)$; $d$ is "the distance from the field point to the nearest wall"; $\Omega = \sqrt{2 W_{ij} W_{ij}}$ is the vorticity magnitude.
- The $\omega$ equation carries the cross-diffusion term $2(1 - F_1) \dfrac{\rho \sigma_{\omega 2}}{\omega} \dfrac{\partial k}{\partial x_j} \dfrac{\partial \omega}{\partial x_j}$.
- Constants: $\sigma_{k1} = 0.85$, $\sigma_{\omega 1} = 0.5$, $\beta_1 = 0.075$, $\sigma_{k2} = 1.0$, $\sigma_{\omega 2} = 0.856$, $\beta_2 = 0.0828$, $\beta^* = 0.09$, $\kappa = 0.41$, $a_1 = 0.31$; $\gamma_i = \beta_i/\beta^* - \sigma_{\omega i} \kappa^2/\sqrt{\beta^*}$.
- Production limiter recommended (Menter AIAA 93-2906): $P$ in the $k$ equation replaced by $\min(P, 20 \beta^* \rho \omega k)$.
- Wall conditions: $\omega_\text{wall} = 10 \cdot 6 \nu / (\beta_1 (\Delta d_1)^2)$, $k_\text{wall} = 0$. Far-field values should give a free-stream eddy viscosity "between 10-5 and 10-2 times freestream laminar viscosity"; the turbulence variables "decay (sometimes dramatically)" in the far field.
- SST-2003 (Menter, Kuntz and Langtry 2003): eddy viscosity uses the strain invariant $S = \sqrt{2 S_{ij} S_{ij}}$ instead of the vorticity, $\mu_t = \rho a_1 k / \max(a_1 \omega, S F_2)$; the limiter acts in both equations with constant 10, $\min(P, 10 \beta^* \rho \omega k)$; $CD_{k\omega}$ floor $10^{-10}$; $\gamma_1 = 5/9$, $\gamma_2 = 0.44$.
- Common approximations: the $\tfrac{2}{3} \rho k \delta_{ij}$ term "is sometimes ignored" in the momentum and energy equations, and $P = \mu_t S^2$ is "exact for incompressible flows" (naming suffix "m").

### `wilcox.html` — "The Wilcox k-omega Turbulence Model" (most recent update row 10/04/2024)
- Wilcox 2006: $k$ equation: $P - \beta^* \rho \omega k + \dfrac{\partial}{\partial x_j}\left[\left(\mu + \sigma_k \dfrac{\rho k}{\omega}\right) \dfrac{\partial k}{\partial x_j}\right]$; $\omega$ equation: $\dfrac{\gamma \omega}{k} P - \beta \rho \omega^2 + \dfrac{\partial}{\partial x_j}\left[\left(\mu + \sigma_\omega \dfrac{\rho k}{\omega}\right) \dfrac{\partial \omega}{\partial x_j}\right] + \dfrac{\rho \sigma_d}{\omega} \dfrac{\partial k}{\partial x_j} \dfrac{\partial \omega}{\partial x_j}$; $\mu_t = \rho k / \hat{\omega}$ with the stress limiter $\hat{\omega} = \max\left[\omega, C_\text{lim} \sqrt{2 \bar{S}_{ij} \bar{S}_{ij} / \beta^*}\right]$, $C_\text{lim} = 7/8$; $\sigma_k = 0.6$, $\sigma_\omega = 0.5$, $\beta^* = 0.09$, $\gamma = 13/25$, $\beta_0 = 0.0708$; $\sigma_d = 0$ or $1/8$ depending on the sign of $\dfrac{\partial k}{\partial x_j} \dfrac{\partial \omega}{\partial x_j}$.
- Wilcox 1988: $\mu_t = \rho k/\omega$, $\sigma_k = \sigma_\omega = 0.5$, $\beta^* = 0.09$, $\beta = 3/40$, $\gamma = 5/9$, no cross-diffusion.
- $\gamma$ is chosen to give the Karman constant via $\gamma = \beta_0/\beta^* - \sigma_\omega \kappa^2/\sqrt{\beta^*}$ ($\kappa = 0.40$ in the 2006 note).
- "the Wilcox k-omega two-equation models exhibit some sensitivity to freestream boundary conditions on $\omega$".
- Walls: $k_\text{wall} = 0$; smooth-wall asymptote $\omega \to 6 \nu_\text{wall}/(\beta_0 d^2)$ as $y \to 0$; "according to Menter (AIAA J 32(8):1598-1605, 1994), it is not appropriate to use this asymptotic value for the BC at a wall", so many codes use Menter's approximate wall value.

### `implementrans.html` — "Implementing Turbulence Models into the Compressible RANS Equations" (Last Updated 10/10/2024)
- Favre-averaged mass, momentum and energy equations; the overbar is a time average "with the averaging time scale assumed to be long compared to turbulent fluctuations, and short compared to unsteadiness in the mean flow"; the hat is the density-weighted average $\hat{f} = \overline{\rho f}/\bar{\rho}$, with $f = \bar{f} + f' = \hat{f} + f''$.
- The Reynolds stress $\tau_{ij} = -\overline{\rho u_i'' u_j''}$ "is defined in the literature both as shown here, as well as with the opposite sign, and sometimes without the density".
- $k$ is "the kinetic energy of the fluctuating field".
- "Most turbulence modeling focuses on the Reynolds stress terms"; they are "either solved directly (as in full second-moment Reynolds stress models) or defined via a constitutive relation for simpler models", e.g. the Boussinesq approximation $\tau_{ij} = 2 \mu_t \left(S_{ij} - \tfrac{1}{3} \dfrac{\partial u_k}{\partial x_k} \delta_{ij}\right) - \tfrac{2}{3} \rho k \delta_{ij}$, with $\mu_t$ "the eddy viscosity obtained by the turbulence model".
- Turbulent heat flux by a Reynolds analogy with a turbulent Prandtl number "often taken to be constant (e.g., around 0.9 for air)".

### `flatplate.html` — 2DZP verification case (Last Updated 03/24/2021)
- Turbulent flat plate at $M = 0.2$, $Re = 5$ million per unit length, plate from $x = 0$ to $2$; maximum boundary-layer thickness "about $0.03 L$". Verification "is not to establish the 'goodness' of a model compared to experiment" but that it is "implemented correctly".

### `flatplate_val.html` — 2DZP validation case (Last Updated 04/12/2022)
- Definitions: $Re_\theta = \rho_\infty U_\infty \theta/\mu_\infty$; momentum thickness $\theta = \int_0^\infty (\rho/\rho_\infty)(u/U_\infty)(1 - u/U_\infty)\, dy$; $c_f = \tau_w / (\rho_\infty U_\infty^2/2)$; $\tau_w = \mu_w (du/dy)_w$; $u^+ = u/\sqrt{\tau_w/\rho_w}$; $y^+ = y \sqrt{\tau_w/\rho_w}/\nu_w$.
- Karman-Schoenherr skin friction: $c_f = \left(17.08 (\log_{10} Re_\theta)^2 + 25.11 \log_{10} Re_\theta + 6.012\right)^{-1}$.
- Coles law of the wall with wake: $u^+ = (1/\kappa) \ln(y^+) + C + (2 \Pi/\kappa) \sin^2(\pi y/(2 \delta))$, "along with a van Driest type damping near the wall".
- "these particular theoretical correlations are not necessarily perfect"; White's $C_f$ theories plotted with 5 % error bars "although it is not clear what the error levels actually are".

### `flatplate_val_ypluseffect.html` — effect of minimum y+
- "Conventional wisdom states that grids for turbulent flow RANS (when integrating to the wall, i.e., without wall functions) should be constructed so that the average minimum $y^+$ is less than 1."
- $\omega$-based models (SST family, Wilcox2006-klim, SSG/LRR-RSM-w2012) showed "higher sensitivity to average minimum $y^+$" than SA or k-kL; with them, average minimum $y^+$ 0.8 gave a drag coefficient "between 2-3% low" compared with $y^+$ 0.1. The page attributes this to the $\omega$ equation and/or the wall condition $\omega_\text{wall} = 10 \cdot 6 \nu/(\beta_1 (\Delta d_1)^2)$.

### `backstep_val.html` — 2DBFS validation case (Driver and Seegmiller)
- "a turbulent boundary layer encounters a sudden back step, causing flow separation. The flow then reattaches and recovers downstream of the step."
- $Re_\theta$ before the step 5000, $Re_H$ about 36,000, boundary layer about $1.5 H$ before the step, $M = 0.128$.
- "One of the key measures of success for this flowfield is the prediction of reattachment point"; measured $x/H_\text{reattach} = 6.26 \pm 0.10$ (laser oil-flow interferometry, zero skin-friction location).

### `rsm-ssglrr.html` — SSG/LRR-omega full Reynolds stress model
- Second-moment models "compute each of the 6 Reynolds stresses directly", each with its own transport equation, plus "a seventh transport equation for the lengthscale-determining variable"; simpler models use "a constitutive relation ... (such as Boussinesq's hypothesis)".
- Equation for $\rho R_{ij}$ ($= -\tau_{ij}$): production $P_{ij} = -R_{ik}\, \partial u_j/\partial x_k - R_{jk}\, \partial u_i/\partial x_k$ ("exact"), pressure-strain $\Pi_{ij}$, dissipation modelled isotropic $\varepsilon_{ij} = \tfrac{2}{3} \varepsilon \delta_{ij}$ with $\varepsilon = C_\mu k \omega$, generalized gradient diffusion (Daly and Harlow), mass-flux term neglected. Pressure-strain coefficients blended from LRR near walls to SSG away from walls by an SST-like $F_1$ using $d$, the distance to the nearest wall; $C_\mu = 0.09$.

### `spalart.html` — "The Spalart-Allmaras Turbulence Model" (minimum-distance and DES notes only)
- "computing minimum distance (d) by searching along grid lines or by finding the nearest wall gridpoint (or cell center) are incorrect"; they "will yield grid-dependent differences", particularly where grid lines are not normal to the body or the nearest body is in another grid zone; at a sharp convex corner or edge the correct distance "is the distance to that corner or edge, which is not a wall normal".
- SA is the basis of Detached-Eddy Simulation (1997) and DDES (2006); with them the $f_{t2}$ term "may cause an undesirable delay in transition to turbulence in the RANS region".

## Contradictions with the wiki

- None with the repo docs. rcfd's SST constants (`a1 0.31`, `sigma_w1 0.5`, `gamma1 5/9`, limiter 10, `CD_kw` floor `1e-10`) match the SST-2003 section of `sst.html` ([[doc-reference-data-sst-wall]]).
- Version difference with [[ref-menter-1992-nasa-tm-103975]] (not a contradiction; different model versions): the 1992 TM prints $a_1 = 0.3$, $\sigma_{\omega 1} = 0.65$ for the SST inner set and $400 \nu/(y^2 \omega)$ in $\arg_1$, and has no $CD_{k\omega}$ term in $\arg_1$; the TMR standard SST (Menter 1994) prints $a_1 = 0.31$, $\sigma_{\omega 1} = 0.5$, $500 \nu/(d^2 \omega)$ and the $CD_{k\omega}$ term.

## Pages this source informs

- [[k-omega-sst]], [[k-epsilon]], [[spalart-allmaras]], [[flat-plate]]
- [[reynolds-averaging]], [[reynolds-stress]], [[boussinesq-eddy-viscosity]], [[two-equation-turbulence-models]], [[k-omega-wilcox]], [[sst-blending-functions]], [[bradshaw-assumption-and-sst-limiter]], [[wall-distance]], [[reynolds-stress-models]], [[les-and-des]], [[skin-friction-coefficient]], [[friction-velocity-and-y-plus]], [[law-of-the-wall]], [[boundary-layer]], [[flow-separation-and-reattachment]]
