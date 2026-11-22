---
type: concept
tags: [foundation, shared-roots, mechanics]
sources: [ref-openstax-university-physics-1, ref-wikipedia-shared-roots, ref-bar-meir-fluid-mechanics, ref-openstax-calculus]
builds_on: [continuum-hypothesis]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Strain and strain rate

## In short

Strain measures how much a piece of material is deformed (stretched, squeezed or sheared) relative to its size; strain rate measures how fast it is being deformed. Solids are described by strain, fluids by strain rate, which comes from the velocity gradient: its symmetric part is the strain-rate tensor and its antisymmetric part is rotation, linked to vorticity ([[ref-openstax-university-physics-1]] sec. 12.3; [[ref-wikipedia-shared-roots]] "Strain-rate tensor").

## Explanation

**Strain in a solid (first-year).** Strain is a dimensionless fractional change: tensile strain `dL / L0` (Eq. 12.35), bulk strain `dV / V0` (Eq. 12.37) and shear strain `dx / L0`, the sideways shift over the transverse distance (Eq. 12.41). In the linear range stress = modulus x strain (Eq. 12.33), with Young's modulus `Y`, bulk modulus `B = -dp/(dV/V0)` and shear modulus `S` ([[ref-openstax-university-physics-1]] sec. 12.3). The tensor form for solids (displacement gradient, small-strain tensor) was not covered by the sources read in this job; see `linear-elasticity` and `finite-strain-kinematics`.

**Velocity gradient in a fluid.** Near a point `p`, the velocity is `v(p + r) ~ v(p) + J r` with `J_ij = dv_i/dx_j`, the velocity gradient `L = grad v`, unit 1/time ([[ref-wikipedia-shared-roots]] "Strain-rate tensor"). It splits into two parts:

- strain-rate tensor `E = (L + L^T)/2`, components `E_ij = (dv_i/dx_j + dv_j/dx_i)/2`: stretching and shearing;
- spin (rotation) tensor `W = (L - L^T)/2`: rigid rotation that does not change distances between fluid particles, so it causes no deformation. The rotation rate is `omega = (1/2) curl v`, and `curl v` is the vorticity ([[ref-wikipedia-shared-roots]] "Strain-rate tensor"; curl as in [[ref-openstax-calculus]] Vol. 3 Eq. 6.17).

**Volume change and shear.** `tr E = div v` is the rate of volume change per unit volume; `E` splits into a spherical part `(1/3)(div v) delta_ij` and a traceless deviatoric (pure shear) part ([[ref-wikipedia-shared-roots]] "Strain-rate tensor"). For an incompressible flow `div v = 0`, so `E` is pure shear ([[incompressible-flow]]).

**Shear strain rate in the engineering derivation.** In Bar-Meir's derivation the shear strain rate in the `xy` plane is the sum of the two cross gradients `dU_y/dx + dU_x/dy` (sec. 8.5, Eq. 16), which is `2 E_xy`; for a simple shear flow between plates it reduces to `dU/dy` (sec. 1.4) ([[ref-bar-meir-fluid-mechanics]]).

**What it adds.** First-year strain is one number for one bar; the tensor measures deformation in every direction at a point and separates deformation from rotation. That separation is what makes a constitutive law possible: stress responds to `E`, not to `W` ([[viscosity]]).

## Builds on

- [[continuum-hypothesis]]: strain and velocity gradient are point fields.
- Floor: partial derivatives, matrices (transpose, trace), vectors.

## Leads to

- [[viscosity]] (Newtonian stress is linear in `E`) and [[shear-stress]].
- [[boussinesq-eddy-viscosity]] (eddy-viscosity models use `S`), `linear-elasticity`, `finite-strain-kinematics`.

## How rcfd uses it

- The SST limiter on the rcfd page is `nu_t = a1 k / max(a1 omega, S F2)`, with `S` a strain-rate magnitude ([[k-omega-sst]]); the TMR page used for it defines the vorticity magnitude `Omega = sqrt(2 W_ij W_ij)` from the rotation tensor ([[ref-nasa-tmr]]).
- The [[structural-solver]] handles centrifugal and thermal strain ([[structural-solver]]).
