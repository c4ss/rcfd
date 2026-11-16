# ADR 0638: F-710 registration: the sliding interface at coinciding nodes (the parts of the interface graph)

Status: **REGISTERED 2026-11-16, written before any code of F-710 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-710 was written. One diagnostic run of the base code was made
before this text (disclosed below, "Diagnosis"); it ran the unchanged step of ADR 0216 and printed what it computes, from a
scratch test file that was moved out of the worktree after the run and never committed
(`w49i-work/f710-scratch/zz_f710_diag.rs`). Results go in the addenda at the end; nothing above the addenda is changed after
a run.

Roadmap item F-710 (deps F-250), proposed by [ADR 0637](0637-f303-registration-sliding-interface-in-the-case-tree.md)
amendment 2 (F-303: U1c missed at coinciding nodes). Parents: [ADR 0216](0216-f250-registration-sliding-interface-on-a-cylinder.md)
(F-250: the sliding mesh, the step M2 to M4, the pair P, the uniform-flow checks U1 to U4),
[ADR 0629](0629-f304-registration-planar-and-sector-sliding-interfaces.md) (F-304, `graph_balance`) and
[ADR 0634](0634-f704-registration-sliding-beyond-one-period-re-imaging-of-the-rotor.md) (F-704). Number 0638 given by the
lead. Branch `f710-work` from 76661520 (the F-303 tip, on F-704 and F-304), worktree `w49i-work/f710`, scratch
`w49i-work/f710-scratch`.

House rules (`w49i-work/AGENT_RULES.md`) apply: limits fixed here; each registered run made once; existing tests not edited;
defaults bit-identical; no edit of `baseline_hashes.rs`, of a hash table or of a golden; no new dependency, no unsafe code;
clean-room; the roadmap files are not edited. Every choice the maintainer would normally be asked about is taken on the
maintainer's standing instruction of 2026-11-11 (take the recommendation), except where listed as open.

Clean-room: the fix is a property of the graph Laplacian already used by ADR 0216 M3/M4 (a weighted minimum-norm flow on a
graph is solvable exactly when every connected part's targets sum to zero; first principles, below). No source was read; no
vendor code, text or install was used. The reference is a closed form (a uniform field) and the base code itself.

## What F-710 asks

The acceptance, binding:

> A uniform flow on the sliding pair of ADR 0216 with a step that lands on node coincidences (every whole rotor pitch) is kept to 1e-10 on every step, including the steps where the interface loses or regains zero-area overlaps; the cause found in ADR 0637 amendment 2 is diagnosed and registered before the fix; the F-250, F-304, F-704 and F-303 results are unchanged where no coincidence occurs. Every limit not stated here is registered in the ADR before the run.

Clauses:

- **a1** uniform flow on ADR 0216's pair P (`cylinder_ogrid(144, 8, 1, 1.5)` against `cylinder_ogrid(120, 8, 1.5, 2)`) with a
  step that lands on a node coincidence every whole rotor pitch, kept to `1e-10` (velocity in `|U|`, pressure in `|U|^2`) on
  every step, the steps that lose and the steps that regain the zero-area overlaps included;
- **a2** the cause of ADR 0637 amendment 2 diagnosed and registered before the fix (this document, "Diagnosis");
- **a3** the F-250, F-304, F-704 and F-303 results unchanged where no coincidence occurs.

## What the code read shows (base 76661520, read only)

1. `rcfd_mesh::non_conformal::join_impl`: an overlap whose clipped area is below `1e-10` of the smaller parent face, or whose
   points merge to fewer than three registry points, is dropped (`n_dropped`). A rotor face and a stator face that touch only
   along a radial line (their end nodes at one angle) give such a zero-area overlap.
2. `rcfd_fv::sliding::sliding_step` (M3, M4): the mesh flux of the interface faces and the correction of the old-level flux on
   the interface faces are each the area-weighted minimum-norm solution of `sum_f sigma_cf x_f = t_c - mean` on the graph whose
   nodes are the cells touched by interface faces and whose edges are the interface faces (`interface_balance`, from
   `graph_balance`). `graph_balance` removes the mean of the targets **per connected part** of that graph and reports the
   largest removed mean (`removed_mean`); `sliding_step` does not look at it.
3. P: rotor 144 cells around, stator 120; `gcd(144, 120) = 24`. At the rotor angle `k 2 pi / 144` (a whole rotor pitch) 24
   rotor nodes coincide with stator nodes (every 6th rotor node, every 5th stator node); the interface then has `264 - 24 =
   240` faces and 48 dropped overlaps (2 per coinciding node). At every other angle reached by the steps below it has 264
   faces and none dropped.
4. `crates/rcfd-fv/src/sliding_periodic.rs` (F-304, F-704) builds its own edge lists and calls `graph_balance`; it does not
   call `sliding_step`. The tree path of F-303 (`crates/rcfd-api/src/sim/sliding.rs`) calls `sliding_step`.

## Diagnosis (one run of the base code, disclosed)

Set-up: ADR 0216's U2 loop (PS with `nu = 0.1`, every patch but `empty` fixed at `U = (1, 0.5, 0)`, uniform start) on P with
`dt = 2 pi / 144 / 8 = 0.00545415391248228` (the rotor pitch over 8, ADR 0637's step), 18 steps; per step the scratch test
recomputed the step's GCL targets `t` and remap targets `e` from the returned `SlidingStep` and solved `interface_balance` on
them again to read `removed_mean`. Printed (base code):

| Step | Interface faces | `removed_mean` of `t` | `removed_mean` of `e` | `flux_residual` | velocity dev. |
| --- | --- | --- | --- | --- | --- |
| 1 to 7 | 264 | `<= 6e-17` | `<= 5e-20` | `<= 9.1e-16` | `<= 7.2e-15` |
| 8 | 240 | `9.9e-17` | **`2.16e-5`** | **`2.07e-3`** | **`2.41e-4`** |
| 9 to 15 | 264 | `<= 5e-17` | `<= 3.4e-17` | `<= 1.6e-12` | decays to `1.22e-4` |
| 16 | 240 | `1.1e-16` | **`2.16e-5`** | **`2.07e-3`** | `3.55e-4` |

The GCL residual stayed `<= 1.1e-14` on every step. A second scratch run (same file) on the pair LP of the checks below (rotor
and stator with two equal axial layers, `dt = 0.01`, 6 steps; old flux = the uniform field's face flux) gave 176 interface
faces and 176 dropped overlaps on every step (the axial layers touch along the plane `z = 0.2`), `removed_mean` of `t` up to
`3.2e-16` (largest target `1.7e-3`, ratio `1.9e-13`) and of `e` up to `1.6e-19`, and `flux_residual 3.0e-16`.

**Cause (registered).** At a whole rotor pitch the 24 coinciding nodes cut the interface graph into 24 connected parts (each
a sector of 6 rotor and 5 stator cells), because the overlaps that crossed those angles have zero area and are dropped. The
remap target of a part, `sum_{c in part} e_c`, is minus the old-level flux through the part's faces off the interface. The old
flux was divergence-free on the old mesh, but there the part's cells were not a closed region: the rotor cells of the part
were turned back by `dt` and overlapped stator cells of the neighbouring parts. For the uniform field, the rotor faces keep
their old flux `U . S_f(old)` (M4 maps them unchanged), so a part's sum is of order `|U| |S| omega dt`, not zero; it is the
flux that crossed the vanished overlaps. `graph_balance` removes that mean per part, so the remapped old flux keeps a
divergence of the size of the mean in every cell of the part (`flux_residual 2.07e-3`, step 8), and the momentum step of a
uniform field, which needs `sum_f sigma phi_old = 0` per cell, loses the uniform state. On a connected graph (every other
step, 264 faces) the sum over the whole graph is the flux through a closed region and vanishes, which is why U1d of ADR 0637
and ADR 0216 U2 (no coincidence) are met. The GCL targets of a part sum to zero here in exact arithmetic (the 24-fold symmetry
of P makes the slivers that a part gains and loses equal, arithmetic 1) and measured `<= 1.1e-16`; on a pair without that
symmetry they need not. The join's snapping of nearly coinciding nodes is not the cause (the dropped overlaps would be
dropped with exact coincidence too, and the GCL closes to `1e-14` on those steps). Regaining the overlaps (step 9) is not a
cause by itself: step 1 leaves the coincident mesh at angle 0 with a deviation of `4.8e-15`; the deviation after step 9 is
step 8's error decaying.

Arithmetic 1 (by hand): a part's GCL target sum equals `seg(P0, [phi_k - d, phi_k]) - seg(P0, [phi_{k+1} - d, phi_{k+1}])`,
`seg` the area between the arc and the old interface polygon `P0` over an angle interval, `d = omega dt`; both intervals hold
the same pattern of merged angles because both node sets are periodic in `2 pi / 24`, so the difference is zero.

## Design

**D1. Coupling faces.** On a step whose new interface graph has a connected part with a mean that cannot be a rounding error,
the parts are coupled through the faces that remain between them: the internal faces that are not interface faces, whose
owner and neighbour are both touched by interface faces and lie in **different** connected parts of the interface graph. On
P at a whole rotor pitch these are, at each coinciding node, the rotor face and the stator face that end at that node (48
faces). These faces carry the flux that crossed the vanished overlaps.

**D2. The coupled solve.** For a target vector `t` (the GCL targets of M3, the remap targets of M4), the base solve on the
interface faces is made first, exactly as now. If its `removed_mean > 1e-10 * max_c |t_c|` (the largest over the cells of the
graph) and coupling faces exist, the solve is made again with `graph_balance` on the edges "interface faces, then coupling
faces" (weights the face areas, as M3), and its values replace the base values: on an interface face the value is set as now
(`m_f = x_f`; `phi_f = interp_f + x_f`), on a coupling face it is **added** to the face's mapped value (`m_f += x_f`;
`phi_f += x_f`). Otherwise the base values are used as now, so the code path and every operation of a step without such a
part are those of the base (bit-identical). `old_defect` is not changed on a coupling face (the defect is the mapped face's
own, as `step_ale`; adding the correction there would put it into the new predicted flux, which the pressure would then have
to remove). The extended graph's solution satisfies every cell's balance exactly when the extended graph is connected (the
Laplacian's null space is the constant per part, first principles), which holds on P (the rotor ring and the stator ring are
connected through their radial faces).

**D3. The threshold.** `1e-10` relative: a part's mean from rounding is at most about `n eps max|t|`; with `n <= 1e3` cells
per part that is `2e-13`, and the diagnosis measured `1.9e-13` (LP, the GCL targets of axial parts that are closed rings);
the coincidence measured `2.16e-5` against a largest target of order `|U| |S_f| ~ 1.1 * 6.5e-3 = 7e-3` (arith), about `3e-3`.
The threshold sits more than two decades above the rounding and seven below the defect. It is what keeps LP (axial parts,
every step, means at rounding) on the base path; without it LP's results would change.

**D4. Scope.** Only `sliding_step` (the cylindrical path of F-250 and F-303). `graph_balance`, `interface_balance`, the
periodic step of F-304 and F-704 (`sliding_periodic.rs`), `step_sliding` and the mesh join are not changed. Public API: no
new item; `SlidingStep` keeps its fields. The coupling is not optional (no key): it acts only on steps where the base result
violates the step's own conservation by more than the threshold, which is what F-710 asks to change; everywhere else the
result is the base's bit for bit (B1, B2).

## Checks (limits fixed now)

Set-ups:

- **UC**: ADR 0216 U2's loop (`uniform_run`: PS, `nu = 0.1`, every non-empty patch fixed at `U = (1, 0.5, 0)`) on P with
  `dt = 2 pi / 144 / 8`, `omega = 1`, **50 steps** (6.25 rotor pitches): coincidences at steps 8, 16, ..., 48 (6 steps that
  lose the overlaps), regains at steps 9, 17, ..., 49 (6 steps). Deviation per step: `max_c |u_c - U| / |U|` and `max_c |p_c
  - p_0| / |U|^2`, as U1.
- **UCt**: ADR 0637's UTc (the case tree: `annulus_3d_cylindrical_interface` 144/120, `[mesh.sliding] angular_velocity = 1`,
  every boundary fixed at `(1, 0.5, 0)`, `dt` = the rotor pitch over 8, 40 steps), the measures of ADR 0637 U1 (velocity in
  `|U|`, pressure about its volume mean in `|U|^2`).
- **LP**: rotor `annulus_3d(48, 4, 2, 1, 1.5, 0.4)`, stator `annulus_3d(40, 4, 2, 1.5, 2, 0.4)` (equal axial layers), uniform
  `U = (1, 0.5, 0.3)`, `dt = 0.01`, 20 steps, as U3.
- **R**: the replica, a copy in the test file of the base `sliding_step`'s M3/M4 (from 76661520, the same operations in the same
  order), returning `(mesh_flux, phi_old, old_defect)` for the same inputs.

| Id | Clause | Set-up | Check | Limit |
| --- | --- | --- | --- | --- |
| A1 | a1 | UC | the deviation on every one of the 50 steps, velocity and pressure | `<= 1e-10` each |
| A1c | a1 | UC | the same on the 6 coincidence steps and the 6 regain steps (listed) | `<= 1e-10` each |
| A1t | a1 (tree) | UCt | as A1 over 40 steps | `<= 1e-10` each |
| A2 | a2 | this document | the cause registered before the fix (this commit, alone, first) | present |
| B1 | a3 | UC | `sliding_step` against R, same inputs, every step: bit-equal on the 44 steps without coincidence; differ on exactly steps 8, 16, ..., 48 | exact |
| B2 | a3 | U2 of ADR 0216 (`dt = 0.005`, 100 steps), U3 (P3, 40 steps), LP | `sliding_step` against R on every step | bit-equal |
| B3 | a3 | existing suites, unedited | `rcfd-fv`: lib, `sliding_interface_f250`, `_records`, `sliding_periodic_f304`, `sliding_reimaging_f704`; `rcfd-api`: `sliding_case_f303`, `baseline_hashes`, `cylindrical_interface_w44b`, `dynamic_mesh`; `rcfd-cli` `sliding_case_f303` | pass as before |
| B4 | a3 | code | no line of `sliding_periodic.rs`, `graph_balance`, `interface_balance`, `pimple/moving.rs`, `rcfd-mesh` changed | none |
| S1 | supporting | UC | on the coincidence steps, `flux_residual` and `gcl_residual` of the step | `<= 1e-12` each |
| S2 | supporting | UC | coupling faces on a coincidence step | 48 |
| T1 | D2 (GCL path) | unit | two parts with target sums `+1`, `-1` (a 4-cell graph: interface edges (0,1), (2,3), one coupling edge (1,2)); the coupled solve's residual against the targets | `<= 1e-14` |
| N1 | negative control | UC with R in place of `sliding_step` (the base step) | the largest velocity deviation | `>= 1e-6` |

Estimates: A1, A1t from the base's steps without coincidence (`7.2e-15`, diagnosis; U1d `7.3e-15`, ADR 0637) and the coupled
remap closing every cell to rounding (S1 `~1e-15`); four decades of margin. N1 from the diagnosis (`2.4e-4`, the base).
Each run is made once.

Mutations (on a scratch copy, after the checks):

- **MU1** the coupled solve never used (the trigger always false): A1 must fail.
- **MU2** the coupling-face values not added to `phi_old` (interface faces only take the coupled values): A1 or S1 must fail.
- **MU3** the threshold removed (couple whenever coupling faces exist): B2 (LP) must fail.
- **MU4** the sign of the coupling-face value flipped in `phi_old`: A1 or S1 must fail.

Decision rule: **done** when A1, A1c, A1t, B1 to B4, S1 and N1 are met; **partial** when A1 and A1c are met but A1t, or a B
check other than B3, is missed, or when only A1t fails; **todo** when A1 is missed. A B3 failure stops the work and is
reported (no existing test is edited).

## Limits (not covered)

- The periodic path (F-304 sector and planar sliding, F-704 re-imaging) is not changed: its own coincidences are not checked
  or fixed here (follow-up proposed if a check shows the same loss).
- A part that has no coupling face (an interface graph part with no remaining face to its neighbours) keeps the base result.
- The GCL coupling is exercised by T1 only; on P its part means are rounding (symmetry), so UC does not trigger it.
- Non-uniform flows at coincidence steps: the coupled remap changes their old flux on the coupling faces; no accuracy check
  beyond the existing F-303 non-axisymmetric checks (which run through coincidence steps and must still pass, B3) is made.

## Addendum 1: results (2026-11-16)

Measured against the limits registered above; no limit was changed. Release builds on Windows. The registration is 382f8d0c;
the code and the tests fbe58e64. Each registered run was made once and every check was met in it. After that run two lint
fixes demanded by clippy were made before the commit (`coupled_balance`'s return type named `Coupled`; its threshold test
written as `partial_cmp(..) != Some(Greater)` in place of `!(a > b)`, the same result for every non-NaN value; the test
helper `find` taking `&mut [usize]`); the new tests, `baseline_hashes` and `sliding_case_f303` were run once more on the
committed code as a confirmation and printed the same numbers. No limit or set-up was chosen after a run.

**Built as registered** (D1 to D4). `crates/rcfd-fv/src/sliding.rs` (shared solver file): module text, lines 15 to 18;
`PART_MEAN_TOL`, `coupling_faces`, `Coupled`, `coupled_balance` (private), lines 221 to 283; the M3 assignment, lines 369 to
383; the M4 assignment, lines 423 to 437; the unit test T1, lines 639 to 669. Nothing else changed; `graph_balance`,
`interface_balance`, `sliding_periodic.rs`, `pimple/moving.rs` and `rcfd-mesh` are untouched (B4). Tests:
`crates/rcfd-fv/tests/sliding_coincidence_f710.rs` (3: UC, B2, N1), `crates/rcfd-api/tests/sliding_coincidence_f710.rs` (1:
A1t). Docs: `docs/dynamic-mesh.md` (the coincidence paragraph). No golden, no hash table, no existing test edited.

### Results

| Check | Limit | Measured | Verdict |
| --- | --- | --- | --- |
| A1 (a1) UC, 50 steps | `<= 1e-10` each | velocity `8.86e-15`, pressure `5.20e-14` | met |
| A1c (a1) the 6 coincidence steps 8, ..., 48; the 6 regain steps 9, ..., 49 | `<= 1e-10` each | `8.86e-15`, `4.12e-14`; `6.86e-15`, `4.22e-14` | met |
| A1t (a1, tree) UCt, 40 steps, coincidences at 8, ..., 40 | `<= 1e-10` each | velocity `6.05e-15`, pressure `3.23e-13` | met |
| A2 (a2) cause registered before the fix | present | 382f8d0c, alone, before fbe58e64 | met |
| B1 (a3) UC against the replica | bit-equal on the 44 others; differ exactly on 8, ..., 48 | as stated | met |
| B2 (a3) U2 (100 steps), U3 (40), LP (20) against the replica | bit-equal every step | no step differs | met |
| B3 (a3) existing suites, unedited | pass as before | `rcfd-fv` lib 109/6 (108 before plus T1), `sliding_interface_f250` 14/4, `_records` 3/0, `sliding_periodic_f304` 8/2, `sliding_reimaging_f704` 12/2; `rcfd-api` `sliding_case_f303` 7/4, `baseline_hashes` 3/1, `cylindrical_interface_w44b` 5/1, `dynamic_mesh` 5/1; `rcfd-cli` `sliding_case_f303` 2/0 (passed / ignored) | met |
| B4 (a3) the periodic path, `graph_balance`, `moving.rs`, `rcfd-mesh` | no line changed | none | met |
| S1 coincidence steps: flux residual, GCL residual | `<= 1e-12` each | `1.24e-15`, `1.05e-14` (base: `2.07e-3` flux) | met |
| S2 coupling faces per coincidence step | 48 | 48 on each of the 6 | met |
| T1 two parts, coupled solve residual | `<= 1e-14` | `0` (removed mean `0.5`, the one coupling face found) | met |
| N1 UC with the base step (negative control) | `>= 1e-6` | `5.74e-4` (first above `1e-10` at step 8) | met |

LP (equal axial layers) has 176 dropped overlaps and 2 graph parts on every step and stays on the base path (B2), which is
what the threshold of D3 is for (MU3).

### Mutations (scratch copy `w49i-work/f710-scratch/mut`, script `mutate.sh`, log `mutations.log`)

| Mutation | Caught by |
| --- | --- |
| MU1 the coupled solve never used | A1 (`5.74e-4`), A1c, B1 (no step differs), S1 (`2.07e-3`) |
| MU2 coupling-face values not added to `phi_old` | A1 (`4.55e-3`), A1c, S1 (`6.49e-2`) |
| MU3 the threshold removed | B2 on LP (all 20 steps differ from the base) |
| MU4 sign of the coupling-face value flipped | A1 (`9.11e-3`), A1c, S1 (`1.30e-1`) |

Four of four caught. `cargo fmt -p rcfd-fv -p rcfd-api --check` clean; `cargo clippy -p rcfd-fv -p rcfd-api --all-targets --
-D warnings` clean.

### Decision

By the registered rule: A1, A1c, A1t, B1 to B4, S1 and N1 met: **done**.

Not covered (as registered): the periodic path of F-304 and F-704 at its own coincidences (not checked; it builds its own
edge lists, the same loss is possible there); the GCL coupling runs only in T1 (on P the part means of the GCL targets are
rounding by symmetry, so UC never takes that branch); non-uniform flows at coincidence steps are checked only through the
F-303 non-axisymmetric checks that pass unedited (B3); a part with no coupling face keeps the base result.

### Remaining questions

1. ADR 0637's ignored record `utc_known_miss_coinciding_nodes` now meets its limit (A1t is that set-up). Options: (a) leave
   it as the ignored record of the miss, as the house rules require, and note in the F-303 roadmap entry that F-710 resolves
   it; (b) edit it. Recommendation (a), taken on the maintainer's standing instruction of 2026-11-11 (take the
   recommendation); no existing test is edited.
2. The periodic path. Options: (a) a follow-up item that checks a uniform flow through coincidences on the sector pair of
   ADR 0629 and applies the same coupling to `periodic_sliding_step` if it misses; (b) apply it now unchecked. Recommendation
   (a) (follow-up below).

## For the lead: roadmap entries

> Proposed status: done.

> ADR 0638, 2026-11-16: the sliding interface at coinciding nodes. Cause (diagnosed and registered before the fix): at a whole rotor pitch 24 rotor and stator nodes of ADR 0216's pair coincide, the join drops the 48 zero-area overlaps there and the interface graph falls into 24 parts; the remap of the old flux (ADR 0216 M4) removed each part's target mean, which is the old flux that crossed the vanished overlaps, so the old flux kept a divergence of 2.1e-3 and the uniform state was lost (2.4e-4). Fix in rcfd_fv::sliding::sliding_step: when a part's mean exceeds 1e-10 of the largest target, the GCL closure and the remap are solved on the interface faces together with the faces between the parts (48 per coincidence step), whose mapped values receive the correction; every other step runs the base code. Uniform flow on the pair with dt = rotor pitch / 8 over 50 steps: 8.9e-15 velocity, 5.2e-14 pressure (limit 1e-10), the 6 coincidence and 6 regain steps included; through the case tree (ADR 0637's UTc, 40 steps) 6.0e-15 and 3.2e-13; the base step as control 5.7e-4. sliding_step equals a replica of the base step bit for bit on every step without a coincidence (44 of 50; U2, U3 and an equal-axial-layer pair on every step); every F-250, F-304, F-704 and F-303 test and baseline_hashes pass unedited. Four of four mutations caught. Limits: the periodic (sector, planar) path is not changed; the GCL coupling is exercised by a unit test only.

> F-303 note: replace "but 2.3e-4 on the steps where rotor and stator nodes coincide and the interface drops its zero-area overlaps (a defect of the F-250 step; follow-up)" with "and, since F-710 (ADR 0638), 6.0e-15 on the steps where rotor and stator nodes coincide (ADR 0637's ignored U1c record stays as the record of the miss)".

> F-250 note: replace "the uniform state is lost on steps where rotor and stator nodes coincide (ADR 0637 amendment 2)" with "the uniform state at coinciding nodes is kept since F-710 (ADR 0638)".

> Follow-up proposed: "Periodic sliding at coinciding nodes". Deps F-710, F-704. Acceptance: a uniform flow on the sector pair of ADR 0629 with a step that lands on node coincidences is kept to 1e-10 on every step through periodic_sliding_step, the same coupling of the interface graph's parts applied if the base misses; F-304 and F-704 results unchanged where no coincidence occurs.
