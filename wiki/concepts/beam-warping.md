---
type: concept
tags: [structural, fem, beams, warping, thin-walled, buckling]
sources: [adr-thread-structural-beam-warping, adr-thread-structural-beams-shells]
builds_on: [beam-theory, linear-buckling-and-stress-stiffening]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Beam warping (Vlasov torsion) and lateral-torsional buckling

Open thin-walled sections (I, channel) resist twist both by Saint-Venant torsion $G J$ and by restrained warping $E C_w$; a seventh unknown per node, the rate of twist, carries the warping. It governs lateral-torsional buckling (LTB), where the critical moment depends on $C_w$, the shear-centre position, the monosymmetry coefficient $\beta_x$ and the height at which the load acts ([[adr-thread-structural-beam-warping]]).

## How rcfd uses it

- An opt-in 14-unknown Beam2 in `rcfd_fem::beam_warping` (cubic Hermite twist, warping unknowns numbered after the six-unknown DofMap), F-598, ADR 0546: doubly symmetric I under uniform moment within `5.9e-4` at 16 elements ([[adr-thread-structural-beam-warping]]). The six-unknown Beam2 without warping had missed I-section LTB by `-19.2` percent (F-558) ([[roadmap-structural-solver]]).
- Static, modal, pre-stressed, harmonic and transient paths, monosymmetric sections, load height for point and distributed loads, and a negative-pivot count of the pre-stressed solves ([[adr-thread-structural-beam-warping]]).

## Builds on

- [[beam-theory]]: Euler-Bernoulli and Timoshenko beams.
- [[linear-buckling-and-stress-stiffening]]: geometric stiffness and the buckling eigenproblem.

## Related

- [[adr-thread-structural-beam-warping]], [[rcfd-fem]], [[structural-solver]]

## Open questions

- Warping inertia $\rho C_w$ is not modelled (ADR 0548 Q1); the consistent offset mass (F-678) and the case-file key (F-625) are approved and todo at the snapshot ([[adr-thread-structural-beam-warping]]).
