---
type: source
tags: [reference, foundation, multiphase, particles, turbulence, cc-by]
sources: []
origin: https://arxiv.org/abs/2501.00037
licence: CC BY 4.0 (arXiv licence link, read 2026-11-22)
read: 2026-11-22
raw: raw/marzouk-huckaby-2009-particle-laden-swirl.pdf
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Marzouk and Huckaby (2009), Turbulence modelling and parcel approach in dispersed two-phase swirling flow

**Origin:** O. A. Marzouk, E. D. Huckaby, "Effects of Turbulence Modeling and Parcel Approach on Dispersed Two-Phase Swirling Flow", Proceedings of the World Congress on Engineering and Computer Science 2009 Vol II (WCECS 2009), San Francisco, 20-22 October 2009; arXiv:2501.00037 (posted 2024-12-24). Copy: [raw/marzouk-huckaby-2009-particle-laden-swirl.pdf](../raw/marzouk-huckaby-2009-particle-laden-swirl.pdf).
**Kind:** conference paper (US DOE NETL), Euler-Lagrange RANS of the Sommerfeld and Qiu particle-laden swirling jet.
**Licence:** CC BY 4.0.
**Read:** 2026-11-22 by job p6-multiphase-heat: sec. 1-3 (governing equations, k-epsilon variants, wall functions, particle motion), the parcel results and conclusions skimmed.

## Summary

A clear, compact statement of a standard Euler-Lagrange closure: Lagrangian particle equations with drag, gravity and buoyancy, a sphere drag law close to Schiller-Naumann, the discrete random walk for turbulent dispersion with an eddy lifetime and a crossing time from $k$ and $\epsilon$, two-way momentum coupling through a source term, and grouping of particles into parcels.

## Key points by section

### Sec. 1 "Introduction"
- Test case: Sommerfeld and Qiu's coaxial swirling jet, glass beads of 2,500 kg/m$^3$, log-normal size distribution with mean number diameter 45 $\mu$m, loading 0.034 kg particles per kg air; parcels group "multiple similar particles" to cut cost.

### Sec. 2 "Governing Equations"
- Carrier momentum with a source $S_p$ "accounting for the momentum from the particle-phase" (Eq. 3); the gas and particles "are coupled through two-way momentum exchange" (abstract).
- Eddy viscosity $\mu_t = C_\mu\rho k^2/\epsilon$ (Eq. 5); standard constants $C_\mu = 0.09$, $\sigma_k = 1.0$, $\sigma_\epsilon = 1.3$, $C_{\epsilon1} = 1.44$, $C_{\epsilon2} = 1.92$ (sec. 2.1).

### Sec. 3 "Particle Motion"
- $dx/dt = u$ (Eq. 16a), $m\,du/dt = f$ (Eq. 16b) with drag, gravity and buoyancy: $f = -\frac{\pi d^2}{8}\rho C_D|u-U|(u-U) + m g - \rho\mathcal V g$ (Eq. 17), $\mathcal V$ the particle volume and $U$ the instantaneous fluid velocity at the particle.
- $U = \bar U + U'$ (Eq. 18): $\bar U$ is the resolved fluid velocity interpolated to the particle, $U'$ comes from the discrete random walk: "uncorrelated eddies are generated randomly, but the particle trajectory is deterministic within the eddy". The fluctuation acts over an interaction time $T_{interac} = \min(T_{eddy}, T_{cross})$ with $T_{eddy} = k/\epsilon$ (Eq. 19a, "Lagrangian integral time scale of turbulence") and $T_{cross} = C_{cross}\,k^{3/2}/(\epsilon\,|u - \bar U - U'|)$ (Eq. 19b), $C_{cross} = 0.16432 = C_\mu^{3/4}$; the turbulence data are updated every time step.
- Sphere drag (Eq. 20): $C_D = \frac{24}{Re_d}\left(1 + \frac16 Re_d^{2/3}\right)$ for $Re_d \le 1000$, $C_D = 0.424$ for $Re_d > 1000$, "very similar to the Schiller-Naumann expression for $Re_d \le 1000$"; $Re_d = \rho|u-U|d/\mu$ (Eq. 21).
- With $m = \rho_s\pi d^3/6$: $du/dt = -(u-U)/\tau + (1-\rho/\rho_s)g$ (Eq. 22b), momentum relaxation time $\tau = \frac{4}{3}\frac{\rho_s d}{\rho C_D|u-U|} = \frac{\rho_s d^2}{18\mu}\frac{24}{Re_d C_D}$.
- Tracking: substeps within a cell, forward Euler for position, backward Euler for velocity; the force on the fluid accounts for the particle's residence time in each cell. Walls: "elastic collisions. The tangential friction with the wall is neglected."

### Sec. 5-6 Results and conclusions (skimmed)
- The standard k-epsilon model gave the best overall agreement; the results were not sensitive to particle versus parcel tracking at this loading.

## Contradictions with the wiki

None. The low-Re drag of Eq. 20 differs from the Schiller-Naumann form $1 + 0.15\,Re^{0.687}$ that [[doc-reference-data-wave29b-cavitation]] recalls from memory; the paper itself says only that the two are "very similar". Not a contradiction: different correlations.

## Pages this source informs

[[particle-equation-of-motion]], [[turbulent-dispersion-random-walk]], [[two-way-coupling]], [[particle-wall-interaction]]
