# ADR 0359: F-442 registration: the step-size-dependent remainder made by the motion through the overset coupling

Status: **REGISTERED 2026-11-12, written before any code of F-442 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-442 was written, compiled or run: the documents and the code under "What was read"
were read and the arithmetic under "Arithmetic before this registration" was done by hand. Results go in the addendum at the
end; nothing above the addendum is changed after a run.

Parents: [ADR 0245](0245-f344-registration-bdf2-step-remainder.md) (F-344: A2, H9, H10, question 1, from which this item
comes; the opt-in C, `rhie_chow_step_consistent`), [ADR 0241](0241-f338-registration-overset-bdf2.md) (F-338: BDF2 on the
overset step, TG_2, D2), [ADR 0219](0219-f134-registration-overset-coupling-of-a-moving-component-mesh.md) (the overset step, M2 to M10),
[ADR 0251](0251-f336-registration-overset-quadratic-donor-interpolation.md) (the quadratic donor stencil). Roadmap item F-442 (depends
on F-344). Number 0359, branch `f442-overset-bdf2-remainder` from main 8a7d8570, worktree `w49i-work/f442`, scratch
`w49i-work/f442-scratch`, given by the lead.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run; a miss becomes an ignored
`*_known_miss` test with the limit intact; a defective set-up is corrected only by an amendment committed before the corrected
run; each registered run is made once; defaults bit-identical; no hash table, golden or existing test edited; no new crate, no
unsafe code; no commit trailers, no push. Clean-room: everything used is derived below. Names of ideas, recalled from memory,
nothing opened: bilinear interpolation on the dual cell of the cell centres; moving least squares with a compactly supported
weight (P. Lancaster and K. Salkauskas, Math. Comp. 37 (1981) 141-158); the loss of order of a time integrator at a
discontinuity of its forcing (any text on ODE solvers, e.g. Hairer, Norsett, Wanner). No formula is taken from them unverified.

## What F-442 asks

The acceptance of `docs/roadmap.json`, binding in full:

> The remainder of TG_2 with BDF2, three outer iterations and rhie_chow_step_consistent (max abs(d_200) 2.057e-5 with the ratio
> 2.454 to max abs(d_400), ADR 0245 A2; 1.3e-7 with the ratio 4.98 on the coupled meshes of level 1 at rest against 5.2e-5
> moving, ADR 0245 H10) is traced to a named event or term of the moving coupling with a number, and either removed as an
> opt-in (defaults bit-identical; D2 of ADR 0241 at least 4 on TG_2) or the ADR names why not with a number.

**Reading (registered here).**

1. TG_2 is the set-up of ADR 0245 A2 (`tg2_run` of `bdf2_remainder_f344.rs`): the level-2 background (80 by 80 cells of
   `[0, pi]^2`), the 40 by 40 component square of half side 0.6, hole `max(abs(x'), abs(y')) < 0.25` in the component frame,
   `tg_pose(t)` (rotation `t`, translation `(0.5 t, 0.3 t)`), the exact Taylor-Green velocity on the four sides, `nu = 0.1`, PS
   of ADR 0219, `MassMode::Conservative`, BDF2, three outer iterations, C on, `N` steps to `t = 0.5`. `d_N` is the difference of
   the velocities of the runs in `N` and 800 steps over the decay, over the non-hole cells; D2 is `max abs(d_200) / max
   abs(d_400)`. TG_1 is the same at level 1 (40 by 40 background, 20 by 20 component), `N` = 100, 200, 400.
2. "Traced to a named event or term of the moving coupling with a number": an event of the coupling (a thing that happens to a
   cell or a stencil at a moment fixed by the motion) or a term, written down, and a measurement in which removing that event
   alone removes the remainder, with the deciding number registered here.
3. The two ways: (i) an opt-in that removes it, with D2 at least 4 on TG_2 and the defaults bit for bit; (ii) the ADR names
   why not with a number. A miss of D2 by a built opt-in is a miss of way (i), reported as such.

## What was read (no run)

- ADR 0245 in full (the derivation of C, amendments 1 to 3, both addenda); ADR 0241 (the set-ups, D2, H16 to H19, the
  amendment on the misses); the roadmap entries F-344, F-338, F-442; ADR 0356 (the step-consistent intermediate flux of the
  fractional step: the same carry-over argument as C, on another path; it does not touch the overset step).
- `crates/rcfd-fv/src/overset.rs`: `prepare`, `advance` (the kept faces, the carried defect, M9, the BDF2 data D2 to D4),
  `piso` (M5 to M8, the C branch), `Bdf2Step::add_ddt`. `crates/rcfd-mesh/src/overset.rs`: `connectivity`, `linear_weights`,
  `Stencil`, `Connectivity`, `DonorInterpolation`. `crates/rcfd-fv/tests/bdf2_remainder_f344.rs`: `tg2_run`, `tg1_run`,
  `tg2_difference`, `a2_diagnosis_of_the_coupled_remainder`.

What the code shows (the facts this registration builds on):

1. **The linear stencil of a fringe cell is chosen by its donor cell** (`connectivity`): the cell of the other part that
   contains the fringe centre, and the calculated face neighbours of that cell, with the least-squares weights of
   `linear_weights` (exact for linear fields). When the moving fringe centre crosses a face of the other part's mesh, the donor
   changes and with it the whole stencil. Both stencils reproduce a linear field, so **the interpolated value jumps**, at that
   moment, by the difference of the two interpolation errors, `O(h^2 |D^2 u|)`. The same holds for the quadratic stencil of
   ADR 0251 (one order higher). This happens to every fringe cell of both parts, at moments fixed by the motion and not by the
   step.
2. **Cells change kind** at moments fixed by the motion (background cells only: the component's kinds are fixed, its outer
   ring is always fringe): calculated to fringe ahead of the hole, fringe to hole, hole to fringe, fringe to calculated behind
   it. A cell that becomes calculated starts its time history from fringe values (`u_old`, and for BDF2 the old-old value); a
   face that was not "kept" (between two fringe cells, or next to a hole) starts with zero carried defect (`kept`, `defect`).
3. The rest of the moving coupling (the moving component mesh, its mesh flux and volumes) is the plain moving-mesh step, which
   ADR 0245 H9 found clean with C (ratio `5.015`). With the component held still the coupled meshes are clean (H10: `1.26e-7`,
   ratio `4.98`). So what is left are the events of 1 and 2 and the outer loop under motion.
4. ADR 0245 A2 places the largest `d_200` of TG_2 in the calculated component cell at `(1.668, 0.969)`. With `tg_pose(0.5)`
   (centre `(1.521, 1.521)`, angle 0.5) that is `(-0.136, -0.555)` in the component frame: the second layer of the component
   (`h_c = 0.03`, centres at `-0.585`, `-0.555`), **the first calculated cell inside the component's outer fringe ring**, far
   from the hole (`0.25`) and from the background cells that change kind.

## Arithmetic before this registration (disclosed, by hand)

- **A jump of a forcing at a moment that is not a step boundary is a first-order event.** A cell next to a fringe cell
  relaxes towards its neighbours at the rate `a = 4 nu / h^2` (`444` on the component, `a dt = 1.1, 0.56, 0.28` for 200, 400,
  800 steps at level 2). A jump `J` of the fringe value at the time `t_s` is seen by the discrete step at the next step
  boundary `t_k >= t_s`: the discrete response lags the exact one by `t_k - t_s`, uniform in `[0, dt)`. Its error is of the
  order `J min(1, a (t_k - t_s))` and decays at the rate `a`. So at `t = 0.5` the error carries the last few events, of the
  order `J a dt` for `a dt` small, with a phase that is not a smooth function of `dt`: first order with a scatter, a ratio of
  about 2 (and not 5) between the steps. ADR 0245 measured `2.454` (level 2) and `3.95` (level 1).
- **Size.** The jump of the 5-point least-squares interpolant across a donor face: the two stencils' errors for a quadratic
  field `u = x^2 / 2` at the face midpoint (`s = h / 2` from each centre) differ by of the order `h^2 / 4 |D^2 u|`; Taylor-Green
  has `|D^2 u|` about 1, the background `h = pi / 80 = 0.039`: `J` of the order `4e-4`. The cell next to the fringe takes a
  quarter of a neighbour (one face of four) and `min(1, a dt)` about 0.5: of the order `4e-4 / 4 / 2 = 5e-5` at 200 steps,
  against the measured `2.06e-5`. The size is ample; no closer estimate was made.
- **Count.** A component fringe cell moves at about `|v| = |(0.5, 0.3)| + 0.6 = 1.2` (translation and rotation at radius 0.6),
  so it crosses a background face about every `0.039 / 1.2 = 0.033` time units: 160 outer fringe cells, about 15 switches
  each to `t = 0.5`, of the order 2400 events, one every 13 steps of `0.0025` per cell.
- **A pose-continuous stencil removes the event.** Interpolation that is a continuous function of the fringe position (the
  bilinear interpolant on the four cell centres of the other part around the point; on a uniform rectangular mesh it is
  continuous across the lines between centres and exact for bilinear fields) makes the fringe value a continuous function of
  time with kinks only. A kink in the forcing is a jump of the second derivative of the response: one step with a local error
  `O(dt^2 [u''])`, globally `O(dt^2)` per event, second order. So with that stencil the remainder made by the switches should
  fall to the level of the rest of the step.
- Cost. ADR 0245: three level-2 runs about 6 minutes, six level-1 runs with the single moving mesh about 4 minutes. The
  diagnosis below is 4 level-2 series (12 runs) and 4 level-1 series: about 30 minutes.

## Hypotheses

All with BDF2, three outer iterations and C. The bilinear stencil B (test side, diagnosis only): for every fringe cell, the
four cells of the other part whose centres bound the dual cell containing the fringe point (the background index from
`floor(x / h - 1/2)`; the component index likewise in its reference frame, the point mapped by `pose_inverse_apply`), with the
weights `(1 - s)(1 - t)`, `s (1 - t)`, `(1 - s) t`, `s t`; it is built in the test from `OversetSolver::prepare` and handed to
`advance` in place of the step's stencils, kinds unchanged; a stencil whose four cells are not all calculated stops the run (a
defective set-up). Both meshes are uniform and rectangular, so B is defined there and nowhere else.

| Id | Hypothesis | Measurement | Deciding number |
| --- | --- | --- | --- |
| H1 | **the remainder is made by the donor switches of the fringe stencils** (fact 1) | TG_2 with B in 200, 400, 800 steps: `b = max abs(d_200)` and the ratio `b / max abs(d_400)` | confirmed: `b <= 6.2e-6` (0.3 of `2.057e-5`) and the ratio `>= 4`; ruled out: `b >= 1.44e-5` (0.7) |
| H1a | the same at level 1 | TG_1 with B, 100, 200, 400 steps: `max abs(d_100)` and the ratio | confirmed `<= 1.6e-5` (0.3 of `5.18e-5`) and ratio `>= 4`; ruled out `>= 3.6e-5` |
| H2 | what is not made by the switches is made by the background cells changing kind (fact 2) | TG_2 without a hole (the hole function false everywhere: no background fringe or hole, no cell changes kind; the component's outer fringe as before), with B: `max abs(d_200)` and the ratio; printed: the same without B | decided only if H1 is not confirmed: confirmed if the ratio without a hole with B is `>= 4` and its `max abs(d_200) <= 0.3 b`; ruled out if `max abs(d_200) >= 0.7 b` |
| H3 | the outer loop does not converge under motion (the extrapolated first iteration, the refresh M9) | TG_1 moving, default stencils, 100 steps with three and with six outer iterations: `e_out = max abs(u_3 - u_6)` over the decay | confirmed `>= 5.2e-6` (0.1 of `5.18e-5`); ruled out `<= 5.2e-7` |

Printed, no decision: per run the number of donor switches (fringe cells whose first stencil cell changed over a step) and
the sum of `n_changed`; the cell, kind and part of each maximum; TG_1 with B without a hole.

Predictions (mine, before any run): H1 confirmed (ratio about 5, `b` near `1e-6`); H1a confirmed; H2 printed only; H3 ruled
out (`1e-10`).

## Registered checks

Release, Windows. New test file `crates/rcfd-fv/tests/overset_motion_f442.rs`; the helpers of the set-up are copied from
`bdf2_remainder_f344.rs` (not edited). Tests that take more than about two minutes are `#[ignore]` with the run time in the
reason and are run once.

| Id | Check | Limit |
| --- | --- | --- |
| S0 (set-up) | TG_2 with C, default stencils, 200, 400, 800 steps: `max abs(d_200)` and the ratio (ADR 0245 A2: `2.057e-5`, `2.454`) | in `[2.0e-5, 2.1e-5]`; in `[2.40, 2.50]` |
| S1 | continuity of the calculated cells, every step of every run of this ADR | `<= 1e-9` |
| S2 (set-up of B) | B reproduces a linear field: at the initial pose of TG_2, `u = 1 + 2x - 3y` in every cell, the B interpolant at every fringe cell against `u` at the fringe centre | `<= 1e-12` |
| F0 (defaults, fingerprint) | a printed fingerprint (the bits of `u`, `p`, `phi` folded by FNV-1a) of TG_1, default settings, implicit Euler, one outer iteration, 20 steps of `0.005`; of the same with BDF2, three outer iterations and C; and of the connectivity (kinds, stencil cells, weight bits) of TG_2 at `tg_pose(0.3)` with the linear and with the quadratic stencil: run on the tree before any library change and on the final tree | equal, each of the four |
| A2 (acceptance) | TG_2 with the opt-in of amendment 1 (if one is built), 200, 400, 800 steps: D2 | `>= 4` |
| A2b | the same runs: `max abs(d_200)` | `<= 2.057e-5` |
| A2c | the same runs: `e(TG_2)` in 800 steps over the value with C and default stencils, `3.1859e-4` (ADR 0245 A2c) | `<= 1.05` |
| B1 (defaults) | the existing tests of the crates touched (named in amendment 1), unedited, release; `cargo test -p rcfd-api --release --test baseline_hashes` | pass |

Amendment 1 adds the checks of the opt-in's own parts, its negative controls and its mutations, before its code. If the
diagnosis confirms no hypothesis, no opt-in is built and the addendum names why with the numbers (way ii).

### Mutations (scratch copy `w49i-work/f442-scratch`, not committed)

Each applied alone to a `git archive` copy of the final tree by an exact replacement that must occur once; the named test is
run once and must fail; the file is restored and compared byte for byte.

| Id | Mutation | Must fail |
| --- | --- | --- |
| MS1 | S2: the weight `s t` of B replaced by `s` | S2 |
| MS2 | F0: the fingerprint folds `u` only once per component (the `p` bits left out) | F0 (a different fingerprint) |
| MO* | the mutations of the opt-in: registered in amendment 1 | the check amendment 1 names |

`scripts/limit_audit.py` is run on the new test file and every hit is read.

## Decision rule

- **done**: S0, S1, S2 held; an event is named and confirmed by its registered deciding number (H1, or H2 by its clause);
  and either (i) the opt-in is built, F0 and B1 hold, A2, A2b, A2c are met, or (ii) no opt-in is built and the ADR names why
  with a number while every registered check that was run is met; every registered mutation was caught by its registered
  check.
- **partial**: an event is named and confirmed, but a registered check is missed (A2 among them), or a mutation was caught only
  after an amendment written after a run, or B1 or F0 could not be shown.
- **todo**: no event is confirmed.

## Open choices taken

Each taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation); none is a decision the maintainer
made in person; none changes a default.

- **O1. Diagnosis by a test-side stencil.** (a) build the pose-continuous stencil in the test, through the public `prepare` /
  `advance` pair and the public `OversetStep::conn`, so the library is unchanged while diagnosing; (b) instrument the library in
  a scratch copy. Taken: (a); it is a removal test of the event, the strongest kind, and it needs no library code.
- **O2. Where an opt-in goes.** If H1 is confirmed, the corrective is a donor interpolation that is continuous in the pose. B
  is defined only on uniform rectangular meshes. (a) a new variant of `DonorInterpolation` in `rcfd-mesh` defined on any mesh
  (its design registered in amendment 1); (b) B itself as a library option restricted to rectangular meshes. Taken: (a), with
  (b) as the fallback only if (a) cannot be made continuous; amendment 1 says which.

## Limits of this registration

- L1. One motion (`tg_pose`), two levels, Taylor-Green at `nu = 0.1`, uniform rectangular meshes, central convection, fixed
  velocities on the outer sides. Nothing is concluded for walls on the component, pressure boundaries or other motions.
- L2. The kind changes of the background (fact 2) are discrete by nature; if they carry a part of the remainder, no opt-in of
  this item removes that part (H2 measures its size).
- L3. The first step of a BDF2 run stays an implicit Euler step; the initial connectivity is the solver's own.

## Open questions

Listed with options and a recommendation in the addendum, after the results.
