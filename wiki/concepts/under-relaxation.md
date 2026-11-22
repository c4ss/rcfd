---
type: concept
tags: [foundation, pressure-velocity, iteration]
sources: [ref-jasak-1996-phd-thesis, ref-klaij-vuik-2013-simple-preconditioners]
builds_on: [stationary-iterative-methods]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Under-relaxation

## In short

In a steady segregated solver each outer iteration would, left alone, jump too far and the iteration would oscillate or diverge. Under-relaxation takes only part of the step: the new value is a blend of the freshly computed one and the previous iterate. It can be applied after the solve (explicit) or built into the matrix (implicit). At convergence the blend changes nothing, so the converged answer is unaffected, but how fast (and whether) the iteration gets there depends strongly on the factors ([[ref-jasak-1996-phd-thesis]] 3.5; [[ref-klaij-vuik-2013-simple-preconditioners]] 3.4, 4.4).

## Explanation

**Explicit.** $x_\text{new} = x_\text{old} + \alpha (x_\text{computed} - x_\text{old})$ with $0 < \alpha \le 1$. In SIMPLE the pressure is relaxed this way, $p_\text{new} = p_\text{old} + \alpha_p (p_P - p_\text{old})$ ([[ref-jasak-1996-phd-thesis]] 3.8.2.2); the velocity prediction can be relaxed the same way, $u^{k+1} = (1 - w_u)\, u^k + w_u u^*$ ([[ref-klaij-vuik-2013-simple-preconditioners]] 3.1.2).

**Implicit.** Add the same diagonal term to both sides of the momentum equation, once with the new and once with the previous-iteration value: $\left(Q + \dfrac{1 - w_i}{w_i} \operatorname{diag}(Q)\right) u^{k+1} = f + \dfrac{1 - w_i}{w_i} \operatorname{diag}(Q)\, u^k$ ([[ref-klaij-vuik-2013-simple-preconditioners]] 3.4, Eq. 25). At convergence $u^{k+1} = u^k$ and the added terms cancel, so the converged equation is unchanged; Jasak describes the same artificial term with factor $0 < \alpha \le 1$ for steady problems (3.5, Eqn. 3.96; the OCR of the formula is not legible) ([[ref-jasak-1996-phd-thesis]] 3.5).

**Recommended values.** For SIMPLE, Jasak quotes Peric's recommendation $\alpha_p = 0.2$, $\alpha_u = 0.8$ ([[ref-jasak-1996-phd-thesis]] 3.8.2.2). Klaij and Vuik find SIMPLE as a solver "highly sensitive to the relaxation parameters, notably $w_p$"; on their backward-facing step "The only choice that gives convergence is $w_p = 0.2$", while SIMPLER "does not diverge for any of the parameter choices" ([[ref-klaij-vuik-2013-simple-preconditioners]] 4.4, Table I). Used as preconditioners instead of solvers, the SIMPLE-type methods need far fewer nonlinear iterations ([[ref-klaij-vuik-2013-simple-preconditioners]] 4.4, Table II).

**Side effect on the face flux.** Implicit relaxation adds to $\operatorname{diag}(Q)$, and through the Rhie-Chow (PWIM) coefficients this can make the converged solution depend on the relaxation factor unless that contribution is left out ([[ref-klaij-vuik-2013-simple-preconditioners]] 2.5); see [[rhie-chow-interpolation]].

**Relation to iterative methods.** Explicit relaxation is the same blending as damped Jacobi or SOR on a linear system, applied to the outer nonlinear iteration; see [[stationary-iterative-methods]].

## Builds on

- [[stationary-iterative-methods]]: relaxation of an iteration by a factor.

## Leads to

- rcfd pages: [[simple]], [[piso]] (PIMPLE relaxation), [[block-coupled-solver]]

## How rcfd uses it

- SIMPLE 0.7/0.3 ($\alpha_u$/$\alpha_p$) and plain SIMPLEC with $\alpha_u$ 0.95 on the lid-driven cavity Re 1000 (660 and 442 iterations); at $Re_h = 400$ on the backward-facing step vanLeer with SIMPLE 0.7/0.3 limit-cycles and "the working recipe is vanLeer + SIMPLEC alpha_u 0.9" ([[simple]]).
- The block-coupled solver uses `solver.alpha_u` as implicit momentum relaxation (0.95 to 0.98 on the cavity) and no pressure relaxation ([[doc-case-schema-coupled]]).
- In relaxed PIMPLE the averaged component-diagonal error "can have either sign and depends on `alpha_u`, the time step and the flow-direction spacing" ([[component-diagonal]]); on the cylinder O-grid PIMPLE `n_outer = 2` or `4` with $\alpha$ 0.9 diverged ([[doc-investigation-job48t-cylinder-spokes]]).
