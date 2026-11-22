---
type: concept
tags: [foundation, structural]
sources: [ref-roylance-mit-3-11-mechanics-of-materials, ref-bathe-mit-2-094-fea-notes, adr-thread-structural-beams-shells, adr-thread-structural-beam-warping]
builds_on: [linear-elasticity]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Beam theory (Euler-Bernoulli and Timoshenko)

## In short

A beam is long compared with its cross-section, so its deformation can be described by the bending of its axis alone. Euler-Bernoulli theory assumes cross-sections stay plane and perpendicular to the axis; Timoshenko theory lets them rotate independently, which adds shear deformation and matters for short or thick beams ([[ref-roylance-mit-3-11-mechanics-of-materials]], "Stresses in Beams"; [[ref-bathe-mit-2-094-fea-notes]], lectures 19-20).

## Explanation

**Euler-Bernoulli bending.** With $v(x)$ the deflection and $y$ measured from the neutral axis ([[ref-roylance-mit-3-11-mechanics-of-materials]], "Stresses in Beams"):

1. plane transverse sections stay plane and rotate by about $v_{,x}$;
2. kinematics: $\epsilon_x = -y\,v_{,xx}$, with $v_{,xx}$ the curvature;
3. Hooke's law: $\sigma_x = E\epsilon_x$, linear in $y$;
4. zero axial force puts the neutral axis at the centroid of the section; the moment balance gives $$M = E\,v_{,xx}\int_A y^2\,dA = EI\,v_{,xx}$$ (eq. 4) and $$\sigma_x = -\frac{My}{I}$$ (eq. 7).

The stress is independent of the material; shear stresses are often negligible compared with the bending stresses when the length-to-height ratio is large ([[ref-roylance-mit-3-11-mechanics-of-materials]]).

**Timoshenko (shear-flexible) beam.** A fibre originally normal to the axis moves with the axis and rotates by $\beta$, without changing length, but $\beta$ is no longer tied to the slope $w'$; the difference $\gamma = w' - \beta$ is the transverse shear strain. Virtual work: $$EI\int_0^L \beta'\,\delta\beta'\,dx + (Ak)G\int_0^L (w' - \beta)(\delta w' - \delta\beta)\,dx = \int_0^L p\,\delta w\,dx$$ (Bathe, eqs. 19.25, 20.1), with a shear area factor $k$. For a thin beam the shear term acts as a penalty driving $\gamma \to 0$, the Euler-Bernoulli limit ([[ref-bathe-mit-2-094-fea-notes]]).

**Elements.** Interpolating $w$ and $\beta$ separately with the same functions (20.8) leads to shear locking of the 2-node element (about 200 elements for 10 percent error); an assumed constant shear strain equal to its value at the element middle (20.26) cures it ([[ref-bathe-mit-2-094-fea-notes]], lecture 20; see [[element-locking]]).

**Torsion and warping.** Roylance's "Shear and Torsion" module (circular shafts) was not read; Saint-Venant torsion of general sections, the shear centre and warping are not covered by the sources read in this job. See [[beam-warping]] for what rcfd records.

## Builds on

- [[linear-elasticity]]: Hooke's law and the shear modulus in the shear term.

## Leads to

- [[plate-and-shell-theory]] (the same kinematics in two dimensions), [[beam-warping]], [[element-locking]], [[linear-buckling-and-stress-stiffening]]

## How rcfd uses it

- Beam2: six unknowns per node, nodal moments, "exact prismatic Timoshenko stiffness" (F-391, ADR 0417, done); line and gravity loads, VTK line output, use on the [[mpc-transformation]] ([[adr-thread-structural-beams-shells]]).
- The six-unknown Beam2 missed I-section lateral-torsional buckling by `-19.2` percent (F-558), which led to the 14-unknown warping Beam2 (F-598, ADR 0546) ([[beam-warping]]; [[adr-thread-structural-beam-warping]]).
- Flexible rotors are built from Beam2 shafts on bearings (F-416) ([[rotordynamics-gyroscopic-effects]]).
