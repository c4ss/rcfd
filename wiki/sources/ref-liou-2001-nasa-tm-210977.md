---
type: source
tags: [reference, foundation, compressible, riemann-solver, ausm, flux-splitting, public-domain]
sources: []
origin: https://ntrs.nasa.gov/citations/20020009014
licence: NTRS determination GOV_PUBLIC_USE_PERMITTED (record API read 2026-11-22); no AIAA copyright notice found in the report text
read: 2026-11-22
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Liou (2001), Ten Years in the Making: AUSM-Family, NASA/TM-2001-210977

**Origin:** M.-S. Liou, "Ten Years in the Making - AUSM-family", NASA/TM-2001-210977 (also AIAA Paper 2001-2521, 15th AIAA CFD Conference, Anaheim, June 2001), NASA Glenn Research Center, November 2001. NTRS 20020009014. PDF: https://ntrs.nasa.gov/api/citations/20020009014/downloads/20020009014.pdf
**Kind:** NASA technical memorandum / conference paper (review of the author's scheme family).
**Licence:** US Government work, public use permitted; full copy in [raw/liou-2001-nasa-tm-210977.pdf](../raw/liou-2001-nasa-tm-210977.pdf).
**Read:** 2026-11-22 by job p6-compressible-acoustics: introduction, "Equations of Conservation Laws", "Numerical Flux: AUSM-Family" (How it began; How it is shaping up), the low-Mach-number section in outline. Equations were partly garbled in the text layer. Locators are section headings and equation numbers.

## Summary

Reviews the Advection Upstream Splitting Method (AUSM) family of numerical inviscid fluxes for conservation laws: the split of the flux into a convective part carried by a mass flux and a pressure part, the interface Mach number and pressure splittings of AUSM+ and AUSMDV, the common speed of sound, and extensions to low Mach numbers and multiphase flow.

## Key points (own summary)

- **Introduction:** progress in the two decades before 2001 was largely in "accurate and robust schemes for capturing shock and contact discontinuities"; the numerical inviscid flux "plays a central role" in the robustness and accuracy of a calculation. AUSM dates from 1990.
- **Two classes of upwind schemes ("How It Began"):** the 1980s saw "flux-vector and flux-difference schemes"; the van Leer scheme is a flux-vector splitting and "perhaps the most robust one among all for the Euler equations"; the motivation for AUSM was to keep that robustness while resolving contact discontinuities and boundary layers better.
- **Flux split (Eqs. 3-4):** the inviscid flux is written `F = F^(c) + P = m_dot psi + P`, with mass flux `m_dot = rho u`, `psi = (1, u, v, w, H)^T` and pressure flux `P = (0, p n_x, p n_y, p n_z, 0)^T`; carrying total enthalpy `H` in `psi` guarantees its conservation.
- **Interface flux (Eqs. 9-12):** `f_1/2 = m_dot_1/2 psi_L/R + p_1/2`, with `psi_L` taken when the interface mass flux is positive and `psi_R` when negative; equivalently a central average of `psi` plus a dissipation term proportional to `|m_dot_1/2|`. This is a scalar (not matrix) dissipation; the paper places central differencing with artificial damping, the van Leer flux-vector scheme and the HLLE scheme in the same scalar-dissipation category, while flux-difference splittings have matrix dissipation.
- **Split Mach numbers and common speed of sound (Eqs. 13-23):** polynomial split functions `M^+-(M)`, `P^+-(M)` (first degree `(M +- |M|)/2` for `|M| >= 1`, higher-degree polynomials otherwise) give the interface Mach number `M_1/2 = M^+(M_L) + M^-(M_R)`; a common interface speed of sound `a_1/2` is used for both sides, which allows exact capture of a stationary normal shock with a special choice.
- **Low speed:** compressible upwind codes applied to low-speed flow show "extremely slow or stalled convergence" and can give "globally incorrect" solutions; local preconditioning (van Leer, Turkel, Merkle) addresses the first by equalising the eigenvalues; AUSM+-up style scalings of the dissipation address accuracy.

## Contradictions with the wiki

None found.

## Pages this source informs

[[approximate-riemann-solvers]], [[riemann-problem-and-godunov-method]], [[euler-equations]]
