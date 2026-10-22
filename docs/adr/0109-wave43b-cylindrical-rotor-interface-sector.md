# ADR 0109: Wave 43B, a cylindrical non-conformal rotor/stator interface and sector periodicity through it (F-126, F-114)

Status: registered before any code of this wave was written. Limits and decision rules below are not changed after a run; a
missed limit becomes an ignored `*_known_miss` test with the limit intact; a defective set-up gets a new corrected test
registered here as an amendment before it runs. Written at HEAD 80c26e9, target `<target dir>`. Clean-room:
first principles only (exact solutions of the Navier-Stokes equations and polygon geometry); no literature value enters a limit.

## Context

F-126 (partial) lists as not delivered, among others: curved or non-aligned interfaces and sector periodicity of a stage. F-114
(done) joins two meshes only across planar, coplanar, opposed patches (`rcfd_mesh::non_conformal::join_non_conformal`), and ADR
0008 limits the frozen-rotor (MRF) interface to that planar join perpendicular to the axis. In an axial fan the natural
rotor/stator boundary around the rotor is a cylinder (the rotor zone is a drum `r <= R` or a shell between hub and casing), and
the rotor and stator meshes have different counts around (different blade counts per passage).

## Which slice, and why

Attempted: **(a) a non-conformal interface on a cylinder about the rotation axis**, used as a frozen-rotor interface between an
MRF zone and a stationary zone with different cell counts around and axially, and **(b) rotational sector periodicity through
that interface** (the sides of a periodic sector cross the interface).

Not attempted, with the reason:

* **A planar interface that is not perpendicular to the axis.** An absolute-velocity MRF zone must be bounded by a surface of
  revolution (ADR 0008; enforced by `MrfZone::worst_non_axisymmetry`, limit 1e-6), because otherwise the frame flux
  `(Omega x r) . S` does not vanish and the zone loses mass. A tilted plane is not a surface of revolution, so it has no
  frozen-rotor meaning in this formulation. Between zones in the same frame the existing planar join already accepts any plane
  orientation (the normal is taken from patch A). Nothing to add.
* **MRF on a periodic sector.** The seam faces of a sector are planes containing the axis, which carry frame flux, so the MRF
  zone check rejects a rotor zone with cyclic seams; the absolute-velocity rotor passage with a frame flux on seams was
  removed after ADRs 0082 / 0085 because it did not converge. Part (b) is therefore checked in the inertial frame and in the
  whole-domain relative-velocity frame (`RotatingFrame`, the set-up of C2b in ADR 0071), not with an MRF zone. MRF on seams is
  the blocker of the next slice.
* **The F-126 acceptance** (a public low-speed compressor/rotor case reproducing pressure ratio and efficiency within published
  scatter). Out of reach this wave: the only open case the generator can represent (FAN-01) misses for numerical reasons that
  this interface does not touch (docs/milestone-fan.md). Not attempted, not claimed. F-126 stays partial.
* Conical or general surfaces of revolution, partially overlapping patches, transient sliding, the mixing plane on a
  cylindrical interface.

## Decision (method)

New function `rcfd_mesh::non_conformal::join_non_conformal_cylindrical(a, patch_a, b, patch_b, origin, axis, tol)`. It reuses
the planar join's algorithm with a different chart: interface points map to `(s, z) = (R theta, axial coordinate)` on the
cylinder of radius `R` (mean radius of patch A's points) about the given axis, clipping (Sutherland-Hodgman), point
registry, hanging-node insertion and assembly are unchanged, and new points map back to `(R cos theta, R sin theta, z)`.
Each overlap of an A face with a B face spans one interval of the merged set of angles and one interval of the merged axial
breakpoints, so it is a planar chord rectangle whose centroid lies on the bisecting radial plane: its normal is radial and its
frame flux is zero to round-off. The interface becomes the polygon through the merged angles on the true circle, so the cells
next to it on the inner side gain a sliver and those on the outer side lose one (their axial faces become slightly
non-convex); the total volume is the closed-form volume between the inner wall polygon and the outer wall polygon. The seam of
the chart (theta = +-pi) is handled by a period `2 pi R` in the registry and by unwrapping each face loop about its centre;
the planar path keeps its arithmetic unchanged (no period), so planar joins stay bit-identical.

Opt-in only: no generator, schema key or default changes. Solver checks write the joined mesh with
`rcfd_io::openfoam::write_dir` to a test-unique temporary directory and read it through `mesh.source = "file"`. The MRF zone is
the cell zone `rotor` (the inner block), which the join carries over.

## Registered checks

Common geometry: `r_i = 0.5`, interface `R = 0.75`, `r_o = 1.0` about the z axis through the origin; `nu = 0.1`, `rho = 1`;
`W_i = 1`, `V_i = W_i r_i = 0.5`. Taylor-Couette `u_theta = A r + B / r` with `A = -1/3`, `B = 1/3` (ADR 0008). "Interface
faces" are the faces listed in `JoinReport::interface_faces`.

### Mesh level (rcfd-mesh unit tests in `non_conformal.rs`)

* **G0 planar bit-identity.** `Mesh::content_hash` of (i) the planar join of the 3-D channel test (3x4x5 against 2x7x3) and
  (ii) the planar join of `annulus_3d(48, 16, 2)` / `annulus_3d(48, 11, 3)` at z = 0.2 equal the values recorded at HEAD
  80c26e9 before any code change. Exact equality.
* **G1 full annulus.** A = `annulus_3d(48, 6, 2, 0.5, 0.75, 0.4, 0)` (patch `outer`), B = `annulus_3d(35, 6, 3, 0.75, 1.0,
  0.4, 0)` (patch `inner`); 35 is odd, so B faces straddle theta = +-pi, and gcd(48, 35) = 1. Criteria: overlap count
  (48 + 35 - 1) x 4 = 328 exactly (axial breakpoints {0, 0.4/3, 0.2, 0.8/3, 0.4}); worst face tiling error <= 1e-12; every
  cell's area-vector sum <= 1e-14 of the sum of its face areas; total volume within 1e-12 (relative) of
  `0.4 (35/2 r_o^2 sin(2 pi/35) - 48/2 r_i^2 sin(2 pi/48))`; sum of interface face areas within 1e-12 (relative) of
  `0.4 sum_k 2 R sin(dtheta_k / 2)` over the 82 intervals of the merged angle set; on every interface face
  `|theta_hat(c_f) . S_f| / |S_f| <= 1e-12` and `|z_hat . S_f| / |S_f| <= 1e-12`; `FvGeometry` is not built at this level
  (the solver checks build it).
* **G2 60-degree sector.** A = `annulus_sector(8, 8, 0.5, 0.75, pi/3, uniform, 0.1)` (patch `outer`), B =
  `annulus_sector(6, 8, 0.75, 1.0, pi/3, uniform, 0.1)` (patch `inner`), joined, then `set_periodic_rotation("theta_min",
  "theta_max", 0, z, pi/3)` on the merged pair. Criteria: 12 overlap faces exactly (merged angles in units of pi/72:
  {0,3,4,6,8,9,12,15,16,18,20,21,24}); tiling <= 1e-12; closure <= 1e-14; the periodic pair is accepted by `FvGeometry::new`.
* **G3 located errors.** B at another radius (inner radius 0.76): error mentions `cylinder`; a planar patch (`zmax`) passed as
  the cylindrical interface: error mentions `radial`; B shorter axially (height 0.3): error mentions `cover`.

### Solver level, full annulus (rcfd-api, `tests/cylindrical_interface_w43b.rs`)

Mesh M: inner block `annulus_3d(48, 8, 2, 0.5, 0.75, 0.4)` = cell zone `rotor`, outer block `annulus_3d(35, 8, 3, 0.75, 1.0,
0.4)`, joined on the cylinder. Solver as ADR 0008: SIMPLEC, `linear_upwind`, one non-orthogonal corrector, tolerances
1e-10, at most 4000 iterations, end planes `symmetry` unless stated, walls by velocity expressions. MRF source
`zone = "cell_zone"`, `cell_zone = "rotor"`.

* **C1 solid-body rotation** (the M1 set-up): both walls at `W = 2` (`u = W x r`), MRF `Omega = 2` on `rotor`. Limits (M1):
  `max |u - W x r| <= 2e-2 W r_o`; pressure differences to cell 0 against `W^2 (r^2 - r_0^2)/2` within 5e-2 of
  `W^2 (r_o^2 - r_i^2)/2`.
* **C2 Taylor-Couette across the cylinder** (the M4 set-up): inner wall `W_i`, outer at rest, MRF `Omega = W_i` on `rotor`.
  Limits: `max |u_theta - analytic| <= 2e-2 V_i` (M4) and `<= 1.5 x max(e_48, e_35)` (the F-114 rule), where `e_48`, `e_35`
  are the same error on the conformal `annulus_3d(n, 16, 2, 0.5, 1.0, 0.4)` with `n` = 48 resp. 35, MRF `Omega = W_i`,
  `zone = "all"` (the M2 set-up). The `moment` torque on `inner` is reported, not a criterion (it misses 3 % on conformal
  48-around meshes already, ADR 0008).
* **C3 conservation in the C2 run** (M5 limits): converged (`is_converged`); continuity monitor <= 1e-8; `|sum of interface
  fluxes| <= 1e-8 V_i A_if`; `sum |interface flux| <= 1e-3 V_i A_if` (no spurious radial flow), `A_if` the interface area.
* **C4 frame invariance across the cylinder** (M3 pairwise limit): C2 with the rotor frame at `Omega = 0` (an MRF source of
  zero speed, the stationary frame) and at `Omega = 2 W_i`: max cell difference of `(u_theta, u_r, u_z)` to the
  `Omega = W_i` run `<= 1e-2 V_i` each.
* **C5 radial source flow through the cylinder.** Exact solution `u = m r_hat / r`, `m = 0.5` (inlet speed 1), axisymmetric,
  so the MRF form is exact for it. Inner patch: fixed velocity `m (x, y)/(x^2 + y^2)`; outer patch: zero-gradient velocity,
  fixed pressure 0; ends symmetry; MRF `Omega = W_i` on `rotor`. Limits (M6 analogues): `|inflow - outflow| <= 1e-9 |inflow|`;
  `|sum of interface fluxes - inflow| <= 1e-9 |inflow|`; `max |u - m r_hat / r| <= 2e-2 m / r_i`; `max |u_theta| <= 2e-2 m / r_i`.

### Solver level, periodic sector (rcfd-fv, `tests/cylindrical_interface_w43b.rs`)

The G2 sector mesh; template of C2b in ADR 0071 (`crates/rcfd-fv/tests/rotating_foundation.rs`): SIMPLEC, alpha_u 0.8,
alpha_p 1, central convection, inner wall at `W_i` and outer wall at rest in the inertial frame (given in the solved frame),
converged when `res_u < 1e-10` and continuity < 1e-9 within 4000 iterations.

* **S1 inertial frame** (no source) and **S2 relative-velocity rotating frame** `Omega_f = W_i` (whole domain, centrifugal on).
  Limits for each: converged; `max |u_theta,abs - analytic| <= 1.5 x max(e_8, e_6)`, the errors of the conformal
  `annulus_sector(n, 16, 0.5, 1.0, pi/3)` with `n` = 8 resp. 6 in the same frame; `|sum of interface fluxes| <= 1e-8 V_i A_if`;
  `sum |interface flux| <= 1e-3 V_i A_if`; seam balance `|sum phi(theta_min) + sum phi(theta_max)| <= 1e-8 V_i A_seam`.

### Status decision rule

F-126 stays partial whatever the outcome (its acceptance is the published compressor case). F-114 is already done; its note is
extended. Defaults and `crates/rcfd-api/tests/baseline_hashes.rs` unchanged (checked in the gate).

## Smoke results before the registered solver runs (rule d)

Written after the join was implemented and before any registered solver run. The mesh-level tests G0-G3 had already run once as
registered (they are construction checks; their values are in the addendum). Smoke = construction plus 50 iterations, not a
criterion (ignored tests `smoke_w43b_setups` in rcfd-api and `smoke_w43b_sector` in rcfd-fv):

* Mesh M: 1608 cells (768 rotor + 840 stator), interface area 1.88410 (2 pi R lz = 1.88496; the 82-gon is inscribed), read back
  through `mesh.source = "file"` with the cell zone `rotor`; the MRF zone check accepted the zone in every set-up (no located
  error). C1, C2, C4 (Omega 0), C5 and both conformal controls ran 50 iterations without failure; the C5 inflow is 1.258 (the
  `m (x, y)/r^2` expression with `[m^2/s]` parses and is evaluated); the C2 conformal controls read 6.8e-3 and 1.17e-2 V_i at
  iteration 50.
* Sector: FvGeometry accepts the re-applied rotational pair after the join; S1 and S2 ran 50 iterations (continuity 9e-17 and
  4e-10).

No set-up was found defective; no limit changes. The registered runs follow.
