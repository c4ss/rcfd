---
type: concept
tags: [foundation, turbulence, boundary-layer, mesh]
sources: [ref-nasa-tmr, ref-davidson-turbulence-notes, ref-menter-1992-nasa-tm-103975]
builds_on: [shear-stress, viscosity, boundary-layer]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Friction velocity and y+

## In short

The friction velocity $u_\tau = \sqrt{\tau_w/\rho}$ turns the wall shear stress into a velocity. Dividing wall distance by the viscous length $\nu/u_\tau$ gives the wall coordinate $y^+ = y u_\tau/\nu$, and dividing velocity by $u_\tau$ gives $u^+$. In these "wall units" all smooth-wall turbulent boundary layers look alike near the wall. The $y^+$ of the first cell centre tells which near-wall layer the mesh starts in, and so which wall treatment is valid ([[ref-nasa-tmr]], [[ref-davidson-turbulence-notes]]).

## Explanation

**Definitions** ([[ref-nasa-tmr]] `flatplate_val.html`; [[ref-davidson-turbulence-notes]] eqs. 6.16-6.25; [[ref-menter-1992-nasa-tm-103975]] p. 10):
- wall shear stress $\tau_w = \mu (dU/dy)_\text{wall}$;
- friction velocity $u_\tau = \sqrt{\tau_w/\rho_w}$ (the TMR writes $v^*$);
- viscous length $\nu_w/u_\tau$;
- $y^+ = y u_\tau/\nu_w$,  $u^+ = U/u_\tau$.

**Why wall units.** Near the wall the relevant quantities are the wall stress and the viscosity; scaling with them ("inner scaling") makes velocity profiles from different flows and Reynolds numbers collapse ([[ref-davidson-turbulence-notes]] p. 91). In these units: viscous region $y^+ \le 5$ with $u^+ = y^+$; buffer region in between; log region from $y^+ \ge 30$ ([[ref-davidson-turbulence-notes]] p. 91, see [[law-of-the-wall]]).

**First-cell $y^+$ and the wall treatment.**
- Resolving the wall (no wall functions): "Conventional wisdom states that grids for turbulent flow RANS (when integrating to the wall, i.e., without wall functions) should be constructed so that the average minimum $y^+$ is less than 1" ([[ref-nasa-tmr]] `flatplate_val_ypluseffect.html`). On the TMR flat plate, $\omega$-based models were more sensitive than SA: average minimum $y^+$ 0.8 gave a drag "between 2-3% low" compared with $y^+$ 0.1 (same page).
- Wall functions: the log law is valid for $30 < y^+ < 400$; below that the linear law is used, switching at $y^+ = 11$; the buffer region $11 < y^+ < 30$ is covered only by blends ([[ref-davidson-turbulence-notes]] sec. 11.14.1, p. 157; [[wall-functions]]).

**Estimating $y^+$ before solving.** Since $u_\tau = U_\infty \sqrt{C_f/2}$, a skin-friction correlation ([[skin-friction-coefficient]]) and a target $y^+$ give the first-cell height $y = y^+ \nu/u_\tau$ (the cell centre of a wall cell of height $h$ sits at $h/2$).

**What it adds to its prerequisites.** [[shear-stress]] gives $\tau_w$, [[viscosity]] gives $\nu$, and the [[boundary-layer]] provides the region in which they set the scales. Wall units add a universal ruler for the near-wall region; $y^+$ becomes the main mesh-quality number for turbulent wall flows.

## Builds on

- [[shear-stress]]: $\tau_w$.
- [[viscosity]]: $\nu$ in the viscous length.
- [[boundary-layer]]: the near-wall region being scaled.

## Leads to

- [[law-of-the-wall]], [[wall-functions]], [[skin-friction-coefficient]]
- [[k-omega-sst]], [[spalart-allmaras]], [[fan-passage-mesh]]

## How rcfd uses it

- SST's least accurate first-cell range is $y^+$ about 3 to 20 ([[doc-case-schema-turbulence]], [[k-omega-sst]]); F-043's gap includes a "$y^+$ stability spread 3.03 to 3.45% vs 3%" ([[roadmap-turbulence]]).
- SA wall mode by $y^+$: with the wall cell in the buffer layer, default `auto` over-predicts plate friction by +19.9 % at $y^+$ 5.31; opt-in `auto_sa_law` is within +2.3 % to +4.4 % of TMR at $y^+$ 1 to 60 ([[spalart-allmaras]], [[flat-plate]]).
- Fan passage meshes are built for $y^+$ 30 (set-up A) and $y^+$ 1 (set-up B), first cell $h_1 = 2 y^+ \nu/u_\tau$ ([[fan-passage-mesh]]).
- The F-040 channel ran at first-cell $y^+$ 49.4, 32.9 and 24.7, each above `11.6` ([[adr-thread-transient-rans]]).
