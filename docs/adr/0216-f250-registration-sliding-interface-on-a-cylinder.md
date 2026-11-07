# ADR 0216: F-250 registration: a sliding (rotating) non-conformal interface on a cylinder, in the library

Status: **REGISTERED 2026-11-07**, written before any code of F-250 and before any run of its checks. Not yet accepted by
the maintainer. Before this text one thing was run on main 08316f7, with the code of main as it is: the record tests R1 to
R3 (the new file `crates/rcfd-fv/tests/sliding_interface_f250_records.rs`, no source change), twice, which made the
records below. Results go in the addendum at the end.

Parents: [ADR 0109](0109-wave43b-cylindrical-rotor-interface-sector.md) (F-126/F-114: the cylindrical join
`join_non_conformal_cylindrical`, its slivers and its first-order interface interpolation),
[ADR 0112](0112-wave44b-cylindrical-interface-preflight-sizing-case-tree.md) (the interface non-orthogonality pre-flight
and its 70 deg limit), [ADR 0033](0033-wave20b-dynamic-mesh.md) (F-063: the incompressible ALE step D2, the swept volumes
and the GCL, the moving-wall pressure condition D3, the rotating annulus G6), [ADR 0101](0101-wave41a-oscillating-cylinder-two-timescale-decay-vortex-bracket-milne.md)
(the continuity 1e-9 and GCL 1e-12 mean(V) limits of the moving-mesh checks), [ADR 0213](0213-f257-registration-prism-layering-and-incompressible-layering.md)
(F-257: `PimpleSolver::replace_mesh`, the first change of topology in the PISO path), [ADR 0016](0016-wave13-numerics.md)
(the Taylor-Couette torque closed form and the first-order wall-gradient moment), [ADR 0137](0137-linux-gate-platform-values.md)
(platform values). Roadmap item F-250 (depends on F-063 and F-114).

Number: the lead gave 0216 to F-250. Main 08316f7 holds 0213 as its highest; 0212 is on the branch `f296-pair-diffusivity`
and 0214 on `f299-piso-thin-gap`. No branch holds a 0215 or a 0216.

House rules (ADR 0130, ADR 0191, ADR 0213):

- limits are fixed here, before the code and the runs, and are not changed after a run;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact and the measured number recorded, and the
  cause is diagnosed, not tuned away;
- a defective set-up gets a new corrected test by an amendment written before the corrected run, and the old one stays as
  an ignored record;
- every new code path is opt-in: a run that does not call the new types and functions runs as before, bit for bit (R1 to
  R3 and the existing records);
- no case-schema key is added (S4), so neither `crates/rcfd-api/tests/baseline_hashes.rs` nor the record R0
  (`case_hashes_w48u.rs`) is touched, no golden is re-recorded, and no existing test is edited;
- each registered run is made once, and nothing is re-run to pick a result;
- no new third-party crate, no commit trailers.

Clean-room: the method below is derived from the definitions of ADR 0033 (swept volumes, the discrete geometric
conservation law, the ALE step) and ADR 0109 (the cylindrical join), and from the discrete continuity of a face flux. The
reference values are closed forms derived here (Taylor-Couette, solid-body rotation, the decaying azimuthal mode of an
annulus with its Bessel-function eigenvalue). No source was read for this work, and no vendor code, text or install was
used.

## What F-250 asks

The roadmap entry: sliding interface (AMI), an arbitrary mesh interface between non-conforming moving or rotating zones.
Acceptance (the numeric limits fixed by the user on 2026-10-22):

- (A1) a rotor-stator case with a sliding interface, the mesh sliding by at least one full cell pitch, conserves mass across
  the interface to 1e-10 relative per step summed over all interface faces;
- (A2) rigid rotation of a fluid across the interface differs from the same case on a conforming mesh by no more than 1.5
  times the conforming-mesh velocity error;
- (A3) a Taylor-Couette torque across the interface is within 2 percent of the closed form;
- (A4) a case without the interface is bit-identical (baseline hashes unchanged).

The note: F-114 handles planar interfaces only; F-126 covers frozen-rotor and mixing-plane; size L. The lead's brief adds
three checks to register: flux conservation across the sliding interface, uniform flow held to round-off on a rotating
interface, and a published or closed-form transient check.

**Reading of the acceptance (registered here).**

1. "Sliding interface" is read as a rotor zone that turns rigidly about an axis against a stator zone, across a
   non-conforming interface on a cylinder about that axis, with the interface rebuilt every step (the overlaps change as
   the rotor turns). The planar (translational) slide and an interface that does not cover itself after the motion (a
   sector) are not in this registration (Q3).
2. A1: "mass across the interface ... summed over all interface faces" is, for a through-flow, the sum of the face fluxes
   of all interface faces against the inflow, per step, relative to the inflow (S1); for a closed annulus, the same sum
   against zero, relative to a reference flux `V_ref A_I` (S7, T5). "Sliding by at least one full cell pitch": the rotor
   turns by at least the larger of the two pitches `2 pi / n` in every set-up that checks A1 to A3.
3. A2 has two readings, both registered. Reading A (the primary, as F-114 and ADR 0109 read "1.5 x the conforming
   error"): the velocity error of the sliding run against the exact rigid rotation is at most 1.5 times the larger of the
   errors of two conforming controls, one with the rotor's count around and one with the stator's (R1). The conforming
   controls are static meshes: a conforming mesh has no rotor zone that can slide. A supplementary control rotates the
   whole conforming mesh with the rotor (R1b), so the error of the moving mesh itself can be told apart from that of the
   interface. Reading B (literal, "differs from the same case"): with equal counts on both sides, at a time when the rotor
   has turned by a whole number of pitches, the cell-by-cell difference to the static conforming run is at most 1.5 times
   that run's error (R2).
4. A3: the Taylor-Couette flow between a rotor wall turning with the rotor zone and a stator wall at rest, run in time from
   rest to the steady state on the sliding mesh. "The torque across the interface" is measured three ways, each against
   the closed form: on the rotor wall, on the stator wall, and transmitted through the interface faces (T1 to T3, M7).
5. A4: no case-tree change (S4), so a case is bit-identical by construction; the gate runs `baseline_hashes.rs` and H1 of
   `case_hashes_w48u.rs` unedited, and the records R1 to R3 show that the library paths that F-250 does not call are
   unchanged.
6. "A published or closed-form transient check" (the brief): the decay of the slowest azimuthal mode of the annulus with
   both walls at rest while the rotor zone turns (D1, D2); no published data are used.
7. The acceptance is library work. The case tree is not in it (S4, Q1).

## What the code read shows (main 08316f7, read only)

1. **`rcfd_mesh::non_conformal::join_non_conformal_cylindrical(a, patch_a, b, patch_b, origin, axis, tol)`**: cells of
   `a` keep their indices and those of `b` follow (`JoinReport::b_cell_offset`); every overlap is one internal face with an
   `a` cell as owner and a `b` cell as neighbour (`JoinReport::interface_faces`); internal faces are sorted by `(owner,
   neighbour)` (stable), and two cells may share at most one face; the boundary patches of both meshes are kept face for face
   in their order (a's patches first, same-name patches merged, a's faces first), so the boundary face count and order do
   not depend on where the overlaps fall. The interface becomes the polygon through the merged angles: the cells on both
   sides gain or lose a sliver, and the faces that end on the interface receive the merged points (their point lists change
   with the overlaps).
2. **`rcfd_mesh::motion`**: `rotation_matrix`, `Pose`, `moved_mesh` (same topology, validated), `swept_volumes_about`
   (ADR 0033's swept volume of every face between two point sets of one topology; `sum_f sigma_f W_f = V1 - V0` per cell to
   round-off).
3. **`rcfd_fv::pimple::moving`**: `step_ale_with_walls(dt, new_geom, mesh_flux, wall_dpdn)` refuses a new geometry whose
   cell or face count differs; it builds `AleStep { mesh_flux, vol_old, defect, wall_dpdn }` with the old-level Rhie-Chow
   defect `r_f (phi_f - (u_old)_f . S_f)` on the old geometry (`r_f = w V0_o / V1_o + (1 - w) V0_n / V1_n`), installs the
   new geometry and calls `step`. The step convects with `phi - m`, puts the old volume in the transient term (`fix_ddt`),
   applies `wall_dpdn` as the fixed-flux pressure condition and refuses every time scheme but implicit Euler.
   `replace_mesh` (ADR 0213) installs another topology between two steps with a given `u`, `p`, `phi`; it refuses a changed
   patch list and set `nu_face`, `body_force`, `source_force`, `sink` or `face_jump`.
4. **`PimpleSolver::step`** reads `self.phi` (the convecting flux of the first outer iteration and the continuity
   measure), `hist_u` (the old level, one value per cell) and, off the moving path, `phi_hist`; the boundary conditions live
   in the fields per patch, and a `Profile::PerFace` condition indexes the faces of its patch locally.
5. **`rcfd_mesh::gen::cylinder_ogrid(n_theta, n_r, r_in, r_out, grading, depth)`** (2-D, one cell deep): cell
   `ir n_theta + it`; patches `cylinder` (inner, `Wall`), `farfield` (outer, `Generic`), `frontAndBack` (`Empty`).
   **`annulus_3d`**: patches `inner`, `outer`, `zmin`, `zmax` (walls).
6. Nothing in the solver, the mesh crate or the case tree moves one zone against another with a changing topology; the
   README says "Sliding-mesh transient solves are not available".

## Run before this registration (disclosed)

The test file `crates/rcfd-fv/tests/sliding_interface_f250_records.rs` was written with no source change and run twice on
main 08316f7 (Windows, release). Both runs printed the same values:

| Record | Run | Hash |
| --- | --- | --- |
| R1 | ADR 0033 G6 as a library loop: `annulus_3d(48, 8, 2, 0.5, 1, 0.2)` rotating rigidly with the fluid (`W = 1`), walls following the mesh, PISO with central convection and tight solvers, `nu = 0.01`, `dt = 0.02`, 20 steps; the bits of `u`, `p`, `phi`, the time and the step count | `e4bc6f7b23554262` |
| R2 | the cylindrical join of the pair P below with the rotor at 0 and at 0.0123 rad; both content hashes, the join reports and the interface face indices | `77a3c29e4ad6f213` |
| R3 | PISO Taylor-Couette from rest on the static joined pair P (rotor at 0), `nu = 1`, two non-orthogonal correctors, `dt = 0.005`, 20 steps; as R1 | `6378ceeeaacb6799` |

R2 printed, for P: 2112 cells; at angle 0, 240 overlaps (48 zero-area overlaps dropped where the two polygons share
vertices, tiling error 8.5e-15) and a worst interface non-orthogonality of 43.5 deg; at 0.0123 rad, 264 overlaps (none
dropped, tiling 2.1e-15) and 47.0 deg. These are set-up properties; no check below was run.

## The model (derived)

**M1. The sliding mesh.** The rotor block `A` (reference position) and the stator block `B` have interface patches on one
cylinder of radius `R` about the axis `a` through `o`. At rotor angle `theta` the points of `A` are rotated by `theta`
about the axis (`x' = o + Rot(a, theta)(x - o)`, from the reference points every time, so no round-off accumulates) and
joined to `B` by `join_non_conformal_cylindrical`. Cells keep their indices at every angle (A first, then B), so cell
fields carry over unchanged. The interface patches must cover each other at every angle; a full annulus does.

**M2. Face identity across a step.** From the mesh `M0` at `theta0` to `M1` at `theta1`: an internal face of `M1` that is
not an interface face is the face of `M0` with the same `(owner, neighbour)` (unique, read 1); a boundary face is the face
at the same position after the internal faces (`f1 - n_int1 = f0 - n_int0`, read 1) and must have the same owner; the
interface faces of `M1` are new.

**M3. Mesh flux.** A rotor face (owner in `A`, and a boundary face or neighbour in `A`) moves rigidly: its swept volume
`W_f` is ADR 0033's swept volume between its point loop in `M1` rotated back by `theta1 - theta0` and the same loop in
`M1`, and `m_f = W_f / dt`. A stator face does not move, `m_f = 0`. An empty face has `m_f = 0` (as `step_ale`). On the
interface faces the mesh flux closes the geometric conservation law (GCL) of the cells next to the interface: their
volumes change from step to step because the slivers of the merged polygon change (read 1), and no point motion describes
that. With `t_c = (V1_c - V0_c)/dt - sum_{f not I} sigma_cf m_f` for every cell `c` with an interface face, the interface
values solve `sum_{f in I(c)} sigma_cf m_f = t_c`; the solution taken is the area-weighted minimum-norm one,
`m_f = |S_f| (lambda_owner - lambda_neighbour)` with `L lambda = t` on the graph whose nodes are those cells and whose
edges are the interface faces (`L` the weighted graph Laplacian). `L` is singular with the constants of each connected
part; the mean of `t` over each part is removed first (it is the change of the total volume of those cells, which is zero
up to round-off: the total volume does not depend on the angle, M5). Solved by conjugate gradients with that projection.
Then `V1_c - V0_c = dt sum_f sigma_cf m_f` for every cell, to the solve's tolerance.

**M4. The old-level face flux on the new mesh.** A mapped face keeps its old value `phi_f`. On the interface faces the
value is the face interpolation of the old velocity on the new face, `(u_old)_f . S_f`, plus the area-weighted
minimum-norm correction (M3's solve) that makes the old flux of every interface cell divergence-free, as it was on `M0`
(target `-sum_f sigma_cf phi_f` over its faces). The Rhie-Chow defect of the old level is `step_ale`'s on the mapped
internal faces (old value minus the old velocity's face flux on the old geometry, times `r_f`) and zero on the interface
faces. Why: a uniform velocity `U` then stays uniform to round-off. Its old flux is divergence-free on `M1`, so the
momentum predictor's transient and convection terms cancel by the GCL of M3; every defect is zero (on the mapped faces the
old value is `U . S_old`, on the interface faces the defect is zero by definition), so `phiHbyA = U . S_new` is
divergence-free and the pressure stays uniform. With the interface defect taken as the projected value minus the
interpolation instead, `phiHbyA` would carry `U . (S_chord_old - S_chord_new)` of every rotor interface cell and break the
uniform state.

**M5. Total volume.** The joined mesh fills the region between the stator's outer wall polygon and the rotor's inner wall
polygon whatever the angle (the rotor cells end on the merged polygon and the stator cells start on it), so the total
volume does not depend on `theta`; with the 2-D pair P it is `L (60 r_o^2 sin(2 pi/120) - 72 r_i^2 sin(2 pi/144))`.

**M6. Walls on the rotor.** A rotor wall that turns with the rotor at `omega_w` (or is at rest, `omega_w = 0`) gets
`u_b = (omega_w a x (x_f - o))_t + n_f m_f / |S_f|` at the new face centre (the tangential part of the rigid velocity
plus the normal part that carries the swept volume, as `ale::follow_velocity`) and the fixed-flux pressure condition of
ADR 0033 D3, `dp/dn = -n_f . (u_b,new - u_b,old) / dt`, with the old value from the same boundary face of `M0`.

**M7. Taylor-Couette and the torque.** Inner wall `r_i` at `Omega_i`, outer `r_o` at rest: `u_theta = A r + B / r`,
`omega = u_theta / r = A + B / r^2`, `B = Omega_i r_i^2 r_o^2 / (r_o^2 - r_i^2)`; the torque transmitted through any
cylinder is `T = 2 pi r^2 L nu r d(omega)/dr` (kinematic, per unit density), of magnitude `T_ex = 4 pi nu L B`. Measured
with the cells' `omega_c = (x_c x u_c) . a / r_c^2`, `r_c` the distance of the centroid from the axis:

- on a wall, per wall face `f` (centre distance `r_f`, wall value `omega_w`): `T_w = |sum_f nu r_f^2 |S_f| omega'_f|`
  with `omega'_f` the derivative at `r_f` of the quadratic through `(r_f, omega_w)`, the wall cell `(r_1, omega_1)` and
  the next cell outward from the wall in the same block column `(r_2, omega_2)` (cell `c +- n_theta`, read 5); second
  order, a relative error of about `1.5 dr^2 / r^2` for this profile;
- through the interface: `T_I = |sum_{f in I} nu r_f^2 |S_f| (omega_N - omega_P) / (r_N - r_P)|` (central difference
  across each overlap face).

**M8. The decaying mode.** With both walls at rest, `u_theta = phi(r) exp(-nu k^2 t)` with
`phi(r) = J1(k r) Y1(k r_i) - J1(k r_i) Y1(k r)` and `k` the first positive root of
`J1(k r_o) Y1(k r_i) - J1(k r_i) Y1(k r_o) = 0`. For `r_i = 1`, `r_o = 2`: `k = 3.19658` (a pure-Python bisection with the
integral forms below gave `3.1965783808`; the test computes its own value). The Bessel functions are evaluated from
`J_n(x) = (1/pi) int_0^pi cos(n s - x sin s) ds` and `Y_n(x) = (1/pi) int_0^pi sin(x sin s - n s) ds - (1/pi) int_0^inf
(e^{n s} + (-1)^n e^{-n s}) e^{-x sinh s} ds` by composite Simpson rules; the test checks its evaluation against the
Wronskian `J_{n+1}(x) Y_n(x) - J_n(x) Y_{n+1}(x) = 2 / (pi x)` (B0).

## Settings and code

**S1. Library (`rcfd-mesh`, new module `sliding`, additive).**

- `SlidingInterface::cylindrical(rotor: Mesh, rotor_patch, stator: Mesh, stator_patch, origin, axis, tol) -> Result<Self>`
  (checks the patches and joins once at angle 0, so a bad pair fails here with the join's located message),
  `n_rotor_cells()`, `mesh_at(angle) -> Result<SlidingMesh>` (M1; a non-finite angle is an error).
- `SlidingMesh { mesh, report: JoinReport, angle, .. }` with `is_interface(f)`, `is_rotor_face(f)`,
  `face_map_from(&old) -> Result<Vec<Option<usize>>>` (M2; a face without its partner is an error) and
  `rotor_swept_volumes(old_angle) -> Result<Vec<f64>>` (M3's `W_f` on rotor faces, zero elsewhere), `origin()`, `axis()`.
- `non_conformal.rs`, `motion.rs` and every existing function: not edited.

**S2. Library (`rcfd-fv`, new module `sliding`, additive).**

- `interface_balance(mesh, faces, target) -> Result<InterfaceSolve, String>` (M3's solve: the face values, the largest
  residual and the removed means).
- `SlidingStep { geom, mesh_flux, vol_old, phi_old, old_defect, interface_faces, gcl_residual, flux_residual, .. }` (all
  public, so a test can alter them for the negative controls) and `sliding_step(old_geom, old: &SlidingMesh, new:
  &SlidingMesh, phi_old, u_old, dt) -> Result<SlidingStep, String>` (M2 to M4).
- `rotating_wall_conditions(u, step, old_geom, patches, omega_w, dt) -> Result<Vec<f64>, String>` (M6; returns the
  fixed-flux gradient per boundary face, `NaN` elsewhere, as `ale::set_follow_conditions`).

**S3. Library (`rcfd-fv`, `pimple::moving`, additive).** `PimpleSolver::step_sliding(dt, step: SlidingStep, wall_dpdn) ->
Result<StepInfo, String>`: installs the new geometry and the remapped old flux (`phi`, `phi_hist` restarted from it, as
`replace_mesh`), builds the `AleStep` with `step.mesh_flux` (zero on empty faces), `step.vol_old`, the defect `r_f
old_defect_f` (zero without `ddt_correction`) and `wall_dpdn`, and calls `step`. Refused, the solver unchanged: a time
scheme other than implicit Euler, another cell count, sizes that do not fit the new mesh, a patch list that differs in
number, names, kinds or sizes, and a set `nu_face`, `body_force`, `source_force`, `sink` or `face_jump`. `step`,
`step_ale*` and `replace_mesh` are not edited.

**S4. Case tree, CLI, Studio.** Not changed; no key, generator or schema entry is added. The wiring is Q1. The README
line on sliding-mesh solves and `docs/dynamic-mesh.md` get prose only (what the library does, that the tree does not use
it).

## Set-ups

Common: the axis `z` through the origin. "Pitch" is `2 pi / n` for a block with `n` cells around. `|S_f|` the face area,
`A_I = sum_{f in I} |S_f|`, `mean(V)` the mean cell volume of the new mesh. PS (solver settings):
`PimpleSettings::piso(nu)` with central convection, `non_orth_correctors = 2`, implicit Euler, `ddt_correction` on and the
tight solvers `tol_abs = 1e-16`, `tol_rel = 1e-13`, 5000 iterations for momentum and pressure. SL (the loop): with the
solver at `theta_n = Omega n dt`, `new = mesh_at(theta_{n+1})`, `st = sliding_step(&solver.geom, &cur, &new, &solver.phi,
&solver.u.values, dt)`, the boundary values of the set-up on the moved rotor patches, `step_sliding(dt, st, dpdn)`, `cur =
new`.

- **P** (2-D pair, depth `L = 0.1`): rotor `cylinder_ogrid(144, 8, 1, 1.5)` (inner wall `cylinder`, interface
  `farfield`), stator `cylinder_ogrid(120, 8, 1.5, 2)` (interface `cylinder`, outer boundary `farfield`);
  `SlidingInterface::cylindrical(rotor, "farfield", stator, "cylinder", 0, z, 1e-8)`: 2112 cells (1152 rotor), patches
  `cylinder` (rotor wall, moves), `frontAndBack`, `farfield` (stator wall, static). Pitches 0.0436 (rotor) and 0.0524
  (stator); `r_i = 1`, `R = 1.5`, `r_o = 2`, radial spacing `1/16` everywhere.
- **P=**: P with the stator `cylinder_ogrid(144, 8, 1.5, 2)` (equal counts; conforming whenever the rotor has turned by a
  whole number of pitches).
- **P3** (3-D pair): rotor `annulus_3d(48, 4, 2, 1, 1.5, 0.4, 0)` (interface `outer`), stator `annulus_3d(40, 4, 3, 1.5, 2,
  0.4, 0)` (interface `inner`): 864 cells, patches `inner`, `zmin`, `zmax`, `outer`; the axial breakpoints differ (2
  against 3 layers).
- **C144, C120**: the conforming annuli `cylinder_ogrid(144, 16, 1, 2)` and `cylinder_ogrid(120, 16, 1, 2)` (the radial
  nodes of P).

## Registered checks

All tests run in release. The flow checks are in `crates/rcfd-fv/tests/sliding_interface_f250.rs`, the mesh checks are
unit tests in `crates/rcfd-mesh/src/sliding.rs`, the records in `sliding_interface_f250_records.rs`. "Every step" means
the largest value over the steps of the run. Every flow set-up also prints the worst interface non-orthogonality over its
steps (ADR 0112's measure; no limit beyond the set-up check X0).

### K. The sliding mesh (unit, `rcfd-mesh`)

| Id | Check | Limit |
| --- | --- | --- |
| K1 | P at the angles `0.0123 k`, `k = 0..4`, each against the next: every non-interface face of the new mesh has a partner with the same owner and neighbour (boundary: same owner and patch); the unmapped faces are exactly the interface faces; no old face is used twice | exact |
| K2 | P from 0.0123 to 0.0173 rad: (i) every rotor cell without an interface face, `abs(sum_f sigma_f W_f) <= 1e-14 V_c`; (ii) every radial face (in a plane through the axis) of such a cell, `abs(W_f)` against `L (r_2^2 - r_1^2) sin(0.005) / 2` (the straight-path sector swept by the segment `r_1..r_2`); (iii) `W_f = 0` on every stator and interface face | (i) as stated; (ii) relative `1e-12`; (iii) exact |
| K3 | the total volume of `mesh_at(theta)` for `theta` in `{0, 0.0123, 0.05, 1, 3}` against M5's closed form | relative `1e-12` each |
| K4 | refusals: a rotor patch that does not exist, a stator patch that does not exist, a zero axis, a stator whose interface lies at radius 1.6 (`cylinder_ogrid(120, 8, 1.6, 2)`), `mesh_at(NaN)` | `Err` each, the message naming the patch, `axis`, `cylinder`, `angle` |

### F. The interface solve (`rcfd-fv`)

| Id | Check | Limit |
| --- | --- | --- |
| F1 | P at 0.0123 rad: targets pseudo-random in `[-1, 1]` (a fixed linear congruential sequence) on the interface cells; `interface_balance` returns face values whose cell sums equal the targets minus the removed mean | `<= 1e-12` of `max abs(t)` |
| F1n (negative control) | the same values checked against the targets without removing the mean | `>= 1e-3` of `max abs(t)` (the mean of 264 such values is about `1/sqrt(3 . 264) = 3.6e-2`) |

### S. Flux conservation across the sliding interface (acceptance A1)

Set-up SF (radial source flow through the sliding interface): P; the rotor turns at `Omega = 1`; the inner wall `cylinder`
is an inlet, `u_b = U_in x_hat_f` (the in-plane unit vector of the new face centre, set every step), `U_in = 1`; the outer
`farfield` has `p = 0` and zero-gradient velocity; `nu = 0.1`; start `u = U_in (r_i / r) x_hat`, `p = 0`; PS; SL with no
wall gradient; `dt = 0.005`, 100 steps (0.5 rad, 9.5 stator pitches). `Q_in = -sum_{cylinder} phi_f`, `Q_out =
sum_{farfield} phi_f`, `Q_I = sum_{f in I} phi_f`, all after the step.

| Id | Check | Limit |
| --- | --- | --- |
| S1 (acceptance) | every step `abs(Q_I - Q_in) / Q_in` | `<= 1e-10` |
| S2 | every step `abs(Q_out - Q_in) / Q_in` | `<= 1e-10` |
| S3 | every step's `continuity` | `<= 1e-9` (ADR 0101) |
| S4 | every step the GCL residual `max_c abs(V1_c - V0_c - dt sum_f sigma_cf m_f) / mean(V)`, computed in the test from the step's volumes and mesh flux | `<= 1e-12` (ADR 0101) |
| S5 | every step the remapped old flux `max_c abs(sum_f sigma_cf phi_old,f) / max_f abs(S_f)` | `<= 1e-9` |
| S6 (set-up) | the interface overlaps differ between consecutive steps (count or owner-neighbour pairs) in every step | every step |
| N3 (negative control) | every step `abs(sum_{f in I} (u . S)_f - Q_in) / Q_in` with `(u . S)_f` the face interpolation of the step's final velocity (the flux of an interpolating, non-conservative interface) | the largest `>= 1e-6` |

### U. Uniform flow on a rotating interface (the brief)

Set-up U2: P; `U = (1, 0.5, 0)`; `cylinder` and `farfield` fixed value `U`; start `u = U`, `p = 0`; `nu = 0.1`; the rotor
turns at `Omega = 1`; PS; SL with no wall gradient; `dt = 0.005`, 100 steps (0.5 rad). Set-up U3: P3; `U = (1, 0.5,
0.3)` on all four patches; `nu = 0.1`; `Omega = 1`; `dt = 0.01`, 40 steps (0.4 rad, 2.5 stator pitches).

| Id | Check | Limit |
| --- | --- | --- |
| U1 | U2, every step `max_c abs(u_c - U) / abs(U)` and `max_c abs(p_c - p_0) / abs(U)^2` | `<= 1e-10` each |
| U2 | U2, every step the GCL residual (S4's measure) | `<= 1e-12` |
| U3 | U3, every step as U1 | `<= 1e-10` each |
| U4 | U3, every step the GCL residual | `<= 1e-12` |
| N1 (negative control) | U2 with the interface faces' mesh flux set to zero after `sliding_step` (the GCL of M3 not closed): the largest `max_c abs(u_c - U) / abs(U)` | `>= 1e-6` (M3: the sliver of a rotor interface cell changes by about `1e-3` of its volume per step) |
| N2 (negative control) | U2 with the old flux of the interface faces replaced by the interpolation `(u_old)_f . S_f` (no projection, M4): the same measure | `>= 1e-6` |

### R. Rigid rotation across the interface (acceptance A2)

Set-up RR: P; the fluid and the rotor turn at `Omega = 1`; start `u = Omega a x x_c`, `p = Omega^2 r_c^2 / 2`; the rotor
wall `cylinder` turns with the rotor (`rotating_wall_conditions`, `omega_w = 1`, its gradient passed to the step), the
stator wall `farfield` has `u_b = Omega a x x_f` on its static faces; `nu = 1`; PS; SL; `dt = 0.0005`, 200 steps (0.1 rad,
1.9 stator pitches). The small step keeps the moving-mesh time error of the rotor zone (a tangential drift of about
`Omega^2 dt / (2 nu k^2)` of `Omega r`, 2.5e-5 here) below the spatial error of the controls. Error `e = max_c abs(u_c -
Omega a x x_c) / (Omega r_o)` after the last step. Controls: C144 and C120, static, the same start, walls `u_b = Omega a x
x_f` on their static faces, PS, `step(dt)` 200 times. Rotating controls: C144 and C120 turning rigidly with `Omega` as
ADR 0033 G6 (`ale::move_mesh`, `set_follow_conditions` on both walls, `step_ale_with_walls`), 200 steps.

Set-up RR=: P= with `dt = 2 pi / (144 . 80)` (80 steps per pitch), 160 steps (two pitches; the mesh is conforming again at
the end), otherwise RR. Its control is P= at angle 0 (the conforming join, the same cells), static, 160 steps.

| Id | Check | Limit |
| --- | --- | --- |
| R1 (acceptance, reading A) | `e_sliding <= 1.5 max(e_C144, e_C120)` (static controls) | as stated |
| R1b (reading A, rotating controls) | `e_sliding <= 1.5 max(e_C144,rot, e_C120,rot)` | as stated |
| R2 (acceptance, reading B) | RR= at the end: `max_c abs(Rot(-theta) u_c - u_ctl,c) / (Omega r_o)` (rotor cells turned back by the rotor angle, stator cells as they are) `<= 1.5 e_ctl` | as stated |
| R0 (set-up) | RR= at the end: 144 interface faces (conforming) | exact |
| R3 | RR and RR=: every step `abs(Q_I) / (Omega r_o A_I)`, the GCL residual and `continuity` | `<= 1e-10`, `<= 1e-12`, `<= 1e-9` |

### T. Taylor-Couette torque across the interface (acceptance A3)

Set-up TC: P; the rotor wall `cylinder` turns with the rotor at `Omega_i = 1` (`rotating_wall_conditions`, `omega_w = 1`),
the stator wall `farfield` at rest; `nu = 1`; start from rest, `u = 0`, `p = 0`; PS; SL; `dt = 0.005`, 240 steps (`t =
1.2`, 1.2 rad, 23 stator pitches; the slowest mode decays as `exp(-10.2 t)`). Closed form (M7) `T_ex = 4 pi nu L B =
1.675516`, `V_i = Omega_i r_i = 1`.

| Id | Check | Limit |
| --- | --- | --- |
| T1 (acceptance) | `abs(T_w / T_ex - 1)` on the rotor wall `cylinder` (`omega_w = 1`, next cell `c + 144`) after the last step | `<= 2e-2` |
| T2 (acceptance) | the same on the stator wall `farfield` (`omega_w = 0`, next cell `c - 120`) | `<= 2e-2` |
| T3 (acceptance) | `abs(T_I / T_ex - 1)` through the interface faces | `<= 2e-2` |
| T4 (set-up) | steady: the relative change of `T_w` on `cylinder` over the last 20 steps | `<= 1e-4` |
| T5 (acceptance A1, closed annulus) | every step `abs(Q_I) / (V_i A_I)`, the GCL residual and `continuity` | `<= 1e-10`, `<= 1e-12`, `<= 1e-9` |

Printed, no limit: `max_c abs(u_theta - (A r + B/r)) / V_i`; `T_w` of the first-order wall gradient
`(omega_1 - omega_w) / (r_1 - r_f)` (ADR 0016's measure); the same three torques of C144 and C120 run as TC on a static
mesh.

### D. The decaying mode (closed-form transient check, the brief)

Set-up DM: P; both walls at rest (`rotating_wall_conditions` with `omega_w = 0` on `cylinder`, `u_b = 0` on `farfield`);
the rotor turns at `Omega = 1`; `nu = 0.1`; start `u = phi(r_c) e_theta(c) / max phi`, `p = 0`; PS; SL; `dt = 0.005`, 200
steps (`t = 1`, 1 rad). `lambda = nu k^2 = 1.02181`. The amplitude `a(t) = sum_c V_c (u_c . e_theta) phi(r_c) / sum_c V_c
phi(r_c)^2`; the measured rate `lambda_m = ln(a(0.2) / a(1)) / 0.8` (steps 40 and 200). Controls: C144 and C120, static,
the same start, walls at rest, PS, 200 steps of `step`.

| Id | Check | Limit |
| --- | --- | --- |
| B0 (set-up) | the Bessel evaluation: the Wronskian `J_{n+1} Y_n - J_n Y_{n+1} - 2/(pi x)` for `n = 0, 1` and `x` in `{1, 2, 3.2, 6.4}` | `<= 1e-11` of `2/(pi x)` |
| D1 (closed form) | `abs(lambda_m / lambda - 1)` | `<= 2e-2` |
| D2 | `abs(lambda_m - lambda) <= 1.5 max(abs(lambda_m,C144 - lambda), abs(lambda_m,C120 - lambda))` | as stated |
| D3 | every step the GCL residual and `continuity` | `<= 1e-12`, `<= 1e-9` |

The expected error of `lambda_m`, all methods alike: about `-0.26 %` from implicit Euler (`ln(1 + lambda dt)/dt`) and
`-0.3 %` from the radial spacing (`(k dr)^2 / 12`).

### E. Refusals of the solver path

| Id | Check | Limit |
| --- | --- | --- |
| E1 | `step_sliding` with: BDF2; a step built for P3 on a solver of P; a set `nu_face`: `Err` each, and the solver's `u`, `p`, `phi` and time unchanged bit for bit | as stated |
| E2 | `sliding_step` with an old flux one face short, and with an old mesh of another pair: `Err` each | as stated |

### X. Set-up of every flow run

| Id | Check | Limit |
| --- | --- | --- |
| X0 | every step of SF, U2, U3, RR, RR=, TC, DM: the worst interface non-orthogonality, and every rotor angle of a run reaches at least one stator pitch | `<= 70 deg` (ADR 0112); as stated |

### B. Defaults

| Id | Check | Limit |
| --- | --- | --- |
| D-R | records R1 to R3 above, after the code | equal (Windows; printed elsewhere, ADR 0137) |
| D-S | the existing suites, unedited: the `rcfd-mesh` unit tests (`non_conformal`, `motion`), `rcfd-fv` `non_conformal`, `cylindrical_interface_w43b`, `cylindrical_interface_w44b`, `mrf_cyclic_seams_w44c`, `ale_flows`, `ale_gcl`, `layering_f257`, `rotating_mesh_study`; `rcfd-api` `baseline_hashes`, H1 of `case_hashes_w48u`, `cylindrical_interface_w43b`, `cylindrical_interface_w44b`, `dynamic_mesh` | pass (the gate runs every test) |

### Mutations (scratch, not committed)

Each is applied alone to the final tree, the named tests are run once, and they must fail; the file is restored and
checked byte for byte against a kept copy. Their numbers go in the addendum. `scripts/limit_audit.py` is run on the new
test files and every hit is read.

| Id | Mutation | Must fail |
| --- | --- | --- |
| MU1 | `sliding_step`: the interface mesh flux left at zero (M3's solve skipped) | U1, S4 |
| MU2 | `sliding_step`: the interface old flux left at the interpolation (M4's projection skipped) | U1, S5 |
| MU3 | `face_map_from`: a boundary face mapped to the same index instead of the same position | K1 |
| MU4 | `rotor_swept_volumes`: all zero | K2, R1b |
| MU5 | `step_sliding`: the new volumes used as the old ones | U1 |

### The gate

`cargo fmt` per package, `cargo clippy --workspace --all-targets -- -D warnings`, and every test in release on a
`git archive` copy of the gated commit, one process per test, three jobs (`scripts/gate_test.py`), in the target dir
`E:/projects/cfd/w49i-work/target-f250-gate`, taken in turn through the machine's gate lock, with the free physical memory
at the start disclosed. A failure of a known flaky test is run again alone on the same build and reported with its name.

### Classes

D-R and D-S are preconditions of landing: a miss means the change is not merged and the cause is recorded. A miss of any
other limit is reported with its number and kept as an ignored `*_known_miss` test with the limit intact. A set-up check
(S6, R0, T4, B0, X0) or a negative control (F1n, N1, N2, N3) that does not hold, or a mutation that does not make its check
fail, means the check is not shown to be able to fail: it is reported as a defective set-up, corrected by an amendment
before any re-run, and never counted as a pass.

**Decision rule.** The acceptance checks are S1 (A1), R1 and R2 (A2), T1 to T3 (A3) and D-R, D-S (A4). Every registered
check met, the set-up checks and negative controls held and every mutation failed its tests: the acceptance as written is
met in the library, but no case can express a sliding interface yet (S4); F-250 is set **partial** with the note that
the library meets the acceptance and the case-tree wiring is open (Q1, Q6). A miss of any acceptance check: partial with
the miss named. The sliding loop does not run at all: todo.

## Not delivered

The case-tree, CLI and Studio wiring (Q1); a planar (translational) sliding interface and a partly covering interface
such as a sector with cyclic seams (Q3); conical or other non-cylindrical surfaces of revolution; more than one sliding
interface or rotor; BDF2, turbulence, energy, scalars, a face viscosity, body forces, sources or jumps on the sliding path
(refused by `step_sliding`); the compressible solver; MRF zones combined with a sliding interface; a remap of the old
Rhie-Chow defect onto new interface faces (Q5); parallel runs. The first-order interpolation of the azimuthal velocity
between cells at different angles (ADR 0109) is unchanged.

## Open questions and choices for the maintainer

- **Q1. The case-tree wiring.** Not in this registration. Options: (a) a follow-up registration: an optional table (for
  example `[mesh.sliding]` with the rotor block, the stator block, the patches and the angular velocity) or keys with
  `.hash_omit_default()`, a generator for a rotor-stator annulus pair, and the transient loop in `sim.rs` calling the S2/S3
  path, with a tree run equal to the library run and a case without the table bit-identical; (b) keep the library only.
  Recommendation (a), as a new roadmap item split from F-250 (the maintainer's decision; no item exists for it).
- **Q2. The reading of A2.** Options: (a) as registered: reading A against static conforming controls (R1) is the
  acceptance, with the rotating controls (R1b) and the literal reading B (R2) beside it; (b) the rotating controls as the
  acceptance; (c) reading B only. Recommendation (a).
- **Q3. Planar and sector sliding.** A translational slide needs a planar chart with a period in the join and cyclic
  pairs that the join carries over (read 1: the join drops periodic information); a sector rotor needs the same for
  rotational pairs. Options: (a) a separate item; (b) extend F-250. Recommendation (a).
- **Q4. The interface mesh flux.** Options: (a) the area-weighted minimum-norm closure of the GCL (M3, as registered:
  exact GCL, a small radial interface velocity of the order of the sliver change); (b) no interface mesh flux (the GCL
  error of the slivers stays; N1 measures it); (c) a geometry without slivers (not possible with planar faces on a
  cylinder). Recommendation (a).
- **Q5. The old Rhie-Chow defect on new interface faces.** Options: (a) zero (M4, as registered: the interface faces lose
  the transient Rhie-Chow memory of one step); (b) the area-weighted mean of the defects of the old overlaps of the same
  rotor face. Recommendation (a) unless a time-step dependence of the steady state is found at the interface.
- **Q6. Status of F-250.** With every check met: options (a) partial until the tree can run a sliding case (the decision
  rule); (b) done, with the wiring as its own item. Recommendation (a): unlike F-257, whose wiring is tracked by F-259, no
  item tracks this wiring yet. The roadmap follows the decision rule until the maintainer decides.

## Amendment 1 (2026-11-07, written after the registered run of K1 to K4 and before any flow check ran)

The registered run of the unit tests K1 to K4 (once, release) gave: K1 exact at all four steps, K3 `5.2e-15` at worst,
K4 every refusal with its word, K2 (ii) `2.53e-14` over 1008 radial faces and K2 (iii) no non-zero value; and a **miss of
K2 (i)**: the largest `abs(sum_f sigma_f W_f) / V_c` over the rotor cells without an interface face is `1.07e-14`
against `1e-14`. The records R1 to R3 (D-R) were run once after the code, before K1 to K4: equal.

Reading of the miss (not a diagnosis yet; the check below decides it): ADR 0033's identity is `sum_f sigma_f W_f = V1 -
V0` for the two point sets the swept volume is given, here the mesh `M1` and `M1` turned back by `theta1 - theta0`. A
rotation in floating point keeps a cell's volume only to the round-off of the turned points (about `1e-16 |x|` against
cell sizes of 0.04 to 0.06, so `1e-15` to `1e-14` of the volume), so `V1 - V_back` is of that size, not zero, and K2 (i)
measured it against zero. K2 (i) stays as registered, as an ignored `*_known_miss` test with the measured number.

Added check, registered here before it runs:

| Id | Check | Limit |
| --- | --- | --- |
| K2c | P from 0.0123 to 0.0173 rad, every rotor cell without an interface face: `abs(sum_f sigma_f W_f - (V1_c - V_back,c)) / V_c`, `V_back` the cell volume of `M1` with all points turned back by `theta0 - theta1` (`motion::moved_mesh`, the same rotation as `rotor_swept_volumes`), the mesh the swept volumes are computed from; printed with it, no limit, the largest `abs(V1_c - V_back,c) / V_c` | `<= 1e-14` |

If K2c holds and `abs(V1_c - V_back,c) / V_c` is of the size of the K2 (i) value, the miss is the round-off of the
rotation and not of the swept volume. The flow checks' GCL limit (`1e-12 mean(V)`, S4 and the others) is not affected.

## Amendment 2 (2026-11-07, written after the registered run of the flow checks and before any corrected run)

The registered run of `sliding_interface_f250.rs` (once, release, one thread, 75 s) met F1, F1n, S2, S4, N3, U1 to U4, N1,
N2, R0, R1, R2, R3 of RR=, T1 to T4, B0, D1, D2, E1, E2 and X0 in every set-up, and missed:

| Id | Limit | Measured |
| --- | --- | --- |
| S1 (acceptance A1) | `<= 1e-10` | `1.51e-10` |
| S3 | `<= 1e-9` | `1.78e-9` |
| S5 | `<= 1e-9` | `7.77e-3` |
| S6 (set-up) | every step | 58 of 100 steps |
| T5 | net `<= 1e-10`, GCL `<= 1e-12`, continuity `<= 1e-9` | `8.58e-11`, `1.20e-14`, **`4.21e-9`** |
| R1b | `<= 2.153e-5` (1.5 x the rotating controls' `1.435e-5`) | `5.939e-4` |
| R3 of RR | net `<= 1e-10`, GCL `<= 1e-12`, continuity `<= 1e-9` | `6.20e-11`, `1.38e-14`, **`1.42e-8`** |
| D3 | GCL `<= 1e-12`, continuity `<= 1e-9` | `1.20e-14`, **`1.67e-8`** |

All results go in the addendum. Four diagnostics were run after it (scratch tests, not committed; disclosed here):

- (a) SF, steps 1 to 6, the largest cell divergence of the remapped old flux and where: step 1 `7.77e-3` (relative to
  `max abs(S_f)`) at cell 2016, a stator cell at `r = 1.968` that has no interface face, where the old flux is the start
  flux of `PimpleSolver::new` (the face interpolation of the start velocity, divergence `8.1e-5`), which M4 does not
  touch; steps 2 to 6 at most `2.5e-12`. S5 as registered measures the start flux in its first step: a defective set-up.
- (b) TC, 60 steps, with 0, 2 and 6 non-orthogonal correctors, sliding and on the static joined P at 0.0123 rad: the
  largest `continuity` sliding `2.8e-13`, `3.8e-9`, `3.1e-13`, static `1.9e-10` (2), `7.6e-14` (6); the largest net
  interface flux sliding `7.6e-11` (2) and `1.7e-15` (6). The continuity, S1 and T5 misses are the lag of the explicit
  non-orthogonal flux: the final flux carries the non-orthogonal term of the last pressure, which the last solved equation
  did not hold, so its divergence is the change of that term over the last corrector. The sliding mesh makes the pressure
  correction of a step near the interface about 20 times larger than on the static non-conforming mesh. With 2
  correctors on these 49-degree interface faces the registered set-up does not meet the precondition of the continuity
  measure (a converged non-orthogonal correction).
- (c) S6: the expectation "every step" was wrong. A rotor vertex at `2 pi i / 144 + theta` meets a stator vertex at `2 pi j
  / 120` when `theta = 2 pi (6 j - 5 i) / 720`, so the owner-neighbour pairs of the interface change only at the angles
  `m 2 pi / 720`: 57 of them in `(0, 0.5]`, at most one per step (`0.00873 > Omega dt = 0.005`), none on a step end
  (`m pi / 360 = 0.005 n` has no integer solution), plus the first step from the coincident vertices at angle 0: 58.
  S6 is a defective set-up check.
- (d) RR, the cell of the largest error: sliding `5.939e-4` at `r = 1.968` (the outermost stator cell, next to the static
  stator wall; the rotor cells at most `5.03e-4`), the static control C120 `5.935e-4` at the same radius. The rotating
  controls have no static wall (both walls follow the mesh with ADR 0033's fixed-flux pressure gradient). R1b's miss is
  the zero-normal-gradient pressure condition of the static stator wall, which the sliding case shares with the static
  controls (R1 and R2 against them are met), not the interface.

The registered checks stay as they are and the missed ones become ignored `*_known_miss` tests with their limits
(S1, S3, S5, S6, T5, R1b, R3 of RR, D3). Corrected checks, registered here before they run, each run once:

| Id | Set-up | Check | Limit |
| --- | --- | --- | --- |
| S5c | SF | every step from the second, S5's measure | `<= 1e-9` |
| S6c (set-up) | SF | the number of steps in which the owner-neighbour pairs of the interface change | 58 exactly |
| (all of S) | SF6 | S1, S2, S3, S4, S5c, S6c, N3 and X0 | as registered |
| (all of T) | TC6 | T1 to T5 and X0 | as registered |
| (R) | RR6 | R1, R3 and X0 | as registered |
| (all of D) | DM6 | D1 to D3 and X0 | as registered |

SF6, TC6, RR6 and DM6 are SF, TC, RR and DM with `non_orth_correctors = 6` in every solver of the set-up (the controls
too), the count at which diagnostic (b) gave a continuity of `3e-13`; nothing else changes. R1b is not re-checked on RR6
(its cause, (d), does not depend on the correctors). The decision rule is unchanged: A1 is met if S1 holds on SF6, and the
roadmap status stays partial (Q6).
