---
type: concept
tags: [foundation, linear-solvers, coupling, aitken, quasi-newton]
sources: [ref-spenke-2020-interface-quasi-newton, ref-mit-ocw-2-29-linear-systems, ref-yao-2026-ale-chalmers]
builds_on: [newton-raphson-method, stationary-iterative-methods]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Fixed-point iteration and its acceleration (relaxation, Aitken, quasi-Newton)

## In short

Many coupled problems are solved by applying a map $H$ repeatedly, $x^{(k+1)} = H(x^{(k)})$, until $x$ stops changing. The plain iteration may converge slowly or blow up. Relaxation damps each step; Aitken's method chooses the damping automatically from the last two steps; quasi-Newton methods learn an approximate Jacobian from the history and take Newton-like steps.

## Explanation

- **Fixed point and residual.** A solution satisfies $x = H(x)$; the residual $R(x) = H(x) - x$ is zero there, so finding a fixed point is a root-finding problem ([[ref-spenke-2020-interface-quasi-newton]], sec. 3.3, eqs. 12-14). In FSI, $H$ is one Dirichlet-Neumann coupling pass: fluid solve with the interface deformation, then structural solve with the fluid stresses (same section).
- **Linear analogue.** A stationary linear iteration $x^{(k)} = B x^{(k-1)} + c$ is a fixed-point iteration of an affine map; it converges for every start exactly when the spectral radius of $B$ is below one ([[ref-mit-ocw-2-29-linear-systems]], lecture 7). Fixed-point (Picard) iteration of a nonlinear equation $x = g(x)$ is introduced in the same course (lecture 5 review of roots of nonlinear equations).
- **Constant relaxation.** Blend the new and old iterate, $x^{(k+1)} = \omega \tilde{x}^{(k)} + (1 - \omega)\, x^{(k)}$ with $\omega < 1$; $\omega$ must be small enough for every time step, which "often comes at a very high price in terms of efficiency" ([[ref-spenke-2020-interface-quasi-newton]], sec. 3.3, eq. 15).
- **Aitken's dynamic relaxation.** Recompute $\omega$ each iteration from the two most recent residuals $R(x^{(k)})$ and $R(x^{(k-1)})$ (eq. 16); simple, and "in many cases" a significant speed-up without losing stability (same section).
- **Interface quasi-Newton (IQN-ILS).** Newton's method on $R$ needs the inverse Jacobian, which involves derivatives of both solvers and is not available. IQN-ILS approximates it from the pairs of residual differences and deformation differences collected during the step (a multi-dimensional secant), closes the underdetermined fit by a least-squares (minimum-norm) condition solved by QR, and starts each step with one relaxation step. Reusing pairs from earlier time steps helps but risks rank deficiency and contradictory information ([[ref-spenke-2020-interface-quasi-newton]], secs. 4.1-4.2).
- **Not every repetition converges.** "Simply repeating an unrelaxed exchange need not converge"; strong coupling uses relaxation, interface quasi-Newton acceleration or Robin conditions ([[ref-yao-2026-ale-chalmers]], sec. 7.7).

## Builds on

- [[newton-raphson-method]]: the step that quasi-Newton methods approximate.
- [[stationary-iterative-methods]]: the linear fixed-point iteration and its spectral-radius condition; relaxation as in SOR.

## Leads to

- [[partitioned-fluid-structure-interaction]], [[strong-fsi-coupling]], [[cosimulation-protocol]]
- [[under-relaxation]] (the same damping idea inside SIMPLE)

## How rcfd uses it

- **Strong FSI step:** `rcfd_fv::fsi::strong_step` (F-251, ADR 0250) with Aitken relaxation and the safe start $2M/(2M + m_a)$; with `max_iterations = 1` it equals loose coupling bit for bit ([[strong-fsi-coupling]]).
- **Co-simulation:** `rcfd_fv::cosim::coupled_step` offers fixed, Aitken or least-squares quasi-Newton updates (ADR 0261); Aitken stays the default (ADR 0261 Q1); Aitken took 8.8 iterations a step on the model problem; on a flow-coupled elastic bar Aitken and least-squares converge to $10^{-6}$ in 3 iterations on a matching wall, least-squares better on a 3:2 non-matching wall (F-401, ADR 0434) ([[cosimulation-protocol]], [[strong-fsi-coupling]]).
