---
type: source
tags: [reference, foundation, multiphase, droplets, evaporation, cc-by]
sources: []
origin: https://arxiv.org/abs/2411.18753
licence: CC BY 4.0 (arXiv licence link, read 2026-11-22)
read: 2026-11-22
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Salimnezhad, Turkeri, Gokalp and Muradoglu (2025), interface-resolved droplet evaporation

**Origin:** F. Salimnezhad, H. Turkeri, I. Gokalp, M. Muradoglu, "A Hybrid Immersed-Boundary/Front-Tracking Method for Interface-Resolved Simulation of Droplet Evaporation", *Computers and Fluids* 291 (2025) 106570; arXiv:2411.18753. PDF: https://arxiv.org/pdf/2411.18753
**Kind:** research paper.
**Licence:** CC BY 4.0. Copyable, but not copied into `raw/` because the PDF is about 23 MB; the lead may add it.
**Read:** 2026-11-22 by job p6-multiphase-heat: introduction, sec. 4.3 "The $d^2$-law", and the opening of the point-particle section (sec. 5). The method sections were not read.

## Summary

An interface-resolved evaporation method, validated first against the classical $d^2$-law for a stationary droplet in stagnant air, then against the wet-bulb temperature and low-order convective models (Abramzon-Sirignano and the classical model).

## Key points by section

### Sec. 4.3 "The $d^2$-law"
- A stationary single droplet of initial diameter $d_0$ evaporating in stagnant air, natural convection neglected: $d^2 = d_0^2 - 8\frac{\rho_g}{\rho_l}D\,\ln(1+B_M)\,t$ (Eq. 32), with gas density $\rho_g$, liquid density $\rho_l$, vapour diffusion coefficient $D$, and the mass transfer number $B_M = (Y_{v,s} - Y_{v,\infty})/(1 - Y_{v,s})$ from the saturation and far-field vapour mass fractions.
- The numerical results converge to the law as the domain grows, "since the $d^2$-law assumes that the droplet evaporates in an unbounded domain". Tested for $B_M = 0.025$, 0.05 and 0.1.

### Sec. 5 (opening)
- Interface-resolved methods are not feasible for sprays and droplet clouds; Lagrangian point-particle methods with low-order evaporation models compute the mass and energy transfer instead, and the accuracy of such simulations depends on those models (reviews by Faeth, Sirignano, Law, Sazhin).

## Contradictions with the wiki

None.

## Pages this source informs

[[droplet-evaporation-and-breakup]]
