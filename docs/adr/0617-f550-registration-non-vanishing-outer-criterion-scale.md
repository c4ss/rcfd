# ADR 0617: F-550 registration: a non-vanishing scale for the outer-iteration criterion

Status: **REGISTERED 2026-11-15, written before any code of F-550 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-550 was written, compiled or run: the files under "What was read" were read and the
arithmetic below was done by hand (no script, no run). Results go in the addendum at the end; nothing above the addendum is
changed after a run.

Parents: [ADR 0458](0458-f536-registration-outer-iteration-criterion.md) (F-536: the opt-in criterion
`PimpleSettings::outer_pressure_tolerance`, its record `PimpleSolver::outer_record`, the settings K, the channel runs FCk and QFk,
and the K1 miss diagnosed at the extremes of the pressure history; remaining questions 3 and 4 name this item),
[ADR 0451](0451-f530-registration-piso-stability-at-large-diffusion-numbers.md) (F-530: the fixed-mesh channel FC, the fast
channel FQ, the discrete model `Z_dt`, the formula, the cycle rule), [ADR 0073](0073-wave34b-transient-floor-pimple-lag.md) (the
pressure-lag estimate B4). Roadmap item F-550 (deps F-536). Number 0617 given by the lead. Branch `f550-work` from `b61032f4`
(main), worktree `w49i-work/f550`, scratch `w49i-work/f550-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number); each registered run made once; defaults bit-identical; no
edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; existing tests and ignored records not
edited; no new dependency, no unsafe code; clean-room (the scale is derived below from the fixed-point contraction of ADR 0458 C1
and the shape of a sinusoidal pressure history; nothing is taken from literature or another code). The roadmap files are not
edited. Every choice the maintainer would normally be asked about is taken on the maintainer's standing instruction of
2026-11-11 (take the recommendation) and marked so.

## What F-550 asks

The acceptance, verbatim, binding:

> Every limit not stated here is registered in the ADR before the run. The opt-in outer-iteration criterion of ADR 0458 gets a scale that does not vanish at the extremes of the pressure history (for example the largest first-iteration pressure change of recent steps, or an absolute floor tied to the linear solvers' tolerances), derived in the ADR, and the pressure-lag estimate of a step with the criterion uses the count the step used; on the fixed-mesh channel of ADR 0451 at nu dt/dy^2 = 25, 50 and 100 every step meets the criterion before the cap, every level matches the discrete implicit-Euler impedance within 1 percent and the extrapolation the oscillating-channel impedance within 3 percent, with the outer iterations per step recorded and no more in total than with the scale of ADR 0458; the default results of every existing case stay bit-identical.

Clauses: (c0) every other limit registered here; (c1) a scale that does not vanish at the extremes, derived; (c2) the B4 estimate
of a step with the criterion uses the count the step used; (c3) FC at `D = 25, 50, 100` (`N = 100, 50, 25` steps per period):
every step meets the criterion before the cap; (c4) every level within 1 percent of `Z_dt` (each part, as ADR 0458 A3k); (c5)
`R_c = 2 Z_100 - Z_50` within 3 percent of the formula (each part, as A4k); (c6) the outer iterations per step recorded; (c7) the
total outer iterations of each level no more than with ADR 0458's scale; (c8) the default results bit-identical. The brief adds:
the new scale is an option of the criterion, ADR 0458's scale unchanged when it is not selected; no case-file key.

## What was read (no Rust run)

ADR 0458 (whole, with its addendum: the records of FCk per level, mean / min / max and total of outer iterations: `N = 25`:
`14.36 / 11 / 50`, total **3590**; `N = 50`: `11.08 / 8 / 50`, total **2769**; `N = 100`: `8.12 / 7 / 33`, total **2436**; the
diagnosis: the capped steps are steps 23 and 48 of every period of 50, where the step's own pressure change is `4.4e-4` of its
largest value). `crates/rcfd-fv/src/pimple.rs`: `PimpleSettings::outer_pressure_tolerance`, `OuterRecord`, the outer loop of
`step_with` (the ratio `c_k`, `outer_first`, `final_next`, the record), `LagCtx` (constructed in two places only, both in the outer
loop on the final iteration: the B4 check of the step and the adaptive corrector count `pick_correctors`), `lag_amplification_n`,
`lag_map` (`for outer in 0..s.n_outer`, the cap's count), `lag_preflight`/`preflight_run`/`lag_trial` (a copy of the solver takes
a step, so they go through the same loop), `scratch`, `new`, `reinitialise`. The test files of ADR 0458
(`crates/rcfd-fv/tests/outer_criterion_f536.rs`, `crates/rcfd-post/tests/piso_outer_criterion_f536.rs`). `OuterRecord` is
constructed by struct literal only in `pimple.rs`.

**Arithmetic before this registration (disclosed).** By hand: the windowed scale's lower bound for a sinusoidal history
(below), the window for the finest level, the error bound of the windowed criterion.

## Design

### S1. Why ADR 0458's scale vanishes

ADR 0458 C1 measures the change of outer iteration `k` against `d_1 = max_c |p_1 - p^n|`, the change of the step's first outer
iteration. Near the fixed point `p*` of the step, `d_1 = (1 - r) |p^n - p*|`, and `p^n - p*` is (to first order in `dt`) the
pressure change of the step. On a periodic history `p(t) = P sin(w t + phi)` the step change is `P w dt |cos(w t + phi)|`, which
vanishes twice a period, at the extremes of `p`. There the criterion asks for an absolute change `tol d_1` that goes to zero with
`cos`, so the count grows like `ln(1/|cos|)/ln(1/r)` and reaches the cap (ADR 0458's diagnosis: `|cos| ~ 4.4e-4`, 49 iterations).

### S2. The scale (derived)

What the criterion protects is the accuracy of the response over a period (the impedance is a projection of the history on the
first harmonic), not the relative accuracy of every step. Write `E_1(m)` for the iteration error the non-iterated step `m` would
leave (`|E_1| = r |p^m - p*|`) and `E_K(m)` for the error left after the step's final iteration. ADR 0458 C1 gives, for a step that
meets `c_k <= tol` and runs one final iteration, `|E_K| <= r^2 tol S / (1 - r)` when the ratio is formed against a scale `S`
(from `dp_k = (r - 1) e_{k-1}`, `e_{k+1} = r^2 e_{k-1}`). With `S = d_1(m) = (1 - r)|p^m - p*|` this is ADR 0458's `r tol |E_1(m)|`.

**The windowed scale.** `S_m = max(d_1(m), d_1(m - 1), ..., d_1(m - W + 1))`: the largest first-iteration change of the step and
the `W - 1` completed steps before it. `W = 1` is ADR 0458's scale. For every `W`:

* `S_m >= d_1(m)`, so the ratio is at most ADR 0458's and the criterion is met no later than ADR 0458's on the same step
  (the count per step is not increased; c7 to first order, the trajectories differ only by the iteration error);
* `S_m <= max over the window of d_1`, so `|E_K(m)| <= r^2 tol S_m/(1 - r) <= r tol max_{window} |E_1|`: the iteration error left
  in a step is at most `r tol` of the **largest** non-iterated error of the recent steps, the bound of C1 taken over the window
  instead of the step. This is the quantity a projection over the period sees.

**It does not vanish at the extremes (c1).** For `d_1(m) = d |cos(theta_m)|`, `theta_m = 2 pi m/N` (`N` steps per period), the
trailing window covers the phase `L = 2 pi (W - 1)/N`, and `S_m/d = max(|cos theta|, |cos(theta - L)|)` (`|cos|` is monotone
between an extreme and the next zero of `sin`, so the largest value in the window sits at one of its ends while `L <= pi/2`). Its
minimum over `theta` is where the two ends are equal, at `theta = pi/2 + L/2`: `min_m S_m/d = sin(L/2) = sin(pi (W - 1)/N)`, and 1
once `L >= pi`. So the scale is bounded below by a fixed fraction of the period's largest first change, independent of where the
extremes fall; ADR 0458's `W = 1` gives `sin 0 = 0`.

**The window (registered).** Require the floor `sin(pi (W - 1)/N) >= 1/2` at the finest level `N = 100` (the most steps per
period, hence the smallest phase per step): `W - 1 >= N/6 = 16.7`, **`W = 18`**. Then the floor is `sin(0.534) = 0.509` at
`N = 100`, `sin(1.068) = 0.876` at `N = 50`, and 1 at `N = 25` (`L = 4.27 > pi`). The cost of the floor at the worst step is at
most `ln 2 / ln(1/r)` outer iterations more than a mid-period step (one at `r = 0.5`), against ADR 0458's 30 to 40 more.

**Accuracy (c4, c5).** ADR 0458 C1 estimated the iteration error of a part at most `2.7 tol` of the part, assuming each step's
error at most `r tol |E_1(m)|`. With S2 the per-step bound is `r tol max_window |E_1|`; for a sinusoidal `E_1` history the
largest value is `sqrt 2` times the RMS, which is the amplitude of the first harmonic the impedance sees, so the estimate becomes
at most `sqrt 2 x 2.7 tol = 3.8e-3` with `tol = 1e-3`. With the converged step's own distance from `Z_dt` (ADR 0458 A3k: real
`+5.3e-3` to `+5.7e-3`, imaginary `-2.8e-3` to `-2.5e-4`) at most about `9.5e-3` against `1e-2` (real part, `N = 100`); the
measured iteration error of ADR 0458 is far smaller than its bound (the `N = 100` real part moved by `2e-4` between 8 fixed and the
criterion), so the margin is in practice larger. `R_c`: ADR 0458 `+6.06e-3`, `+3.77e-3`; the same bound adds at most `3 x 3.8e-3`
(the extrapolation's weights `2, -1`), well within `3e-2`.

**The start.** From rest, the first step's change is the jump to the periodic pressure, larger than a step change (by up to about
`N/(2 pi)` times). For the next `W - 1` steps the scale is then larger and the criterion looser; the error made decays with the
start transient (the cycle rule runs at least 3 periods and only the last are projected). Recorded as a limit.

**Rejected alternative: an absolute floor.** A floor tied to the linear solvers' tolerances would bound the change below which
the outer iteration only sees the solver's own error. The pressure solver stops on a residual (`tol_rel |b|`, `tol_abs`); the
pressure change that residual allows is `|A^{-1}|` times it, and that norm is not available in the loop without a further
estimate. A floor in pressure units would be a case-dependent absolute. The window needs neither; the floor is listed as not
built (Limits).

### S3. The count of the pressure-lag estimate (c2)

`LagCtx` gains the field `outer: usize`, the outer iterations of the step it was captured in, set to `outer + 1` where it is built
(both places are inside `if last` on the final iteration). `lag_map` runs `ctx.outer` outer passes instead of `s.n_outer`. Without
the criterion the final iteration is the `n_outer`-th, so `outer + 1 == s.n_outer` and the estimate is the same bits (c8). With the
criterion the B4 check of the step (`pressure_lag_check`) and the adaptive corrector count (`corrector_control`, chosen on the
frozen matrices of the final iteration before its correctors) both use the count the step uses; `lag_preflight` and `lag_trial`
go through a step and inherit it.

### S4. The library change (opt-in)

* `PimpleSettings::outer_scale_window: Option<usize>`, default `None` (in `piso`, hence `pimple`). Read only with
  `outer_pressure_tolerance = Some(_)`. `None`: ADR 0458's scale, the code path unchanged. `Some(w)`: the scale S2 with `W =
  max(w, 1)`. Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation): an `Option<usize>` field
  rather than an enum of scales, since the floor is not built.
* `OuterRecord` gains `first` (`d_1` of the step, NaN when no ratio was formed) and `scale` (the denominator of the ratio, equal to
  `first` with `None`).
* A private `PimpleSolver` history of the `d_1` of the last completed steps (pushed with the record, only with the criterion,
  trimmed to `W - 1` entries; a NaN `d_1` is not pushed). `new` starts it empty, `scratch` copies it (it is state of the history,
  like `hist_u`), `reinitialise` clears it.
* In the loop: after the first outer iteration `S = d_1` with `None` (the present line) or the windowed maximum with `Some`; the
  ratio `change/S` with the present rules for `S = 0`.
* `LagCtx::outer` and `lag_map` as S3.
* No case key: `solver.outer_scale_window` would change the schema golden (house rule 3); open question.

## Set-ups

FC, FQ, the inflow, the force, `periodic_response`, `model_zdt`, `impedance_formula`, the cycle rule (B4 of the fixed 8 of ADR
0451's `outer_settings()`: `C = 10, 5, 3` on FC, `6` on FQ at `N = 16`) and boundedness exactly as ADR 0458 (copied into a new
file `crates/rcfd-post/tests/piso_outer_scale_f550.rs`; the ADR 0458 file is not edited).

**The settings KW.** ADR 0458's settings K (`settings()` of ADR 0451 with the consistent correctors, `n_outer = 50` the cap,
`outer_pressure_tolerance = Some(1e-3)`) with `outer_scale_window = Some(18)`.

**The small channel** of ADR 0458 E1 to E4 (`12 x 6 x 1`, `NU = 1e-2`, `DT = 0.05`, the inflow `sin(pi t)`), copied into
`crates/rcfd-fv/tests/outer_scale_f550.rs`, run for `STEPS = 8` steps here (so that a window of 3 is filled and slides).

## Registered checks

Every check is run once on the registered set-up.

| Id | Clause | What | Limit | Estimate behind it |
| --- | --- | --- | --- | --- |
| K1w | c3 | FCw (FC with KW) at `N = 25, 50, 100`: every step's record `met` with `used < 50` | every step | S2: floor `>= 0.509`; ADR 0458's non-capped steps at most 33 |
| A1w | c4 | the same runs bounded over all `C` periods | bounded | ADR 0458 A1k |
| A2w | c4 | periodic | cycle change `<= 1e-6` | ADR 0458: `3e-10` at most |
| A3w | c4 | each level against `Z_dt`, real and imaginary | `<= 1e-2` each | S2: at most about `9.5e-3`; ADR 0458 `5.7e-3` |
| A4w | c5 | `R_c` against the formula, each part | `<= 3e-2` each | S2: at most about `1.8e-2`; ADR 0458 `6.1e-3` |
| K2w | c6 | one record per step, numbered, with `first`, `scale` | every step | S4 |
| K4w | c7 | total outer iterations of each level against ADR 0458's recorded totals (the same set-up with `W = 1`, run once in ADR 0458) | `<= 3590, 2769, 2436` at `N = 25, 50, 100` | S2: no step's count increased to first order; the capped steps go from 50 to about 10 |
| QFw | c3, c4, c7 fast | FQ, `N = 16`, `C = 6`, run with KW and with K (ADR 0458's scale) in the same test: KW bounded, periodic `<= 1e-6`, within `5e-2` of `Z_dt` each part, every step met with `used < 50`, a record per step, and total(KW) `<=` total(K) | each | ADR 0458 QFk: `1.67e-2`, `1.46e-2`, 13 to 23 |
| W1 | brief | `rcfd-fv`: driven small channel, `Some(1e-2)`, cap 30: `outer_scale_window = Some(1)` gives the fields and the records (`used`, `met`, `ratio`, `first`, `scale`) of `None` bit for bit | bitwise | S2: `W = 1` is ADR 0458's scale |
| W2 | c1 | the same with `Some(3)`: for every step `scale` equals the maximum of `first` of the step and of the 2 records before it (exactly), `scale >= first`, `met`, `used < 30`, `ratio <= 1e-2` | exact; each | S4 |
| W3 | c8 | `outer_scale_window = Some(3)` without the criterion: the fields of the fixed `n_outer = 6` run bit for bit, no record | bitwise; empty | S4: read only with the criterion |
| W4 | c1 | fluid at rest, `Some(1e-3)`, window 3: `used = 2`, `met`, every step | exact | ADR 0458 E3 |
| L1 | c2 | `rcfd-fv`: driven small channel with the criterion `Some(1e-2)`, cap 30, window 3 and `pressure_lag_check` every step (`Warn`, 6 power iterations), in lockstep with a second solver without the criterion whose `n_outer` is set before each step to the count the first one used in it: the B4 estimates (`rho`, `cell`) and the fields equal bit for bit every step, and at least one step uses fewer than 30 | bitwise; `< 30` | S3: the same sequence of iterations with the same final count |
| L2 | c2 | the same with the adaptive corrector count (`CorrectorControl::registered()` with `every = 1`) instead of the B4 check: the fields and the corrector estimates (`correctors`, `rho` bits) equal bit for bit every step | bitwise | S3 |
| N1 | c8 | `cargo test -p rcfd-api --release --test baseline_hashes` | passes | S4 |
| N2 | c8 | the touched `rcfd-fv` code's tests in release: the unit tests of `pimple.rs` (`--lib pimple`), `outer_criterion_f536`, and every `rcfd-fv` integration test whose name contains `lag`, `corrector` or `outer` | pass | S3, S4: the default and ADR 0458 paths unchanged |
| N3 | rules | `git diff b61032f4 -- crates/*/tests` adds files only | adds only | rules |

Printed records, no limit: per level the mean / min / max of `used`, the total and the time against ADR 0458's, the minimum of
`scale/max scale` over the last period (S2's floor), the observed order of the three levels.

## Negative controls

* NC1w (fast): FQ with KW but `tol = 0.5` must miss `5e-2` against `Z_dt` in at least one part (the window does not mask a loose
  tolerance). Estimate: as ADR 0458 NC1 (`-1.87e-1` imaginary), the window can only loosen it.
* NC2w (heavy): the `N = 25` level of A3w against the formula instead of `Z_dt` must miss `1e-2` (D2: `-0.351` imaginary).
* The control of K1w is ADR 0458's recorded FCk (`W = 1`): 20 steps at the cap on the same set-up. It is not re-run (house rule 2:
  each registered run once; it is a registered result).

## Planned mutations (scratch copy, not committed)

* MW1: the window ignored (`S = d_1` also with `Some`). Expected: W2.
* MW2: the window one longer (`W` previous steps instead of `W - 1`). Expected: W2.
* MW3: `lag_map` back to `s.n_outer` passes. Expected: L1.
* MW4: `LagCtx::outer` set to `s.n_outer` in `pick_correctors`' context only. Expected: L2.
* MW5: the history not pushed. Expected: W2.
* MW6: the window applied when `outer_scale_window` is `None` (with `W = 18`). Expected: W1 (`None` against `Some(1)`).
* MW7: the history kept in `scratch` reset to empty. Expected: none of the registered checks may see it (a preflight copy then has
  a looser scale); recorded as not caught if so.

## Decision rule

* **done**: K1w, A1w to A4w, K2w, K4w, QFw, W1 to W4, L1, L2, N1 to N3 met on their registered runs, NC1w and NC2w hold, every
  mutation caught except MW7 (registered as possibly not caught), no amendment after a run.
* **partial**: the option is built and opt-in with N1 to N3 met, but a check missed, a control failed, a mutation (other than MW7)
  was not caught, or a check was met only after an amendment made after its run.
* **todo**: the option cannot be built without changing a default (N1 or N2 fails and cannot be mended opt-in).

## Limits (not covered)

* Only the channel of ADR 0451 (Womersley 1, 40 cells across) and FQ; the window `W = 18` is derived for at most 100 steps per
  period; a history with more steps per period has a lower floor (`sin(pi 17/N)`), a non-periodic history none beyond `W` steps.
* The looser scale for `W - 1` steps after a start from rest (S2).
* The absolute floor tied to the solvers' tolerances is not built (S2).
* A non-monotone outer iteration, a hook that changes the sources between outer iterations, relaxation: as ADR 0458.
* No case key (schema golden); not the default; the moving piston and the stable default runs of ADR 0451 are not run.

## Amendment 1 (2026-11-15, written after the mutation run of MW6, before the run of MW6b)

MW6 was applied as one replacement, `s.outer_scale_window` read as `s.outer_scale_window.or(Some(18))` where the scale is formed.
It was not caught. Diagnosis (by reading the code, no run): the history of first changes is pushed only under `if let Some(w) =
s.outer_scale_window`, which the mutation left alone, so with `None` the history stays empty and the scale is
`fold(change, empty) = change`, bitwise ADR 0458's. The mutation as applied is equivalent (it changes no result), so no test can
catch it; it stays recorded as not caught. Corrected mutation **MW6b**: the same replacement at both places (the scale and the
push), so that `None` runs the window of 18. Expected: caught by W1 (`None` against `Some(1)`). Run once after this amendment is
committed. By the decision rule, a mutation met only after an amendment made after its run gives **partial**; this is recorded,
not changed.

## Addendum 1: results (2026-11-15)

Commits: `4a3fd521` (this registration), `13149130` (the library change and the registered tests; every registered run was made
on it), `5eac3548` (`cargo fmt` of the two new test files, layout only), `ad543ef1` (amendment 1; the bullet of
`docs/dynamic-mesh.md` went into the same commit), and the last commit (this addendum). Every run in release
(`CARGO_TARGET_DIR=f550-scratch/target`); logs in `w49i-work/f550-scratch` (`run1_fv.log`, `run2_fast.log`, `run3_fcw.log`,
`baseline_hashes.log`, `fv_n2.log`, `post_f536.log`, `fmt.log`, `clippy.log`, `mutlogs/`, `mut_run.log`).

**The library change** (`crates/rcfd-fv/src/pimple.rs`, opt-in only): `PimpleSettings::outer_scale_window` (default `None` in
`piso`), `OuterRecord::first` and `::scale`, the private history `PimpleSolver::outer_firsts` (empty in `new`, copied in
`scratch`, cleared in `reinitialise`), the windowed scale in the outer loop of `step_with` (only with `Some`), the history pushed
with the record (only with `Some`), `LagCtx::outer` set to `outer + 1` in its two constructions, and `lag_map` running
`ctx.outer` passes. Without the criterion `outer + 1 == n_outer` there, so the estimate is unchanged.

### Results

| Check | Limit | Measured | Met |
| --- | --- | --- | --- |
| K1w every step met before the cap, `N = 25, 50, 100` | every step `met`, `used < 50` | every step; max 12, 9, 7 | yes |
| A1w bounded (`C = 10, 5, 3`) | bounded | bounded; max `|u|` `1.490, 1.493, 1.496 V0` | yes |
| A2w periodic | `<= 1e-6` | `1.26e-12`, `6.6e-13`, `1.30e-12` | yes |
| A3w against `Z_dt` | `1e-2` each part | real `+5.26e-3`, `+5.52e-3`, `+5.66e-3`; imaginary `-2.82e-3`, `-9.0e-4`, `-2.6e-4` | yes |
| A4w `R_c` against the formula | `3e-2` each part | real `+6.06e-3`, imaginary `+3.77e-3` | yes |
| K2w one record per step, with `first`, `scale` | every step | 250, 250, 300, numbered, `scale >= first` | yes |
| K4w total against ADR 0458 | `<= 3590, 2769, 2436` | `2735`, `2044`, `1983` (24, 26, 19 percent fewer) | yes |
| QFw FQ, `N = 16`, `C = 6` | bounded; `1e-6`; `5e-2`; met before the cap; record per step; total `<=` K's | bounded; `1.3e-13`; real `+1.67e-2`, imaginary `-1.47e-2`; every step (6 to 13); 96; `1113 <= 1380` | yes |
| W1 `Some(1)` against `None` | bitwise fields and records | equal | yes |
| W2 windowed maximum, window 3 | exact; met, `used < 30`, ratio `<= 1e-2` | exact every step (2 of 8 steps scaled by an earlier one); `used` 3 to 4 | yes |
| W3 window without the criterion | bitwise; no record | equal; empty | yes |
| W4 at rest | `used = 2`, `met` | yes, every step | yes |
| L1 B4 estimate against the fixed count used | bitwise `rho`, `cell`, fields; a step `< 30` | equal every step; 8 of 8 below the cap | yes |
| L2 adaptive corrector count | bitwise estimates, fields | equal every step | yes |
| N1 `baseline_hashes` | passes | 3 passed, 1 ignored | yes |
| N2 `rcfd-fv` touched tests, release | pass | `--lib pimple` 8; `consistent_correctors_f301` 12, `immersed_lag_estimate_f483` 5, `immersed_single_corrector_f363` 4, `lag_refusal_f535` 6, `outer_criterion_f536` 3, `w34b_lag` 3, `w48v_corrector_control` 5; 0 failed | yes |
| N3 tests: new files only | adds only | `crates/rcfd-fv/tests/outer_scale_f550.rs`, `crates/rcfd-post/tests/piso_outer_scale_f550.rs` added | yes |
| NC1w FQ, `tol = 0.5` | must miss `5e-2` | imaginary `-3.54e-1` (2 to 4 outer iterations) | holds |
| NC2w `N = 25` against the formula | must miss `1e-2` | `+9.25e-2`, `-3.53e-1` | holds |

Also run (not registered; the existing tests that cover the criterion path): the fast tests of ADR 0458's
`piso_outer_criterion_f536`, 3 passed.

Records (no limit). Outer iterations per step mean / min / max, total, time (ADR 0458's in brackets):

* `N = 25`: `10.94 / 4 / 12`, 2735 (3590), 131 s (148 s).
* `N = 50`: `8.18 / 5 / 9`, 2044 (2769), 83 s (96 s).
* `N = 100`: `6.61 / 4 / 7`, 1983 (2436), 76 s (86 s).

S2's floor over the last period, `min scale / max scale`: `0.997`, `0.902`, `0.531` at `N = 25, 50, 100` (derived `1`, `0.876`,
`0.509`). Over the same period the step's own first change falls to `2.0e-2`, `1.0e-2`, `5.4e-3` of the largest: that is the
near-zero scale of ADR 0458. Order of the levels: real `0.832`, imaginary `1.101` (the same as ADR 0458). The accuracy is that of
ADR 0458 to the third digit, so the iteration error the looser scale allows is far below S2's bound.

### Mutations

Applied one at a time to a `git archive` copy of `5eac3548` by `f550-scratch/mutate.py` (exact replacement, the count asserted).
After each, the fast tests of both new files and of ADR 0458's `outer_criterion_f536` were run in release
(`f550-scratch/target-mut`), and the file was restored and compared with the archive.

| Mutation | Caught by |
| --- | --- |
| MW1 the window ignored | W2 |
| MW2 the window one longer | W1, W2 |
| MW3 `lag_map` back to `s.n_outer` | L1, L2 |
| MW4 `pick_correctors`' context with `s.n_outer` | L2 |
| MW5 the history not pushed | W2 |
| MW6 the window when `None`, at the scale only | **not caught**: equivalent as applied (amendment 1: the push is gated on the same option, so the history stays empty) |
| MW6b (amendment 1) the window when `None`, at the scale and the push | W1 |
| MW7 the history not copied in `scratch` | not caught (registered as possibly not caught: no check takes a preflight copy with the window) |

### fmt, clippy, documentation

* `cargo fmt -p rcfd-fv --check`, `-p rcfd-post --check`: clean after `5eac3548` (the new test files only).
* `cargo clippy -p rcfd-fv -p rcfd-post --all-targets -- -D warnings`: clean.
* `docs/dynamic-mesh.md`: one bullet after ADR 0458's.

### Deviations and what was not run

* MW6 as applied was an equivalent mutant. Amendment 1 (committed before its run) registered MW6b, which was caught. No check was
  amended and no registered check was re-run.
* ADR 0458's FCk (`W = 1`) was not re-run; as registered, K4w compares with its recorded totals.
* Not run: the moving piston, the stable default runs, the whole workspace suite (the lead's gate), a case with more than 100
  steps per period.

### Decision

Every check of every acceptance clause is met on its registered run: K1w (every step meets the criterion before the cap at all
three levels), A3w, A4w, K2w, K4w (19 to 26 percent fewer outer iterations), L1 and L2 (the estimate and the adaptive count use
the count the step used), and N1 to N3 (defaults bit-identical). The registered rule still gives **partial**: MW6 was not caught,
and its corrected form MW6b was met only after an amendment made after the run. The cause is a defective mutation (equivalent as
applied), not a weakness of the checks, since W1 catches the intended mutation. **Proposed status: partial.** Question 3 asks the
lead whether to judge it done.

### Remaining questions

1. **A case key** (`solver.outer_tolerance` with `solver.outer_scale_window`). Options: (a) add both with `hash_omit_default`,
   which changes the schema golden and needs the maintainer's approval; (b) library only. Recommendation: (a), with the window
   defaulting to 18 when the tolerance is given. Open (golden).
2. **The window as the default of the criterion** (`outer_pressure_tolerance = Some` implying `W = 18`). It changes no default
   result, because the criterion itself is opt-in, but it changes ADR 0458's opt-in results. Options: (a) keep `None` meaning ADR
   0458's scale; (b) default to 18. Recommendation: (a) for now, and (b) together with question 1's key. Option (a) is what is
   built, taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).
3. **The status.** Options: (a) partial by the letter of the rule; (b) done, since the miss is an equivalent mutant.
   Recommendation: the lead decides; this ADR proposes partial because the rule requires it.
4. **The absolute floor** tied to the solver tolerances (for a history that becomes steady, where the window's scale decays with
   the flow). Options: (a) a follow-up item; (b) leave it. Recommendation: (b) until a case shows the need; the window covers the
   periodic case asked for.

## For the lead: roadmap entries

> Proposed status: partial.

> Numerics. PARTIAL (ADR 0617, 2026-11-15; registered before the code, amendment 1 for one mutation). The opt-in outer-iteration criterion of ADR 0458 gets the option PimpleSettings::outer_scale_window = Some(w): the ratio is formed against the largest first-iteration pressure change of the step and the w - 1 steps before it. This is derived so that on a periodic history of N steps per period the scale stays above sin(pi (w - 1)/N) of the period's largest (w = 18 for a floor of 1/2 at N = 100). None keeps ADR 0458's scale bit for bit, and Some(1) equals it. The pressure-lag estimate and the adaptive corrector count of a step with the criterion now run the outer iterations the step used (LagCtx::outer; bitwise equal to a solver with that fixed count). On the fixed-mesh channel of ADR 0451 with the consistent correctors, tol 1e-3, cap 50 and w 18, every step at nu dt/dy^2 = 100, 50 and 25 meets the criterion before the cap (at most 12, 9 and 7 outer iterations). Every level is bounded and periodic and within 5.7e-3 (real) and 2.8e-3 (imaginary) of the discrete implicit-Euler impedance, and the extrapolation within 6.1e-3 and 3.8e-3 of the oscillating-channel impedance. Totals are 2735, 2044 and 1983 outer iterations against ADR 0458's 3590, 2769 and 2436. OuterRecord gains first and scale. Defaults bit-identical (baseline_hashes and the touched rcfd-fv tests pass). Partial only by the registered rule: one mutation (the window with None) was equivalent as applied, and its corrected form, caught by the W1 check, was registered after the run; every acceptance clause is met. No case key (schema golden); docs/dynamic-mesh.md. Open: ADR 0617 Q1 to Q4.

F-536, the note's last sentence "Follow-up F-NEW (a scale that does not vanish)." becomes:

> Follow-up F-550 (a scale that does not vanish; ADR 0617: with a window of 18 steps every step of the channel meets the criterion before the cap, with 19 to 26 percent fewer outer iterations).

> Follow-up proposed: Case-file keys for the outer-iteration criterion and its window. Deps F-536, F-550. Acceptance: Every limit not stated here is registered in the ADR before the run. Case keys solver.outer_tolerance and solver.outer_scale_window (hash_omit_default) reach PimpleSettings::outer_pressure_tolerance and outer_scale_window; a case read from a file reproduces the bits of the library call on the channel of ADR 0451; existing keys, goldens and baseline hashes unchanged except the schema golden approved by the maintainer.

## Answers of the maintainer (2026-11-18), written into this ADR by the lead on 2026-11-19

The maintainer decided these on 2026-11-18 as sets of options with a recommendation; until now the decision stood in the roadmap note of the item only. Nothing above this section is changed.

- **F-692, remaining question 1 (a case key).** Asked: the keys `solver.outer_tolerance` and `solver.outer_scale_window`. Answer: Approved under the class rule for additive golden edits (agent rules, rule 3, from version 2): an edit that only adds entries and removes or changes none is reported by the agent as an "additive golden edit" with its diff, re-recorded by the lead and written in the ledger line of the merge. `baseline_hashes.rs` is never in this class, and an edit that changes or removes an entry still needs the maintainer's word for that item. The item may be issued.
