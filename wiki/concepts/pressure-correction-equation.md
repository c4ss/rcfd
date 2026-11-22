---
type: concept
tags: [foundation, pressure-velocity]
sources: [ref-jasak-1996-phd-thesis, ref-klaij-vuik-2013-simple-preconditioners, ref-mit-ocw-2-29-numerics]
builds_on: [pressure-velocity-coupling, momentum-matrix-and-h-operator]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Pressure (correction) equation

## In short

Take the momentum equation solved for the cell velocity, `U = H/a_P - grad p / a_P`, interpolate it to the faces and insert it into the continuity equation. The result is a Poisson-type equation for the pressure whose right-hand side is the divergence of `H/a_P`. Solving it and correcting the face fluxes and cell velocities with the new pressure gradient gives a velocity that satisfies continuity ([[ref-jasak-1996-phd-thesis]] 3.8.1).

## Explanation

**Derivation.** With `U_P = H(U)/a_P - (grad p)/a_P` (Eqn. 3.139) and the face velocity `U_f = (H(U)/a_P)_f - (1/a_P)_f (grad p)_f` (Eqn. 3.140), the discrete continuity equation `sum_f S . U_f = 0` (Eqn. 3.138) gives `div((1/a_P) grad p) = div(H(U)/a_P)` (Eqn. 3.141) ([[ref-jasak-1996-phd-thesis]] 3.8.1). The left side is discretised as a Laplacian with the face-normal pressure gradient, the right side as a sum of face fluxes of `H/a_P` (Eqns. 3.142-3.143).

**Flux correction.** The face flux `F = S . (H(U)/a_P)_f - S . (1/a_P)_f (grad p)_f` (Eqn. 3.144) uses the same face-normal pressure gradient as the pressure equation; when the pressure equation is satisfied these fluxes are conservative ([[ref-jasak-1996-phd-thesis]] 3.8.1). Cell velocities are corrected explicitly with Eqn. 3.139.

**Correction form (SIMPLE).** In incremental form: predict `u*` from momentum with the old pressure, `Q u* = f - G p^k`; write corrections `u' = -Q^-1 G p'`; continuity then gives `(C - D Q^-1 G) p' = g - D u* - C p^k`, whose matrix "is the Schur complement" of the block system. SIMPLE approximates `Q^-1` by `diag(Q)^-1` ([[ref-klaij-vuik-2013-simple-preconditioners]] 3.1.1, Eqs. 13-19). Boundary conditions: `p'` is zero where `p` is prescribed and has zero normal gradient where the normal pressure gradient is prescribed ([[ref-klaij-vuik-2013-simple-preconditioners]] 3.1.1); with Neumann pressure conditions everywhere the pressure must be fixed at one point or by its mean ([[ref-mit-ocw-2-29-numerics]] L19 s.22).

**Variants by what is neglected.** The velocity correction `u'` of the neighbour cells appears in the exact correction equation; SIMPLE "Neglects contributions of u' in the pressure equation"; SIMPLEC approximates `u'` in the pressure equation "as a function of p' (better)"; SIMPLER and PISO iterate to obtain `u'` ([[ref-mit-ocw-2-29-numerics]] L18 s.18-19). SIMPLER adds a pressure-prediction equation before the momentum step ([[ref-klaij-vuik-2013-simple-preconditioners]] 3.1.3, Eqs. 20-23). In PISO the explicit velocity correction neglects the neighbour corrections, so `H(U)` is updated and the pressure equation solved again, with the `H` coefficients kept fixed ([[ref-jasak-1996-phd-thesis]] 3.8.2.1).

**Non-orthogonal meshes.** The Laplacian of the pressure equation has a non-orthogonal part that is usually treated by non-orthogonal correctors, solving the pressure equation several times ([[ref-jasak-1996-phd-thesis]] 3.5); see [[non-orthogonal-and-skewness-correction]].

## Builds on

- [[pressure-velocity-coupling]]: why a pressure equation is needed.
- [[momentum-matrix-and-h-operator]]: `a_P` and `H(U)`.

## Leads to

- [[rhie-chow-interpolation]], [[projection-method]], [[block-coupled-solver]]
- rcfd pages: [[simple]], [[piso]]

## How rcfd uses it

- SIMPLE/SIMPLEC in `SimpleSolver`; SIMPLEC "uses a consistent correction that allows weaker relaxation" ([[simple]]).
- PISO/PIMPLE in `PimpleSolver`: "per time step a momentum predictor and several pressure correctors" ([[piso]]); consistent (SIMPLEC-type) correctors are opt-in through `[solver.consistent_correctors]` (F-301, ADR 0223) ([[piso]]).
- On the warped mesh W the explicit (deferred-correction) non-orthogonal step of the pressure equation diverges; `implicit_non_orth_pressure` solves it implicitly ([[simple]]).
