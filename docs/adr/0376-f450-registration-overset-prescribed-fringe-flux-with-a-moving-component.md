# ADR 0376: F-450 registration: the prescribed fringe flux of the overset step with a moving, turning component

Status: **REGISTERED 2026-11-12**, written before any code of F-450 and before any run of its checks. Not yet accepted by the
maintainer. Before this text nothing was compiled or run: the code, the documents and the existing run logs listed under
"What was read" were read, and the arithmetic under "Arithmetic before this registration" was done by hand. Results go in
the addendum at the end; nothing above the addendum is changed after a run.

Parents: [ADR 0347](0347-f445-registration-overset-prescribed-fringe-flux.md) (F-445: `OversetSolver::set_fringe_flux`,
`FringeFlux::PrescribedOwnGradient` (variant G, D1 to D5), the variant P that blew up; question 4, whose recommendation is
this item, taken on the maintainer's standing instruction of 2026-11-11),
[ADR 0219](0219-f134-registration-overset-coupling-of-a-moving-component-mesh.md) (F-134: the model M1 to M11, the set-up
TG_L with the moving, turning component, the corrected time step of its amendment 2),
[ADR 0251](0251-f336-registration-overset-quadratic-donor-interpolation.md) (F-336: the quadratic donor interpolation),
[ADR 0359](0359-f442-registration-overset-bdf2-motion-remainder.md) (F-442: donor switches and kind changes next to the
hole), [ADR 0367](0367-f368-registration-overset-skewness-and-non-orthogonal-corrections.md) (F-368: the geometry
corrections, refused together with a prescribed fringe flux, so not used here). Roadmap item F-450 (depends on F-445,
F-336).

Number: the lead gave 0376. Branch `f450-overset-moving-fringe-flux` from main 56a14aa5, worktree `w49i-work/f450`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run; a missed limit becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded; a defective set-up gets a corrected test by an
amendment committed before the corrected run; each registered run is made once; defaults bit-identical; `baseline_hashes.rs`,
the hash tables and the goldens are not touched; existing tests are not edited; no new crate, no unsafe code; unsigned
commits without trailers; no push.

Clean-room: everything below follows from the finite-volume definitions of the solver (ADR 0219 M1 to M11, ADR 0347 D1 to
D5) and the closed-form Taylor-Green solution. No paper, book, code or vendor document was read for this work.

## What F-450 asks

Acceptance (quoted, binding): "With OversetSolver::set_fringe_flux(PrescribedOwnGradient) and the quadratic donor
interpolation of F-336, the Taylor-Green set-up of ADR 0219 with the moving, turning component (levels 1 and 2, the
corrected time step of its amendment 2) is run against the default coupling with the same stencils: the largest velocity
error over the calculated and fringe cells is at most 1.2 times that of the default coupling at both levels, continuity and
the outflow balance hold to 1e-9 in every step, and no step is refused for a calculated cell between fringe cells on
opposite sides; or the ADR names with a number what the moving fringe adds (the lag of the prescribed flux on the faces
that change kind is the first candidate)."

**Reading (registered here).** Four clauses, each with its check: the error ratio at levels 1 and 2 (A1), continuity in
every step (A2), the outflow balance in every step (A3), no refused step (A4). "The same stencils" is the quadratic donor
interpolation on both runs of a pair. "The corrected time step of its amendment 2" is `dt = 0.0025` (200 steps) at level
1 and `dt = 0.000625` (800 steps) at level 2, `t = 0.5` (ADR 0219 T1c). The outflow balance of this closed domain (the
exact velocity is fixed on the four sides, ADR 0219 TG_L) has two parts, as C2 of ADR 0347: the net flux out of the
calculated background cells into the background fringe (`OversetInfo::hole_flux`, the flux into the region enclosed by
the moving hole ring) and the net flux through the domain boundary, each over the largest face area. The last sentence of
the acceptance is an alternative for a miss of A1 (see the decision rule).

## What was read (no run, nothing compiled)

- ADR 0347 in full; ADR 0219 (the model, the set-ups, amendment 2 and the results table of addendum 1); the F-336, F-442,
  F-445 and F-368 entries of `docs/roadmap.json`.
- `crates/rcfd-fv/src/overset.rs`: `advance` (the refresh M9, the kept faces, the `ddt_correction` defect), `piso` (the
  hooks of ADR 0347), `FringeRing::new` and `own_gradient` (D5 and its refusal), `prescribed_pressure` (D1 to D3),
  `set_fringe_flux`.
- `crates/rcfd-fv/tests/overset_f336.rs` (the TG_L helpers with the quadratic stencils, `errors`, the classes) and
  `crates/rcfd-fv/tests/overset_f445.rs` (the conservation measures, `state_hash`, B1).
- **The logs of ADR 0251** in `E:/projects/cfd/w49i-work/f336-scratch` (existing output, nothing re-run): `f336-a1.log`.
  Quoted from it, TG_L with the quadratic stencils and the default coupling, `dt` proportional to `h^2`: level 1, 15.4 s,
  largest error by class CF `5.4601e-4`, CC `6.0322e-4`, BF `5.6871e-4`, BC `7.6831e-4`, the largest in cell 20 (BC) at
  `(1.6101, 0.0393)`, continuity `2.80e-14`; level 2, 218 s, CF `1.2451e-4`, CC `1.3359e-4`, BF `1.2495e-4`, BC
  `1.9299e-4`, the largest in cell 3279 (BC) at `(3.1220, 1.5904)`, continuity `5.76e-14`; no fallback, no unconverged
  solve.

## Arithmetic before this registration (disclosed, by hand; no script)

1. **The moving step needs no new code under G (expected).** Under G the flux of every face between a calculated and a
   fringe cell is set again in every corrector from the velocity that corrector starts from (D1), so no flux of an older
   step or kind survives on such a face: a face that changes kind in a step (M9) has its old flux only as the start of the
   predictor's convecting flux, as under the default coupling. The regions, the ring faces and the matrices of D5 are
   built from the connectivity of the new pose in every step (`FringeRing::new` in `piso`). The flux `phi` of the solver
   is the absolute flux through the moving faces; the rigid motion keeps every cell volume (the swept volumes of a cell
   sum to zero, U2 of ADR 0219: `5e-16`), so continuity is `div phi = 0` in the moving cells too and the face
   interpolation of the absolute velocity is the right prescription on the component ring. **No library change is
   planned.** If a run shows that one is needed, it is registered by an amendment before its code and runs.
2. **The acceptance measure is set by a background cell at the domain boundary.** In the default runs with the quadratic
   stencils (log above) the largest error is in a calculated background cell next to the domain boundary (`y = h / 2` at
   level 1, `x = pi - h / 2` at level 2), with the error of the background alone (TGB_1 `7.68e-4`, ADR 0219); the largest
   error of the overlap classes (CF, CC, BF) is `6.0322e-4` and `1.3359e-4`. So the ratio of A1 stays `1` up to round-off
   while the overlap error of G stays below the boundary error, and reaches `1.2` only when the largest overlap error of G
   is `1.2 x 7.6831e-4 / 6.0322e-4 = 1.53` (level 1) or `1.2 x 1.9299e-4 / 1.3359e-4 = 1.73` (level 2) times that of the
   default coupling. On RE at rest (ADR 0347 HT) the root mean square error of the calculated cells under G with the
   quadratic stencils was `1.145`, `1.056` times the default's at levels 1 and 2. **Estimate: A1 met at both levels with
   a ratio within `1.02`**, unless the motion adds to the overlap error of G more than half of it again. Because A1 alone
   cannot see a growth of the overlap error by less than these factors, the overlap ratio and the split between motion
   and rest (D below) are printed with every pair.
3. **Refusals of D5 (A4).** D5 refuses a calculated cell whose matrix `I - (1 / V) sum sigma_f S_f (x_f - x_C)^T` has a
   determinant below `0.05`; on these Cartesian cells one fringe face gives `1/2` in its normal direction, two faces on
   adjacent sides `1/4`, two faces on opposite sides `0`. Component: the calculated cells are the inner `(20 L - 2)^2`
   block, the fringe the fixed outer ring, so only adjacent sides (determinant `0.25`) occur. Background: a calculated
   cell with fringe cells on its west and east sides needs a hole neighbour of each of them; the hole is the turning
   square `max(|x_ref|, |y_ref|) < 0.25` (`3.2 h` half width at level 1), a convex set, and for each pair of such hole
   neighbours whose centres have a cell centre as their midpoint that midpoint is a hole and makes the calculated cell a
   fringe cell (a contradiction); the other pairs are three cells apart along one direction and one across, which a
   convex square more than six cells wide cannot hold without the cells between. **Estimate: no refusal** at either
   level. Not proven for every pose; A4 measures it.
4. **Conservation (A2, A3).** Under G the net flux of each region is zero by D2 (closed domain: `q_R` from the rows of the
   region) to the solver tolerance; F-445 measured `4.5e-13` (continuity) and `1.9e-14` (`Gamma_B`) on RE. The exact
   velocity on the sides of `[0, pi]^2` has a net flux that cancels by symmetry (`sum 2 sin(x_i) A` out of the x sides
   against the same sum into the y sides, the same nodes): round-off. **Estimate: `1e-13` or less** in every step.
5. **Order of G (S1).** If the largest error of G stays at the domain boundary its order between levels 1 and 2 is that
   of the default runs, `log2(7.6831e-4 / 1.9299e-4) = 1.993`; if the overlap error of G sets the maximum at level 1 only,
   the order is larger. **Estimate `>= 1.9`.**
6. **Cost.** Default runs 15 s (level 1) and 218 s (level 2) (log); G has no second pressure solve for the offset (ADR
   0347: G 3.6 s and 47 s against 5.2 s and 58 s on RE). The four runs of a level (moving and at rest, G and default):
   about 1 minute at level 1, 12 minutes at level 2 on a loaded machine.

## The design

No library change (item 1). The test file `crates/rcfd-fv/tests/overset_f450.rs` (new) copies the TG_L helpers of
`overset_f336.rs` (the meshes, the pose `tg_pose`, the exact solution and conditions, the classes and `errors`) and adds the
fringe mode as a parameter of a run and the conservation measures of `overset_f445.rs`. A run is TG_L with
`set_donor_interpolation(Quadratic)` on the coupling and, for G, `set_fringe_flux(PrescribedOwnGradient)` after the solver
is built; `200 L^2` steps to `t = 0.5`; every step through `OversetSolver::step`; a step that returns an error ends the run
and is recorded with its message. Every number below is taken from the same runs.

## Set-ups

- **TG_L moving** (`L = 1, 2`): ADR 0219 TG_L, the pose `tg_pose(t)` (rotation by `t`, translation `(pi/2 - 0.3 + 0.5 t,
  pi/2 - 0.2 + 0.3 t, 0)`), `nu = 0.1`, the settings PS of ADR 0219 (`PimpleSettings::piso` with central convection, two
  correctors, the controls `1e-15`, `1e-12`, 3000), closed domain, mass mode conservative, `dt = 0.5 / (200 L^2)`.
- **TG_L at rest** (the diagnostic D): the same with the pose held at `tg_pose(0)` over the whole run.
- Runs per level: G moving, default moving, G at rest, default at rest; all with the quadratic stencils.

## Registered checks

Release: `cargo test --release -p rcfd-fv --test overset_f450`. Level 1 (the four runs, about a minute) is a fast test;
level 2 is an ignored heavy record, run once; the order S1 between the two test processes is formed in the addendum from
the printed numbers and kept in a record test without a flow run (as ADR 0347 did).

| Id | Check | Limit | Estimate behind it |
| --- | --- | --- | --- |
| A1 (acceptance) | TG_L moving, `L = 1, 2`: `e_max(G) / e_max(default)`, `e_max` the largest `abs(u_c - u_ex(x_c, 0.5)) / e^{-2 nu 0.5}` over the calculated and fringe cells at `t = 0.5` | `<= 1.2` each | item 2: within `1.02` |
| A2 (acceptance) | G moving, both levels, every step: `OversetInfo::continuity` | `<= 1e-9` | item 4: `1e-13` |
| A3 (acceptance) | G moving, both levels, every step: `abs(hole_flux)` over the largest face area; the net flux through the four sides over the largest face area | `<= 1e-9` each | item 4: round-off |
| A4 (acceptance) | G moving, both levels: every step returns `Ok` (no refusal, in particular none of D5 for a cell between fringe cells on opposite sides) | all 200 / 800 steps | item 3: none |
| C3 | every run: unconverged linear solves; fallbacks of the quadratic stencils (`n_fallback`, every step) | `0`; `0` | ADR 0251: none |
| K (set-up) | G moving, level 1: the number of cells whose kind changes, summed over the steps | `>= 20` | the hole turns and moves by 0.29 over a cell size 0.079 |
| S1 | G moving: the order `log2(e_max(G, TG_1) / e_max(G, TG_2))` | `>= 1.7` | item 5: `>= 1.9` |
| B1 (defaults) | without the opt-in: the FNV hash of `u`, `p`, `phi` and the time (`state_hash` of `overset_f445.rs`) after 40 steps of TG_1 moving with the quadratic stencils and after 40 steps of TG_1 moving with the default (linear) stencils, both with the default coupling, equal to the hashes the unedited library gives (measured once on the registration commit before any other run, constants in the test) | exact | item 1: no library change |
| B2 (defaults) | `cargo test -p rcfd-api --release --test baseline_hashes`; the existing `overset_*.rs` test files of `rcfd-fv` (their tests that are not ignored) and `rcfd-fv --lib`, unedited, release | all pass | no library change |

Printed with every level, no limit: `e_max` and its cell, class and place; the largest and the root mean square error of
each class; `e_ov`, the largest error over the overlap classes CF, CC and BF, and the ratio `e_ov(G) / e_ov(default)`
(my estimate: `1.1` to `1.4` at level 1, `1.05` to `1.3` at level 2); the same of the runs at rest; and the split **D**,
the part the motion adds: `m_X = e_ov(X, moving) / e_ov(X, rest)` for `X` = G and default, and `m_G / m_default`
(my estimate: `0.8` to `1.25`; no trusted number). With them: the number of kind changes, the run time, the largest
continuity and conservation measures of the default runs.

### Mutations (scratch copy, not committed)

Each applied alone to a `git archive` copy of the final tree in `E:/projects/cfd/w49i-work/f450-scratch`, the named test
run once, and it must fail by the named check. `scripts/limit_audit.py` is run on the new test file if it exists and every
hit is read.

| Id | Mutation (in `crates/rcfd-fv/src/overset.rs`) | Must fail |
| --- | --- | --- |
| MU1 | D2: `q_R = 0` (`prescribed_pressure`) | A2 or A3 (fast test, level 1) |
| MU2 | D5: the determinant threshold of `inverse3` raised from `0.05` to `0.3` (every calculated cell with ring faces on two adjacent sides refused) | A4 (fast test, level 1) |
| MU3 | D1 under motion: the prescribed flux of the ring faces is the face interpolation of the velocity minus the mesh flux (`fi[f] - mesh_flux[f]`, the relative instead of the absolute flux) | A1 (fast test, level 1) |
| MU4 | the default mode builds the ring and takes the branch of D1 (`ring` built when the mode is `PressureEquation`) | B1 |

### Classes

A miss of A1 to A4, C3, K or S1 is reported with its number and kept as an ignored `*_known_miss` test (or record) with
the limit intact. A control (B1, B2) that does not hold, or a mutation that does not make its check fail, is a defective
set-up or a fault: corrected by an amendment before any re-run, never counted as a pass. A library change that the runs
make necessary is registered by an amendment committed before its code and runs, and B1, B2 then measure it.

## Predictions (mine, before any run)

A1 to A4, C3, K, S1, B1, B2 met. A1 at `1.00` to `1.02` at both levels because the maximum stays in the boundary cell of
the default runs. `e_ov(G) / e_ov(default)` above `1` (the interpolation error handed on in full, ADR 0347 HT) and below
`1.5`. The split D: I do not know its side.

## Decision rule

- **done**: A1, A2, A3, A4 at both levels, C3, K, S1, B1, B2 met, and every mutation caught by its registered check; or
  the alternative clause: A1 missed while A2, A3, A4, C3, B1, B2 hold and the split D registered here names with its
  number the share of the excess that the motion adds (`m_G / m_default`) and the share present at rest (`e_ov(G) /
  e_ov(default)` at rest).
- **partial**: any other registered check missed (Rule I of the campaign), or a mutation not caught by its registered
  check, or caught only after a post-run amendment; or the cause of a miss of A1 named only by a hypothesis registered
  after the runs.
- **todo**: no registered run could be made.

## Limits of this registration

- One set-up (TG_L, two levels, uniform Cartesian parts, the square hole), implicit Euler, one corrector count; no BDF2,
  outer iterations or `rhie_chow_step_consistent` with G under motion (ADR 0347 X covered them at rest only).
- The acceptance measure is set by the boundary of the domain (item 2); the overlap error is printed, not limited.
- The linear stencils with G under motion are not run (ADR 0347: G needs the quadratic stencils to be competitive).
- Level 3 is not run (about 40 minutes per pair).
- The geometry corrections of ADR 0367 are refused with a prescribed fringe flux and are not part of this item.

## Open questions

Listed with options and a recommendation in the addendum, after the results.
