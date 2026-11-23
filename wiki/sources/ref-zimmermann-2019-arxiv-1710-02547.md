---
type: source
tags: [reference, foundation, structural, time-integration, arxiv]
sources: []
origin: https://arxiv.org/abs/1710.02547
licence: arXiv non-exclusive distribution licence 1.0 (arXiv abstract page). Not an open licence, so cite only.
read: 2026-11-22
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Zimmermann, Toshniwal, Landis, Hughes, Mandadapu and Sauer (2019), An isogeometric finite element formulation for phase transitions on deforming surfaces

**Origin:** C. Zimmermann, D. Toshniwal, C. M. Landis, T. J. R. Hughes, K. K. Mandadapu, R. A. Sauer, arXiv:1710.02547v3 [math.NA], 22 April 2019; published in *Computer Methods in Applied Mechanics and Engineering*.
**Kind:** research paper (arXiv preprint). Used here only for its appendix that restates the generalised-alpha scheme.
**Licence:** arXiv non-exclusive distribution; cite only.
**Read:** 2026-11-22 by job p6-structural: Appendix C.1 "Generalized-$\alpha$ method" (eqs. 123-124) and Appendix E.3 Newmark formulae (eqs. 174-175). The body of the paper (phase transitions on surfaces) was not read.

## Summary

Appendix C.1 states the generalised-alpha time integration of Chung and Hulbert (1993) for a second-order system, with the equations satisfied at intermediate times and the parameters expressed through one user parameter, the spectral radius at infinite step.

## Key points

- **App. E.3, eq. (174), Newmark's formulae for second-order systems:** $\mathbf x_{n+1} = \mathbf x_n + \Delta t\,\dot{\mathbf x}_n + \Delta t^2\big[(\tfrac12 - \beta)\ddot{\mathbf x}_n + \beta\ddot{\mathbf x}_{n+1}\big]$, $\dot{\mathbf x}_{n+1} = \dot{\mathbf x}_n + \Delta t\big[(1-\gamma)\ddot{\mathbf x}_n + \gamma\ddot{\mathbf x}_{n+1}\big]$; (175) is the first-order analogue.
- **App. C.1, eq. (123):** the system is required to hold at intermediate times: $\mathbf x_{n+\alpha_f} = (1-\alpha_f)\mathbf x_n + \alpha_f\mathbf x_{n+1}$, the same for $\dot{\mathbf x}$, and $\ddot{\mathbf x}_{n+\alpha_m} = (1-\alpha_m)\ddot{\mathbf x}_n + \alpha_m\ddot{\mathbf x}_{n+1}$, with the Newmark formulae linking the end values.
- **App. C.1, eq. (124):** the four parameters control numerical dissipation and follow from $\rho_\infty \in [0, 1]$, the spectral radius of the amplification matrix as $\Delta t \to \infty$: $\alpha_f = \dfrac{1}{1+\rho_\infty}$, $\alpha_m = \dfrac{2-\rho_\infty}{1+\rho_\infty}$, $\gamma = \tfrac12 + \alpha_m - \alpha_f$, $\beta = \tfrac14(1 + \alpha_m - \alpha_f)^2$ ("see Chung and Hulbert (1993) for further details"); the paper uses $\rho_\infty = 0.5$.

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[newmark-and-generalised-alpha]]
