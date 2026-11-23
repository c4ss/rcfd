# ADR 0771: F-881 registration: why the mean drag of the cylinder at Re 100 converges at an order near 1.4

Status: **REGISTERED 2026-11-22, written before any code of F-881 and before any run of its checks.** Before this text
no solver was run and no test was run by this work. What was done: reading (below) and the arithmetic below (a Python
script on recorded numbers and on the formula of the geometric grading). Results go in addenda at the end; nothing above
them is changed after a run.

It answers the brief `docs/adr/briefs/f881-cylinder-drag-order.md` and follows `docs/process/agent-rules.md`, version 5.
Branch `f881-cylinder-drag-order` from main d6e44b27, scratch `E:/projects/cfd/w49i-work/f881-cylinder-drag-order-scratch`.
Parents: [ADR 0763](0763-f869-registration-the-mesh-converged-mean-drag-of-the-cylinder-at-re-100.md) (the sequence, the
order 1.387, the maintainer's answer that made this item) and
[ADR 0761](0761-f869-registration-the-mean-drag-of-the-cylinder-at-re-100-against-the-published-spread.md) (the first
sequence and its order 1.38). Neither is edited, and neither of their test files.

## What is asked

The acceptance of F-881 (`docs/roadmap.json`, read by a script): "Every limit not stated here is registered in the ADR
before the run. The observed order of the mean drag of the cylinder at Re 100 (1.38 on the sequence of ADR 0761, 1.387
on that of ADR 0763) is explained by a cause that a registered run shows: candidates named before the run (among them
the grading of the O-grid, the wall force integral, the far-field condition), each tested by a run that changes it
alone, with the order it gives; a cause is named only if its run moves the order by more than a registered amount, and
none is named otherwise. No solver change is made by this item; a fix, if one is found, is a proposal for a further
item."

## What was read

- `AGENTS.md`; `docs/process/agent-rules.md` (version 5) whole; the brief whole; ADR 0763 whole;
  `crates/rcfd-api/tests/f869_cylinder_mesh_converged_drag.rs` whole and the head of its mutation patch.
- ADR 0761: its section headings, lines 316 to 395 (addendum 1: the results, the checks and the reading of its miss) and
  the lines a search for "order" gave; **not read whole**. `crates/rcfd-api/tests/f869_cylinder_re100_mean_drag.rs`
  lines 59 to 300 (its set-up, force, run and record).
- [`docs/process/tolerances.md`](../process/tolerances.md) whole; the entry F-881 of `docs/roadmap.json` (by script);
  `docs/roadmap/proposals/0763-F-869.json`; the head of ADR 0770 (style of a registration of a diagnosis).
- Solver and post-processing, read to know what each candidate changes, not changed: `crates/rcfd-post/src/integrals.rs`
  lines 1 to 230 (`wall_force_and_moment` and `force_and_moment`); `crates/rcfd-fv/src/bc.rs` lines 20 to 60
  (`FaceCoeffs`, `BcCtx`) and 405 to 430 (`Freestream`); `crates/rcfd-fv/src/convection.rs` lines 78 to 160 (the
  schemes; `LinearUpwind` is the unlimited face extrapolation `phi_U + grad(phi)_U . (x_f - x_U)`);
  `crates/rcfd-fv/src/pimple.rs` lines 40 to 200 (the fields of `PimpleSettings`) and 1007 to 1047 (`piso`);
  `crates/rcfd-fv/src/pressure.rs` the heads of `pressure_bc` and `boundary_values`; `crates/rcfd-fv/src/field.rs` the
  signatures of `values`, `from_fn` and `boundary_coeffs`; `crates/rcfd-mesh/src/gen.rs` lines 11 to 31 (`Grading`) and
  540 to 600 (`cylinder_ogrid`: cell `it + n_theta ir`, the patch `cylinder` the faces of ring 0).
- `git log 2bab1863..d6e44b27` on `crates/rcfd-fv`, `rcfd-mesh`, `rcfd-post`, `rcfd-linalg`, `rcfd-core`: no commit,
  so the solver is the one that made the records of ADR 0763.
- No paper was fetched and no published value is used: the item is about the order of the code's own sequence. The
  Taylor expansions below are first principles. This registration adds no reference fragment.

## Disclosure of prior knowledge and the arithmetic done beforehand

Known: every number of ADR 0761 and ADR 0763. Not known: any result of the candidate runs below, and any value of the
drag with the wall terms of C1. ADR 0763 reported the order of its sequence by two forms: 1.387 with h = N^(-1/2) (N the
cell count) and 1.479 with both ratios sqrt 2.

**The order hangs on the definition of h by about 0.1.** The procedure of ADR 0763 (bisection for unequal ratios) applied
to the printed five-decimal values, with five definitions of the refinement ratio, by a scratch script:

| Ratio from | ADR 0763 sequence (90 x 70, 128 x 100, 181 x 141; dt 0.025; 30 D) | ADR 0761 sequence (64 x 50, 90 x 70, 128 x 100; dt 0.05; 20 D) |
| --- | --- | --- |
| cell counts, h = N^(-1/2) (ADR 0763) | 1.42539, 1.41203: order 1.385 | 1.40312, 1.42539: order 1.512 |
| both sqrt 2 (ADR 0761) | order 1.477 | order 1.377 |
| n_theta | 1.42222, 1.41406: order 1.417 | 1.40625, 1.42222: order 1.474 |
| n_r | 1.42857, 1.41000: order 1.353 | 1.40000, 1.42857: order 1.550 |
| the first radial cell | 1.41905, 1.40364: order 1.392 | 1.38750, 1.41905: order 1.610 |

(1.385 here and 1.387 in ADR 0763 differ by the rounding of the inputs.) Two readings follow, and both are arithmetic, not
results. (i) "The same order 1.38 on two sequences" holds only when each sequence is read by a different form: by the one
form of ADR 0763 the order of ADR 0761's sequence is 1.51, at the edge of the range [1.5, 2.5], not 1.38. (ii) With three
meshes refined by nearly equal ratios near 1.41, a change of the ratio by 1 % moves the order by about 0.1; the order is
a soft number. What stays under every definition: both sequences converge at an order between 1.35 and 1.61, under the
formal 2.

Consequence for the design: the cheaper sequence of ADR 0761 does not reproduce "the order near 1.4" by the procedure of
ADR 0763, so it is not used (the brief allows a cheaper one only if it reproduces the order first). Every run here is on
the sequence of ADR 0763, and every order here is by its procedure (h = N^(-1/2)), for the baseline and for each
candidate alike, so that the definition of h is common to both sides of every comparison.

**The size of the first-order wall terms, estimated.** The force of `wall_force_and_moment` takes the pressure on a wall
face as the value of the wall cell (`p_f = p_P`: the pressure condition on a no-slip wall is zero gradient) and the
wall-normal derivative of the velocity as `(0 - u_P)/d` with d the normal distance of the cell centre (`FaceCoeffs::
from_value`, `c = (a - 1)/dn`). With s the distance from the wall into the fluid, a smooth profile u(s) = a s + b s^2 +
O(s^3) gives u_P / d = a + b d: an error b d, first order in d. At a no-slip wall the momentum equation gives
nu d^2u_t/ds^2 = dp/dt (the tangential pressure gradient), so b = (1/(2 nu)) dp/dt and the shear error is (d/2) dp/dt.
Integrated round the cylinder, the error of the viscous drag is of the size (d/2)/R times the pressure drag: with d =
0.0049 on 128 x 100 at 30 D (half of the first cell 0.00977) and R = 0.5, about 0.0049 C_D,p, about 0.005 in C_D, and
0.0035 on 181 x 141. Their difference, about 0.0014, is a quarter of the measured difference of the fine pair (0.00615).
The pressure term `p_P = p_w + d dp/ds` is smaller (dp/ds at the wall is a viscous term). So the wall force integral can
plausibly hold the order down; whether it does is what C1 decides. This estimate decides nothing.

## Design

No solver source changes. One new test file, `crates/rcfd-api/tests/f881_cylinder_drag_order.rs` (header: ADR 0771,
F-881), and its mutation patch. The tests of ADR 0761 and ADR 0763 are not edited. No new case text: the test drives
`PimpleSolver` directly, as ADR 0763 does.

### The baseline B0

The set-up of ADR 0763 copied into the new file: `cylinder_ogrid(n_theta, n_r, 0.5, 30, Expansion(150), 0.1)` on 90 x
70, 128 x 100, 181 x 141; D = 1, U = 1, nu = 0.01; `PimpleSettings::piso` with `time_scheme = Bdf2`, `LinearUpwind`,
`n_outer = 2`, `amg_pressure`, `alpha_u = alpha_p = 0.9`; the cylinder a no-slip wall; the far field `Freestream` (the
velocity fixed on the faces with u_ref . n < 0, the pressure 0 on the others); the same initial disturbance and blow-up
guard; dt 0.025 to t = 160; the probe v at (3, 0, 0.05) for t > 60, the force for t > 85; the window W of the last 8
whole periods between upward zero crossings and its halves W1, W2; the mean drag over each; the order by the bisection
of ADR 0763 for the ratios from the cell counts (1.42539, 1.41203). B0 is a new run of ADR 0763's registered set-up
because C1 needs its fields at every step; it also shows whether the record is reproduced (R0).

### The candidates, each changing one thing against B0

- **C1, the wall force integral.** On the fields of B0 at every step (no further solve), the force is taken with
  second-order wall terms in place of the first-order ones. For each face of the patch `cylinder` with owner cell P
  (ring 0) and the cell Q of ring 1 on the same ray (index P + n_theta; that Q lies on the face normal through P is
  asserted to 1e-9 of its distance), with n the unit outward normal (into the cylinder) and s_P = (x_f - x_P) . n, s_Q =
  (x_f - x_Q) . n the distances from the face into the fluid:
  - pressure, linear extrapolation to the face: p_w = p_P - (p_Q - p_P) s_P / (s_Q - s_P), exact for p linear in s;
  - velocity, the derivative at s = 0 of the quadratic through (0, 0), (s_P, u_P), (s_Q, u_Q): du/ds = (u_P s_Q^2 - u_Q
    s_P^2) / (s_P s_Q (s_Q - s_P)), exact for u = a s + b s^2, for each Cartesian component; the outward-normal
    derivative is its negative.
  The code's force is changed by exactly these two terms and nothing else: with Delta the change of the normal
  derivative (second-order less the code's `coeffs.gradient(u_P)`) the viscous force changes by -nu |S| (Delta + (n .
  Delta) n), which is the derivative of `force_and_moment`'s traction `dn + sum_j n_j (g_j + (dn_j - g_j . n) n)` in
  dn; the pressure force by (p_w - p_f) S. C1 is the force with both; C1p (pressure only) and C1v (velocity only) are
  reported. Derived from Taylor series (first principles).
- **C2, the grading of the O-grid.** `Grading::Expansion(50)` in place of `Expansion(150)`, all else as B0 (the same
  n_theta, n_r, far field, step). The ratio 50 is one third of 150: the first radial cell becomes 0.0332, 0.0233, 0.0166
  on the three meshes (from 0.0139, 0.0098, 0.0070) and the growth per cell 1.058, 1.040, 1.028 (from 1.075, 1.052,
  1.036), by the formula of a geometric sequence; the outer cells become smaller (1.66, 1.17, 0.83 from 2.08, 1.47,
  1.04). It tests whether the strong grading, with its coarse outer region, holds the order down. Taken on the
  maintainer's standing instruction of 2026-11-11 (take the recommendation): the factor 3 is chosen as large enough to
  move what the grading does and small enough that the wall layer keeps about three cells in a thickness D/sqrt(Re) =
  0.1 on the coarse mesh.
- **C3, the far-field condition.** The pressure on the far field fixed at 0 on every face (`FixedValue`) in place of
  `Freestream` (0 on the outflow faces, zero gradient on the inflow faces); the velocity condition unchanged; all else as
  B0. It is the one change of the far-field condition that keeps the problem the same in the limit of an infinite
  domain (the free-stream pressure is 0 on both halves) and keeps the outflow velocity free. Taken on the maintainer's
  standing instruction of 2026-11-11 (take the recommendation).

Considered and not run as candidates, with the reason: the time step (ADR 0763 halved it against ADR 0761, with the
meshes and the far field changed too, and its order stayed under 1.5 by every form above); the convection scheme
(`LinearUpwind` is unlimited, and central differences are not stable at a cell Peclet number near 100 in the outer
cells); the definition of h (not a run; the table above is its effect); the options of the pressure-velocity coupling.

Each candidate is measured with both force integrals; C2 and C3 are decided by the standard integral (they change one
thing against B0); their C1 values are reported.

### What decides: the classification of a candidate

p0 is B0's order and pc the candidate's, both by the same procedure on the same cell counts.

- **raises the order into the range** if pc - p0 > 0.1 and pc is in [1.5, 2.5]: the cause is named ("the order is held
  down by this cause");
- **raises the order** if pc - p0 > 0.1 and pc is outside [1.5, 2.5]: named as contributing;
- **lowers the order** if p0 - pc > 0.1: the cause moves the order, but is not shown to hold it down;
- **does not move the order** if |pc - p0| <= 0.1;
- **does not decide** if either sequence is not monotone or has no root, if a run blows up at the guard, if p0 is not
  under 1.5 (R0b: B0 does not show the low order), or if the class on W1 or on W2 differs from the class on W.

The amount **0.1** and the range **[1.5, 2.5]** are the two clauses of the row "Observed order of accuracy" of the
[tolerance table](../process/tolerances.md) ("within 0.5 of the formal order, or at least the formal order less 0.1"):
0.1 is the resolution with which that row reads an observed order. Against the noise of the window it is wide: on ADR
0763's sequence the halves gave the orders 1.388 and 1.387 against 1.387 on W, a spread of 0.001. Against the definition
of h it is not wide (above), and that is why the definition is the same on both sides.

### What the gate runs and what is a heavy record

The three sequences are nine solves, about 855 million cell-steps; ADR 0763's three took 3260 s one after the other.
Here the three meshes of a sequence run in three threads of one process, and the sequences one after the other (the
tests with `--test-threads=1`). Expected about 1.5 to 3 hours beside the other agents; no limit on time is registered.

- **Heavy records** (ignored, run once): `h1_...` (B0 with C1: R0, C1), `h2_...` (C2), `h3_...` (C3), `h4_...` (the
  decision over the three, D). Each sequence is behind its own `OnceLock`; `h2_` to `h4_` read B0 from its lock.
- **Gate** (not ignored, seconds): `g1_...` (the wall terms on a manufactured field), `g2_...` (the procedure on a
  manufactured triple and the record of ADR 0763), `n1_...` (the classification and the decision), `g4_...` (each of
  the three set-ups on 90 x 70, 40 steps through `advance` and the force of both integrals).

## Table of checks

Estimates: R0 from the record; C1 from the estimate of the first-order terms above (a quarter of the fine difference
removed gives an order of about 1.6 to 2.0 if the rest is second order); C2 and C3 not known, guessed near B0. Margin:
for a one-sided limit the limit divided by the estimate; for the amount 0.1, the amount divided by the window noise.

| Id | Check | Limit | Source of the limit | Estimate | Margin | Kind |
| --- | --- | --- | --- | --- | --- | --- |
| R0a | B0 reproduces the record: its three mean drags against 1.32259, 1.33285, 1.33900 (heavy) | each within 1e-5 | row "Agreement with a published value" of `docs/process/tolerances.md` ("the published digits where the same discrete problem is reproduced"); ADR 0763 addendum 1 | 0 (the solver is unchanged) | wide | solution verification |
| R0b | acceptance, "the observed order ... 1.387 on that of ADR 0763": B0 shows the low order (heavy) | order under 1.5 | row "Observed order of accuracy" (the range [1.5, 2.5] it is outside of); the acceptance of F-881 | 1.387 | 1.08 | solution verification |
| C1 | acceptance, "the wall force integral ... tested by a run that changes it alone, with the order it gives": the class of C1 on B0's fields (heavy) | decided (any class but "does not decide"); the class is the result | the acceptance of F-881; amount 0.1 and range [1.5, 2.5] from row "Observed order of accuracy" | raises, 1.6 to 2.0 | 0.1 / 0.001 = 100 | solution verification |
| C2 | acceptance, "the grading of the O-grid": the class of C2 (heavy) | decided; the class is the result | as C1 | does not move, near 1.4 | 100 | solution verification |
| C3 | acceptance, "the far-field condition": the class of C3 (heavy) | decided; the class is the result | as C1 | does not move, near 1.4 | 100 | solution verification |
| K4w | sustained shedding in each of the nine runs | more than 9 upward zero crossings after t = 60 | ADR 0060, K4 (as ADR 0763) | 16 | 1.6 | solution verification |
| D | acceptance, "a cause is named only if its run moves the order by more than a registered amount, and none is named otherwise": the decision over C1, C2, C3 by the function `decide` (heavy) | "named" (at least one raises) or "none moves it upward" (every candidate decided, none raises) | the acceptance of F-881; the brief, decision rule | named: C1 | n/a | solution verification |
| G1 | the wall terms (gate) on 90 x 70 at 30 D with u = (a s + b s^2, a' s + b' s^2, 0), p = c + e s and s = r - r_f (r_f = 0.5 cos(pi / n_theta), the radius of the face centres): for every face the second-order derivative equals -a (outward), the code's equals -(a + b s_P), the extrapolated pressure c and the code's c + e s_P; the cell Q on the normal | relative error at most 1e-9 each | row "Agreement with a closed form" (exact for the discrete problem) | 1e-13 | 1e4 | code verification |
| G2 | the procedure (gate, pure): a manufactured f_0 + C h^p on the cell counts at p = 1.4, 2, 2.5; the five-decimal record of ADR 0763 | relative error at most 1e-9; the record's order within 0.005 of 1.385 (this ADR's arithmetic) | row "Agreement with a closed form"; this ADR, the table above (inputs printed to five decimals) | 1e-13; 1.3849 | wide | code verification |
| N1 | negative controls of the classification and decision (gate, pure): p0 1.387 with pc 1.45, 1.60, 1.495, 1.20, 2.60, NaN; p0 1.55 with pc 1.9; inconsistent halves; the decision on (none moves, none moves, none moves), (raises into range, none, none), (does not decide, none, none) | the classes: does not move; raises into range; raises; lowers; raises; does not decide; does not decide; does not decide; and the decisions: none moves; named C1; not decided | the brief, decision rule; the amount and range of row "Observed order of accuracy" | holds | n/a | code verification (negative control) |
| G4 | the same path as the heavy records (gate): B0, C2 and C3 on 90 x 70, 40 steps of dt 0.025, both integrals | 40 samples each, every value finite, the guard silent | rule 7 of `docs/process/agent-rules.md` ("a fast test on the same code path") | holds | n/a | code verification |
| M1 | committed mutation: the second-order derivative replaced by u_P / s_P | `g1_` fails | rule 16 | fails | n/a | code verification (negative control) |
| M2 | mutation: the amount 0.1 written as 0.0 | `n1_` fails (pc 1.45 then "raises") | the brief, decision rule | fails | n/a | code verification (negative control) |
| M3 | mutation: the sign of the pressure extrapolation flipped | `g1_` fails | rule 16 | fails | n/a | code verification (negative control) |
| R1 | reported per run: C_D (standard, C1, C1p, C1v), its pressure and viscous parts, St, C_L amplitude and rms, crossings, window | none | the brief, task 2 | | n/a | solution verification (reported) |
| R2 | reported per sequence: the order and Richardson value for every integral and on W1, W2; for B0 the orders of the pressure and viscous parts alone | none | the brief, task 2 | | n/a | solution verification (reported) |
| E | `git diff --stat d6e44b27` names this ADR, the new test file, its patch and the proposal file only | no other file | the brief ("Files you own"; "Change no solver source") | holds | n/a | unchanged default output |
| F | `cargo fmt -p rcfd-api --check`; `cargo clippy -p rcfd-api --all-targets -- -D warnings`; `cargo test -p rcfd-api --release --lib`; `cargo test -p rcfd-cli --release --test docs_site` | clean; clean; passes; passes | rule 7 of `docs/process/agent-rules.md` | clean | n/a | code verification |
| P | `python scripts/process_report.py --check` and `python scripts/roadmap_proposals.py check` | no failure of this work | rules 7 and 9 | generated index not current | n/a | code verification |

No row is of kind validation: the item compares the code with itself, not with a measured case.

## Negative control and mutations

N1 runs in the gate. M1 is committed as `crates/rcfd-api/tests/f881_cylinder_drag_order.mutation.patch`, first line
`# expect-fail: cargo test --release -p rcfd-api --test f881_cylinder_drag_order g1_`, made with `git diff` on a scratch
copy. M1, M2 and M3 run on a scratch copy at the last code commit, release, each once, against the gate tests only.

## Size

Size (agent): done; 0; 1; 1; 125000

The heavy run is one process with the four ignored tests (nine solves). The one question is the brief's: whether a fix,
if one is indicated, is issued.

## Decision rule

- **done** if R0b holds and either at least one candidate raises the order (into the range or not: it is named) or every
  one of C1, C2, C3 is decided and none raises it ("no candidate moves it upward", which is a result), and G1, G2, N1, G4
  pass, M1 to M3 are caught, E, F and P hold. Where a candidate is named, the sentence of the brief is written with it.
- **partial** if R0b fails (the run does not show the order it is to explain), if no candidate raises the order and one
  or more does not decide, if a gate check fails, a mutation survives, or a check was not run.
- **todo** if the heavy runs give no number.

R0a is reported against its limit; a miss of R0a alone does not decide, because the order, not the digits, is the thing
explained (said now). A run that misses is not repeated and no limit is changed (rule 2); a heavy test that fails
becomes a `*_known_miss` by an amendment written before the test file changes.

## What is not covered

- Any change of the solver, and so any fix: if C1 is named, the fix is in post-processing (a second-order wall force),
  proposed for a further item, not made here.
- Other gradings (another ratio, wall layers), other far-field forms (a fixed velocity, a convective outflow), a larger
  domain, a fourth mesh, the time step as a run, the convection scheme and the coupling options.
- A mesh-converged drag: C1's Richardson value is reported, not compared with the band; F-869 is done.
- Why the solver's field converges at the order it does, beyond what the three candidates show.

## Amendment 1 (2026-11-22): C3 as registered changes nothing; corrected before any heavy run

Written after the first build and run of the gate tests of `crates/rcfd-api/tests/f881_cylinder_drag_order.rs` (not yet
committed; log `gate_run1.log` in the scratch folder: G1, G2, N1 and G4 passed) and before any heavy run. No limit, amount,
range, mesh, step, window, B0, C1 or C2 changes.

**The defect.** G4 printed, after 40 steps on 90 x 70, the same forces for C3 as for B0 in every variant (1.54758 and so
on). The reason, read in `crates/rcfd-fv/src/pressure.rs` lines 45 to 90 (`pressure_bc`): on every boundary face where
the velocity is fixed, the effective pressure condition of the solver is zero gradient, whatever the pressure field's
own condition says. The faces where C3 fixed the pressure in addition are exactly the inflow faces, where `Freestream`
fixes the velocity; there the fixed pressure is overridden. So C3 as registered is the problem of B0, and its run would
repeat B0. That set-up is defective (rule 2); it was not run as a heavy record.

**The corrected C3, the far-field condition.** The velocity fixed at the free stream (`FixedValue` (1, 0, 0)) on every
far-field face, in place of `Freestream` (fixed on the inflow faces, zero gradient on the outflow faces); the pressure
field's condition is left as in B0, and by `pressure_bc` it is then zero gradient on every far-field face, the level
fixed by the solver's reference cell (`p_ref`, cell 0 at 0; read in `pimple.rs` line 2331 and `pressure.rs` line 427).
A constant in the pressure does not change the force on the closed wall (the area vectors of a closed polygon sum to
zero), and the boundary flux of a uniform velocity through the closed far field is zero, so the pressure problem is
compatible. It changes the outflow half of the far field from a free velocity to a fixed one, the strongest change of
the far-field condition the `Bc` types read allow with the solver unchanged. Everything else as B0. Taken on the
maintainer's standing instruction of 2026-11-11 (take the recommendation). Whether the forced outflow is stable at 30 D
is not known; a blow-up at the guard makes C3 "does not decide" and is not repeated.

**A gate check added (G4b).** For C2 and for the corrected C3, the drag after 40 steps on 90 x 70 differs from B0's by
at least 1e-8 relative: a negative control of the row "Same discrete problem by two routes" of the
[tolerance table](../process/tolerances.md) (`<= 1e-8` when two routes are the same problem), stated the other way round
as that table's last paragraph says. It is the check whose absence let the defect through to the gate run only.

**Deviation recorded now.** The test file was written in the worktree before the registration was committed; it was not
built or run before that commit (ad4b1967), and its first build is the gate run above.

## Addendum 1: results (2026-11-23)

All runs in the release profile, `CARGO_TARGET_DIR` under the scratch folder, beside the solves of other agents. The gate
tests and the heavy record at 63880397 (the test file as committed after amendment 1). The heavy record was one process,
`-- --ignored --nocapture --test-threads=1`, each of the nine solves made once, 8634 s together (B0 about 6150 s while
the machine was most loaded; C2 and C3 about 1250 s each). Logs in the scratch folder: `gate_run1.log` (before amendment
1), `gate_run2.log`, `heavy_record.log`, `m1_run.log`, `m2_run.log`, `m3_run.log`, `lib_run.log`. No limit, amount or
range was changed.

### The three sequences (mean C_D over W; W1 and W2 agree with W to 1e-5 in every row)

| Set-up and integral | 90 x 70 | 128 x 100 | 181 x 141 | Order on W (W1, W2) | Richardson value |
| --- | --- | --- | --- | --- | --- |
| B0, standard | 1.32259 | 1.33285 | 1.33900 | **1.387** (1.388, 1.387) | 1.34901 |
| B0, C1 (both wall terms) | 1.33528 | 1.34223 | 1.34586 | **1.777** (1.778, 1.776) | 1.35014 |
| B0, C1p (pressure term only) | 1.32623 | 1.33589 | 1.34136 | 1.549 | 1.34911 |
| B0, C1v (velocity term only) | 1.33165 | 1.33919 | 1.34349 | 1.525 | 1.34971 |
| B0, pressure part (standard) | 0.98294 | 0.99128 | 0.99646 | 1.286 | 1.00572 |
| B0, viscous part (standard) | 0.33965 | 0.34157 | 0.34255 | 1.873 | 0.34362 |
| C2 (`Expansion(50)`), standard | 1.33667 | 1.33957 | 1.34059 | **2.906** (2.910, 2.901) | 1.34118 |
| C2, C1 | 1.36121 | 1.35946 | 1.35590 | not monotone | |
| C3 (velocity fixed on the far field), standard | 1.32431 | 1.33517 | 1.34187 | **1.305** (1.303, 1.307) | 1.35366 |
| C3, C1 | 1.33701 | 1.34456 | 1.34874 | 1.611 | 1.35437 |

St on the fine mesh: B0 0.16486, C2 0.16509, C3 0.16517; C_L rms 0.2412, 0.2369, 0.2417; crossings 17, 16, 16 in every
set-up. The other reported orders (C2: C1p 3.998, C1v and the pressure part not monotone, the viscous part 3.825; C3:
C1p 1.447, C1v 1.400, the pressure part 1.214, the viscous part 1.733) are in `heavy_record.log`.

### Every check

| Id | Limit | Measured | Met |
| --- | --- | --- | --- |
| R0a | each C_D within 1e-5 of 1.32259, 1.33285, 1.33900 | equal to all printed digits (differences print as 0.00000) | yes |
| R0b | B0's order under 1.5 on W, W1, W2 | 1.3874, 1.3881, 1.3867 | yes |
| C1 | decided | **raises the order into the range**: 1.387 to 1.777 (+0.390), the same class on W1 (1.778) and W2 (1.776) | yes |
| C2 | decided | **raises the order** (beyond the range): 1.387 to 2.906 (+1.519), W1 2.910, W2 2.901 | yes |
| C3 | decided | **does not move the order**: 1.387 to 1.305 (-0.082, within 0.1), W1 1.303, W2 1.307 | yes |
| K4w | more than 9 crossings, all nine runs | 17, 16, 16 in each set-up | yes |
| D | named, or none raises | named: C1 and C2 | yes |
| G1 | relative error at most 1e-9 | all four print as 0.00000 | yes |
| G2 | 1e-9; the record's order within 0.005 of 1.385 | 0.00000; 1.3849 (0.00013 off) | yes |
| N1 | every class and decision as stated | as stated (with one further case: a candidate that lowers gives "none raises") | yes |
| G4 | 40 samples, finite, the guard silent | yes for B0, C2 and C3 | yes |
| G4b | C2 and C3 differ from B0 by at least 1e-8 after 40 steps | 0.00657 and 0.00046 | yes |
| M1 | `g1_` fails | failed: second-order derivative off by 0.16235 relative | caught |
| M2 | `n1_` fails | failed: pc 1.45 gave "Raises" | caught |
| M3 | `g1_` fails | failed: extrapolated pressure off by 0.09045 relative | caught |
| E | only this work's files | `git diff --stat d6e44b27`: this ADR, the test file, its patch, and the proposal file added with this addendum | yes |
| F | fmt, clippy, `--lib`, docs_site | fmt clean; clippy clean; `--lib` 159 passed, 4 ignored; docs_site below | yes |
| P | process check, proposal check | below | see there |

### Reading

The sentence the brief asks for: **the order of the mean drag on the sequence of ADR 0763 is held down by the wall force
integral.** Its two first-order wall terms (the wall-cell pressure taken as the wall pressure, and the wall-normal
derivative `u_P / d`), replaced alone by second-order ones on the same fields, raise the order from 1.387 to 1.777, into
[1.5, 2.5]; each term alone raises it to about 1.53 to 1.55, so both contribute. The difference of the two integrals is
0.01269, 0.00938, 0.00686 on the three meshes, falling by 1.35 and 1.37 per step against the ratios 1.43 and 1.41, that
is at an order near 0.9: the first-order term of the Taylor estimate of the registration, about twice its estimated size.
This does not show the solver's field to be second order: the order with C1, 1.777, is still under 2, and what holds it
there is not found here.

The grading also moves the order (C2), upward past the range, and the reading is that it does so through the same wall
term: with `Expansion(50)` the wall cell is 2.4 times larger, the first-order wall term doubles (C1 less standard 0.0245,
0.0199, 0.0153) and, being of the opposite sign to the remaining error, nearly cancels it in the standard integral (the
differences 0.0029 and 0.0010 give the order 2.9); with the second-order terms the C2 sequence falls with refinement and
is not monotone. So C2 is named as the registered rule names it, as contributing, and its order of 2.9 is a
cancellation, not a cleaner convergence. The far-field condition (C3) moves the order by -0.08, less than the amount; with
C1 its order is 1.611, so the wall term acts the same way there.

A consequence for F-869, reported and not a check: the Richardson value with C1 on B0 is 1.35014 at the order 1.777
(inside the range), against 1.34901 at 1.387 by the standard integral; both are in the band [1.314, 1.370]. The
fine-mesh value by C1 is 1.34586.

No defect of the solver appeared and no solver source was changed. The fix indicated is in post-processing: a second-order
wall force integral, opt-in, beside `wall_force_and_moment`.

### Deviations

- Amendment 1: C3 as first registered was overridden by `pressure_bc` and was corrected before any heavy run; G4b added.
- The test file was written before the registration was committed (built and run only after it; amendment 1 says so).
- M1 was made as `u_p / s_p + 0.0 * (u_q * s_q)` so that the mutated file builds without unused-argument warnings.
- One build of `docs_site` was started without `CARGO_TARGET_DIR`, into the worktree's own `target`; it was stopped, that
  folder (made by it alone) deleted, and the check rerun into the scratch target.
- N1 holds one case more than the registration lists (a lowering candidate gives "none raises").

### What was not run

The whole workspace suite and the other integration tests of `rcfd-api` (no solver file was touched, so not
`baseline_hashes`); the lead's application of the committed patch (checked with `git apply --check`); a fourth mesh,
other gradings or far fields; the cheap sequence of ADR 0761 (not used, see the registration).

### Size

Size (agent, actual): done; 0; 1; 1; 130000

### Decision by the registered rule

**Done.** R0b holds (B0 reproduces the record to every printed digit and its order, 1.387, is under 1.5); C1 raises the
order into the range and is named; C2 raises it beyond the range and is named as contributing, through the size of the
same wall term; C3 does not move it; every gate check passes, M1 to M3 are caught, E and F hold.

## Open questions for the maintainer

1. **Is the fix issued: a second-order wall force integral as an option of `rcfd-post`?** Options: (a) a new item adds an
   opt-in function (the two wall terms of C1, every default bit-identical) with its exactness check and, as acceptance,
   the order of ADR 0763's sequence by it; (b) as (a), and a later registration also makes it the force of the cylinder's
   validation tests; (c) no item: the finding stays a record, and F-869's conclusion, which holds by either integral, is
   unchanged. Recommendation: (a). The first-order wall terms bias the wall force of the code by about 1 % of C_D on these
   meshes; the remedy is small, outside the solver and opt-in, and C1's test code is its reference. Not taken on the
   standing instruction because it adds an item (rule 6).

## For the lead: roadmap entries

Status proposed: done.

> ADR 0771, 2026-11-23, done: the observed order 1.387 of the cylinder drag on the sequence of ADR 0763 is held down by the first-order wall terms of the wall force integral: replaced alone by second-order ones on the same fields the order is 1.777 (in [1.5, 2.5]); the grading Expansion(50) raises it to 2.906 through the same wall term (a cancellation), and the velocity fixed on the far field leaves it at 1.305 (within 0.1); B0 reproduced ADR 0763 to every printed digit; ADR 0761's 1.38 is 1.51 by the same ratio form.

> Follow-up (proposed; the maintainer decides by question 1 of ADR 0771): "Second-order wall force integral as an option of rcfd-post", deps F-881; acceptance: "Every limit not stated here is registered in the ADR before the run. An opt-in wall force in rcfd-post with the pressure extrapolated linearly to the wall and the wall-normal velocity derivative from the wall and the first two cells (ADR 0771, C1), every default bit-identical, exact on a quadratic wall profile to a relative error of at most 1e-9, gives on the mesh sequence of ADR 0763 an observed order of the mean drag in [1.5, 2.5] (1.777 recorded by ADR 0771)."

The same entries are in `docs/roadmap/proposals/0771-F-881.json`.

For the lead, not roadmap: `docs/adr/index.md` is generated and not current after this work; it is left to the lead
(rule 9). No reference fragment was added.

### The process check (row P)

`python scripts/roadmap_proposals.py check docs/roadmap/proposals/0771-F-881.json`: "F-881: todo -> done; note + 522
characters" and "F-888: new, todo, phase 9, deps F-881" (the follow-up), exit 0. `python scripts/process_report.py
--check` on the commit of addendum 1: 2 failures, "generated file is not current" for `docs/adr/index.md` (this ADR)
and for `CHANGELOG.md`; the second fails in the same way on an export of main d6e44b27, so it is not of this work. Both
are left to the lead (rules 7 and 9). `cargo test -p rcfd-cli --release --test docs_site` on the tree with addendum 1: 2
passed.
