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

## Addendum 1: results (2026-11-12)

Measured against the limits registered above (`17053005`); none was changed. The library change and the test file were committed
compiled and not run (`0993aa4c`); every registered run was then made once, release, Windows, this worktree's target directory
(`fractional_step_consistent_f454`, three test threads, 30 s). D0 "before" ran on a `git archive` copy of main ce0db7d1 with
the D0 test alone (`f454-scratch/main`). Logs in `E:/projects/cfd/w49i-work/f454-scratch` (`f454-run.log`, `d0-before.log`,
`e0-fv.log`, `e0-api.log`, `f454-MF*.log`, `mut-summary.log`).

**Said plainly.** The fractional step's `8.7e-2` is its face term: the steady face defect of the default step is `dte R(p)` to
`6.5e-16` (relative), a pressure smoothing of first order in the step and absent from the cell equations. With the opt-in
`fractional_step_consistent` (the harmonic face coefficient of `V / a_P` and the carried old face defect in the intermediate
flux) the two steady cavities agree to `1.5e-13`, and they agree with the PISO step's steady cavity with ADR 0245's C to
`1.6e-13`: one steady discretisation for both steps. Defaults bit for bit. One registered mutation (MF5) was not caught: it is
equivalent by construction (below), and by the registered rule that makes the item partial.

### What was built

- `crates/rcfd-fv/src/pimple.rs` (final numbering): 143 to 144 (the documentation of `rhie_chow_step_consistent` points to the
  new field), 146 to 152 (the field `fractional_step_consistent` and its documentation), 868 (its default `false` in `piso()`).
  `step_with` is not touched.
- `crates/rcfd-fv/src/pimple/fractional.rs`: 13 to 15 (module documentation), 62 to 83 (`carried_defect`, the carried old
  defects, the expression of `step_with`'s `ddt_defect`), 119 to 121 (the gate `sc`), 198 to 202 (the predictor diagonals), 215
  to 225 (`Gamma` and `K`), 236 to 241 (the internal-face flux with `sc`; the default statement unchanged in the `else`).
- Test: `crates/rcfd-fv/tests/fractional_step_consistent_f454.rs` (3 gate-run tests, 30 s together). No existing test, hash
  table, golden or `rcfd-api` file edited.

### Checks

| Check | Limit | Measured | Verdict |
| --- | --- | --- | --- |
| S0 FR default, `0.04` against `0.01` | `[8.6e-2, 8.8e-2]` | `8.6856e-2` | held |
| **T1** steady face defect against `dte R(p)`, relative | `<= 1e-6` each | `4.0e-16` (`0.04`), `6.5e-16` (`0.01`) | met |
| T1p (printed) `max abs(d) / abs(S)` | none | `0.1984` (`0.04`), `0.1208` (`0.01`), ratio `1.64` | printed |
| **A1** FR-F, `0.04` against `0.01` | `<= 1e-9` | `1.55e-13` (change over the last step below `1e-15`) | met |
| A1d continuity | `<= 1e-9` | `2.85e-12` | met |
| A2 FR-F with the plain boundary | `<= 1e-9` | `1.19e-13` | met |
| **X1** FR-F against CAVC (PISO with C), `0.01` | `<= 1e-9` | `1.62e-13` | met |
| X1n (control) FR default against CAVC | `>= 1e-6` | `4.36e-2` | held |
| D0 fingerprints before (main) and after | equal | fractional `efb9b0f6f209a81b`, PISO `b1c6b772eb162cb4`, both | met |
| D1 Chorin form, flag on against off, 100 steps | bit for bit | equal | met |
| D2 PISO step, flag on against off, 50 steps | bit for bit | equal | met |
| E0 `rcfd-fv` `--lib` 104 (6 ignored), `fractional_step` 13 (3), `step_consistent_paths_f443` 5 (2; F1 among them), `bdf2_remainder_f344` 4 (4), `w33b_default_bits` 1; `rcfd-api` `baseline_hashes` 3 (1) | pass | all passed, none edited | met |

Printed: P1, the Chorin form, `0.04` against `0.01`: `3.71e-2` (D3: its cell equations carry `dte`; F does not act there). FR-B
(BDF2): default `8.07e-2`, with F `1.09e-13` (stable at both steps; BDF2 is covered by the derivation and measured here, but
carried no limit). With F the steady defect is `max abs(d) / abs(S) = 0.0805` at both steps, between the default's two values.

**The trace (a1), with the numbers.** In a steady state of the default step the increment is zero, the cell equations contain no
`dt`, and the face defect is `dte R(p)` (T1 to `6.5e-16`): that is the only place the step enters, so it is the term. Changing it
alone (A1) takes the difference from `8.69e-2` to `1.5e-13`. D1's prediction of a ratio near 4 for T1p was wrong: the steady
pressure adapts to the smoothing, so `R(p)` itself is larger at the smaller step (ratio `1.64`, printed only).

### Mutations

Applied alone to a `git archive` copy of `0993aa4c` (`f454-scratch/mut`) by `f454-mut-run.py` (an exact replacement that occurs
once), the named test run once, the file restored and compared byte for byte.

| Id | Must fail | Result |
| --- | --- | --- |
| MF1 the carried defect left out | A1 | failed: A1 `1.33e-2`, X1 `1.88e-2` |
| MF2 `dte` in place of `Gamma` | A1 | failed: A1 `9.4e-5`, X1 `0.54` |
| MF3 the linear `face_interpolate(rAU)` | A1 | failed: A1 `7.6e-5`, X1 `2.5e-4` (ADR 0245 MC2 gave `9.4e-5` on the PISO step) |
| MF4 the gate inverted | S0 | failed: S0 `1.5e-13`, T1 `0.89` |
| MF5 the gate without `fs.incremental` | D1 | **not caught**: D1 bit for bit |

MF5 is an equivalent mutation: the changed flux sits inside the existing `if fs.incremental` block of the intermediate flux, so in
the Chorin form `Gamma` and `K` are computed and never used, and no test can see the gate's second conjunct. The registration
did not see this. By the registered rule a mutation not caught makes the item partial; nothing was amended after the run.
`scripts/limit_audit.py` on the new test file: 0 hits.

### Deviations, not run

- E0 ran in one `cargo test` process while the mutation copy built and ran.
- Not run: the workspace suite, the gate, Linux, ignored tests; `rcfd-api` tests other than `baseline_hashes` (no `rcfd-api` file
  changed; a new `PimpleSettings` field is set only through `piso()`).
- `cargo fmt -p rcfd-fv --check`: clean. `cargo clippy --release -p rcfd-fv --all-targets -- -D warnings`: clean.

### Decision by the registered rule

**Partial.** S0 held, T1, A1, A1d, A2, X1, D0 to D2 and E0 met, X1n held, MF1 to MF4 caught; MF5 not caught (equivalent by
construction). Every clause of the acceptance is met in its first way: traced (T1), the same to `1.5e-13` by an opt-in, defaults
bit for bit.

### Open questions (each with options and a recommendation)

- **Q1. MF5.** (a) keep partial as the rule says; (b) the lead reads an equivalent mutation as not a miss and takes done.
  Recommendation (a) by the rule, with the note that every acceptance clause is met; a later item may drop the redundant
  conjunct or register a mutation that removes the outer `if fs.incremental` (a lead's call).
- **Q2. A case key.** (a) let `[solver.rhie_chow] step_consistent` with `fractional_step` set the new field (changes ADR 0352's
  tested located error, T2 there: needs the maintainer); (b) a new optional key for the fractional step; (c) none for now.
  Recommendation (b) as a follow-up when a case needs it; not taken (it touches the case tree).
- **Q3. F by default** for the fractional step: changes every fractional-step result and hash. Open for the maintainer; not taken.
- **Q4. Outlets and periodic faces** keep `dte R` (L2). (a) a follow-up with a C3-type check on a channel with an outlet; (b) not
  now. Recommendation (b).

Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation), not decisions the maintainer made in
person: O1 to O3 of the registration.

### For the lead: roadmap entries

No roadmap file was edited. **F-454**, proposed status: `partial` (by the rule: one equivalent mutation not caught; every
acceptance clause met). Proposed note:

> From ADR 0352 question 2, recommendation (a). ADR 0356, 2026-11-12: PARTIAL by the rule (every acceptance clause met; mutation MF5 equivalent by construction, not caught). The fractional step's 8.69e-2 between dt 0.04 and 0.01 is its face term: in a steady state the increment vanishes, the cell equations carry no step, and the face defect is dte R(p) (measured to 6.5e-16 relative), a first-order pressure smoothing. The opt-in PimpleSettings::fractional_step_consistent (incremental form; default false) takes on internal faces Gamma R(p^n) + (Gamma/dt)(c1 d^n - c2 d^{n-1}) with Gamma the harmonic face value of V/a_P of the predictor, so the steady defect is R(p)/interp(a): the steady cavity is then the same to 1.5e-13 for the two steps (1.2e-13 with the plain boundary, 1.1e-13 with BDF2, printed) and equal to the PISO step's with ADR 0245's C to 1.6e-13 (4.4e-2 without). Defaults bit for bit (fingerprints before and after equal; the rcfd-fv suites touching the changed lines and baseline_hashes unedited). The Chorin form is not changed (its cell equations carry dt; 3.7e-2 printed). No case key; outlets and periodic faces keep dte R. Four of five mutations caught.

**F-443**: no change to its note is needed; its sentence on the fractional step stays true for `rhie_chow_step_consistent`.
