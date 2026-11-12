# ADR 0245: F-344 registration: the step-size-dependent remainder of the library's BDF2 step

Status: **REGISTERED 2026-11-12**, written before any code of F-344 and before any run of its checks. Not yet accepted by the
maintainer. Before this text no check was run: the documents and the code listed under "What was read" were read, the
arithmetic under "Arithmetic before this registration" was done by hand, and the unchanged tree of main was compiled once
(`cargo test --release -p rcfd-fv --test overset_f338 --no-run`, to fill the target directory of the new worktree; nothing
was executed). Results go in the addendum at the end; nothing above the addendum is changed after a run.

Parents: [ADR 0241](0241-f338-registration-overset-bdf2.md) (F-338: hypothesis H16, the set-ups SMc and TG_2, `d_200`,
question 1, whose recommendation is this item), [ADR 0232](0232-f314-registration-overset-observed-order-miss.md) (SMc),
[ADR 0238](0238-f335-registration-overset-outer-iterations.md) (outer iterations on the overset path),
[ADR 0009](0009-transient-variable-density.md) (the transient Rhie-Chow term of the PISO step),
[ADR 0043](0043-wave25a-temporal-accuracy.md) and [ADR 0069](0069-wave33b-transient-accuracy-coupling.md) (BDF2 in the
step, its measured order), [ADR 0073](0073-wave34b-transient-floor-pimple-lag.md) and
[ADR 0076](0076-wave35a-transient-seams-and-relaxation.md) (the order floor of the BDF2 step, hypothesis H-COMM, the
fixes F-C and F-G and their removal). Roadmap item F-344 (depends on F-338).

Number: the lead gave 0245 to F-344. Branch `f344-bdf2-remainder` from main 989be08d, worktree `w49i-work/f344`.

House rules (ADR 0130, ADR 0213, ADR 0232, ADR 0241):

- limits and deciding numbers are fixed here, before the code and the runs, and are not changed after a run;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact and the number recorded, diagnosed, not
  tuned away; a defective set-up gets a corrected test by an amendment committed before the corrected run, the old one
  stays as an ignored record;
- each registered run is made once (a deterministic run that two tests share is the same run);
- defaults stay bit-identical; `crates/rcfd-api/tests/baseline_hashes.rs`, the hash tables and the goldens are not edited
  or re-recorded; no existing test or known-miss record is edited;
- a corrective is opt-in and is registered in an amendment, with its derivation, before its code and its runs;
- no new third-party crate, no unsafe code, no commit trailers, no push.

Clean-room. Everything used is derived in this text or in the ADRs of this repository cited above. Names of the ideas,
cited from memory (no paper, book, code or vendor document was opened for this work): momentum interpolation on
collocated meshes (C. M. Rhie and W. L. Chow, AIAA J. 21 (1983) 1525-1532); its dependence on the time step and the
transient form that removes it for a uniform coefficient (S. Majumdar, Numer. Heat Transfer 13 (1988) 125-132; S. K.
Choi, Numer. Heat Transfer A 36 (1999) 545-550; B. Yu, W.-Q. Tao and others, Numer. Heat Transfer B 42 (2002) 141-166);
face coefficients built so that the converged face velocity does not depend on the time step (A. Cubero and N. Fueyo,
Numer. Heat Transfer B 52 (2007) 507-529; A. Pascau, Int. J. Numer. Methods Fluids 65 (2011) 812-833); PISO (R. I.
Issa, J. Comput. Phys. 62 (1986) 40-65); the three-level backward difference (Hairer, Nørsett, Wanner). The citations
name where such schemes are discussed; no formula is taken from them unverified: the formulas are derived below.

## What F-344 asks

The roadmap entry (acceptance, quoted, binding): "The remainder of PimpleSolver::step with BDF2 and three outer iterations
on a mesh at rest (max abs(d_200) 5.13e-6 with the ratio 2.30 to max abs(d_400) on the mesh of SMc, ADR 0241 H16) is
traced to a named term of the step with a number, and either removed as an opt-in (defaults bit-identical; the ratio at
least 4 at rest and D2 of ADR 0241 at least 4 on TG_2) or shown to be a spatial term with its order in h, or the ADR
names why not with a number." Note: "From ADR 0241 question 1 ... The library's own BDF2 step on a mesh at rest converges
in the step size at a ratio of 2.30 where second order gives 4. Candidates not measured: the transient Rhie-Chow term,
the boundary faces."

**Reading (registered here).**

1. "The mesh of SMc at rest" is the set-up of ADR 0241 H16 (`sm_rest_library` of `overset_f338.rs`): the 40 by 40 mesh of
   the square `[-0.6, 0.6]^2` translated by `tg_pose(0)` (no rotation at `t = 0`), `h = 0.03`, the exact Taylor-Green
   velocity less its net flux fixed on the four sides at the new time of each step, `nu = 0.1`, the settings PS of ADR
   0219 (central convection, two correctors, no relaxation, linear solves to `1e-12` relative), `PimpleSolver::step` with
   BDF2 and three outer iterations, `N` equal steps to `t = 0.5`. Called **R** below; `R_n` is R with `n` by `n` cells.
2. `d_N` is the difference of the cell velocities of the runs in `N` and in 800 steps, over the decay `e^{-2 nu t}`; the
   ratio is `max abs(d_200) / max abs(d_400)`. Second order gives `(16 - 1) / (4 - 1) = 5`, first order 3; the limit of
   the acceptance is 4. A ratio of `2.30` is below first order: with an error `A dt^q` the ratio is `2^q + 1`, so `q =
   0.38`. That is not what a term of first order gives; it is what a term gives that saturates in the step size.
3. "Traced to a named term with a number": a term of the equations the converged step solves, written as a formula, and
   a measurement in which removing that term alone removes the remainder (or a measurement of the term itself that
   accounts for it), with the deciding number registered before the run.
4. The three ways to meet the item are those of the acceptance. The first (removal as an opt-in) needs both ratios; if
   the corrective reaches 4 at rest and not on TG_2, that is a miss of the first way, reported as such.

## What was read (no run)

- ADR 0241 in full; ADR 0232 and ADR 0238 (the set-ups); ADR 0009 (the transient Rhie-Chow term); ADR 0069 (the parts on
  the measured BDF2 order and the X-matrix); ADR 0073 (R1 to R4, the fixes F-C and F-G, amendments 1 to 3 and the final
  table); ADR 0076 and ADR 0080 (the lines on F-G: measured in the bundle K, not accepted, removed from the code); the
  roadmap entries F-338 and F-344.
- `crates/rcfd-fv/src/pimple.rs`: `step_with` in full (lines 963 to 1576), the settings, `ddt_history_weights`, the unit
  tests on the Taylor-Green box; `crates/rcfd-fv/src/ddt.rs` (`History`, `add_ddt`); `crates/rcfd-fv/src/pressure.rs`
  (`pressure_bc`, `boundary_hby_a`, `face_interpolate`); `crates/rcfd-fv/src/convection.rs` (`face_flux`);
  `crates/rcfd-fv/src/overset.rs` lines 930 to 1000 (the face coefficient and the flux of the overset step).
- `crates/rcfd-fv/tests/overset_f338.rs`: the settings, `sm_conditions`, `sm_run_steps`, `sm_rest_library`, the
  diagnosis test of amendment 1; the head of `w34b_floor.rs`.

What the code read shows (`a_P` is the mean over the active components of the momentum diagonal, `rAU = V / a_P`,
`gamma = interp(rAU)` by the linear face weights, `G p` the Green-Gauss cell gradient, `snGrad p` the two-point normal
gradient):

1. The step, per corrector: `HbyA = H(u) / a_P` with the old levels of the time term in `H`; `phiHbyA = interp(HbyA) . S
   + (gamma / dt) [c1 (phi^n - interp(u^n) . S) - c2 (phi^{n-1} - interp(u^{n-1}) . S)]` on internal faces (the
   transient Rhie-Chow carry-over; boundary faces take the prescribed velocity); the pressure equation with the face
   coefficient `gamma`; `phi = phiHbyA - gamma snGrad p abs(S)`; `u = HbyA - rAU G p`. With the outer loop converged this
   is the fully implicit BDF2 momentum balance of the cells with the convecting flux `phi`, and `sum phi = 0`.
2. A fixed-velocity boundary gives the pressure a zero normal gradient (`pressure_bc`), so the Green-Gauss gradient of a
   boundary cell takes the cell value at the boundary face: its normal component is half the two-point gradient to the
   next cell. The momentum diagonal of a boundary cell is larger than that of an inner cell (the boundary face's
   half-distance: on the uniform mesh `5 nu / h^2` on a side, `6 nu / h^2` in a corner, `4 nu / h^2` inside, plus the
   upwind part of the convection, which varies from cell to cell).
3. **This term has been found before.** ADR 0073 (R2, amendment 1, verdict 1) names it: the face defect `d = phi -
   interp(u) . S` of the converged step obeys `d^{n+1} = (gamma / dt)(c1 d^n - c2 d^{n-1}) + gamma R(p) + C(G p)`, with
   `R(p) = interp(G p) . S - snGrad p abs(S)` and the commutator `C(x) = interp(rAU x) . S - interp(rAU) interp(x) . S`,
   which for linear weights is `w (1 - w)(rAU_N - rAU_P)(x_N - x_P) . S`. With `rAU = dt / (c0 + a dt)` (`a` the
   non-temporal diagonal per volume) `C(G p)` is of first order in `dt` for `a dt` small and saturates once `a dt`
   exceeds `c0`. ADR 0073 measured it on the periodic Taylor-Green box (its Y1, Y2 and the opt-in F-G, which subtracted
   `C(G p)` and brought the order to 2.0 to 2.3 there). F-G was not accepted by the rule of ADR 0073 (one no-harm
   comparison on a periodic seam at 1.0504 against 1.05; the under-relaxed PIMPLE row) and was removed from the code with
   the bundle K of ADR 0076 (misses on periodic and rotational seams). ADR 0241 did not know of it when it listed "the
   transient Rhie-Chow term" as a candidate not measured. So the leading hypothesis of this item is not new; what is new
   is the set-up (fixed-velocity boundaries, where the diagonal and the pressure gradient both jump by a factor of order
   one between the boundary cell and its neighbour) and the request for an opt-in.

## Arithmetic before this registration (disclosed, by hand)

- On R: `a = 4 nu / h^2 = 444`; `a dt = 1.11, 0.56, 0.28` in 200, 400 and 800 steps. The time term does not dominate the
  diagonal at these steps: the step sizes are in the range where a function of `a dt / (c0 + a dt)` bends.
- The steady face defect. With `w = 1 / (c0 + a dt)` per cell (`rAU = dt w`) and `c0 = c1 - c2`, a defect that no longer
  changes satisfies `d (1 - c0 interp(w)) = dt [interp(w G p) . S - interp(w) snGrad p abs(S)]`, and `1 - c0 interp(w) =
  dt interp(a w)`, so

  `d = [interp(w G p) . S - interp(w) snGrad p abs(S)] / interp(a w)`.

  If `a` is the same in both cells, `w` cancels: `d = R(p) / a`, independent of `dt` (that is what the carry-over is for).
  If it is not, `d` depends on `dt` through the weights `w`.
- A face between a side cell (1: `a_1 = 5`, normal gradient `snGrad / 2`, item 2 above) and its inner neighbour (2: `a_2 =
  4`, gradient about `snGrad`), in units of `nu / h^2`: `d = -0.5 snGrad / (a_1 + a_2 w_2 / w_1)`, and `w_2 / w_1 = (c0 +
  a_1 dt) / (c0 + a_2 dt)` runs from 1 (`dt -> 0`) to 1.25 (`dt -> infinity`): the defect of such a face changes by 10
  percent over the range of `dt`, in a quantity of size `snGrad p h^2 / (18 nu)`, which is `5e-4 snGrad p` here. The
  remainder to explain is `5e-6`. The size is ample; no closer estimate was made.
- The shape. `1 / (c0 + a_1 dt) - 1 / (c0 + a_2 dt)` at `a_2 dt = 1.11, 0.555, 0.278`: `0.0368, 0.0308, 0.0212`; the ratio
  of the differences to the last is `1.63`. A term of this kind gives a ratio well below 3, as measured (`2.30`); a clean
  first-order term gives 3, a second-order one 5.
- With `C(G p)` subtracted and `gamma = interp(rAU)` kept (F-G of ADR 0073) the steady defect is `interp(w) R(p) / interp(a
  w)`; for `a_1 = 5`, `a_2 = 4` the factor runs from `0.2222` to `0.2250`, 1.2 percent. With the face coefficient taken
  from the face mean of `a`, `Gamma_f = dt / (c0 + a_f dt)` with `a_f = interp(a)` (the harmonic face value of `rAU`), the
  steady defect is `R(p) / a_f`, independent of `dt` exactly. This is noted for the corrective; its design is registered
  in an amendment after the diagnosis and before its code.
- Cost: ADR 0241 ran twelve runs of the single mesh in about two minutes. The diagnosis below is about forty runs, the
  largest 80 by 80 cells in 3200 steps; about fifteen minutes.

## Hypotheses

Each with the measurement that confirms or rules it out and the deciding number. Between the two numbers a hypothesis is
undecided. The letters are the candidates of the lead's brief.

| Id | Hypothesis | Measurement (set-up R unless stated) | Deciding number |
| --- | --- | --- | --- |
| H1 (a) | the ratio `2.30` is an effect of the measure: the differences to one reference mix a second-order part with a part that vanishes faster, and the step is of second order between the finer pairs | runs in 100, 200, 400, 800 and 1600 steps; `delta_N = max abs(u_N - u_2N)` over the decay; `q_N = log2(delta_N / delta_2N)` for `N = 100, 200, 400` (the order between consecutive pairs, free of the choice of a reference) | confirmed: `q_400 >= 1.8`; ruled out: `q_400 <= 1.3` |
| H2 (c) | the remainder is left by the first step (implicit Euler) | every run takes 160 steps of `0.000625` to `t = 0.1`, then 160, 320 and 640 steps to `t = 0.5`: `R_start = max abs(d'_160) / max abs(d'_320)` (differences to the run with 640) | confirmed: `R_start >= 4`; ruled out: `R_start < 3.5` |
| H3 (f, g) | the remainder is what three outer iterations and two correctors leave (the extrapolated first iteration, the deferred correction of the central scheme; the mesh is orthogonal, so there is no non-orthogonal correction) | 200 steps with six outer iterations, and with four correctors: `e_out = max abs(u_3 - u_6)`, `e_corr = max abs(u_2c - u_4c)`, over the decay | confirmed: either `>= 1e-6`; ruled out: both `<= 1e-7` (2 percent of the remainder) |
| H4 (d) | the velocity is of second order and only the pressure is not | the ratio of the velocity is known (`2.30`), so the hypothesis as stated is ruled out by ADR 0241; measured for the record: `q_N` of the pressure (the mean removed) | no decision; printed |
| H5 (b) | the remainder enters through the face flux, not through the cell balance: the face defect `d = phi - interp(u) . S` of the converged step depends on the step size | from the runs of H1: `D_N = max over internal faces of abs(d_N - d_800) / abs(S)` (a velocity), against `max abs(d_200)` of the velocity | confirmed: `D_200 >= 0.5 max abs(d_200)`; ruled out: `D_200 <= 0.1 max abs(d_200)` |
| H6 (e) | it is made where the diagonal jumps, at the boundary cells, and not in the interior | the same `D_200`, split: faces with a cell in the two layers next to a side (near) against all other internal faces (inner) | confirmed: `D_200(near) >= 5 D_200(inner)`; ruled out: `D_200(near) <= 2 D_200(inner)` |
| H7 | **the term is the commutator `C(G p)` of ADR 0073 together with the step dependence of `interp(rAU)`: the face coefficient of the pressure term is not the one the carry-over is consistent with unless `a` is uniform** (the steady defect `[interp(w G p) . S - interp(w) snGrad p abs(S)] / interp(a w)` in place of `R(p) / a_f`) | removal of the term alone by an opt-in (design in amendment 1, after the diagnosis): R with the opt-in in 200, 400 and 800 steps | confirmed: `max abs(d_200) <= 1.7e-6` (a third of `5.13e-6`) and the ratio `>= 4`; ruled out: `max abs(d_200) >= 3.6e-6` (less than 30 percent removed) |
| H8 | the remainder is a spatial term in the sense of the acceptance: at a fixed step size it falls with `h` | `R_20`, `R_40`, `R_80` in 200, 400 and 800 steps: `s = log2(max abs(d_200)(R_20) / max abs(d_200)(R_40))` and the same for 40 and 80 | no decision; the orders are printed and reported as the order in `h` at fixed `dt` |

Printed for the record, no decision: R with `ddt_correction = false` (no carry-over: the face term is then proportional to
`rAU`), `max abs(d_200)` and the ratio; `R_20` in 50, 100, 200 steps and `R_80` in 800, 1600, 3200 steps (the same `nu dt /
h^2` as R in 200, 400, 800): `max abs(d)` of the coarsest against the finest step of each, the order in `h` at fixed `nu
dt / h^2`; the periodic Taylor-Green box of the library's unit tests (`box_2d(64, 64, 2 pi, 2 pi)`, `nu = 0.1`, PS, BDF2,
three outer iterations, 50, 100, 200, 400 steps to `t = 0.5`): `q_N` between consecutive pairs and the error of the
amplitude against the exact decay (the set-up of ADR 0073 A1 with the settings of this item; ADR 0073 found the floor
there); the cell and the face of each maximum.

Predictions (mine, before any run): S0 holds. H1 ruled out (`q_200` about 0.4, `q_400` about 0.6). H2 ruled out. H3 ruled
out. H5 confirmed. H6 confirmed, less sure (the convective part of the diagonal varies everywhere). H7 confirmed. H8: the
remainder falls faster than `h^2` at fixed `dt` (the factor `a dt / (c0 + a dt)` saturates as `h` falls). `ddt_correction
= false`: much larger than `5e-6`.

## Registered checks

Release, Windows. New test file `crates/rcfd-fv/tests/bdf2_remainder_f344.rs`; the helpers of the set-up are copied from
`overset_f338.rs` (that file is not edited). Tests that take more than about two minutes are `#[ignore]` with their run
time as the reason and are run once.

### S. Set-up

| Id | Check | Limit |
| --- | --- | --- |
| S0 (set-up) | R in 200, 400, 800 steps, defaults: `max abs(d_200)` and the ratio: the numbers of ADR 0241 H16 (`5.13e-6`, `2.296`) | in `[5.0e-6, 5.3e-6]`; in `[2.2, 2.4]` |
| S1 | every step of every run of this ADR: the continuity of `StepInfo` | `<= 1e-9` |

### A. Acceptance (the corrective, if amendment 1 registers one; the limits are those of the roadmap entry)

| Id | Check | Limit |
| --- | --- | --- |
| A1 (acceptance) | R with the opt-in: `max abs(d_200) / max abs(d_400)` | `>= 4` |
| A2 (acceptance) | TG_2 of ADR 0241 (BDF2, three outer iterations) with the opt-in: `max abs(d_200) / max abs(d_400)` (D2 of ADR 0241) | `>= 4` |
| A3 (no harm) | R with the opt-in in 800 steps: the largest error of the velocity against the exact Taylor-Green field, over that of the default run | `<= 1.05` |
| B1 (defaults) | the existing tests of `rcfd-fv` that run a line touched in `pimple.rs` (named in amendment 1), unedited, with the opt-in off; `cargo test -p rcfd-api --release --test baseline_hashes` | pass |
| B2 (defaults) | R in 200 steps with the opt-in field present and off against the numbers of S0's run | the run of S0 is that run (one code path); bit for bit by B1 |

Amendment 1 adds the checks of the corrective's own terms (what each must do), its negative controls and its mutations,
before its code. If the diagnosis rules H7 out, no corrective is built on it and amendment 1 says what is.

### Mutations (scratch copy `w49i-work/f344-scratch`, not committed)

Each applied alone to a `git archive` copy of the final tree by an exact replacement that must occur once; the named test
is run once and must fail; the file is restored and compared byte for byte.

| Id | Mutation | Must fail |
| --- | --- | --- |
| MD1 | `pimple.rs`: the old-old part of the transient Rhie-Chow defect is left out (`c2` taken as zero in the defect) | S0 |
| MD2 | the test's reference run takes 400 steps in place of 800 | S0 |
| MC* | the mutations of the corrective (each term left out or mis-signed, the gate inverted): registered in amendment 1 | A1 or the check amendment 1 names |

`scripts/limit_audit.py` is run on the new test file and every hit is read.

## Decision rule

- **done**: S0 held; a term is named and confirmed by its registered deciding number (H7, or another hypothesis by an
  amendment registered before its run); and the acceptance is met in one of its ways: (i) the opt-in is built, B1 holds,
  A1 and A2 are met; or (ii) the remainder is shown to be a spatial term with its order in `h` measured (H8 and the
  fixed-`nu dt / h^2` series) and the ADR says so with the numbers; the controls held and the mutations failed their
  checks.
- **partial**: a term is named and confirmed with a number, but neither way is complete: the opt-in exists and A1 or A2
  is missed, or no opt-in was built; or B1 could not be shown for a built opt-in (then the opt-in is not to be merged);
  or a control or mutation did not hold and was not corrected.
- **todo**: no term is confirmed beyond what ADR 0241 located.

A cause named with a number and no fix is reported as partial, not as done. If the conclusion is that the default step
carries a term of lower order in `dt`, the addendum says so with the number; the default is not changed here.

## Open choices taken

Each is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation); none is a decision the
maintainer made in person, and each is listed again in the addendum. None changes a default.

- **O1. Diagnosis by measurement of the state against diagnosis by a scratch change of the library.** (a) measure what
  the public state gives (`u`, `p`, `phi`) and decide H7 by the opt-in itself; (b) instrument `pimple.rs` in a scratch
  copy to print the commutator flux (ADR 0073 Y2 did). Taken: (a): `pimple.rs` is about to be reworked by another job,
  and the removal test is the stronger one.
- **O2. An opt-in whose kind ADR 0076 removed.** F-G of ADR 0073 was removed by the registered rule of ADR 0076, as part
  of a bundle that missed checks on periodic seams. The acceptance of F-344 asks for an opt-in. (a) build the corrective
  for internal faces only, leaving coupled (periodic) faces as they are, and say that the seam terms of ADR 0073 and
  ADR 0076 are not part of it; (b) build nothing and report the cause only. Taken: (a), provided the diagnosis confirms
  H5; whether an opt-in of this kind is wanted back in the library after ADR 0076 is listed as a question for the
  maintainer in the addendum.

## Limits of this registration

- L1. The numbers are measured on the Taylor-Green set-up at one viscosity (`nu = 0.1`), uniform Cartesian meshes,
  central convection, fixed-velocity boundaries. Nothing is concluded for walls with body forces, pressure boundaries,
  face jumps, immersed solids, the consistent correctors, the fractional step or moving meshes; a corrective is not
  defined there unless amendment 1 says so.
- L2. Periodic and rotationally periodic seams keep the terms ADR 0073 R3 and ADR 0076 found; this item does not touch
  them.
- L3. The first step of a BDF2 run stays an implicit Euler step.
- L4. H5 and H6 measure the face defect, a diagnostic quantity; they locate, they do not prove. The proof registered is
  H7.

## Open questions

Listed with options and a recommendation in the addendum, after the results.

## Amendment 1 (2026-11-12, written after the diagnosis runs of S0 and H1 to H6 and before any code or run of the corrective)

The test file was committed compiled and not run (`0cf3da56`), then `s0_rest_remainder_is_that_of_adr_0241` and
`diagnosis_of_the_remainder_at_rest` were started once, release. When this amendment was written the 80 by 80 series of
H8 and the periodic box (both printed only, no decision) were still running in that one process; every number with a
deciding number was printed. No limit or deciding number is changed.

| Id | Deciding number (registered) | Measured | Verdict |
| --- | --- | --- | --- |
| S0 (set-up) | `[5.0e-6, 5.3e-6]`; `[2.2, 2.4]` | `5.1262e-6`; `2.2956` | held |
| H1 the ratio is an effect of the measure | confirmed `q_400 >= 1.8`; ruled out `<= 1.3` | `q_100 = 0.059`, `q_200 = 0.374`, `q_400 = 0.623` (`delta_N`: `3.01e-6`, `2.89e-6`, `2.23e-6`, `1.45e-6` for `N` = 100 to 800) | **ruled out** |
| H2 left by the first step | confirmed `R_start >= 4`; ruled out `< 3.5` | `2.2956` (`5.1262e-6`, `2.2331e-6`: the numbers of S0 to five digits) | **ruled out** |
| H3 left by the outer loop or the correctors | confirmed either `>= 1e-6`; ruled out both `<= 1e-7` | `e_out = 1.46e-11`, `e_corr = 1.46e-11` | **ruled out** |
| H4 only the pressure | no decision | pressure `q_100 = -0.014`, `q_200 = 0.309`, `q_400 = 0.575` (`delta_N` of the pressure `3.9e-5` to `2.1e-5`): the same behaviour as the velocity | printed |
| H5 it enters through the face flux | confirmed `D_200 >= 0.5 max abs(d_200)`; ruled out `<= 0.1` | `D_200 = 3.95e-6` against `5.13e-6` (`0.77`); the defect itself is `2.8e-4` | **confirmed** |
| H6 it is made at the boundary cells | confirmed near `>= 5` inner; ruled out `<= 2` | near `3.95e-6`, inner `3.04e-7`: `12.98` (and `12.99` for `D_400`); the largest `d_200` of the velocity is in a cell of the second layer next to a corner | **confirmed** |

Printed: without the carry-over (`ddt_correction = false`) `max abs(d_200) = 1.29e-4`, ratio `1.91`. The difference between
consecutive step sizes hardly falls between 100 and 400 steps (order `0.06`, `0.37`) and falls at `0.62` between 400 and
1600: no power of `dt`, a function that bends where `a dt` passes `c0` (`a dt = 2.2` at 100 steps, `0.28` at 800), as the
arithmetic before the registration said. A common start over the first fifth of the time changes no digit: the
remainder is a function of the present step size, not an accumulated error.

So the diagnosis leaves H7, and a corrective is registered.

### The corrective C: a face coefficient the carry-over is consistent with (opt-in)

Derivation (first principles; the idea of a face coefficient built from the face value of the spatial coefficient is
that of Cubero and Fueyo 2007 and Pascau 2011, recalled from memory, not opened; the subtraction of `C(G p)` is F-G of
ADR 0073). Notation as above: `a_P` the momentum diagonal, `rAU = V / a_P`, `c0 / dt + a = a_P / V`, so `a` is the
non-temporal diagonal per volume.

With the outer loop converged the cell velocity is `u = HbyA - rAU G p`. The default flux is `phi = interp(HbyA) . S +
(gamma / dt) K - gamma snGrad p abs(S)` with `gamma = interp(rAU)` and `K = c1 d^n - c2 d^{n-1}` the carried defects. C
replaces this, on internal faces, by

`phi = interp(HbyA) . S - interp(rAU G p_lag) . S + Gamma_f interp(G p_lag) . S + (Gamma_f / dt) K - Gamma_f snGrad p abs(S)`,

`Gamma_f = 1 / interp(1 / rAU)` (the harmonic face value of `rAU` with the linear weights; on a boundary face the cell
value, as now),

where `p_lag` is the pressure the corrector starts from and `G` the Green-Gauss gradient the corrector uses for the
velocity (with a body force `b`: `G p_lag - b` in both added terms, and the body-force face term takes `Gamma_f` like
the pressure term). In the code: the face coefficient handed to the carry-over, the pressure equation and the flux
correction is `Gamma_f` in place of `gamma`, and `[Gamma_f interp(x) - interp(rAU x)] . S` with `x = G p_lag - b` is
added to `phiHbyA` before the pressure solve. Nothing else changes; the cell velocity is `HbyA - rAU G p` as before.

Converged (`p_lag = p`), `interp(HbyA) - interp(rAU G p) = interp(u)`, so the face defect obeys

`d^{n+1} = Gamma_f R(p) + (Gamma_f / dt)(c1 d^n - c2 d^{n-1})`, `R(p) = interp(G p) . S - snGrad p abs(S)`.

`1 / Gamma_f = interp(a_P / V) = c0 / dt + a_f` with `a_f = interp(a)`, so this is

`(c0 d^{n+1} - c1 d^n + c2 d^{n-1}) / dt + a_f d^{n+1} = R(p^{n+1})`:

the BDF2 formula applied to `d' + a_f d = R(p)`, an equation in which the step size does not appear. Its solution is
second-order accurate in `dt`, and a steady one is `d = R / a_f` exactly, whatever the step. The default satisfies the
same statement only where `a` is equal in the two cells of the face. F-G of ADR 0073 (the commutator alone, `gamma`
kept) leaves `interp(w) / interp(a w)` in the steady defect, which the arithmetic above puts at 1.2 percent over the
range of `dt` for diagonals of 5 and 4; C leaves none, which is why C and not F-G is built.

The fixed point of the correctors is unchanged in kind (with `p_new = p_lag` the added terms are those of the converged
formula); the lag `C(G (p_new - p_lag))` of a finite number of correctors is removed by the outer iterations, as every
other corrector lag.

**The switch.** New field `PimpleSettings::rhie_chow_step_consistent: bool`, default `false`, set by `piso()`; no schema
key, no `rcfd-api` change. With `false` every statement of the step is the present one (the new expressions sit in
branches taken only with the field set). It acts in `PimpleSolver::step_with` on a mesh at rest and in `OversetSolver`
(below). It does not act (the step is the default one, said in the field's documentation) with the consistent
correctors, on the library's own moving-mesh steps or in the fractional step; coupled (periodic) faces, face jumps and
the immersed-solid flux reconstruction are left as they are (L1, L2). The helpers (the harmonic face value and the
added flux) go in a new file `crates/rcfd-fv/src/pimple/face_consistent.rs`, so that the hunk in `pimple.rs` is the
field, its default, the choice of the face coefficient and one call.

**The overset step** (`overset.rs`, `piso`). The same two changes, with the fringe and hole cells treated as the step
treats `HbyA` and `rAU` there (M6 of ADR 0219): `Gamma_f` from the `rAU` the step already builds (interpolated at the
fringe cells, `dt` at the hole cells); the cell quantities `q = rAU G p_lag` and `x = G p_lag` of a fringe cell are the
donor interpolants of the calculated values (the stencil weights that give its `HbyA`), and zero at a hole cell; the
added flux is `[Gamma_f interp(x) - interp(q)] . S`. Then `interp(HbyA) - interp(q)` is the face value of the cell
velocities, fringe cells included (a fringe velocity is the donor interpolant of `HbyA - rAU G p`), so the defect
equation above holds on every internal face with a calculated cell. With every cell calculated it is the library's
formula, expression for expression.

### Checks of the corrective (limits fixed here)

R with C is R with `rhie_chow_step_consistent = true`.

| Id | Check | Limit |
| --- | --- | --- |
| A1 (acceptance) | R with C in 200, 400, 800 steps: `max abs(d_200) / max abs(d_400)` | `>= 4` |
| H7 | the same runs: `max abs(d_200)` | confirmed `<= 1.7e-6` and A1 met; ruled out `>= 3.6e-6` |
| C1 | the same runs: `D_200`, the largest change of the face defect over all internal faces | `<= 4e-7` (a tenth of the default's `3.95e-6`) |
| C2 | R with C in 100, 200, 400, 800 steps: the order between consecutive pairs `q_200` | `>= 1.7` |
| A3 (no harm) | R in 800 steps: the largest error against the exact field with C over that of the default | `<= 1.05` |
| C3 | **the steady solution does not depend on the step.** The lid-driven cavity (`cavity_2d(20, 1.0)`, `nu = 0.1`, lid velocity 1, PS, BDF2, three outer iterations) from rest to `t = 20` in steps of `0.04` and of `0.01`, with C: the largest difference of the cell velocities of the two runs; printed: the change of the velocity over the last step of each | `<= 1e-9` |
| C3n (negative control) | the same two runs without C | `>= 1e-6` |
| C4 | every step of every run with C: continuity | `<= 1e-9` |
| E1 | TGB_1 of ADR 0241 Q0b (the level-1 background at rest, 40 steps of `0.0025`, BDF2, three outer iterations) with C: `OversetSolver::advance_mesh` against `PimpleSolver::step`: the largest difference of `u`, `p`, `phi` (over the largest flux) | `<= 1e-12` each |
| E1n (negative control) | the library with C against the library without, the same run: `u` | `>= 1e-9` |
| A2 (acceptance) | TG_2 of ADR 0241 (BDF2, three outer iterations, 200, 400, 800 steps) with C: `max abs(d_200) / max abs(d_400)` | `>= 4` |
| A2b | the same runs: `max abs(d_200)` | `<= 2.125e-5` (the default's, ADR 0241 D1) |
| A2c | the same runs: continuity of the calculated cells, every step; `e(TG_2)` in 800 steps over the default's `3.179e-4` | `<= 1e-9`; `<= 1.05` |
| B1 (defaults) | unedited, with the field off: the unit tests of `rcfd-fv` (`--lib`), `w33b_default_bits`, `consistent_defaults_f301`, `piso_thin_gap_f299`, `fractional_step`, `periodic`, `w48v_corrector_control`, `w48ab_dt_trigger`, `overset_f134`, `overset_f134_records`, `overset_f335`, `overset_f335_records`, `overset_f338` (Q0b: the overset step equals the library bit for bit), `ale_gcl`, `immersed_solid_f219` (their non-ignored tests); `cargo test -p rcfd-api --release --test baseline_hashes` | pass |

B2 of the registration is dropped as a separate check: it is S0 run on the final tree (S0's band is the number of the
tree before the change).

Mutations of the corrective (scratch copy, each alone, the named check must fail):

| Id | Mutation | Must fail |
| --- | --- | --- |
| MC1 | the added flux is left out (the harmonic coefficient kept) | C3 |
| MC2 | the face coefficient stays `interp(rAU)` (the added flux kept: F-G of ADR 0073) | C3 |
| MC3 | the added flux with the opposite sign | C3 |
| MC4 | the gate inverted in `pimple.rs` (C acts when the field is off) | S0 |
| MC5 | `overset.rs`: the fringe cells take their own `rAU G p_lag` in place of the donor interpolant | A2 (if A2 is met; otherwise the number is reported and the mutation counts as not caught) |
| MC6 | `overset.rs`: the added flux left out (the harmonic coefficient kept) | E1 |

Predictions (mine): A1 met, `max abs(d_200)` between `1e-8` and `5e-7`; C1 near `1e-8`; C2 about 2 unless the remainder
is so small that the linear solves show (`1e-11`); A3 `1.00`; C3 near `1e-12`, C3n near `1e-4`; E1 zero or `1e-15`. A2:
uncertain. The coupled meshes carry five times the remainder of the single mesh (ADR 0241 did not show that it is the
same term); if the fringe cells make a step-dependent part of their own, A2 is missed and the item is partial by the
rule.

The decision rule is unchanged.

## Amendment 2 (2026-11-12, written after the runs of amendment 1 and before any further run)

The opt-in and its tests were committed checked and not run (`36c40e8b`), then A1, C3, E1 and A2 ran once. All numbers
are in addendum 1. Met: A1 (`5.019`), H7 (`8.41e-10`), C1, C2, A3, C3, C3n, C4, E1 (bit for bit), E1n, A2b, A2c. Not met:

| Id | Limit | Measured |
| --- | --- | --- |
| A2 (acceptance) | `>= 4` | `2.454` (`max abs(d_200) = 2.057e-5`, `max abs(d_400) = 8.38e-6`, both in the calculated component cell at `(1.668, 0.969)`, the cell of ADR 0241 D2) |

The limit stays; A2 becomes an ignored `*_known_miss` test with the limit intact, and the test of A2 keeps the checks
that held. On the single mesh at rest C takes the remainder from `5.13e-6` to `8.4e-10`; on TG_2 it takes it from
`2.125e-5` to `2.057e-5`, three percent. So the remainder of TG_2 is, to 97 percent, **not** the term of this item: ADR
0241 suggested that it was the same term and did not show it; it is not. MC5 (its check is A2) is not run and counts as
not caught.

**The miss of A2: hypotheses, fixed before their runs.** Both measurements take BDF2, three outer iterations and C.

| Id | Hypothesis | Measurement | Deciding number |
| --- | --- | --- | --- |
| H9 | on a single moving mesh with every cell calculated the remainder (ADR 0241 M1, M2: `3.96e-6`, ratio `2.28`) is the term of this item, and C removes it there as at rest | SMc of ADR 0241 through `OversetSolver::advance_mesh` (the helper of `overset_f338.rs`, copied) with C in 200, 400, 800 steps: `max abs(d_200)` and the ratio | confirmed: `max abs(d_200) <= 1.3e-6` and the ratio `>= 4`; ruled out: `max abs(d_200) >= 2.8e-6` |
| H10 | the remainder of the coupled meshes is made by the motion through the coupling (cells changing kind, the donor stencils changing with the pose), not by the coupling as such | TG_1 of ADR 0219 (level 1, the coupled meshes) with C in 100, 200, 400 steps (the step sizes `0.005`, `0.0025`, `0.00125`; differences to the run in 400), once with the component moving (`tg_pose(t)`) and once with it held at `tg_pose(0)` (a coupled run at rest: the same meshes, holes and fringe, no change of kind): `m = max abs(d_100)` moving, `r = max abs(d_100)` at rest, over the non-hole cells | confirmed: `r <= 0.1 m`; ruled out: `r >= 0.5 m` |

Level 1 and not level 2 for H10: six runs of level 2 take half an hour under the present load; the hypothesis is about
the mechanism, and the remainder of level 1 is measured by the same runs. Printed: the ratios `max abs(d_100) / max
abs(d_200)` of both, the cell and the kind of each maximum.

Reading fixed here. If H9 is confirmed, the moving-mesh terms carry no step-dependent term of their own and what is left
on TG_2 belongs to the coupling. If H10 is confirmed, it is made where cells change kind or donors as the component
moves, an event per cell and step whose size does not fall with `dt` as a truncation does; if H10 is ruled out, it is
made by the coupling at rest (the interpolated rows of the fringe). Either way no corrective for it is built in this
item: it is a follow-up. None of this moves the limit of A2.

Test: `a2_diagnosis_of_the_coupled_remainder` (ignored, printed, asserts nothing about its numbers). Predictions (mine):
H9 confirmed; H10 confirmed, less sure. By the decision rule the opt-in exists and A2 is missed: **partial**, unless
the lead reads the second way of the acceptance (a spatial term with its order in `h`, measured by H8) as meeting the
item; the addendum says which numbers bear on that.

## Addendum 1: results (2026-11-12)

Measured against the limits and deciding numbers registered above and in amendments 1 and 2; none was changed. Release
builds on Windows (`cargo test --release -p rcfd-fv --test bdf2_remainder_f344 ...`, one test thread). The registration
is `55c60bcf`; the set-up and the diagnosis test, compiled and not run, `0cf3da56`; amendment 1 `2d8687cd` and the opt-in
with its tests `36c40e8b`; amendment 2 `537f59eb` and its tests `0436b0d4`. Each amendment was committed before its runs
and each registered run was made once. The logs are in `E:/projects/cfd/w49i-work/f344-scratch` (`f344-diag.log`,
`f344-c.log`, `f344-a2.log`, `f344-a2diag.log`, `f344-b1.log`, `f344-b1b.log`, `f344-baseline.log`, `f344-M*.log`,
`f344-final.log`).

**Said plainly.** The remainder at rest is named, measured and removed by an opt-in; the second number of the
acceptance's first way is missed. The term is the one ADR 0073 found on the periodic box in wave 34B: the face flux of
the converged step depends on the step size where the momentum diagonal differs between the two cells of a face, here
at the cells next to a fixed-velocity boundary. With the opt-in the remainder on the mesh of SMc at rest falls from
`5.13e-6` to `8.4e-10` and its ratio rises from `2.30` to `5.02`. On TG_2 the same opt-in changes the remainder by three
percent (`2.057e-5`, ratio `2.454` against the limit 4): the remainder of the coupled meshes is another term, made by
the motion of the component through the coupling, and it is not removed here.

**The default step.** `PimpleSolver::step` with BDF2 (and with implicit Euler: the formula does not depend on the
scheme) has, in its converged equations, a face flux that depends on the step size. The dependence is not of first
order: it follows `a dt / (c0 + a dt)` with `a` of the order `4 nu / h^2`, so between two step sizes it falls at an
observed order of `0.06` to `0.62` here, and the steady solution of a lid-driven cavity differs by `3.7e-4` (of a lid
velocity of 1) between steps of `0.04` and `0.01` on 20 by 20 cells. In size it is below the spatial error (`6.2e-4` on
the mesh of SMc) by a factor 120 at `dt = 0.0025`. Changing the default is the maintainer's decision and is not taken
here.

### What was built

- `crates/rcfd-fv/src/pimple/face_consistent.rs` (new, 65 lines): `harmonic_face`, `add_commutator`,
  `add_lagged_gradient`.
- `crates/rcfd-fv/src/pimple.rs`, 20 lines added, 1 replaced, in five places (line numbers of the final file): line 37
  (`pub(crate) mod face_consistent;`); lines 132 to 137 (the field `PimpleSettings::rhie_chow_step_consistent` and its
  documentation, after `moving_mesh_lag`); line 556 (its default `false` in `piso()`); lines 1325 to 1331 (the flag
  `rc` and the choice of `gamma_p`, in place of the one line `let gamma_p = face_interpolate(&g, &r_au);`); lines 1360
  to 1363 (the call of `add_lagged_gradient` after the carry-over, inside the corrector loop). Nothing else in
  `pimple.rs` is touched; the consistent-corrector flags are read once (`!s.consistent_correctors` in line 1326).
- `crates/rcfd-fv/src/overset.rs`, `piso`: lines 946 to 952 (the choice of `gamma_p`) and 999 to 1026 (the added flux
  with the donor interpolants at the fringe cells).
- `crates/rcfd-fv/tests/bdf2_remainder_f344.rs`: 8 tests, 4 gate-run (S0, A1, C3, E1) and 4 ignored (the two diagnosis
  tests, the slow A2 test, the known miss of A2). No existing test was edited.
- Not touched: `immersed.rs`, `compressible*.rs`, `rcfd-io`, `rcfd-api` (`sim.rs`, `schema.rs`), `ddt.rs`,
  `pimple/moving.rs`, the roadmap files.

### Hypotheses

| Id | Deciding number (registered) | Measured | Verdict |
| --- | --- | --- | --- |
| H1 (a) the ratio is an effect of the measure | confirmed `q_400 >= 1.8`; ruled out `<= 1.3` | `q_100 = 0.059`, `q_200 = 0.374`, `q_400 = 0.623` | **ruled out** |
| H2 (c) left by the first step | confirmed `R_start >= 4`; ruled out `< 3.5` | `2.2956`, the numbers of S0 to five digits | **ruled out** |
| H3 (f, g) left by the outer loop or the correctors | confirmed either `>= 1e-6`; ruled out both `<= 1e-7` | `1.46e-11`, `1.46e-11` | **ruled out** |
| H4 (d) only the pressure | no decision | the pressure behaves as the velocity (`q` of `-0.01`, `0.31`, `0.58`; `delta_N` `3.9e-5` to `2.1e-5`) | printed |
| H5 (b) it enters through the face flux | confirmed `D_200 >= 0.5 max abs(d_200)`; ruled out `<= 0.1` | `D_200 = 3.95e-6` (`0.77` of `5.13e-6`) | **confirmed** |
| H6 (e) it is made at the boundary cells | confirmed near `>= 5` inner; ruled out `<= 2` | `12.98` (near `3.95e-6`, inner `3.04e-7`) | **confirmed** |
| H7 the term is the commutator and the step dependence of `interp(rAU)` | confirmed `max abs(d_200) <= 1.7e-6` with C and A1 met; ruled out `>= 3.6e-6` | `8.41e-10`, ratio `5.019` | **confirmed** |
| H8 the order in `h` | no decision | at fixed `dt`: `max abs(d_200)` `7.99e-6`, `5.13e-6`, `1.50e-6` on 20, 40, 80 cells per side (orders `0.64`, `1.77`; ratios to `d_400` `2.72`, `2.30`, `1.84`); at fixed `nu dt / h^2`: `1.75e-5`, `5.13e-6`, `1.38e-6` (orders `1.77`, `1.90`; ratios `2.28`, `2.30`, `2.30`) | printed |
| H9 (amendment 2) on a single moving mesh C removes the remainder too | confirmed `max abs(d_200) <= 1.3e-6` and ratio `>= 4`; ruled out `>= 2.8e-6` | `2.71e-7` (`3.96e-6` without C in ADR 0241), ratio `5.015` | **confirmed** |
| H10 (amendment 2) the remainder of the coupled meshes is made by the motion through the coupling | confirmed `r <= 0.1 m`; ruled out `r >= 0.5 m` | `r / m = 0.0024`: level 1 with C, at rest `1.26e-7` (ratio `4.98`), moving `5.18e-5` (ratio `3.95`) | **confirmed** |

**The term, named.** In the converged step the face defect `d = phi - interp(u) . S` obeys

`d^{n+1} = (gamma / dt)(c1 d^n - c2 d^{n-1}) + gamma R(p) + C(G p)`, `C(x) = interp(rAU x) . S - interp(rAU) interp(x) . S`,

and its steady value is `[interp(w G p) . S - interp(w) snGrad p abs(S)] / interp(a w)` with `w = 1 / (c0 + a dt)`: a
function of `dt` wherever `a`, the non-temporal momentum diagonal per volume, differs between the two cells of the face.
The term is `C(G p)` together with the step dependence of the face coefficient `gamma = interp(rAU)` (ADR 0073 R2 and
its amendment 1 named `C(G p)`). Numbers: on R the face defect changes between 200 and 800 steps by `3.95e-6` at the
faces of the two layers next to a side and by `3.0e-7` elsewhere, and the cell velocity by `5.13e-6`; with the term
removed, the defect changes by `9.2e-12` and the velocity by `8.4e-10`, which converges at the order `2.01` (`2.7e-9`,
`6.7e-10`, `1.7e-10` between 100, 200, 400 and 800 steps). Its shape is that of `a dt / (c0 + a dt)`: without the
carry-over at all (`ddt_correction = false`) the remainder is `1.29e-4`. It does not depend on the start, the
correctors or the outer iterations (H2, H3), as ADR 0241 measured.

**Its order in `h`.** At a fixed `nu dt / h^2` the remainder falls with `h` at the order `1.77` (20 to 40 cells per
side) and `1.90` (40 to 80), and its ratio between step sizes stays `2.3`: it is a term of the spatial discretisation,
close to the form `h^2 F(nu dt / h^2)` with `F` bounded (two refinements; the orders are below 2). At a fixed `dt` the orders are
`0.64` and `1.77` (the argument of `F` grows by 4 with each refinement). It never becomes a term of first order in
`dt`: refining `dt` alone at fixed `h` gives observed orders below 1 that rise slowly (`0.06`, `0.37`, `0.62`).

**The periodic box** (printed; 64 by 64 cells, the settings of this item, default step): the differences between
consecutive step sizes fall at `1.31` and `0.85` (50 to 400 steps), the amplitude of the mode at `2.1` and `2.2`
(differences `2.05e-6`, `4.76e-7`, `1.00e-7`). The same floor as ADR 0073 A1 found; not run with C (coupled faces are
not part of C, L2).

### Checks

| Check | Limit | Measured | Verdict |
| --- | --- | --- | --- |
| S0 (set-up) R: `max abs(d_200)`; ratio | `[5.0e-6, 5.3e-6]`; `[2.2, 2.4]` | `5.1262e-6`; `2.2956` (before the library change and again on the final tree) | held |
| S1 continuity, the runs of R | `<= 1e-9` | `8.5e-12` | met |
| **A1 (acceptance)** R with C: ratio | `>= 4` | `5.019` (`8.41e-10`, `1.68e-10`) | **met** |
| C1 R with C: change of the face defect `D_200` | `<= 4e-7` | `9.2e-12` | met |
| C2 R with C: `q_200` | `>= 1.7` | `2.007` (`q_100 = 2.013`) | met |
| A3 (no harm) error against the exact field, C over default, 800 steps | `<= 1.05` | `1.0039` (`6.2264e-4` over `6.2024e-4`) | met |
| C3 steady cavity with C, steps `0.04` against `0.01` | `<= 1e-9` | `2.4e-13` (change over the last step `2.1e-13`, `3.9e-17`) | met |
| C3n (negative control) the same without C | `>= 1e-6` | `3.75e-4` | held |
| C4 continuity, the runs with C | `<= 1e-9` | `2.6e-12` | met |
| E1 overset at rest against the library, both with C: `u`, `p`, `phi` | `<= 1e-12` each | `0`, `0`, `0`; bit for bit | met |
| E1n (negative control) the library with C against without | `>= 1e-9` | `4.59e-6` | held |
| **A2 (acceptance)** TG_2 with C: ratio | `>= 4` | `2.454` (`2.057e-5`, `8.38e-6`) | **missed** (known-miss test) |
| A2b TG_2 with C: `max abs(d_200)` | `<= 2.125e-5` | `2.057e-5` | met |
| A2c continuity; `e(TG_2)` in 800 steps over `3.179e-4` | `<= 1e-9`; `<= 1.05` | `6.3e-14`; `1.0022` (`3.1859e-4`) | met |
| B1 (defaults) the existing suites, unedited | pass | unit tests of the crate 91 passed (5 ignored); `ale_gcl` 4; `consistent_defaults_f301` 6; `fractional_step` 13 (3 ignored); `overset_f134` 12 (2 ignored); `overset_f134_records` 1; `overset_f335` 5 (4 ignored); `overset_f335_records` 4; `overset_f338` 9 (9 ignored; Q0b among them); `periodic` 3; `piso_thin_gap_f299` 18 (2 ignored); `w33b_default_bits` 1; `w48ab_dt_trigger` 5; `w48v_corrector_control` 5; `immersed_solid_f219`: 9 of its 11 gate-run tests passed, **the two long cylinder runs (`a1_a2_two_d_one_drag_and_lift_on_m3`, `a3_two_d_two_strouhal_number`) had not finished after two hours under load when this was written** | met, except the two unfinished runs of `immersed_solid_f219` (not shown) |
| B1 `cargo test -p rcfd-api --release --test baseline_hashes` | pass | 3 passed, 1 ignored | met |

The cavity is the plainest statement of what C does: its steady solution is the same to `2.4e-13` for two step sizes,
where the default's differs by `3.75e-4`. With the commutator alone and `interp(rAU)` kept (F-G of ADR 0073, mutation
MC2) the difference is `9.4e-5`: at boundary cells the commutator removes three quarters of the effect and the face
coefficient the rest, which is why C takes the harmonic face coefficient as well.

### Mutations

Applied alone to a `git archive` copy of `36c40e8b` (`f344-scratch/mut`, its own target directory; the library files of
the final tree are those of `36c40e8b`, the later commits add tests and text) by `f344-mut-run.py` (an exact replacement
that occurs once), the named test run once, the file restored and compared byte for byte.

| Id | Must fail | Result |
| --- | --- | --- |
| MD1 the old-old part of the carried defect left out | S0 | failed: the run diverges (`max abs(d_200) = 33`) |
| MD2 the reference run in 400 steps | S0 | failed: `2.89e-6`, ratio infinite |
| MC1 the added flux left out | C3 | failed: `7.0e-4` |
| MC2 the face coefficient stays `interp(rAU)` | C3 | failed: `9.4e-5` |
| MC3 the added flux with the opposite sign | C3 | failed: `1.4e-3` |
| MC4 the gate inverted in `pimple.rs` | S0 | failed: `8.4e-10`, ratio `5.02` (the opt-in's numbers) |
| MC5 `overset.rs`: fringe cells take their own `rAU G p_lag` | A2 | **not run, counts as not caught** (A2 is missed; amendment 2) |
| MC6 `overset.rs`: the added flux left out | E1 | failed: `4.7e-6`, `4.6e-5`, `2.3e-7` |

So the fringe branch of the overset change (the donor interpolants) is guarded by no failing check. `scripts/limit_audit.py`
on the new test file: 0 hits. The two diagnosis tests print and assert nothing about their numbers (decided here); by
design, and ignored.

### Deviations and disclosures

- The leading hypothesis was not mine: ADR 0073 had the commutator, measured on the periodic box, and an opt-in for it
  that ADR 0076 removed. I found that while reading for the registration and wrote it there. What this item adds is the
  boundary cells (where it is largest), the harmonic face coefficient, and the measurement at rest.
- The arithmetic before the registration predicted the shape and the place; it is disclosed there. All my predictions
  held except A2, which I called uncertain.
- Amendment 1 was written while the 80 by 80 series of H8 and the periodic box, both without a deciding number, were
  still running in the diagnosis process.
- B2 of the registration was dropped by amendment 1 (it is S0 on the final tree; run, held).
- ADR 0241 wrote that the five times larger remainder of the coupled meshes "is suggested ... and not shown" to be the
  same term. It is not the same term (A2, H10). ADR 0241 is not edited.
- H10 was measured on level 1, not on level 2 (cost; said in amendment 2). The remainder of level 1 moving falls at a
  ratio of `3.95` between its step sizes, that of level 2 at `2.45`; why they differ is not diagnosed.
- The diagnosis test runs R on 40 cells in 200, 400, 800 steps more than once (S0, H1, H8); a deterministic run, the
  same numbers each time. After the last test edit the gate-run tests of the new file ran once more on the final tree
  (`f344-final.log`: 4 passed, 4 ignored; the same numbers).
- The unchanged tree was compiled once before the registration (disclosed in the status line).

### Not run

- The whole workspace suite, the gate, the other `rcfd-api` tests, Linux.
- MC5. TG_2 at rest and level 2 for H10. The periodic box and the cylinder with C.
- **Paths of C that no check exercises:** a body force (`G p_lag - b` in `add_lagged_gradient` and the body-force face
  term with the harmonic coefficient): derived, written, not run by any test; the fringe branch in `overset.rs` (MC5);
  a non-orthogonal or graded mesh; implicit Euler with C; one outer iteration with C (the corrector lag of C).
- The two long tests of `immersed_solid_f219` named under B1 did not finish in the time of this job (they started on the
  tree `36c40e8b`, whose library files are the final ones). The immersed forcing is off in every run of this item and
  the step with the opt-in off is the default one statement for statement; `n1_immersed_forcing_is_off_by_default`
  and eight others passed. The lead's gate covers them.

### After the tests

`cargo fmt -p rcfd-fv --check`: clean. `cargo clippy -p rcfd-fv --all-targets -- -D warnings`: clean.

### Decision by the registered rule

**Partial.** A term is named and confirmed by its registered number (H7: `8.41e-10` against `1.7e-6`, the ratio `5.02`);
the opt-in is built and the defaults are bit for bit (B1, with two long immersed-solid runs unfinished); A1 is met; **A2 is missed** (`2.454` against 4), and by the
reading registered before any run ("if the corrective reaches 4 at rest and not on TG_2, that is a miss of the first
way") that is the clause "the opt-in exists and A1 or A2 is missed". The controls held; seven of eight mutations failed
their checks and one was not run.

The rule has a second clause that the numbers also fit, and I do not take it: the second way of the acceptance ("shown
to be a spatial term with its order in h") is measured (H8: order `1.8` to `1.9` at fixed `nu dt / h^2`). Whether that
alone makes the item done is the lead's reading of the acceptance; I propose partial because the item was opened to
explain the missed D2 of TG_2, and D2 is still missed, for a cause this item located (H10) and did not name.

### Open questions (each with options and a recommendation)

- **Q1. The remainder of the coupled meshes in motion** (`2.06e-5` on TG_2, ratio `2.45`; `5.2e-5` on level 1 moving
  against `1.3e-7` at rest). (a) a follow-up item that finds which event of a moving coupling carries it (a cell
  changing kind, a donor stencil changing with the pose, the refresh of the fringe values before the step, the flux of
  the faces without a calculated cell) with the set-ups of H10, where the coupled meshes at rest are a clean reference;
  (b) leave it: it is below the spatial error of TG_2 (`3.2e-4`) by a factor 15. Recommendation (a); proposed below,
  not taken here (a new item is the lead's).
- **Q2. An opt-in of a kind ADR 0076 removed (O2).** (a) keep `rhie_chow_step_consistent` as built: internal faces only,
  no seam terms, met on every check of its own; (b) remove it again and keep the diagnosis. Recommendation (a): the
  misses that removed F-G were on periodic and rotational seams and in a no-harm comparison on a seam, none of which C
  touches. Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation), not a decision the
  maintainer made in person; it is in conflict with the letter of ADR 0076's removal, so it is flagged for the
  maintainer.
- **Q3. Where C does not act.** (a) a follow-up that extends it to the consistent correctors and the library's
  moving-mesh steps, checks the body-force path, and adds the case-tree key; (b) leave C as a library-only opt-in.
  Recommendation (a), after the rework of the corrector flags in `pimple.rs`. Not taken here.
- **Q4. Coupled (periodic) faces.** ADR 0073 R3 and ADR 0076 found two further step-dependent terms there; C leaves
  them. (a) take them up with Q3; (b) not now. Recommendation (b) until a case needs BDF2 across a periodic seam.
- **Q5. C by default.** It would change every transient result of the library (the steady cavity by `3.7e-4`). It
  changes a default; not taken. Open for the maintainer. What speaks for it: a steady solution that does not depend on
  the step; what speaks against: every baseline and record of the transient step changes, and C is not yet defined on
  all paths (Q3, Q4).
- **Q6. The text of ADR 0241 and of the note of F-338** on the coupled remainder ("that ... is the same term is not
  shown"): now shown not to be. (a) correct the roadmap note of F-338 (text below); (b) leave it. Recommendation (a).

### For the lead: roadmap entries

No roadmap file was edited on this branch.

**F-344**, proposed status: `partial`. Proposed note:

> From ADR 0241 question 1, recommendation taken on the maintainer's standing instruction of 2026-11-11. ADR 0245 (registration of F-344, 2026-11-12, branch f344-bdf2-remainder): PARTIAL. Named: the remainder at rest is the step-size dependence of the converged face flux that ADR 0073 found on the periodic box (the commutator of rAU with the face interpolation, C(G p), and the step dependence of the face coefficient interp(rAU)): the steady face defect is [interp(w G p).S - interp(w) snGrad p |S|] / interp(a w) with w = 1/(c0 + a dt), a function of dt wherever the momentum diagonal differs between the two cells of a face, largest at the cells next to a fixed-velocity boundary (the face defect changes by 3.95e-6 between 200 and 800 steps there, thirteen times more than inside). It is not of first order in dt: it follows a dt/(c0 + a dt), observed orders 0.06 to 0.62 between consecutive step sizes; at fixed nu dt/h^2 it falls with h at the order 1.8 to 1.9. Ruled out by measurement: the measure, the start, the outer loop and the correctors (1.5e-11). Built: the opt-in PimpleSettings::rhie_chow_step_consistent (default false, defaults bit for bit: the existing suites and baseline_hashes unedited): the harmonic face value of rAU as the face coefficient and the commutator of the lagged pressure gradient added to the flux of internal faces, in PimpleSolver::step on a mesh at rest and in the overset step (new file pimple/face_consistent.rs, 20 lines in pimple.rs); the face defect then obeys a BDF2 equation without the step size. Met: at rest max abs(d_200) falls from 5.13e-6 to 8.4e-10 and the ratio rises from 2.30 to 5.02 (limit 4), the order between consecutive steps is 2.01; the steady lid-driven cavity is the same to 2.4e-13 for steps of 0.04 and 0.01 (3.7e-4 without the opt-in); the error against the exact field is unchanged (1.004); the overset step at rest equals the library bit for bit with it; on the single moving mesh 3.96e-6 falls to 2.7e-7, ratio 5.02. Missed (acceptance): D2 of ADR 0241 on TG_2 with the opt-in is 2.454 (limit 4), max abs(d_200) 2.057e-5 against 2.125e-5 without: the remainder of the coupled meshes is another term; located: with the component held still the coupled meshes of level 1 leave 1.3e-7 at a ratio of 4.98, moving 5.2e-5, so it is made by the motion through the coupling (cells changing kind, donors changing with the pose); not named. Seven mutations caught; the one of the fringe branch of the overset change was not run (its check is the missed one). Not exercised by any check: a body force with the opt-in. The opt-in does not act with the consistent correctors, on the library's moving-mesh steps, in the fractional step, on coupled faces; no case-tree key. Whether the default step should change is open for the maintainer. Follow-ups proposed (ADR 0245 questions 1 and 3).

**F-338**, status unchanged (`partial`). In its note, replace "that the five times larger remainder on the coupled meshes is the same term is not shown" by:

> the five times larger remainder on the coupled meshes is not that term (ADR 0245: with the at-rest term removed by an opt-in it is 2.06e-5 at a ratio of 2.45; it is made by the motion of the component through the coupling)

Proposed follow-up items (the numbers are the lead's):

- **Overset BDF2: the step-size-dependent remainder made by the motion through the coupling.** Depends on F-344. Acceptance: "The remainder of TG_2 with BDF2, three outer iterations and rhie_chow_step_consistent (max abs(d_200) 2.057e-5 with the ratio 2.454 to max abs(d_400), ADR 0245 A2; 1.3e-7 with the ratio 4.98 on the coupled meshes of level 1 at rest against 5.2e-5 moving, ADR 0245 H10) is traced to a named event or term of the moving coupling with a number, and either removed as an opt-in (defaults bit-identical; D2 of ADR 0241 at least 4 on TG_2) or the ADR names why not with a number." Note:

> From ADR 0245 question 1, recommendation (a); proposed by the F-344 addendum, not yet taken. Candidates not measured: a cell changing kind, a donor stencil changing with the pose, the refresh of the fringe values before the step, the flux of the faces without a calculated cell. The coupled meshes at rest are a clean reference (ratio 4.98). The fringe branch of the F-344 opt-in in overset.rs has no failing mutation yet (ADR 0245 MC5).

- **Transient step: the step-consistent face coefficient on the remaining paths.** Depends on F-344. Acceptance: "rhie_chow_step_consistent acts with the consistent correctors and on PimpleSolver::step_ale_with_walls (defaults bit-identical); with a body force a hydrostatic column stays at rest to 1e-10 and the steady solution of a buoyant cavity is the same to 1e-9 for two step sizes; the case tree has a key for it; the check C3 of ADR 0245 holds on each path, or the ADR names why not with a number." Note:

> From ADR 0245 question 3, recommendation (a), after the rework of the corrector flags in pimple.rs; proposed by the F-344 addendum, not yet taken. Periodic seams (ADR 0073 R3, ADR 0076) are not part of it.

## Amendment 3 (2026-11-12, after the merge of main and before any run of the checks below)

Written on the lead's brief of 2026-11-12, which asks for three steps before the opt-in is merged: bring the work up to main,
finish the evidence addendum 1 left open (the two long immersed-solid runs, MC5), and answer ADR 0076's question directly
(does the opt-in harm the periodic and rotational seams where F-G of ADR 0073 failed?). Nothing above this line is changed;
no limit of the registration or of amendments 1 and 2 is moved. Each run below is made once.

### The merge of main (done before this amendment; disclosed)

`git merge main` (main `2fa39daf`, merge commit `aa325442`) merged without a textual conflict: F-322 (ADR 0258) changed
`pimple.rs` around the face coefficient (`ConsistentCorrectors`, `phi_solve`, the X1 and X3 terms), and git placed the
hunks of F-344 between them. One conflict of meaning was found by reading the merged step: the gate of the opt-in read
`!s.consistent_correctors`, and after ADR 0258 V1 the consistent correctors can also be asked for through
`PimpleSettings::consistent` (the step reads them as `cf = s.consistent_flags()`). With `consistent.enabled` set and the
old field false the opt-in would have acted with the consistent correctors, which its documentation excludes. Commit
`10171174` makes the gate `s.rhie_chow_step_consistent && !cf.enabled && self.ale.is_none()`. With the opt-in off the
conjunction is false whatever `cf` is, so the step is the main step statement for statement; checked by the B rows below.
`overset.rs` refuses the consistent correctors through the older field only; that is ADR 0258's listed item and is not
touched here.

### G. A check that guards the fringe branch of the overset opt-in (MC5)

Addendum 1: MC5 (the fringe cells take their own `rAU G p_lag` in place of the donor interpolant) had A2 as its check, A2
is missed, so the branch is guarded by nothing.

Why the obvious run-level check does not guard it. In a uniform flow through a moving component the pressure is uniform,
so `G p_lag = 0` in every cell and `x = q = 0` at every cell whichever value the fringe branch gives: MC5 changes no bit
there. A steady overset flow at rest, two step sizes, would catch it in principle (the design makes the defect equation
hold on every face with a calculated cell), but the overset step at rest carries other step-dependent parts that no check
has separated (the carry-over of the moving-mesh defect, the fringe rows), so a miss there would not locate the branch.

What the branch must give (amendment 1, "The overset step"): for every cell `c`,

- calculated: `x_c = (G p_lag)_c`, `q_c = rAU_c x_c`;
- fringe with stencil `(k_j, w_j)`: `x_c = sum_j w_j (G p_lag)_{k_j}`, `q_c = sum_j w_j rAU_{k_j} (G p_lag)_{k_j}`;
- hole: `x_c = q_c = 0`;
- with a trivial connectivity: `x = G p_lag`, `q = rAU x` everywhere.

So the check is a direct comparison of the branch with this formula. To make that possible the statements of the branch
move unchanged (same expressions, same order of summation) from `OversetSolver::piso` into a function
`face_consistent::overset_lagged_gradient(geom, pbc, p_lag, r_au, conn, trivial) -> (x, q)`, called where the block was.
Nothing else changes; with the opt-in off it is not called.

| Id | Check | Limit |
| --- | --- | --- |
| G1 | unit test in `pimple/face_consistent.rs`: `box_2d(8, 8, 1, 1)`, `p = x^2 + 3 x y - y^3` (the Green-Gauss gradient then varies from cell to cell), `rAU_c = 1 + 0.01 c`, a connectivity with two fringe cells (stencils of three cells, weights 0.5, 0.3, 0.2 and 0.6, 0.25, 0.15), one hole cell, the rest calculated: `x` and `q` of `overset_lagged_gradient` against the formula above evaluated in the test from `green_gauss` of the same `p` | largest difference `<= 1e-14` times the largest `abs(x)` (resp. `abs(q)`): rounding only |
| G1n (negative control, in the same test) | at the two fringe cells the donor interpolant differs from the cell's own value | `>= 1e-3` times the largest `abs(x)` for `x` and for `q` (else the comparison could not tell MC5 from the design) |
| G1t | the same with `trivial = true` | `x` equal to `G p_lag` and `q` to `rAU x`, bit for bit |

Mutation, run on a `git archive` copy of the final tree after G1 has passed on the tree:

| Id | Mutation | Must fail |
| --- | --- | --- |
| MC5 | in `overset_lagged_gradient`, the fringe cells keep their own `x` and `q` (the two assignments of the donor interpolants removed) | G1 |

### P. ADR 0076's seam comparisons with the opt-in

**Derivation: what "inert on coupled faces" can mean here.** The opt-in changes three expressions of the step
(`face_consistent.rs`): (1) the face coefficient `harmonic_face(rAU)` in place of `face_interpolate(rAU)`: on a boundary
face, coupled faces included, both return `rAU` of the owner cell, the same expression; (2) the added flux
`add_commutator`: a loop over `f < n_internal`, so no boundary or coupled face is written; (3) through (1), the carry-over
of the defect, a loop over internal faces too. The pressure equation's coupled coefficient `coupled_gamma` and the pair
mean of the body force read `gamma_p` on the two faces of a pair only, which (1) leaves equal. So **for the same cell
values of `rAU` and `p`, every coefficient and every flux entry of a coupled face is bit for bit the default's** with the
opt-in on. That is checked directly (P0).

It does not follow that a run on a periodic set-up is unchanged, and the derivation says it is not: the opt-in acts on
every internal face whose two cells have different `a_P / V`. The implicit part of every convection scheme of the library
is upwind (`convection.rs`, head of the file), so `a_P` carries the outflow `sum max(phi_f, 0)`, which varies with the
local speed. On TG-S (`N = 32`, `h = 0.196`, `dt = 0.05`) the time part per volume is `c0 / dt = 30`, the viscous part
`4 nu / h^2 = 10.4`, the upwind part up to about `2 abs(u) / h = 10`: the diagonal varies by about a quarter from cell to
cell. On the sector the cell sizes vary with the radius and the walls add their half-distance. So the runs with and without
the opt-in differ from the first step, at internal faces, and bit equality of the runs is not a valid check. The run-level
checks are therefore ADR 0076's own ratio limits, and the bit-equality check is at the operator level, where the
derivation says the opt-in must be inert.

Set-ups (copied, not re-invented): TG-S from `consistent_benchmarks_f301.rs` (ADR 0073's TG-S: `box_2d(32, 32, 2 pi, 2 pi)`
periodic both ways, the Taylor-Green field shifted by 0.6, `nu = 0.1`, central convection, BDF2, PISO, momentum and
pressure solves to `1e-12` relative, `1e-14` absolute, `t = 1`); SWIRL and SECT-M from `w35a_seam.rs` (`annulus_sector(16,
16, 0.5, 1, pi/3, Uniform, 0.1)`, rotational pair `theta_min`/`theta_max`, no-slip walls, `nu = 0.02`, central, BDF2, `t =
1`, the SECT-M source through the `step_with` hook; `coupled_body_pair_mean` off). The error measures are those files'
(relative L2 of the cell velocity against the exact field; pressure with the volume means removed). "C" is the same run
with `rhie_chow_step_consistent = true`, nothing else changed. The new file is
`crates/rcfd-fv/tests/seam_no_harm_f344.rs`.

| Id | Check | Limit |
| --- | --- | --- |
| P0 | unit test in `face_consistent.rs`, on `box_2d(8, 8, 1, 1)` periodic both ways and on `annulus_sector(8, 8, 0.5, 1, pi/3, Uniform, 0.1)` (rotational pair), with a varying positive `rAU` and a nonlinear `p`: `harmonic_face` and `face_interpolate` on every boundary face, `coupled_gamma` of both on every coupled face, and the flux of every boundary face after `add_lagged_gradient` (from a given flux) | bit for bit, every face |
| P0n (negative control, in the same test) | the two face coefficients on the internal faces | differ by `>= 1e-6` relative on some face |
| S3 (set-up) | TG-S default, `dt = 0.05`: the pressure and velocity errors of ADR 0076 (pressure `8.2130e-3`, final addendum; velocity `1.6869e-3 / 0.8028 = 2.101e-3` from its N1 row) | pressure in `[8.20e-3, 8.23e-3]`; velocity in `[2.09e-3, 2.11e-3]` |
| N1s (ADR 0076 N1, TG-S rows) | TG-S PISO, `dt = 0.05, 0.025, 0.0125`: the exact velocity error and the exact pressure error of C over those of the default | `<= 1.05` each (6 comparisons) |
| R4s (ADR 0076 R4) | SWIRL and SECT-M, PISO, `dt = 0.05, 0.025, 0.0125`: velocity and pressure errors of C over the default | `<= 1.05` each (12 comparisons) |
| S4 (set-up) | SECT-M PIMPLE-4 (`n_outer = 4`, `alpha = 1`) default fine-pair order (0.025, 0.0125, against the same-arm `dt = 1/1280` reference): ADR 0076's `0.538` | in `[0.52, 0.56]` |
| R2b (ADR 0076 R2b) | the same with C | in `[1.9, 2.3]` |
| R2bn (no harm, the ratio form of R2b) | the two fine-pair errors against the reference, C over the default | `<= 1.05` each |
| R4b (ADR 0080 B-R4, its PIMPLE-4 rows) | SECT-M PIMPLE-4, `dt = 0.05, 0.025, 0.0125`: velocity and pressure errors of C over the default | `<= 1.05` each (6 comparisons) |

Printed, no limit: for every pair of runs the largest difference of the cell velocities of C and the default at `t = 1`,
over the cells next to a seam and over the others; SECT-M PIMPLE-4 with C and `coupled_body_pair_mean` together (the
order).

**Predictions (mine, before any run), with the reasons.** P0 holds by construction. S3 and S4 hold (the default is
unchanged since ADR 0076). N1s, R4s, R4b: met, ratios within a few percent of 1: the opt-in changes the internal faces by
the step dependence it removes, which is small against the spatial error at these steps (on R, A3 was `1.004`). The
pressure rows of TG-S are the least sure: ADR 0076 found the default's TG-S pressure error lower than TG's because seam
errors cancel part of the spatial error, and the opt-in changes the internal faces next to the seam without changing the
seam. **R2b: missed**, order well below 1.9: the SECT-M PIMPLE-4 floor is made at the seam (ADR 0076 Y5: the seam
body-force coefficient; with F-G's seam terms and that coefficient both fixed the order was `2.02`; ADR 0080 B-R2b: with
the coefficient fixed alone `0.531`), and the opt-in does not act on the seam (L2). R2bn met.

**Reading, fixed now.** N1s, R4s, R4b and R2bn are the no-harm rows: a miss of any is a finding against the merge, kept as
an ignored `*_known_miss` test with the limit intact, and the addendum says the merge should wait. R2b is ADR 0076's
acceptance row for F-G, a row the opt-in is not built to move (L2); a miss there is recorded the same way, as a known miss
with the limit intact, and reported plainly; it shows that the opt-in leaves the seam floor of ADR 0076 in place. If S3 or
S4 is missed the set-up differs from ADR 0076's and the rows that depend on it are void, not judged.

### B (repeated on the merged tree)

The checks of B1 (amendment 1), on the merged tree, unedited: the F-344 test file; the F-322 files
`consistent_value_f322.rs` and `consistent_extensions_f322.rs`; the files that pin default results (`w33b_default_bits`,
`w33b_transient_order`, `w34b_lag`, `w34c_symmetry`, `w48v_corrector_control`, `consistent_correctors_f301`,
`consistent_defaults_f301`, `piso_thin_gap_f299`, `ale_flows`, `ale_gcl`, every `overset_*` file); the unit tests of the
crate; `cargo test -p rcfd-api --release --test baseline_hashes --test case_hashes_w48u --test consistent_defaults_case_f301`;
and `immersed_solid_f219.rs` in full (its non-ignored tests, including the two long cylinder runs addendum 1 left
unfinished; started on `10171174`, whose library files differ from the final tree only by the move of the fringe branch
into a function, which no immersed-solid run calls). Limit: pass. The ignored tests of those files are not run.

## Addendum 2: results of amendment 3 (2026-11-12)

Measured against the limits of amendment 3 (`d2ab14cd`), committed before any of its runs; none was changed. Release, Windows,
the target directory of this worktree; a full gate of another job was running on the machine throughout (the immersed-solid
file could not be run alone). Each registered run was made once. Logs in `E:/projects/cfd/w49i-work/f344-scratch`:
`s2a-immersed.log`, `s3-unit.log`, `s3-seam.log`, `mc5.log`, `s2c-fv.log`, `s2c-api.log`.

Commits after addendum 1: `aa325442` (the merge of main `2fa39daf`), `10171174` (the gate reads `cf.enabled`), `d2ab14cd`
(amendment 3), `42a22117` (the fringe branch moved into `face_consistent::overset_lagged_gradient`; G1, P0;
`seam_no_harm_f344.rs`; compiled and linted, not run), `b306c62b` (R2b as a known miss).

### Step 1: the merge

No textual conflict (amendment 3). The conflict of meaning (the gate read only the older field of the consistent correctors)
is fixed in `10171174`. The F-344 hunks against main, numbering of the final tree: `pimple.rs` line 37 (the module), 137 to
142 (the field), 857 (its default), 1670 to 1676 (the gate and the face coefficient, in place of main's line 1662), 1705 to
1708 (the call after the carry-over); `overset.rs` 965 to 971 (the face coefficient, in place of main's line 965) and 1018 to
1030 (the added flux, through `overset_lagged_gradient`). F-322's code is not touched. With the opt-in off the step is
main's statement for statement; the B rows below hold.

### Step 2

| Check | Limit | Measured | Verdict |
| --- | --- | --- | --- |
| `immersed_solid_f219.rs`, all 11 non-ignored tests (tree `10171174`) | pass | 11 passed, 2 ignored (the V1 known miss and its diagnosis), 2333 s | met |
| of those: `a1_a2_two_d_one_drag_and_lift_on_m3` | pass | passed: `C_D` 5.57833680 (error 0.0215 %), `C_L` 1.06899517e-2 (0.6686 %), orders 2.350 and 1.957 over M1 to M3 | met |
| of those: `a3_two_d_two_strouhal_number` | pass | passed: St 0.30073 (-0.382 % of 0.30188), 7 crossings, `C_D` mean 3.2135 | met |
| G1 the fringe branch against its formula | `<= 1e-14` of the largest value | `x`: 0 of 4.438; `q`: 2.2e-16 of 7.189 | met |
| G1n own value against the donor interpolant at the fringe cells | `>= 1e-3` of the largest value | `x` 0.475, `q` 0.721 | held |
| G1t trivial connectivity | bit for bit | equal | met |
| MC5 (the donor assignments removed) | G1 must fail | G1 failed: differences 0.475 and 0.721 | caught |

B rows on the merged tree (`b306c62b`), the non-ignored tests, each passed: unit tests of `rcfd-fv` 102 (5 ignored; G1 and
P0 among them); `bdf2_remainder_f344` 4 (S0, A1, C3, E1; 4 ignored); `consistent_value_f322` 5; `consistent_extensions_f322`
8; `w33b_default_bits` 1; `w33b_transient_order` 6; `w34b_lag` 3; `w34c_symmetry` 9; `w48v_corrector_control` 5;
`consistent_correctors_f301` 12; `consistent_defaults_f301` 6; `piso_thin_gap_f299` 18; `ale_flows` 5; `ale_gcl` 4;
`overset_f134` 12; `overset_f134_records` 1; `overset_f314` 4; `overset_f334` 3; `overset_f335` 5; `overset_f335_records`
4; `overset_f336` 5; `overset_f338` 9; `overset_f353` 1; `overset_f357` 5. `rcfd-api`: `baseline_hashes` 3 (1 ignored),
`case_hashes_w48u` 1, `consistent_defaults_case_f301` 3. No failure.

### Step 3: ADR 0076's seam rows with the opt-in

| Check | Limit | Measured | Verdict |
| --- | --- | --- | --- |
| P0 coupled faces, periodic box (32 coupled faces) and rotational sector (16) | bit for bit | face coefficient, coupled coefficient and flux equal on every boundary face | met |
| P0n internal faces | `>= 1e-6` relative | `0.36`; the flux of all 112 internal faces changed | held |
| S3 (set-up) TG-S default, `dt = 0.05` | velocity `[2.09e-3, 2.11e-3]`, pressure `[8.20e-3, 8.23e-3]` | `2.1011e-3`, `8.2130e-3` | held |
| N1s TG-S PISO, C over default, velocity; pressure | `<= 1.05` | dt 0.05: 0.9902; 1.0018. dt 0.025: 0.9927; 1.0012. dt 0.0125: 0.9957; 1.0006 | met (6 of 6) |
| R4s SWIRL PISO, velocity; pressure | `<= 1.05` | 1.0001; 1.0002. 1.0002; 1.0001. 1.0002; 1.0001 | met |
| R4s SECT-M PISO, velocity; pressure | `<= 1.05` | 1.0026; 0.9703. 0.9997; 0.9788. 0.9994; 0.9858 | met (12 of 12 with SWIRL) |
| R4b SECT-M PIMPLE-4, velocity; pressure | `<= 1.05` | 0.9979; 0.9751. 0.9984; 0.9805. 0.9990; 0.9864 | met (6 of 6) |
| S4 (set-up) SECT-M PIMPLE-4 default order | `[0.52, 0.56]` | `0.538` (errors 6.5924e-4, 4.5413e-4) | held |
| **R2b** SECT-M PIMPLE-4 order with C | `[1.9, 2.3]` | **`0.578`** (errors 2.4881e-4, 1.6669e-4) | **missed** (known-miss test) |
| R2bn the fine-pair errors, C over default | `<= 1.05` | `0.377`, `0.367` | met |

Printed: the largest change of the cell velocity by C is of the same size next to a seam and elsewhere (TG-S `2.9e-5`
against `5.8e-5` at `dt = 0.05`; SECT-M `1.8e-3` against `1.8e-3`): C acts through the internal faces everywhere, as
derived, and not through the seam. SECT-M PIMPLE-4 with C and `coupled_body_pair_mean` together: order `0.463`.

**Reading.** Every no-harm row of ADR 0076 that removed F-G holds with this opt-in: the TG-S pressure rows of N1, where F-G
gave 1.0952 and 1.0521, are 1.0018 and 1.0012; R4 on SECT-M, where F-G gave 1.0651, is at most 1.0026. P0 shows, bit for
bit, that the opt-in does not act on periodic or rotationally periodic faces. R2b is missed as predicted in amendment 3: the
SECT-M PIMPLE-4 floor stays (0.578 against the default's 0.538), because it is made at the rotational seam (ADR 0076 Y5,
ADR 0080 B-R2b), where the opt-in does not act; the opt-in still cuts the fine-pair errors to 0.37 of the default's. The
opt-in together with the pair mean of ADR 0080 does not lift it either (0.463), so the seam floor needs the seam terms of
F-G (ADR 0076 Y5 reached 2.02 only with them), which this opt-in leaves out by design (O2).

**Said plainly.** By the brief's instruction a miss of any of the step-3 rows means the merge should wait. R2b is missed.
It is an acceptance row of F-G that the default also misses, not a no-harm row, and every no-harm row is met; whether that
miss holds the merge is the maintainer's call and is not decided here.

### Not run, deviations

- The immersed-solid file ran on `10171174`, before the fringe branch moved into a function (no immersed-solid test calls
  it); everything else ran on `b306c62b`.
- A2 (TG_2) and the other ignored tests of the F-344 file were not run again; the move of the fringe branch is checked by
  G1 and E1, not by a repeat of A2.
- The machine was not idle (a gate of another job ran throughout).
- Q2 of addendum 1 (an opt-in of a kind ADR 0076 removed) is answered by the decision record below; Q1, Q3 to Q6 stand.

### After the tests

`cargo fmt -p rcfd-fv --check`: clean. `cargo clippy -p rcfd-fv --all-targets -- -D warnings`: clean.

### Decision by the registered rule

Unchanged: **partial**. A1 met, A2 missed (2.454 against 4 on TG_2); the defaults are bit for bit on the merged tree; all
eight mutations of the corrective are now caught (MC5 by G1).

## Decision record (2026-11-12)

The maintainer decided in person on 2026-11-12 to merge F-344 as an opt-in (`PimpleSettings::rhie_chow_step_consistent`),
limited to internal faces, not as the default. ADR 0076 stands as the record of F-G's removal and is not edited. The
reasons: the dependence of the default face flux on the step size is real and measured (addendum 1: `5.13e-6` at a ratio of
`2.30` on the mesh at rest, `3.7e-4` in the steady cavity); the opt-in is off by default; it does not act on periodic seams,
shown by the step-3 checks (P0 bit for bit, and ADR 0076's no-harm rows N1 on TG-S and R4 on the sector met).

## For the lead: roadmap entries (addendum 2)

No roadmap file was edited. **F-344**, proposed status: `partial` (A2 on TG_2 stays missed). Proposed note (it replaces the
note of addendum 1):

> From ADR 0241 question 1, recommendation taken on the maintainer's standing instruction of 2026-11-11. ADR 0245, 2026-11-12: PARTIAL. Named: the remainder of the BDF2 step at rest is the step-size dependence of the converged face flux where the momentum diagonal differs across a face (the commutator C(G p) of ADR 0073 and the step dependence of interp(rAU)), largest next to fixed-velocity boundaries. Built, opt-in, internal faces only, merged as an opt-in by the maintainer's decision in person of 2026-11-12 (not the default): PimpleSettings::rhie_chow_step_consistent (the harmonic face value of rAU and the commutator of the lagged pressure gradient, in PimpleSolver::step at rest and in the overset step); defaults bit for bit on the tree merged with main (F-322 included). Met: at rest the ratio 2.30 -> 5.02 (limit 4), the steady cavity the same to 2.4e-13 for two steps (3.7e-4 by default). Missed: D2 on TG_2, 2.454 (limit 4): the remainder of the moving coupled meshes is another term (with the component at rest 1.3e-7, ratio 4.98). ADR 0076's seam rows with the opt-in: bit-inert on periodic and rotational faces (operator level), no-harm met (TG-S N1 at most 1.0018, R4 at most 1.0026, PIMPLE-4 rows at most 0.999); R2b on SECT-M missed as predicted, 0.578 (default 0.538): the seam floor of ADR 0076, which the opt-in does not touch. Eight mutations caught (the fringe branch of the overset change by a direct check of its formula). Its body-force path is exercised by the SECT-M runs. Follow-ups proposed (ADR 0245 questions 1 and 3).

The F-338 note and the two follow-up items of addendum 1 stand as written there; in the first follow-up's note, the
sentence "The fringe branch of the F-344 opt-in in overset.rs has no failing mutation yet (ADR 0245 MC5)." should be dropped
(MC5 is now caught by G1).
