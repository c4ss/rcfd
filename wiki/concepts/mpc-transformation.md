---
type: concept
tags: [structural, fem, constraints, method]
sources: [adr-thread-structural-modal-buckling-cyclic, adr-thread-structural-beams-shells, adr-thread-structural-dynamics-harmonic]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# The transformation path (linear multi-point constraints)

In [[rcfd-fem]], linear multi-point constraints, rigid sets with a reference point and elastic supports are imposed by a transformation `x = T q + g` in `rcfd_fem::mpc` (F-393, ADR 0296). ADRs call every analysis that works on the reduced unknowns `q` "the transformation path", as opposed to "the prescribed path" of plain displacement constraints ([[adr-thread-structural-modal-buckling-cyclic]]).

## How rcfd uses it

- Each analysis was brought onto the transformation by its own item: modal analysis (F-486), zero modes and the count of free rigid motions (F-488), pre-stressed modes and buckling (F-493) with the zero-shift pivot-ratio refusal of mechanisms (F-495, F-496) ([[adr-thread-structural-modal-buckling-cyclic]]); the transient integrator and prescribed motion (F-490), harmonic static correction (F-491) ([[adr-thread-structural-dynamics-harmonic]]); beams (F-513) and shells (F-522) ([[adr-thread-structural-beams-shells]]); the supernodal factor (F-594) ([[adr-thread-structural-linear-solvers]]).
- The `k = 0` rows of [[cyclic-symmetry]] are rows of the transformation (ADR 0386) ([[adr-thread-structural-modal-buckling-cyclic]]).
- Known miss: three zero modes of a bar on axial springs at 2.07e-6 of the first flexible frequency against 1e-6 (F-486 A1); F-488 measured them at 2.1e-13 sigma against 1e-9 sigma ([[adr-thread-structural-modal-buckling-cyclic]]).

## Related

- [[rcfd-fem]], [[structural-solver]], [[cyclic-symmetry]]
