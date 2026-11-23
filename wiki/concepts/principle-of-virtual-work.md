---
type: concept
tags: [foundation, structural, fem]
sources: [ref-bathe-mit-2-092-fea-notes, ref-bathe-mit-2-094-fea-notes, ref-roylance-mit-3-11-mechanics-of-materials]
builds_on: [equilibrium-equations, divergence-theorem]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Principle of virtual work (weak form)

## In short

Instead of demanding force balance at every point, ask that for every small imagined ("virtual") displacement compatible with the supports, the work done by the internal stresses equals the work done by the applied loads. This statement is equivalent to the equilibrium equations, needs less smoothness of the solution, and is the starting point of the finite element method ([[ref-bathe-mit-2-092-fea-notes]], lecture 4).

## Explanation

**One-dimensional derivation.** For a bar, $EA\,u'' + f^B = 0$ with $u(0) = 0$ and end load $R$. Multiply by an arbitrary virtual displacement $\bar u(x)$, continuous and zero where $u$ is prescribed, integrate over the length and integrate by parts. The boundary term at $x = 0$ vanishes because $\bar u = 0$ there, the one at $x = L$ becomes $R\,\bar u_L$, and $$\int_0^L \bar\epsilon_{xx}\,EA\,\epsilon_{xx}\,dx = \int_0^L \bar u\,f^B\,dx + R\,\bar u_L$$ (Bathe, lecture 4, eq. A'): internal virtual work equals external virtual work. The virtual displacement is set to zero on $S_u$ because the reaction forces there are not known ([[ref-bathe-mit-2-092-fea-notes]]).

**Three dimensions.** With virtual strains $\bar{\boldsymbol\epsilon}$ derived from $\bar{\mathbf u}$, $$\int_V \bar{\boldsymbol\epsilon}^T\mathbf C\,\boldsymbol\epsilon\,dV = \int_V \bar{\mathbf u}^T\mathbf f^B\,dV + \int_{S_f}\bar{\mathbf u}^{S_f\,T}\mathbf f^{S_f}\,dS$$ (Bathe, lecture 4, eq. B). "For any compatible virtual displacement field imposed on the body in its state of equilibrium, the total internal virtual work is equal to the total external virtual work", and this variational formulation is equivalent to the differential one ([[ref-bathe-mit-2-092-fea-notes]]).

**Why it is weaker.** The differential form needs $u''$ to exist; the virtual-work form needs only $u$ (and $\bar u$) continuous, since only first derivatives appear ([[ref-bathe-mit-2-092-fea-notes]], lecture 4). That is what lets piecewise polynomials with kinks between elements be admissible.

**Abstract form.** Written with a bilinear form, find $u$ in the space $V$ of admissible functions such that $a(u, v) = (f, v)$ for all $v \in V$, where $a(w, w) > 0$ for $w \ne 0$ is twice the strain energy ([[ref-bathe-mit-2-094-fea-notes]], lecture 7). The finite element method restricts this to a finite-dimensional subspace ([[finite-element-method-basics]]).

**Stiffness from virtual work.** Roylance reaches the element stiffness by the same argument: a virtual nodal displacement increases the strain energy by an amount that must equal the work of the nodal forces, which gives $k_{ij} = \int_V \mathbf B_i^T\mathbf D\mathbf B_j\,dV$ ([[ref-roylance-mit-3-11-mechanics-of-materials]], "Finite Element Analysis", eqs. 10-12).

## Builds on

- [[equilibrium-equations]]: the strong form being weakened.
- [[divergence-theorem]]: the integration by parts in 3D.

## Leads to

- [[finite-element-method-basics]], [[nonlinear-fem-newton]] (virtual work on the reference configuration), [[beam-theory]] and [[plate-and-shell-theory]] (virtual work of beams and plates)

## How rcfd uses it

- The structural solver is a finite element solver built on this weak form; the wiki pages on it do not name the variational principle explicitly ([[structural-solver]]).
