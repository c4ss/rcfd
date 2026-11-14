# ADR 0458: F-536 registration: outer-iteration criterion for the consistent-corrector step at large diffusion numbers

Status: **REGISTERED 2026-11-14, written before any code of F-536 and before any run of its checks.** Not yet accepted by the
maintainer. Results go in the addendum at the end; nothing above the addendum is changed after a run.

Parents: [ADR 0451](0451-f530-registration-piso-stability-at-large-diffusion-numbers.md) (F-530: the fixed-mesh channel FC and the
fast channel FQ, the discrete model D2, the consistent correctors with `n_outer = 8` of amendment 1, taken from a diagnosis),
[ADR 0455](0455-f531-registration-symmetry-plane-tangential-viscous-flux.md) (F-531: with `alpha_u = 1` every outer iteration
restarts from the predictor; the relaxed `PimpleSettings::pimple` constructor is not the configuration of these runs),
[ADR 0214](0214-f299-registration-piso-instability-at-large-cell-diffusion-number-across-a-thin-gap.md) (the consistent correctors),
[ADR 0073](0073-wave34b-transient-floor-pimple-lag.md) (the B4 estimate). Roadmap item F-536 (deps F-530). Number 0458 given by
the lead. Branch `f536-work` (from main `03525efe`), worktree `w49i-work/f536`, scratch `w49i-work/f536-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here before the code and the runs; each registered run made once; a missed
limit becomes an ignored `*_known_miss` test with the limit intact; defaults bit-identical; no edit of `baseline_hashes.rs`, the
hash tables or a golden file; existing tests are not edited; no new dependency, no unsafe code; clean-room (the criterion is
derived below from the linear contraction of a fixed-point iteration; nothing is taken from literature or another code).

## What F-536 asks

The acceptance of the roadmap entry, binding: "Every limit not stated here is registered in the ADR before the run. The number of
outer iterations of the transient step with consistent correctors is set by a registered criterion (a derived contraction of the
pressure error, or a tolerance on the change of the pressure between outer iterations) instead of a fixed count, so that the
fixed-mesh channel of ADR 0451 at nu dt/dy^2 = 25, 50 and 100 matches the discrete implicit-Euler impedance within 1 percent at
every level and its extrapolation the oscillating-channel impedance within 3 percent, with the outer iterations used per step
recorded; the default results of every existing case stay bit-identical."

Clauses: (c0) every other limit registered here; (c1) the outer count set by a registered criterion, not a fixed count; (c2) FC at
`D = 25, 50, 100` (`N = 100, 50, 25`) within 1 percent of `Z_dt` at every level (read, as in ADR 0451 A3, as the real and the
imaginary part each); (c3) the extrapolation `R_c = 2 Z_100 - Z_50` within 3 percent of the formula (each part, as ADR 0451 A4);
(c4) the outer iterations used per step recorded; (c5) the default results of every existing case bit-identical.

## What was read (no Rust run)

ADR 0451 (whole, with amendment 1 and its addendum), `crates/rcfd-post/tests/piso_diffusion_f530.rs` (the channel, the model,
`cycles_for`, `outer_settings`, the Ao and QFo tests), ADR 0455 (amendment 1 and the addendum's reading of Q4/Q4c), and in
`crates/rcfd-fv/src/pimple.rs`: `PimpleSettings` (every field), `piso`, `pimple`, `StepInfo`, `OuterHook`/`OuterUpdate`,
`PimpleSolver::new` and `scratch`, the outer loop of `step_with` (`for outer in 0..s.n_outer`, `last = outer + 1 == s.n_outer`, and
what `last` controls: the relaxation `alpha_u`, `alpha_p` of non-final iterations, the frozen matrices of the B4 check
`lag_ctx`, the adaptive corrector count `corrector_control`, `n_corr_final`, `cc_change`), the end of the step, and `lag_map`
(which runs `s.n_outer` outer passes). `moving.rs`: `step_ale`, `step_ale_with_walls` and `step_sliding` call `self.step`, so a
change of the loop reaches them. `crates/rcfd-api/src/schema.rs` (`solver.n_outer`, range 1 to 50) and `sim.rs`
(`PimpleSettings::pimple(nu, n_outer)`). `PimpleSettings` and `PimpleSolver` are built by struct literal only in `pimple.rs`.

**Finding of the read (the brief's first question).** The PIMPLE loop has **no** outer-convergence tolerance: `n_outer` is always
run in full, and no hook result can end the loop (`OuterUpdate` carries sources only). There is no case key for one either. So
the item needs a small library change (opt-in), not only the registration of a setting.

**Arithmetic before this registration (disclosed).** By hand from the numbers ADR 0451 records (no script, no run): the contraction
per outer iteration `r` bounded from the one- and eight-outer impedance errors (below); the outer counts the criterion would use;
`R_c` of the Ao runs from their recorded parts.

## Design

### C1. The criterion (derived)

Write the outer iterations of one step as a fixed-point iteration on the pressure, `p_k = M(p_{k-1})`, `p_0 = p^n` (the pressure
at the start of the step), with fixed point `p*` (the converged step: with `alpha_u = alpha_p = 1` and the consistent correctors,
the two-corrector map with a converged pressure, ADR 0455 amendment 1). Near `p*` the error `e_k = p_k - p*` contracts as `e_k = r
e_{k-1}` for its slowest mode, `0 < r < 1`. The change between outer iterations is then `dp_k = p_k - p_{k-1} = (r - 1) e_{k-1}`,
so

* `|dp_k| / |dp_1| = r^{k-1}` (the change measures the contraction of the step's own error with no unknown scale), and
* the error left after one further iteration is `e_{k+1} = r^2 e_{k-1} = r^2 dp_k/(r - 1)`, and relative to the error the
  non-iterated step would leave, `e_1 = r e_0`: `|e_{k+1}| / |e_1| = r^k = r (|dp_k|/|dp_1|)`.

**Criterion.** After outer iteration `k` (1-based) the solver forms `c_k = max_c |p_k - p_{k-1}| / max_c |p_1 - p^n|`; if `c_k <=
tol`, iteration `k + 1` is run as the **final** one (unrelaxed, the B4 check and the adaptive corrector count on its matrices, as
`last` does now) and the loop ends after it; the cap is `n_outer` (the final iteration is then the `n_outer`-th, as now). The
final iteration after the criterion is met is needed because `last` decides, before an iteration, its relaxation and the
matrices of the checks; it also gives the factor `r` above. By C1, the iteration error left in the step is at most `r tol <= tol`
times that of the non-iterated step. The scale `|p_1 - p^n|` is the pressure change of the step's first outer iteration: it is
zero for a step that changes nothing (the criterion is then met at once, `0 <= 0`, and the step runs two outer iterations), and
it is not the pressure itself (the channel's pressure passes through zero twice a period, which would make a criterion relative
to `|p|` arbitrarily strict there). The max norm over the cells, as `cc_change` of ADR 0156.

**The tolerance for the channel.** The impedance error of the non-iterated consistent step (ADR 0451 amendment 1, diagnosis 2 and
A3) is `-14.3 %` of the imaginary part at `N = 50` and `-3.08 %` at `N = 100`; at Womersley 1 the imaginary part is about 0.4 of
the real part (from the low-Womersley expansion of `1 - tanh(kappa)/kappa = kappa^2/3 - 2 kappa^4/15`, `kappa^2 = i`: real `2.59`,
imaginary `1.03` in units of `A w rho L_f`), so the error relative to `|Z|` is at most about 0.4 of that relative to the imaginary
part. Assume the non-iterated step's error relative to `|Z|` is at most 1 at every level (`N = 25` is not measured; at `N = 50` it
is about 0.06). Then the iteration error of a part is at most `(|Z|/|part|) r tol <= 2.7 tol` for the imaginary part. With **`tol =
1e-3`** this is `2.7e-3`, which with the converged step's own distance from `Z_dt` (the Ao runs with 8 outer iterations: real
`+5.3e-3` to `+5.7e-3`, imaginary `-4.6e-3` to `-2.5e-4`, mostly the ends and the half-cell pressure of ADR 0451) gives at most
about `8.4e-3` against the limit `1e-2`.

**The contraction `r` and the expected counts.** From the recorded errors, if the iteration error is `eps_K = eps_1 r^{K-1}` and
the 8-outer error is at most the measured total: `r <= (9.4e-4/0.143)^{1/7} = 0.488` at `N = 50`, `r <= (2.5e-4/0.0308)^{1/7} =
0.503` at `N = 100` (upper bounds; part of the 8-outer error is not iteration error). With `r = 0.5` the criterion is met at `k = 1
+ ceil(ln(1e-3)/ln 0.5) = 11` and the step uses `K = 12` outer iterations (1.5 times the fixed 8). The first change `dp_1` holds
the fast modes too, so `c_k` can fall faster than `r^{k-1}` at first and the count be lower. At `N = 25` (`D = 100`) `r` is not
known; the cap is set to 50 (the schema's upper bound of `solver.n_outer`), which the criterion reaches only if `r > 0.866`
(`r^{48} > 1e-3`).

### C2. The library change (opt-in)

* `PimpleSettings::outer_pressure_tolerance: Option<f64>`, default `None` (in `piso`, hence in `pimple`): `None` runs exactly
  `n_outer` outer iterations as now; `Some(tol)` applies C1 with `n_outer` as the cap. A negative or NaN `tol` is never met (the
  cap is run).
* In the loop of `step_with`: `last = outer + 1 == s.n_outer || final_next`, where `final_next` is set at the end of an iteration
  by C1 (only with `Some`); after the final iteration the loop ends. With `None`, `final_next` stays `false` and the loop is the
  present one line for line (the added `break` after `last` changes nothing: the loop ends there anyway). The pressure at the
  start of each outer iteration is copied only with `Some`.
* `PimpleSolver::outer_record: Vec<OuterRecord>`, one entry per completed step (not for a step stopped by the guard), pushed only
  with `Some`: `step`, `used` (outer iterations run), `met` (whether C1 ended the loop before the cap) and `ratio` (the last `c_k`
  formed). Empty with `None`. (c4)
* Not changed: `lag_map` runs `s.n_outer` outer passes, so the B4 estimate of a step with the criterion is that of the cap's count
  (recorded as a limit; the cycle rule below uses the B4 of the fixed 8 instead). The fractional step does not use the outer loop.
  No case key: a key `solver.outer_tolerance` would change the schema golden (house rule 3); it is listed as an open question.
* `baseline_hashes` (rcfd-api) and the `rcfd-fv` suite are run (house rule 3, the brief).

## Set-ups

FC, FQ, the inflow, the force, `periodic_response`, `model_zdt`, `impedance_formula` and boundedness exactly as ADR 0451 (copied
into a new test file `crates/rcfd-post/tests/piso_outer_criterion_f536.rs`; `piso_diffusion_f530.rs` is not edited).

**The settings K.** ADR 0451's `settings(true)` (`PimpleSettings::piso(nu)`, central convection, tight solvers, the consistent
correctors; 2 correctors, `alpha_u = alpha_p = 1`) with `n_outer = 50` (the cap) and `outer_pressure_tolerance = Some(1e-3)`.

**Periods.** The cycle rule of ADR 0451 amendment 1, `C = max(3, 2 + ceil(ln(1e-7)/(N ln rho_c)))`, with `rho_c` the B4 estimate of
ADR 0451's `outer_settings()` (the fixed 8; recorded there `0.920, 0.852, 0.731` at `N = 25, 50, 100`, so `C = 10, 5, 3`; and
`0.731` on FQ at `N = 16`, `C = 6`). Estimate that this suffices: the slow mode decays at least as fast with more outer
iterations (B4 `0.952` with 1 and `0.731` with 8 on FQ), and C1 predicts more than 8 on FC. The periodicity check A2k guards it.

## Registered checks

Every check is run once on the registered set-up.

| Id | Clause | What | Limit | Estimate behind it |
| --- | --- | --- | --- | --- |
| K1 | c1 | FC with K at `N = 25, 50, 100`: every step's `outer_record` entry has `met = true` and `used < 50` (the count set by C1, not by the cap) | every step | C1: about 12 at `r = 0.5`; the cap only if `r > 0.866` |
| A1k | c2 | the same runs bounded (ADR 0451's sense) over all `C` periods | bounded | Ao: bounded with 8 |
| A2k | c2 | periodic | cycle change `<= 1e-6` | Ao: `4.5e-12` to `5.6e-11` |
| A3k | c2 | each level against `Z_dt`, real and imaginary part | `<= 1e-2` each | C1: at most about `8.4e-3`; Ao `5.7e-3` |
| A4k | c3 | `R_c = 2 Z_100 - Z_50` against the formula, each part | `<= 3e-2` each | Ao: `+6.1e-3`, `+3.8e-3` |
| K2 | c4 | `outer_record` has one entry per step of each run, with `step` equal to the step number | every step | C2 |
| K3 | c1 | the mean count per step at `N = 50` and `N = 100` | `5 <= mean <= 20` | C1: 12 at `r = 0.5`; 5 to 20 is `r` from about 0.3 to 0.7 |
| QFk | fast | FQ with K, `N = 16`, `C = 6`: bounded; periodic `<= 1e-6`; real and imaginary part within `5e-2` of `Z_dt`; every step `met` with `used < 50`; one record per step | each | QFo (8 outer): `1.66e-2`, `2.55e-2`; QF2's limit |
| E1 | c5 | `rcfd-fv` test, a small driven channel (default PISO settings with `n_outer = 6`): `outer_pressure_tolerance = Some(0.0)` (never met for a changing flow) gives the velocity, pressure and flux of the run without it bit for bit, and records `used = 6`, `met = false` every step | bitwise equal | C2: with the criterion never met the loop is the fixed one |
| E2 | c4 | `rcfd-fv`: the same channel without the criterion records nothing (`outer_record` empty) | empty | C2 |
| E3 | c1 | `rcfd-fv`: a fluid at rest with no driving and the criterion `Some(1e-3)`, `n_outer = 6`: `used = 2`, `met = true` every step | exact | C1: `dp_1 = 0`, met at once, one final iteration |
| E4 | c1 | `rcfd-fv`: the driven channel with `Some(1e-2)`, `n_outer = 30`: every record `met`, `used < 30`, `ratio <= 1e-2` | each | C1 |
| N1 | c5 | `cargo test -p rcfd-api --release --test baseline_hashes` | passes | C2: the default path unchanged |
| N2 | c5 | the `rcfd-fv` suite in release (the touched file's tests and every integration test of the crate) | passes | C2 |
| N3 | rules | `git diff 03525efe -- crates/*/tests` adds files only | adds only | rules |

Printed records, no limit: per level the mean, minimum and maximum `used`, the total outer iterations and the run time against
the 8-outer Ao runs (`8 x steps`), the observed order of the three levels per part (ADR 0451's O, not registered here as a check),
`R_c` against Ao's.

## Negative controls

* NC1 (fast): FQ with `tol = 0.5` (else K) must miss `5e-2` against `Z_dt` in at least one part. Estimate: met at `k = 2` (`r =
  0.5`), 3 outer iterations, an iteration error `r^2` of the non-iterated one (QFc: the imaginary part `-210 %` off with one
  outer), about `-50 %`.
* NC2 (heavy): the `N = 25` level of A3k against the formula instead of `Z_dt` must miss `1e-2` (D2: `-0.351` imaginary).
* E1 is the control of K1: the cap reached when the criterion is not met.

## Planned mutations (scratch copy, not committed)

* MU1 (library): `|| final_next` dropped from `last` (the criterion never ends the loop). Expected: caught by QFk and E4 (`used`
  equals the cap) and E3.
* MU2 (library): `c_k` formed against the step-start pressure `p^n` instead of `p_{k-1}` (`|p_k - p^n|/|p_1 - p^n|`, about one).
  Expected: caught by QFk and E4 (never met).
* MU3 (library): the loop ends at once when the criterion is met (no final iteration). Expected: caught by E3 (`used = 1`); the
  accuracy checks may not see it (one iteration less, a factor `r`), which is recorded if so.
* MU4 (library): the tolerance multiplied by 100 inside the solver. Expected: caught by QFk (accuracy) and E4 (`ratio`).
* MU5 (library): the record not pushed. Expected: caught by QFk and E3, E4 (one record per step).
* MU6 (library): the `break` after the final iteration dropped (with `final_next` set, every later iteration is final too, up to
  the cap). Expected: caught by QFk, E3, E4 (`used`).
* MU7 (test): `s = i w` in the model. Expected: caught by the model check (copied) and QFk.

## Decision rule

* **done**: K1, A1k to A4k, K2, K3, QFk, E1 to E4, N1 to N3 met on their registered runs, NC1 and NC2 hold, every mutation caught
  (MU3 by at least E3), no amendment made after a run.
* **partial**: the criterion is built and opt-in with N1 to N3 met, but a check missed, a control failed, a mutation was not
  caught, or a check was met only after an amendment made after its run.
* **todo**: the criterion cannot be built without changing a default (N1 or N2 fails and cannot be mended opt-in).

## Limits (not covered)

* Only the channel of ADR 0451 at Womersley 1, 40 cells across (and FQ); the moving piston (ADR 0451 APo) and the low-frequency
  case are not run.
* The criterion is a heuristic of a linear contraction (C1); a non-monotone outer iteration or a hook that changes the sources
  between outer iterations is not analysed. With relaxation (`alpha_u`, `alpha_p < 1`), `c_k` measures the relaxed change.
* The B4 estimate and `lag_preflight` of a step with the criterion use the cap's count of outer iterations (C2).
* No case key (a schema golden change); not the default.
* The stable default runs (ADR 0451 A5, A6) are not repeated: `|R_c - R_d|` is not part of F-536's acceptance.
