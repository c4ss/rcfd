---
type: source
tags: [reference, foundation, rotating, mechanics, non-commercial]
sources: []
origin: https://phys.libretexts.org/Bookshelves/Classical_Mechanics/Variational_Principles_in_Classical_Mechanics_(Cline)/12%3A_Non-inertial_Reference_Frames
licence: CC BY-NC-SA 4.0 (LibreTexts page footer, verified 2026-11-22). Non-commercial, so cite only (maintainer decision 6).
read: 2026-11-22
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Cline, Variational Principles in Classical Mechanics, ch. 12 (non-inertial frames) and ch. 13 (rigid-body rotation)

**Origin:** D. Cline, *Variational Principles in Classical Mechanics*, University of Rochester (River Campus Libraries), LibreTexts edition: chapter 12 "Non-inertial Reference Frames" (URL above) and chapter 13 "Rigid-body Rotation" (`.../13%3A_Rigid-body_Rotation`).
**Kind:** open textbook (HTML). Locators are section and equation numbers as shown on the pages.
**Licence:** CC BY-NC-SA 4.0; cite only.
**Read:** 2026-11-22 by job p6-linear-rotating: secs. 12.3, 12.5, 12.8 and the chapter 12 and 13 contents; secs. 13.11 and 13.17. Sec. 12.7 (centrifugal force) and 13.4 (inertia tensor definition) were not opened.

## Summary

Chapter 12 derives the kinematics of a rotating frame and Newton's law in a non-inertial frame, naming the fictitious forces (translational, Euler-type, Coriolis, centrifugal) and discussing Coriolis effects (rotating table, weather systems, Foucault pendulum). Chapter 13 treats the rigid body: inertia tensor, principal axes, angular momentum, Euler angles and Euler's equations of motion.

## Key points

- **Sec. 12.3, eqs. (12.15)-(12.17):** the basis vectors of a frame rotating with angular velocity `omega` change as `(d e_rot / dt)_fix = omega x e_rot` (12.15); hence for any vector `G`, `(dG/dt)_fix = (dG/dt)_rot + omega x G` (12.16); `d omega / dt` is the same in both frames (12.17).
- **Sec. 12.3, eq. (12.9):** velocity in the non-rotating frame equals the velocity measured in the rotating frame plus `omega x r'`.
- **Sec. 12.5, eqs. (12.27)-(12.28):** from `F = m a_fix` (12.27), the equation of motion in a frame that translates with acceleration `A_fix` and rotates with `omega` is `m a_rot = F - m A_fix - m (d omega/dt) x r' - 2 m omega x v_rot - m omega x (omega x r')` (12.28): the physical force minus the frame-acceleration, angular-acceleration, Coriolis and centrifugal terms.
- **Sec. 12.8, eq. (12.41):** the Coriolis force `F_cor = -2 m omega x v_rot` is "perpendicular to both the axis of rotation and the velocity vector in the rotating frame" (compared with the magnetic Lorentz force); examples: a puck on a rotating table follows a straight path in the inertial frame and a spiral in the rotating frame; a rotating liquid surface is a paraboloid `z = omega^2 rho^2 / (2 g) + const`; low-pressure weather systems.
- **Sec. 13.11, eq. (13.55):** angular momentum `L = {I} . omega` with the inertia tensor `{I}`; `L` and `omega` are "not necessarily colinear", they align only along a principal axis.
- **Sec. 13.17:** Euler's equations: `N = (dL/dt)_space` in the inertial frame and `N = (dL/dt)_body + omega x L` in the body-fixed frame; in principal axes `N_1 = I_1 dw_1/dt - (I_2 - I_3) w_2 w_3` and cyclically. The body frame is used because the inertia tensor is constant there; the torque components must be expressed in body axes, so the orientation must be known.
- **Chapter 13 contents:** 13.4 inertia tensor, 13.6-13.7 principal axes and diagonalisation, 13.8 parallel-axis theorem, 13.13 Euler angles, 13.14 angular velocity in body and space frames.

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[rotating-reference-frame]], [[relative-and-absolute-velocity-formulations]], [[rigid-body-dynamics-six-dof]]
