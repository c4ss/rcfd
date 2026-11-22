---
type: concept
tags: [foundation, turbulence, wall-functions]
sources: [ref-davidson-turbulence-notes, ref-mit-ocw-2-29-turbulence, ref-menter-1992-nasa-tm-103975, ref-nasa-tmr]
builds_on: [law-of-the-wall]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Wall functions (and the low-Re alternative)

## In short

A wall function uses the law of the wall as a boundary condition: instead of resolving the steep near-wall gradients, the first cell is placed in the log layer, the friction velocity is found from the log law and the cell's velocity, and the wall shear stress and the turbulence values in that cell are set from it. This saves many cells but is only as good as the log-law assumption. The alternative, "low-Reynolds-number" treatment, resolves the wall with a fine mesh ($y^+$ about 1) and a model that is valid down to the wall ([[ref-davidson-turbulence-notes]], [[ref-mit-ocw-2-29-turbulence]]).

## Explanation

**Two options** ([[ref-davidson-turbulence-notes]] sec. 11.14, pp. 154-155): a coarse mesh near the wall with the log law assumed (wall functions), or a fine mesh with the turbulence model modified for viscous effects (low-Reynolds-number models).

**Standard wall function for k-epsilon** ([[ref-davidson-turbulence-notes]] eqs. 11.125-11.135; [[ref-mit-ocw-2-29-turbulence]] L25 s.23):
1. Log law $U_P/u_\tau = (1/\kappa) \ln(E u_\tau y_P/\nu)$ at the wall-adjacent cell centre $P$, solved iteratively for $u_\tau$ (it "converges very quickly").
2. Wall shear $\tau_w = \rho u_\tau^2$ is applied as a force (flux) condition for the wall-parallel velocity.
3. In the log layer production balances dissipation and $-\overline{u'v'} = u_\tau^2$, which gives $C_\mu = u_\tau^4/k^2$; so $k_P = u_\tau^2/\sqrt{C_\mu}$ and $\varepsilon_P = u_\tau^3/(\kappa y_P)$ are fixed in the wall cell, not solved.
4. Validity $30 < y^+ < 400$. If the first cell is below $y^+ = 11$ (the intersection of the linear and log laws), $u_\tau$ comes from the linear law $u_\tau = (U_P \nu/y_P)^{1/2}$; between 11 and 30 combinations of the two are "sometimes used".

The MIT notes summarise the point: one "can avoid the need to solve k-epsilon right at the wall" and match the wall stress "without resolving the viscous sub-layer" ([[ref-mit-ocw-2-29-turbulence]] L25 s.23).

**Limits.** The near-wall flow is assumed to behave like an equilibrium boundary layer, which "is often not true at all"; wall functions "can be rather crude" where the near-wall flow matters, e.g. external aerodynamics and heat transfer ([[ref-davidson-turbulence-notes]] pp. 155, 157).

**The low-Re alternative.** Resolving the wall needs the model to work in the viscous sublayer. Low-Re k-epsilon models "employ damping functions in one form or another in the sublayer", which are highly nonlinear and can upset convergence, and $\varepsilon$ does not go to zero at a wall, which needs either a nonlinear wall condition or extra terms ([[ref-menter-1992-nasa-tm-103975]] p. 2). k-omega models need no damping functions and have Dirichlet wall conditions ($k = 0$, $\omega$ from $\omega_\text{wall} = 10 \cdot 6 \nu/(\beta_1 \Delta d_1^2)$) ([[ref-menter-1992-nasa-tm-103975]] pp. 2-3, 9; [[ref-nasa-tmr]] `sst.html`), which is one reason SST keeps k-omega near the wall ([[sst-blending-functions]]). Wall-resolved RANS grids are usually built for an average minimum $y^+$ below 1 ([[ref-nasa-tmr]] `flatplate_val_ypluseffect.html`).

**What it adds to its prerequisite.** The [[law-of-the-wall]] describes the profile; a wall function turns it into a boundary condition for a coarse mesh, and adds the equilibrium relations that fix $k$ and $\varepsilon$ in the wall cell.

## Builds on

- [[law-of-the-wall]]: the log and linear laws, $\kappa$, $E$, the $y^+$ ranges.

## Leads to

- [[k-epsilon]], [[spalart-allmaras]] (`auto`, `auto_sa_law`), [[friction-velocity-and-y-plus]] (first-cell targets)

## How rcfd uses it

- Defaults `wall_treatment = smooth`, `wall_function = standard`, `kappa` 0.41, `e_wall` 9.8 ([[doc-case-schema-turbulence]]); k-epsilon walls use wall functions and must be at rest ([[k-epsilon]]).
- F-203 (near-wall catalogue: enhanced and blended treatments, scalable and non-equilibrium wall functions, wall-omega options, roughness) is done; ADR 0015 found SST at $y^+ \sim 1$, Standard at $y^+ \sim 30$ and SST roughness missing, Enhanced not converging, non-equilibrium wall functions not attempted ([[roadmap-turbulence]], [[adr-thread-early-turbulence]]).
- The standard k-epsilon wall function's description is "UNVERIFIED against a published source"; rcfd differs for $y^* < y^*_\text{lam}$ ($G = 0$, $\varepsilon = 2 \nu k/y^2$) ([[doc-reference-data-wave31b-turbulence-wall]]). ADR 0063 traced the k-epsilon wall-function mesh dependence in the $Re_\tau$ 178 channel to "the validity limit of the log-law function with a viscous fall-back branch and a positive feedback of $y^*$ on $k$" ([[adr-thread-early-turbulence]]).
- F-042: scalable wall functions "are not wired to the settings tree" ([[roadmap-turbulence]]); SA wall modes `auto` / `resolved` / `auto_sa_law` ([[spalart-allmaras]]).
- F-043 asks for SST "with low-Re wall treatment"; "the low-Re toggle" is not done ([[roadmap-turbulence]]).

## Related

- [[wall-distance]] — F-040 pairs wall distance with standard wall functions; both live in [[rcfd-turb]].
