---
type: source
tags: [reference, foundation, multiphase, cavitation, cc-by]
sources: []
origin: https://arxiv.org/abs/2211.16323
licence: CC BY 4.0 (arXiv licence link, read 2026-11-22)
read: 2026-11-22
raw: raw/folden-aschmoneit-2023-cavitation-models.pdf
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Folden and Aschmoneit (2023), classification and review of cavitation models

**Origin:** T. S. Folden, F. J. Aschmoneit, "A classification and review of cavitation models with an emphasis on physical aspects of cavitation", *Physics of Fluids* 35, 081301 (2023); arXiv:2211.16323 v4. Copy: [raw/folden-aschmoneit-2023-cavitation-models.pdf](../raw/folden-aschmoneit-2023-cavitation-models.pdf).
**Kind:** review paper.
**Licence:** CC BY 4.0.
**Read:** 2026-11-22 by job p6-multiphase-heat: sec. I, sec. II.1 (Eqs. 1-6), sec. III.9 "The Bubble Density-Liquid Volume Coupling Model by Schnerr and Sauer" (Eqs. 42-47), sec. IV conclusions. Other models skimmed only.

## Summary

Sorts cavitation models by approach (homogeneous mixture with a transport equation, equation of state, interface tracking, Lagrangian, multiscale) and by the physical effects they include, then classifies selected models. The Schnerr-Sauer model is derived from a simplified Rayleigh-Plesset equation.

## Key points by section

### Sec. I "Introduction"
- Cavitation: local liquid-to-vapour phase change when the pressure drops below the vapour pressure; sheet, vortex and cloud cavities; collapse in higher pressure ("bubble implosion") gives shock waves that can damage material. Cavitating flow is tightly coupled to turbulence.

### Sec. II.1 "Categorization by modelling approach"
- Homogeneous mixture: phases in mechanical and thermal equilibrium, one velocity and pressure; $\rho = \alpha_v\rho_v + (1-\alpha_v)\rho_l$, likewise $\mu$ (Eq. 2), $\alpha_v$ the vapour volume fraction.
- Transport equation models (TEMs): $\partial(\alpha_i\rho_i)/\partial t + \nabla\cdot(\alpha_i\rho_i\vec u) = \dot m_i$ (Eq. 3); the source splits into evaporation $\dot m^+$ for $p < p_v$ and condensation $\dot m^-$ for $p > p_v$ (Eq. 4).
- Rayleigh-Plesset equation for a spherical bubble of radius $R$ in an infinite liquid (Eq. 5): $R\ddot R + \frac32\dot R^2 = \frac{p_b - p}{\rho_l} - \frac{4\mu_l\dot R}{\rho_l R} - \frac{2\sigma}{\rho_l R}$; "The full RPE is rarely used in cavitation modelling"; simplified versions drop surface tension or inertial terms.
- For a cluster of equal bubbles, $\alpha_v = n_0\frac43\pi R^3$ (Eq. 6), $n_0$ the bubble number density (a constant or its own transported variable).
- Limits named: TEMs that assume no bubble-bubble interaction can fail for cloud cavities; EOS models avoid the RPE's assumptions (spherical bubbles in infinite liquid).

### Sec. III.9 Schnerr and Sauer
- Vapour volume approximated as $V_v = \frac43\pi n_0 R^3$ (Eq. 42), which with $V_{cell} = V_l + V_v$ gives $\alpha_v = \frac{\frac43\pi n_0 R^3}{1 + \frac43\pi n_0R^3}$ (Eq. 43); the transport equation for $\alpha_v$ gets a source proportional to $\frac{n_0}{1+\frac43\pi n_0R^3}\,\frac{d}{dt}\big(\frac43\pi R^3\big)$ (Eq. 44).
- Bubble growth from the RPE with the second-order terms, viscosity and surface tension neglected, the bubble pressure set to $p_v$ and the reference pressure to the cell pressure: $dR/dt = \sqrt{\frac{2|p_v - p|}{3\rho_l}}$ (Eq. 45); with Eq. 44 this gives the evaporation and condensation sources (Eqs. 46, 47).
- "no empirical constants besides the uniform bubble number $n_0$ are used"; the model is implemented in a VOF framework and was validated on hydrofoil cavitation.

### Sec. IV "Conclusions"
- "The current standard for cavitation modelling is set by the combination of TEM and RPE approaches" (Kunz, Schnerr and Sauer, Singhal, Zwart, all from around 2000).

## Contradictions with the wiki

None. Eq. 43 ($\alpha_v = x/(1+x)$ with $x = n\frac43\pi R^3$) and Eq. 45 match the forms that [[doc-reference-data-wave29b-cavitation]] lists as "recalled from memory" and UNVERIFIED for ADR 0056; this paper is a readable secondary source for them, not the 2001 original.

## Pages this source informs

[[cavitation-and-mass-transfer-models]], [[two-phase-flow-basics]]
