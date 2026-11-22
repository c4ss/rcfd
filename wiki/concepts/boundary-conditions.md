---
type: concept
tags: [foundation, discretisation]
sources: [ref-jasak-1996-phd-thesis, ref-mit-ocw-2-29-numerics, ref-klaij-vuik-2013-simple-preconditioners]
builds_on: []
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Boundary conditions

## In short

A differential equation has many solutions; the boundary conditions pick the one that belongs to a given problem. Numerically there are two basic kinds: fix the value on the boundary (Dirichlet) or fix its normal gradient (Neumann); a mix of both is a Robin condition. Physical boundaries of a flow (inlet, outlet, wall, symmetry) are built from these for each variable ([[ref-jasak-1996-phd-thesis]]).

## Explanation

**Numerical conditions (3.3.3.1).** A fixed-value (Dirichlet) condition prescribes $\phi_b$ on the boundary face: the convection flux uses it directly and the diffusion flux uses the gradient between $\phi_b$ and the cell value. A fixed-gradient (Neumann) condition prescribes $g_b = n \cdot \nabla \phi$ at the face: the face value is extrapolated as $\phi_b = \phi_P + \lvert d \rvert g_b$ and the diffusion flux is $\lvert S \rvert g_b$; this is first order because the vector $d$ from the cell centre does not in general meet the face at its centre ([[ref-jasak-1996-phd-thesis]] 3.3.3.1). Combinations of value and gradient ("radiation boundaries, for example") are built from the two ([[ref-jasak-1996-phd-thesis]] 3.3.3).

**Physical conditions for incompressible flow (3.3.3.2).** Each physical boundary sets one condition per variable ([[ref-jasak-1996-phd-thesis]] 3.3.3.2):
- **Inlet:** velocity fixed; pressure zero gradient.
- **Outlet:** either the velocities are projected from the interior and scaled to satisfy overall continuity with zero pressure gradient (unstable if inflow happens there), or the pressure is fixed and the velocity has zero gradient.
- **Symmetry plane:** the normal gradient of each variable is zero; for a vector, the normal component is zero and the tangential components have zero normal gradient.
- **No-slip wall:** velocity equal to the wall velocity; pressure zero gradient.

**Wall and symmetry stresses (OCW L19 s.20).** A no-slip wall is a Dirichlet condition on velocity; at a symmetry plane the shear stress is zero and the normal stress is not ([[ref-mit-ocw-2-29-numerics]] L19 s.20).

**Pressure and its correction.** On collocated grids the boundary pressure is extrapolated from the interior; where the mass flux is specified, the pressure-correction gradient is zero; if every boundary has a Neumann pressure condition, the pressure is defined only up to a constant and must be fixed at one point or by its mean ([[ref-mit-ocw-2-29-numerics]] L19 s.21-22). In SIMPLE the correction $p'$ is zero where $p$ has a Dirichlet condition and has zero normal derivative where $p$ has a Neumann condition ([[ref-klaij-vuik-2013-simple-preconditioners]] 3.1.1).

## Builds on

- Floor: PDE basics (a boundary-value problem needs conditions on the boundary to have a unique solution).

## Leads to

- [[finite-volume-method]], [[pressure-correction-equation]], [[momentum-matrix-and-h-operator]]
- rcfd pages: [[component-diagonal]], [[spalart-allmaras]]

## How rcfd uses it

- The `corrected` momentum diagonal of [[component-diagonal]] answers a boundary-condition effect: where a condition acts on one velocity component only (`symmetry`, `shear`, `partial_slip`, `marangoni`), the `averaged` diagonal leaves a spurious sink on the tangential velocity that "acts like a partial no-slip wall" ([[component-diagonal]]).
- On the NACA 0012 a `zero_gradient` far field let $\tilde{\nu}$ grow to $6.0 \times 10^4\, \nu$; the far-field role `freestream` was part of the fix ([[spalart-allmaras]]).
