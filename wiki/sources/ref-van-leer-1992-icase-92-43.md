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

## Contradictions with the wiki

None found.

## Pages this source informs

- [[upwind-and-central-differencing]], [[finite-volume-method]], [[tvd-schemes-and-flux-limiters]]
