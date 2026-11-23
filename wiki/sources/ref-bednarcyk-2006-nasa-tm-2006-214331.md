---
type: source
tags: [reference, foundation, structural, plasticity, nasa]
sources: []
origin: https://ntrs.nasa.gov/api/citations/20060045066/downloads/20060045066.pdf
licence: NASA technical memorandum, work of the US Government (NASA Glenn Research Center, with Ohio Aerospace Institute co-authors); no licence statement on the pages read. Not copied into raw/ (licence not verified on the page).
read: 2026-11-22
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Bednarcyk, Aboudi and Arnold (2006), The Equivalence of the Radial Return and Mendelson Methods for Integrating the Classical Plasticity Equations, NASA/TM-2006-214331

**Origin:** B. A. Bednarcyk, J. Aboudi (Ohio Aerospace Institute) and S. M. Arnold (NASA Glenn Research Center), NASA/TM-2006-214331, July 2006, NTRS 20060045066.
**Kind:** NASA technical memorandum (PDF). Locators are section, table and equation numbers.
**Licence:** see frontmatter; cite only.
**Read:** 2026-11-22 by job p6-structural: abstract, sec. 1, Tables 1 and 2, sec. 2 (radial return method, eqs. 1-19). The Mendelson sections, results and the FORTRAN appendix were not read.

## Summary

The radial return method (attributed to Wilkins 1964; Simo and Taylor 1985) and Mendelson's method for integrating rate-independent classical (von Mises, isotropic hardening) plasticity at an integration point over a strain increment are shown to be identical. Sec. 2 gives the radial return algorithm step by step.

## Key points

- **Sec. 1, problem:** at an integration point and load step $n$, given the strain increment, find the stress at $n+1$, the converged state at $n$ being known; small deformation, quasi-static, rate-independent; local iterations to stay on the yield surface are separate from the global iterations of the structural solve.
- **Sec. 2, eqs. (1)-(8):** yield function with isotropic hardening $f(\mathbf s, \bar\epsilon_p) = \|\mathbf s\| - \sqrt{2/3}\,\sigma_y(\bar\epsilon_p) = 0$ (1), $\mathbf s$ the stress deviator (2), $\|\mathbf s\| = \sqrt{s_{ij}s_{ij}} = \sqrt{2J_2}$ (3); equivalent plastic strain increment $\Delta\bar\epsilon_p = \sqrt{\tfrac23\Delta\epsilon^p_{ij}\Delta\epsilon^p_{ij}}$ (4); associated flow rule $\Delta\boldsymbol\epsilon^p = \Delta\gamma\,\partial f/\partial\mathbf s = \Delta\gamma\,\hat{\mathbf n}$ (5) with $\hat{\mathbf n} = \mathbf s/\|\mathbf s\|$, so $\Delta\bar\epsilon_p = \sqrt{2/3}\,\Delta\gamma$ (6); deviatoric elasticity $\mathbf s = 2G(\mathbf e - \boldsymbol\epsilon^p)$ (7).
- **Sec. 2, eqs. (9)-(17), the algorithm:** elastic trial stress $\mathbf s^T_{n+1} = \mathbf s_n + 2G(\mathbf e_{n+1} - \mathbf e_n)$ (9); if $f(\mathbf s^T_{n+1}, \bar\epsilon_{p,n}) < 0$ (10) the step is elastic; otherwise $\mathbf s_{n+1} = \mathbf s^T_{n+1} - 2G\Delta\gamma\,\hat{\mathbf n}$ (12) and the normal is that of the trial stress, $\hat{\mathbf n} = \mathbf s^T_{n+1}/\|\mathbf s^T_{n+1}\|$, "key to the radial return method": the return to the yield surface is along the trial direction. The magnitude solves $g(\Delta\gamma) = \|\mathbf s^T_{n+1}\| - 2G\Delta\gamma - \sqrt{2/3}\,\sigma_y(\bar\epsilon_{p,n+1}) = 0$ (17).
- **Updates, eqs. (18)-(19):** $\bar\epsilon_{p,n+1} = \bar\epsilon_{p,n} + \sqrt{2/3}\,\Delta\gamma$ (18), $\boldsymbol\epsilon^p_{n+1} = \boldsymbol\epsilon^p_n + \Delta\gamma\,\hat{\mathbf n}$ (19).
- **Table 1, linear hardening:** closed form $\Delta\gamma = \dfrac{\|\mathbf s^T_{n+1}\| - \sqrt{2/3}\,(\sigma_Y + H\bar\epsilon_{p,n})}{2G + \tfrac23 H}$ with $H = E E_T/(E - E_T)$; Table 2: for nonlinear hardening, Newton-Raphson on $g(\Delta\gamma)$ with a tolerance.

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[plasticity-j2]]
