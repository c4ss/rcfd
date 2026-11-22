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
- All forms on the page are "linear eddy viscosity models" using the Boussinesq relation `tau_ij = mu_t (2 S_ij - (2/3) (du_k/dx_k) delta_ij) - (2/3) rho k delta_ij`.
- Standard SST (reference Menter 1994, AIAA J. 32(8)): conservation-form k and omega equations; `P = tau_ij du_i/dx_j`; `mu_t = rho a1 k / max(a1 omega, Omega F2)`; constants blended by `phi = F1 phi1 + (1 - F1) phi2`; `F1 = tanh(arg1^4)`, `arg1 = min[max(sqrt(k)/(beta* omega d), 500 nu/(d^2 omega)), 4 rho sigma_w2 k/(CD_kw d^2)]`, `CD_kw = max(2 rho sigma_w2 (1/omega) dk/dx_j domega/dx_j, 1e-20)`; `F2 = tanh(arg2^2)`, `arg2 = max(2 sqrt(k)/(beta* omega d), 500 nu/(d^2 omega))`; `d` is "the distance from the field point to the nearest wall"; `Omega = sqrt(2 W_ij W_ij)` is the vorticity magnitude.
- The omega equation carries the cross-diffusion term `2(1 - F1) (rho sigma_w2/omega) dk/dx_j domega/dx_j`.
- Constants: `sigma_k1 = 0.85`, `sigma_w1 = 0.5`, `beta1 = 0.075`, `sigma_k2 = 1.0`, `sigma_w2 = 0.856`, `beta2 = 0.0828`, `beta* = 0.09`, `kappa = 0.41`, `a1 = 0.31`; `gamma_i = beta_i/beta* - sigma_wi kappa^2/sqrt(beta*)`.
- Production limiter recommended (Menter AIAA 93-2906): `P` in the k equation replaced by `min(P, 20 beta* rho omega k)`.
- Wall conditions: `omega_wall = 10 * 6 nu / (beta1 (Delta d1)^2)`, `k_wall = 0`. Far-field values should give a free-stream eddy viscosity "between 10-5 and 10-2 times freestream laminar viscosity"; the turbulence variables "decay (sometimes dramatically)" in the far field.
- SST-2003 (Menter, Kuntz and Langtry 2003): eddy viscosity uses the strain invariant `S = sqrt(2 S_ij S_ij)` instead of the vorticity, `mu_t = rho a1 k / max(a1 omega, S F2)`; the limiter acts in both equations with constant 10, `min(P, 10 beta* rho omega k)`; `CD_kw` floor `1e-10`; `gamma1 = 5/9`, `gamma2 = 0.44`.
- Common approximations: the `(2/3) rho k delta_ij` term "is sometimes ignored" in the momentum and energy equations, and `P = mu_t S^2` is "exact for incompressible flows" (naming suffix "m").

### `wilcox.html` — "The Wilcox k-omega Turbulence Model" (most recent update row 10/04/2024)
- Wilcox 2006: `dk`: `P - beta* rho omega k + d/dx_j[(mu + sigma_k rho k/omega) dk/dx_j]`; `domega`: `(gamma omega/k) P - beta rho omega^2 + d/dx_j[(mu + sigma_w rho k/omega) domega/dx_j] + (rho sigma_d/omega) dk/dx_j domega/dx_j`; `mu_t = rho k / omega_hat` with the stress limiter `omega_hat = max[omega, C_lim sqrt(2 Sbar_ij Sbar_ij / beta*)]`, `C_lim = 7/8`; `sigma_k = 0.6`, `sigma_w = 0.5`, `beta* = 0.09`, `gamma = 13/25`, `beta0 = 0.0708`; `sigma_d = 0` or `1/8` depending on the sign of `dk/dx_j domega/dx_j`.
- Wilcox 1988: `mu_t = rho k/omega`, `sigma_k = sigma_w = 0.5`, `beta* = 0.09`, `beta = 3/40`, `gamma = 5/9`, no cross-diffusion.
- `gamma` is chosen to give the Karman constant via `gamma = beta0/beta* - sigma_w kappa^2/sqrt(beta*)` (`kappa = 0.40` in the 2006 note).
- "the Wilcox k-omega two-equation models exhibit some sensitivity to freestream boundary conditions on omega".
- Walls: `k_wall = 0`; smooth-wall asymptote `omega -> 6 nu_wall/(beta0 d^2)` as `y -> 0`; "according to Menter (AIAA J 32(8):1598-1605, 1994), it is not appropriate to use this asymptotic value for the BC at a wall", so many codes use Menter's approximate wall value.

### `implementrans.html` — "Implementing Turbulence Models into the Compressible RANS Equations" (Last Updated 10/10/2024)
- Favre-averaged mass, momentum and energy equations; the overbar is a time average "with the averaging time scale assumed to be long compared to turbulent fluctuations, and short compared to unsteadiness in the mean flow"; the hat is the density-weighted average `f_hat = mean(rho f)/mean(rho)`, with `f = f_bar + f' = f_hat + f''`.
- The Reynolds stress `tau_ij = -mean(rho u_i'' u_j'')` "is defined in the literature both as shown here, as well as with the opposite sign, and sometimes without the density".
- `k` is "the kinetic energy of the fluctuating field".
- "Most turbulence modeling focuses on the Reynolds stress terms"; they are "either solved directly (as in full second-moment Reynolds stress models) or defined via a constitutive relation for simpler models", e.g. the Boussinesq approximation `tau_ij = 2 mu_t (S_ij - (1/3) du_k/dx_k delta_ij) - (2/3) rho k delta_ij`, with `mu_t` "the eddy viscosity obtained by the turbulence model".
- Turbulent heat flux by a Reynolds analogy with a turbulent Prandtl number "often taken to be constant (e.g., around 0.9 for air)".

### `flatplate.html` — 2DZP verification case (Last Updated 03/24/2021)
- Turbulent flat plate at M = 0.2, Re = 5 million per unit length, plate from x = 0 to 2; maximum boundary-layer thickness "about 0.03 L". Verification "is not to establish the 'goodness' of a model compared to experiment" but that it is "implemented correctly".

### `flatplate_val.html` — 2DZP validation case (Last Updated 04/12/2022)
- Definitions: `Re_theta = rho_inf U_inf theta/mu_inf`; momentum thickness `theta = int_0^inf (rho/rho_inf)(u/U_inf)(1 - u/U_inf) dy`; `c_f = tau_w / (rho_inf U_inf^2/2)`; `tau_w = mu_w (du/dy)_w`; `u+ = u/sqrt(tau_w/rho_w)`; `y+ = y sqrt(tau_w/rho_w)/nu_w`.
- Karman-Schoenherr skin friction: `c_f = (17.08 (log10 Re_theta)^2 + 25.11 log10 Re_theta + 6.012)^-1`.
- Coles law of the wall with wake: `u+ = (1/kappa) ln(y+) + C + (2 Pi/kappa) sin^2(pi y/(2 delta))`, "along with a van Driest type damping near the wall".
- "these particular theoretical correlations are not necessarily perfect"; White's Cf theories plotted with 5 % error bars "although it is not clear what the error levels actually are".

### `flatplate_val_ypluseffect.html` — effect of minimum y+
- "Conventional wisdom states that grids for turbulent flow RANS (when integrating to the wall, i.e., without wall functions) should be constructed so that the average minimum y+ is less than 1."
- Omega-based models (SST family, Wilcox2006-klim, SSG/LRR-RSM-w2012) showed "higher sensitivity to average minimum y+" than SA or k-kL; with them, average minimum y+ 0.8 gave a drag coefficient "between 2-3% low" compared with y+ 0.1. The page attributes this to the omega equation and/or the wall condition `omega_wall = 10 * 6 nu/(beta1 (Delta d1)^2)`.

### `backstep_val.html` — 2DBFS validation case (Driver and Seegmiller)
- "a turbulent boundary layer encounters a sudden back step, causing flow separation. The flow then reattaches and recovers downstream of the step."
- Re_theta before the step 5000, Re_H about 36,000, boundary layer about 1.5 H before the step, M = 0.128.
- "One of the key measures of success for this flowfield is the prediction of reattachment point"; measured `x/H_reattach = 6.26 +- 0.10` (laser oil-flow interferometry, zero skin-friction location).

### `rsm-ssglrr.html` — SSG/LRR-omega full Reynolds stress model
- Second-moment models "compute each of the 6 Reynolds stresses directly", each with its own transport equation, plus "a seventh transport equation for the lengthscale-determining variable"; simpler models use "a constitutive relation ... (such as Boussinesq's hypothesis)".
- Equation for `rho R_ij` (`= -tau_ij`): production `P_ij = -R_ik du_j/dx_k - R_jk du_i/dx_k` ("exact"), pressure-strain `Pi_ij`, dissipation modelled isotropic `eps_ij = (2/3) eps delta_ij` with `eps = C_mu k omega`, generalized gradient diffusion (Daly and Harlow), mass-flux term neglected. Pressure-strain coefficients blended from LRR near walls to SSG away from walls by an SST-like `F1` using `d`, the distance to the nearest wall; `C_mu = 0.09`.

### `spalart.html` — "The Spalart-Allmaras Turbulence Model" (minimum-distance and DES notes only)
- "computing minimum distance (d) by searching along grid lines or by finding the nearest wall gridpoint (or cell center) are incorrect"; they "will yield grid-dependent differences", particularly where grid lines are not normal to the body or the nearest body is in another grid zone; at a sharp convex corner or edge the correct distance "is the distance to that corner or edge, which is not a wall normal".
- SA is the basis of Detached-Eddy Simulation (1997) and DDES (2006); with them the `ft2` term "may cause an undesirable delay in transition to turbulence in the RANS region".

## Contradictions with the wiki

- None with the repo docs. rcfd's SST constants (`a1 0.31`, `sigma_w1 0.5`, `gamma1 5/9`, limiter 10, `CD_kw` floor `1e-10`) match the SST-2003 section of `sst.html` ([[doc-reference-data-sst-wall]]).
- Version difference with [[ref-menter-1992-nasa-tm-103975]] (not a contradiction; different model versions): the 1992 TM prints `a1 = 0.3`, `sigma_w1 = 0.65` for the SST inner set and `400 nu/(y^2 omega)` in `arg1`, and has no `CD_kw` term in `arg1`; the TMR standard SST (Menter 1994) prints `a1 = 0.31`, `sigma_w1 = 0.5`, `500 nu/(d^2 omega)` and the `CD_kw` term.

## Pages this source informs

- [[k-omega-sst]], [[k-epsilon]], [[spalart-allmaras]], [[flat-plate]]
- [[reynolds-averaging]], [[reynolds-stress]], [[boussinesq-eddy-viscosity]], [[two-equation-turbulence-models]], [[k-omega-wilcox]], [[sst-blending-functions]], [[bradshaw-assumption-and-sst-limiter]], [[wall-distance]], [[reynolds-stress-models]], [[les-and-des]], [[skin-friction-coefficient]], [[friction-velocity-and-y-plus]], [[law-of-the-wall]], [[boundary-layer]], [[flow-separation-and-reattachment]]
