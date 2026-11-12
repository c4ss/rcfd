# ADR 0255: F-357 registration: the uniform flow U1 on a warped three-dimensional overset mesh with the component moving

Status: **REGISTERED 2026-11-12**, written before any code of F-357 and before any run of its checks. Not yet accepted by the
maintainer. Before this text no check was run and nothing was compiled: the code and the documents listed under "What was
read" were read, and the arithmetic under "Arithmetic before this registration" was done by hand. Results go in the
addendum at the end; nothing above the addendum is changed after a run.

Parents: [ADR 0219](0219-f134-registration-overset-coupling-of-a-moving-component-mesh.md) (F-134: `OversetSolver`, the
uniform-flow checks U1 to U4 and the negative control N2, the set-up U and how it moves the component),
[ADR 0251](0251-f336-registration-overset-quadratic-donor-interpolation.md) (F-336: the quadratic donor interpolation
opt-in, the set-ups B3 and B3d, the uniform-flow check A3 on U) and
[ADR 0253](0253-f352-registration-overset-donor-search-on-non-planar-faces.md) (F-352: the donor search on cells with
non-planar faces, B3d; its result is kinematic, no flow was run; its question Q3 is this item). Roadmap item F-357
(depends on F-352).

Number: the lead gave 0255 to F-357. Branch `f357-warped-overset-uniform-flow` from main 9a52283a, worktree
`w49i-work/f357`.

House rules (ADR 0130, ADR 0213, ADR 0219, ADR 0251, ADR 0253):

- limits are fixed here, before the code and the runs, and are not changed after a run; the tests and ignored records of
  F-134, F-314, F-334, F-335, F-336, F-338 and F-352 are not edited;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact and the number recorded, and the cause is
  diagnosed, not tuned away;
- a defective set-up gets a new corrected test by an amendment written and committed before the corrected run, and the old
  one stays as an ignored record;
- defaults stay bit-identical: this is a checking item and is expected to add a test file and this text only; library code
  is changed only if a check fails and its cause is found, and then only as an opt-in or as a change provably identical on
  planar meshes (every existing overset and moving-mesh test passing unedited); otherwise the miss and its cause are
  recorded and the library is not changed;
- `crates/rcfd-api/tests/baseline_hashes.rs`, the hash tables and the goldens are not edited or re-recorded;
- each registered run is made once, and nothing is re-run to pick a result;
- no new third-party crate, no unsafe code, no commit trailers, no push.

Clean-room. Nothing outside this repository was opened for this work. The argument below uses only the discrete
identities of the library's own finite-volume geometry, derived here.

## What F-357 asks

The roadmap entry (acceptance, quoted, binding): "The uniform-flow check U1 of ADR 0219 on the set-up B3d of ADR 0251 with
the component moving: velocity and pressure within 1e-10, continuity of the calculated cells within 1e-9 at every step,
with the default and with the quadratic donor interpolation."

**Reading (registered here).**

1. "U1 of ADR 0219": every step, over the cells that are not holes, `max abs(u_c - U) / abs(U)` and `max abs(p_c) /
   abs(U)^2`, each `<= 1e-10`, with the step, the solver settings PS and the mass mode (conservative) of ADR 0219.
2. "On the set-up B3d of ADR 0251 with the component moving": the two warped hexahedral parts of B3d (the same code as
   `crates/rcfd-mesh/tests/overset_f352.rs`), the component starting at the pose of ADR 0251 and moving rigidly (turning
   and translating) every step, the hole and the fringe rebuilt every step by `OversetSolver::step`. The motion is fixed
   below (set-up W3).
3. "Continuity of the calculated cells within 1e-9 at every step": `OversetInfo::continuity` (the largest net outflow of a
   calculated cell over the largest face area), the measure of U3 of ADR 0219, every step `<= 1e-9`.
4. "With the default and with the quadratic donor interpolation": two runs of the same set-up, the second with
   `Overset::set_donor_interpolation(DonorInterpolation::Quadratic)` before the solver is built.

## What was read (no run)

- ADR 0253 in full; ADR 0251: the set-ups B3, B3d, B3s, the check A3, amendment 1, the questions; ADR 0219: the model M5 to
  M10, the set-ups (PS, U, TG_L), the checks U1 to U4, N2 to N4, the mutations, the addendum.
- `crates/rcfd-fv/src/overset.rs` in full; `crates/rcfd-fv/src/ale.rs` (`move_mesh`, `follow_velocity`);
  `crates/rcfd-mesh/src/motion.rs` (`Pose`, `swept_volumes_about` and its module text, the unit test of the GCL on warped
  faces); `rcfd_mesh::mesh::polygon_geometry`; `rcfd_mesh::overset::quadratic_weights` (its head);
  `crates/rcfd-fv/tests/overset_f134.rs` (the set-ups and the U checks), `crates/rcfd-fv/tests/overset_f336.rs` (A3);
  `crates/rcfd-mesh/tests/overset_f352.rs` (the set-ups, B3d).

## Why a uniform flow should stay uniform on a warped moving mesh (derivation)

Take `u = U` in every non-hole cell, `p = 0`, `phi_f = U . S_f`. The step keeps this state when, per calculated cell:

1. **The face area vectors close**: `sum_f sigma_f S_f = 0`. `polygon_geometry` gives `S_f` as half the sum of the cross
   products of the triangles about the mean point of the face, which is the vector area `(1/2) sum_i p_i x p_{i+1}` of the
   closed loop of its points whatever the warp (the term of the mean point cancels around the loop). The faces of a cell
   form a closed surface in which every edge is traversed once in each direction, so the sum is zero in exact arithmetic
   for warped faces as for planar ones. Then the divergence of `U . S_f` is zero, Green-Gauss gives a zero gradient of a
   uniform field, and the diffusion of a uniform field is zero (the face differences vanish and the corrections act on a
   zero gradient).
2. **The geometric conservation law holds**: `sum_f sigma_f W_f = V1 - V0`. The module text of `rcfd_mesh::motion` derives
   it for warped faces (the side quads are split along the same diagonal from both faces of an edge, the caps use the
   `fc`, `S` of `polygon_geometry`), and the unit test `rotation_and_warped_faces_satisfy_the_gcl_and_origin_independence`
   checks it to `1e-13` of the mean volume on a warped mesh under a rotation. For a rigid motion `V1 = V0` as well. Then
   the transient term with `AleStep::fix_ddt` and the convection by the relative flux `phi - W` cancel for a uniform `u`.
3. **The stencil weights sum to one**: the linear (M4) and the quadratic (ADR 0251) weights sum to one to round-off (ADR
   0253 A2, A3 measured `7.8e-16` and `2.0e-15` on B3d), so the fringe values, `rAU` and `HbyA` of a uniform field are
   that field.
4. **The pressure stays zero**: with 1 to 3 the right-hand side of the pressure equation vanishes to round-off, and `p =
   0` satisfies the condition `p = 0` at the outflow.

So the uniform state is exact to round-off and the solver tolerances (`tol_rel = 1e-12`, `tol_abs = 1e-15`), as on the
planar set-up U (ADR 0219 measured `1.4e-14` and `6.2e-13`). Warping enters only through the round-off of these identities.
A failure would therefore point at one of them, or at a path of the step that a uniform field reaches through something
else than these identities (the flux of faces that change kind, M9).

**A remark on the negative control the lead suggested** (the check failing when the fringe refresh is skipped). A uniform
field is insensitive to *which* cells a stencil uses as long as its weights sum to one: a stale stencil from the pose before
interpolates a uniform field exactly. Skipping the rebuild of the connectivity from the test side therefore does not move
U1. What a uniform flow does see is the refresh of the **flux** of the faces that change kind (M9: a face that was between
a hole and a fringe cell carries the flux of the interpolated rigid velocity; when its cells become fringe and calculated
the flux must be rebuilt from the new values). That refresh is inside `advance` and cannot be skipped from a test, so it is
registered as the mutation MU3. The negative controls that the test itself can apply are the weights (N1, as N2 of ADR
0219) and the mesh flux (N2).

## Arithmetic before this registration (disclosed, by hand)

- B3d as in ADR 0251 and ADR 0253: background `24^3` hexahedra of `0.1` on `[0, 2.4]^3`, component `14^3` of `0.1` on
  `[0, 1.4]^3`, interior points of both displaced by up to `0.015` (`0.15 h`) in each direction, warp up to `0.142 h`
  (measured by ADR 0253). The pose of ADR 0251: rotation by `0.3` about `(1, 2, 3) / sqrt(14)` through `(0.7, 0.7, 0.7)`,
  translation `(0.53, 0.52, 0.49)`, so the centre is at `(1.23, 1.22, 1.19)`.
- The half extent of the turned component along a coordinate is `0.7 sum_j abs(R_ij)`. By Rodrigues' formula with `k =
  (0.2673, 0.5345, 0.8018)`: at the angle `0.3` the row sums are `1.357, 1.271, 1.231` (half extents `0.950, 0.890,
  0.861`); at `0.4` they are `1.453, 1.337, 1.301` (`1.017, 0.936, 0.911`). They grow monotonically in between.
- The motion (W3 below): angle `0.3 + 0.5 t`, translation `(0.53, 0.52, 0.49) + (-0.25, 0.25, 0.25) t`, `dt = 0.005`, 40
  steps, so `t` from 0 to `0.2`: angle `0.3` to `0.4`, centre from `(1.23, 1.22, 1.19)` to `(1.18, 1.27, 1.24)`. The
  component's box then spans at most x `[0.16, 2.20]`, y `[0.28, 2.21]`, z `[0.28, 2.15]` over the run (end values with
  the larger extents, start values for the other side): at least `0.16`, more than one background cell, from the
  background boundary, so every component fringe centre has a background donor.
- Per step the component moves by `0.0022` (translation) plus at most `0.0025 * 1.21 = 0.003` (rotation at a corner, half
  diagonal `0.7 sqrt(3)`), at most `0.06 h`; a hole cell cannot be uncovered in one step (the fringe layer is one cell).
- Courant number: `abs(U) dt / h = 1.146 * 0.005 / 0.1 = 0.057` with `U = (1, 0.5, 0.25)`; the mesh speed at a component
  corner is at most `0.43 + 0.5 * 1.21 = 1.04`, so the relative Courant number stays below `0.12`.
- Kind changes (the set-up check U4): the hole of the background is the turned cube of side `0.3` (`|x_ref - 0.7| <
  0.15`); its translation `0.087` over the run along `(-1, 1, 1)` sweeps about `0.156 * 0.087 = 0.0135` of volume (the
  projected area of the cube along that direction is about `0.3^2 sqrt(3)`), about 13 cells entering and 13 leaving the
  hole, and as many changing between fringe and calculated at the outer side of the fringe: about 50 changes, plus the
  rotation. ADR 0219's limit `>= 20` is kept.
- Cost: B3d has 16568 cells; a step is a move of the mesh, a connectivity (`5.9 ms` measured by ADR 0253), three momentum
  solves and two pressure correctors with an AMG build and the offset solve: I expect `0.2` to `1` s for each step, so `10`
  to `40` s for each run of 40 steps; eight runs below.

## Set-ups

PS as ADR 0219: `PimpleSettings::piso(nu)` with central convection, two correctors, no non-orthogonal correctors (the
overset solver refuses them), implicit Euler, `ddt_correction` on, momentum and pressure controls `tol_abs = 1e-15`,
`tol_rel = 1e-12`, at most 3000 iterations. `nu = 0.01`. `U = (1, 0.5, 0.25)`. Start `u = U`, `p = 0`. `dt = 0.005`, 40
steps. "Every step" means the largest value over the 40 steps. Mass mode conservative.

The motion `P(t)`: rotation by `0.3 + 0.5 t` about the axis `(1, 2, 3)` through `(0.7, 0.7, 0.7)`, then the translation
`(0.53 - 0.25 t, 0.52 + 0.25 t, 0.49 + 0.25 t)`; `P(0)` is the pose of ADR 0251. Step `k` goes from `P((k - 1) dt)` to
`P(k dt)`.

- **W3** (the acceptance set-up): B3d (`b3(true)` of `overset_f352.rs`, copied), the composite at `P(0)`, `OversetSolver`
  with the coupling, `step(dt, P(k dt), &[])` for `k = 1..40`. Background patches `xmin`, `ymin`, `ymax`, `zmin`, `zmax`:
  `u = U`; `xmax`: `p = 0`, velocity zero gradient. **W3q**: the same with the quadratic donor interpolation.
- **P3** (the planar reference): B3 (`b3(false)`, planar faces), otherwise as W3, default interpolation.
- **M0** (the single warped mesh at rest): the warped component mesh of B3d alone (no overset, every cell calculated),
  placed at `P(0)` and left there: `OversetSolver` without coupling, each step `advance_mesh(dt, move_mesh(geom, the same
  points, dt), [])`. Patches `xmin`, `ymin`, `ymax`, `zmin`, `zmax`: `u = U`; `xmax`: `p = 0` (the patches are those of the
  component's own box, turned with it; `U . n` on `xmax` stays positive for these angles).
- **M1** (the single warped mesh moving): M0 with the points moved rigidly to `P(k dt)` of the reference points every step.
- **MB** (the warped background at rest): the warped background of B3d alone, as M0 with the patches of W3.

## Registered checks

New test file `crates/rcfd-fv/tests/overset_f357.rs`, release (`cargo test --release -p rcfd-fv --test overset_f357`). No
existing test is edited.

### A. The acceptance

| Id | Check | Limit | Estimate |
| --- | --- | --- | --- |
| A1 | W3, every step, over the non-hole cells: velocity `max abs(u_c - U) / abs(U)`, pressure `max abs(p_c) / abs(U)^2` | `<= 1e-10` each | the derivation; U measured `1.4e-14`, `6.2e-13` |
| A1c | W3, every step: `OversetInfo::continuity` | `<= 1e-9` | U3 measured `1.6e-14` |
| A2 | W3q, the same as A1 | `<= 1e-10` each | the same; A3 of ADR 0251 on U |
| A2c | W3q, the same as A1c | `<= 1e-9` | the same |
| A2s (set-up) | W3q, every step: the share of the fringe cells whose stencil is quadratic (not a fallback); printed with it, no limit: the largest number of fallbacks of a step (the "fallback count") and the number of stencils | `>= 0.95` | ADR 0253 A3: no fallback on B3d at `P(0)` |

### U. The rest of ADR 0219's U checks, on W3 and W3q

| Id | Check | Limit | Estimate |
| --- | --- | --- | --- |
| U2 | every step, the GCL residual of the component cells `max_c abs(V1_c - V0_c - dt sum_f sigma_cf m_f) / mean(V)` | `<= 1e-12` | the unit test of `motion` has `1e-13` on a warped mesh |
| U3 | every step, `abs(Q_xmax - U_x A_xmax) / (U_x A_xmax)`, `A_xmax = 2.4^2` | `<= 1e-10` | U measured `2.4e-15` |
| U4 (set-up) | the number of cells whose kind changes, summed over the steps (W3) | `>= 20` | about 50 (arithmetic above) |
| U5 (set-up) | linear solves that did not converge, summed over the steps (`OversetInfo::unconverged`) | printed, no limit | |

### S. Separating checks: the single warped mesh, and the planar overset

Each with the limits of A1 and A1c (velocity and pressure `<= 1e-10` every step, continuity `<= 1e-9` every step); M1
also with U2.

| Id | Set-up | What it separates |
| --- | --- | --- |
| S0 | M0, the warped component mesh at rest | the finite-volume step on warped cells, without motion and without overset |
| S1 | M1, the warped component mesh moving | the swept volumes and the GCL of a warped moving mesh, without overset |
| SB | MB, the warped background at rest | the same as S0 on the other part |
| SP | P3, the planar B3 overset moving | the overset coupling in 3-D with the same motion, without warped faces |

Reading, fixed now: if A1 or A2 misses and S0, S1, SB are met, the cause is in the overset coupling on the warped mesh
(compare SP); if S0, S1 or SB misses, the cause is in the finite-volume geometry of the single warped mesh, and the miss is
reported with that number and not tuned.

### N. Negative controls

| Id | Check | Limit | Estimate |
| --- | --- | --- | --- |
| N1 | W3 with every stencil weight multiplied by `1.001` after `prepare` (N2 of ADR 0219): the largest velocity deviation | `>= 1e-6` | ADR 0219 measured N2 on U far above; a fringe value off by `1e-3 abs(U)` |
| N2 | W3 with the mesh flux of every internal face of the component perturbed after `prepare` by `1e-6 abs(S_f) j_f`, `j_f` the fixed pseudo-random number in `[-1, 1]` of the face centre (the `jitter` of the set-up, seed 9): the largest velocity deviation | `>= 1e-8` | a GCL error per cell of about `dt 6e-6 0.01 = 3e-10` of `V = 1e-3`, a velocity change of about `3e-7` per step |

### Mutations (scratch copy, not committed)

Each is applied alone to a `git archive` copy of the final tree in `E:/projects/cfd/w49i-work/f357-scratch` (own target
directory), the new test file is run once and the named check must fail; the numbers go in the addendum.

| Id | Mutation | Must fail |
| --- | --- | --- |
| MU1 | `rcfd_mesh::mesh::polygon_geometry`: for a polygon of four or more points the area vector keeps its length but takes the direction of the first fan triangle `(p_0, p_1, e)` (on a planar face the same direction up to round-off; on a warped face the area vectors of a cell no longer close) | A1 or S0 (a warped set-up) |
| MU2 | `rcfd_mesh::overset::quadratic_weights`: the first weight increased by `1e-6` (the weights no longer sum to one) | A2 |
| MU3 | `rcfd_fv::overset::OversetSolver::advance`, M9: the flux of the faces that do not keep their flux is not rebuilt from the refreshed values (the "fringe refresh" skipped) | A1 or A2 |

`scripts/limit_audit.py` is run on the new test file and every hit is read.

### Classes

A miss of A1, A1c, A2, A2c, U2, U3 or a separating check is reported with its number and kept as an ignored
`*_known_miss` test with the limit intact; its cause is diagnosed by the separating checks and, if needed, a diagnostic
run disclosed as such. A miss of A2s or U4 is a defective set-up: corrected by an amendment before any corrected run. A
control (N1, N2) that does not hold, or a mutation that its check does not catch, is a defective set-up too, never
counted as a pass.

## Predictions (mine, before any run)

Every limit met. A1 and A2 near `1e-14` (velocity) and `1e-12` (pressure), continuity near `1e-14`, U2 near `1e-14`, U3
near `1e-14`; no fallback; U4 between 30 and 150. S0, S1, SB and SP near the same numbers. N1 near `1e-4`, N2 near
`1e-7`. MU1 makes the uniform flow fail on every warped set-up at about `1e-4` or more and leaves SP met; MU2 fails A2 near
`1e-6`; MU3 fails A1 and A2 at `1e-3` or more. I am least sure of MU3 (it needs a face that changes from a hole-fringe pair
to a fringe-calculated pair; the moving hole should give some every step).

## Decision rule

- **done**: A1, A1c, A2, A2c met; U2, U3, S0, S1, SB, SP met; A2s, U4 met; the controls held and the mutations were
  caught.
- **partial**: A1, A1c, A2, A2c met but another check missed with its cause diagnosed, or a control or mutation did not
  hold and was not corrected; or an acceptance check missed with its cause diagnosed and measured (the separating checks
  name the part that fails).
- **todo**: an acceptance check missed without a diagnosed cause, or the runs could not be made.

## Limits of this registration

- One warped mesh family (hexahedra with randomly displaced interior points), one motion, 40 steps; uniform flow only: it
  shows that the warped geometry and the coupling keep a uniform state, not the accuracy of a non-uniform flow on warped
  cells (no Taylor-Green or order check in 3-D).
- No non-orthogonal correctors (the overset solver refuses them); a uniform field does not reach them anyway.
- The background does not move; the component moves rigidly, so its cells keep their volumes and warps.

## Open questions

Listed with options and a recommendation in the addendum, after the results.
