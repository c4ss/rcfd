---
type: concept
tags: [foundation, shared-roots, mechanics]
sources: [ref-openstax-university-physics-1, ref-wikipedia-shared-roots, ref-bar-meir-fluid-mechanics, ref-openstax-calculus, ref-roylance-mit-3-11-mechanics-of-materials]
builds_on: [continuum-hypothesis]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Strain and strain rate

## In short

Strain measures how much a piece of material is deformed (stretched, squeezed or sheared) relative to its size; strain rate measures how fast it is being deformed. Solids are described by strain, fluids by strain rate, which comes from the velocity gradient: its symmetric part is the strain-rate tensor and its antisymmetric part is rotation, linked to vorticity ([[ref-openstax-university-physics-1]] sec. 12.3; [[ref-wikipedia-shared-roots]] "Strain-rate tensor").

## Explanation

**Strain in a solid (first-year).** Strain is a dimensionless fractional change: tensile strain $\Delta L / L_0$ (Eq. 12.35), bulk strain $\Delta V / V_0$ (Eq. 12.37) and shear strain $\Delta x / L_0$, the sideways shift over the transverse distance (Eq. 12.41). In the linear range stress = modulus x strain (Eq. 12.33), with Young's modulus $Y$, bulk modulus $B = -\Delta p/(\Delta V/V_0)$ and shear modulus $S$ ([[ref-openstax-university-physics-1]] sec. 12.3). ~~The tensor form for solids (displacement gradient, small-strain tensor) was not covered by the sources read in this job; see `linear-elasticity` and `finite-strain-kinematics`.~~ The tensor form is in the next paragraph (added by job p6-structural, [[ref-roylance-mit-3-11-mechanics-of-materials]]).

**Small-strain tensor in a solid.** With displacement $\mathbf u = (u, v, w)$, normal strain is the stretch gradient $\epsilon_x = \partial u/\partial x$ (Roylance, "The Kinematic Equations", eq. 1) and engineering shear strain the sum of the cross gradients $\gamma_{xy} = \partial v/\partial x + \partial u/\partial y$ (eq. 3). The symmetric tensor $$\epsilon_{ij} = 	frac12\,(u_{i,j} + u_{j,i})$$ (eq. 4) has six independent components, with $\epsilon_{xy} = \gamma_{xy}/2$; in the vector form of finite element codes $oldsymbol\epsilon = \mathbf L\mathbf u$ (eq. 8), and the trace $\epsilon_{kk}$ is the volume change per unit volume (eq. 10). It is the displacement analogue of the strain-rate tensor $E$ below, and like it drops the rotation part of the gradient. It holds only for small gradients; the Lagrangian strain keeps the quadratic terms (eqs. 11-13) ([[ref-roylance-mit-3-11-mechanics-of-materials]]; [[linear-elasticity]], [[finite-strain-kinematics]]).

**Velocity gradient in a fluid.** Near a point $p$, the velocity is $v(p + r) \approx v(p) + J r$ with $J_{ij} = \partial v_i/\partial x_j$, the velocity gradient $L = \nabla v$, unit 1/time ([[ref-wikipedia-shared-roots]] "Strain-rate tensor"). It splits into two parts:

- strain-rate tensor $E = (L + L^T)/2$, components $E_{ij} = (\partial v_i/\partial x_j + \partial v_j/\partial x_i)/2$: stretching and shearing;
- spin (rotation) tensor $W = (L - L^T)/2$: rigid rotation that does not change distances between fluid particles, so it causes no deformation. The rotation rate is $\omega = \tfrac{1}{2} \nabla \times v$, and $\nabla \times v$ is the vorticity ([[ref-wikipedia-shared-roots]] "Strain-rate tensor"; curl as in [[ref-openstax-calculus]] Vol. 3 Eq. 6.17).

**Volume change and shear.** $\operatorname{tr} E = \nabla \cdot v$ is the rate of volume change per unit volume; $E$ splits into a spherical part $\tfrac{1}{3}(\nabla \cdot v)\, \delta_{ij}$ and a traceless deviatoric (pure shear) part ([[ref-wikipedia-shared-roots]] "Strain-rate tensor"). For an incompressible flow $\nabla \cdot v = 0$, so $E$ is pure shear ([[incompressible-flow]]).

**Shear strain rate in the engineering derivation.** In Bar-Meir's derivation the shear strain rate in the $xy$ plane is the sum of the two cross gradients $\partial U_y/\partial x + \partial U_x/\partial y$ (sec. 8.5, Eq. 16), which is $2 E_{xy}$; for a simple shear flow between plates it reduces to $dU/dy$ (sec. 1.4) ([[ref-bar-meir-fluid-mechanics]]).

**What it adds.** First-year strain is one number for one bar; the tensor measures deformation in every direction at a point and separates deformation from rotation. That separation is what makes a constitutive law possible: stress responds to $E$, not to $W$ ([[viscosity]]).

## Builds on

- [[continuum-hypothesis]]: strain and velocity gradient are point fields.
- Floor: partial derivatives, matrices (transpose, trace), vectors.

## Leads to

- [[viscosity]] (Newtonian stress is linear in $E$) and [[shear-stress]].
- [[boussinesq-eddy-viscosity]] (eddy-viscosity models use $S$), [[linear-elasticity]], [[finite-strain-kinematics]].

## How rcfd uses it

- The SST limiter on the rcfd page is $\nu_t = \dfrac{a_1 k}{\max(a_1 \omega,\, S F_2)}$, with $S$ a strain-rate magnitude ([[k-omega-sst]]); the TMR page used for it defines the vorticity magnitude $\Omega = \sqrt{2 W_{ij} W_{ij}}$ from the rotation tensor ([[ref-nasa-tmr]]).
- The [[structural-solver]] handles centrifugal and thermal strain ([[structural-solver]]).
