---
type: source
tags: [reference, foundation, multiphase, particles, cc-by]
sources: []
origin: https://www.frontiersin.org/articles/10.3389/fphy.2023.1167338
licence: CC BY 4.0 ("This is an open-access article distributed under the terms of the Creative Commons Attribution License (CC BY)", article copyright box)
read: 2026-11-22
raw: raw/jaganathan-2023-basset-history-force.pdf
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Jaganathan, Prasath, Govindarajan and Vasan (2023), The Basset-Boussinesq history force

**Origin:** D. Jaganathan, S. G. Prasath, R. Govindarajan, V. Vasan, "The Basset-Boussinesq history force: its neglect, validity, and recent numerical developments", *Frontiers in Physics* 11:1167338 (2023), doi:10.3389/fphy.2023.1167338. Mini review. Copy: [raw/jaganathan-2023-basset-history-force.pdf](../raw/jaganathan-2023-basset-history-force.pdf).
**Kind:** open-access review paper.
**Licence:** CC BY 4.0.
**Read:** 2026-11-22 by job p6-multiphase-heat: abstract, sec. 1 (Introduction, with Eqs. 1a, 1b, 2), sec. 2.1 (Eqs. 3-5). The equation bodies were partly garbled in the text extraction; only what the prose states is used here.

## Summary

The Maxey-Riley-Gatignol (MRG) equation is the force balance for a small rigid sphere in a non-uniform flow. The review explains its assumptions, the place of the Basset-Boussinesq history (BBH) force among the other forces, when neglecting BBH is or is not justified, and newer history kernels and numerical methods.

## Key points by section

### Abstract and sec. 1 "Introduction"
- Inertial (finite-size) particles relax to the surrounding flow in finite time and so leave the fluid trajectories; tracers relax instantly. Inertial particles can collect in regions of the flow (preferential concentration).
- MRG assumptions: rigid, spherical particles small compared with the flow length scales; dilute suspension, so particle-particle interaction is neglected and coupling is one-way ("neglect the effect of particle on fluid flow"); a weak disturbance flow, so that the particle Reynolds number $Re_p = W_s a/\nu$ (slip velocity $W_s$, particle radius $a$) and the shear Reynolds number $Re_s = a^2 s/\nu$ (flow gradient $s$) stay small (creeping flow).
- The forces on such a particle: Stokes and pressure drag, added mass, and BBH. Faxén corrections are omitted for small enough particles. In the non-dimensional MRG (Eqs. 1a, 1b), $x_p$ is the particle position, $w_s = \dot x_p - u(x_p)$ the slip velocity, $R$ a function of the particle-to-fluid density ratio, and $S$ the Stokes number, "ratio of particle relaxation timescale to flow timescale"; the BBH term is a time integral of the slip acceleration with the Basset kernel $K_B \propto 1/\sqrt t$ (Eqs. 1b, 4).
- BBH is "often neglected, not because such neglect is known to be justified, but because it is difficult to be included". Conventional density-ratio arguments: BBH as important as Stokes drag for marginally heavy particles ($R \sim 1$), negligible for much heavier particles, but the latter holds only for a point particle; for a finite-size particle the relative strength of Stokes drag and BBH depends on $S$, not on the density ratio.
- With BBH a sphere relaxing in quiescent fluid approaches its terminal velocity algebraically, without it exponentially; experiments (Mordant and Pinton) support the algebraic behaviour.

### Sec. 2.1 "Late-time onset of advective/convective inertial effects"
- At early times the unsteady Stokes equation gives the Basset kernel (Boussinesq, Basset; Maxey and Riley, Gatignol for non-uniform flow). At later times inertia changes the kernel; Mei and Adrian proposed a semi-empirical kernel that reduces to the Basset kernel at short times (Eq. 5).

## Contradictions with the wiki

None.

## Pages this source informs

[[particle-equation-of-motion]], [[two-way-coupling]]
