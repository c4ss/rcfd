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
