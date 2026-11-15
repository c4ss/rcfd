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
