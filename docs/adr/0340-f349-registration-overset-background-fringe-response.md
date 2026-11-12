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

## Amendment 1 (2026-11-12, written after the runs of K, the gate check, L, F at level 4 and E, while D, S and F at level 5 were still running, and before any further run)

The tests were committed as registered (`a1b1d772`, compiled, formatted and linted without running) and run once each.
Numbers are in addendum 1. Two things need this amendment.

**K (i) missed.** The order of the linear truncation of the background fringe is `1.810`, `2.565`, `1.621`, `2.371`
between the successive levels 1 to 5; the limit `1.8` is missed between levels 3 and 4. `kappa_b` is `0.0685`, `0.0781`,
`0.0621`, `0.0693`, `0.0638`: it alternates between the odd and the even levels. The registration took from ADR 0246 that
the normal fraction of the fringe centres (`0.667` on odd, `0.333` on even levels) is locked "up to a reflection" and
treated the reflection as a symmetry. It is not one: the exact velocity has a mixed second derivative, whose part of the
truncation changes sign when one coordinate is reflected, and the tangential fractions `0`, `1/3`, `2/3` are not
symmetric about the cell centre (the class on a face takes its donor by the tie). On RE, which is locked exactly, the
orders are `1.995`, `1.956`. K is kept as the ignored test `k_background_fringe_truncation_known_miss` with its limits
intact and is not run again. It follows that an order of `R_b` between successive levels of RL carries this alternation of
its source (a factor `1.26` between neighbours); RE does not.

New check **K'** (test `k2_truncation_within_the_parity_families`): K (ii), K (iii) and QS as registered, and K' (i): the
order of the linear truncation between the levels of the same parity, 1 to 3, 3 to 5, 2 to 4, limit `>= 1.8` each.
Disclosed: these three numbers follow from the root mean squares that K printed (`2.09`, `1.95`, `2.17`); the check
repeats a deterministic kinematic computation.

**What the runs show so far** (all at the registered limits; details in the addendum). A3 is missed (`1.418`, `1.089`
at `t = 3`), E misses A1 with the orders `0.983`, `1.015` on the exactly locked set-up, so Hmod is ruled out and HT is ruled
out. The printed coefficients say where the order is lost: the calculated background cell next to the fringe follows the
change of the normal velocity of its fringe cell only in part, `a_c = 0.51`, `0.66`, `0.76`, `0.79` at levels 1 to 4 at
`t = 0.5` (`0.10`, `0.24`, `0.37`, `0.42` at `t = 0.025`), and the calculated component cells follow their fringe in the
same way (`0.34`, `0.73` at levels 1, 3). The response per unit of truncation therefore grows with the refinement.

New hypothesis, with the term named:

| Id | Hypothesis | Measurement | Deciding number |
| --- | --- | --- | --- |
| HK | the normal velocity of a fringe cell is not imposed on the flux of its face: the flux is that of the pressure equation with the interpolated fringe pressure, and it differs from the face-interpolated velocity by the defect `D`, which is held by a pressure jump across the face. From `D_new = theta D + R` with `R = -(3/4) rAU_f dJ / h` (a jump `dJ` of `p_F - p_C` against the smooth continuation changes the compact gradient by `dJ / h` and the Green-Gauss gradient of the calculated cell by `dJ / (2 h)`; equal `rAU`), `1 - theta = a / (1 + a)`, `rAU = dt / (1 + a)`, `a = 4 nu dt / h^2`: in the steady state `dJ = -(16 nu / (3 h)) D`. The coupling of the normal velocity is a resistance proportional to `nu / h`, not an identity | the base and Qb runs at `t = 0.5`, face by face on `Gamma_B`: `dD`, the change of `phi / abs(S) - (u)_f . n`, and `dJ`, the change of `p_F - p_C`: `k = -(3 h / (16 nu)) sum dJ dD / sum dD^2` and the correlation `-sum dJ dD / sqrt(sum dJ^2 sum dD^2)`; RE at levels 1 and 2 (equal `rAU`, the derivation applies); RL at levels 1 and 2 and the component fringe printed | confirmed: `0.6 <= k <= 1.4` and correlation `>= 0.8` on RE at both levels; ruled out: correlation below 0.5 at both; otherwise undecided |

Estimate behind the limits, disclosed: from the root mean squares that E printed, `(16 nu / (3 h)) (1 - a_m) rms(t) / 2`
against the printed change of the pressure jump gives ratios of about `0.6` to `0.65` at levels 1 to 3 (this takes `t -
m` for twice the defect and ignores the part of `c`); I expect `k` between `0.6` and `1.2`. Tests
`hk_kink_relation_re_level_1` (in the gate) and `hk_kink_relation_level_2` (ignored, slow, run once). `pair` now also
returns its two runs; the registered runs are unchanged.

A further mutation: **MU5**, the test's `kink`: the defect without the face-interpolated velocity (the flux change
alone); must fail HK.

The decision rule is unchanged.

## Addendum 1: results (2026-11-12)

Measured against the limits and deciding numbers registered above and in amendment 1; none was changed. Release builds on
Windows; the library is that of main a5db938b, unchanged: this work adds one test file and this ADR, and `git diff a5db938b
-- crates/*/src` is empty, so no default and no pinned result can have moved. Commits: the registration `71796963` (alone,
before any code); the tests as registered `a1b1d772` (compiled, formatted and linted without running); amendment 1 with
its tests `846325bb`, before their runs; the known-miss renames `37a3cad8`. Each registered run was made once (the
deterministic run of RL at level 1 is shared by several tests and printed the same digits each time). Logs:
`E:/projects/cfd/w49i-work/f349-scratch` (`f349-fast.log`, `f349-l_long_run_*.log`, `f349-f_finer_level_*.log`,
`f349-d_half_*.log`, `f349-e_equal_*.log`, `f349-s_spacings_apart.log`, `f349-a1-fast.log`, `f349-hk_level_2.log`,
`f349-MU1.log` to `f349-MU5.log`).

### What the diagnosis found, in short

1. **No clause of the order check is met.** `R_b` at `t = 0.5` on levels 1 to 5 is `4.996e-5`, `2.177e-5`, `1.412e-5`,
   `8.917e-6`, `6.425e-6`: orders `1.198`, `1.068`, `1.598`, `1.469`. Run on to `t = 3` (levels 1 to 3): `1.403e-4`,
   `5.250e-5`, `3.376e-5`, orders `1.418`, `1.089`; `R_b` is still growing there (by 6 percent from `t = 2.5`). Half the
   time step changes `R_b` by `0.9`, `0.7`, `0.7` percent.
2. **The source itself is of second order but alternates.** The truncation of the background fringe has `kappa_b =
   0.0685`, `0.0781`, `0.0621`, `0.0693`, `0.0638` on levels 1 to 5: the reflection of the fringe phase between odd and even
   levels is not a symmetry (amendment 1). Between levels of the same parity the truncation converges at `2.09`, `1.95`,
   `2.17`, and `R_b` at `1.150` (1 to 3), `1.288` (2 to 4), `1.542` (3 to 5). The response per unit of truncation,
   `R_b / rms(tau_b)`, is `0.210`, `0.321`, `0.590`, `0.593`, `0.726`: **it grows with the refinement.**
3. **On a set-up without the alternation and without the modulation along the ring the order is 1.0.** RE (equal
   spacings, both fringes at one fixed fraction of their donor cells, truncation orders `1.995`, `1.956`): `R_b =
   6.428e-5`, `3.251e-5`, `2.154e-5`, orders **`0.983`, `1.015`**; `R_b / rms(tau_b) = 0.168`, `0.339`, `0.497`,
   proportional to the level with no sign of levelling off (`1 / ratio = 5.95`, `2.95`, `2.01`).
4. **Where the order is lost: the fringe velocity is imposed softly, at both fringes.** `a_c`, the share of a change of
   the normal velocity of a fringe cell that the calculated cell across the face follows, at the background fringe of RL
   at `t = 0.5`: `0.51`, `0.66`, `0.76`, `0.79`, `0.83` (levels 1 to 5); at `t = 0.025`: `0.10`, `0.24`, `0.37`, `0.42`,
   `0.51`; at `t = 3`: `0.63`, `0.76`, `0.85` (levels 1 to 3). At the component fringe at `t = 0.5`: `0.34`, `0.64`, `0.73`,
   `0.79`, `0.83`. On RE along the path at `t = 0.5`: the truncation converges at `1.995`, `1.956`; the calculated
   background cells next to the ring at `1.451`, `1.485`; the component fringe cells at `1.604`, `1.398`; the calculated
   component cells at `0.983`, `1.015`. Each of the two fringes costs about half an order on these levels.
5. **The term is named and measured (HK, confirmed).** The flux of a face between a calculated and a fringe cell is the
   flux of the pressure equation with the interpolated fringe pressure; it differs from the face-interpolated velocity by
   the defect `D`, held by a pressure jump `dJ = -(16 nu / (3 h)) D`. Face by face on the background fringe of RE the
   coefficient is `0.9986` (level 1) and `1.0162` (level 2) of that, correlation `0.995` and `0.999`. The coupling of the
   normal velocity across a fringe face is a resistance proportional to `nu / h`, not an identity: a mismatch of second
   order is carried by a pressure of first order next to the fringe, and the interior follows the fringe only as fast as
   that pressure moves it.
6. **Not shown:** the order that `R_b` reaches once `a_c` is close to one. The saturation of `R_b / rms(tau_b)` is not
   visible up to level 5 at `t = 0.5` or up to level 3 at `t = 3`, and I have no formula that gives `R_b` from the
   resistance: the estimate "order = 2 minus the sum of `1 - a_c` over the two fringes" gives `1.13`, `1.43` on RE where
   `0.98`, `1.02` is measured.

### Checks

| Check | Limit | Measured | Verdict |
| --- | --- | --- | --- |
| A1 (D) half the step, orders of `R_b`, levels 1 to 2, 2 to 3 | `>= 1.7` | `1.201`, `1.068` (`5.0402e-5`, `2.1928e-5`, `1.4223e-5`) | **missed** (known-miss test) |
| A1 (E) RE, orders of `R_b`, levels 1 to 2, 2 to 3 | `>= 1.7` | `0.983`, `1.015` | **missed** (known-miss test) |
| A1, the transient Rhie-Chow term off | `>= 1.7` | `1.763`, `1.398` (ADR 0246, not run again) | missed on the second pair |
| A2 orders of `R_b` at `t = 0.5`, levels 3 to 4, 4 to 5 | `>= 1.7` | `1.598`, `1.469` | **missed** (record test) |
| A3 orders of `R_b` at `t = 3`, levels 1 to 2, 2 to 3 | `>= 1.7` | `1.418`, `1.089` | **missed** (known-miss test, record test) |
| C1 continuity, every step of every overset run | `<= 1e-9` | largest `1.0e-13` | met |
| C2 unconverged linear solves | `0` | `0` in every run | met |
| QL `R_b(L, 0.5)` against ADR 0246, levels 1, 2, 3 | within 5 percent | `2.5`, `0.7`, `0.3` percent | held |
| QS the substitution | exact | no component stencil changed, every background fringe stencil nine calculated component cells with the linear donor first, no fallback, in every run and on RL levels 1 to 5 and RE levels 1 to 3 | held |
| NL orders of `R_b` at `t = 0.5`, levels 1 to 2, 2 to 3 | `<= 1.5` | `1.198`, `1.068` | held |
| K (i) linear truncation of the background fringe, levels 2 to 3, 3 to 4, 4 to 5 | `>= 1.8` | `2.565`, `1.621`, `2.371` | **missed** between levels 3 and 4 (known-miss test; amendment 1) |
| K (ii) quadratic truncation, same pairs | `>= 2.7` | `3.108`, `2.916`, `3.075` | met |
| K (iii) the quadratic field | `<= 1e-11` | largest `1.1e-15` | met |
| K' (i) linear truncation within the parity families, 1 to 3, 3 to 5, 2 to 4 (amendment 1) | `>= 1.8` | `2.089`, `1.948`, `2.173` | met |
| HK kink relation on RE, levels 1, 2 (amendment 1) | `0.6 <= k <= 1.4`, correlation `>= 0.8` | `0.9986`, `1.0162`; `0.9952`, `0.9988` | met |

### Hypotheses

| Id | Deciding number (registered) | Measured | Verdict |
| --- | --- | --- | --- |
| HT settled second order | confirmed: A3 met; ruled out: both orders at `t = 3` below 1.5 | `1.418`, `1.089`; `G_L = 2.81`, `2.41`, `2.39` | **ruled out** |
| HF not yet asymptotic | confirmed: A2 met; ruled out: both below 1.5 | `1.598`, `1.469` | **undecided** by the rule; within the parity families the order rises, `1.150`, `1.288`, `1.542` |
| Hdt a time-step term | ruled out: `abs(rho_dt - 1) <= 0.10` at three levels | `1.009`, `1.007`, `1.007` | **ruled out** |
| Hmod unequal spacings, modulation along the ring | confirmed: both orders on RE at least 1.7; ruled out: both below 1.5 | `0.983`, `1.015` | **ruled out** |
| Hsp which spacing | background: `x <= -0.5`, `y >= 1.5`; component: `abs(x) <= 0.5`, `y <= 1.5` | `y_1 = 0.962`, `y_2 = 1.688`, `x_1 = -0.117`, `x_2 = -0.324` (`R_b(1,2) = 2.565e-5`, `R_b(2,1) = 5.417e-5`, `R_b(2,4) = 6.757e-6`, `R_b(4,2) = 2.726e-5`) | **mixed**. Refining the background alone *raises* `R_b` (by 8 and 25 percent) although the truncation is that of the unchanged component; `a_c` at the background fringe goes with the background level whatever the component (`0.51`, `0.50` at level 1; `0.66`, `0.67`, `0.70` at level 2; `0.77` at level 4). Within the families: `0.991` (B2), `1.925` (C2) |
| HK the kink relation (amendment 1) | confirmed: `0.6 <= k <= 1.4`, correlation `>= 0.8` on RE at both levels | `0.9986`, `1.0162`; `0.9952`, `0.9988` | **confirmed** |

Printed, no limit. HK elsewhere: on the component fringe of RE `1.029`, `1.045` (correlation `0.997`, `0.999`); on RL,
where the spacings differ, `1.439`, `1.383` on the background fringe and `0.820`, `0.795` on the component fringe
(correlation `0.997` and above): the relation is as sharp, the coefficient is not the one derived for equal `rAU` (it is
close to `1.08` times the ratio of the receiving to the donor spacing; not derived). `R_b` at `t = 0.5, 0.75, 1, 1.5, 2,
2.5, 3`: level 1 `4.996e-5`, `7.111e-5`, `8.670e-5`, `1.076e-4`, `1.215e-4`, `1.319e-4`, `1.403e-4`; level 2 `2.177e-5`,
`2.897e-5`, `3.413e-5`, `4.116e-5`, `4.594e-5`, `4.957e-5`, `5.250e-5`; level 3 `1.412e-5`, `1.871e-5`, `2.204e-5`,
`2.659e-5`, `2.965e-5`, `3.194e-5`, `3.376e-5`. The order between levels 1 and 2 is `3.28`, `2.60`, `2.47` at `t =
0.0025`, `0.025`, `0.125` and `1.31` at `0.25`: at level 1 the mean of `d` over the calculated component cells changes
sign between `t = 0.125` and `0.5` (`(-1.76e-5, 2.35e-5)` to `(4.6e-6, -1.26e-5)`), an early part and a later part of
opposite sign. RL at level 1, `t = 0.5`, root mean square of `d`: BF `2.86e-4`, calculated background cells `1.23e-4`
(`rho < 5 H`), `1.27e-4` (overlap), `8.6e-5`, `3.7e-5` (outside), CF `1.12e-4`, CC `5.0e-5`; at level 5 `1.58e-5`,
`1.09e-5`, `7.7e-6`, `5.0e-6`, `2.0e-6`, `6.6e-6`, `6.4e-6`. The change of the pressure jump at the background fringe faces
(root mean square) `2.86e-4`, `9.8e-5`, `5.2e-5`, `3.4e-5`, `2.3e-5` at levels 1 to 5: first order, as the kink relation
says. Run times (three processes at once on a shared machine at full load): L 1771 s (levels 1, 2) and 6288 s (level 3);
F 2382 s and 7256 s; D 2613 s; E 1097 s; S 4284 s; HK 566 s.

### Mutations

Applied alone to a `git archive` copy of `37a3cad8` (`f349-scratch/mut`, own target directory) by `mutate.py` (an exact
replacement that must occur once), the named tests run once, the file restored and compared byte for byte (equal in all
five). The commits after `37a3cad8` add one ignored record test without a flow run and this addendum.

| Id | Must fail | Result |
| --- | --- | --- |
| MU1 the offset `delta` not applied | C1 | failed: continuity `4.5e-3`, `2.8e-3` (and QL `0.17`) |
| MU2 the linear stencil whatever the mode | QS and K (ii) | failed: 28 to 124 background stencils not quadratic; K (ii) `2.565`, `1.621`, `2.371`; QL `1.0` |
| MU3 the substitution on the component fringe | QS | failed: 76 component stencils changed, 28 background stencils linear |
| MU4 `R_b` without the decay factor | QL | failed: `7.3` percent |
| MU5 (amendment 1) the defect without the face-interpolated velocity | HK | failed: coefficient `-0.177`, correlation `-0.777` |

`scripts/limit_audit.py` on the new file: no hit.

### Deviations and disclosures

- The registration treated the reflection of the fringe phase as a symmetry (from ADR 0246) and K (i) missed for it. The
  orders of `R_b` between successive levels of RL, here and in ADR 0246, carry the alternation of the source; the
  acceptance's numbers `1.153`, `1.093` are such orders. RE and the parity families do not.
- The numbers of K' were known from K before K' was registered; the estimate for HK was made from printed numbers of E.
  Both are said in amendment 1.
- Amendment 1 was written while D, S and F at level 5 were still running; their numbers were not known then, except
  `R_b` of the first five pairs of S, which I had looked at (they are not used in the amendment).
- The pairs 2 to 3 (A3, NL) and 3 to 4, 4 to 5 (A2) are formed from the printed numbers of different test processes and
  recorded in `a_orders_between_test_processes_known_miss`, as registered.
- My predictions: A2 missed, Hdt ruled out, the controls met: right. K met: wrong. For A3, Hmod and Hsp I had none that
  I trusted; the model of a settling time proportional to `h` that I leaned on is ruled out as the whole explanation (the
  response does not settle at second order by `t = 3`), although `a_c` does rise faster on the finer levels.
- The long run times of the three heaviest records (level 3 to `t = 3`, level 5, `RL(4,2)`) are those of a loaded machine.

### Not run

- Level 4 at `t = 3`, levels above 5, RE above level 3: not registered (hours each).
- No library switch that imposes the fringe flux exists, so clause 1 could not be tested by removing the term; the term
  is identified by its relation (HK), not by its removal.
- Outer iterations, BDF2 and the number of correctors as switches: with `rho_dt = 1.007` a splitting or time error is
  excluded, and they were not run.
- The existing overset suites and the rest of `rcfd-fv`: the library is unchanged; the lead runs the gate. The new file
  without the ignored tests ran on the tree of amendment 1: 3 passed, 9 ignored, 59 s.

### After the tests

`cargo fmt -p rcfd-fv --check` clean. `cargo clippy -p rcfd-fv --all-targets -- -D warnings` clean (the only crate
touched, tests only).

### Decision by the registered rule

**Partial.** No clause is met in the registered sense: A1, A2 and A3 are missed, and the strong form of clause 3 asks for
a scaling that reproduces the measured `R_b`, which I do not have. The term is located by decided hypotheses with their
numbers: a time-step term, a late settling at second order and the unequal spacings are ruled out; on the exactly locked
set-up the order is `0.98`, `1.02`; the cause named and confirmed is the soft imposition of the normal fringe velocity
(HK: `dJ = -(16 nu / (3 h)) D` to `0.2` and `1.6` percent), which lets the calculated cells follow their fringe by `0.5` to
`0.83` only and makes the response per unit of truncation grow with the refinement. Said plainly: why `R_b` is below 1.7
is named with numbers; at which level or time it would reach 1.7 is not shown, and whether removing the resistance
restores the order is a conjecture that needs the library switch below.

### The library switch the diagnosis points at (not built)

Today (ADR 0219 M6 to M8) the flux of a face `f` between a calculated cell `C` and a fringe cell `F` comes from the
pressure equation, with `p_F` a Dirichlet value. The interpolated velocity of `F` enters that flux only through `HbyA_F`,
and the difference to `(u)_f . S` relaxes as `D_new = theta D + R`, which is the resistance of HK.

Switch (opt-in, default off, defaults bit-identical): **the flux of a calculated-fringe face is prescribed**, `phi_f =
(u^I)_f . S_f + q A_f`, where `u^I_F` is the interpolated fringe velocity of the new time level, `(u^I)_f` its face
interpolation with `u_C`, and `q` one number per closed fringe ring. In the pressure equation such a face is a fixed-flux
(zero-coefficient) face, as a wall with a given flux is: the row of `C` loses the coefficient of `p_F` and gains the source
`-phi_f`. Then `D` on these faces is zero by construction and the continuity of `C` imposes the interpolated normal
velocity at once (`a_c = 1` up to the interpolation). What follows from it, to be designed in the follow-up:

- compatibility: the calculated cells of each part enclosed by fixed-flux faces need a net flux of zero; `q` (the mean
  normal velocity defect of the interpolated velocity over the ring, `-1.6e-5` to `-2.0e-8` on RL by ADR 0246) restores
  it and takes the place of the pressure offset `delta`;
- the pressure of each part is then fixed up to a constant by its own equation: one reference per part, or the level
  taken from the interpolated fringe pressure in the mean;
- because `u^I_F` depends on the donors' new velocity, the prescription is lagged within a corrector or needs the
  corrector loop to converge; the existing outer iterations can carry it;
- the interpolated fringe pressure is still needed for the pressure gradient of `C`.

The switch belongs in `rcfd_fv::overset` (`piso`, `overset_pressure`), which another item holds now. Whether it restores
the order is the acceptance of the follow-up, not a result of this ADR.

### Open questions (each with options and a recommendation)

- **Q1. The status of F-349.** Options: (a) partial, as the rule gives; (b) done on the third clause (why not is named
  with numbers). Recommendation (a): the order at which `R_b` settles is not shown and the cause is confirmed by its
  relation, not by its removal. Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation);
  not a decision the maintainer made in person, and (b) is the maintainer's to choose.
- **Q2. The prescribed-flux fringe face as an opt-in.** Options: (a) a follow-up item (text below), after the held work on
  `overset.rs` has landed; (b) leave it: the quadratic donor interpolation of F-336 removes the truncation that the
  response answers to, whatever the response's order. Recommendation (a) at low priority: the soft coupling acts on every
  difference between the two parts, not only on the truncation (it is a candidate for the excess of F-369). Taken on the
  standing instruction as a proposal to the lead.
- **Q3. The order checks of RL between successive levels.** The background fringe of RL alternates between odd and even
  levels (a factor `1.26` in the source). Options: (a) keep the existing records as they are and use RE, or the parity
  families of RL, for new order checks of the coupling; (b) redefine RL. Recommendation (a): existing tests and records
  are not edited. Taken on the standing instruction.
- **Q4. F-369.** Its acceptance asks for the shares of the fringe pressure, the mass offset and the flux of the faces that
  change kind. Option: add the coefficient `a_c` and the kink relation of this ADR to its measurements. Recommendation:
  yes, as a note on F-369 (text below); its acceptance is unchanged.

### For the lead: roadmap entries

No roadmap file was edited.

**F-349**, proposed status: `partial`. Proposed note:

> From ADR 0246 question 4, recommendation taken on the maintainer's standing instruction of 2026-11-11. ADR 0340, 2026-11-12: PARTIAL, a diagnosis, tests only, no library change. No clause of the order check is met. R_b (the library's quadratic donor interpolation of F-336 on the background fringe against the default stencils; within 2.5 percent of ADR 0246) is 4.996e-5, 2.177e-5, 1.412e-5, 8.917e-6, 6.425e-6 at t = 0.5 on levels 1 to 5, orders 1.198, 1.068, 1.598, 1.469; run on to t = 3 on levels 1 to 3 it is 1.403e-4, 5.250e-5, 3.376e-5, orders 1.418, 1.089, and still growing; half the time step changes it by under 1 percent. Found on the way: the truncation of the background fringe of RL alternates between odd and even levels (coefficient 0.0685, 0.0781, 0.0621, 0.0693, 0.0638; the reflection of the fringe phase is not a symmetry), so orders between successive levels of RL carry a factor 1.26; within the parity families R_b has 1.150, 1.288, 1.542. On a set-up locked exactly, with equal spacings and no modulation along the ring (RE), the truncation converges at 1.995, 1.956 and R_b at 0.983, 1.015: the response per unit of truncation is proportional to the level (0.168, 0.339, 0.497) with no levelling off. Ruled out with numbers: a time-step or splitting term, a late settling at second order, the unequal spacings and the modulation. Named and confirmed: the normal velocity of a fringe cell is imposed softly. The flux of a face between a calculated and a fringe cell is that of the pressure equation with the interpolated fringe pressure and differs from the face-interpolated velocity by a defect D held by a pressure jump dJ = -(16 nu / 3 h) D (measured face by face on RE: 0.9986, 1.0162 of that, correlation 0.995, 0.999), a resistance proportional to nu / h. The calculated cell across a fringe face follows a change of the normal fringe velocity by 0.51, 0.66, 0.76, 0.79, 0.83 at the background fringe (levels 1 to 5, t = 0.5; 0.10 to 0.51 at t = 0.025) and by 0.34 to 0.83 at the component fringe; each fringe costs about half an order on these levels. Refining the background alone raises R_b. Not shown: the level or time at which R_b reaches 1.7, and a formula for R_b from the resistance. One registered kinematic check missed (the truncation order between levels 3 and 4, 1.621 against 1.8, the alternation) and is kept as a known miss. Controls held, five mutations caught. Follow-up (ADR 0340 question 2, recommendation taken on the maintainer's standing instruction of 2026-11-11): an opt-in that prescribes the flux of the calculated-fringe faces from the interpolated velocity.

**F-334**, status unchanged (`partial`). In its note, replace the sentence "Open: why the response to a second-order truncation stays below second order." by:

> Traced by ADR 0340 (F-349): the normal velocity of a fringe cell is imposed on the calculated cells through a resistance proportional to nu / h (a flux defect held by a pressure jump dJ = -(16 nu / 3 h) D), so the response per unit of truncation grows with the refinement; the truncation of the background fringe of RL also alternates between odd and even levels.

**F-369**, status unchanged. Add to its note:

> ADR 0340 (F-349) found that the fringe velocity is imposed softly on the calculated cells (the following coefficient a_c of 0.5 to 0.83 and the kink relation dJ = -(16 nu / 3 h) D at the fringe faces); both are candidates for the measurements of this item.

Proposed follow-up item (the number is the lead's): **Overset: prescribed flux on the faces between calculated and fringe cells.** Depends on F-349 and F-344. Acceptance:

> An opt-in of the overset step under which the flux of a face between a calculated and a fringe cell is the face interpolation of the interpolated fringe velocity plus one correction per closed fringe ring that makes the net flux of each enclosed part zero (ADR 0340, the switch described there): on RE of ADR 0340 at levels 1 to 3 the following coefficient a_c at both fringes is at least 0.9 at t = 0.025 and the order of R_b at t = 0.5 is at least 1.7 on both pairs, on RL the order of R_b is at least 1.7 within the parity families (levels 1 to 3 and 2 to 4), continuity of the calculated cells and the mass of the domain hold to 1e-9, the uniform-flow check U1 of ADR 0219 holds, and without the opt-in every overset result is bit-identical; or the ADR shows with a number why the prescribed flux is not admissible.
