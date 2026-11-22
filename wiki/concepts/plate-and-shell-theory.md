---
type: concept
tags: [foundation, structural, fem]
sources: [ref-bathe-mit-2-094-fea-notes, ref-roylance-mit-3-11-mechanics-of-materials, adr-thread-structural-beams-shells, adr-thread-structural-creep-and-temperature]
builds_on: [beam-theory, element-locking]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Plate and shell theory (Kirchhoff, Reissner-Mindlin, MITC elements)

## In short

Plates and shells are thin in one direction, like a fan blade or a panel. Their deformation is described by the mid-surface: its transverse deflection and the rotations of the normal fibres. Kirchhoff theory ties the rotations to the slopes (no transverse shear), Reissner-Mindlin theory keeps them independent, as Timoshenko does for beams. Practical shell elements use Reissner-Mindlin kinematics with mixed interpolation of the shear strains to avoid locking ([[ref-bathe-mit-2-094-fea-notes]], lectures 20-21).

## Explanation

**Reissner-Mindlin kinematics.** $w(x, y)$ is the mid-surface deflection; material particles at any height $z$ move vertically with it, and the in-plane displacements come from the fibre rotations: $u = -z\beta_x(x, y)$, $v = -z\beta_y(x, y)$ (Bathe, eq. 21.8). Bending strains are $z$ times curvatures (e.g. $\gamma_{xy} = -z(\partial\beta_x/\partial y + \partial\beta_y/\partial x)$), and the transverse shear strains are $$\gamma_{xz} = \frac{\partial w}{\partial x} - \beta_x, \qquad \gamma_{yz} = \frac{\partial w}{\partial y} - \beta_y$$ (21.11). With plane-stress elasticity through the thickness, the stiffness of a flat element is a bending part plus a shear part with a shear correction factor $k$ (21.21) ([[ref-bathe-mit-2-094-fea-notes]]).

**Kirchhoff theory.** Setting the transverse shears to zero forces $\beta_x = \partial w/\partial x$; the strains then contain second derivatives of $w$, so the interpolation needs continuous slopes across elements, which "results in difficulties to develop a good element". Interpolating rotations independently (Reissner-Mindlin) avoids this ([[ref-bathe-mit-2-094-fea-notes]], lecture 21).

**Shear locking and MITC.** A displacement-based Reissner-Mindlin element "locks in shear" when thin, like the Timoshenko beam element. The MITC (mixed interpolation of tensorial components) elements interpolate the transverse shear strains from values tied at edge points (21.22-21.24); "with this mixed interpolation, the element works" ([[ref-bathe-mit-2-094-fea-notes]], lecture 21; [[element-locking]]).

**Shells.** For flat facet elements the plate-bending and plane-stress (membrane) stiffnesses are superposed; curved shells need curved elements (references: Dvorkin and Bathe 1984 on the four-node shell; Chapelle and Bathe, *The Finite Element Analysis of Shells*) ([[ref-bathe-mit-2-094-fea-notes]], lecture 21). The rotation about the shell normal ("drilling") is not discussed in the sources read.

**Laminates.** Roylance has a module on laminated composite plates (classical lamination theory); it was not read in this job. The ply-level anisotropy it builds on is on [[linear-elasticity]].

## Builds on

- [[beam-theory]]: Timoshenko kinematics generalised to two dimensions.
- [[element-locking]]: why thin displacement-based plate elements fail and how assumed strains fix them.

## Leads to

- [[structural-solver]], [[adr-thread-structural-beams-shells]], [[arc-length-continuation]] (corotated shells)

## How rcfd uses it

- Shell4: bilinear membrane, Reissner-Mindlin bending with MITC4 shear, a drilling penalty $\alpha G t (\theta_z - \omega)^2$ with $\alpha = 10^{-3}$, rigid offsets for warped nodes (F-392, ADR 0432, done); Shell3: CST membrane with MITC3 and Lyly-Stenberg-Vihinen stabilisation (F-419, ADR 0454) ([[adr-thread-structural-beams-shells]]).
- Laminated Shell4 by classical lamination theory with ply stresses and failure criteria (F-428, ADR 0496, done); corotational facets in the Newton driver (F-578) ([[adr-thread-structural-beams-shells]]).
- Through-thickness temperature gradients on Shell4, Shell3 and layup Shell4 (F-527, F-687) ([[adr-thread-structural-creep-and-temperature]]).
- The hinged cylindrical roof limit load with corotated shells, 2222.4 N against 2226.3 N of Sze, Liu and Lo (2004) (F-610) ([[structural-solver]]).
