---
type: concept
tags: [foundation, shared-roots, mathematics]
sources: [ref-openstax-calculus, ref-bar-meir-fluid-mechanics]
builds_on: []
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Divergence theorem (Gauss's theorem)

## In short

The divergence theorem says that what flows out through the closed surface of a region equals the sum, over the region's volume, of the field's divergence. It is the step that turns a balance written for a whole volume into a balance of fluxes through its faces, which is exactly what a finite-volume cell does ([[ref-openstax-calculus]] Vol. 3 sec. 6.8).

## Explanation

**The operators.** For a vector field `F = <P, Q, R>` ([[ref-openstax-calculus]] Vol. 3 sec. 6.5):
- divergence `div F = dP/dx + dQ/dy + dR/dz` (Eq. 6.16), a scalar: positive at a source (net outflow), negative at a sink;
- curl `curl F = (R_y - Q_z) i + (P_z - R_x) j + (Q_x - P_y) k` (Eq. 6.17), a vector whose direction is the local axis of rotation and whose size is the rotation rate;
- gradient `grad f` of a scalar; the identities `div(curl F) = 0` (Theorem 6.16) and `curl(grad f) = 0`;
- the Laplacian `laplacian f = div(grad f) = f_xx + f_yy + f_zz`.

**The theorem.** For a solid `E` with piecewise smooth closed boundary `S`, normal pointing outward (Theorem 6.20, Eq. 6.24):

`triple integral over E of (div F) dV = surface integral over S of F . dS`

The left side adds up sources inside; the right side is the net outflow through the boundary. It is Green's theorem in flux form "in one higher dimension" ([[ref-openstax-calculus]] Vol. 3 sec. 6.8).

**Divergence as flux per volume.** Shrinking a small ball `B_r` around a point `P` gives `div F(P) = lim_(r->0) (1/V(B_r)) surface integral of F . dS`: divergence is "the net rate of outward flux of the fluid per unit volume" ([[ref-openstax-calculus]] Vol. 3 sec. 6.8). If `F = v` is a fluid velocity, the surface integral of `v . dS` is the volume flow rate across the surface (Example 6.79).

**What it adds.** First-year calculus integrates along a line; this theorem links a volume integral to a boundary integral in 3D. It is what lets a conservation law be written either for a finite volume (fluxes through faces) or at a point (a divergence): Bar-Meir uses it to go from the control-volume mass balance to `d rho/dt + div(rho U) = 0` ([[ref-bar-meir-fluid-mechanics]] sec. 8.2, book Appendix A.1.2).

## Builds on

Floor only: partial derivatives, multiple and surface integrals (first-year calculus).

## Leads to

- [[conservation-law]]: integral and differential balances are equivalent through this theorem.
- [[finite-volume-method]]: a cell balance is the theorem applied to one cell, with the surface integral split into face fluxes.
- [[equilibrium-equations]] and [[principle-of-virtual-work]]: the weak form of solid mechanics uses the same integration by parts.
- [[geometric-conservation-law]]: a moving cell's volume change equals the volume swept by its faces.

## How rcfd uses it

- On a moving mesh rcfd checks that a cell's volume change equals the sum of its faces' swept volumes to round-off (checks G1 to G3); this is the face-flux form of a volume balance ([[dynamic-mesh]]).
- No rcfd page states the theorem itself; it sits under every face-flux balance of the finite-volume solvers, e.g. [[simple]] and [[piso]], via [[finite-volume-method]].
