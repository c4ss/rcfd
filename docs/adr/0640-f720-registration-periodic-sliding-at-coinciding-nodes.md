# ADR 0640: F-720 registration: periodic sliding at coinciding nodes (the base step checked, no fix needed for the uniform flow)

Status: **REGISTERED 2026-11-16, written before any code of F-720 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-720 was written. One diagnostic run of the base code was made
before this text (disclosed below, "Diagnosis"); it ran the unchanged `periodic_sliding_step` and `step_sliding_periodic` of
d40cd665 and printed what they compute, from a scratch test file that was moved out of the worktree after the run and never
committed (`w49i-work/f720-scratch/zz_f720_diag.rs`, log `diag.log`). Results go in the addenda at the end; nothing above the
addenda is changed after a run.

Roadmap item F-720 (deps F-710, F-704), proposed by [ADR 0638](0638-f710-registration-sliding-interface-at-coinciding-nodes.md)
question 2 (F-710: the coinciding-node defect of `sliding_step`, its cause and the coupling of the interface graph's parts).
Parents: [ADR 0629](0629-f304-registration-planar-and-sector-sliding-interfaces.md) (F-304: the periodic join, the step M5, M6,
the pairs PP, SS, S3 and the flows PU, SU, SF), [ADR 0634](0634-f704-registration-sliding-beyond-one-period-re-imaging-of-the-rotor.md)
(F-704: turned offsets and the re-imaging step). Number 0640 given by the lead. Branch `f720-work` from the local main
d40cd665 (F-304, F-704, F-303 and F-710 merged), worktree `w49i-work/f720`, scratch `w49i-work/f720-scratch`.

House rules (`w49i-work/AGENT_RULES.md`) apply: limits fixed here; each registered run made once; existing tests not edited;
defaults bit-identical; no edit of `baseline_hashes.rs`, of a hash table or of a golden; no new dependency, no unsafe code;
clean-room; the roadmap files are not edited. Every choice the maintainer would normally be asked about is taken on the
maintainer's standing instruction of 2026-11-11 (take the recommendation), except where listed as open.

Clean-room: the argument below uses the divergence theorem for a uniform field over a closed cell, the invariance of an axial
vector under a rotation about that axis, and the symmetry argument of ADR 0638 (arithmetic 1). No source was read; no vendor
code, text or install was used. The reference is a closed form (a uniform field).

## What F-720 asks

The acceptance, binding:

> A uniform flow on the sector pair of ADR 0629 with a step that lands on node coincidences is kept to 1e-10 on every step through periodic_sliding_step, the same coupling of the interface graph's parts applied if the base misses; F-304 and F-704 results unchanged where no coincidence occurs. Every limit not stated here is registered in the ADR before the run.

Clauses:

- **a1** a uniform flow on the sector pair of ADR 0629, with a step that lands on node coincidences, kept to `1e-10` (velocity
  in `|U|`, pressure in `|U|^2`) on every step through `periodic_sliding_step`;
- **a2** the coupling of ADR 0638 applied to the periodic step **if the base misses** a1 (the brief: first run the base at
  coincidences once, disclosed, and register whether it misses and why; if it does not, register that and record it with the
  check only);
- **a3** the F-304 and F-704 results unchanged where no coincidence occurs.

"The sector pair of ADR 0629" with a uniform flow is S3 with SU: a uniform flow on a rotating periodic sector must be invariant
under the sector rotation (ADR 0629 M6), so it is axial, and the 2-D pair SS (empty in `z`) has no such flow but zero. The
planar pair PP with PU (any uniform `U` is invariant under a translation) runs through the same function and is checked as a
supporting set-up (A2).

## What the code read shows (base d40cd665, read only)

1. `rcfd_fv::sliding_periodic::periodic_sliding_step` builds its own graph (direct interface faces, then the wrap pairs as
   edges between the owner of the rotor copy and the owner of the stator copy) and calls `graph_balance` for the GCL closure
   (M5) and the remap of the old flux (M6). It does not look at `removed_mean`; it does not call `sliding_step`, so F-710's
   coupling (`coupled_balance`, private in `sliding.rs`) does not act there.
2. S3 (rotor `annulus_3d_sector(12, 4, 2, 1, 1.5, 0.4, pi/3)`, stator `annulus_3d_sector(10, 4, 3, 1.5, 2, 0.4, pi/3)`): rotor
   nodes at `delta + i pi/36`, stator at `j pi/30`. A rotor node meets a stator node when `delta = (6 j - 5 i) pi/180`, i.e.
   at every multiple of `pi/180`; then the coinciding angles repeat every `pi/6` (2 in the sector). With `dt = pi/288` (the
   rotor pitch over 8, ADR 0638's choice), `n pi/288 = m pi/180` needs `5 n = 8 m`: **every 8th step** lands on a
   coincidence; one period `pi/3` is 96 steps.
3. PP (slider 20 cells on `[0, 1]`, stator 16): nodes at `delta + i/20` and `j/16`, meeting at every multiple of `1/80`
   (ADR 0629); with `dt = 1/640` every 8th step lands on a coincidence, 4 coinciding nodes per period; one period is 640 steps.
4. SS (rotor `annulus_sector(24, 8, 1, 1.5, pi/3)`, stator `annulus_sector(20, ...)`): meetings at multiples of `pi/360`;
   with `dt = pi/576` every 8th step, 4 coinciding nodes per sector, one of them on the stator seam.

## Diagnosis (one run of the base code, disclosed)

Set-ups as the checks below (ADR 0634's turned loop with `mesh_turned(n dt)`, PS6 of ADR 0629, every non-empty non-cyclic
patch fixed at `U`, uniform start). Printed per step: direct faces, wrap pairs, dropped overlaps, connected parts of the
interface graph (direct and wrap edges, cells touched by them), `flux_residual`, `gcl_residual`, and the deviations.

| Run | Steps | Coincidence steps (parts) | Deviation on coincidence steps (u, p) | Other steps (u, p) | `flux_residual` | GCL |
| --- | --- | --- | --- | --- | --- | --- |
| S3 / SU `U = (0, 0, 1)`, `dt = pi/288` | 200 (2.08 periods; re-imaging at 97, 193) | 25 (every 8th; 2 parts, 8 to 14 dropped) | `4.80e-15`, `2.54e-14` | `5.80e-15`, `3.91e-14` | `<= 2.2e-15` | `<= 6.2e-15` |
| PP / PU `U = (1, 0.5, 0)`, `dt = 1/640` | 650 (re-imaging at 640, offset 0, no wrap pair) | 81 (every 8th; 4 parts, 7 or 8 dropped) | `2.13e-14`, `2.88e-13` | `2.28e-14`, `3.19e-13` | `<= 5.3e-15` | `<= 1.1e-14` |
| SS / non-symmetric source (below), `dt = pi/576` | 40 | 5 (every 8th; 4 parts, 7 or 8 dropped) | not measured | not measured | **`3.98e-4` to `9.86e-4`** on the 5; `<= 2.5e-14` on the others from step 2 | `<= 1.0e-14` |

The non-symmetric source: ADR 0629's SF on SS (inlet `inner` on the rotor, outlet `outer` with `p = 0` and zero-gradient
velocity, `nu = 0.1`, start `u = (r_i / r) x_hat`) with the inlet `u_b = (1 + 0.5 sin 6 theta) x_hat` at the new face centres
every step (`theta = atan2(y, x)`): periodic in `pi/3`, but not in `pi/6` (`sin 6 theta` changes sign under `pi/6`). Step 1
printed `flux_residual 2.07e-1`, the start flux of `PimpleSolver::new` (as ADR 0629 S5 notes).

**Finding (registered).** The base periodic step **does not miss** a1: on S3 the uniform axial flow is kept to `4.8e-15`
(velocity) and `2.5e-14` (pressure) on the 25 coincidence steps, the steps after them, and the two re-imaging steps; the planar
pair likewise (`2.1e-14`, `2.9e-13`, 81 coincidence steps). Coupling is therefore not applied (a2, as the brief says).

**Why the uniform flows are immune (first principles).** ADR 0638's cause is a part of the interface graph whose remap target
`sum_{c in part} e_c` is not zero: the old flux that crossed the vanished overlaps. Here every cell's target is rounding:

- remap (M6): the mapped faces keep `phi_old = U . S_f(old)` (the state is uniform to rounding); a stator face does not move; a
  rotor face is `S_f(new) = R S_f(old)` with `R` the rotation (sector) or `I` (translation), so `U . S_f(new) = (R^T U) .
  S_f(old) = U . S_f(old)` because `R^T U = U` for an axial `U` and for any `U` under a translation. The new faces get `U .
  S_f(new)` (direct faces) and `(w U + (1 - w) R U) . S_a = U . S_a` (wrap pairs). So `e_c = -sum_f sigma_cf U . S_f(new) = 0`
  for every closed cell (divergence theorem of a uniform field), and every part's mean is rounding. On P of ADR 0638 the flow
  `(1, 0.5, 0)` is not invariant under the rotation, the rotor faces keep `U . S_f(old) != U . S_f(new)`, and the part sums are
  `O(|U| |S| omega dt)` (its diagnosis);
- GCL (M5): on PP the interface is flat and no sliver changes (every `t_c` is rounding); on S3 the rotor and stator node sets
  are both periodic in `pi/6`, every part is a `pi/6` sector between two coinciding angles, and ADR 0638's arithmetic 1 gives a
  zero part sum (measured: GCL `<= 6.2e-15`).

**Latent defect (registered, outside the acceptance).** For an old flux that is not invariant under the part symmetry, the
part sums are not zero and the base periodic step leaves them in the remapped old flux exactly as ADR 0638 found for
`sliding_step`: SS with the non-symmetric source has `flux_residual 4e-4` to `1e-3` on the coincidence steps against `2.5e-14`
elsewhere. Its effect on the velocity has no reference here and is not measured. It is recorded as a negative control (N1) and
proposed as a follow-up; F-720's acceptance asks for the uniform flow only.

## Design

**D1. No library change.** The base keeps a1; the coupling of ADR 0638 is not added to `periodic_sliding_step` (a2: "applied if
the base misses"). F-720 adds one test file, `crates/rcfd-fv/tests/sliding_periodic_coincidence_f720.rs`, with the checks
below. `docs/dynamic-mesh.md` gets one paragraph. Defaults are bit-identical by construction (B1).

**D2. A note for the follow-up (not built).** The coupling faces of ADR 0638 D1 are internal faces. On the periodic graph a
coincidence can fall on a seam (on SS at the reduced offsets `k pi/72` that CN's steps reach, one coinciding node is always
the stator seam; at a multiple of `pi/12` another is the rotor seam), where one of the two faces at the node is a seam pair, a boundary face. The graph is a ring
cut at `k` points; `k - 1` internal coupling faces already connect it, and the ring's total target is zero because the seam
pairs carry opposite fluxes, so internal coupling faces are expected to suffice except where both seams coincide (offset 0,
where the wrap pairs vanish too). This is an estimate for the follow-up, not a check of F-720.

## Checks (limits fixed now)

Set-ups (PS6 of ADR 0629; the loop of ADR 0634 with `mesh_turned(n dt)`, `Omega = 1`, slide speed `1`):

- **CS** (a1): S3, SU `U = (0, 0, 1)` fixed on every non-empty non-cyclic patch, `dt = pi/288`, **200 steps** (2.08 periods):
  coincidences at steps 8, 16, ..., 200 (25 steps), the steps after them 9, 17, ..., 193 (24), re-imaging at 97 and 193.
- **CP** (supporting): PP, PU `U = (1, 0.5, 0)` likewise, `dt = 1/640`, **650 steps**: coincidences at 8, ..., 648 (81 steps,
  among them 640), re-imaging at 640.
- **CN** (control): SS with the non-symmetric source of the diagnosis, `dt = pi/576`, **40 steps**.

| Id | Clause | Set-up | Check | Limit |
| --- | --- | --- | --- | --- |
| A1 | a1 | CS | velocity `max_c abs(u_c - U)/abs(U)` and pressure `max_c abs(p_c - p_0)/abs(U)^2` on every one of the 200 steps | `<= 1e-10` each |
| A1c | a1 | CS | the same over the 25 coincidence steps, the 24 steps after them, and the 2 re-imaging steps (printed separately) | `<= 1e-10` each |
| X1 | a1 (set-up) | CS | the graph has 2 parts and the join drops overlaps on exactly the steps `n % 8 == 0`; 1 part and none dropped on all others; re-imaging steps | exact; `{97, 193}` |
| A2 | supporting | CP | as A1, every step, and over the 81 coincidence steps | `<= 1e-10` each |
| X2 | supporting (set-up) | CP | 4 parts and dropped overlaps on exactly the steps `n % 8 == 0`, 1 part and none dropped on all others; re-imaging steps | exact; `{640}` |
| S1 | supporting | CS, CP | on the coincidence steps, `flux_residual` and `gcl_residual` of the step | `<= 1e-12` each |
| A3 | a2 | this document | the base's result at coincidences registered before any check ran (this commit, alone, first); no coupling built | present |
| B1 | a3 | code | `git diff d40cd665 -- crates/` adds the one test file and changes no other line | exact |
| B2 | a3 | existing suites, unedited | `rcfd-fv` `sliding_periodic_f304`, `sliding_reimaging_f704`, `sliding_coincidence_f710`; `rcfd-mesh` lib (`sliding_periodic` unit tests) | pass as before |
| N1 | negative control (the latent defect) | CN | `flux_residual` on each of the 5 coincidence steps 8, ..., 40; on the other steps from step 2 | `>= 1e-6` each; `<= 1e-12` |
| N2 | negative control (A1 sees the coincidence steps) | CS with the mesh flux of the direct interface faces and of both wrap copies set to zero after `periodic_sliding_step` on the coincidence steps only (their sliver GCL not closed) | the largest velocity measure | `>= 1e-6` |

Estimates: A1, A1c, A2 from the diagnosis (`5.8e-15` and `3.9e-14` on S3, `2.3e-14` and `3.2e-13` on PP; four decades of
margin), S1 likewise (`<= 1.1e-14`). N1 from the diagnosis (`3.98e-4` the smallest). N2 from ADR 0629 SN2 (`2.87e-3`, the same
zeroing on every step of S3); here on 25 of 200 steps, each of which leaves a sliver GCL error of the same size in its step.
The checks A1, A1c, X1, A2, X2, S1, N1 repeat set-ups of the disclosed diagnosis on unchanged code; their outcome is expected
from it, and their limits come from the acceptance (`1e-10`) and the derivations above, not from the printed numbers. Each
registered run is made once.

Mutations (on a scratch copy of the library, after the checks; each applied alone, the named tests run once; each must fail).
`coinc` stands for `new.report.join.n_dropped > 0` inside `periodic_sliding_step`, so that each mutation acts on the
coincidence steps only and shows that the checks see those steps:

- **MU1** on `coinc` steps the GCL closure not applied (`put(&mut m, &gcl.values)` skipped): A1 or S1 must fail.
- **MU2** on `coinc` steps the stator copy of a wrap pair given `+x` instead of `-x` (in `put`, for the mesh flux and the old
  flux): A1, A2 or S1 must fail.
- **MU3** on `coinc` steps the old flux of the direct faces and wrap pairs given the correction only (the interpolation
  `interp_edge` dropped): A2 must fail.

Decision rule: **done** when A1, A1c, X1, A3, B1, B2 and N1, N2 are met and every mutation is caught; **partial** when A1 and
A1c are met but A2, X2, S1, a negative control or a mutation is missed; **todo** when A1 is missed (then the coupling of ADR
0638 is to be applied by an amendment registered before its run, as a2 says). A B2 failure stops the work and is reported (no
existing test is edited).

## Limits (not covered)

- Non-uniform flows at coincidences on the periodic path: the defect exists (N1) and is not fixed here (follow-up proposed).
- The 2-D sector SS with a uniform flow (it has none but zero); the sector of ADR 0216's full annulus is F-710's.
- Steps that do not land on coincidences are covered by F-304 and F-704's own tests (B2), not re-run with new limits.
- Case tree: the periodic path has no case key (ADR 0637 Q3), so no tree check.

## Open questions taken on the standing instruction

- **Q1. Apply the coupling to the periodic path anyway (for the latent defect of N1)?** (a) Not in F-720: the acceptance asks
  for the coupling only if the base misses the uniform flow, and the brief says to record a non-miss with the check only; a
  follow-up with a non-symmetric check and its own limits (taken); (b) apply it now, unchecked by a reference. Taken (a) on the
  maintainer's standing instruction of 2026-11-11 (take the recommendation).
