# ADR 0268: F-354 registration: immersed solid, DFG 2D-2 force extrema, mesh and time-step convergence

Status: **REGISTERED 2026-11-12**, written before any code of F-354 and before any run of its checks. Not yet accepted by the
maintainer. Results go in an addendum; a defective set-up is corrected by an amendment written and committed before the
corrected run; nothing above an amendment or the addendum is changed after a run.

Parents: [ADR 0249](0249-f346-registration-immersed-force-in-unsteady-flow.md) (F-346: 2D-2 on T and M2, the +2.58 percent
lift maximum on T, `force_moment_unsteady`, its question Q5, from which F-354 was made),
[ADR 0254](0254-f355-registration-immersed-second-order-forcing-in-transient-piso.md) (F-355: `build_second_order_bounded`,
2D-2 on M2 with it, its question Q5), [ADR 0218](0218-f219-registration-immersed-solid-with-modified-forcing.md) (F-219: the
forcing `build`, the DFG channel, the mesh family M1 to M3, the T mesh, the 2D-2 run and its window). Roadmap item F-354
(depends on F-346).

Number: the lead gave 0268. Branch `f354-dfg-convergence` from main 43198740, worktree `E:/projects/cfd/w49i-work/f354`.

House rules (the campaign rules of 2026-11-12): limits are fixed here and not changed after a run; a missed limit becomes an
ignored `*_known_miss` test with the limit intact and the measured number recorded, and is diagnosed, not tuned away; a
defective set-up is corrected by an amendment before the corrected run, the old test staying as an ignored record; each
registered run is made once; parameters are fixed here, not searched for. **This item is a measurement: no library code is
written or changed.** If a library cause is found it is recorded and reported, not repaired here. No case-schema key, hash
table, golden or existing test file is edited; no new third-party crate; no unsafe code; unsigned commits without trailers;
no push.

Clean-room: nothing new is derived about the method; the set-ups are those of ADR 0218, 0249 and 0254, and the refinement
arithmetic below is first principles (the order of a discretisation error under a halving of `h` and `dt`). The reference
values are the FeatFlow level-6 values of DFG 2D-2 recorded in `docs/reference-data.md` (section "F-219 immersed solid with
modified forcing", the row "DFG 2D-2 set-up", read 2026-11-07): max/min/mean `C_D` 3.2200/3.1569/3.1884, max/min `C_L`
0.9859/-1.0206, `St` 0.30188. Nothing is supplied from memory. No vendor code, text, manual or binary was used.

## What F-354 asks

The roadmap entry: "Immersed solid: DFG 2D-2 force extrema, mesh and time-step convergence"; acceptance "DFG 2D-2 with the
immersed cylinder is run on the T mesh and on T refined once (D/80, half the time step) and on M2 and M3; the maximum drag and
lift on the finest mesh are within 1.5 percent of the FeatFlow level-6 values and the error decreases from D/40 to D/80 on both
families, or the part that does not converge (mesh of the wake, forcing order, outlet condition, time step) is identified by a
registered measurement." The note adds the M2 numbers of ADR 0254 with `build_second_order_bounded` and says "the convergence
runs should be made with both forcings" (`build`, the first-order forcing of ADR 0218, and `build_second_order_bounded`;
`build_second_order` is refused by the transient solver on 2D-2, ADR 0254 E2).

The acceptance is taken as binding. Its readings (each a reading, not a decision of the maintainer; the questions Q1 to Q4):

- **"T refined once (D/80, half the time step)"**: every cell of T bisected in x and in y (`T2`, 648 x 328 cells), `dt = 5e-4`
  in place of `1e-3`, the same end time `t = 8` and the same window `[5.5, 8]`.
- **M3 and the time step (Q1).** The acceptance names half the time step for T only. The pair "D/40 to D/80" is read as one
  refinement in space and time on both families: M3 is also run with `dt = 5e-4`, so that the two families are refined alike
  and the Courant number of the D/80 runs is that of the D/40 runs.
- **"the finest mesh" (Q2).** Read per family: T2 and M3 (both D/80 at the cylinder); the limit 1.5 percent is checked on
  each. T2 is the finer of the two (212 544 against 73 216 cells); "done" needs both.
- **"both forcings" (Q3).** The note's request is read as part of the acceptance: every clause (the 1.5 percent on the
  finest meshes and the decrease on both families) is checked with `build` and with `build_second_order_bounded`; "done" needs
  both.
- **"the error"** of a coefficient `X` is `e = (X - X_ref)/abs(X_ref)`; **"decreases"** is `abs(e(D/80)) < abs(e(D/40))`, for
  max `C_D` and for max `C_L` separately, on each family and with each forcing.
- **"identified by a registered measurement"**: the part-by-part measurements P1 to P5 below with the numbers that decide,
  fixed now.

## What was read and done before this text (disclosed)

Read: the F-354 entry of `docs/roadmap.json` (acceptance and note); ADR 0249 in full (its 2D-2 set-up, results, the known
miss and its diagnosis, Q5); ADR 0254 in full (the bounded form, the run A on M2, the refusal E2, the known misses, Q5); of
ADR 0218 the DFG set-up, the mesh family, the T mesh, the 2D-2 run, the results (2D-1 on M1 to M3, 2D-2 on T) and the run
times; `crates/rcfd-fv/tests/immersed_unsteady_force_f346.rs` (the DFG helpers, `run_2d2`, `judge_2d2`, the 2D-2 tests);
`crates/rcfd-fv/tests/immersed_piso_stop_f355.rs` (`build` with the three forcings, `dfg_fields`, `r_solver`, the run A);
`docs/reference-data.md` (the DFG rows); `docs/run-control.md` (the solver's results are bit-identical for any thread count,
measured there; this item uses that to run several records side by side with few threads each).

Run: no code of F-354 exists and no check below was run. One scratch Python program (`f354-scratch/pre.py`, not committed)
counted the cell centres inside the circle of radius 0.05 on a uniform lattice whose lines pass through the centre (1264 at
`h = D/40`, the number ADR 0218 and 0249 report, and **5024 at `h = D/80`**), and solved for the expansion ratio of the
graded part of T (1.0398, last cell 0.0636).

Known numbers (all FeatFlow-relative errors in percent; the D/40 runs used `dt = 1e-3`):

| Run | Source | max `C_D` | max `C_L` | `St` |
| --- | --- | --- | --- | --- |
| T, `build` | ADR 0249 (= ADR 0218) | 3.24624 (+0.815) | 1.01134 (+2.580) | 0.30073 (-0.382) |
| M2, `build` | ADR 0249 | 3.22841 (+0.261) | 0.98432 (-0.161) | 0.30041 (-0.486) |
| M2, bounded | ADR 0254 | 3.20930 (-0.332) | 0.97593 (-1.011) | 0.30112 (-0.250) |
| 2D-1 (steady, Re 20) `build` M2; M3 | ADR 0218 | `C_D` -0.17; -0.02 | `C_L` +2.73; +0.67 | |

The two D/40 meshes have the same spacing and the same staircase at the cylinder (1264 solid cells, 116 first fluid cells) and
differ away from it: T is uniform (`h = 0.0025`) over the whole height and to `x = 0.6`, M2 is uniform only in the core
`[0.12, 0.32] x [0.12, 0.28]` and graded outside it. The **finer** mesh away from the body (T) has the **larger** lift error.
That is the observation to explain: either the M2 agreement is a cancellation of the error at the body by an error of the
coarse outer mesh, or the T mesh has a defect away from the body.

## Arithmetic done beforehand (disclosed, it shaped the set and the expectations)

1. **What one refinement can say.** With `e(h) = C h^p` plus a mesh-independent offset `e_0` (outlet condition, the window),
   halving `h` and `dt` together gives `e(h/2) = e_0 + (e(h) - e_0)/2^p`. The forcing of ADR 0218 is first order at the body
   (ADR 0218 V1: order 1.02 on circular Couette); the bounded form is second order (ADR 0254: 1.947); the convection
   (linear upwind) and BDF2 are second order. The 2D-1 lift on the M family went from +2.73 (M2) to +0.67 percent (M3), a
   factor 4. Prediction for T2 with `build`, if the T error is all discretisation error: max `C_L` +1.3 percent (order 1)
   to +0.65 percent (order 2): **inside 1.5 percent, by a small margin in the first case**; max `C_D` +0.41 to +0.20 percent.
2. **The M family with `build` (a cancellation).** If the T error at D/40 (+2.58) is the error at the body and M2's -0.16 is
   that error plus -2.74 points from the coarser outer mesh, both halve (or quarter) under refinement: M3 between about -0.1
   and +0.7 percent. Whether `abs(e)` decreases from 0.161 percent on the M family is **not predicted**: the D/40 error there is
   smaller than the uncertainty of the estimate. A miss of the decrease on M with both D/80 values inside 1.5 percent is the
   signature of a cancellation at D/40, which the part measurements P3 and P4 below can show.
3. **The bounded form.** On M2 it moves max `C_L` by -1.17 points and max `C_D` by -0.59 points against `build` (ADR 0254). If
   the same shift applies on T (same staircase), T bounded is near +1.4 percent in the lift; its D/80 values are then expected
   inside 1.5 percent. No number is predicted for the decrease on the M family with it (-1.01 percent at D/40).
4. **The time step.** `dt = 1e-3` gives 331 steps per shedding period. BDF2's phase error per period is of order
   `(omega dt)^2/12` relative, with `omega dt = 0.019`: 3e-5; the splitting error of PISO with two correctors is smaller than
   the tolerances (1e-6). Prediction: halving `dt` alone changes max `C_D` and max `C_L` by **less than 0.1 percentage point**.
5. **The outlet.** The DFG set-up has the do-nothing condition at `x = 2.2`; rcfd has a zero-gradient velocity and a fixed
   pressure 0 there (ADR 0218, a known difference). For a nearly developed outflow the two differ by the normal viscous stress
   `nu du_x/dx`, of order `nu U/D = 1e-2` against the dynamic pressure 0.5, at 20 diameters behind the cylinder. rcfd has no
   do-nothing condition, and adding one is library work, which this item does not do. What can be measured without library
   code is the influence of the outlet as a whole on the forces: the same run with the outlet moved from `x = 2.2` to
   `x = 4.4` (the channel continued with the condition unchanged). If the forces do not change when the outlet goes twice as
   far, a condition at `x = 2.2` that differs from it by a stress of 2 percent of the dynamic pressure cannot carry a
   percentage-point error. Prediction: the extension changes max `C_D` and max `C_L` by less than 0.1 point.
6. **Separating the mesh at the cylinder from the mesh of the wake on the T family.** T is a tensor mesh; refining T only in
   the core `[0.12, 0.32] x [0.12, 0.28]` (bisecting the T intervals that lie in it; the lines 0.12, 0.32 and 0.28 are lines of
   T) gives `Tc`: D/80 over the cylinder and one diameter around it, D/40 elsewhere, with a jump of the spacing by 2 at the core
   edges (disclosed as a limit; M3 has the same core but graded transitions). Then, at `dt = 5e-4`, the chain T -> Tc -> T2
   changes first the core (and with it the staircase of the forcing: 5024 solid cells) and then everything outside it. With
   the change from `dt = 1e-3` to `5e-4` on T as the first link, the total change from T to T2 is the exact sum of a time
   part, a core part and an outer part.

## Cost of every run (estimated now)

The per-cell-step times measured before: T 2234 s for 53 136 cells x 8000 steps (5.3 microseconds, ADR 0249, with other jobs
on the machine; ADR 0218: 4.1); M2 1133 s for 18 304 cells x 8000 steps (7.7 microseconds). For the D/80 meshes 1.3 times
the T rate is assumed (more pressure iterations); for Tc 1.15 times. The machine is shared with a test gate and other agents
(load 99 percent when this was written, 8 cores, 16 threads, 24 GB free); the records run side by side, one process each,
with `RAYON_NUM_THREADS` 1 or 2 (bit-identical results for any count, `docs/run-control.md`). Wall times can be 2 to 3
times the single-run estimates.

| Run | Mesh (cells) | Forcing | `dt`, steps | Estimate | Needed for |
| --- | --- | --- | --- | --- | --- |
| R0 | M2 (18 304) | `build` | 1e-3, 8000 | 0.3 h | C0 (the copied set-up reproduces ADR 0249 on M2) |
| — | T (53 136) | `build` | 1e-3, 8000 | not run: ADR 0249 | D/40 of the T family, `build` |
| — | M2 | bounded | 1e-3, 8000 | not run: ADR 0254 | D/40 of the M family, bounded |
| R1 | T | bounded | 1e-3, 8000 | 0.65 h | D/40 of the T family, bounded |
| R2 | T2 (212 544) | `build` | 5e-4, 16000 | 6.5 h | A1, A2 (T, `build`); P2, P3 |
| R3 | T2 | bounded | 5e-4, 16000 | 6.5 h | A1, A2 (T, bounded); P4 |
| R4 | M3 (73 216) | `build` | 5e-4, 16000 | 2.2 h | A1, A2 (M, `build`) |
| R5 | M3 | bounded | 5e-4, 16000 | 2.2 h | A1, A2 (M, bounded); P4 |
| R6 | M2 | `build` | 5e-4, 16000 | 0.65 h | P1 (time step, M family) |
| R7 | T | `build` | 5e-4, 16000 | 1.25 h | P1 (time step, T family); P2 |
| R8 | Tc (92 112) | `build` | 5e-4, 16000 | 2.5 h | P2, P3 (core and outer mesh) |
| R9 | Te (58 876: T continued to `x = 4.4`) | `build` | 1e-3, 8000 | 0.7 h | P5 (outlet) |

Sum about 24 single-process hours. Reused, not run again: the T and M2 runs with `build` (ADR 0249) and the M2 run with the
bounded form (ADR 0254). The solver and both forcings are deterministic and bit-identical for any thread count; the
library has not changed on the path of `build` since ADR 0249 (ADR 0254 N1 and `baseline_hashes`), and R0 re-measures M2
with the copied helpers as the check of that reuse. The bounded form exists only since ADR 0254, whose M2 run is reused as
is. Each record gets a time budget of 48 h (no timeout in the test; the run is not killed for being slow).

Left out, with the reason: the time step on the T family with the bounded form and on the M family with it (the time step is
a property of the time scheme and the flow, P1 with `build` on both families measures it); Tc and Te with the bounded form
(the core/outer split and the outlet are measured once, with the forcing of ADR 0218 whose T miss is the subject); a level
beyond D/80 (an order needs three levels; the acceptance asks for one refinement); M1 (D/20). If time does not allow every
record, the order of the runs is R0, R1, R6, R7, R9 first, then R4, R5, R8, then R2, R3 (all are started at once; this is
the order in which their results are expected), and any record not finished is reported as not run with the clause it
leaves open.

## Set-ups

**2D-2 (the run of ADR 0218 and 0249, unchanged except the mesh and the time step).** The DFG channel `[0, 2.2] x [0, 0.41]`,
`dz = 0.01`, cylinder centre `(0.2, 0.2)`, radius 0.05, `nu = 1e-3`, `rho = 1`, `U_m = 1.5`, inlet the face averages of the
parabola, walls velocity zero, outlet velocity zero gradient and pressure 0; `K = 1e7`, `theta_min = 1e-2`
(`ImmersedSettings::new`); `PimpleSettings::piso(1e-3)` with BDF2, `LinearUpwind`, AMG pressure, two correctors, `tol_rel`
1e-6 for momentum and pressure; the start field of ADR 0218 (the inflow profile with the transverse blob); `C_D, C_L =
F_x,y/(0.5 U_bar^2 D dz)`, `U_bar = 1`, `F` from `force_moment` (for a body at rest it equals `force_moment_unsteady` to 2e-6,
ADR 0249 D1p); the window `t in [5.5, 8]`: the upward crossings of `C_L` minus its window mean, the Strouhal number from
them, the extrema over the whole periods between the first and the last crossing (the `window` of ADR 0249, copied). Steps:
`8/dt` (8000 or 16000). Forcings: `build` (ADR 0218, `modified = true`) and `build_second_order_bounded` (ADR 0254).

**Meshes.** M2, M3: `dfg_family(2)`, `dfg_family(3)` of ADR 0218 (copied). T: ADR 0218 (copied). T2: the coordinates of T,
each interval bisected (x and y). Tc: the coordinates of T with the intervals inside `[0.12, 0.32]` (x) and `[0.12, 0.28]` (y)
bisected: 404 x 228 cells. Te: the x coordinates of T continued from 2.2 to 4.4 by 35 uniform cells (0.062857, the size of the
last graded cell of T rounded to fit), 359 x 164 cells; the outlet patch `xmax` at `x = 4.4`.

All in the new file `crates/rcfd-fv/tests/immersed_dfg_convergence_f354.rs`; the helpers are copied from
`immersed_unsteady_force_f346.rs` and `immersed_piso_stop_f355.rs` (those files are not edited). Each record R0 to R9 is one
`#[ignore]` test (a heavy record, run once in release with `--ignored`), printing every number it uses and writing its line
of results; the checks that compare two runs are made in the addendum from the printed numbers of the records (the records
are separate processes), and are repeated as assertions in one more ignored test that reads nothing: it holds the measured
numbers as constants and asserts the registered limits on them (added with the addendum, so that the verdicts are in the
code with their limits; a missed limit is kept there as a `*_known_miss` test).

## Registered checks

"Point" is a percentage point of the FeatFlow-relative error. `e_X(run)` is the error of coefficient `X` (max `C_D` or max
`C_L`) of a run.

### The acceptance

| Id | Check | Limit | Estimate behind it |
| --- | --- | --- | --- |
| A1-T-1 | T2, `build`: `abs(e)` of max `C_D`; of max `C_L` | `<= 1.5 percent` each | item 1: +0.2 to +0.4; +0.65 to +1.3 |
| A1-M-1 | M3, `build`: the same | the same | item 2: within about +-0.7 each |
| A1-T-2 | T2, bounded: the same | the same | item 3: inside |
| A1-M-2 | M3, bounded: the same | the same | item 3: inside |
| A2-T-1 | T family, `build`: `abs(e(T2)) < abs(e(T))`, max `C_D`; max `C_L` | strict | item 1: expected met (0.815 and 2.580 at D/40) |
| A2-M-1 | M family, `build`: `abs(e(M3)) < abs(e(M2))`, max `C_D`; max `C_L` | strict | item 2: **not predicted** (0.261 and 0.161 at D/40) |
| A2-T-2 | T family, bounded: `abs(e(T2)) < abs(e(T))` | strict | no number for T bounded exists |
| A2-M-2 | M family, bounded: `abs(e(M3)) < abs(e(M2))` | strict | not predicted (0.332 and 1.011 at D/40) |

### The part measurements (the second branch of the acceptance)

For a part, `d` is the change of `e_X` in points that the part alone causes. A part is **ruled out** for coefficient `X` if
`abs(d) <= 0.3` point; it is **identified** (it alone moves the extremum by more than the margin by which T misses the
limit, 2.58 - 1.5 = 1.08, rounded down) if `abs(d) >= 1.0` point; between the two it **contributes** and decides nothing.
The 0.3 point is a fifth of the acceptance limit and an eighth of the T miss; items 4 and 5 predict below 0.1 for the time
step and the outlet.

| Id | Part | `d` (both coefficients, `build` unless named) | Estimate |
| --- | --- | --- | --- |
| P1 | the time step | `e(R7) - e(T)` (T family) and `e(R6) - e(M2)` (M family) | item 4: below 0.1 point: ruled out |
| P2 | the mesh at the cylinder (with the staircase of the forcing) | `e(R8) - e(R7)` (T to Tc at `dt = 5e-4`) | item 1: of the size of the T2 change |
| P3 | the mesh of the wake and of the channel away from the cylinder | `e(R2) - e(R8)` (Tc to T2 at `dt = 5e-4`); and the family difference `e(T) - e(M2)` at D/40 (known: +0.55 and +2.74 points) against `e(T2) - e(M3)` at D/80 | item 2: the family difference shrinks if the M2 agreement is a cancellation |
| P4 | the forcing order | `e(bounded) - e(build)` on T (R1 against ADR 0249), T2 (R3 against R2), M2 (known: -0.59 and -1.17 points) and M3 (R5 against R4) | item 3 |
| P5 | the outlet condition | `e(R9) - e(T)` (outlet at 4.4 against 2.2, `dt = 1e-3`) | item 5: below 0.1 point: ruled out |

The family difference of P3 is **identified** as a discretisation error of the outer mesh (the M2 agreement a
cancellation) if `abs(e(T) - e(M2)) >= 1.0` point (known: 2.74 in the lift, so this half is disclosed as already true) and
`abs(e(T2) - e(M3)) <= abs(e(T) - e(M2))/2` in the same coefficient; it is **ruled out** as converging if
`abs(e(T2) - e(M3)) > 0.75 abs(e(T) - e(M2))` (it does not shrink with the mesh: a defect of one of the two meshes); between,
it contributes.

**The second branch is reached** for a missed clause of the acceptance (one coefficient, one family, one forcing) if at least
one part is identified for that coefficient by its number, and P1 and P5 are each either ruled out or identified for it (the
time step and the outlet settled). The addendum names the part.

### Controls and set-up

| Id | Check | Limit | Estimate behind it |
| --- | --- | --- | --- |
| C0 | R0 (M2, `build`, the copied helpers): max `C_D`, max `C_L`, `St` | 3.22841, 0.98432, 0.30041 to the digits given (half a unit of the last digit) | the same code and set-up as ADR 0249, deterministic |
| S2 (each of R0 to R9) | the window of ADR 0218 on `C_L` | at least 6 upward crossings; every period within 1 percent of their mean; the maximum of `C_L` of every period within 2 percent of their mean; `max C_L - min C_L >= 1.0`; no stop | ADR 0249 and 0254: 7 crossings, 1e-5, 3e-4, about 2.0 on every D/40 run |
| S3 (fast) | the meshes: cell counts (T 53 136, T2 212 544, Tc 92 112, Te 58 876, M2 18 304, M3 73 216); the spacing of every cell whose centre lies within `0.06` of the cylinder centre (`0.0025` on T, Te, M2; `0.00125` on T2, Tc, M3, to 1e-12); the solid cells of both forcings (1264 on the D/40 meshes, 5024 on the D/80 meshes) mirror-symmetric about `x = 0.2` and `y = 0.2`; the forcing of the bounded form has at least 10 faces with an image and
`build` none (ADR 0254 UB3: 32 of 160 on M2); the coordinates of T2 and Tc contain every coordinate of T; the outlet of Te at `x = 4.4` | as stated | `pre.py`; ADR 0218 S3 |
| S4 (fast) | the run table: R2 to R8 have `dt = 5e-4` and 16000 steps where named; R0, R1, R9 `dt = 1e-3` and 8000 steps; each mesh and forcing as in the cost table | equal | this registration |
| S5 (fast) | the window on a synthetic series `C_L = 0.25 + sin(2 pi 3 t)`, `C_D = 3 + 0.1 cos(2 pi 6 t)`, sampled at `dt = 1e-3` to `t = 8`: `St` (`D/U_bar = 0.1`: 0.3), max `C_L`, max `C_D`, period and maximum spreads | `abs(St - 0.3) <= 1e-6`; max `C_L` within 1e-4 of 1.25; max `C_D` within 1e-4 of 3.1; spreads `<= 1e-4` | linear interpolation of a sine's crossings is exact to `O(dt^2)`; the sampled maximum misses the peak by at most `1 - cos(pi f dt)` = 4.4e-5 |
| S6 (fast) | the judgement functions on given numbers: within 1.5 percent (3.2200 x 1.0149 met, x 1.0151 missed), the strict decrease (0.5 against 0.6 met, 0.6 against 0.6 missed, -0.7 against 0.6 missed), the part verdicts (0.29 ruled out, 0.31 contributes, 1.0 identified, -1.2 identified) | as stated | the definitions above |
| S7 (fast) | the run helper: 3 steps of 2D-2 on M2 with each forcing | no stop; `C_D` and `C_L` finite | the path of the records |

The fast tests S3 to S7 are what the gate runs (release; S3 builds both forcings on six meshes, S7 makes six steps on 18 304
cells: a few seconds in release).

### Printed, no limit

For every run: max, min and mean `C_D`, max and min `C_L`, `St` with their errors; the run time; the number of solid and
first fluid cells; for the bounded form the number of image faces and fall-back faces. The observed order
`log2(abs(e(D/40))/abs(e(D/80)))` where both errors have the same sign (an order from two levels assumes `e_0 = 0`; printed
only). The Strouhal numbers of all runs against 0.30188.

### Negative controls and mutations (scratch, not committed)

The control C0 (the reuse of the D/40 runs). Negative controls inside the fast tests: S6's cases that must miss. Mutations,
each applied alone to the test file by an exact text replacement (checked to occur once), the fast tests run once in
release, the file restored and compared byte for byte with a kept copy:

| Id | Mutation | Must fail |
| --- | --- | --- |
| MU1 | T2 bisected in x only | S3 |
| MU2 | Tc bisected over `[0.12, 0.30]` in x | S3 |
| MU3 | the D/80 runs with `dt = 1e-3` in the run table | S4 |
| MU4 | the decrease judged with `<=` in place of `<` | S6 |
| MU5 | the limit 1.5 percent written as 0.015 percent (`1.5e-4`) | S6 |
| MU6 | the window's Strouhal number from `n` crossings in place of `n - 1` intervals | S5 |
| MU7 | the bounded forcing replaced by `build` in the run helper | S3 (the forcings of the meshes: image faces counted) |

The records R0 to R9 are not run under the mutations.

## Decision rule

- **done**: C0 and S2 to S7 held; every record R0 to R9 made; and for each of the four pairs (family, forcing) and each of
  the two coefficients, either A1 and A2 met, or the second branch reached for each missed clause; every mutation failed a
  test it must fail.
- **partial**: C0 and the fast checks held and the records made, but a clause neither met nor accounted for by the second
  branch (a known miss with its limit, diagnosed as far as the part numbers go); or one or more records not made (the
  clauses they decide are then reported as not run).
- **todo** (unchanged): C0 missed and not repaired by an amendment (the reuse of the D/40 runs is then not valid), or the
  D/80 records with `build` (R2, R4) not made.

A set-up check that does not hold (C0, S2 of a record, S3 to S7) is a defective set-up: an amendment before any corrected run,
never counted as a pass. A record that stops (the divergence guard) is a measured result of that configuration (recorded with
its step and quantity, S2 not held): the clauses it decides are missed for that forcing and the stop is reported as a found
library behaviour, not repaired here.

By the arithmetic above the expected outcome is: A1 met on T2 and M3 with `build` (T2's lift by a small margin), A2 met on
the T family, A2 on the M family not predicted, P1 and P5 ruled out, and P3 showing the M2 agreement as a cancellation. That
expectation is written here so that the result cannot be read as a surprise explained afterwards.

## Limits (not covered)

One refinement level (no order from three levels, no Richardson estimate); one Reynolds number; two dimensions; a stationary
body. The outlet is measured by its distance, not by a do-nothing condition (none exists in rcfd; adding one is library
work). Tc has a jump of the spacing by 2 at the edges of its core. The time step on the bounded form, and Tc and Te with it,
are not run. `build_second_order` is not run (refused, ADR 0254). The window `[5.5, 8]` is that of ADR 0218 at every level;
the start transient at D/80 is not examined beyond S2. The FeatFlow values are themselves the level-6 values of a finite
element computation with `k = 1/200`, not an exact solution; their own uncertainty is not in the repository and is not
used.

## Open questions

- **Q1. The time step of M3.** Options: (a) `dt = 5e-4` on M3 as on T2 (one refinement in space and time on both families);
  (b) `dt = 1e-3` on M3 (the text names half the step for T only). Taken on the maintainer's standing instruction of
  2026-11-11 (take the recommendation): (a); P1 measures the time step alone on M2.
- **Q2. "The finest mesh".** Options: (a) each family's D/80 mesh (T2 and M3); (b) T2 alone. Taken on the standing
  instruction: (a).
- **Q3. Both forcings in the acceptance.** Options: (a) every clause with both forcings ("done" needs both); (b) the clauses
  with `build`, the bounded form printed. Taken on the standing instruction: (a), as the note asks.
- **Q4. The outlet as a variable.** Options: (a) its distance (Te, no library code); (b) a do-nothing condition (library
  work, outside this item). Taken on the standing instruction: (a); (b) is a possible follow-up if P5 is not ruled out.
