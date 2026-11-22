---
type: concept
tags: [foundation, mechanics, six-dof, rigid-body]
sources: [ref-cline-variational-mechanics]
builds_on: [rotating-reference-frame]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Rigid-body dynamics (six degrees of freedom)

## In short

A rigid body can translate in three directions and rotate about three axes: six degrees of freedom. Its centre of mass obeys Newton's second law; its rotation obeys Euler's equations, which are written in axes fixed to the body because there the body's resistance to rotation (the inertia tensor) does not change.

## Explanation

- **Translation.** In an inertial frame `F = m a` for the centre of mass ([[ref-cline-variational-mechanics]], eq. 12.27).
- **Angular momentum and the inertia tensor.** `L = {I} . omega` (eq. 13.55), with the 3 x 3 inertia tensor `{I}`; `L` and `omega` are "not necessarily colinear" and align only when `omega` lies along a principal axis, where the tensor is diagonal ([[ref-cline-variational-mechanics]], sec. 13.11). Principal axes, diagonalisation and the parallel-axis theorem are secs. 13.6-13.8.
- **Euler's equations.** In the inertial (space) frame the torque is `N = (dL/dt)_space`; transforming the derivative to the rotating body frame with the rule of [[rotating-reference-frame]] gives `N = (dL/dt)_body + omega x L`; in principal axes `N_1 = I_1 dw_1/dt - (I_2 - I_3) w_2 w_3` and cyclically ([[ref-cline-variational-mechanics]], sec. 13.17). The body frame is used because the inertia tensor is constant there; the torques must then be expressed in body axes, so the body's orientation must be tracked alongside (same section).
- **Orientation.** Orientation can be described by Euler angles (sec. 13.13) and the angular velocity expressed in body or space axes (sec. 13.14). Quaternion orientation, springs and dampers are in the map's scope for this node but were not covered by the source read.

## Builds on

- [[rotating-reference-frame]]: the rule `(dG/dt)_fix = (dG/dt)_rot + omega x G` that turns `dL/dt` into Euler's equations.
- Floor: Newton's laws.

## Leads to

- [[added-mass]], [[dynamic-mesh]] (six-DOF body), [[strong-fsi-coupling]]

## How rcfd uses it

- **Six-DOF body in rcfd:** mass, inertia, locks, springs, dampers, drag; RK4 or Newmark time integration; quaternion orientation; loose explicit coupling, unstable above an added-mass ratio of 2 (ADR 0250). F-244: a falling block within 0.55 percent of the lubrication terminal velocity; a spring piston within 0.13 percent ([[dynamic-mesh]]).
- **Strong coupling** of the body with the flow: [[strong-fsi-coupling]], [[adr-thread-six-dof-fsi-cosimulation]].
