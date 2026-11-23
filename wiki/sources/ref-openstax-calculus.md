---
type: source
tags: [reference, foundation, shared-roots, mathematics, cite-only]
sources: []
origin: https://openstax.org/books/calculus-volume-3/pages/6-8-the-divergence-theorem
licence: CC BY-NC-SA 4.0 (page footer, both volumes); the pages also state the books "may not be used to train large language models" without OpenStax's written permission
read: 2026-11-22
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# OpenStax Calculus, Volumes 1 and 3 (sections read)

**Origin:** Gilbert Strang, Edwin "Jed" Herman et al., *Calculus Volume 1* and *Calculus Volume 3*, OpenStax, Rice University. Pages read: https://openstax.org/books/calculus-volume-3/pages/6-5-divergence-and-curl, https://openstax.org/books/calculus-volume-3/pages/6-8-the-divergence-theorem, https://openstax.org/books/calculus-volume-1/pages/4-9-newtons-method
**Kind:** open textbook (web edition).
**Licence:** CC BY-NC-SA 4.0 as stated in each page footer; OpenStax name and logo excluded. Cite only, never copied into `raw/` (maintainer decision 6, [[ingest-plan]]).
**Read:** 2026-11-22, by job p6-roots.

## Summary

First-year vector calculus and root finding: the definitions of divergence, curl, gradient identities and the Laplacian (Vol. 3 sec. 6.5), the divergence theorem with divergence read as outward flux per unit volume (Vol. 3 sec. 6.8), and Newton's method with its failure modes (Vol. 1 sec. 4.9).

## Key points by section

### Vol. 3, sec. 6.5 "Divergence and Curl"
- Eq. 6.16: $\nabla \cdot F = P_x + Q_y + R_z$ for $F = \langle P, Q, R \rangle$; divergence is a scalar. Positive divergence is a source (net outflow), negative a sink.
- Theorem 6.14 and 6.15 (Divergence Test): a source-free field has zero divergence; on a simply connected domain $\nabla \cdot F = 0$ if and only if $F$ is source free.
- Eq. 6.17: $\nabla \times F = (R_y - Q_z)\, \mathbf{i} + (P_z - R_x)\, \mathbf{j} + (Q_x - P_y)\, \mathbf{k}$; its direction is the axis of rotation and its magnitude how fast particles rotate (paddlewheel picture, Fig. 6.54).
- Theorem 6.16: $\nabla \cdot (\nabla \times F) = 0$; text after Theorem 6.17: $\nabla \times (\nabla f) = 0$; Theorem 6.17-6.18: conservative fields have zero curl, and on simply connected domains zero curl implies conservative.
- Laplacian (text after Example 6.56): $\nabla^2 f = \nabla \cdot (\nabla f) = f_{xx} + f_{yy} + f_{zz}$; $\nabla^2 f = 0$ is Laplace's equation.

### Vol. 3, sec. 6.8 "The Divergence Theorem"
- Theorem 6.20, Eq. 6.24: for a piecewise smooth closed surface $S$ enclosing a solid $E$, oriented outward, $\displaystyle\iiint_E \nabla \cdot F \, dV = \iint_S F \cdot dS$.
- The theorem is Green's theorem in flux form "in one higher dimension"; the proof is omitted; an informal argument uses small boxes whose shared-face fluxes cancel.
- Shrinking a ball $B_r$ around a point $P$: $\nabla \cdot F(P) = \displaystyle\lim_{r \to 0} \dfrac{1}{V(B_r)} \iint_{S_r} F \cdot dS$, i.e. "the net rate of outward flux of the fluid per unit volume".
- Example 6.79: the flow rate of a fluid across $S$ is the surface integral of $v \cdot dS$. The section says the theorem is used to derive heat-flow and mass-conservation models, without a worked derivation.
- Gauss's law (Theorem 6.21): flux of the electric field through a closed surface is $q/\varepsilon_0$ with the charge inside, 0 outside. Example 6.81 prints $4.518 \times 10^9$ V m; our recomputation $0.004 / (8.854 \times 10^{-12})$ gives about $4.518 \times 10^8$, so the printed exponent looks like a typo (not used on any wiki page).

### Vol. 1, sec. 4.9 "Newton's Method"
- Eq. 4.8: $x_n = x_{n-1} - f(x_{n-1}) / f'(x_{n-1})$, from setting the tangent line $y = f(x_0) + f'(x_0)(x - x_0)$ to zero and repeating.
- Example 4.47: for $f(x) = x^2 - 2$, $x_0 = 2$, the iterates are $1.5$, $1.416666667$, ... $x_5 = 1.414213562$ (Fig. 4.79).
- "Failures of Newton's Method": $f'(x_n) = 0$ stops the process; a start far from the root can converge to a different root (Fig. 4.80); iterates can oscillate, e.g. $f(x) = x^3 - 2x + 2$ with $x_0 = 0$ alternates between 0 and 1 (Example 4.48).
- When it works the approximations approach the root "very quickly"; the rate is left to numerical analysis texts.

## Contradictions with the wiki

None found.

## Pages this source informs

[[divergence-theorem]], [[conservation-law]], [[newton-raphson-method]]
