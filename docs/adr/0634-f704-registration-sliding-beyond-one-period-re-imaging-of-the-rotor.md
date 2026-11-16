# ADR 0634: F-704 registration: sector and planar sliding beyond one period (re-imaging of the rotor)

Status: **REGISTERED 2026-11-16, written before any code of F-704 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-704 was written and nothing was run; the code of the base was read
only. Results go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-704 (deps F-304), proposed by [ADR 0629](0629-f304-registration-planar-and-sector-sliding-interfaces.md) (F-304)
question Q3 and its "not covered" (offsets within one period). Parents: ADR 0629 (the periodic join, M1 to M7, its set-ups PP,
SS, S3 and flows PF, PU, SF, SU), [ADR 0216](0216-f250-registration-sliding-interface-on-a-cylinder.md) (F-250: the sliding
interface on the full annulus, its pair P, its flow SF and SF6, its loop SL). Number 0634 given by the lead. Branch
`f704-work` from 7814deab (the F-304 tip), worktree `w49i-work/f704`, scratch `w49i-work/f704-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number); each registered run made once; existing tests not edited;
defaults bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new
dependency, no unsafe code; clean-room. The roadmap files are not edited. Every choice that the maintainer would normally be
asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation), except where listed as
open.

Clean-room: the method follows from the definition of a periodic image (ADR 0629 M1: a point `x` and `T x` are one point of
the periodic domain) and of a periodic vector field under a rotation (`v(T x) = R v(x)`, `R` the linear part of `T`). No source
was read and no vendor code, text or install was used. Reference values are closed forms or the repository's own full-annulus
run of ADR 0216.

## What F-704 asks

The acceptance, binding:

> Every limit not stated here is registered in the ADR before the run. A rotor sector turning at least three sector pitches and a planar slider at least three periods, re-imaged by one period whenever the offset reaches it, conserve mass and a uniform flow to round-off in every step including the re-imaging steps; the radial source flow on the sector against the full-annulus sliding run of ADR 0216 within a registered limit; defaults bit-identical.

Clauses:

- **c1** a rotor sector turning at least three pitches, re-imaged whenever its offset reaches one pitch, conserves mass to
  round-off in every step, the re-imaging steps included;
- **c2** the same sector keeps a uniform flow to round-off in every step;
- **c3** a planar slider moving at least three periods, re-imaged likewise, conserves mass to round-off in every step;
- **c4** the same slider keeps a uniform flow to round-off in every step;
- **c5** the radial source flow on the sector against the full-annulus sliding run of ADR 0216 within a registered limit;
- **c6** defaults bit-identical (the F-304 tests unchanged and passing, no case key, existing paths unchanged).

## What the code read shows (base 7814deab, read only)

1. `PeriodicSlidingInterface::mesh_at(offset)` refuses `offset` outside `[0, P)` (`P = |t|` or the pitch `Theta`) and always
   builds the joined mesh from the rotor's reference points moved by `offset`. `PeriodicSlidingMesh` has public `mesh`,
   `report`, `offset` and private fields, so it is built only inside `rcfd-mesh`.
2. `PeriodicSlidingMesh::face_map_from(old)` maps internal faces off the interface by `(owner, neighbour)` and the boundary
   faces of the original patches by position; it does not look at the offset. `rotor_swept_volumes(old_offset)` moves the
   new points back by `old_offset - offset` (any finite value) and takes ADR 0033's swept volumes, zero off the rotor faces.
3. `rcfd_fv::sliding_periodic::periodic_sliding_step(old_geom, old, new, phi_old, u_old, dt)`: mesh flux from
   `new.rotor_swept_volumes(old.offset)` closed on the graph of the direct faces and the wrap pairs; the old flux kept on mapped
   faces, `(u_old)_f . S_f` on the direct faces (`face_flux` on the new geometry), the weighted wrap interpolation with the
   stator vector turned by `wrap_rotation()`, then the same closure; the old Rhie-Chow defect from `face_flux` on the old
   geometry. `PeriodicSlidingStep` has public fields and is built only by that function (the F-304 tests alter fields of a
   built step; none builds one literally).
4. `PimpleSolver::step_sliding_periodic` calls the private `sliding_advance` (also behind `step_sliding`), which checks the
   step, then sets `phi`, `phi_hist`, `geom`, `ale` and calls `step`. The per-cell vector state of the solver is `u.values` and
   the three component histories `hist_u[k]` (`old`, and `older`, `oldest`, which `History::push` fills after every step and
   which the Euler path reads where the convection extrapolation is set); `p` is a scalar; `fs_q` is cleared by
   `sliding_advance`.
5. `rcfd_mesh::sliding::SlidingInterface::mesh_at(angle)` (the full annulus of ADR 0216) takes any finite angle.
6. `annulus_sector(24, 8, 1, 1.5, pi/3)` places its nodes at `r (cos, sin)(pi/3 . i/24)`, `cylinder_ogrid(144, 8, 1, 1.5)` at
   `r (cos, sin)(2 pi i/144)`, with the same radial nodes: the sector SS of ADR 0629 is the first sixth of ADR 0216's pair P (the
   stator `annulus_sector(20, ...)` of `cylinder_ogrid(120, ...)` likewise), node for node up to the rounding of the angles.

## The model (derived)

**R1. The image of the rotor.** Write the rotor's motion as the turned offset `tau >= 0` (a length along `t`, or an angle). With
`k = floor(tau / P)` and the reduced offset `r = tau - k P` in `[0, P)`, the rotor at `tau` is the image under `T^k` of the
rotor at `r`: every rotor point `x(tau) = T^k x(r)`, because the two motions differ by `k` whole periods (translation by `k t`,
or rotation by `k Theta`, about the same axis). In the periodic domain `x` and `T^{-k} x` are one point, so the joined mesh at
`tau` is the joined mesh of ADR 0629 at `r`: the same cells with the same indices (the rotor's cells are always built from its
reference points), the stator untouched. Only the representation of the rotor's state changes: a scalar (pressure) is the same
at a point and its image; a vector field periodic under `T` satisfies `v(T x) = R v(x)`, so the rotor cell `c` that was
represented at `T^{k_0} x` with the velocity `u_c` is represented at `T^{k_1} x` with `R^{k_1 - k_0} u_c`; here the image only
goes back (`k_1 = k_0 + 1` when the turned offset passes a whole period), so `u_c -> R^{-1} u_c` (rotation by `-Theta` about
the axis; identity for a translation). A face flux `u . S` is invariant (both vectors turn). The old cell volumes are invariant
(a rigid motion), up to the rounding of the image's points.

**R2. The re-imaging step.** A step from `tau_0` (image `k_0`, reduced `r_0`) to `tau_1 = tau_0 + d` (image `k_1`, reduced
`r_1`); `d = V dt` or `Omega dt` with `0 < d < P`, so `k_1 - k_0` is `0` or `1`. With `k_1 = k_0` it is the step of ADR 0629,
unchanged. With `k_1 = k_0 + 1` (a **re-imaging step**) the old mesh is taken in the new image: the old rotor at `r_0` moved
back by one period, i.e. the rotor at `r_0 - P`, and:

- face identity: as ADR 0629 M4 (`face_map_from` does not depend on the offset; the rotor's internal faces and the original
  boundary faces keep their indices; the direct and wrap faces are new);
- mesh flux (M5): the rotor's swept volumes of the motion from `r_0 - P` to `r_1`, which is the rotation (translation) by
  `r_1 - r_0 + P = d`, the same motion as a step inside one image: `new.rotor_swept_volumes(old.offset - P)`; then the closure
  of ADR 0629 M5 unchanged (it also takes up the rounding of the image's volumes);
- old velocity: the rotor cells' old velocity in the new image, `R^{-1} u_c` for `c` a rotor cell, the stator's unchanged; the
  interpolations of ADR 0629 M6 on the new faces (direct faces and wrap pairs) use these vectors;
- old Rhie-Chow defect on the mapped internal faces: computed on the old geometry with the old vectors, as now (`u . S` and the
  defect are invariant under the joint rotation of a rotor face and its cells' vectors, R1);
- old flux on the mapped faces: kept (invariant, R1);
- the solver: before the step, the rotor cells' velocity and every stored level of its history (`u.values`, `hist_u[k].old`,
  `older`, `oldest`) turned by `R^{-1}`; the pressure and the face flux unchanged.

For a translation `R = I`: the re-imaging step differs from a step inside one image only in the reduced offsets and the swept
volumes' old offset `r_0 - P`; no vector turns.

**R3. The uniform flow and mass arguments carry over.** A uniform `U` invariant under `T` (any `U` for a translation, an axial
`U` for a rotation) satisfies `R^{-1} U = U`, so the re-imaged state is the uniform state again, and ADR 0629 M6's argument
holds at a re-imaging step as at any other. The radial source flow `U_in (r_i / r) x_hat` is a periodic vector field
(`v(R x) = R v(x)`), so its re-imaged state is the same field at the image points. Mass: ADR 0629 M7 holds on every step; the
re-imaging only relabels the image.

**R4. Without re-imaging** (the negative controls): building a step from `r_0` to `r_1 = r_0 + d - P` as a step inside one image
sweeps the rotor's faces back by `P - d` within one step (a mesh flux `(P - d) / d` times the true one, of the opposite sign),
and leaving the vectors in the old image turns the rotor's velocity by `+Theta` against the field it represents.

## Settings and code (additive)

**S1 (`rcfd-mesh`, `sliding_periodic.rs`).** `PeriodicSlidingMesh` gets a public field `image: i64` (the image `k`; `mesh_at`
sets `0`) and `turned()` (`offset + image P`). New `PeriodicSlidingInterface::reduce(turned) -> Result<(i64, f64)>` (`k =
floor(turned / P)`, `r = turned - k P`, corrected by one period when rounding puts `r` outside `[0, P)`; refused for a
negative or non-finite `turned`) and `mesh_turned(turned) -> Result<PeriodicSlidingMesh>` (`mesh_at(r)` with `image = k`).
`mesh_at`, the join, `face_map_from`, `rotor_swept_volumes`: not edited.

**S2 (`rcfd-fv`, `sliding_periodic.rs`).** `periodic_sliding_step` takes `dk = new.image - old.image`, refuses `dk` outside
`{0, 1}` (message naming `image`), and for `dk = 1` applies R2: swept volumes from `old.offset - P`, the rotor cells' vectors
turned by `R^{-1}` for the new-face interpolations, the defect from the old vectors on the old geometry. For `dk = 0` the
operations are those of F-304 in the same order (the same bits). `PeriodicSlidingStep` gets public `reimage:
Option<Reimage>`, `Reimage { n_rotor_cells, rotation }` (`Some` only on a re-imaging step of a sector; `None` for a translation,
where `R = I`).

**S3 (`rcfd-fv`, `pimple/moving.rs`).** `sliding_advance` gets an argument `reimage: Option<&Reimage>` (`None` from
`step_sliding`); after its checks pass (one more: `n_rotor_cells <= n_cells`) and before the step, it turns `u.values` and the
stored history levels of the rotor cells (R2). `step_sliding_periodic` passes `step.reimage`. Nothing else is edited.

**S4.** No case-tree, CLI, Studio or schema change (F-303). `docs/dynamic-mesh.md` gets prose.

## Set-ups

Solver settings PS6 of ADR 0629 (six non-orthogonal correctors, central convection, implicit Euler, `ddt_correction` on,
solvers `tol_abs = 1e-16`, `tol_rel = 1e-13`, 5000 iterations; the solver tolerance is `tol_rel = 1e-13`). Loop: ADR 0629's
loop with `mesh_turned(speed . n . dt)` in place of `mesh_at`. Pairs PP, SS, S3 of ADR 0629 unchanged.

Run lengths and steps (arithmetic before the run, disclosed, `f704-scratch/arith.py`):

- **PP** (planar, period 1, slider speed `V = 1`): `dt = 0.00493`, **620 steps**, turned offset 3.0566 (3.06 periods). The
  F-304 step 0.0049 cannot be kept: a slider node meets a stator node at offsets `m / 80`, and `0.0049 n = j / 80` at every
  multiple of 125 steps (exact coincidences at steps 125 to 500). With `0.00493 . 80 = 493 / 1250` a meeting needs a multiple of
  1250 steps; the closest approach over the run is `1.0e-5` (relative `2e-4` of a slider cell; the join's merge tolerance is
  `1e-8`). Re-imaging steps (turned offset passes 1, 2, 3): **203, 406, 609**, reduced offsets after them `7.9e-4`, `1.58e-3`,
  `2.37e-3`.
- **SS** (sector, pitch `pi/3`, `Omega = 1`): `dt = 0.005` (ADR 0216's step, so that F can run in lockstep), **640 steps**,
  3.2 rad (3.06 pitches). Re-imaging steps **210, 419, 629** (reduced offsets `2.80e-3`, `6.05e-4`, `3.41e-3`); closest
  approach to a node meeting (multiples of `pi/360`) `1.0e-5` rad.
- **S3** (3-D sector, pitch `pi/3`, `Omega = 1`): `dt = 0.01`, **320 steps**, 3.2 rad. Re-imaging steps **105, 210, 315**;
  closest approach to a node meeting (multiples of `pi/180`) `2.0e-5` rad.
- **P** (ADR 0216's full annulus, `cylinder_ogrid(144, 8, 1, 1.5)` against `cylinder_ogrid(120, 8, 1.5, 2)`), flow SF6 of ADR
  0216 amendment 2 (the same PS6), `dt = 0.005`, 640 steps, through `step_sliding`.

Flows: PF, PU on PP; SF on SS; SU on S3; all as ADR 0629 (PF: inflow through the stator bottom `ymin`; SF: inflow through the
rotor's `inner`, `u_b = U_in x_hat_f` set every step at the new face centres; PU: `U = (1, 0.5, 0)`; SU: `U = (0, 0, 1)`).
SF6 on P as ADR 0216 (`cylinder` inlet, `farfield` outlet).

## Registered checks

All in release. "Every step" is the largest value over all steps of the run; each such check is also printed over the three
re-imaging steps alone, with the same limit (the acceptance says "including the re-imaging steps"). Measures as ADR 0629 (and
its amendment 1 for the sign of the planar interface flux: `Q_I` is the flux out of the rotor; in PF and PU the inflow enters
the stator).

### K. Turned offsets and the re-imaging step on the mesh (unit, `rcfd-mesh`)

| Id | Check | Limit |
| --- | --- | --- |
| K1 | `mesh_turned(t)` for `t` in `{0, 0.0123, 0.3, 0.9}` (PP) and `{0, 0.0123, 0.3, 1.0}` (SS): `image = 0`, and points, owners, neighbours, faces and patches equal to those of `mesh_at(t)` | bit for bit |
| K2 | `mesh_turned(t + k P)`, the same `t`, `k = 1, 2, 3`: `image = k`; `abs(offset - t) <= 1e-14 . k`; every rotor cell centre against the reference centre moved by the turned offset `t + k P` and then back by `k` periods | image exact; `1e-14 k` (rounding of `k P`); centres `<= 1e-12` (PP: absolute, period 1; SS: absolute, radius `<= 2`) |
| K3 | the re-imaging step, PP from turned `0.9975` to `1.0025`, SS from `pi/3 - 0.0025` to `pi/3 + 0.0025`: (i) the face map as ADR 0629 K1 (unmapped faces exactly the direct and wrap faces, no old face used twice); (ii) `rotor_swept_volumes(old.offset - P)` as ADR 0629 K2 (i), (ii) for a motion of `0.005`, and zero on the stator, direct and wrap faces | (i) exact; (ii) relative `1e-12`, zero exact |
| K4 | refusals: `reduce` / `mesh_turned` of `-0.1`, `NaN`, `inf` | `Err` each, the message naming `turned` |

### P. The planar slider over three periods (c3, c4)

| Id | Check | Limit |
| --- | --- | --- |
| P1 (c3) | PF, every step `abs(Q_I + Q_in) / Q_in` | `<= 1e-10` (`1e3` solver tolerances; ADR 0629 P1c) |
| P2 | PF, every step `abs(Q_out - Q_in) / Q_in` | `<= 1e-10` |
| P3 | PF, every step `continuity` | `<= 1e-9` |
| P4 | PF, every step the GCL residual (computed in the test) | `<= 1e-12` |
| P5 | PF, every step from the second the remapped old flux | `<= 1e-9` |
| P6 | PF, every step `sum_pairs abs(phi_f + phi_g) / Q_in` over all periodic pairs | `<= 1e-12` |
| P7 (set-up) | PF and PU: the re-imaging steps; the final turned offset | exactly `{203, 406, 609}`; `>= 3` |
| P8 (c4) | PU, every step `max_c abs(u_c - U) / abs(U)` and `max_c abs(p_c - p_0) / abs(U)^2` | `<= 1e-10` each |
| P9 (c3) | PU, every step `abs(Q_I + Q_in) / Q_in`, the GCL residual, `continuity` | `<= 1e-10`, `<= 1e-12`, `<= 1e-9` |
| PN1 (negative control of P1) | PF, the largest over the steps of `abs(sum_direct phi + Q_in) / Q_in` (the direct faces alone) | `>= 0.1` (the wrap share is `r / P`, up to `0.99` within each period) |
| PN2 (negative control of P8, no re-imaging) | PU with every re-imaging step built as a step inside one image (the old mesh's `image` set to the new one's before `periodic_sliding_step`, R4) | the largest velocity measure `>= 1e-6`, or a refused or non-finite step (held) (estimate: a slider mesh flux `(1 - d)/d ~ 200` times the true one on one step) |

### S. The rotor sector over three pitches (c1, c2)

| Id | Check | Limit |
| --- | --- | --- |
| S1 (c1) | SF, every step `abs(Q_I - Q_in) / Q_in` | `<= 1e-10` |
| S2 | SF, every step `abs(Q_out - Q_in) / Q_in` | `<= 1e-10` |
| S3 | SF, every step `continuity` | `<= 1e-9` |
| S4 | SF, every step the GCL residual | `<= 1e-12` |
| S5 | SF, every step from the second the remapped old flux | `<= 1e-9` |
| S6 | SF, every step the periodic pair sums | `<= 1e-12` |
| S7 (set-up) | SF and SU: the re-imaging steps; the final turned angle | SF exactly `{210, 419, 629}`, SU exactly `{105, 210, 315}`; `>= pi` |
| S8 | SF, every step `max_c abs(u_c - U_in (r_i / r_c) x_hat_c) / U_in` | `<= 2e-2` (estimate: ADR 0629 printed `7.61e-3` on SS at 0.5 rad, ADR 0216 `6.19e-3` on P; a factor 2.6 for the longer run, whose error is not known) |
| S9 (c2) | SU, every step velocity and pressure as P8 | `<= 1e-10` each |
| S10 (c1) | SU, every step `abs(Q_I) / (abs(U) A_I)`, the GCL residual, `continuity` | `<= 1e-10`, `<= 1e-12`, `<= 1e-9` |
| SN1 (negative control of S8) | SF with the rotor vectors not re-imaged in the solver (`step.reimage = None` on the re-imaging steps) | the largest S8 measure `>= 0.1` (estimate: the rotor velocity turned by `+60 deg`, `abs(R - I) = 1`, and the swirl part `sin 60 deg U_in r_i / r >= 0.58` is divergence-free, so the pressure does not remove it) |
| SN2 (negative control of S9, no re-imaging) | SU as PN2 | as PN2 |

### F. The sector against the full annulus (c5)

One test runs three solvers in lockstep for 640 steps of `dt = 0.005`: SF6 on P (ADR 0216, `step_sliding`), SF on SS (as S),
and SF on SS with the rotor vectors not re-imaged (as SN1). After every step every sector cell is matched to the annulus cell
whose centre is nearest.

| Id | Check | Limit |
| --- | --- | --- |
| F0 (set-up) | every step: every sector cell's nearest annulus cell centre within `1e-9`, the match one-to-one | as stated (R1: the sector is the first sixth of P, read 6) |
| F1 (c5) | every step `max_c abs(u_c^sector - u_c^annulus) / U_in` | `<= 2e-2` (estimate: the two printed errors against the potential flow of ADR 0629 and ADR 0216 add to `1.38e-2`, by the triangle inequality, at 0.5 rad; the difference comes from the wrap faces, coupled with the orthogonal pressure coefficient only where the annulus has internal faces with the non-orthogonal correction, ADR 0629 Q4) |
| F2 | last step, the volume-weighted rms of the same difference | `<= 1e-2` (the difference sits near the interface, about one cell in eight; `2e-2 . sqrt(1/8) = 7e-3`) |
| F3 | last step `abs(pbar^sector - pbar^annulus) / dp_pot`, `pbar` the volume-weighted mean pressure of the rotor's inner ring of cells, `dp_pot = U_in^2 (1 - r_i^2 / r_o^2) / 2 = 0.375` (Bernoulli for the potential flow, outlet `p = 0`) | `<= 5e-2` |
| FN (negative control of F1) | the third solver's F1 measure | `>= 0.1` (as SN1) |

Printed, no limit: F1's measure over steps 1 to 209 (one period, no re-imaging) and over 210 to 640; the annulus's and the
sector's errors against the potential flow at the end. F is the heaviest run (2112 + 352 + 352 cells, 640 steps; estimate
from ADR 0216's 100-step SF, 75 s for ten tests: under two minutes); if its one run takes more than 120 s, the test is marked
`#[ignore]` afterwards as a heavy record (no limit changes), S covering the same code path fast.

### E. Refusals

| Id | Check | Limit |
| --- | --- | --- |
| E1 | `periodic_sliding_step` from image 0 to image 2, and from image 1 to image 0 (the `image` field set by hand) | `Err` each, naming `image` |
| E2 | `step_sliding_periodic` with `reimage.n_rotor_cells` above the cell count | `Err`, the solver's `u`, `p`, `phi`, time unchanged bit for bit |

### D. Defaults (c6)

| Id | Check | Limit |
| --- | --- | --- |
| D1 | `cargo test -p rcfd-api --release --test baseline_hashes` (moving.rs is a shared solver file) | pass |
| D2 | unedited suites pass: `rcfd-fv` `sliding_periodic_f304` (8 run, 2 ignored records), `sliding_interface_f250`, `sliding_interface_f250_records` (R1 to R3 hashes), the `rcfd-mesh` and `rcfd-fv` library tests | pass |
| D3 | PF for 20 steps through `mesh_turned` against the same run through `mesh_at` (ADR 0629's loop): `u`, `p`, `phi` | equal bit for bit |

### Mutations (scratch, not committed)

Each applied alone to the final tree on a scratch copy, the named tests run once; each must fail.

| Id | Mutation | Must fail |
| --- | --- | --- |
| MU1 | `periodic_sliding_step`: the swept volumes of a re-imaging step from `old.offset` (the period not taken off) | P8 or P9 or S9 or S10 |
| MU2 | the re-imaging rotation turned the wrong way (`R` in place of `R^{-1}`) | S8 |
| MU3 | the solver turns `u.values` but not the history levels | S8 |
| MU4 | the solver turns every cell (the stator too) | S8 |

### Decision rule

**Done**: P1, P8, P9 (c3, c4), S1, S9, S10 (c1, c2), F1 (c5) and D1, D2, D3 (c6) met; the set-up checks P7, S7, F0 and the
negative controls PN1, PN2, SN1, SN2, FN held; every mutation caught. A miss of an acceptance check, or a control or mutation
that does not show its check can fail: **partial** with the miss named. The re-imaging step cannot be built or the loop does
not run past one period: **todo**. Every other check is supporting and reported.

## Limits (not covered)

Turned offsets `>= 0` and steps of less than one period (a step across two images, or backwards across one, is refused);
boundary data that the caller sets per face on rotor patches (a fixed velocity profile) are the caller's to re-image (the
set-ups set them every step from the new face centres, or use an invariant value); the stator of another pitch (ADR 0629 Q5);
the non-orthogonal correction on coupled faces (ADR 0629 Q4, measured here only through F); BDF2 (refused by the moving step),
turbulence, energy and the other refusals of `step_sliding`; more than one interface; the case tree (F-303); the compressible
solver; parallel runs.

## Open questions taken on the standing instruction

- **Q1. Where the re-imaging happens.** (a) Inside the step that crosses the period (taken: the old mesh is read in the new
  image, R2; no extra operation between steps, and the face map is the one of every step); (b) a separate solver operation
  between two steps that rebuilds the old mesh at `r_0 - P` (negative offsets, wrap overlaps of the other sign, a remap of the
  flux from one face set to the other). Taken (a) on the maintainer's standing instruction of 2026-11-11 (take the
  recommendation).
- **Q2. Who turns the solver's vectors.** (a) `step_sliding_periodic`, from a field of the step (taken: the step and the state
  stay consistent, and a caller cannot forget it); (b) a separate public method the caller calls. Taken (a) on the standing
  instruction.
