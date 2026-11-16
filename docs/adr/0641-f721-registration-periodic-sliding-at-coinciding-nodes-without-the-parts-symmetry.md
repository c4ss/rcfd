# ADR 0641: F-721 registration: periodic sliding at coinciding nodes for flows without the parts' symmetry

Status: **REGISTERED 2026-11-16, written before any code of F-721 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-721 was written and no diagnostic run was made; the base numbers
quoted are those of ADR 0640 (its N1, measured on d40cd665 / 0224981b). Results go in the addenda at the end; nothing above
the addenda is changed after a run.

Roadmap item F-721 (deps F-720), proposed by [ADR 0640](0640-f720-registration-periodic-sliding-at-coinciding-nodes.md)
question 1 (the latent defect N1). Parents: [ADR 0638](0638-f710-registration-sliding-interface-at-coinciding-nodes.md)
(F-710: the coupling faces and the coupled solve of `sliding_step`), [ADR 0629](0629-f304-registration-planar-and-sector-sliding-interfaces.md)
(F-304: the periodic join, the pairs PP, SS, S3), [ADR 0634](0634-f704-registration-sliding-beyond-one-period-re-imaging-of-the-rotor.md)
(F-704: turned offsets, re-imaging, the lockstep run F against the full annulus), [ADR 0216](0216-f250-registration-sliding-interface-on-a-cylinder.md)
(F-250: the full-annulus pair P). Number 0641 given by the lead. Branch `f721-work` from 0224981b (the F-720 tip, on the
local main d40cd665 with F-304, F-704, F-303 and F-710), worktree `w49i-work/f721`, scratch `w49i-work/f721-scratch`.

House rules (`w49i-work/AGENT_RULES.md`) apply: limits fixed here; each registered run made once; existing tests not edited;
defaults bit-identical; no edit of `baseline_hashes.rs`, of a hash table or of a golden; no new dependency, no unsafe code;
clean-room; the roadmap files are not edited. Every choice the maintainer would normally be asked about is taken on the
maintainer's standing instruction of 2026-11-11 (take the recommendation), except where listed as open.

Clean-room: the method is ADR 0638's (a weighted minimum-norm flow on a graph is solvable exactly when every connected part's
targets sum to zero; adding edges between parts joins them) applied to a different graph, with one argument from first
principles (below, "Why the seam pairs": the sector graph is the quotient of the full-annulus graph by the pitch rotation,
and a minimum-norm solution of a rotation-invariant problem is rotation-invariant because it is unique). No source was read;
no vendor code, text or install was used. The references are the repository's own full-annulus run (ADR 0216, coupled at
coincidences since F-710) and closed forms (zero divergence).

## What F-721 asks

The acceptance, binding (from the lead's brief):

> On the sector pair SS of ADR 0629 with a source flow periodic in the sector pitch but not in the coincidence spacing, and a step that lands on node coincidences, the remapped old flux of periodic_sliding_step is divergence-free to 1e-12 on every step (ADR 0640 N1 measured 4e-4 to 1e-3 at the base), the coupling of ADR 0638 extended to the graph of direct faces and wrap pairs; the sector run against the full-annulus run of ADR 0216 within a registered limit; every result of F-304, F-704 and F-720 unchanged where no coincidence occurs. Every limit not stated here is registered in the ADR before the run.

Clauses:

- **a1** SS, a source flow periodic in the pitch `pi/3` but not in the coincidence spacing, a step that lands on node
  coincidences: the remapped old flux divergence-free to `1e-12` (`flux_residual`, ADR 0629's measure `max_c abs(sum_f
  sigma_cf phi_old,f) / max_f abs(S_f)`) on every step;
- **a2** by the coupling of ADR 0638 extended to the graph of direct faces and wrap pairs;
- **a3** the sector run against the full-annulus run of ADR 0216 within a registered limit;
- **a4** every result of F-304, F-704 and F-720 unchanged where no coincidence occurs.

**Reading of "on every step" (registered).** On step 1 the old flux is the start flux of `PimpleSolver::new` (the face
interpolation of the start velocity), which is not divergence-free on the start mesh itself (ADR 0640 printed `2.07e-1`;
ADR 0634 S5 and ADR 0640 N1 start their remap checks at step 2 for this reason). No remap can make a flux divergence-free in
cells off the interface whose faces it does not touch. a1 is checked on every step from step 2 (every step whose old flux is
a flux the solver produced); step 1 is printed and covered by B1 (equal to the base bit for bit, no coincidence).

**Reading of "periodic_sliding_step" (registered; question Q1).** F-720's test `n1_non_symmetric_source_flow_leaves_the_part_means_at_coincidences`
(an existing test, not to be edited) asserts that `periodic_sliding_step` leaves `flux_residual >= 1e-6` on the coincidence
steps of this very set-up. A fix in `periodic_sliding_step` itself would make that test fail. So the coupling is built
**opt-in**, as a sibling `periodic_sliding_step_coupled` with the same arguments and result type (rule 3: new behaviour is
opt-in; defaults bit-identical), and a1 to a3 are checked on it. Switching `periodic_sliding_step` itself to the coupled path
(and turning F-720's N1 into an ignored record) changes a default result and an existing test: it is not taken here and is
listed as open for the maintainer.

## What the code read shows (0224981b, read only)

1. `rcfd_fv::sliding_periodic::periodic_sliding_step`: graph edges are the direct interface faces `(own, nei)` then the wrap
   pairs `(own[fa], own[fb])` (rotor copy `fa` carries `x`, stator copy `fb` carries `-x`), weights the face areas (the rotor
   copy's for a pair). M5: `graph_balance` of the GCL targets `t`; M6: `graph_balance` of the remap targets `e`, the new faces
   getting `interp_edge + correction`. `removed_mean` is not looked at.
2. `rcfd_fv::sliding`: `PART_MEAN_TOL = 1e-10`, `coupling_faces`, `coupled_balance` (private) act only in `sliding_step`.
   `coupling_faces` takes internal faces only.
3. `FvGeometry::periodic_pairs()` lists every periodic pair `(f, g)`, `f < g`, of the mesh: on the joined periodic mesh these
   are the rotor's and the stator's seam pairs and the wrap pairs (the patches `sliding_wrap_a/b` are a periodic pair).
4. `rcfd_mesh::sliding_periodic::PeriodicSlidingInterface::reduce`: `k = floor(turned / P)`, `r = turned - k P`.
5. The full annulus P of ADR 0216 (`cylinder_ogrid(144, 8, 1, 1.5)` against `cylinder_ogrid(120, 8, 1.5, 2)`) is six copies
   of SS (`annulus_sector(24, 8, 1, 1.5, pi/3)` against `annulus_sector(20, 8, 1.5, 2, pi/3)`); ADR 0634 F0 matched every
   sector cell to an annulus cell within `1.6e-15`.

## Arithmetic before any run (disclosed, by hand and one line of Python on the floating-point products)

- SS: rotor nodes at `delta + i pi/72`, stator nodes at `j pi/60`; they meet when `delta` is a multiple of `pi/360`. With
  `dt = pi/576` (the rotor pitch over 8, ADR 0640's CN) `n pi/576 = m pi/360` needs `5 n = 8 m`: steps `n = 8 k` land on
  coincidences, at the turned offset `k pi/72` (a whole rotor pitch). At a whole rotor pitch every rotor node lies on a
  multiple of `pi/72`, and the coinciding angles are the multiples of `pi/12` in `[0, pi/3]`: 4 coinciding nodes per sector,
  the stator seam (`0 = pi/3`) always one of them; the graph falls into 4 parts (ADR 0640 N1 printed 4).
- The rotor seam (at the reduced offset `r = (k mod 24) pi/72`) is a coinciding node when `r` is a multiple of `pi/12`, i.e.
  `k mod 6 == 0`. At `k mod 24 == 0` (`r = 0`) the two seams coincide and there are no wrap pairs.
- Re-imaging: Python gives `192 * (pi/576) == pi/3` and `384 * (pi/576) == 2 pi/3` exactly in binary64, so `reduce` returns
  `(1, 0.0)` at step 192 and `(2, 0.0)` at step 384: the re-imaging steps are **192 and 384**, both coincidence steps with
  `r = 0` (both seams coincide). With 400 steps: 50 coincidence steps (8, 16, ..., 400), of them `k mod 6 == 0` at `k = 6,
  12, ..., 48` (8 steps, among them 192 and 384 with `r = 0`).
- The full annulus at `theta = n pi/576`: rotor nodes `theta + i pi/72`, stator `j pi/60`, the same coincidence steps; at a
  whole rotor pitch 24 coinciding nodes and 48 coupling faces (ADR 0638 S2).
- PP (supporting): slider nodes `delta + i/20`, stator `j/16`, meeting at multiples of `1/80`; `dt = 1/640`: steps `8 k`
  land on coincidences (ADR 0640), 4 coinciding nodes per period.

## Design

**D1. Coupling edges on the periodic graph.** On a step where the base solve of a target vector (the GCL targets `t`, the
remap targets `e`) removed a part mean above `1e-10` of the largest target over the graph's cells (ADR 0638 D3's threshold,
the same constant), the parts are coupled through the remaining faces between them. The candidates, with the connected parts
of the base graph (direct faces and wrap pairs):

- (i) an internal face that is not a direct interface face, whose owner and neighbour are both cells of the graph and lie in
  different parts: an edge `(own, nei)` (ADR 0638 D1);
- (ii) a seam pair: a periodic pair `(f, g)` of `FvGeometry::periodic_pairs()` that is not a wrap pair, whose two owner cells
  are both cells of the graph and lie in different parts: an edge `(own[f], own[g])`, weight `|S_f|`, the value `x` added to
  `f` and `-x` to `g` (antisymmetric, as the solver's periodic coupling and the wrap pairs).

**Why the seam pairs (first principles).** The full annulus P is the union of six rotated copies of SS, and the interface
graph of P with ADR 0638's coupling faces is invariant under the pitch rotation. The sector's graph (direct faces, wrap
pairs) is its quotient by that rotation: a wrap pair is the image of an annulus interface face that crosses the stator seam
angle. An annulus coupling face lying on a seam angle (the stator's radial face at `0` when a rotor node meets the stator
seam; the rotor's radial face at `r` when the rotor seam meets a stator node) has as its quotient a seam pair, not an
internal face. With (i) alone the sector's extended graph is connected too (each cut has an internal face, except the seam
cut at `r = 0`, where the four parts still form a chain), so (i) alone would meet a1; but its minimum-norm solution would
differ from the annulus's. With (i) and (ii) the extended sector graph is exactly the quotient of the extended annulus graph.
For targets periodic in the pitch the annulus's weighted minimum-norm solution is unique and so invariant under the
rotation; the periodic solutions of the annulus problem are the lifts of the solutions of the quotient problem, with six
times the norm; so the sector's minimum-norm solution is the annulus's restricted to one sector. This is checked directly
(Q, below). This settles ADR 0640 Q1's "registered decision on seam pairs where both seams coincide": seam pairs are coupling
edges whenever their cells lie in different parts, at `r = 0` both seam pairs are (the quotient of the annulus's two faces at
the node on the seam).

Count on SS at a coincidence step (arithmetic): 2 coupling edges per coinciding node (the rotor's radial face and the
stator's radial face ending there), **8 per step**; seam pairs among them: the stator seam pair always; the rotor seam pair
too when `k mod 6 == 0`; so **2 when `k mod 6 == 0`, else 1**.

**D2. The coupled solve.** As ADR 0638 D2: the solve is made again with `graph_balance` on the edges "direct faces, wrap
pairs, coupling faces (i), seam pairs (ii)" (weights the face areas) and its values replace the base values: on the direct
faces and wrap pairs set as now (`m = x`, `phi = interp + x`; `-` on the stator copy), on a coupling face or seam pair
**added** to the mapped value (`+x` on `f`, `-x` on `g` for a seam pair). `old_defect` is not changed (ADR 0638 D2). When the
threshold is not exceeded, or no coupling edge exists, the base values are used: the same operations in the same order as
`periodic_sliding_step`. The coupling edges are found once per step, only when one of the two solves needs them.

**D3. API, opt-in.** `crates/rcfd-fv/src/sliding_periodic.rs` only: the body of `periodic_sliding_step` moves into a private
function with a flag `couple`; `periodic_sliding_step` calls it with `false` (every operation as before); the new public
`periodic_sliding_step_coupled` (same arguments, same `PeriodicSlidingStep`) calls it with `true`. `PeriodicSlidingStep`
keeps its fields. `sliding.rs` (and its `PART_MEAN_TOL`), `graph_balance`, `pimple/moving.rs` and `rcfd-mesh` are not changed
(the constant is repeated in `sliding_periodic.rs` with a reference to ADR 0638 D3). No case key (the periodic path has none,
ADR 0637 Q3), no golden, no hash table. Unit tests T1 in `sliding_periodic.rs`; the checks in a new test file
`crates/rcfd-fv/tests/sliding_periodic_coupled_f721.rs`. `docs/dynamic-mesh.md` gets one paragraph.

## Set-ups

PS6 of ADR 0629 (`piso(0.1)`, central convection, six non-orthogonal correctors, solvers `tol_abs 1e-16`, `tol_rel 1e-13`,
5000 iterations). ADR 0634's turned loop (`mesh_turned(n dt)`, `Omega = 1`).

- **NS** (the source flow of a1, ADR 0640's CN): on SS, inlet the rotor's `inner` with `u_b = U_in (1 + 0.5 sin 6 theta)
  x_hat` at the new face centres every step (`theta = atan2(y, x)`, `U_in = 1`): periodic in `pi/3`, not in the coincidence
  spacing `pi/12` (nor `pi/6`); outlet `outer` with `p = 0` and zero-gradient velocity; start `u = (r_i / r) x_hat`. On P the
  same with `cylinder` and `farfield` (ADR 0634's annulus loop).
- **LS** (lockstep, a1 to a3): `dt = pi/576`, **400 steps** (2.08 pitches; coincidences at the 50 steps `n % 8 == 0`;
  re-imaging at 192 and 384, both with `r = 0`). Three solvers advance together: **C** NS on SS through
  `periodic_sliding_step_coupled`; **B** NS on SS through `periodic_sliding_step` (the base, control); **A** NS on P through
  `sliding_step` (coupled since F-710) and `step_sliding`. After every step every sector cell is matched to the nearest annulus
  cell (ADR 0634 F).
- **Q-steps** (a2, on LS's 50 coincidence steps, before the solvers advance): the sector's old state is taken from the
  annulus's: every cell of C's old mesh gets the velocity of the nearest annulus cell of A's old mesh, every non-empty face
  the flux of the nearest annulus face times `sign(S_f . S_g)` (empty faces 0). From that state the sector step data are built
  with `periodic_sliding_step_coupled` (and with `periodic_sliding_step`, control QN) on C's old and new meshes, and compared
  with A's step data (`sliding_step` from A's own state) on the nearest faces of A's new mesh, with the same sign. This tests
  D1's quotient argument on the step itself, with inputs that are equal by construction; the annulus's state is periodic in
  the pitch up to its solver tolerance.
- **PL** (supporting, the translation): PP with ADR 0634's PF (inflow through the stator's `ymin`, `u_b = (0, 0.5 (1 + 0.5
  sin 2 pi x), 0)`, outlet `ymax`), `dt = 1/640`, 160 steps (20 coincidence steps), through `periodic_sliding_step_coupled`;
  the base `periodic_sliding_step` on the same inputs every step.
- **UC** (a4, F-720's set-ups): CS (S3, `U = (0, 0, 1)`, `dt = pi/288`, 200 steps) and CP (PP, `U = (1, 0.5, 0)`, `dt =
  1/640`, 650 steps) of ADR 0640, run with `periodic_sliding_step`, and on every step the coupled step data built from the
  same inputs.

## Checks (limits fixed now)

| Id | Clause | Set-up | Check | Limit |
| --- | --- | --- | --- | --- |
| A1 | a1 | LS, C | `flux_residual` of every step from step 2 (399 steps); printed separately over the 50 coincidence steps and the 2 re-imaging steps | `<= 1e-12` each |
| A1g | supporting | LS, C | `gcl_residual` of every step | `<= 1e-12` each |
| X1 | set-up | LS, C | 4 parts and dropped overlaps exactly on the steps `n % 8 == 0`, 1 part and none dropped elsewhere; re-imaging steps; the reduced offset on them | exact; `{192, 384}`; `0` |
| S2 | set-up of D1 | LS, C's new mesh on every coincidence step | coupling edges found by D1's rule (recomputed in the test): 8; seam pairs among them 2 when `k mod 6 == 0`, else 1 | exact |
| Q | a2 | Q-steps (50) | `max_f abs(phi_old^sector_f - s_f phi_old^annulus_g) / max_f abs(S_f)` over the non-empty faces of the new sector mesh, and the same for the mesh flux | `<= 1e-10` each |
| QX | set-up of Q | Q-steps | every sector cell's and non-empty face's nearest annulus cell / face centre within `1e-9`; the cell match one-to-one | as stated |
| F0 | set-up of F | LS | every step: every C cell's nearest annulus cell within `1e-9`, one-to-one | as stated |
| F1 | a3 | LS | every step `max_c abs(u_c^C - u_c^A) / U_in`; printed separately over the coincidence steps | `<= 3e-2` |
| F2 | a3 | LS | last step, the volume-weighted rms of the same difference | `<= 1.5e-2` |
| B1 | a4 | LS, C | on every step, the base step data from the same inputs (`periodic_sliding_step` on C's state): `mesh_flux`, `phi_old`, `old_defect`, `vol_old`, residuals bit-equal on the 350 steps without coincidence; differing on exactly the 50 coincidence steps | exact |
| B1u | a4 (F-720) | UC | coupled against base step data on every step of CS (200) and CP (650), coincidence steps included | bit-equal every step |
| B2 | a4 | existing suites, unedited | `rcfd-fv` lib, `sliding_periodic_f304`, `sliding_reimaging_f704`, `sliding_periodic_coincidence_f720`, `sliding_coincidence_f710`; `rcfd-mesh` lib | pass as before |
| B3 | a4 | code | `git diff 0224981b -- crates/` touches `sliding_periodic.rs`, `lib.rs` only if needed for an export, and the new test file; no line of `sliding.rs`, `pimple/`, `rcfd-mesh`, `rcfd-api` | as stated |
| A2 | supporting | PL | coupled `flux_residual` every step from step 2; base printed (no limit) | `<= 1e-12` |
| T1 | D1, D2 (also the GCL branch) | unit, SS at `r = pi/72` and `r = 0` | coupling edges 8 with 1 and 2 seam pairs; the coupled solve of a synthetic target (zero total over the graph, part means of order 1) meets every cell's target | exact; residual `<= 1e-12 max abs(t)` |
| N1 | control of A1 | LS, B | B's `flux_residual` on each of the 50 coincidence steps | `>= 1e-6` each |
| QN | control of Q | Q-steps with `periodic_sliding_step` | Q's flux measure, largest | `>= 1e-6` |

Printed, no limit: F1 of B against A (the base sector against the annulus); C and B's largest velocity difference; A's own
`flux_residual` on the coincidence steps; elapsed time.

Estimates. A1: on a connected extended graph whose total target is zero the coupled solve closes every cell to the solve's
rounding (ADR 0638 S1 measured `1.2e-15`; ADR 0640's non-coincidence steps `2.5e-14`); the total is zero from step 2 because
the cells off the graph keep their (divergence-free) mapped flux and the seam pairs' mapped fluxes cancel. A1g: the GCL part
means on SS are rounding by the `pi/12` symmetry of both node sets (ADR 0638 arithmetic 1); measured `<= 1.0e-14` in ADR 0640
CN. Q: inputs equal by construction, geometry equal to rounding (`1.6e-15`), the conjugate-gradient solves converged to
`1e-14` of their targets, the annulus state periodic to its solver tolerance (`1e-13` relative): `<= 1e-12` expected, two
decades of margin. F1: ADR 0634 F1 measured `8.0e-3` against a limit `2e-2` for the uniform inlet `U_in` over 640 steps; the
inlet here reaches `1.5 U_in`, so `2e-2 . 1.5 = 3e-2`; F2 likewise `1e-2 . 1.5`. The coincidences add no difference beyond
rounding when Q holds. N1, QN: ADR 0640 N1, `3.98e-4` the smallest of five. T1: the solve to rounding (`1e-14` of the
targets per pass, three passes). LS takes three solvers over 400 steps (ADR 0634 F: three solvers, 640 steps, 35 s); if its
one run takes more than 120 s the test is marked `#[ignore]` afterwards as a heavy record (no limit changes), T1 and PL
covering the code path fast.

Mutations (on a scratch copy of the library, after the checks; each applied alone; the named tests run once; each must fail):

- **MU1** the coupled path never taken (the trigger always false): A1 must fail (and Q).
- **MU2** the seam pairs left out of the coupling edges (internal faces only): Q must fail. A1 is **not** expected to fail
  (registered: the extended graph stays connected through the internal faces, D1).
- **MU3** a seam pair's partner value given `+x` instead of `-x` in the remap: A1 or Q must fail.
- **MU4** the threshold removed (couple whenever coupling edges exist): B1u must fail.
- **MU5** the coupling-edge values set (`=`) instead of added (`+=`) on the coupling faces and seam pairs in the remap: A1 or Q
  must fail.

Decision rule: **done** when A1, X1, S2, Q, QX, F0, F1, F2, B1, B1u, B2, B3, T1 and the controls N1, QN are met and every
mutation is caught; A1g and A2 are supporting and reported. **Partial** when A1 is met but Q, F1, F2, B1, B1u, a set-up
check, a control or a mutation is missed. **Todo** when A1 is missed. A B2 failure stops the work and is reported (no
existing test is edited). Whatever the outcome, the switch of `periodic_sliding_step` itself to the coupled path is open for
the maintainer (Q1); the lead may prefer to record partial until it is made.

## Limits (not covered)

- `periodic_sliding_step` itself keeps the base path (opt-in, Q1); no case-tree key for the periodic path.
- The GCL branch of the coupled solve runs only in T1 (on SS and PP the GCL part means are rounding by symmetry).
- The 3-D sector S3 with a non-uniform flow is not run (the acceptance names SS); PL covers the translation.
- A part without any coupling edge keeps the base result.
- The velocity effect of the coupling against a reference is measured only through F1 against the annulus, whose own
  difference from the sector (the orthogonal coupling of the wrap and seam faces, ADR 0629 Q4) dominates.

## Open questions taken on the standing instruction

- **Q2. Seam pairs as coupling edges.** (a) Internal faces and seam pairs (D1; the quotient of the annulus's coupling, checked
  by Q) (taken); (b) internal faces only (meets a1 but differs from the annulus). Taken (a) on the maintainer's standing
  instruction of 2026-11-11 (take the recommendation).
- **Q3. Reading of "every step".** From step 2 (the start flux is not divergence-free; above) (taken); (b) a divergence-free
  start flux constructed for the test. Taken (a) on the standing instruction.

## Open (not taken)

- **Q1. Make the coupling the default of `periodic_sliding_step`** (as F-710 did for `sliding_step`). It changes default
  results at coincidence steps and makes F-720's existing test N1 fail (it would become an ignored record). Needs the
  maintainer.

## Amendment 1 (2026-11-16, written after the registered run and before any corrected run)

The registered run (code and tests 9413b5ea, run once, log `w49i-work/f721-scratch/run.log`) met T1, A2, A1, A1g, S2, Q, QX,
F0, F2, B1, N1 and QN, and missed three checks. One diagnostic run followed (disclosed; scratch test
`w49i-work/f721-scratch/zz_f721_diag.rs`, moved out of the worktree after the run and never committed, log `diag.log`). No
limit is changed. Results of the registered run go in addendum 1.

**X1 missed: an arithmetic error of this registration (a defective set-up check).** The pitch of SS is `FRAC_PI_3`, whose
binary64 value is one unit in the last place above `PI / 3.0`; the registration's Python line compared `192 * (pi/576)` with
`pi/3` computed as a quotient. With the constant: `192 dt = FRAC_PI_3 - 2.2e-16` and `384 dt = 2 FRAC_PI_3 - 4.4e-16`, so
`reduce` gives image 0 at step 192 (reduced offset `P - 2.2e-16`) and image 1 at step 384 (`P - 4.4e-16`), and the re-imaging
steps are **193 and 385** with the reduced offset `193 dt - P = 5.454e-3` (one step). The run printed exactly that. The
coincidence pattern of X1 held (4 parts exactly on `n % 8 == 0`). Steps 192 and 384 are still the steps where both seams
coincide (geometrically the offset `P` is the offset `0`), but in the representation with every overlap a wrap overlap and
no direct face of positive area; the exact `r = 0` case is covered by T1 only. Corrected check, **X1c**: the coincidence
pattern as X1; the re-imaging steps exactly `{193, 385}` with reduced offset within `1e-12` of `dt`; on steps 192 and 384 the
reduced offset within `1e-15` of `P`.

**F1 missed (a3): diagnosed, not corrected.** Measured `3.494e-2` against `3e-2`. The diagnosis printed the largest
difference per step: it is reached on **step 1**, equal for the coupled sector C and the base sector B (`3.4941e-2` both), at
the rotor's inner-ring cell next to the rotor seam (`r = 1.031`, `theta = 1.031`, the inlet ring, where the inlet's
modulation `sin 6 theta` has its largest tangential gradient at the seam angle). On step 1 no coincidence and no coupling
acts (B1: C equals B bit for bit), so the miss is the start transient through the seam faces' orthogonal coupling (ADR 0629
Q4) under an inlet that the start field `(r_i/r) x_hat` does not match; the estimate scaled ADR 0634's limit for a uniform
inlet and did not foresee the modulated inlet's start. From step 2 the diagnosis printed at most `1.93e-2` (C) and `1.96e-2`
(B). F1 stays a miss (the record keeps it); no new limit is registered for it.

**B1u missed (stricter than a4): diagnosed, not corrected.** On every coincidence step of F-720's uniform runs (CS 25 steps,
CP 81) the coupled step data differ from the base's. The diagnosis printed the size: on CS the old flux differs by at most
`4.8e-16 max|S_f|` and the mesh flux not at all, on CP `3.2e-15` and `9.8e-14`; the flux residuals are equal. Cause: for a
uniform flow every remap target (and on CP every GCL target) is itself rounding, so D2's threshold `1e-10 max_c |t_c|` is
relative to a rounding-level scale and does not keep the base path; the coupled solve then redistributes rounding. a4 asks
for unchanged results where no coincidence occurs, which B1 (350 of 350 steps bit-equal) and B2 cover; B1u asked more and
is missed. Consequence for the mutations: MU4 (threshold removed) cannot be caught by B1u; it is run and reported as it
comes out.

**Records and the corrected test.** The registered tests `ls_...` and `b1u_...` become ignored records named `*_known_miss_*`
with their limits intact and the measured numbers in the ignore text. A corrected test `ls_corrected_...` runs LS once more
(the run is deterministic) and asserts A1, A1g, X1c, S2, Q, QX, F0, F2, B1, N1 and QN with the registered limits; it prints
F1 (all steps, and from step 2) against its registered limit without asserting it (the record holds the miss). No other
change.

## Addendum 1: results (2026-11-16)

Measured against the limits registered above and in amendment 1; no limit was changed. Release builds on Windows. The
registration is e9bc2b44 (alone, first); the code and the tests 9413b5ea (committed before their run); amendment 1 f64fa647;
the known-miss records and the corrected test fe8ad09e (committed before the corrected run). The registered run (T1, then
the three tests, 20.3 s) was made once; one diagnostic run followed (amendment 1); the corrected test ran once (21.7 s, the
same deterministic numbers as the registered run for every shared check).

**Built as registered** (D1 to D3). `crates/rcfd-fv/src/sliding_periodic.rs` (shared solver file): module text lines 16 to
19; `periodic_sliding_step` now calls `step_impl(.., false)`, and `periodic_sliding_step_coupled`, `PART_MEAN_TOL`,
`Coupling`, `coupling_edges`, `coupled_solve`, `add_coupling` and the head of `step_impl`, lines 95 to 234; the M5 assignment,
lines 306 to 321; the M6 assignment, lines 396 to 414; the unit test T1, lines 441 to 514. Nothing else in the body of the
step changed. New `crates/rcfd-fv/tests/sliding_periodic_coupled_f721.rs` (4 tests: the corrected LS and PL run; the LS and
B1u records are ignored). `docs/dynamic-mesh.md`: the periodic paragraph. `sliding.rs`, `pimple/`, `rcfd-mesh`, `rcfd-api`,
goldens, hash tables and existing tests: untouched.

### Results

| Check | Limit | Measured | Verdict |
| --- | --- | --- | --- |
| A1 (a1) C `flux_residual`, steps 2 to 400; the 50 coincidence steps; the re-imaging steps | `<= 1e-12` each | `2.46e-14`; `2.57e-15`; `7.46e-16` (step 1, the start flux: `2.07e-1`, printed) | met |
| A1g C `gcl_residual`, every step | `<= 1e-12` | `1.17e-14` | met |
| X1 (set-up, as registered) re-imaging `{192, 384}` at offset 0 | exact | `{193, 385}` at offset `dt`; the coincidence pattern held | **missed** (arithmetic error, amendment 1) |
| X1c (amendment 1) pattern; re-imaging `{193, 385}`, offset `dt`; steps 192, 384 within `1e-15` of `P` | exact; `1e-12`; `1e-15` | as stated; `0`; `2.2e-16`, `4.4e-16` (images 0, 1) | met |
| S2 coupling edges per coincidence step; seam pairs | 8; 2 if `k mod 6 == 0` else 1 | 8 on all 50 (7 + 1 on 42, 6 + 2 on 8) | met |
| Q (a2) sector step from the annulus's state against the annulus step: old flux; mesh flux | `<= 1e-10` each | `2.22e-14`; `1.56e-13` | met |
| QX nearest centres on the Q-steps; one-to-one | `<= 1e-9` | `1.73e-15`; yes | met |
| F0 cell match every step | `<= 1e-9` | `1.45e-15`, one-to-one | met |
| F1 (a3) `max abs(u_C - u_A) / U_in`, 400 steps | `<= 3e-2` | `3.494e-2` (step 1; base B the same); coincidence steps `1.94e-2`; from step 2 `2.67e-2` | **missed** |
| F2 (a3) rms at the last step | `<= 1.5e-2` | `8.63e-3` | met |
| B1 (a4) coupled = base bit for bit off coincidences; differing on the coincidence steps | 350 equal, 50 differ | 350, 50 | met |
| B1u (a4, stricter) coupled = base on every step of F-720's CS and CP | bit-equal | differ on every coincidence step (25, 81), by `<= 4.8e-16` / `3.2e-15` of `max abs(S_f)` in the old flux (diagnosis) | **missed** (amendment 1) |
| B2 (a4) existing suites, unedited | pass | `rcfd-fv` lib 110/6 (109 before plus T1), `sliding_periodic_f304` 8/2, `sliding_reimaging_f704` 12/2, `sliding_periodic_coincidence_f720` 4/0 (its N1 still holds: the default is unchanged), `sliding_coincidence_f710` 3/0; `rcfd-mesh` lib 77/2; `rcfd-api` `baseline_hashes` 3/1 | met |
| B3 (a4) diff of `crates/` | `sliding_periodic.rs` and the new test file only | as stated | met |
| A2 (supporting) PL coupled `flux_residual` from step 2 | `<= 1e-12` | `4.37e-15` (base on the coincidence steps `1.88e-4`, printed) | met |
| T1 coupling edges at `r = pi/72`, `r = 0`; residual | 7 + 1, 6 + 2; `<= 1e-12` | as stated; `4.2e-16`, `2.1e-16` | met |
| N1 (control) base B `flux_residual` on the 50 coincidence steps | `>= 1e-6` each | `3.98e-4` to `2.19e-3` | held |
| QN (control) Q with the base step | `>= 1e-6` | `1.82e-2` | held |

Printed, no limit: the annulus's own `flux_residual` on its coincidence steps `7.5e-16` (F-710); C against B (the coupling's
effect on the velocity) `4.15e-4 U_in`; base B against the annulus `3.494e-2 U_in` (the same as C: F1's maximum is the
step-1 transient). Correction to amendment 1's text (not a limit): it said the diagnosis printed at most `1.93e-2` from step
2; that diagnosis printed only every 50th step and each new maximum, and the corrected test's full maximum from step 2 is
`2.67e-2`.

### Mutations (scratch copy `w49i-work/f721-scratch/mut` of fe8ad09e, script `mutate.py`, log `mutations.log`, target `target-mut`)

| Mutation | Must fail | Caught by |
| --- | --- | --- |
| MU1 coupled path never taken | A1 (and Q) | A1 (`2.19e-3`), Q (`1.82e-2`), B1 (no step differs), A2 (`1.88e-4`), T1 |
| MU2 seam pairs left out | Q (A1 registered not to fail) | Q (`5.49e-3`), T1; A1 met (`2.46e-14`), as registered |
| MU3 seam partner `+x` | A1 or Q | A1 (`1.07e-2`), Q (`1.10e-2`), A2 (`5.56e-4`) |
| MU4 threshold removed | B1u | **not by B1u** (it already misses at the base); caught by Q's mesh flux (`2.70e-3`): without the threshold the GCL solve of SS couples on rounding-level part means and moves the sliver fluxes away from the annulus's |
| MU5 coupling values set, not added | A1 or Q | A1 (`6.52e-2`), Q (`6.48e-2`), A2 (`4.57e-2`) |

Five of five caught, MU4 by a different check than registered. The file was restored and compared after each.
`cargo fmt -p rcfd-fv --check` clean; `cargo clippy --release -p rcfd-fv --all-targets -- -D warnings` clean. Not run: the
whole workspace suite (the lead's gate).

### Decision

By the registered rule: A1 met, but F1 (a3) missed, the set-up check X1 missed as registered (corrected X1c met), B1u
missed and MU4 caught only by another check: **partial**. What is shown: a1 (divergence-free to `2.5e-14` on every step from
the second, `2.6e-15` on the 50 coincidence steps) and a2 (the sector's coupled step equals the full annulus's coupled step
to `2e-14` from the same state, so the extension to direct faces, wrap pairs and seam pairs is the annulus's coupling seen
in one sector). a4 holds where no coincidence occurs. a3 misses by the step-1 start transient at the rotor seam next to the
modulated inlet, the same for the base sector and unrelated to the coincidences. The coupling is opt-in, so
`periodic_sliding_step` itself is unchanged (Q1, open).

### Remaining questions

1. **Q1 (open, not taken): the default.** Options: (a) switch `periodic_sliding_step` to the coupled path, with F-720's N1
   test becoming an ignored record (it changes default results on coincidence steps and an existing test: the maintainer's
   approval); (b) keep it opt-in. Recommendation (a), after question 3, for consistency with F-710's `sliding_step`.
2. **F1.** Options: (a) a follow-up that registers a limit for the sector-against-annulus difference under this inlet from
   a derivation that includes the start transient at the seam (or starts both runs from a state consistent with the inlet),
   and runs it once; (b) accept F2 and Q as the a3 evidence. Recommendation (a); Q already shows that the step matches the
   annulus to rounding, so the remaining difference is the solver's treatment of the seam faces (ADR 0629 Q4).
3. **The threshold scale (B1u, MU4).** A threshold relative to the largest target does not keep the base path when every
   target is rounding (uniform flows). Options: (a) scale it by a flux scale of the graph's faces (for example the largest
   mapped face flux) in both `sliding_step` and the periodic step, so that rounding-level means never couple; (b) leave it
   (the changes are rounding, `<= 1e-13`). Recommendation (a), as a follow-up with its own registration, since it touches
   F-710's `sliding_step` (default results).

## For the lead: roadmap entries

> Proposed status: partial.

> ADR 0641, 2026-11-16: periodic sliding at coinciding nodes for flows without the parts' symmetry. New opt-in rcfd_fv::sliding_periodic::periodic_sliding_step_coupled (the same arguments as periodic_sliding_step, which is unchanged): when a part of the graph of direct faces and wrap pairs keeps a target mean above 1e-10 of the largest target, the GCL closure and the remap are solved together with the faces between the parts, internal faces and seam pairs (8 edges per coincidence step on the 60-degree sector, the quotient of F-710's coupling on the full annulus). On the sector pair SS with an inlet (1 + 0.5 sin 6 theta) x_hat and dt = rotor pitch / 8 over 400 steps (50 coincidence steps, two re-imagings) the remapped old flux is divergence-free to 2.5e-14 on every step from the second (limit 1e-12; the base 4.0e-4 to 2.2e-3 on the coincidence steps); the sector step built from the full annulus's state equals the annulus's step to 2.2e-14 (old flux) and 1.6e-13 (mesh flux), the base step 1.8e-2; off coincidences the coupled step equals the base bit for bit (350 of 350 steps). Against the full-annulus run of ADR 0216 in lockstep: rms 8.6e-3 U_in at the end (limit 1.5e-2), but the largest difference 3.49e-2 U_in against the registered 3e-2, reached on step 1 by the start transient at the rotor seam next to the modulated inlet, the same for the base sector (known miss, diagnosed; 2.7e-2 from step 2). Also missed: a registered re-imaging-step set-up check (an arithmetic error, FRAC_PI_3 is one ulp above pi/3; corrected check met) and a check that the coupled step equals the base on F-720's uniform runs at coincidence steps too (they differ by rounding, 3e-15: the threshold is relative to rounding-level targets). Every F-304, F-704, F-720 and F-710 test and baseline_hashes pass unedited. Five of five mutations caught (one by another check than registered). Open: making the coupling the default of periodic_sliding_step (changes F-720's N1 test; maintainer).

> Follow-up proposed: "Threshold scale of the coupled interface solves". Deps F-721, F-710. Acceptance: the part-mean threshold of sliding_step and periodic_sliding_step_coupled is scaled by a flux scale of the interface faces rather than by the largest target, registered from a derivation; the coupled step equals the base step bit for bit on every step of the uniform flows of ADR 0638 and ADR 0640 (coincidence steps included), and every result of ADR 0638 and ADR 0641 at coincidences with a non-symmetric flow is kept within registered limits; then periodic_sliding_step is switched to the coupled path with the maintainer's approval (ADR 0640's N1 test becomes an ignored record). Every limit not stated here is registered in the ADR before the run.

> Follow-up proposed: "Sector against full annulus with a non-axisymmetric inlet". Deps F-721. Acceptance: the lockstep of ADR 0641 (sector SS and the full annulus of ADR 0216, inlet periodic in the pitch, steps through node coincidences) with a limit on the largest velocity difference derived before the run to include the start transient at the seams (or from a start state consistent with the inlet), met on every step; the seam faces' orthogonal coupling (ADR 0629 Q4) identified as the source of the remaining difference or ruled out. Every limit not stated here is registered in the ADR before the run.
