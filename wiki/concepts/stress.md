---
type: concept
tags: [foundation, shared-roots, mechanics]
sources: [ref-openstax-university-physics-1, ref-bar-meir-fluid-mechanics, ref-wikipedia-shared-roots]
builds_on: [continuum-hypothesis]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Stress (stress tensor, traction)

## In short

Stress is force per unit area inside a material. Because the force on a small surface depends on which way the surface faces, stress at a point is not one number but a $3 \times 3$ table, the stress tensor: its diagonal entries are normal stresses (push or pull), its off-diagonal entries shear stresses (sliding) ([[ref-openstax-university-physics-1]] sec. 12.3; [[ref-bar-meir-fluid-mechanics]] sec. 8.4).

## Explanation

**First-year picture.** Stress is force over area, in pascals; tensile or compressive stress $F_\perp / A$ acts normal to a surface, shear stress $F_\parallel / A$ parallel to it, and pressure $p = F_\perp / A$ is a scalar that acts equally in all directions (Eq. 12.34, 12.38, 12.42) ([[ref-openstax-university-physics-1]] sec. 12.3).

**Why a tensor.** The force on a surface element depends on both the force and the orientation of the area ([[ref-bar-meir-fluid-mechanics]] sec. 8.4). Writing $\tau_{ij}$ for the stress on a face whose normal is the $i$ direction, acting in the $j$ direction, the nine components form the stress tensor. A small tetrahedron shrunk to a point shows that the stress on any inclined face follows from the three coordinate faces ([[ref-bar-meir-fluid-mechanics]] sec. 8.4). In tensor notation the traction (force per area) on a surface with unit normal $n$ is Cauchy's relation ([[ref-wikipedia-shared-roots]] "Cauchy stress tensor"):

$$T(n) = n \cdot \sigma, \quad \text{i.e.} \quad T_j = \sigma_{ij} n_i$$

**Normal and shear parts.** The normal stress on that surface is $\sigma_n = \sigma_{ij} n_i n_j$ and the shear stress magnitude is $\tau_n = \sqrt{T \cdot T - \sigma_n^2}$ ([[ref-wikipedia-shared-roots]] "Cauchy stress tensor").

**Symmetry.** A moment balance on a shrinking cube gives $\tau_{ij} = \tau_{ji}$ when there is no body torque (e.g. magnetic), so only six components are independent ([[ref-bar-meir-fluid-mechanics]] sec. 8.4, label `dif:eq:tauSymmetry`; [[ref-wikipedia-shared-roots]] "Cauchy stress tensor", from the balance of angular momentum).

**Pressure and deviator.** A fluid at rest carries only $\tau = -P n$, so pressure is the normal stress with a minus sign ([[ref-bar-meir-fluid-mechanics]] sec. 8.4). In general the stress splits into a mean (hydrostatic) part $\pi = (\sigma_{11} + \sigma_{22} + \sigma_{33})/3$ and a traceless deviator $s_{ij} = \sigma_{ij} - \pi \delta_{ij}$ that does the distorting; the mechanical pressure is minus the mean normal stress ([[ref-wikipedia-shared-roots]] "Cauchy stress tensor"; [[ref-bar-meir-fluid-mechanics]] sec. 8.5). The eigenvalues of $\sigma$ are the principal stresses; $\tau_\text{max} = (\sigma_1 - \sigma_3)/2$, and the von Mises stress $\sqrt{3 J_2}$ is built from the deviator ([[ref-wikipedia-shared-roots]] "Cauchy stress tensor").

**What it adds.** First-year physics uses one force on one surface; the tensor gives the force on every surface through a point, which is what a balance of momentum over a volume needs (the surface force term $\nabla \cdot \sigma$).

## Builds on

- [[continuum-hypothesis]]: stress is defined at a point of a continuous body.
- Floor: force, pressure, vectors and matrices, eigenvalues as a definition.

## Leads to

- [[viscosity]] (fluid stress from strain rate) and [[shear-stress]].
- [[navier-stokes-equations]] (momentum balance with $\nabla \cdot \sigma$).
- `linear-elasticity`, `equilibrium-equations`, `finite-strain-kinematics`, [[reynolds-stress]], `fatigue`.

## How rcfd uses it

- The [[structural-solver]] computes stresses and writes them to `.vtu` output; the Studio cannot colour by stress or strain at the snapshot ([[structural-solver]]).
- On the flow side, boundary conditions that act on one velocity component only (`symmetry`, `shear`, `partial_slip`, `marangoni`) are the subject of [[component-diagonal]].
