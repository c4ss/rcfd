---
type: concept
tags: [foundation, structural, stability, buckling]
sources: [ref-felippa-nfem, ref-bathe-mit-2-094-fea-notes, adr-thread-structural-modal-buckling-cyclic, adr-thread-structural-rotordynamics, adr-thread-structural-beams-shells, adr-thread-structural-beam-warping]
builds_on: [modal-analysis, finite-strain-kinematics]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Linear buckling and stress stiffening (geometric stiffness)

## In short

A stressed structure has a different stiffness from an unstressed one: tension stiffens a guitar string sideways, compression softens a column until, at a critical load, it buckles. In a finite element model this effect is the geometric (initial-stress) stiffness, proportional to the current stresses. Linear buckling finds the load factor at which material plus geometric stiffness becomes singular, an eigenvalue problem ([[ref-felippa-nfem]] ch. 24; [[ref-bathe-mit-2-094-fea-notes]] lecture 13).

## Explanation

**Where the geometric stiffness comes from.** Linearising virtual work on the reference configuration splits the tangent into $\mathbf K_L$, from the material law, and $$\mathbf K_{NL} = \int_{{}^0V}\mathbf B_{NL}^T\,{}^t\mathbf S\,\mathbf B_{NL}\;d\,{}^0V$$ (Bathe, eq. 13.13), which contains the current second Piola-Kirchhoff stress ${}^t\mathbf S$. For a bar along $x$ under axial stress, $\mathbf K_L$ has no stiffness for the transverse displacement, but $\mathbf K_{NL}$ does, proportional to the stress (lecture 13, truss example): this is stress stiffening ([[ref-bathe-mit-2-094-fea-notes]]; [[finite-strain-kinematics]]).

**Stability test.** For conservative loading the tangent stiffness is the Hessian of the potential energy; the equilibrium is stable while all its eigenvalues are positive, and the transition to instability passes through a zero eigenvalue, $\det\mathbf K(\lambda_{cr}) = 0$, $\mathbf K\mathbf z = 0$ with $\mathbf z$ the buckling mode (Felippa, eqs. 24.3-24.4) ([[ref-felippa-nfem]]; [[stability-of-equilibrium]]).

**Linearized prebuckling (LPB).** Neglect the geometry change before buckling. Split $\mathbf K = \mathbf K_M + \mathbf K_G$; take the material stiffness at the reference state, $\mathbf K_M = \mathbf K_0$, and a geometric stiffness linear in the load factor, $\mathbf K_G = \lambda\mathbf K_1$, with $\mathbf K_1$ from a reference-load stress state. The stability condition becomes the eigenproblem $$(\mathbf K_0 + \lambda\,\mathbf K_1)\,\mathbf z = 0$$ (Felippa, eq. 24.9), a generalised symmetric eigenproblem whose eigenvalues are real when $\mathbf K_0$ is positive definite; their sign depends on the physics and the sign convention of $\lambda$. Under the LPB assumptions the critical points found are bifurcation points, not limit points ([[ref-felippa-nfem]], ch. 24).

**When it fails.** LPB assumes conservative loads, small prebuckling deformations and negligible imperfections. It suits columns and frames; it is poor for arches, shells, very thin members and imperfection-sensitive structures such as the axially compressed cylinder, whose real critical load is strongly reduced by imperfections (Felippa, ch. 25, sec. 25.4). There a nonlinear path analysis is needed ([[nonlinear-fem-newton]], [[arc-length-continuation]]) ([[ref-felippa-nfem]]).

**Pre-stressed modes.** The same stress-dependent stiffness enters vibration: a pre-stressed structure's frequencies come from $(\mathbf K_L + \mathbf K_{NL})\boldsymbol\phi = \omega^2\mathbf M\boldsymbol\phi$, e.g. the stiffening of a rotating blade by its centrifugal stress. That rotating case also has spin softening, which was not described in the sources read; the rcfd pages describe it ([[campbell-diagram]]).

## Builds on

- [[modal-analysis]]: generalised symmetric eigenproblems and their solution.
- [[finite-strain-kinematics]]: the second Piola-Kirchhoff stress and the nonlinear strain increment that produce $\mathbf K_{NL}$.

## Leads to

- [[stability-of-equilibrium]], [[beam-warping]] (lateral-torsional buckling), [[campbell-diagram]] (stress stiffening of rotating parts)

## How rcfd uses it

- Geometric (initial-stress) stiffness, pre-stressed modes and buckling, also on the transformation path with a zero-shift pivot-ratio refusal of mechanisms (F-493, F-495, F-496); buckling F-395 (ADR 0390) partial: the second factor misses by `2.0135e-2` against `2e-2` ([[adr-thread-structural-modal-buckling-cyclic]]).
- Stress stiffening of beams and shells (F-420), pre-stress on mixed models (F-557), beam Wagner terms (F-558) ([[adr-thread-structural-beams-shells]]); lateral-torsional buckling with the warping Beam2 within `5.9e-4` (F-598) ([[adr-thread-structural-beam-warping]]; [[beam-warping]]).
- Rotating parts: stress stiffening by the centrifugal pre-stress and spin softening (F-396, ADR 0403) ([[adr-thread-structural-rotordynamics]]).
