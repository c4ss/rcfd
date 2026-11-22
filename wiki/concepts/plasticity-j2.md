---
type: concept
tags: [foundation, structural, plasticity, nonlinear]
sources: [ref-roylance-mit-3-11-mechanics-of-materials, ref-bednarcyk-2006-nasa-tm-2006-214331, adr-thread-structural-materials-nonlinear]
builds_on: [linear-elasticity]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# J2 (von Mises) plasticity and the radial return algorithm

## In short

Metals deform elastically up to a yield stress and then flow plastically, leaving permanent strain after unloading. The von Mises (J2) criterion says yield starts when the distortional part of the stress reaches a limit, independent of hydrostatic pressure. In a finite element code the plastic update at each integration point is done by "radial return": compute an elastic trial stress and, if it lies outside the yield surface, scale its deviator back onto the surface ([[ref-roylance-mit-3-11-mechanics-of-materials]], "Yield and Plastic Flow"; [[ref-bednarcyk-2006-nasa-tm-2006-214331]], sec. 2).

## Explanation

**Yield criteria.** Tresca: yield at a maximum shear stress $k = Y/2$. Von Mises: the equivalent stress $$\sigma_M = \sqrt{\tfrac12\big[(\sigma_1-\sigma_2)^2 + (\sigma_1-\sigma_3)^2 + (\sigma_2-\sigma_3)^2\big]}$$ equals the tensile yield stress $Y$ at yield; it is expressed through the second invariant $J_2$ of the deviatoric stress, and the shear yield stress is $k = Y/\sqrt3$. Since hydrostatic stress causes only elastic volume change, yield depends on the deviator ([[ref-roylance-mit-3-11-mechanics-of-materials]], "Yield and Plastic Flow"; [[linear-elasticity]]).

**Elastic-plastic behaviour.** In the elastic-perfectly plastic idealisation the stress stays at $Y$ once yielded; unloading follows a line parallel to the elastic one, leaving residual plastic strain, and in a non-uniformly loaded body residual stresses ([[ref-roylance-mit-3-11-mechanics-of-materials]], "Continuum plasticity").

**Rate-independent J2 with isotropic hardening.** Yield function $$f = \|\mathbf s\| - \sqrt{2/3}\;\sigma_y(\bar\epsilon_p) = 0$$ (Bednarcyk et al., eq. 1), $\mathbf s$ the stress deviator, $\|\mathbf s\| = \sqrt{2J_2}$ (3). Associated flow: $\Delta\boldsymbol\epsilon^p = \Delta\gamma\,\hat{\mathbf n}$, $\hat{\mathbf n} = \mathbf s/\|\mathbf s\|$ (5), so plastic flow is along the deviator and volume-preserving; $\Delta\bar\epsilon_p = \sqrt{2/3}\,\Delta\gamma$ (6); deviatoric elasticity $\mathbf s = 2G(\mathbf e - \boldsymbol\epsilon^p)$ (7) ([[ref-bednarcyk-2006-nasa-tm-2006-214331]]).

**Radial return** (sec. 2, eqs. 9-19), given the strain increment of a step:

1. elastic trial deviator $\mathbf s^T_{n+1} = \mathbf s_n + 2G(\mathbf e_{n+1} - \mathbf e_n)$ (9);
2. if $f(\mathbf s^T_{n+1}, \bar\epsilon_{p,n}) < 0$ the step is elastic (10);
3. otherwise $\mathbf s_{n+1} = \mathbf s^T_{n+1} - 2G\,\Delta\gamma\,\hat{\mathbf n}$ with $\hat{\mathbf n}$ the trial direction (12), the "key" to the method; $\Delta\gamma$ solves $$g(\Delta\gamma) = \|\mathbf s^T_{n+1}\| - 2G\,\Delta\gamma - \sqrt{2/3}\;\sigma_y(\bar\epsilon_{p,n+1}) = 0$$ (17);
4. update $\bar\epsilon_{p,n+1} = \bar\epsilon_{p,n} + \sqrt{2/3}\,\Delta\gamma$ (18) and $\boldsymbol\epsilon^p_{n+1} = \boldsymbol\epsilon^p_n + \Delta\gamma\,\hat{\mathbf n}$ (19).

For linear hardening (Table 1) $\Delta\gamma$ is closed form, $$\Delta\gamma = \frac{\|\mathbf s^T_{n+1}\| - \sqrt{2/3}\,(\sigma_Y + H\bar\epsilon_{p,n})}{2G + \tfrac23 H}, \qquad H = \frac{E\,E_T}{E - E_T};$$ nonlinear hardening needs a local Newton iteration on $g$ (Table 2). The method is due to Wilkins (1964) and Simo and Taylor (1985), and the report shows it equals Mendelson's method ([[ref-bednarcyk-2006-nasa-tm-2006-214331]]).

**Local and global iterations.** The return-mapping iterations at an integration point are separate from the global Newton iterations of the structure (sec. 1); quadratic global convergence needs a tangent consistent with the return mapping, which the report does not derive ([[ref-bednarcyk-2006-nasa-tm-2006-214331]]; [[nonlinear-fem-newton]]). Kinematic hardening was not covered by the sources read.

## Builds on

- [[linear-elasticity]]: the deviatoric/volumetric split, the shear modulus $G$.

## Leads to

- [[nonlinear-fem-newton]], [[viscoelasticity-and-creep]] (rate-dependent flow on the same deviatoric structure)

## How rcfd uses it

- J2 plasticity with isotropic and kinematic hardening, radial return and consistent tangent (F-422, ADR 0475, partial: residual hoop stress 2.6 percent against 2 percent, Newton order 1.27 to 1.44 against 1.8); the sphere re-run on Hex20 layers (F-561, ADR 0512) ([[adr-thread-structural-materials-nonlinear]]).
