# ADR 0259: F-353 registration: the moving cylinder TR of ADR 0219 with the quadratic donor interpolation

Status: **REGISTERED 2026-11-12**, written before any code of F-353 and before any run of its checks. Not yet accepted by the
maintainer. Before this text no check was run: the documents and code listed under "What was read" were read, the
arithmetic under "Arithmetic before this registration" was done by hand, and the unchanged tree of main 2e0eefd0 was
compiled once (`cargo test --release -p rcfd-fv --test overset_f134 --test overset_f336 --no-run`, to fill the target
directory of the new worktree; nothing was executed). Results go in the addendum at the end; nothing above the addendum is
changed after a run.

Parents: [ADR 0219](0219-f134-registration-overset-coupling-of-a-moving-component-mesh.md) (F-134: the set-up TR, the
body-fitted run BF, the checks A1a, A1b, A2, X1 to X4), [ADR 0251](0251-f336-registration-overset-quadratic-donor-interpolation.md)
(F-336: `DonorInterpolation::Quadratic`, the fallback D4, the refusal of a non-positive interpolated `rAU` D5, its questions
Q1, Q3 and Q5), [ADR 0246](0246-f334-registration-overset-first-order-term.md) (F-334: the linear stencils give the
smaller error of the calculated component cells on the coarse levels of RL through a cancellation, a factor 2.1 at level 1),
[ADR 0238](0238-f335-registration-overset-outer-iterations.md) and [ADR 0241](0241-f338-registration-overset-bdf2.md) (the
opt-ins of outer iterations and BDF2, not used here). Roadmap item F-353 (depends on F-336).

Number: the lead gave 0259 to F-353. Branch `f353-cylinder-quadratic-donors` from main 2e0eefd0, worktree
`w49i-work/f353`.

House rules (ADR 0219, ADR 0251, `w49i-work/AGENT_RULES.md`):

- limits are fixed here, before the code and the runs, and are not changed after a run; the tests and records of F-134 and
  F-336 are not edited;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact and the number recorded, and the cause is
  diagnosed, not tuned away;
- a defective set-up gets a new corrected test by an amendment written and committed before the corrected run, and the old
  one stays as an ignored record;
- no library code is changed (this is a measuring item); defaults are therefore bit-identical by construction;
- `crates/rcfd-api/tests/baseline_hashes.rs`, the hash tables and the goldens are not touched;
- each registered run is made once; nothing is re-run to pick a result;
- no new third-party crate, no unsafe code, no commit trailers, no push.

Clean-room. Nothing new is derived here: the set-up, the solver, the interpolation and the force are those of ADR 0219 and
ADR 0251, which derive them from first principles. The reference is a run of the solver itself (BF); no published data
are used.

## What F-353 asks

The roadmap entry (acceptance, quoted, binding): "The set-up TR of ADR 0219 run with DonorInterpolation::Quadratic: mass
of the domain and continuity within 1e-9, no fallback or the count reported, drag within 5 percent of the body-fitted run
at every step from t = 2 to t = 8 and the difference to the linear-stencil run reported; the ADR recommends for or against
making the quadratic interpolation the default."

**Reading (registered here).**

1. "The set-up TR of ADR 0219": exactly the set-up of `crates/rcfd-fv/tests/overset_f134.rs` (`tr_overset`, `tr_pose`,
   `tr_new`, `walls_tr`, `settings`, `DT = 0.02`, 400 steps to `t = 8`), copied into the new test file without change,
   with the one addition `Overset::set_donor_interpolation(DonorInterpolation::Quadratic)` on the overset before the
   solver is built ("TR-Q"). The same set-up without the call is "TR-L", the run of ADR 0219. Settings: PS of ADR 0219
   (PISO, central convection, two correctors, implicit Euler, one outer iteration, conservative mass mode). The opt-ins of
   ADR 0238 (outer iterations) and ADR 0241 (BDF2) are off; no combined run is registered (see Limits).
2. "Mass of the domain ... within 1e-9": as ADR 0219 A1b, `abs(Q_out) / (U D L)` after every step, `Q_out` the net flux
   through `xmax` (the only open side), `U D L = 0.1`. "Continuity": as A1a, the library's continuity measure of the
   calculated cells (`OversetInfo::continuity`) after every step. Both over the 400 steps.
3. "No fallback or the count reported": `Connectivity::n_fallback()` of the solver's connectivity at its construction and
   `OversetInfo::n_fallback` of every step; the largest, the sum over the steps and the first step with a fallback are
   printed. The acceptance is met by either zero or a reported count; zero is a condition of the recommendation (below).
4. "Drag within 5 percent of the body-fitted run at every step from t = 2 to t = 8": as ADR 0219 A2, `F_x` from
   `patch_force_moment` on `c_cylinder` (`rho = 1`, centre the pose's translation) after every step, against `F_x` of BF on
   `cylinder` at the same step; `e_Q(n) = abs(F_Q(n) - F_BF(n)) / abs(F_BF(n))` for the 301 steps `n = 100..=400`.
5. "The body-fitted run": ADR 0219 recorded BF only at `t = 2, 4, 6, 8` (and the largest difference), not step by step, so
   BF is re-run here once, in the same test, with the code of `overset_f134.rs` (`bf_new`, `bf_points`,
   `step_ale_with_walls`). The library has not changed BF's path since (ADR 0251 D2 and the gate keep R1 and Q0), so the
   re-run must reproduce the recorded digits: set-up check L0.
6. "The difference to the linear-stencil run": TR-L is re-run in the same test (the step-by-step series of ADR 0219 was
   not recorded). Reported: `d(n) = (F_Q(n) - F_L(n)) / F_L(n)` (largest absolute value, mean, at `t = 2, 4, 6, 8`), `e_L(n)`
   beside `e_Q(n)` at every step (the whole series printed), the number of steps where `e_Q(n) > e_L(n)`, the means of
   `e_Q` and `e_L` and of `C_D` over the window. TR-L is checked against ADR 0219 (L0).
7. "The ADR recommends for or against": by the rule R below, fixed now. The recommendation is a proposal; making
   `Quadratic` the default changes a default, the records R2 of ADR 0238 and the numbers of the existing overset tests,
   and stays the maintainer's decision whatever the numbers say.

## What was read (no run)

- ADR 0251 in full; ADR 0219 in full; ADR 0246 the addendum (results, decision, questions); ADR 0238 and ADR 0241 the
  headers, the records R2 and their questions on the default.
- `crates/rcfd-fv/tests/overset_f134.rs` in full (the set-up TR and BF, the test
  `a_cylinder_moving_through_a_background_mesh_conserves_mass_and_matches_the_body_fitted_drag`),
  `crates/rcfd-fv/tests/overset_f336.rs` in full; of `crates/rcfd-fv/src/overset.rs` the `OversetInfo` fields, the rAU
  check and the mass offset; of `crates/rcfd-mesh/src/overset.rs` the quadratic branch of `connectivity` and
  `Connectivity::fallbacks`.
- The log of the one run of ADR 0219's A test, `w49i-work/f134-a.log`: `C_D` of TR-L `2.00050`, `1.82257`, `1.72278`,
  `1.66752` and of BF `2.00311`, `1.82666`, `1.72919`, `1.67390` at `t = 2, 4, 6, 8`; A2 `6.2485e-3` at step 399; mean
  relative difference `-3.208e-3`; wall time TR 128.6 s, BF 24.2 s; 59.7 pressure iterations per TR step.

## Arithmetic before this registration (disclosed, by hand)

- **Fallbacks.** ADR 0251 P6 found every TR stencil whole (776 stencils at the five poses of K3, no fallback). Along the
  path: the component fringe ring is radial layer 29 of the O-grid, its centres at `r = 1.586` (layer edges `0.5 + 0.01
  (1.08^i - 1) / 0.08`: `1.540` and `1.633`); a donor background cell (size `0.1`) and its point neighbours reach inwards to
  at least `1.586 - 1.5 sqrt(2) 0.1 = 1.37`, outside the background fringe, whose centres lie within one background cell of
  a hole centre (`r < 1 + 0.1 sqrt(2) = 1.14`). The background fringe centres (`1.0 <= r <= 1.14`) lie over component
  layers 21 to 23 (`r = 1.0` at `1.08^i = 5`, `i = 20.9`); their point neighbours are layers 20 to 24, calculated. The
  background under the component stays uniform (`x` in `[-8, 4.5]`, the component from `x = 2` to `-5.5` with radius
  `1.63`). Expected: **no fallback at any step**.
- **rAU.** The quadratic weights of a whole block have `Lambda` near `1.4` and a smallest weight near `-0.07` (ADR 0251
  P7, TR `1.415`); `rAU = V / a_P` changes between neighbouring cells by the volume ratio (`1.08` radially) and the
  diagonal, so the interpolant is within a few percent of the donor's `rAU`. Expected: no refusal.
- **The size of the effect.** The linear stencils leave a truncation of about `h^2 / 8` times the second derivatives; on
  TR's fringes (`h = 0.1` at the component fringe, `0.05` to `0.07` at the background fringe) the velocity varies on the
  length `D = 1`, so the truncation is of order `1e-3 U`, and the quadratic one of order `h^3`, `1e-4` to `1e-3 U`: at
  `h / D = 0.1` the two are not separated by much. ADR 0219 found the drag difference to BF growing with the distance
  travelled (`-0.13 %` at `t = 2`, `-0.38 %` at `t = 8`) as the wake moves from the component onto the background, which
  points at the outer mesh, not at the fringe. Expected: `abs(d(n))` below `1e-3` at every step, `e_Q` close to `e_L`
  (within `1e-3`), A2 met with `e_Q` near `6e-3`; the sign of `e_Q - e_L` I cannot predict (ADR 0246 found that removing the
  truncation can raise the error on coarse meshes).
- **Cost.** Per step about 156 fringe cells (96 + about 60) of 23,230; the quadratic stencils have 9 cells against about
  5, a QR of `9 x 6` per fringe cell and pose; the point table once. ADR 0251 measured 8.5 ms against 4.8 ms for a
  connectivity of 412 fringe cells and 1.2 times the run time of RL level 2. Expected: TR-Q costs 1.0 to 1.3 times TR-L.
  Timings on the shared machine vary by up to a factor of two between processes, so the two runs are stepped alternately
  in one loop (below) and the time of each solver's steps is summed.
- **Run time.** TR 128.6 s and BF 24.2 s in ADR 0219: the heavy test takes about 5 minutes alone, more on the loaded
  machine. Ten steps of TR-Q and TR-L for the fast test: about 10 s.

## The tests

New file `crates/rcfd-fv/tests/overset_f353.rs`, release (`cargo test --release -p rcfd-fv --test overset_f353`). The
set-up helpers are copied from `overset_f134.rs` unchanged (test files share no module); no existing test is edited.

- `tr_quadratic_against_linear_and_body_fitted` (**`#[ignore]`**, heavy record, run once): TR-L and TR-Q are built and
  stepped alternately in one loop, step `n` of TR-L and step `n` of TR-Q in each pass (TR-L first on odd `n`, TR-Q first
  on even `n`), the wall time of each `step` call added to its solver's sum; then BF, 400 steps. All checks below except
  the fast ones.
- `tr_quadratic_first_ten_steps` (gate): TR-Q and TR-L, 10 steps, alternately: the fast checks G1 to G5.

## Registered checks

### A. The acceptance (TR-Q, 400 steps)

| Id | Check | Limit | Estimate |
| --- | --- | --- | --- |
| A1a | every step: the continuity of the calculated cells | `<= 1e-9` | ADR 0219 with M4: `8.1e-17`; ADR 0251 C1 with Q: `8.1e-14` |
| A1b | every step: `abs(Q_out) / (U D L)` | `<= 1e-9` | ADR 0219: `7.5e-15`; the offset `delta` of M7 does not depend on the stencils |
| A2 | every step `n = 100..=400`: `e_Q(n)` | `<= 5e-2` | ADR 0219 with M4: `6.25e-3`; expected near that |
| A3 | fallbacks: at the construction and every step | reported (the acceptance); expected `0` | the arithmetic above |
| A4 | the difference to TR-L: `d(n)`, `e_L(n)`, `e_Q(n)` | reported | expected `abs(d) < 1e-3` |

### X. Set-up and controls

| Id | Check | Limit | Estimate |
| --- | --- | --- | --- |
| L0 (set-up) | TR-L and BF reproduce the run of ADR 0219: `C_D` of TR-L and of BF at `t = 2, 4, 6, 8` against the eight values of `f134-a.log`; A2 of TR-L against `6.2485e-3` | `abs <= 5e-6` each (half a unit of the fifth decimal); `abs <= 5e-8` | the default path is unchanged since (ADR 0251 D2, D3) |
| X1 | TR-Q, every step: every background cell whose centre lies within 0.5 of the cylinder centre is a hole | exact | ADR 0219 X1 |
| X2 | TR-Q, every step: 96 component fringe cells, at least one hole and one background fringe cell | exact | ADR 0219 X2 |
| X3 | TR-Q: the kinds of every cell equal those of TR-L after every step (the kinds do not depend on the interpolation, ADR 0219 M2), and the kind changes sum to at least 500 | exact; `>= 500` | ADR 0219: 6300 |
| X4 | TR-Q and TR-L: no unconverged linear solve (`OversetInfo::unconverged`); BF: the pressure iterations of every step below 3000 | `0`, `0`; `< 3000` | ADR 0219: none; 310 |
| X5 | TR-Q: `conn.interpolation == Quadratic` after every step | exact | ADR 0251 D1 |
| NQ (negative control) | the opt-in reaches the solver: the largest `abs(d(n))` over the 400 steps | `>= 1e-9` | the weights of the two stencils differ by `O(0.1)` on fields that are not linear; the fields differ by the difference of the truncations, `1e-4` to `1e-3` |

Printed, no limit: the smallest stencil and the number of stencils with fewer than nine cells of TR-Q at every 50th step;
`C_D` of the three runs at `t = 2, 4, 6, 8` and their means over the window; the range of `delta / (U^2 / 2)` of both TR
runs; the pressure iterations per step; the wall times (TR-L, TR-Q summed over their `step` calls; BF); the series
`n, t, C_D(BF), C_D(L), C_D(Q), e_L, e_Q, d` of all 301 steps of the window.

### G. The fast test (gate), 10 steps of TR-Q and TR-L

| Id | Check | Limit | Estimate |
| --- | --- | --- | --- |
| G1 | TR-Q, every step: continuity | `<= 1e-9` | A1a |
| G2 | TR-Q, every step: `abs(Q_out) / (U D L)` | `<= 1e-9` | A1b |
| G3 | TR-Q: fallbacks at the construction and every step | `0` | the arithmetic above; ADR 0251 P6 on TR |
| G4 | TR-Q: `conn.interpolation == Quadratic`, every stencil has 9 cells, after every step | exact | ADR 0251 P6 (whole blocks) |
| G5 (negative control) | the largest difference of `u` between TR-Q and TR-L after 10 steps, over `U` | `>= 1e-12` | NQ; after 10 steps (`t = 0.2`, speed `0.1`) smaller than later, still far above round-off |

### Mutations (scratch copy, not committed)

Each is applied alone to a `git archive` copy of the final tree in `E:/projects/cfd/w49i-work/f353-scratch` (own target
directory), the fast test is run once and must fail; the file is restored and compared byte for byte.

| Id | Mutation | Must fail |
| --- | --- | --- |
| MT1 | the test: the call `set_donor_interpolation(DonorInterpolation::Quadratic)` removed from the TR-Q helper | G4, G5 |
| MT2 | `rcfd_fv::overset`, the conservative pressure: the offset not applied (`*pi += delta * qi` becomes `*pi += 0.0 * delta * qi`) | G2 |
| MT3 | `rcfd_mesh::overset::connectivity`, the quadratic branch: the weights asked of the donor alone (`&pts[..1]`), so that every stencil falls back | G3 |

`scripts/limit_audit.py` is run on the new test file and every hit is read.

## Classes

A miss of A1a, A1b, A2, X1 to X5 or G1 to G4 is reported with its number and kept as an ignored `*_known_miss` test with
the limit intact; its cause is diagnosed. A refusal of a step (for example D5 of ADR 0251, a non-positive `rAU`) stops TR-Q:
the refusal and its step are recorded, the checks of TR-Q are missed, and the cause is diagnosed, without a library
change. L0, NQ or G5 that does not hold, or a mutation that does not make its check fail, is a defective set-up,
corrected by an amendment before any re-run and never counted as a pass.

## Decision rule (the item)

- **done**: A1a, A1b, A2 met; A3 and A4 reported; L0 held; X1 to X5 met; NQ held; G1 to G5 met; the mutations failed the
  fast test; and the recommendation is stated by the rule R.
- **partial**: TR-Q ran to `t = 8` but a check of A or X missed with its cause diagnosed, or L0, NQ or a mutation did not
  hold and was not corrected.
- **todo**: TR-Q could not be run to `t = 8` (a refusal or a failure of the step), or no registered run was made.

## The rule of the recommendation (fixed before any run)

The ADR recommends **for** making `DonorInterpolation::Quadratic` the default only if all of these hold, and **against**
otherwise:

| Id | Condition | Why |
| --- | --- | --- |
| R1 | A1a, A1b and A2 are met with TR-Q | a default must meet the acceptance of F-134 |
| R2 | no fallback in TR-Q, at the construction or at any step | a default that silently drops an order in some cells of the reference case is not the stated method; the fallback cells include those that decide the wake whenever there are any, and the count is too coarse a measure to tell which |
| R3 | `e_Q(n) <= e_L(n) + 1e-4` at every step `n = 100..=400` | "no worse than the linear run in drag error at every step"; the margin `1e-4` (one percent of a percent of the drag, 1/60 of the largest `e_L` of ADR 0219) is far above round-off (the solves stop at `1e-12` relative) and far below the 5 percent of the acceptance, so a step where the two runs differ by less than that is a tie |
| R4 | the mean of `e_Q` over the window is at most the mean of `e_L` | R3 alone could be met with `e_Q` above `e_L` by up to the margin at every step |
| R5 | the summed `step` time of TR-Q is at most `1.5` times that of TR-L (stepped alternately in one process) | the expected cost is 1.0 to 1.3; 1.5 leaves room for the noise of the shared machine that alternation does not remove |
| R6 | no unconverged linear solve in TR-Q (X4) | a default must not lean on the linear solvers harder than today |

The evidence of the earlier items (ADR 0246, ADR 0251: on RL the calculated component cells have 2.07 and 1.33 times the
error with Q at levels 1 and 2; Q is better on TG_L at every level and in order) is quoted beside the recommendation and
is not part of the rule: the rule decides on the case the acceptance names. Whatever the rule gives, the change of the
default is not made here and needs the maintainer's decision (it changes the records R2 of ADR 0238 and the numbers of
the existing overset tests).

## Predictions (mine, before any run)

A1a, A1b, A2 met (A2 near `6e-3`); no fallback; L0 held to every printed digit; X1 to X5, NQ, G1 to G5 met; `abs(d)` below
`1e-3`; time ratio 1.0 to 1.3. R3 and R4: I do not know; I lean to `e_Q` slightly above `e_L` (the cancellation of ADR
0246 on a coarse fringe), which would make the recommendation "against".

## Limits of this registration

- One case, one mesh pair, one time step, `Re = 40` (steady wake in the body frame). No refinement study of TR, so the
  part of `e` that the interpolation causes is not separated from the part of the outer mesh beyond what `d(n)` shows.
- No combined run with outer iterations (ADR 0238) or BDF2 (ADR 0241): not registered, not run.
- The cost is measured on a shared machine and is decisive only through R5's factor.
- The recommendation covers the overset path of `rcfd_fv::overset` in 2-D; 3-D flow with Q is not run.

## Open questions

Listed with options and a recommendation in the addendum, after the results.
