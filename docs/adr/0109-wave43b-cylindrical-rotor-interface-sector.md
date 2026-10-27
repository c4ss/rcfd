# ADR 0109: Wave 43B, a cylindrical non-conformal rotor/stator interface and sector periodicity through it (F-126, F-114)

Status: registered before any code of this wave was written. Limits and decision rules below are not changed after a run; a
missed limit becomes an ignored `*_known_miss` test with the limit intact; a defective set-up gets a new corrected test
registered here as an amendment before it runs. Written at HEAD 38718c0, target `<target dir>`. Clean-room:
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
  38718c0 before any code change. Exact equality.
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

## Registered runs, first pass (as registered, before any amendment)

* Sector, rcfd-fv: S1 converged in 285 iterations, error 1.272e-2 V_i (bound 1.5 x max(6.763e-3, 1.108e-2) = 1.661e-2) PASS, net
  interface flux 3.1e-17 V_i A PASS, seam balance 0 PASS, `sum |interface flux|` 5.79e-3 V_i A against 1e-3 MISS. S2 converged in
  295 iterations, error 1.278e-2 V_i PASS (same bound), net 4.7e-11 PASS, seam 3.5e-10 PASS, `sum |flux|` 2.06e-2 MISS. The
  absolute interface flux is not a spurious net flow: the owner and neighbour of an overlap face sit at different angles, so the
  interpolated face velocity of a purely azimuthal flow has a radial component of order `u_theta dtheta`, which the pressure
  correction turns into a circulation in and out through neighbouring overlap faces (net zero to round-off).
* Full annulus, rcfd-api: every run on mesh M DIVERGED (C1, C2, C4 at Omega 0 / 1 / 2, C5: continuity 1e33 to 1e81 at iteration
  4000), so C1-C5 all MISS; the conformal controls converged (48 around: error 6.760e-3 V_i in 1187 iterations; 35 around: 1.164e-2
  in 1357). The 50-iteration smoke had already shown continuity 1e-3 to 1e-4 on mesh M against about 1e-9 on the controls; 50
  iterations were too short to expose the divergence and the smoke did not flag it. That was a smoke weakness.

All misses become ignored `*_known_miss` tests with the limits unchanged.

## Amendment 1 (written after the first pass and five residual-only diagnostics, before any run it governs)

What I knew when writing this: five diagnostic runs of the C2 set-up (Taylor-Couette, MRF on `rotor`), full 4000 iterations,
looking only at residuals and continuity, not at accuracy, and `check_mesh` on each mesh. Mesh M: max non-orthogonality 79.2 deg,
diverged; mesh M without any source: diverged (so MRF is not the cause); axial counts matched (B nz = 2): 75.7 deg, diverged;
counts around matched (B 48 around, nz 3): 72.7 deg, diverged; both matched (the cylindrical join of identical blocks, conformal):
0.0 deg, converged in 741 iterations. Cause: an overlap face joins cell centres that are offset by up to half a cell around and
axially (0.05 to 0.1) but only `dr = 1/32` apart radially, so the overlap faces are 70-80 deg non-orthogonal, and the default
SIMPLEC with one non-orthogonal corrector diverges on them. This is a limitation of the delivered join on high-aspect cells at the
interface, not a construction error (G1 closure, tiling and volume are at round-off).

Corrected set-up, chosen by a convergence-only rule: candidates in this order; the first for which the C2 set-up reaches the
residual targets (`is_converged`) within 4000 iterations, judged from a residual-only probe, is selected; accuracy is not looked at
during the selection. Then C1-C5 run once with the selected candidate, with the registered limits, and the conformal controls of C2
are re-run with the same solver options (and, for K2, the same radial distribution). If none converges, the solver part stays a miss
and no further candidate is added.

* **K0**: mesh M unchanged, `solver.velocity_correction = "face_flux"` (ADR 0084: removes the period-two pressure mode of SIMPLEC
  on strongly non-orthogonal meshes). Controls: the registered conformal `annulus_3d(48 | 35, 16, 2)` with the same option.
* **K1**: K0 plus `non_orthogonal_correctors = 3`. Controls as K0 with the same options.
* **K2**: interface cells closer to isotropic, with the K0 option: rotor block `annulus_3d(96, 6, 4, 0.5, 0.75, 0.4, 0,
  Expansion(3))` (cells grow toward the interface), stator block `annulus_3d(70, 6, 6, 0.75, 1.0, 0.4, 0, Expansion(1/3))` (cells
  shrink away from it). Controls: the cylindrical joins of identical blocks (conformal) `A(96, 6, 4, Expansion(3))` with
  `B(96, 6, 4, Expansion(1/3))`, and the same with 70 around and 6 axial, MRF `zone = "all"`, same solver option.

The sector absolute-flux misses keep their limit; the sector tests are split into the passing criteria (converged, error bound,
net flux, seam balance) and the ignored `sum |flux|` known misses.

## Addendum: measured values (final code)

Candidate selection under amendment 1 (residual-only probe of the C2 set-up): K0 diverged (continuity 2.3e71 at iteration 4000), K1
diverged (NaN), K2 converged in 272 iterations (max non-orthogonality 48.7 deg, skewness 0.14) and was selected. C1-C5 then ran on
K2 with the registered limits.

| check | measured | limit | result |
| --- | --- | --- | --- |
| G0 planar join hashes | equal to the HEAD 38718c0 values (0xd055127ef92b9c59, 0x7c6b5d7b77039063) | exact | PASS |
| G1 overlaps / tiling / closure | 328 / 3.1e-15 / 1.2e-16 | 328 / 1e-12 / 1e-14 | PASS |
| G1 volume / interface area | 1.4e-15 / 1.2e-16 relative | 1e-12 | PASS |
| G1 frame flux (tangential / axial part of S) | 1.8e-14 / 1.1e-15 | 1e-12 | PASS |
| G2 overlaps / tiling / closure / periodic pair | 12 / 5.3e-16 / 7.4e-17 / accepted by FvGeometry | 12 / 1e-12 / 1e-14 | PASS |
| G3 located errors | cylinder, radial, cover | substrings | PASS |
| S1 inertial sector: converged, error | 285 it, 1.272e-2 V_i (bound 1.5 x max(6.763e-3, 1.108e-2) = 1.661e-2) | | PASS |
| S1 net interface flux / seam balance | 3.1e-17 V_i A / 0 | 1e-8 | PASS |
| S1 sum of absolute interface fluxes | 5.79e-3 V_i A | 1e-3 | MISS (`s1_sector_absolute_interface_flux_known_miss`) |
| S2 rotating-frame sector: converged, error | 295 it, 1.278e-2 V_i (bound 1.661e-2; controls 6.758e-3, 1.107e-2) | | PASS |
| S2 net interface flux / seam balance | 4.7e-11 V_i A / 3.5e-10 V_i A_seam | 1e-8 | PASS |
| S2 sum of absolute interface fluxes | 2.06e-2 V_i A | 1e-3 | MISS (`s2_sector_absolute_interface_flux_known_miss`) |
| C1-C5 on the registered mesh M | all runs diverged | | MISS (`*_mesh_m_known_miss`) |
| K2 C1 solid body: velocity / pressure | 1.58e-3 W r_o / 1.40e-2 of the range | 2e-2 / 5e-2 | PASS |
| K2 C2 Taylor-Couette | 3.79e-3 V_i (controls 3.00e-3 at 96 around, 3.88e-3 at 70; bound 5.82e-3) | 2e-2 and 1.5 x control | PASS |
| K2 C2 torque (reported only) | 0.16354 against 0.16755 (-2.4 %) | none | reported |
| K2 C3 converged / continuity / net interface flux | yes / 1.2e-11 / 5.3e-14 V_i A | 1e-8 / 1e-8 | PASS |
| K2 C3 sum of absolute interface fluxes | 1.168e-3 V_i A | 1e-3 | MISS (`k2_c3_absolute_interface_flux_known_miss`) |
| K2 C4 frame invariance (Omega 0 and 2 against 1) | 4.24e-4 / 4.15e-4 V_i | 1e-2 | PASS |
| K2 C5 source flow: outflow / interface flux against inflow | 2.9e-14 / 1.5e-14 relative | 1e-9 | PASS |
| K2 C5 velocity / swirl | 4.81e-3 / 3.0e-4 of m / r_i | 2e-2 | PASS |

Conservation: every overlap face is one internal face, so the flux that leaves the rotor side through it enters the stator side
identically; the net interface flux equals the inflow to 1.5e-14 (C5) and the net flux of a swirl flow is 3e-17 to 5e-14, at the
level of the converged continuity.

Diagnostic, not a criterion: the absolute interface flux of S1 halves when the sector counts are doubled (16 / 12 around, 16 radial
per block: 2.89e-3 V_i A against 5.79e-3; error 5.04e-3 V_i against 1.27e-2), so this spurious in-and-out flux is first order in the
cell size. It comes from interpolating an azimuthal velocity between cell centres at different angles.

Deviations from the registered plan, all stated above or here: (1) the 50-iteration smoke did not expose the divergence on mesh M;
(2) amendment 1 (corrected set-up by a convergence-only rule) was written after the first pass; (3) per variant, C1 velocity and
pressure are one test function with both limits; (4) the K2 C2/C3 test first ran with the absolute-flux limit inside it and failed
on it only; the limit was then moved unchanged into its own ignored known-miss test and the run repeated (the code is
deterministic; the repeat gave identical values); (5) for K2 the conformal controls are cylindrical joins of identical blocks (the
`annulus_3d` generator cannot grade towards the middle).

Unverified references: none; every reference value is an exact solution or polygon geometry.

Status: F-126 stays partial (its acceptance, a published compressor/rotor case, was not attempted). F-114 stays done; its note
records the cylindrical join. What blocks the next slice: (a) the overlap faces of a cylindrical join between cells with large
tangential/axial size against the radial size are 70-80 deg non-orthogonal and the steady solver diverges on them (none of the
registered remedies fixed mesh M; K2 works by making the interface cells closer to isotropic), so the fan mesher must size the
interface layer accordingly; (b) MRF zones cannot have cyclic seams (frame flux on the seam planes), so a periodic rotor passage
with an MRF zone and a cylindrical interface is not yet possible; (c) the first-order absolute interface flux; (d) no case-tree
generator or key yet (library function plus mesh file); (e) conical surfaces, partial cover, transient sliding, the mixing plane on a
cylinder; (f) the FAN-01 numerics of docs/milestone-fan.md for the acceptance case.

## Final gate

Fresh `git archive` of HEAD 54e0170: `cargo fmt --check` clean, `cargo clippy --workspace --all-targets -D warnings` clean,
`cargo test --workspace --release --no-fail-fast` 1977 passed, 0 failed, 595 ignored (this HEAD includes the wave-43A commits made
meanwhile).
