---
type: concept
tags: [foundation, structural, dynamics]
sources: [ref-bathe-mit-2-092-fea-notes, ref-nasa-sp-8050-structural-vibration-prediction, adr-thread-structural-dynamics-harmonic, adr-thread-structural-basic-tier]
builds_on: [finite-element-method-basics]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Structural dynamics (mass, damping and stiffness matrices)

## In short

When loads change quickly, a structure's inertia matters. Newton's second law, discretised by finite elements, gives a system of second-order ordinary differential equations $\mathbf M\ddot{\mathbf U} + \mathbf C\dot{\mathbf U} + \mathbf K\mathbf U = \mathbf R(t)$: mass times acceleration, damping times velocity and stiffness times displacement balance the applied load ([[ref-bathe-mit-2-092-fea-notes]], lectures 7, 15).

## Explanation

**Inertia as a body force.** In the virtual-work equation the body force is replaced by $\mathbf f^B - \rho\,\ddot{\mathbf u}$, and accelerations are interpolated like displacements; this adds $$\mathbf M\ddot{\mathbf U}, \qquad \mathbf M = \sum_m\int_{V^{(m)}}\rho^{(m)}\,\mathbf H^{(m)T}\mathbf H^{(m)}\,dV^{(m)}$$ (Bathe, lecture 7, eqs. 4-5), the consistent mass matrix. Initial displacements and velocities are needed ([[ref-bathe-mit-2-092-fea-notes]]).

**Lumped mass.** Placing a share of the element mass at each node (a quarter for the 4-node element) gives a diagonal mass matrix; with it the explicit central-difference method needs no factorisation ([[ref-bathe-mit-2-092-fea-notes]], lecture 16).

**Damping.** Real structural damping comes from material damping, friction at joints (dominant in bolted and riveted structures) and acoustic radiation; viscous damping is assumed because it fits linear analysis, and finding the right damping per mode is "a major problem" usually settled by tests of similar structures ([[ref-nasa-sp-8050-structural-vibration-prediction]], sec. 2.1.2.2). A common model is Rayleigh damping $$\mathbf C = \alpha\mathbf M + \beta\mathbf K$$ (Bathe, lecture 18): it is diagonal in the modes, $\alpha + \beta\omega_i^2 = 2\omega_i\xi_i$ (14), so $\alpha$ and $\beta$ are fitted to damping ratios at two frequencies (15), and the implied ratio at others is $\xi_i = \alpha/(2\omega_i) + \beta\omega_i/2$ (16): mass-proportional damping dominates at low frequency, stiffness-proportional at high frequency ([[ref-bathe-mit-2-092-fea-notes]]).

**Two solution routes.** Mode superposition transforms to the modal coordinates of the undamped problem and solves decoupled single-degree-of-freedom equations ([[modal-analysis]]); direct integration steps the coupled equations in time ([[newmark-and-generalised-alpha]]) ([[ref-bathe-mit-2-092-fea-notes]], lecture 15). NASA's review lists the mode-displacement, mode-acceleration and admittance / impedance methods for the uncoupled equations ([[ref-nasa-sp-8050-structural-vibration-prediction]]).

**What it adds.** Statics solves $\mathbf K\mathbf U = \mathbf R$ once; dynamics adds $\mathbf M$ and $\mathbf C$ and a time (or frequency) dimension, and with it natural frequencies, resonance and time-step stability.

## Builds on

- [[finite-element-method-basics]]: shape functions and assembly, reused for $\mathbf M$ and $\mathbf C$.
- Floor: Newton's second law; ordinary differential equations.

## Leads to

- [[modal-analysis]], [[newmark-and-generalised-alpha]], [[harmonic-and-random-response]], [[rotordynamics-gyroscopic-effects]], [[substructuring-and-craig-bampton]]

## How rcfd uses it

- Consistent mass is in the basic tier ([[structural-solver]]); modal analysis is part of it (ADR 0270) ([[adr-thread-structural-basic-tier]]).
- Linear dynamics: Newmark and generalised-alpha with Rayleigh damping and load tables (F-398), harmonic response, damped and complex modes, random vibration and substructuring ([[adr-thread-structural-dynamics-harmonic]]).
- `DynamicSolid` is the dynamic co-simulation participant ([[cosimulation-protocol]]; [[structural-solver]]).
