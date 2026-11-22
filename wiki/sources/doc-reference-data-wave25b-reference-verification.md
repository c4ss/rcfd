---
type: source
tags: [reference-data, wave-section]
sources: []
origin: docs/reference-data.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Reference data: Wave 25B reference verification (ADR 0044 D5)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Wave 25B reference verification (ADR 0044 D5)" (lines 968-1009) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Wave 25B re-check of earlier reference values (ADR 0044 D5) and the discrepancies it found.

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


This verification changed no registered limit, and no formula disagreement required a code change. It found these discrepancies:

- Scott author initials
- Lee year and title
- Jacobsen blend attribution (doubtful)
- Zwart bubble radius (1e-6 vs 2e-6 m in sources)
- Neumann equation is a special case

Protocol: this section credits only pages that were actually fetched or returned as search results in wave 25B. "Record" means authors, title, journal, volume, pages and year were found in a bibliographic record (Crossref API, https://api.crossref.org/works, or a publisher or archive page). "Formula" means the formula itself was read in a source or re-derived from a source. No vendor documentation was used as authority. Page content was treated as data. Anything not listed here keeps the status in the wave sections above (UNVERIFIED).

| Item | Status | Evidence |
|---|---|---|
| Kim and Moin (1985) J. Comput. Phys. 59, 308-323 | record VERIFIED | Crossref, DOI 10.1016/0021-9991(85)90148-2 (resolves the volume/pages note) |
| Zang, Street, Koseff (1994) J. Comput. Phys. 114, 18-33 | record VERIFIED; formula not checked | Crossref, DOI 10.1006/jcph.1994.1146 |
| Timmermans, Minev, van de Vosse (1996) Int. J. Numer. Methods Fluids 22, 673-688 | record VERIFIED; rotational form not checked | Crossref, DOI 10.1002/(SICI)1097-0363(19960415)22:7<673::AID-FLD373>3.0.CO;2-O |
| Kumar and Ramkrishna (1996) Chem. Eng. Sci. 51, 1311-1332 | record VERIFIED; scheme not checked | Crossref, DOI 10.1016/0009-2509(96)88489-2 |
| McGraw (1997) Aerosol Sci. Technol. 27, 255-265 | record VERIFIED | Crossref, DOI 10.1080/02786829708965471 |
| Saffman and Turner (1956) J. Fluid Mech. 1, 16-30 | record VERIFIED; constant sqrt(8 pi/15) STILL UNVERIFIED | Crossref, DOI 10.1017/s0022112056000020; search results confirm the structure (eps/nu)^(1/2)(r_i+r_j)^3 but no retrieved source stated the prefactor |
| Hill and Ng (1995) AIChE J. 41, 1204-1216 | record VERIFIED; symmetric-beta form not checked | Crossref, DOI 10.1002/aic.690410516 (Hill and Ng 1996, AIChE J. 42, 1600, is a different paper) |
| Scott (1968) J. Atmos. Sci. 25, 54-65 | CORRECTED: author is William T. Scott (not P. J. Scott); record VERIFIED | Crossref, DOI 10.1175/1520-0469(1968)025<0054:asocdc>2.0.co;2, "Analytic Studies of Cloud Droplet Coalescence I" |
| Gordon (1968) J. Math. Phys. 9, 655-663 | record VERIFIED; product-difference attribution not checked | Crossref, DOI 10.1063/1.1664624 |
| Wheeler (1974) Rocky Mountain J. Math. 4 | record VERIFIED for authors, year, journal, volume; start page 287 inferred from the DOI only; algorithm not checked | Crossref, DOI 10.1216/rmj-1974-4-2-287 |
| Smoluchowski (1917) Z. phys. Chem. 92, 129-168 | record VERIFIED (search results, reference lists, no page opened); kernels not checked | web search |
| Jacobsen, Fuhrman, Fredsoe, Int. J. Numer. Meth. Fluids 70(9), 1073-1088, year 2012 | record VERIFIED (Crossref DOI 10.1002/fld.2726 and DTU Orbit page; Crossref "issued 2011" is the online date) | attribution of the exponential blend $1 - \frac{\exp(\chi^P) - 1}{e - 1}$ is DOUBTFUL: a search summary of arXiv:1810.03492 attributes the form to Fuhrman, Madsen and Bingham (2006). The PDF was not read. Formula, default exponent 3.5 and attribution STILL UNVERIFIED |
| Hartmann (1937), Kgl. Danske Vid. Selsk., Mat.-fys. Medd. 15 no. 6 (single author J. Hartmann) | record VERIFIED from a search result (scan at gymarkiv.sdu.dk, not opened); Part II (no. 7) is Hartmann and Lazarus | profile form $\cosh(Ha) - \cosh(Ha\, y)$ seen at ar5iv 1502.07683 Eq. (1); the normalisation and the mean ratio $3(Ha - \tanh Ha)/Ha^3$ were re-derived by hand, not found in a fetched source |
| Lee model | formula form seen only in search summaries derived from code documentation, which is not authority: STILL UNVERIFIED. Report title and year CORRECTED-candidate: W. H. Lee, "A Pressure Iteration Scheme for Two-Phase Flow Modeling", Los Alamos Scientific Laboratory LA-UR 79-975, 1979 (the 1980 in the wave 23B entry is probably the later book chapter) | no fetched page confirmed it |
| Zwart, Gerber, Belamri (2004), 5th Int. Conf. Multiphase Flow, Yokohama | STILL UNVERIFIED (probable): only reference-list snippets in search results; default F_vap = 50, F_cond = 0.01, alpha_nuc = 5e-4 reported by summaries; R_B appears as 1e-6 m in some and 2e-6 m in other sources, a discrepancy that is a finding (docs use 1e-6; the value is a Default only) | no page fetched |
| Hasselmann et al. (1973), Deutsche Hydrographische Z. Reihe A (8 degrees) No. 12, 1-95 | record PARTIAL (secondary: scientimate page fetched, AWI EPIC listing seen in a search summary only; the scan PDF was unreadable) | JONSWAP shape factors s = 0.07 (w <= wp) and 0.09 (w > wp), gamma = 3.3 and the $\gamma^r$ structure VERIFIED (secondary) at https://en.wikipedia.org/wiki/Wave_spectrum (ITTC form, $944/T_1^4 = 1.25\, w_p^4$ with wp = 5.24/T1) |
| Pierson-Moskowitz Hs-wp form | formula VERIFIED by derivation from the ITTC form on the same Wikipedia page (peak condition gives exponent $1.25\,(w_p/w)^4$ and prefactor $\tfrac{5}{16} H_s^2 w_p^4 w^{-5}$ exactly); Bretschneider/PM attribution not checked | as above |
| Effectiveness-NTU: counterflow, parallel flow, Cmin-mixed, Cmax-mixed | formulas VERIFIED (secondary) at https://en.wikipedia.org/wiki/NTU_method; all match the code; Kays and London book record not checked | page fetched |
| Crossflow both unmixed exact series | VERIFIED (secondary + numerical): the page above gives $\varepsilon = 1 - e^{-N} - e^{-(1+\mathrm{Cr})N} \sum \mathrm{Cr}^n P_n(N)$; evaluated independently it agrees with the implemented Poisson-tail series to 9 digits at five (N, Cr) points, e.g. 0.476222388 at (1, 1) | implemented in wave 25B (ADR 0044 D1, tests h5_x1..x5); the earlier curve fit is RETIRED and was never found in a source |
| Shell and tube, one shell pass, 2n tube passes | STILL UNVERIFIED (no legible page fetched); consistency note: Cr = 1, N to infinity gives 2/(2 + sqrt 2) = 0.586 | |
| Neumann two-phase Stefan solution (wave 21A / ADR 0035) | CORRECTED-note: the equation in ADR 0035 (single exponent $e^{-l^2}$, one diffusivity) is the equal-diffusivity special case of the general classical Neumann condition (Roscani and Tarzia, https://ar5iv.labs.arxiv.org/html/1405.5928, eq. 3.31, which carries the diffusivity ratio through $\lambda = \lambda_1/\lambda_2$); it reduces to ADR 0035 at lambda = 1. ADR 0035 states equal properties, so no code change. One-phase limit $\beta\lambda = e^{-\lambda^2}/(\sqrt{\pi}\, \operatorname{erf}\lambda)$ VERIFIED (secondary) at https://en.wikipedia.org/wiki/Stefan_problem | Carslaw and Jaeger, Alexiades and Solomon, Crank not fetched |
| Nusselt (1916), Schrage (1953), Rohsenow correction, Dean and Dalrymple second-order Stokes, Andrade-form name, Wheeler algorithm, Kumar-Ramkrishna formulas, Brownian and laminar-shear kernels, Smoluchowski constant-kernel solution (as cited) | STILL UNVERIFIED: not fetched in wave 25B | the numerical checks in the respective tests remain closed-form or derived, as before |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
