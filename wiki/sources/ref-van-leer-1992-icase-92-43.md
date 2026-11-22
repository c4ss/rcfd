---
type: source
tags: [reference, foundation, discretisation, upwind, copy]
sources: []
origin: https://ntrs.nasa.gov/citations/19930001620
licence: "Work of the US Gov. Public Use Permitted." (NTRS record, checked 2026-11-22)
read: 2026-11-22
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# van Leer (1992), Progress in multi-dimensional upwind differencing (ICASE 92-43)

**Origin:** B. van Leer, "Progress in multi-dimensional upwind differencing", ICASE Report No. 92-43, NASA CR-189708, September 1992, NTRS 19930001620.
**Kind:** review lecture (NASA contractor report).
**Licence:** US Government work, public use permitted; full copy in [raw/van-leer-1992-icase-92-43.pdf](../raw/van-leer-1992-icase-92-43.pdf).
**Read:** 2026-11-22 by job p6-pv-discretisation: abstract and sections 1 to 3 (one-dimensional upwinding, its two interpretations, how good it is); the later sections on genuinely multidimensional schemes were skimmed only. Locators are section and equation numbers.

## Summary

Reviews a decade of work on upwind schemes for the Euler equations in more than one dimension. Starts from first-order upwind differencing of the scalar convection equation and its two readings (nodal point and finite volume), then shows why the usual finite-volume extension with Riemann solvers normal to cell faces smears waves that are not grid-aligned, and surveys multi-directional and fluctuation-splitting remedies.

## Key points (own summary)

- **First-order upwind (2, Eqs. 1-3):** for `u_t + c u_x = 0`, the simplest upwind scheme uses the backward difference `(u_i - u_{i-1})/dx` when `c > 0` and the forward difference when `c < 0`; it is first-order accurate. "Upwind differencing is a way of differencing convection terms."
- **Two readings (2, Fig. 1):** the same update can be read as advancing a nodal-point value or a cell average; the first leads to fluctuation splitting (Roe), the second to Godunov's projection/evolution (reconstruction/evolution) finite-volume schemes (van Leer).
- **Finite-volume update (2.2, Eq. 13):** cell averages change by the difference of the numerical interface fluxes, `U_i^{n+1} = U_i^n - (dt/dx)(F_{i+1/2} - F_{i-1/2})`; in Godunov's first-order scheme the interface flux comes from the solution of the Riemann problem between the two neighbouring states; approximate Riemann solvers (Roe) are used in practice, giving the interface flux `F = (1/2)(F_L + F_R) - (1/2)|A| (U_R - U_L)` (Eq. 19), a central average minus an upwind dissipation term.
- **Robustness (1, 3):** upwind schemes are "computationally intensive but unsurpassed in their combination of accuracy and robustness"; in 1D they capture sonic points and shocks crisply and converge without trouble.
- **Multidimensional weakness (abstract, 1):** the usual extension assumes waves travel normal to the cell faces, which "leads to smearing of shock and shear waves when these are not grid-aligned".

### Added by job p6-compressible-acoustics (2026-11-22, section 2 re-read)

- **1D Euler equations (2.1, Eq. 4):** `U_t + F(U)_x = 0` with conserved state `U = (rho, rho u, rho E)^T` and flux `F(U) = (rho u, rho u^2 + p, rho u H)^T`; any local imbalance of fluxes (a "fluctuation", Roe) changes the local solution in time.
- **Flux Jacobian and waves (2.1, Eqs. 5-12):** `dF = A(U) dU`; Roe's mean matrix `A_tilde` satisfies `Delta F = A_tilde Delta U` exactly for any two states (Eq. 6), obtained for a calorically perfect gas by averaging the parameter vector `w = sqrt(rho) (1, u, H)^T`. Splitting `A_tilde = A_tilde^+ + A_tilde^-` (Eq. 7) separates forward- and backward-moving disturbances: `Delta U = sum_k alpha_k R_k` and `Delta F = sum_k lambda_k alpha_k R_k` (Eqs. 10-11), `lambda_k` the eigenvalues (wave speeds), `R_k` eigenvectors, `alpha_k` wave strengths; waves with `lambda_k > 0` update the right node, those with `lambda_k < 0` the left (Eq. 12). The name "flux-difference splitting" is common; "fluctuation splitting" includes source terms.
- **Godunov and Roe flux (2.2, Eqs. 13-21):** Godunov's interface flux is the solution at `t > 0` of Riemann's initial-value problem with the two states `U_L` and `U_R` (Eqs. 14-15); exact solutions are often unnecessary, hence "approximate Riemann solvers"; Roe's solution is the exact solution of the locally linearised `U_t + A_tilde U_x = 0` (Eq. 16), giving `F = F_L + A_tilde^- Delta U = F_R - A_tilde^+ Delta U = (1/2)(F_L + F_R) - (1/2)|A_tilde| Delta U` with `|A_tilde| = A_tilde^+ - A_tilde^-` (Eqs. 17-20), expanded as `(1/2)(F_L + F_R) - (1/2) sum_k |lambda_k| alpha_k R_k` (Eq. 21). In the finite-volume form `|A|` may come from any average without losing conservation; the flux formula (19), due to van Leer, preceded Roe's fluctuation approach "by a decade".

## Contradictions with the wiki

None found.

## Pages this source informs

- [[upwind-and-central-differencing]], [[finite-volume-method]], [[tvd-schemes-and-flux-limiters]]
- [[euler-equations]], [[characteristics-and-hyperbolic-systems]], [[riemann-problem-and-godunov-method]], [[approximate-riemann-solvers]] (added by p6-compressible-acoustics)
