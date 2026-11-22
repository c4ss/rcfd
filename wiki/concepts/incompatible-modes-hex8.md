---
type: concept
tags: [structural, fem, element, hex8, locking, finite-strain]
sources: [adr-thread-structural-incompatible-hex8-rule-l, adr-thread-structural-verification-elements]
builds_on: [element-locking, finite-strain-kinematics]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Incompatible-modes (enhanced-strain) Hex8

An eight-node hexahedron whose strain (at finite strain, its deformation gradient) is enriched by internal "incompatible" displacement modes that are condensed out per element, to remove the shear and volumetric locking of the plain trilinear brick. rcfd uses the nine Wilson modes with Taylor's centre-Jacobian correction, and at finite strain the enhanced deformation gradient of Simo and Armero, `F = I + Grad u + sum_k alpha_k (x) G_k` ([[adr-thread-structural-incompatible-hex8-rule-l]]).

## How rcfd uses it

- **Small strain:** opt-in per element block beside B-bar (F-390, ADR 0493); on the remaining linear paths (thermal, buckling, damped modes, harmonic, patch recovery) by F-588, ADR 0525 ([[adr-thread-structural-verification-elements]]).
- **Finite strain:** module `rcfd_fem::nl_incompatible` in the Newton driver; the modes solve `R_a = 0` by a local Newton iteration at every element evaluation (F-596, ADR 0608, done) ([[adr-thread-structural-incompatible-hex8-rule-l]]).
- **Known weakness:** a spurious zero (hourglass) eigenvalue in compression, at stretch `lambda_s = 0.60797` on the registered patch (ADR 0614). The opt-in `IncompatibleStabilisation` (Off by default) offers `RestStress`, `Deviatoric` and `LinearStress` ("rule L"); ADR 0630 shows that a coefficient without the bulk modulus cannot stabilise confined compression near incompressibility ([[adr-thread-structural-incompatible-hex8-rule-l]]).
- **Local mode update:** the default starts the local iteration at `alpha = 0`; the opt-in warm start from the committed modes passes 11 of 11 hard set-ups where the default passes 7 (ADR 0725). Switching the default is open for the maintainer (ADR 0725, ADR 0727, ADR 0729) ([[adr-thread-structural-incompatible-hex8-rule-l]]).
- Compared throughout against the F-bar Hex8 of ADR 0506 as the locking-free reference ([[adr-thread-structural-materials-nonlinear]]).

## Builds on

- [[element-locking]]: shear and volumetric locking and its cures.
- [[finite-strain-kinematics]]: deformation gradient, Green-Lagrange strain, second Piola-Kirchhoff stress.

## Related

- [[adr-thread-structural-incompatible-hex8-rule-l]], [[arc-length-continuation]], [[hyperelastic-materials]], [[rcfd-fem]]

## Open questions

- The default local mode update and whether any stabilisation rule becomes a default ([[adr-thread-structural-incompatible-hex8-rule-l]]).
