# ADR 0340: F-349 registration: the response of the overset coupling to the truncation of the background-fringe interpolation

Status: **REGISTERED 2026-11-12**, written before any code of F-349 and before any run of its checks. Not yet accepted by the
maintainer. Before this text no check was run and nothing was compiled: the code, the documents and the existing run logs
listed under "What was read" were read, and the arithmetic under "Arithmetic before this registration" was done by hand.
Results go in the addendum at the end; nothing above the addendum is changed after a run.

Parents: [ADR 0246](0246-f334-registration-overset-first-order-term.md) (F-334: the set-up RL, the decomposition of the
error into the single-mesh part and the response `R`, the finding that its background-fringe part `R_b` is of first order
and still growing at `t = 0.5`; question Q4, whose recommendation is this item),
[ADR 0251](0251-f336-registration-overset-quadratic-donor-interpolation.md) (F-336: the library's quadratic donor
interpolation, an opt-in), [ADR 0232](0232-f314-registration-overset-observed-order-miss.md) (F-314),
[ADR 0219](0219-f134-registration-overset-coupling-of-a-moving-component-mesh.md) (F-134: the model M1 to M11),
[ADR 0264](0264-f359-registration-linear-shear-flow-on-a-warped-overset-mesh.md) (F-359; its question 2 is F-369, a
related item on a moving component, not treated here). Roadmap item F-349 (depends on F-334).

Number: the lead gave 0340 to F-349 (0270 to 0339 are the structural-solver plans). Branch
`f349-background-fringe-response` from main a5db938b, worktree `w49i-work/f349`.

House rules (ADR 0130, ADR 0213, ADR 0219, ADR 0246):

- **no library code is edited**: this is a diagnosis, tests and this ADR only. `crates/rcfd-fv/src/overset.rs` and
  `pimple.rs` are held by another item and are not touched. A library change that the diagnosis points at is described
  with its derivation and left to a follow-up item;
- limits and deciding numbers are fixed here, before the code and the runs, and are not changed after a run; existing tests
  and ignored records are not edited;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact and the number recorded; a defective set-up
  gets a corrected test by an amendment written and committed before the corrected run; a further measurement that the
  results make necessary is registered by an amendment committed before its run;
- each registered run is made once (a deterministic run that two tests share is the same run);
- no new third-party crate, no unsafe code, no commit trailers, no push.

Clean-room: everything below follows from the finite-volume definitions of the solver, Taylor's theorem and the
potential theory of the Laplace equation. No paper, book, code or vendor document was read for this work.

## What F-349 asks

Acceptance (quoted, binding): "The response R_b of ADR 0246 (the difference of the RL runs with the library stencils and
with a third-order interpolation on the background fringe: 4.87e-5, 2.19e-5, 1.41e-5 at levels 1 to 3, orders 1.153 and
1.093) is traced to a named term of the coupled step by a switch that brings its order to 1.7 or more on levels 1 to 4,
or is shown to reach order 1.7 on finer levels or at a later time, or the ADR names why not with a number."

**Reading (registered here).** Three clauses, each with its check:

1. *A switch* (check A1). A switch is a setting of the library that exists today (the time step, the transient
   Rhie-Chow term, the donor interpolation) or a change of the set-up (the spacings of the two parts). The order is
   measured on levels 1 to 3 first; a switch that reaches 1.7 on both pairs is then run at level 4 by an amendment.
2. *Finer levels* (check A2: levels 4 and 5 at `t = 0.5`) or *a later time* (check A3: levels 1 to 3 at `t = 3`).
3. *Why not, with a number*: a hypothesis below confirmed by its registered deciding number, with the scaling of the term.

## What was read (no run)

- ADR 0246, ADR 0251, ADR 0232 (set-up RL, amendments 3 and 4, addendum), ADR 0264 (addendum, the separating runs), the
  summaries of ADR 0238 and ADR 0241. `crates/rcfd-fv/src/overset.rs` in full, the public interface of
  `crates/rcfd-mesh/src/overset.rs`, `crates/rcfd-fv/tests/overset_f334.rs` in full, the head of `overset_f336.rs`.
- **The logs of the runs of ADR 0246** in `E:/projects/cfd/w49i-work/f334-scratch` (existing output, nothing re-run): the
  histories of `R_b`, the pressure jumps at the fringe faces and the flux defects of the runs with the library stencils
  and with the nine-cell stencils. The numbers used below are quoted.

What the code read shows, beyond the list of ADR 0246:

1. The switches that exist without a library edit: `Overset::set_donor_interpolation(Quadratic)` (both fringes at once;
   one fringe alone by substituting the stencils of a prepared step, `OversetStep::conn` being public), the step size,
   `PimpleSettings::ddt_correction`, the number of correctors, `set_outer_iterations`, `set_time_scheme(Bdf2)`, the hole
   function and the two meshes. There is **no** setting for the width of the fringe layer (one layer of cells), none that
   imposes the flux of a face between a calculated and a fringe cell from the interpolated velocity, and none that treats
   the four interpolated quantities (`u`, `p`, `HbyA`, `rAU`) apart.
2. The flux of a face between a calculated cell `C` and a fringe cell `F` is `phi = (HbyA)_f . S - rAU_f (p_F - p_C)
   abs(S) / d + (rAU_f / dt) (phi_old - (u_old)_f . S)`, with `HbyA_F`, `rAU_F` and `p_F` interpolated. With `HbyA = u +
   rAU grad p` in the donors this is `(u)_f . S` plus a Rhie-Chow term: the flux is tied to the interpolated velocity of
   the fringe cell only through the defect `D = phi - (u)_f . S`, which obeys `D_new = theta D + R` (`theta = rAU_f / dt`).
3. In the closed domain of RL the offset `delta` is fixed by the reference cell; it makes the net flux through the
   background fringe zero. It is one number per step.

## Arithmetic before this registration (disclosed, by hand)

- **Regime.** `a = 4 nu dt / H^2 = 0.162 L` on the background (ADR 0246), so `theta = 1 / (1 + a)` is `0.86`, `0.76`,
  `0.67`, `0.61`, `0.55` at levels 1 to 5: on these levels the momentum coefficient is mostly `1 / dt`, not `4 nu / h^2`.
  Any quantity that carries a factor `(1 + a)` or `a / (1 + a)` has an apparent order between the levels that differs
  from its asymptotic one by up to one (`h^2 a` is `4 nu dt`, of first order).
- **The kink relation** (ADR 0246, arithmetic): in a steady state a difference `m` between the normal velocity that the
  flux of a fringe face carries and the interpolated one is held by a pressure jump `J = (8 nu / 3 h) m` across that face.
  A mismatch of second order therefore gives a pressure of first order next to the fringe. The logs show such jumps: at
  the background fringe faces `6.77e-4`, `2.37e-4`, `1.34e-4` (root mean square, levels 1 to 3, library stencils; orders
  `1.51`, `1.41`) and `2.0e-5` at level 3 with the nine-cell stencils on both fringes.
- **A linear analysis of the continuous problem** (velocity and pressure given on both fringes, the truncation as a
  perturbation of the data, the interior adapting smoothly) gives a response of the size of the truncation, of second
  order, by every path I tried: the direct response of the background, the pressure jump of the kink relation interpolated
  to the other part, the offset `delta`, the unequal `rAU` of the two parts in the flux of a fringe face (the first-order
  parts of `(HbyA)_f` and of the correction cancel). Like the author of ADR 0246 I could not derive a first-order term.
  The item is decided by measurements.
- **The histories of ADR 0246** (logs). `R_b` at `t = 0.0025, 0.025, 0.125, 0.25, 0.5, 0.75, 1`: level 1 `1.57e-5`,
  `2.79e-5`, `3.16e-5`, `2.76e-5`, `4.87e-5`, `6.97e-5`, `8.53e-5`; level 2 `1.63e-6`, `4.63e-6`, `5.77e-6`, `1.149e-5`,
  `2.19e-5`, `2.91e-5`, `3.43e-5`; level 3 (to `0.5`) `8.4e-7`, `2.46e-6`, `3.3e-6`, `7.5e-6`, `1.41e-5`. Two parts are
  visible: one that is there after `t = 0.025` (order `1.56` between levels 2 and 3 at `t = 0.025`), and one that grows
  from about `t = 0.125`. A saturating exponential through the last three points gives the time constants `0.84` (level 1)
  and `0.74` (level 2) and the settled values `1.30e-4` and `4.71e-5`: **order 1.47**. The time constant is close to that
  of the slowest viscous mode of the component square (`a_c^2 / (2 pi^2 nu) = 0.70` with `a_c = 15 H`) and does not
  shrink with `h`. A model in which the response is of second order but settles in a time proportional to `h` (which
  would give first order at a fixed early time) fits the points at `t = 0.5` and `1` alone (time constants `1.74`, `0.86`)
  but not the three-point fits. Both readings are tested by A3.
- **The mean of `R_b`** over the calculated component cells is `(4.7e-6, -1.27e-5)`, `(5.8e-6, -8.8e-6)`, `(4.8e-6,
  -7.1e-6)`: its size falls with the orders `0.36` and `0.50`. `R_b` is flat over the rings of the component.
- **The phase of the background fringe** in its donor cells (ADR 0246: the ring lies `4.0 H`, `3.25 H`, `3.33 H`, `3.125
  H` from the component centre at levels 1 to 4; normal fractions `0.667`, `0.333`, `0.667`, `0.333`). Level 5: the ring
  lies at `3.2 H`, `(3.2 + 7.5) / 0.15 = 71.33`, fraction `0.333`: locked. Level 6 (`3.25 H`, `86.0`) and level 8
  (`3.1875 H`, `114.0`) put the fringe centres on faces of component cells: not locked, not used. Along the ring the
  background centres are `4/3` of a component cell apart, so the tangential fraction takes three values, `0`, `1/3`, `2/3`
  (the first on a face of a component cell): **the truncation of the background fringe is modulated along the ring with a
  period of three cells**, at every level.
- If `R_b` were proportional to `r^n h^2` with the ring at the distance `r`, the two observed orders `1.153` and `1.093`
  would give `n = 0.17` and a true order `1.10`: the position of the ring does not explain the order.
- **A set-up without the modulation and with equal spacings** (RE below): component of half width `7.5 H` with `15 L`
  cells per side (`h_c = h_b = H / L`), its centre at `(16.5 + 1 / (4 L)) H`. A background centre `(i + 1/2) H / L` lies
  at `i + 1/2 - 9 L - 1/4` component cells from the component's corner: fraction `0.25` in both directions for every `i`
  and `L`. A component centre lies at `9 L + 1/4 + k + 1/2` background cells: fraction `0.75`. Both fringes are locked
  exactly, with the same fraction along the whole ring, and `rAU` is the same in both parts.
- **Time step.** ADR 0246 Ht: halving the step changes the velocity of the run with the library stencils by `tau =
  4.99e-6`, `2.25e-6` (levels 1, 2), a tenth of `R_b`. A part of `R_b` proportional to `dt` would need a change of `0.5
  R_b`; the triangle inequality bounds the change of `R_b` by the sum of the changes of the two runs, about `0.2 R_b` if
  the run with the quadratic background fringe changes as much. A time-step term is unlikely but not excluded; D measures
  it.

## Notation

RL, `RL(L_b, L_c)`, the classes CC, CF, BC, BF, `H`, the decay factor and the settings are those of ADR 0246. `base` is a
run with the library's default (linear) stencils. **`Qb`** is the same run in which the stencil of every background
fringe cell is replaced, in every prepared step, by the stencil that the same composite gives with
`DonorInterpolation::Quadratic` (a second `Overset` of the same meshes and hole with the opt-in set; the component is at
rest, so the stencils are computed once and substituted in every step). The component fringe keeps the linear stencils.
`d = u_base - u_Qb`, cell by cell on the same mesh. `R_b(L, t)` is the root mean square of `abs(d)` over CC, weighted
with the cell volumes, over the decay factor `exp(-2 nu t)`. The order between two levels is `ln(R_coarse / R_fine) /
ln(L_fine / L_coarse)`.

## Hypotheses, measurements and the numbers that decide

| Id | Hypothesis | Measurement | Deciding number |
| --- | --- | --- | --- |
| HT | `R_b` is of second order once settled; at `t = 0.5` it is still arriving, and the low order is that of the transient | **L**: RL at levels 1, 2, 3 run on to `t = 3` (`1200 L` steps), base and Qb; `R_b` at `t = 0.5, 0.75, 1, 1.5, 2, 2.5, 3`; `G_L = R_b(L, 3) / R_b(L, 0.5)` | confirmed: A3 met (orders at `t = 3` at least 1.7 on both pairs); ruled out: both orders at `t = 3` below 1.5; otherwise undecided. Printed with it: `G_L` (a settling time that shrinks with `h` shows as `G_1 > G_2 > G_3`) |
| HF | the levels 1 to 3 are not yet asymptotic (the factors `1 + a`) | **F**: RL at levels 4 and 5, `t = 0.5`, base and Qb | confirmed: A2 met (orders 3 to 4 and 4 to 5 at least 1.7); ruled out: both below 1.5; otherwise undecided |
| Hdt | a part of `R_b` is proportional to the step (a time or splitting error of the coupled step, `dt` being proportional to `h` in RL) | **D**: RL at levels 1, 2, 3 with `400 L` steps, base and Qb: `rho_dt(L) = R_b(L; dt / 2) / R_b(L; dt)` at `t = 0.5` | ruled out: `abs(rho_dt - 1) <= 0.10` at the three levels; confirmed: the orders of `R_b(L; dt / 2)` are at least 1.7 on both pairs, or `rho_dt <= 0.65` at the three levels; otherwise mixed |
| Hmod | the term needs the unequal spacings of the two parts: the modulation of the truncation along the ring with a period of three cells, or the unequal `rAU` | **E**: RE at levels 1, 2, 3, `200 L` steps, base and Qb: the orders of `R_b` at `t = 0.5` | confirmed: both orders at least 1.7; ruled out: both below 1.5; otherwise mixed |
| Hsp | the factor `1 / h` belongs to the spacing of one part: with `R_b` proportional to `L_b^-x L_c^-y`, the truncation is proportional to `h_c^2` (the donors are component cells), so `y = 2`, `x = -1` says that the response to a given truncation grows as the background is refined, and `y = 1`, `x = 0` that the donor spacing carries it | **S**: `RL(1,2)`, `RL(2,1)`, `RL(2,4)`, `RL(4,2)` (`200 max(L_b, L_c)` steps), base and Qb, with `RL(1,1)` and `RL(2,2)`: `y_1 = log2(R_b(1,1) / R_b(1,2))`, `y_2 = log2(R_b(2,2) / R_b(2,4))`, `x_1 = log2(R_b(1,1) / R_b(2,1))`, `x_2 = log2(R_b(2,2) / R_b(4,2))` | background: `x_1, x_2 <= -0.5` and `y_1, y_2 >= 1.5`; component: `abs(x_1), abs(x_2) <= 0.5` and `y_1, y_2 <= 1.5`; otherwise mixed. (`y_1`, `y_2` and `x_2` are measured with the ring at nearly the same place; `x_1` moves it from `4.0 H` to `3.25 H`.) |

Printed for every pair of runs (base, Qb) at `t = 0.025`, `0.125`, `0.5` and at the end, no limit; these are what an
amendment would build on:

- the mean vector of `d` over CC, and the share of `d` that is a multiple of the exact velocity;
- the root mean square of `d` over CF, BF, and over the calculated background cells in the bands `rho < 5 H`, `5 H <= rho
  < 7.5 H`, `7.5 H <= rho < 12 H`, `rho >= 12 H` of the distance `rho = max(abs(x - x_c), abs(y - y_c))` from the
  component centre; the same for the difference of the pressures (the mean over BC removed);
- on the faces between a calculated and a fringe background cell (`Gamma_B`, normal `n` from the calculated cell to the
  fringe cell): the root mean square of `t_f = d_F . n` (the change of the normal velocity of the fringe cell), of `c_f =
  d_C . n` (that of the calculated cell), of `m_f` (the change of the flux over the face area), the least-squares
  coefficients `a_c = sum c t / sum t^2` and `a_m = sum m t / sum t^2` (how far the calculated cell and the flux follow the
  change of the fringe velocity), and the mean and the root mean square of the change of the pressure jump `(p_F - p_C)`;
  the same on the faces between a calculated and a fringe component cell;
- `delta` of both runs; the run time; the unconverged solves.

## Registered checks

New test file `crates/rcfd-fv/tests/overset_f349.rs` (the helpers of `overset_f334.rs` copied, as the overset test files
do); release, `cargo test --release -p rcfd-fv --test overset_f349`. The tests that take more than about two minutes are
`#[ignore]` heavy records, run once each. The orders between levels that are run in different test processes are formed
in the addendum from the printed numbers (as ADR 0246 did); a pair that misses gets an ignored record test with the
numbers.

| Id | Check | Limit | Estimate behind it |
| --- | --- | --- | --- |
| A1 (clause 1) | the orders of `R_b` at `t = 0.5` between levels 1 and 2, 2 and 3 under a switch: (D) half the time step; (E) the set-up RE. The transient Rhie-Chow term off is not run again: ADR 0246 measured `1.763`, `1.398` | `>= 1.7` each, for one switch | none trusted; the hand analysis expects second order everywhere and is contradicted by ADR 0246 |
| A2 (clause 2, finer levels) | the orders of `R_b` at `t = 0.5` between levels 3 and 4, 4 and 5 | `>= 1.7` each | the total response `R` of ADR 0246 has `1.800` between levels 3 and 4 |
| A3 (clause 2, later time) | the orders of `R_b` at `t = 3` between levels 1 and 2, 2 and 3 | `>= 1.7` each | the three-point fits give `1.47` for the first pair, the two-point fits `1.8` to `1.9` |
| C1 | continuity of the calculated cells, every step of every overset run | `<= 1e-9` | ADR 0246: `7e-14` |
| C2 | unconverged linear solves, every run | `0` | ADR 0251: none |
| QL (control) | `R_b(L, 0.5)` of this file (library quadratic stencils) against ADR 0246 (nine-cell stencils of the test), `4.8742e-5`, `2.1922e-5`, `1.4073e-5`, levels 1, 2, 3 | within 5 percent | ADR 0251: the errors with the two stencils agree to 1.3 percent |
| QS (control) | in a Qb run: every component fringe stencil equals that of the base run bit for bit; every background fringe stencil has nine cells, all calculated component cells, the donor of the base run first; no fallback | exact | ADR 0251 P5, P6 |
| NL (negative control) | the orders of `R_b` at `t = 0.5` between levels 1 and 2, 2 and 3 (the miss that is diagnosed, reproduced) | `<= 1.5` each | ADR 0246: `1.153`, `1.093` |
| K (kinematic, levels 1 to 5, no flow solve) | the background fringe: (i) the root mean square of the truncation of the exact velocity with the linear stencils, orders between levels 2 and 3, 3 and 4, 4 and 5; (ii) the same with the quadratic stencils; (iii) the field `(1 + x + 2 y)(1 - x + y)` at every background fringe centre from the quadratic stencil, over its largest value | (i) `>= 1.8` each; (ii) `>= 2.7` each; (iii) `<= 1e-11` | (i) second order, the ring moves by up to 7 percent between these levels; (ii) ADR 0251 P8 `3.00`; (iii) ADR 0251 P2 `7e-16` |

Printed with K, no limit: `kappa_b = rms abs(tau_b) / h_c^2` per level, the root mean square of the normal and of the
tangential part of the truncation, its mean over the ring, and its root mean square per tangential fraction (the three
classes of the modulation); the same for RE.

### Mutations (scratch copy, not committed)

Each is applied alone to a `git archive` copy of the final tree in `E:/projects/cfd/w49i-work/f349-scratch`, the named
test is run once and must fail. `scripts/limit_audit.py` is run on the new test file and every hit is read.

| Id | Mutation | Must fail |
| --- | --- | --- |
| MU1 | `rcfd_fv::overset`, `overset_pressure`: the offset `delta` is not applied | C1 (fast test) |
| MU2 | `rcfd_mesh::overset`, `connectivity`: the linear stencil whatever the mode | QS and K (ii) |
| MU3 | the test's substitution: the component fringe cells in place of the background fringe cells | QS |
| MU4 | the test's `R_b`: the decay factor left out | QL |

### Classes

A miss of A1, A2, A3, C1, C2 or K is reported with its number and kept as an ignored `*_known_miss` test (or record
test) with the limit intact. A control (QL, QS, NL) that does not hold, or a mutation that does not make its check fail,
is a defective set-up: corrected by an amendment before any re-run, never counted as a pass. The hypotheses are decided
by the numbers of their table only. What the registered measurements leave undecided is followed up by an amendment
written and committed before its runs, or listed as open.

## Predictions (mine, before any run)

C1, C2, QL, QS, NL, K met. A2 missed, with orders between 1.2 and 1.7. A3: I do not know; the three-point fits say
missed on the first pair (about 1.5). Hdt ruled out. Hmod and Hsp: no prediction I trust; if anything, `x < 0` (the
response to a given truncation grows as the receiving grid is refined). A1 missed for D; for E unknown.

## Decision rule

- **done**: one clause of the acceptance is met by its registered check: A1 by a switch of the library on levels 1 to 3
  and then on level 4 (amendment), or A2, or A3; or clause 3 in the strong sense: a hypothesis (of this table or of an
  amendment) is confirmed by its registered number, names the term of the coupled step, and its scaling reproduces the
  measured `R_b` on the levels run; and C1, C2 and K are met, the controls held and the mutations failed their checks.
- **partial**: no clause is met in that sense, and the term is located by decided hypotheses with their numbers (which
  spacing, which time behaviour, what is ruled out); or a control or mutation did not hold and was not corrected.
- **todo**: nothing is decided, or no registered measurement could be run.

A library switch that the diagnosis points at is described in the addendum with its derivation and proposed as a
follow-up item; it is not built here.

## Limits of this registration

- Taylor-Green in RL and RE only: uniform Cartesian parts, parallel grids, one component at rest, `nu = 0.1`.
- `R_b` is a difference of two runs; it is the response to the truncation of the background fringe in the presence of
  the linear component fringe. The interaction of the two truncations is not separated.
- The quadratic stencil replaces all four interpolated quantities at once.
- A3 is run on levels 1 to 3 only (level 4 to `t = 3` would take hours); A2 at `t = 0.5` only. Levels 6 and 8 are not
  phase-locked and are not used; level 7 is out of reach.
- RE has its own geometry (the ring is not symmetric about the component centre); only its orders are used.
- F-369 (a moving component, the planar B3) is not treated.

## Open questions

Listed with options and a recommendation in the addendum, after the results.
