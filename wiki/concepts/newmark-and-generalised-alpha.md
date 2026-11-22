---
type: concept
tags: [foundation, structural, dynamics, time-integration]
sources: [ref-bathe-mit-2-092-fea-notes, ref-zimmermann-2019-arxiv-1710-02547, adr-thread-structural-dynamics-harmonic, adr-thread-structural-materials-nonlinear, doc-cosimulation-protocol]
builds_on: [structural-dynamics, implicit-time-integration]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Newmark and generalised-alpha time integration

## In short

To step $\mathbf M\ddot{\mathbf U} + \mathbf C\dot{\mathbf U} + \mathbf K\mathbf U = \mathbf R(t)$ in time, the Newmark family assumes how the acceleration varies over a step, which links the new displacement and velocity to the new acceleration; the equation is then enforced at the end of the step (implicit). The trapezoidal rule ("constant average acceleration") is unconditionally stable. Generalised-alpha adds controllable damping of the highest, poorly resolved frequencies ([[ref-bathe-mit-2-092-fea-notes]] lectures 16, 18; [[ref-zimmermann-2019-arxiv-1710-02547]] App. C.1).

## Explanation

**Explicit versus implicit.** The central-difference method uses equilibrium at time $t$ to get the displacement at $t + \Delta t$; with diagonal mass and no damping it needs no factorisation but is stable only for $\Delta t \le \Delta t_{cr} = T_n/\pi$, $T_n = 2\pi/\omega_n$ the shortest period of the mesh (Bathe, lecture 18, eq. 8). Implicit methods use equilibrium at $t + \Delta t$ ([[ref-bathe-mit-2-092-fea-notes]], lectures 16, 18).

**Newmark's formulae.** $$\mathbf x_{n+1} = \mathbf x_n + \Delta t\,\dot{\mathbf x}_n + \Delta t^2\Big[\big(\tfrac12 - \beta\big)\ddot{\mathbf x}_n + \beta\,\ddot{\mathbf x}_{n+1}\Big]$$ $$\dot{\mathbf x}_{n+1} = \dot{\mathbf x}_n + \Delta t\Big[(1 - \gamma)\,\ddot{\mathbf x}_n + \gamma\,\ddot{\mathbf x}_{n+1}\Big]$$ (Zimmermann et al., eq. 174) ([[ref-zimmermann-2019-arxiv-1710-02547]]). Substituting them into equilibrium at $t + \Delta t$ gives one linear solve per step, $(c_1\mathbf M + c_2\mathbf C + \mathbf K)\,{}^{t+\Delta t}\mathbf U = {}^{t+\Delta t}\hat{\mathbf R}$ (Bathe, lecture 18, eq. 12).

**Trapezoidal rule.** The Newmark member with constant average acceleration over the step (in (174) this is $\beta = \tfrac14$, $\gamma = \tfrac12$: both formulas then use the mean of the end accelerations) is unconditionally stable: no step-size limit for stability, the step is chosen for accuracy alone ([[ref-bathe-mit-2-092-fea-notes]], lecture 18). Bathe calls it "a special case of Newmark method" (lecture 16). Stability and accuracy are separate requirements: "the result will not be accurate unless a sufficiently small time step has been used" (lecture 18).

**Generalised-alpha.** The equations are satisfied at intermediate times: displacements and velocities at $n + \alpha_f$, accelerations at $n + \alpha_m$, $\mathbf x_{n+\alpha_f} = (1-\alpha_f)\mathbf x_n + \alpha_f\mathbf x_{n+1}$ and $\ddot{\mathbf x}_{n+\alpha_m} = (1-\alpha_m)\ddot{\mathbf x}_n + \alpha_m\ddot{\mathbf x}_{n+1}$ (eq. 123). All four parameters follow from one, the spectral radius $\rho_\infty \in [0, 1]$ of the amplification matrix as $\Delta t \to \infty$: $$\alpha_f = \frac{1}{1+\rho_\infty},\quad \alpha_m = \frac{2-\rho_\infty}{1+\rho_\infty},\quad \gamma = \tfrac12 + \alpha_m - \alpha_f,\quad \beta = \tfrac14\big(1 + \alpha_m - \alpha_f\big)^2$$ (eq. 124, after Chung and Hulbert 1993). The parameters "control numerical dissipation"; $\rho_\infty$ is the amplitude factor left per step on the infinitely stiff (unresolved) modes, so a smaller value removes them faster. The paper uses $\rho_\infty = 0.5$ ([[ref-zimmermann-2019-arxiv-1710-02547]]). Substituting $\rho_\infty = 1$ into (124) gives $\alpha_f = \alpha_m = \tfrac12$, $\gamma = \tfrac12$, $\beta = \tfrac14$, the trapezoidal values (arithmetic on the cited formulas, not a statement of the source).

**Not covered by the sources read.** Energy-momentum schemes and the accuracy analysis of the methods (period elongation, amplitude decay) were not in the sources read.

## Builds on

- [[structural-dynamics]]: the semi-discrete equations of motion.
- [[implicit-time-integration]]: implicit stepping, stability against accuracy.

## Leads to

- [[cosimulation-protocol]] (`DynamicSolid`), [[adr-thread-structural-dynamics-harmonic]], [[nonlinear-fem-newton]] (Newton inside each implicit step)

## How rcfd uses it

- `transient::Integrator` with Newmark and generalised-alpha (F-398, ADR 0389), partial on clause A8, the generalised-alpha decay at `omega dt = 1e3` (ADR 0389 Q1 open) ([[adr-thread-structural-dynamics-harmonic]]).
- Nonlinear transient: Newton inside Newmark and generalised-alpha (F-423, ADR 0479, partial: a free spinning bar's momentum check missed by "the trapezoidal rule's nonlinear instability"); the Simo-Tarnow energy-momentum step (F-570, ADR 0514) ([[adr-thread-structural-materials-nonlinear]]).
- The dynamic co-simulation participant `DynamicSolid` ([[doc-cosimulation-protocol]]; [[cosimulation-protocol]]).
