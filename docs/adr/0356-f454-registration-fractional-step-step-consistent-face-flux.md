# ADR 0356: F-454 registration: a step-consistent face flux for the fractional step

Status: **REGISTERED 2026-11-12, written before any code of F-454 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-454 was written, compiled or run: the code and documents under "What was read" were
read and the arithmetic under "Arithmetic before this registration" was done by hand. Results go in the addendum at the end;
nothing above the addendum is changed after a run.

Parents: [ADR 0352](0352-f443-registration-step-consistent-face-coefficient-remaining-paths.md) (F-443: D5, the check F0 that
measured `8.69e-2`, question 2, from which this item comes), [ADR 0245](0245-f344-registration-bdf2-step-remainder.md) (F-344:
the check C3, the corrective C and its derivation in amendment 1), [ADR 0037](0037-wave21c-wiring-fractional.md) D3 (the
fractional step, F-210). Roadmap item F-454 (depends on F-443). Number 0356, branch `f454-fractional-step-flux` from main
ce0db7d1, worktree `w49i-work/f454`, scratch `w49i-work/f454-scratch`, given by the lead.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run; a miss becomes an ignored
`*_known_miss` test with the limit intact; a defective set-up is corrected only by an amendment committed before the corrected
run; each registered run is made once; defaults bit-identical; no hash table, golden or existing test edited; no new crate, no
unsafe code; no commit trailers, no push. Clean-room: the only algorithm is derived below from first principles; the idea that a
transient momentum interpolation must carry the old face defect for the steady state not to depend on the step (Choi 1999; Yu,
Tao and others 2002; Cubero and Fueyo 2007, names and years recalled from memory, nothing opened) is the one ADR 0245 used, and
no formula is taken from those sources.

## What F-454 asks

The acceptance of `docs/roadmap.json`, binding in full:

> The steady lid-driven cavity of ADR 0245 C3 with solver.time_advance = "fractional_step" (8.7e-2 between dt = 0.04 and 0.01,
> ADR 0352 F0) is traced to its face term with a number, and either made the same to 1e-9 for the two step sizes by an opt-in
> (defaults bit-identical) or the ADR names why not with a number.

Clauses: (a1) the `8.7e-2` is traced to its face term with a number; (a2) the two steady states are made the same to `1e-9` by an
opt-in, or (a2') the ADR names why not with a number; (a3) defaults bit-identical.

**Reading (registered here).** "The cavity with `solver.time_advance = "fractional_step"`" is the set-up FR of ADR 0352 (the
library's fractional step, which that case key selects, `sim.rs` 1984 to 1990): `cavity_2d(20, 1.0)`, `nu = 0.1`, lid `(1, 0,
0)`, the settings PS, `FractionalSettings::default()` (incremental, Kim-Moin boundary), implicit Euler, from rest to `t = 20` in
steps of `0.04` and `0.01`. "The same to `1e-9`" is the largest difference of the cell velocities (lid speed 1), as in C3.
"Traced to its face term with a number" is read as in ADR 0245 reading 3: the term written as a formula, a measurement that it
is the face defect of the steady state (T1), and a measurement in which changing that term alone removes the difference (A1).
An opt-in in the library meets (a2); a case key is not part of the acceptance (see O3).

## What was read (main ce0db7d1, read only)

1. ADR 0352 in full (D5, F0, F1, Q2 and the roadmap text); ADR 0245 registration, amendment 1 (the derivation of C) and addendum
   1 (C3 `2.4e-13`, C3n `3.75e-4`, MC2 `9.4e-5`).
2. `crates/rcfd-fv/src/pimple/fractional.rs` in full (276 lines); in `crates/rcfd-fv/src/pimple.rs`: the field documentation
   of `rhie_chow_step_consistent` (137 to 144), `piso()` (820 to 861), `step_with` lines 1340 to 1377 (the carried defects
   `ddt_defect`) and 1600 to 1711 (`a_p`, `r_au`, the choice of `gamma_p`, the carry-over); `pimple/face_consistent.rs` lines
   1 to 80 (`harmonic_face`).
3. `crates/rcfd-fv/tests/step_consistent_paths_f443.rs` in full (the set-ups CAV and FR, F1 and F0); the uses of
   `FractionalSettings { .. }` as a struct literal (`sim.rs` 1986, `tests/fractional_step.rs` 25, 346, 444): a new field there
   would break unedited code, so the switch goes into `PimpleSettings` (whose struct literals are only in `pimple.rs`).

What the code shows. One fractional step (incremental form) is: the predictor `M u* = b - V G p^n` with `M` the momentum matrix
(convection by the old flux, diffusion, the time term `c0 V / dt` on the diagonal); the intermediate flux on an internal face

`phi*_f = interp(u*)_f . S_f + dte R_f(p^n)`, `R_f(p) = interp(G p)_f . S_f - snGrad_f(p) |S_f|`, `dte = dt / c0`;

one Poisson solve for the increment `q` with the uniform face coefficient `dte`; `phi = phi* - dte snGrad(q) |S|`, `u = u* - dte
G q`, `p^{n+1} = p^n + q`. There is no carried defect (ADR 0352 D5). Boundary faces: the prescribed flux on fixed-velocity faces
(every face of the cavity).

## Arithmetic before this registration (disclosed, by hand)

**D1. The face term (a1).** In a steady state of the step `u^{n+1} = u^n = u`, `p^{n+1} = p^n = p`, so `q = 0`, `u* = u` and
`phi = phi*`. The steady cell equations are `M u = b - V G p` with the time term cancelling against the old level: `A u - H(u) =
-V G p` with `A` the non-temporal part, which does not contain `dt` (the convecting flux is the steady `phi`; the Kim-Moin shift
`dte (G q_lag)_t` of the boundary values is zero with `q_lag = 0`). Continuity is `sum_f phi_f = 0` with

`phi_f = interp(u)_f . S_f + d_f`, `d_f = dte R_f(p)`.

So the only `dt` in the steady equations is the face defect `d = dte R(p)`: of first order in the step, a factor 4 between
`dt = 0.04` and `0.01` at equal `p`. That is the face term; T1 measures that the steady face defect `phi - interp(u) . S` of the
default runs is `dte R(p)` of their own final pressure, and prints its size and its ratio between the two steps.

Size. With `a = 4 nu / h^2 = 160` (inner cells, `h = 0.05`) the steady PISO step (ADR 0245 C) has the defect `R / a_f`, about
`6.3e-3 R`; the fractional step has `0.04 R` and `0.01 R`: its pressure smoothing is `6.4` and `1.6` times C's. A change of the
smoothing coefficient by a factor 4 changes the steady flux at the order of the defect itself, so a velocity difference of
several percent is what one expects; `8.7e-2` fits. In the PISO step the default coefficient changes much less (`interp(rAU)` of
`dt / (1 + a dt)`, `5.4e-3` and `3.8e-3`), hence its `3.75e-4`.

**D2. The opt-in F (incremental form).** On internal faces the intermediate flux becomes

`phi*_f = interp(u*)_f . S_f + Gamma_f R_f(p^n) + (Gamma_f / dt) K_f`, `K_f = c1 d^n_f - c2 d^{n-1}_f`, `d^m = phi^m - interp(u^m) . S`,

with `Gamma_f = 1 / interp(a_P / V)_f` (the harmonic face value of `rAU = V / a_P`, `face_consistent::harmonic_face`), `a_P` the
mean over the active components of the predictor's diagonal (the time term included), and `(c1, c2)` the history weights the
step already takes (`ddt_history_weights`; `(1, 0)` for implicit Euler). Everything else in the step is unchanged: the predictor,
the Poisson solve and the projection keep `dte` (the increment vanishes in a steady state, so its coefficient does not enter it;
O1). Boundary faces keep the default expression.

Steady state: `d = Gamma R + (Gamma / dt) c0 d` with `c0 = c1 - c2`, so `d (1 / Gamma - c0 / dt) = R`, and `1 / Gamma - c0 / dt =
interp(a_P / V) - c0 / dt = interp(a)`, `a` the non-temporal diagonal per volume (the time term `c0 / dt` is the same in both
cells and passes through the linear interpolation). So `d = R(p) / interp(a)` with no `dt`: the steady equations of the step with
F contain no step size, and the steady states for two steps are the same up to the linear solves and the approach to steady.
They are the steady equations of the PISO step with C (ADR 0245 amendment 1: `d = R / a_f`, the same cell equations, the same
`a`), which X1 checks.

Transient: the carried factor is `c0 Gamma / dt = c0 / (c0 + a_f dt) < 1` (`0.135` at `dt = 0.04`, `0.385` at `0.01` for inner
faces), so the carried defect decays as in the PISO step; after the projection the defect is `Gamma R(p^n) + (Gamma / dt) K -
dte R(q)`, the last term zero in a steady state.

**D3. The Chorin form: why not.** Without `p^n` in the predictor, a steady state has `u* = u + dte G p`, and the predictor's
steady equation is `V (u* - u) / dt + A u* - H(u*) = 0`: the cell equations themselves carry `dte` (a splitting error of first
order), not only the face. A face term cannot remove that; F does not act in the Chorin form (the flag is not read there, D1n
bit for bit) and its own step dependence is printed (P1) as the number.

**D4. Cost.** FR: one predictor (two components) and one Poisson solve per step on 400 cells, 500 and 2000 steps: seconds. The
PISO run of X1 (BDF2, three outer iterations, 2000 steps): about 12 s (ADR 0352: K1's five runs in 62 s). All checks in one
gate-run test file, well under two minutes.

## Design (registered)

- **L1** (`crates/rcfd-fv/src/pimple.rs`): a new field `PimpleSettings::fractional_step_consistent: bool`, default `false` in
  `piso()` (and so in `pimple()`), with its documentation; the documentation of `rhie_chow_step_consistent` points to it. Not
  read by `step_with`.
- **L2** (`crates/rcfd-fv/src/pimple/fractional.rs`): with `sc = s.fractional_step_consistent && fs.incremental`: the predictor
  collects `a_P` (the mean of the component diagonals before the solve); the carried defects `K` are formed from `phi_hist` and
  `hist_u` as in `step_with` (internal faces, zero boundary velocity); on internal faces the flux takes `Gamma_f R_f + (Gamma_f
  / dt) K_f` in place of `dte R_f`. With `sc` false every statement is the present one (the new expressions sit in branches taken
  only with `sc`). Boundary and coupled faces unchanged.
- No case-tree key (O3), no `rcfd-api` change, no golden, no hash table.

## Set-ups (registered)

PS is ADR 0245's `settings(nu)`: `PimpleSettings::piso(nu)`, central convection, momentum and pressure solves to `1e-12`
relative, `1e-15` absolute, 3000 iterations. New test file `crates/rcfd-fv/tests/fractional_step_consistent_f454.rs`; helpers
copied from `step_consistent_paths_f443.rs` (not edited).

- **FR** (ADR 0352): CAV (`cavity_2d(20, 1.0)`, `nu = 0.1`, lid `(1, 0, 0)`, walls at rest, PS, `n_outer = 3`, which the
  fractional step does not read) with `fractional = Some(FractionalSettings::default())`, implicit Euler, from rest to `t = 20`
  in steps of `0.04` (500) and `0.01` (2000). **FR-F**: FR with `fractional_step_consistent = true`. **FR-P**: FR with `kim_moin
  = false` (plain boundary). **FR-C**: FR with `incremental = false` (Chorin). **FR-B**: FR with BDF2.
- **CAVC**: ADR 0245 C3: CAV with BDF2 and `rhie_chow_step_consistent = true` (the PISO step), `dt = 0.01`, to `t = 20`.

## Registered checks

### A. Acceptance

| Id | Clause | Check | Limit |
| --- | --- | --- | --- |
| S0 (set-up) | | FR default, `0.04` against `0.01`: the largest difference of the cell velocities (ADR 0352 F0, `8.69e-2`) | in `[8.6e-2, 8.8e-2]` |
| T1 | a1 | the two runs of S0: over internal faces, `max abs(d_f - dte R_f(p))` over `max abs(dte R_f(p))`, with `d = phi - interp(u) . S` and `R` of the final `p` (the Green-Gauss gradient with the step's pressure boundary values) | `<= 1e-6` each |
| T1p | a1 | printed: `max abs(d_f) / abs(S_f)` of both runs and their ratio (D1: near 4) | printed |
| A1 | a2, a1 | FR-F, `0.04` against `0.01`: the largest difference of the cell velocities | `<= 1e-9` |
| A1d | | continuity, every step of the A1 runs | `<= 1e-9` |
| A2 | a2 | FR-F with FR-P's plain boundary, `0.04` against `0.01` | `<= 1e-9` |
| X1 | a2 | FR-F at `0.01` against CAVC at `0.01` (the same steady equations, D2) | `<= 1e-9` |
| X1n (negative control) | | FR (default) at `0.01` against CAVC at `0.01` | `>= 1e-6` |
| D0 | a3 | a fingerprint (FNV-1a of the bits of `u`, `p`, `phi`) of FR default after 100 steps of `0.04`, and of CAV with BDF2 (PISO) after 50 steps of `0.04`: on a `git archive` copy of main ce0db7d1 with the new test file's D0 test alone (it uses only fields main has), and on the final tree | equal before and after |
| D1 | a3 | FR with `fractional_step_consistent = true` against `false` with `incremental = false` (Chorin), 100 steps of `0.04`: `u`, `p`, `phi` | bit for bit |
| D2 | a3 | CAV with BDF2 (PISO step) with the new flag against without, 50 steps of `0.04` | bit for bit |
| E0 | a3 | unedited, release: `rcfd-fv` `--lib`, `fractional_step`, `step_consistent_paths_f443` (non-ignored; its F1 shows the old flag still not read by the fractional step), `bdf2_remainder_f344` (non-ignored), `w33b_default_bits`; `rcfd-api` `baseline_hashes` (release) | pass |

The `1e-9` limits: C3 of ADR 0245 reached `2.4e-13` with the same solves; the fractional step solves once per step and the
approach to steady at `t = 20` is `e^{-40}` of the start (slowest viscous decay `2 pi^2 nu`, about 2); estimate `1e-12`. T1's
`1e-6`: the residual of the identity is `2 dte R(q)` with `q` the last increment, near the solve tolerance.

Printed for the record, no limit: the change of the velocity over the last step of every run; P1: FR-C default `0.04` against
`0.01` (the Chorin form's own dependence, D3); FR-B with and without F, `0.04` against `0.01` (BDF2 at a Courant number of `0.8`
with a lagged convecting flux: its stability is not established here, so no limit).

### Mutations (scratch copy `w49i-work/f454-scratch`, not committed)

Each applied alone to a `git archive` copy of the final tree by an exact replacement that must occur once; the named test is run
once and must fail (a stopped run fails its test); the file is restored and compared byte for byte.

| Id | Mutation | Must fail |
| --- | --- | --- |
| MF1 | the carried defect left out (`K = 0`, `Gamma` kept) | A1 |
| MF2 | the face coefficient `dte` in place of `Gamma` (the carry-over kept) | A1 |
| MF3 | the face coefficient `face_interpolate(rAU)` (linear) in place of the harmonic value | A1 |
| MF4 | the gate inverted (`!s.fractional_step_consistent`) | S0 |
| MF5 | the gate without `fs.incremental` (F acts in the Chorin form) | D1 |

`scripts/limit_audit.py` is run on the new test file and every hit is read.

## Decision rule

- **done**: S0 held; T1 met; A1, A1d, A2, X1 met; X1n held; D0, D1, D2 and E0 met; every mutation failed its check without an
  amendment after a run.
- **partial**: T1 met and the face term so traced, but A1, A2 or X1 missed (a `*_known_miss` with the cause named with a number,
  which is then the "why not" of a2'); or any registered check missed, a control not held, a mutation not caught, or a mutation
  caught only after a post-run amendment; or E0 not shown in full.
- **todo**: T1 missed (the face term not traced) and A1 missed.

## Open choices taken

Each taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation); none is a decision the maintainer
made in person; none changes a default.

- **O1. The projection's coefficient with F.** (a) keep `dte` in the Poisson solve and the projection (the increment vanishes in a
  steady state; the non-iterative step keeps its one solve with a uniform coefficient); (b) take `Gamma` there too (a PISO-like
  corrector). Taken (a): it changes the step least, and D2 shows the steady state does not see the choice.
- **O2. The switch.** (a) a new field `PimpleSettings::fractional_step_consistent`; (b) read `rhie_chow_step_consistent` in the
  fractional step; (c) a field in `FractionalSettings`. Taken (a): (b) would break ADR 0352 F1 (an existing test: the old flag
  must not change the fractional step), (c) breaks three unedited struct literals.
- **O3. A case-tree key.** Not part of the acceptance. ADR 0352 makes `[solver.rhie_chow] step_consistent` with
  `fractional_step` a located error and tests it (T2 there); letting that key set F would change that tested behaviour. Not taken;
  listed as an open question in the addendum.

## Limits of this registration

- L1. A uniform Cartesian mesh, fixed-velocity walls, central convection, one viscosity, implicit Euler (BDF2 printed only).
- L2. Boundary and coupled faces keep `dte R` (none on the cavity carries a pressure term); pressure (outlet) boundaries and
  periodic faces with F are not covered, and a steady state with an outlet still has `dte R` on those faces.
- L3. No body force (the fractional step has none); the Chorin form is not changed (D3).
- L4. The transient accuracy of F is not measured; only the steady state and its independence of the step.

## Open questions

Listed with options and a recommendation in the addendum, after the results.
