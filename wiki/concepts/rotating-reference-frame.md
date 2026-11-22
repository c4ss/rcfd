---
type: concept
tags: [foundation, rotating, mechanics]
sources: [ref-cline-variational-mechanics, ref-liu-hasegawa-2026-vpm-mrf-sliding]
builds_on: []
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Rotating reference frame (Coriolis and centrifugal forces)

## In short

An observer who turns with a fan blade sees the blade at rest, which simplifies the geometry, but pays for it: Newton's law in a rotating frame gains extra "fictitious" forces, the centrifugal force pushing outward and the Coriolis force deflecting anything that moves relative to the frame.

## Explanation

- **Rates of change in two frames.** The unit vectors of a frame rotating with angular velocity `omega` change in the fixed frame as `(d e / dt)_fix = omega x e` (Cline eq. 12.15). Hence for any vector `G`: `(dG/dt)_fix = (dG/dt)_rot + omega x G` (eq. 12.16). The angular velocity itself has the same rate in both frames (eq. 12.17) ([[ref-cline-variational-mechanics]], sec. 12.3).
- **Velocity.** Applied to the position, the velocity seen from the fixed frame is the velocity relative to the rotating frame plus the frame's own motion at that point: `v_fix = v_rot + omega x r` (eq. 12.9) ([[ref-cline-variational-mechanics]], sec. 12.3). For a turbomachine this is the split of the absolute velocity into relative velocity and blade speed.
- **Acceleration and Newton's law.** Applying eq. (12.16) a second time gives, for a frame whose origin accelerates with `A` and which rotates with `omega`, the equation of motion (eq. 12.28) ([[ref-cline-variational-mechanics]], sec. 12.5): `m a_rot = F - m A - m (d omega/dt) x r - 2 m omega x v_rot - m omega x (omega x r)`. Here `F` is the real force; `-m A` comes from the translating origin; `-m (d omega/dt) x r` from a changing rotation rate; `-2 m omega x v_rot` is the Coriolis force; `-m omega x (omega x r)` is the centrifugal force.
- **Centrifugal force.** `-m omega x (omega x r)` depends only on position, not on the velocity. Example: the free surface of a liquid spinning in a bucket is the paraboloid `z = omega^2 rho^2 / (2 g) + const` ([[ref-cline-variational-mechanics]], sec. 12.8).
- **Coriolis force.** `F_cor = -2 m omega x v_rot` (eq. 12.41) acts only on bodies moving in the rotating frame and is "perpendicular to both the axis of rotation and the velocity vector in the rotating frame", like the magnetic part of the Lorentz force; a puck sliding straight across a frictionless rotating table traces a spiral as seen by a rotating observer, and the force organises the circulation around weather lows ([[ref-cline-variational-mechanics]], sec. 12.8).
- **What it adds to Newton's laws.** Nothing new physically: the same motion described from a turning viewpoint. Its value in CFD is that a rotor can be held fixed in its own frame, with only the inertia terms of the frame added to the momentum equation, so no mesh has to move ([[ref-liu-hasegawa-2026-vpm-mrf-sliding]], sec. 1).

## Builds on

- Floor: Newton's second law, vectors and the cross product (shared-root level; no map node).

## Leads to

- [[relative-and-absolute-velocity-formulations]], [[mrf]], [[rigid-body-dynamics-six-dof]]
- [[campbell-diagram]] and `rotordynamics-gyroscopic-effects` (the Coriolis term in rotating structures)

## How rcfd uses it

- **MRF and the rotating-frame source:** an MRF zone solves for the absolute velocity with the relative flux in convection and a lagged `Omega x u` source (ADR 0008); smoke solves on the bladed fan sector use the relative-frame `rotating_frame` source ([[mrf]]).
- **Rotating structures:** free vibration in the rotating frame `M u_tt + 2 Omega C u_t + K_eff u = 0`, `C` skew (the Coriolis/gyroscopic term); stress stiffening and spin softening (F-396, F-499) ([[campbell-diagram]]).
- **Turbulence in the rotating frame:** ADR 0075 fixed the SST relative wall velocity, a Galilean-invariance error from `2.88e-1` to `3.65e-10` of `U_cl` at y+ 30 ([[k-omega-sst]]).
