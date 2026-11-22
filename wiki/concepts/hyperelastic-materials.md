---
type: concept
tags: [structural, fem, material, hyperelastic, finite-strain]
sources: [adr-thread-structural-hyperelastic-and-follower, adr-thread-structural-materials-nonlinear, adr-thread-structural-incompatible-hex8-rule-l]
builds_on: [finite-strain-kinematics, element-locking]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Hyperelastic materials (neo-Hookean, Mooney-Rivlin)

Elastic materials defined by a strain-energy function of the deformation, used for rubber-like and large-strain behaviour. rcfd's form is compressible: `W = c10 (Ibar1 - 3) + c01 (Ibar2 - 3) + (kappa/2)(J - 1)^2` (neo-Hookean when `c01 = 0`), in `rcfd_fem::mat_hyper` (F-425, ADR 0541) ([[adr-thread-structural-hyperelastic-and-follower]]).

## How rcfd uses it

- On the total Lagrangian Newton driver, with the F-bar Hex8 of ADR 0506 as the mean-dilatation element against volumetric locking; the Full Hex8 is `15.6` off on the inflated sphere at `kappa/mu = 1e4`, the mean-dilatation Hex8 `4.1e-3` (ADR 0557) ([[adr-thread-structural-hyperelastic-and-follower]]).
- In the energy-momentum step of the nonlinear transient (ADR 0523, ADR 0544) ([[adr-thread-structural-materials-nonlinear]]) and in the finite-strain incompatible-modes Hex8 chain (the neo-Hookean punch, `mu = 1e6`, `nu = 0.49`) ([[adr-thread-structural-incompatible-hex8-rule-l]]).
- On the linear paths as the linearised material, the isotropic elasticity of the small-strain moduli; on Beam2, Shell4 and Shell3 via `E = 9KG/(3K + G)`, `nu = (3K - 2G)/(2(3K + G))`. Since ADR 0746 (2026-11-19) this is the default for structural elements, approved by the maintainer on 2026-11-18 ([[adr-thread-structural-hyperelastic-and-follower]]).
- Linear paths about a nonlinear hyperelastic state: modes, buckling, harmonic response and flutter (F-646, F-669, F-671, F-676, F-677) ([[adr-thread-structural-hyperelastic-and-follower]]).

## Builds on

- [[finite-strain-kinematics]]: deformation gradient, Green-Lagrange strain, second Piola-Kirchhoff stress.
- [[element-locking]]: shear and volumetric locking and its cures.

## Related

- [[incompatible-modes-hex8]], [[arc-length-continuation]], [[rcfd-fem]], [[structural-solver]]

## Open questions

- Case-file material kind `hyperelastic` and block key `formulation = "bbar"`: approved 2026-11-18 as F-621, todo at the snapshot ([[adr-thread-structural-hyperelastic-and-follower]]).
- The small-strain clause of F-425 remains a known miss (`4.7e-6` and `4.9e-6` against `1e-6`) ([[adr-thread-structural-hyperelastic-and-follower]]).
