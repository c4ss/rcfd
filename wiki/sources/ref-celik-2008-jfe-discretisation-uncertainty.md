---
type: source
tags: [reference, foundation, verification-validation, gci, cite-only]
sources: []
origin: https://qa12.asme.org/wwwasmeorg/media/ResourceFiles/Shop/Journals/JFENumAccuracy.pdf
licence: no licence statement in the file (ASME-hosted copy of the Journal of Fluids Engineering editorial policy); treated as all rights reserved, cite only
read: 2026-11-22
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Celik, Ghia, Roache and Freitas, "Procedure for Estimation and Reporting of Uncertainty Due to Discretization in CFD Applications"

**Origin:** file `JFENumAccuracy.pdf` hosted on an ASME server (qa12.asme.org), 15 pages: the *Journal of Fluids Engineering* "Editorial Policy Statement on the Control of Numerical Accuracy" (ten criteria, pp. 1-2 of the file) followed by Ismail B. Celik (corresponding author, West Virginia University), Urmila Ghia, Patrick J. Roache and Christopher J. Freitas, "Procedure for Estimation and Reporting of Uncertainty Due to Discretization in CFD Applications" (its own pages 1-13, with Appendix A). The journal version is cited elsewhere as J. Fluids Eng. 130, 078001 (July 2008), with Coleman and Raad as further authors; the file read here lists four authors and carries no journal volume, so it is the policy-page version of the note.
**Kind:** journal editorial policy and procedure note.
**Licence:** none stated; cite only. Text extracted with `pdftotext` from the downloaded file; several equations extracted with lost symbols and were reconstructed from their surrounding definitions (noted below).
**Read:** 2026-11-22, by job p6-roots. This is the Celik et al. source that [[gci-richardson]] recorded as not read.

## Summary

The JFE requires systematic truncation-error testing and accuracy estimation for any CFD paper, and recommends a five-step procedure based on Richardson extrapolation: a representative cell size, three significantly different grids, the apparent order by fixed-point iteration (allowing non-constant ratios), extrapolated values, and the reported errors including the fine-grid convergence index with factor of safety 1.25.

## Key points

### Editorial policy (ten criteria)
- The formal order of the truncation error of each term must be stated; the method must be at least formally second order in space at interior nodes; blended or switched first/second-order schemes ("hybrid", "power-law") count as first order unless shown otherwise.
- Solutions on significantly different grid resolutions must show grid convergence; Richardson extrapolation may be used; stopping criteria of iterations must be explained with convergence-error estimates; temporal accuracy and boundary-condition implementation must be addressed.
- Benchmark comparisons do not in general demonstrate accuracy for another class of problems; "reasonable agreement" with experimental data alone does not justify a single-grid calculation.

### Preliminaries of the procedure (pp. 1-2)
- "Verification" here means the code solves the posed equations correctly and reproduces the exact solution on fine enough grids; previous code verification must be cited and the formal order stated.
- Before any discretisation-error estimate, iterative convergence must be shown "with at least three (preferably four) orders of magnitude decrease in the normalized residuals for each equation" (with a caveat in Appendix A that this does not always suffice).
- Sensitivity to small perturbations of inlet conditions should be studied and reported.
- Richardson extrapolation is "currently the most reliable method available"; its local values need not be smooth or monotone. The GCI method "has been evaluated over several hundred CFD cases"; if used, the method will not be challenged in review.

### Recommended procedure (pp. 3-5)
- **Step 1**, Eq. 1-2: $h = \left[\tfrac{1}{N} \sum \Delta V_i\right]^{1/3}$ in 3D, $h = \left[\tfrac{1}{N} \sum \Delta A_i\right]^{1/2}$ in 2D, for integral quantities; local cell size for field variables.
- **Step 2:** three significantly different grids; "desirable that the grid refinement factor, $r = h_\text{coarse}/h_\text{fine}$, be greater than 1.3", a value "based on experience, and not on formal derivation"; refinement systematic even for unstructured grids.
- **Step 3**, Eq. 3a-3c (reconstructed from the garbled extraction): with $h_1 < h_2 < h_3$, $r_{21} = h_2/h_1$, $r_{32} = h_3/h_2$, $\varepsilon_{32} = \phi_3 - \phi_2$, $\varepsilon_{21} = \phi_2 - \phi_1$: $p = \dfrac{1}{\ln r_{21}} \left\lvert \ln \lvert \varepsilon_{32}/\varepsilon_{21} \rvert + q(p) \right\rvert$, $q(p) = \ln\left(\dfrac{r_{21}^p - s}{r_{32}^p - s}\right)$, $s = 1 \cdot \operatorname{sgn}(\varepsilon_{32}/\varepsilon_{21})$; $q = 0$ for constant $r$; solve by fixed-point iteration starting from the first term. $\varepsilon_{32}/\varepsilon_{21} < 0$ indicates oscillatory convergence, whose percentage should be reported. Agreement of the apparent order with the formal order indicates the asymptotic range; "the converse should not necessarily be taken as a sign of unsatisfactory calculations". If $\varepsilon_{32}$ or $\varepsilon_{21}$ is very close to zero the procedure does not work.
- **Step 4**, Eq. 4: $\phi_\text{ext}^{21} = (r_{21}^p \phi_1 - \phi_2)/(r_{21}^p - 1)$, likewise $\phi_\text{ext}^{32}$.
- **Step 5**, Eq. 5-7: approximate relative error $e_a^{21} = \lvert (\phi_1 - \phi_2)/\phi_1 \rvert$; extrapolated relative error $e_\text{ext}^{21} = \lvert (\phi_\text{ext}^{12} - \phi_1)/\phi_\text{ext}^{12} \rvert$; fine-grid convergence index $GCI_\text{fine}^{21} = 1.25\, e_a^{21} / (r_{21}^p - 1)$.
- Table 1 (backward-facing step, Celik and Karatekin data), dimensionless reattachment length on $N = 18000, 8000, 4500$ cells: $r_{21} = 1.5$, $r_{32} = 1.333$, $\phi_1 = 6.063$, $\phi_2 = 5.972$, $\phi_3 = 5.863$, $p = 1.53$, $\phi_\text{ext}^{21} = 6.1685$, $e_a^{21} = 1.5\,\%$, $e_\text{ext}^{21} = 1.7\,\%$, $GCI_\text{fine}^{21} = 2.2\,\%$. The fine-grid uncertainty "should be reported as 2.2%; note that this does not account for modeling errors". Two further columns (axial velocity, one monotone with $p < 1$, one oscillatory) did not extract cleanly and are not used.

### Error bars and noisy convergence (pp. 5-7)
- Profiles should carry numerical error bars from the GCI with an average order $p_\text{ave}$. Example (turbulent backward step, 980 / 4500 / 18000 cells): local $p$ from 0.012 to 8.47, $p_\text{ave} = 1.49$, oscillatory convergence at 20 % of 22 points, maximum uncertainty 10 % (0.35 m/s). Laminar step (Re 230, 20x20 / 40x40 / 80x80): $p$ 0.1 to 3.7, $p_\text{ave} = 1.38$, 80 % of 22 points oscillatory.
- For noisy grid convergence, the least-squares version of the GCI should be considered (refs. 15-16, Eca, Hoekstra and Roache).

### Appendix A (iteration error)
- Following Ferziger, the iteration error is estimated from the change between iterations and the principal eigenvalue of the iteration (Eq. A.1-A.3); a limiter $\lambda < 2$ is recommended when averaging; the iteration error should be "at least one order of magnitude smaller than the discretization error estimates".

## Contradictions with the wiki

None found. rcfd's F-870 form $U_\text{num} = 1.25\, \lvert f_2 - f_1 \rvert / (r_{21}^{p_\text{band}} - 1)$ ([[gci-richardson]]) equals $\lvert \phi_1 \rvert\, GCI_\text{fine}^{21}$ of Eq. 7, i.e. the same index in absolute rather than relative units. The cap $p_\text{band} = \min(p, 1)$ and the cylinder validity window $p$ in [1.5, 2.5] are rcfd's own rules and are not in this source. Between references: Celik's $r > 1.3$ versus the NPARC tutorial's $r \ge 1.1$ ([[ref-nparc-vv-tutorial]]).

## Pages this source informs

[[solution-verification]], [[gci-richardson]], [[code-verification-and-manufactured-solutions]]
