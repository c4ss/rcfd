# ADR 0222: F-260 registration: CMA-ES from Hansen's tutorial with restarts of increasing population, a multimodal check against Nelder-Mead, and the real-fan study registered as out of scope

Status: **REGISTERED 2026-11-08**, written before any code of F-260 and before any run of its checks. Not yet accepted by the
maintainer. Before this text no rcfd code or test was run for F-260; two papers were read (the reading section below). Results
go in the addendum at the end.

Summary of the results (addendum 1): the opt-in method `cma_es` reaches `f <= 1e-8` on the shifted Rastrigin function in 4
dimensions within 40 000 evaluations on 20 of 20 seeds (limit 18 of 20), from a start where every Nelder-Mead run (40, default and
ADR 0209 options) stops in the local minimum `f = 35.82`; the unimodal, part, study and tool checks are met and every mutation
fails its test. Without restarts 3 of 20 seeds reach the target; in 10 dimensions 14 of 20 (recorded). The FAN-01 study is not
attempted (no converged FAN-01 solve, no blade-parameter keys), so F-260 is partial (question 1). Defaults, baselines and goldens
unchanged.

Parents: [ADR 0067](0067-wave32c-design-loop.md) (the design loop, the store, propose / record, the optimisers' batch interface),
[ADR 0209](0209-f249-registration-nsga2-studies-nelder-mead-options-and-extended-warm-start.md) (F-249: NSGA-II, the Nelder-Mead
options, the house rules of the design work) and [ADR 0220](0220-f297-registration-nsga2-per-variable-exchange-and-the-30-parameter-zdt1-check.md)
(F-297: the reading-before-code pattern); for the FAN-01 state [ADR 0110](0110-wave43c-fan01-rotor-passage-non-convergence-diagnosis.md),
[ADR 0176](0176-job49m-fan01-blade-passage-as-a-3d-example-project.md) and [ADR 0181](0181-job49r-a-mesh-change-at-the-fan01-hub-trailing-edge-site.md).
Roadmap item F-260 (depends on F-249, partial, and F-126, partial).

Number: the lead gave 0222 to F-260. Branch `f260-cmaes` from main 539956da, worktree `w49i-work/f260`.

House rules (ADR 0130, as in ADR 0209 and ADR 0220):

- limits are fixed here, before the code and the runs, and are not changed after a run;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact and the measured number recorded, never tuned;
- a defective set-up gets a new corrected test by an amendment written before the corrected run, and the old one stays as an
  ignored record;
- every new path is opt-in and the defaults stay bit-identical: `crates/rcfd-api/tests/baseline_hashes.rs` and
  `case_hashes_w48u.rs` are not edited or re-recorded; no golden is edited or re-recorded (the tools-list goldens of
  `crates/rcfd-mcp/tests/golden/`); no case-schema key is added; no existing test file is edited;
- each registered run is made once, and nothing is re-run to pick a result;
- no new third-party crate, `Cargo.lock` unchanged, no commit trailers.

Clean-room and sources: the algorithm and its constants are taken from the two texts listed below, read for this text. No source
listing of any CMA-ES or other optimisation code was opened: the tutorial's Appendix C (MATLAB source code, pages 36 and 37) was
not opened or rendered, the link "A Python implementation ... can be found here" on its page 32 was not followed, and the MATLAB
code version named in a footnote of Auger and Hansen (2005) was not looked at. Two textbook parts are written from memory and
checked by their own closed-form checks: the Box-Muller transform for normal draws (K3) and the cyclic Jacobi eigenvalue method
for the symmetric covariance matrix (K2). The test functions (Rastrigin, sphere, ellipsoid) are from memory and checked by their
closed forms (K0). `docs/reference-data.md` records what was read and what was not (documentation commit).

## What F-260 asks

The roadmap entry (title: parametric studies, CMA-ES and a real-fan study; split from F-249 on 2026-10-22, user decision):
"CMA-ES reaches the registered target on a multimodal analytic test function from a start where Nelder-Mead does not (limits
registered in an ADR); a multi-objective study on the FAN-01 passage over at least two blade parameters, using a converged solver,
returns a Pareto set whose members are re-run to confirm their reported objectives within 1 percent". Note: "Blocked on FAN-01
solver convergence (waves 43B-C)."

**Reading of the acceptance (registered here).**

1. **Clause 1 (CMA-ES).** One registered problem: the shifted Rastrigin function in 4 dimensions in the box `[-5.12, 5.12]^4`,
   from a registered start `x0` (P-R below). "CMA-ES" is the method built here (D1) with the registered settings: the
   (mu/mu_W, lambda)-CMA-ES of the tutorial with its default parameters, and restarts with a doubled population (Auger and Hansen
   2005), which the tutorial recommends for multimodal problems (page 32: "Independent restarts with increasing population size
   [3], automated or manually conducted, are a useful policy to perform well on most problems"). "The registered target": best
   `f <= 1e-8` (the target accuracy of Auger and Hansen 2005) within `n x 10^4 = 40 000` evaluations (their budget rule), on at
   least 18 of 20 seeds (C1). "From a start where Nelder-Mead does not": rcfd's Nelder-Mead, with its default options and with the
   ADR 0209 options, from the same `x0` with the same budget, stays above the target on every one of 20 seeds (N0, a set-up check).
2. **Clause 2 (the real-fan study) is out of scope of this registration** and is not attempted, for these reasons (read in the
   documents named, nothing run):
   * F-260 depends on F-126, which is partial: the FAN-01 acceptance is not met and not attempted (ROADMAP F-126), and the item's
     own note says it is blocked on FAN-01 convergence.
   * There is no converged FAN-01 solve to study. With the registered second-order set-up the bladed B3 passage stalls in a limit
     cycle (ADR 0110), and none of the follow-ups converged it with an accepted change (ADRs 0111, 0174, 0175, 0177 to 0181; the
     last, ADR 0181, leaves the B2 cycle in place). First-order upwind is the only set-up that reaches small residuals: in ADR
     0110 S10 only after a switch at iteration 1000 of the stalled run, and the shipped first-order example case of ADR 0176 ends
     `not_converged` at its 1500-iteration cap (its P1); its torque is 35 to 38 % below the measured value.
   * No case key carries a scalar blade parameter: the FAN-01 blade is sixteen station lists of `mesh.fan`
     (`station_chord`, `station_stagger`, ...; `example-projects/axial-fan/cases/fan01-passage-upwind.toml`), and a study
     parameter writes `scale x + offset` to one settings path. Two blade parameters would need new keys or list-element targets
     (schema and tool changes) or a library generator hook, none of which exists.
   F-260 therefore stays **partial** whatever clause 1 gives (question 1).

## What the code read shows (main 539956da, read only)

1. **`rcfd_api::design::optim`**: `Stop` (`Interrupted`, `Missing(points)`, `Error`), `Fitness { f, violation }` with Deb's rules
   (`better`), `Eval` = `FnMut(&[Vec<f64>]) -> Result<Vec<Fitness>, Stop>` (one call per batch: evaluation, resume from the store
   and proposal all go through it), `OptResult { x, fitness, evaluations, converged }`, `Bounds`, `random_in_box`, the
   crate-private `clamp_into`. Nelder-Mead (`NmOptions` with `Default`; restarts from the best point with half the step).
2. **`rcfd_core::rng::Rng`** (re-exported as `design::rng::Rng`): xoshiro256** seeded by SplitMix64; `uniform()` in [0, 1) with 53
   bits, `range`, `below`, `shuffle`; no normal draws.
3. **`design::study`**: `Method` (`Doe`, `NelderMead`, `Differential`, `NelderMeadWith`, `Nsga2`, `Nsga2With`; `#[derive(Debug,
   Clone, PartialEq)]`), `Study::run` matches it, `start_point()` is the parameters' `init` when all have one, `eval_points`
   (store lookup by exact key, propose mode `Stop::Missing`, `allowance` for interruption), `fitness_of` (weighted scalar and
   violation). Nothing outside `design/` matches on `Method` (a search of `crates/`); tests build its variants literally, so a new
   variant breaks nothing.
4. **`design::tools`**: `parse_method` reads `type, budget, restarts, seed, pop, levels, n` and the opt-in keys
   `initial_simplex`, `coefficients` (nelder_mead only) and `crossover` (nsga2 only); other keys are ignored, so `sigma0` is
   ignored today for every type. The unknown-type message is `method.type: '<t>' is not nelder_mead, differential_evolution,
   nsga2, factorial, lhs or sobol (CMA-ES is not available)` and the missing-method message `method: missing (nelder_mead,
   differential_evolution, nsga2, factorial, lhs or sobol)`; no test asserts either text (search of `crates/`). `parse_study` is
   public; `method_json` writes `study.json`'s `method`.
5. **The MCP descriptor** of `design_create_study` (`crates/rcfd-mcp/src/lib.rs`): `method` is a free object `{type, budget?,
   restarts?, seed?, pop?, levels?, n?}`, and the description names `factorial, lhs, sobol, nelder_mead,
   differential_evolution`. A new method type and the key `sigma0` need no descriptor change, so `tools/list` and both goldens
   stay byte-identical (the description keeps not naming the new method: question 4).
6. **No n x n symmetric eigensolver** exists in the workspace (`rcfd_fv::viscoelastic::eig_sym` is 3 x 3).
7. **`design::sampling`**: `latin_hypercube(n, d, seed)` on the unit cube and `scale(points, bounds)`.

## The reading (done before this text)

| Id | Text | Copy read | What it states (used here) |
|---|---|---|---|
| R1 | N. Hansen, "The CMA Evolution Strategy: A Tutorial", arXiv:1604.00772v2 (10 March 2023, compiled 13 March 2023) | the arXiv PDF; pages 1 and 2 (contents) and 28 to 35 (Appendix A, Table 1, Appendix B.1 to B.5) read as rendered page images; the HTML version's sections 4 and A in a summary | Fig. 6, eqs. (38) to (47): `z_k ~ N(0, I)`, `y_k = B D z_k`, `x_k = m + sigma y_k`; `<y>_w = sum_{i<=mu} w_i y_{i:lambda}`; `m <- m + c_m sigma <y>_w`; `p_sigma <- (1 - c_sigma) p_sigma + sqrt(c_sigma (2 - c_sigma) mu_eff) C^{-1/2} <y>_w`; `sigma <- sigma exp((c_sigma/d_sigma)(norm(p_sigma)/E norm(N(0, I)) - 1))`; `p_c <- (1 - c_c) p_c + h_sigma sqrt(c_c (2 - c_c) mu_eff) <y>_w`; `w_i^o = w_i (1 if w_i >= 0 else n / norm(C^{-1/2} y_{i:lambda})^2)`; `C <- (1 + c_1 delta(h_sigma) - c_1 - c_mu sum w_j) C + c_1 p_c p_c^T + c_mu sum_{i<=lambda} w_i^o y_{i:lambda} y_{i:lambda}^T`. Page 28: `E norm(N(0, I)) = sqrt 2 Gamma((n+1)/2)/Gamma(n/2) ~ sqrt n (1 - 1/(4n) + 1/(21 n^2))`; `h_sigma = 1` if `norm(p_sigma)/sqrt(1 - (1 - c_sigma)^(2(g+1))) < (1.4 + 2/(n+1)) E norm(N(0, I))`, else 0, "where g is the generation number"; `delta(h_sigma) = (1 - h_sigma) c_c (2 - c_c)`; `C^{-1/2} = B D^-1 B^T`, `C^{-1/2} y_i = B z_i`, `C^{-1/2} <y>_w = B sum w_i z_{i:lambda}`; the eigendecomposition (not a Cholesky factor) is needed for (43) and (46). Fig. 6 footnote: "If the optimum is expected to be in the initial search interval [a, b]^n we may choose the initial search point, m, uniformly randomly in [a, b]^n, and sigma = 0.3(b - a). Different search intervals Delta s_i for different variables can be reflected by a different initialization of C ... Otherwise a scaling of the variables should be applied." Table 1 (eqs. 48 to 58): `lambda = 4 + floor(3 ln n)` ("can be increased"); `w'_i = ln((lambda + 1)/2) - ln i`; `mu = floor(lambda/2)`; `mu_eff = (sum_{i<=mu} w'_i)^2 / sum_{i<=mu} w'_i^2`; `mu_eff^- = (sum_{i>mu} w'_i)^2 / sum_{i>mu} w'_i^2`; `alpha_mu^- = 1 + c_1/c_mu`; `alpha_mu_eff^- = 1 + 2 mu_eff^-/(mu_eff + 2)`; `alpha_posdef^- = (1 - c_1 - c_mu)/(n c_mu)`; `w_i = w'_i / sum abs(w'_j)^+` for `w'_i >= 0`, `min(alpha_mu^-, alpha_mu_eff^-, alpha_posdef^-) w'_i / sum abs(w'_j)^-` for `w'_i < 0`; `c_m = 1`; `c_sigma = (mu_eff + 2)/(n + mu_eff + 5)`; `d_sigma = 1 + 2 max(0, sqrt((mu_eff - 1)/(n + 1)) - 1) + c_sigma`; `c_c = (4 + mu_eff/n)/(n + 4 + 2 mu_eff/n)`; `c_1 = alpha_cov/((n + 1.3)^2 + mu_eff)`; `c_mu = min(1 - c_1, alpha_cov (1/4 + mu_eff + 1/mu_eff - 2)/((n + 2)^2 + alpha_cov mu_eff/2))`, `alpha_cov = 2`. Page 32: the generalised `c_c` (61) as an alternative; "We do not recommend to change this setting, apart from increasing the population size lambda". B.2: `B` and `D` may be recomputed only every `max(1, floor(1/(10 n (c_1 + c_mu))))` generations. B.3 termination criteria: NoEffectAxis (adding a 0.1-standard deviation vector in a principal axis direction of `C` does not change `m`; footnote 31: `m` equals `m + 0.1 sigma d_ii b_i`, `i = (g mod n) + 1`), NoEffectCoord (`m_i` equals `m_i + 0.2 sigma c_ii` for any `i`; read as `0.2 sigma sqrt(c_ii)`, 0.2 standard deviations, below), ConditionCov (`10^14`), EqualFunValues (range of the best values of the last `10 + ceil(30 n/lambda)` generations zero), Stagnation (best and median fitness over the last 20 % but at least `120 + 30 n/lambda` and at most 20 000 iterations; stop if in both the median of the last 30 % is not better than the median of the first 30 %), TolXUp (`sigma max(diag(D))` increased by more than `10^4`), TolFun (`10^-12`, range of the best values of the last `10 + ceil(30 n/lambda)` generations and all values of the recent generation), TolX (standard deviation below TolX in all coordinates and `sigma p_c` below TolX in all components; `10^-12` times the initial `sigma`). B.4: flat fitness should rather be a termination. B.5: if the optimum is not too close to the boundary, set the fitness of an infeasible point to `f_max + norm(x - x_feasible)` (62), `f_max` larger than the worst feasible fitness, `x_feasible` a constant feasible point "preferably in the middle of the feasible domain", or resample; with a repair, penalise the distance to the repaired point (63), with `alpha` chosen so that the two summands have similar magnitude |
| R2 | A. Auger, N. Hansen, "A Restart CMA Evolution Strategy With Increasing Population Size", Proc. IEEE CEC 2005, 1769-1776 | the proceedings PDF (copy at sci2s.ugr.es, CEC 2005 contributions), pages 1 to 4 read as rendered page images | IPOP-CMA-ES: when a stopping criterion is met, "an independent restart is launched with the population size increased by a factor of 2"; all other parameters default. Stopping criteria: equalfunvalhist / Tolfun `10^-12`, TolX `10^-12 sigma0`, noeffectaxis, noeffectcoord, conditioncov `10^14`. Procedure: for each restart the initial point uniform in `[A, B]^n` and `sigma0 = (B - A)/2`; overall stop after `n x 10^4` evaluations or error below `10^-8`; boundary handling by a penalty on infeasible individuals. Table 2 (n = 10, 25 runs, success when the error reaches `10^-2` within `n x 10^4`): shifted Rastrigin (function 9) success rate 0.76, evaluations min 2.33e4, median 7.85e4; rotated shifted Rastrigin (10) 0.92; the observed largest final population 640 for n = 10 |

Not read: the tutorial's Appendix C (source code), section 0 to 5 in full (only the summary above), Appendix D; Hansen and Kern (2004,
PPSN VIII; no open copy found); pages 5 to 8 of R2.

## Decisions (fixed now)

**D1. The method: `rcfd_api::design::cmaes`.**

* **Coordinates.** CMA-ES runs in the unit cube: `u_i = (x_i - lo_i)/(hi_i - lo_i)`, `x_i = lo_i + u_i (hi_i - lo_i)` (R1, the Fig. 6
  footnote: a scaling of the variables), with `C(0) = I`, `p_sigma = p_c = 0`, `sigma(0) = sigma0` (option, default **0.3**, the
  footnote's `0.3 (b - a)` with `b - a = 1`).
* **Parameters** (`Params::new(n, lambda)`): exactly R1 Table 1 with `alpha_cov = 2`, `c_m = 1` and the `c_c` of (56); `lambda`
  defaults to `4 + floor(3 ln n)`; `mu_eff` and `mu_eff^-` in the form of Table 1's caption (from `w'`); `E norm(N(0, I))` by the
  approximation `sqrt n (1 - 1/(4n) + 1/(21 n^2))` (no Gamma function in std).
* **One generation** (`g` = the number of updates made in the current run before this generation, 0 at first):
  1. Sampling: for `k = 1..lambda`, `z_k` (n normal draws, component order), `y_k = B D z_k`, `u_k = m + sigma y_k`.
  2. Box: a sample with every `u_k,i` in [0, 1] is **in the box** and is evaluated at `x_k = lo + u_k (hi - lo)`, clamped into
     `[lo, hi]` against rounding. The in-box samples of the generation are one batch, in sample order (one call of the evaluation
     closure; none when no sample is in the box). An out-of-box sample is **not evaluated**.
  3. Ranking: the in-box samples first, by Deb's rules (`Fitness::better`; a stable sort, ties by sample index); then the
     out-of-box samples by their distance `norm(u_k - (0.5, ..., 0.5))` to the centre of the box, ascending, ties by index. This is
     R1's (62) for the box: every infeasible point ranks below every evaluated one, ordered by its distance to a constant feasible
     point in the middle of the domain.
  4. Update by R1 (41) to (47) in that order, with `y_{i:lambda}` and `z_{i:lambda}` of the ranked samples:
     `C^{-1/2} <y>_w = B sum w_i z_{i:lambda}` and `norm(C^{-1/2} y_{i:lambda}) = norm(z_{i:lambda})` (R1 page 28); `h_sigma` with
     `(1 - c_sigma)^(2(g+1))`, `g` as above, so that `1 - (1 - c_sigma)^(2(g+1))` is the variance factor of `p_sigma` after this
     update (it starts at 0; the tutorial's "generation number" read that way); `C` is made symmetric (`(C + C^T)/2`) after (47);
     then the eigendecomposition of the new `C` (D2) gives `B` and `D = diag(sqrt(eigenvalue))`, every generation (B.2's lazy
     update is not used), and `g` grows by one.
  5. Termination (R1 B.3), checked after the update in this order; the first that holds ends the run (`Termination`):
     * `NonFinite` (a guard of this ADR, not in R1): a non-finite entry of `m`, `sigma`, `C`, `p_sigma` or `p_c`, or a non-positive
       or non-finite eigenvalue;
     * `NoEffectAxis`: `m + 0.1 sigma D_ii b_i == m` in every component (floating-point sum), `i = g mod n` (0-based, `g` after the
       update), the eigenvalues in ascending order;
     * `NoEffectCoord`: for some `i`, `m_i + 0.2 sigma sqrt(C_ii) == m_i` (R1 writes `0.2 sigma c_ii` in the formula and "0.2-standard
       deviations" in words; the words are followed);
     * `ConditionCov`: largest over smallest eigenvalue of `C` above `1e14`;
     * `EqualFunValues`: with `W = 10 + ceil(30 n / lambda)`, `g >= W` and the best values of the last `W` generations all exist
       and are equal (range 0);
     * `Stagnation`: with `L = min(20 000, max(ceil(120 + 30 n / lambda), ceil(0.2 g)))` and `k = ceil(0.3 L)`, `g >= L`, the
       best and the median values of the last `L` generations all exist, and the median of the last `k` best values is not below
       the median of the first `k` of the window, and likewise for the median values;
     * `TolXUp`: `sigma max(D) > 1e4 sigma0`;
     * `TolFun`: `g >= W`, the last `W` best values exist, and the range of those values together with the values of the
       generation just made is below `1e-12`;
     * `TolX`: `sigma sqrt(C_ii) < 1e-12 sigma0` and `sigma abs(p_c,i) < 1e-12 sigma0` for every `i`.
     A generation's **best value** is `f` of its rank-1 sample when that sample was evaluated, is feasible and has a finite `f`;
     its **median value** is the mean of `f` of the samples of ranks `floor((lambda + 1)/2)` and `ceil((lambda + 1)/2)` (one sample
     when `lambda` is odd) when both are evaluated, feasible and finite; its **values** are `f` of its evaluated, feasible, finite
     samples. A generation without a needed value keeps the criterion that needs it from holding. A median is the middle value of
     the sorted list (the mean of the two middle values for an even count). All in unit-cube coordinates.
* **Restarts** (R2, opt-in, `restarts`, default **0**): when a run ends by a criterion and restarts are left, a new run starts with
  `lambda` doubled, `sigma = sigma0`, `C = I`, `p_sigma = p_c = 0`, `g = 0` and empty histories; its mean is the start point when
  one is given (clamped into the box), else a new uniform random point in the box. R2 draws every run's start uniformly in the
  box; the given start is kept here because the acceptance asks for a run "from a start" (question 5).
* **Budget.** `evaluations <= max_evals`: before a generation's batch is evaluated, if `evaluations + (in-box samples) >
  max_evals` the method ends (`Termination::Budget`; that generation is neither evaluated nor used, and no restart follows). The
  method also ends with `Budget` when the number of generations over all runs reaches `max_evals` (a guard for generations with no
  in-box sample; it cannot be reached while every generation evaluates a point).
* **Result**: the best evaluated point by Deb's rules (the first evaluated on a tie), as `OptResult { x, fitness, evaluations,
  converged }`, `converged` true when the method ended because the last run met a criterion with no restart left.
  `cmaes::run_report` also returns one `RunInfo { lambda, m0, generations, evaluations, stop }` per run.
* **Draw order** (`Rng::new(seed)`): without a start point, the first mean (`n` draws `uniform()`); then per generation `lambda x n`
  normal draws, sample after sample; a restart without a start point draws its mean before its first generation. A normal draw
  uses two uniforms: `z = sqrt(-2 ln(1 - u1)) cos(2 pi u2)` (Box-Muller, cosine branch only).
* **Refusals** (`Stop::Error`, nothing evaluated): no parameter; a bound that is not finite or with `lo >= hi`; `pop` below 2
  (`mu = floor(lambda/2) >= 1`); `sigma0` not finite or not positive; `max_evals` below the first population.
* **Public API**: `Options { max_evals, pop: Option<usize>, sigma0, restarts, seed, x0 }` with `Default` (1000, `None`, 0.3, 0, 1,
  `None`); `run(bounds, &Options, &mut Eval) -> Result<OptResult, Stop>`; `run_report(..) -> Result<(OptResult, Vec<RunInfo>),
  Stop>`; for the closed-form checks `Params::new`, `State` (public fields `m, sigma, c, p_sigma, p_c, b, d, g`; `State::new(m,
  sigma)`, `State::decompose()`), `update(&Params, &mut State, ranked_z)`, `History` and `termination(&Params, &State, &History,
  sigma0) -> Option<Termination>`, `normal(&mut Rng)`, `sym_eigen(matrix) -> (eigenvalues, eigenvectors)`.

**D2. The eigendecomposition.** Cyclic Jacobi rotations on the symmetric matrix (textbook method, from memory): sweeps over the pairs
`p < q` in row order, each zeroing `a_pq` by the rotation with `theta = (a_qq - a_pp)/(2 a_pq)`, `t = sign(theta)/(abs(theta) +
sqrt(theta^2 + 1))` (`t = 1` when `theta = 0`), `c = 1/sqrt(t^2 + 1)`, `s = t c`, the eigenvector matrix accumulated; sweeps until
the off-diagonal Frobenius norm is at most `1e-15` times the Frobenius norm, at most 100 sweeps; eigenvalues sorted ascending with
their columns (ties by index). K2 checks it.

**D3. The study.** A new variant `Method::CmaEs { budget, pop: Option<usize>, sigma0: f64, restarts, seed }`; `Study::run` calls
`cmaes::run` with `x0 = start_point()` and the store's batch evaluation, as for the other optimisers (so propose, record, resume,
`allowance` and `workers` work the same way).

**D4. The tools.** `{"type": "cma_es", "budget": B, "pop": N, "sigma0": s, "restarts": R, "seed": k}`: `budget` required (no
default, as for `nsga2`: a budget on a CFD case is a cost decision); `pop` optional (default the tutorial's lambda for the number of
parameters), at least 2; `sigma0` optional, default 0.3, a finite positive number; `restarts` default 0; `seed` default 1.
`method_json` writes `{type, budget, pop, sigma0, restarts, seed}` with `pop` `null` when not given. Refusals (-32602):
`method.budget: cma_es needs a budget (no default)`; `method.pop: cma_es needs a population of at least 2, got <N>`;
`method.sigma0: expected a positive number, got <v>`; `method.budget: <B> is smaller than the population <lambda0>` (in
`parse_study`, where the parameter count is known); `sigma0` on another method type: `method.sigma0: only cma_es takes this key`
(as ADR 0209 and ADR 0220 do for their keys; until now `sigma0` was ignored, and no `study.json` written by `method_json` holds it).
Two error texts change: the unknown-type message becomes `method.type: '<t>' is not nelder_mead, differential_evolution, nsga2,
cma_es, factorial, lhs or sobol` and the missing-method message names `cma_es` (no test asserts either). The MCP descriptor and
its description are unchanged (item 5; question 4).

**D5. What does not change.** Every existing method in the library, the study and the tools, their proposals, resume and
`study.json`; no case-schema key, case hash, baseline, golden or existing test file.

**D6. Commits.** (1) this text; (2) the V0b record (test only, run on main 539956da's code); (3) the library, the study and the
tools with `f260_cmaes.rs` (the tools come with the study change because `method_json` matches every variant); (4) the MCP test
`design_f260.rs`; (5) documents (`docs/design-loop.md`, `docs/reference-data.md`, the roadmap); (6) the addendum.

## Test problems (fixed now)

* **Shifted Rastrigin** `R_d(x) = 10 d + sum_i (t_i^2 - 10 cos(2 pi t_i))`, `t_i = x_i - o_i`, `o_i = 1.5 cos(i)` (`i = 1..d`, radians),
  box `[-5.12, 5.12]^d`; global minimum 0 at `o`. Start (P-R) `x0_i = o_i + 3 (-1)^(i+1)`, where `R_d(x0) = 9 d` (every `t_i = +-3`)
  next to a local minimum three basins from the optimum in every coordinate. d = 4: `o` = (0.8105, -0.6242, -1.4850, -0.9805),
  `x0` = (3.8105, -3.6242, 1.5150, -3.9805). d = 10: `o_5..o_10` = (0.4255, 1.4403, 1.1309, -0.2183, -1.3667, -1.2586), every
  `x0_i` inside the box. The optimum is not the centre of the box (the centre is the `x_feasible` of D1 step 3).
* **Sphere** `S(x) = sum x_i^2` and **ellipsoid** `E(x) = sum_i 10^(6 (i - 1)/(d - 1)) x_i^2` (condition `1e6`), d = 10, box
  `[-5, 5]^10`, start `(1, ..., 1)`.
* **Bounded sphere** `B(x) = sum (x_i - 6)^2`, d = 4, box `[-5.12, 5.12]^4`, start 0: its minimum `4 x 0.88^2` lies on the corner
  `x_i = 5.12` (record only).
* **Rosenbrock** `100 (y - x^2)^2 + (1 - x)^2` in `[-2, 2]^2` (ADR 0067 O1 / T3).

## Checks and limits (fixed now)

### Library checks (`crates/rcfd-api/tests/f260_cmaes.rs`)

| Id | Check | Limit |
|---|---|---|
| K0 | the problem code: `R_4(o) = 0`, `R_4(x0) = 36`, `R_10(x0) = 90`, `R_d(o + e_1) = 1` (one basin over); `E` at `e_10` = `1e6`, at `e_1` = 1 | within `1e-12` absolute |
| K1 | `Params::new(n, lambda)` for `(n, lambda)` in (1, 4), (2, 6), (3, 7), (4, 8), (10, 10), (30, 14) (the defaults), (2, 2), (4, 50): `lambda` and `mu` exact; the weights, `mu_eff`, `mu_eff^-`, `c_sigma`, `d_sigma`, `c_c`, `c_1`, `c_mu` and `E norm(N(0, I))` equal the test's own transcription of R1 Table 1; the positive weights sum to 1; the negative weights sum to `-min(alpha_mu^-, alpha_mu_eff^-, alpha_posdef^-)`; which of the three is the minimum is printed (recorded) | relative `1e-14` (sums `1e-14` absolute) |
| K2 | `sym_eigen`: 20 matrices `A = M M^T + 1e-3 I`, `M` uniform in [-1, 1] from `Rng::new(500 + s)`, size `1 + (s mod 12)`; and `[[2, 1], [1, 2]]` (eigenvalues 1, 3), `diag(3, 1, 2)` | `max abs(B diag(e) B^T - A) <= 1e-12 max abs(A)`, `max abs(B^T B - I) <= 1e-13`, ascending; the closed forms within `1e-15` |
| K3 | `normal`: 200 000 draws from `Rng::new(7)`: mean, variance, and the empirical distribution against `Phi` at t = -2, -1, 0, 1, 2 (`Phi` by composite Simpson of the density on `[-10, t]` with 20 000 intervals, in the test); the first 10 draws equal the Box-Muller formula of D1 applied to the same uniforms | `abs(mean) <= 0.01`, `abs(var - 1) <= 0.01`, `abs(F - Phi) <= 0.005`; replica bit for bit |
| K4 | one update (R1 (41) to (47)): `n = 3`, `lambda = 7`, `m = (0.2, 0.5, 0.7)`, `sigma = 0.3`, `C = [[1, 0.3, 0.1], [0.3, 2, -0.2], [0.1, -0.2, 0.5]]`, `p_sigma = (0.1, -0.2, 0.3)`, `p_c = (0.05, 0, -0.1)`, `g = 4`, ranked `z` = 7 vectors of `normal` from `Rng::new(11)`: `update` against the test's transcription of (41) to (47) using the state's `B` and `D`; case (b) the same with `p_sigma = (10, 0, 0)`, where the test's `h_sigma` must be 0 (set-up) | every entry of `m`, `sigma`, `p_sigma`, `p_c`, `C`: `abs(a - b) <= 1e-13 max(1, abs(b))`; `g` 5 |
| K5 | `termination` on built states and histories (n = 2, `lambda` = 6, `W` = 20, `sigma0` = 0.3; `m = (0.5, 0.5)`, `sigma = 0.1`, `C = I`, `p_c = 0`, empty histories, `g = 0` unless stated): `m = (1, 1)`, `sigma = 1e-17`: NoEffectAxis; `m = (1, 1)`, `sigma = 1e-14`: TolX (the order); `C = diag(1, 1e-30)`, `g = 1` (the axis of eigenvalue 1): NoEffectCoord; `C = diag(1, 1e-15)`: ConditionCov; `diag(1, 1e-13)`: `None`; `g = 20` with 20 equal best values: EqualFunValues; `g = 19` with 19: `None`; `g = 20` with one of the 20 larger by 1: `None`; `g = 200` with best and median values increasing (`t` and `t + 1` for generation `t`): Stagnation; decreasing (`-t`, `-t + 1`): `None`; `sigma = 3001`: TolXUp; `sigma = 2999`: `None`; `g = 20`, best values `1 + 5e-15 t` and the last generation's values within the same range: TolFun; the same with a range of `2e-12`: `None`; `sigma = 1e-13`: TolX; `sigma = 1e-13` with `p_c = (10, 0)`: `None`; `g = 20` with 20 equal best values of which one is missing: `None` | the stated reason or `None`, exactly |
| C1 | **the acceptance**: `R_4` from `x0`, `cmaes::run`, `max_evals` 40 000, `restarts` 9, `pop` default (8), `sigma0` 0.3, seeds 1 to 20; success when the best `f <= 1e-8`; the evaluator asserts that no requested point lies outside the box (exact comparison) | at least 18 of 20 seeds succeed |
| N0 | **set-up check, "a start where Nelder-Mead does not"**: `optim::nelder_mead` from `x0` on `R_4`, `max_evals` 40 000, `restarts` 9, seeds 1 to 20, with the default options and with `initial_simplex = Start`, `coefficients = Adaptive` | best `f > 1e-8` in all 40 runs |
| C1c | set-up check, the target is not trivial: 40 000 Latin-hypercube points (`sampling::latin_hypercube(40000, 4, 1)` scaled to the box) evaluated directly | best `f > 1e-8` |
| C1r | record: C1 with `restarts` 0 (one run, `lambda` 8); and for each C1 seed the population of the run that first reached `1e-8` and the evaluations then | none (recorded) |
| C2 | record: `R_10` from its `x0`, `max_evals` 100 000 (`n x 10^4`), `restarts` 9, seeds 1 to 20 (CMA-ES successes); Nelder-Mead as N0 (default options) | none (recorded) |
| U1 | sphere, `max_evals` 10 000, `restarts` 0, seeds 1 to 20 | best `f <= 1e-10` on all 20; the stop reasons recorded |
| U2 | ellipsoid, `max_evals` 20 000, `restarts` 0, seeds 1 to 20 | best `f <= 1e-10` on all 20 |
| U3 | record: bounded sphere, `max_evals` 10 000, `restarts` 0, seeds 1 to 20: `f - 4 x 0.88^2` and the largest `5.12 - x_i` of the best point | none (recorded) |
| U4 | record: ADR 0067 O1 with CMA-ES (Rosenbrock in `[-2, 2]^2`, random start from the seed, `max_evals` 800, `restarts` 3, seeds 1 to 20): how many reach `f <= 1e-6` with `abs(x - 1), abs(y - 1) <= 1e-3` | none (recorded) |
| S1 | study = library: `R_4`, `Method::CmaEs { budget: 5000, pop: None, sigma0: 0.3, restarts: 3, seed: 1 }` through `Study` (in-memory store, `workers` 1, every `init` = `x0`) against `cmaes::run` with the same options and `x0` | the records' parameters in order equal the points the library asked for, bit for bit; the same best point |
| S2 | determinism: the S1 study with `budget` 2000, `restarts` 2, seed 4: two runs give equal records; `workers` 1 and 4 give equal records; seeds 4 and 5 differ | exact |
| S3 | propose / record: the S2 study (seed 4) driven by `Study::propose` and `Study::record` gives the records of the evaluator-driven run (index, parameters, metrics) | exact |
| S4 | resume: the S2 study stopped after k = 37 new evaluations (`allowance`) and resumed from a file store gives the records of an uninterrupted run; the resume evaluates `total - 37`, a third launch 0 | exact |
| S5 | refusals (library): no parameter; `pop` 1; `sigma0` 0, -0.1, NaN and infinity; `max_evals` 7 with the default `lambda` 8 (d = 4); a bound with `lo = hi`: `Stop::Error` with nothing evaluated | as stated |
| S6 | box and budget: no point requested by any C, U, S run lies outside the box; `run_report` on the sphere in d = 4 (`max_evals` 1000, `restarts` 9, seed 1): evaluations `<= 1000`, equal to the evaluator's count, the last run's stop `Budget` | exact |
| S7 | restarts: in every C1 run (`run_report`) each run's `m0` equals `x0` bit for bit and each run's `lambda` is twice the previous; without a start point (the sphere in d = 2, box `[-5, 5]^2`, `max_evals` 5000, `restarts` 3, seed 1) at least two runs are made (set-up) and the runs' `m0` differ from each other and lie in the box | exact |

### Tool checks (`crates/rcfd-mcp/tests/design_f260.rs`, the MCP server in process as `design_f297.rs`)

| Id | Check | Limit |
|---|---|---|
| T1 | a `cma_es` study through the tools: `R_2` (the d = 2 shifted Rastrigin, box `[-5.12, 5.12]^2`) on dummy targets (`physics.nu`, `physics.rho` as ADR 0209 MO4), objective `f` min, `{"type": "cma_es", "budget": 300, "restarts": 2, "seed": 1}`, driven by propose / record until `complete`; a second study with the same inputs | `complete` reached; `study.json` `method` is exactly `{"budget": 300, "pop": null, "restarts": 2, "seed": 1, "sigma0": 0.3, "type": "cma_es"}`; the two first batches are equal; the records equal the distinct candidates proposed; `design_list_study`'s best record is the record of smallest `f` in `evaluations.jsonl`; best `f` recorded |
| T2 | refusals (-32602, the message names the key, nothing written): `cma_es` without `budget`; with `pop` 1; with `sigma0` 0 and `"a"`; with `budget` 5 (d = 2: `lambda0` 6); `nelder_mead` with `sigma0` 0.2; the type `cmaes` (the message names `cma_es`). Accepted: `{"type": "cma_es", "budget": 300, "pop": 10, "sigma0": 0.2}`, written with those values | as stated |

### Default bits (V0)

| Id | Check | Limit |
|---|---|---|
| V0a | `baseline_hashes.rs` and `case_hashes_w48u.rs` unedited; they pass in release | pass |
| V0b | `crates/rcfd-api/tests/f260_default_bits.rs`, recorded on main 539956da with this text and the test only, committed before any code: FNV-1a of (1) for a fixed list of method objects of the existing types, valid and invalid (not the unknown-type and missing-method texts, which D4 changes, and no `sigma0`), the `Debug` text of `parse_study`'s method or its error message; (2) for the valid ones, the `method` object of the `study.json` that `design.create_study` writes (`tools::call` on a sandboxed `Service`) | bit-identical after the code |
| V0c | the tools-list goldens unedited: `default_f1.rs`, `project_list_f270.rs`, `tool_list_f275.rs` pass; `design_f249.rs` and `design_f297.rs` pass | pass |
| V0d | every existing test passes unedited, among them `f249_default_bits.rs` and `f297_default_bits.rs` (the optimiser histories, the T3 loop and the NSGA-II records of the default paths) | pass |
| V0e | the gate (fmt per package, clippy `--workspace --all-targets -D warnings`, every test in release on a git-archive copy, `scripts/gate_test.py`, 3 jobs, under the machine gate lock, the free memory at the start disclosed) on the final commit; a known flaky test that fails is run again alone on the gate's build | pass, apart from the known flakes |

### Mutations (scratch, not committed)

Each is applied alone to the final tree, the named test is run once in release, and it must fail; the file is restored and
compared with the commit.

| Id | Mutation | Must fail |
|---|---|---|
| MC1 | `c_1 = c_mu = 0` in the run (no covariance adaptation) | U2 |
| MC2 | the exponent of the step-size update (44) with the opposite sign | U1 |
| MC3 | restarts keep `lambda` (no doubling) | S7 (recorded also: C1's count) |
| MC4 | out-of-box samples evaluated at their unclamped point | C1 (the box assertion) |
| MC5 | the in-box samples ranked by sample index instead of Deb's rules | U1 |
| MC6 | a restart starts from a uniform random point although a start is given | S7 |
| MC7 | `h_sigma` always 1 | K4 (b) |
| MC8 | the negative weights without the factor `n / norm(z)^2` | K4 |
| MC9 | EqualFunValues with the window `W - 1` | K5 |
| MC10 | the tools parse `cma_es`'s `budget` as 100 whatever is given | T1 |
| MC11 | `method_json` of `Method::NelderMead` also writes `"sigma0": null` | V0b |

### Classes and the decision rule

V0 is a precondition of landing: a miss of V0a, V0b or V0c means the change is not merged and the cause is recorded. N0, C1c and
K4 (b)'s `h_sigma = 0` are set-up checks: if one does not hold, or a mutation does not make its test fail, the check is not shown
to be able to fail; that is a defective set-up, corrected by an amendment before any re-run, never counted as a pass. A miss of any
other limit is reported with its number and kept as an ignored `*_known_miss` test with the limit intact.

**Decision rule.** C1 with N0 and C1c is clause 1 of the acceptance. Clause 1 is **met** when C1 and every other registered check
are met, the set-up checks hold and every mutation fails its test. Clause 2 (the FAN-01 study) is not attempted (reading 2), so
F-260 is **partial** in the roadmap in every case, with the note naming clause 1's result and question 1. A miss of C1: partial,
with both clauses open and the numbers recorded.

## Predictions (mine)

| Id | Prediction |
|---|---|
| PC1 | C1 met on 20 of 20 seeds; the target is first reached in a run with `lambda` between 32 and 128, after 5 000 to 25 000 evaluations |
| PC2 | N0 holds: every Nelder-Mead run ends next to the local minimum near `x0` (`f` about 35.8) |
| PC3 | C1r: 1 to 8 of 20 seeds without restarts |
| PC4 | C2: 10 to 18 of 20 (R2's 0.76 for uniform starts at the looser `1e-2`) |
| PC5 | U1 under 3 000 evaluations, U2 under 10 000, on every seed |
| PC6 | U3: `f - f*` below `1e-6` on most seeds (low confidence: half of the samples near a corner are not evaluated) |

## Budgets

In release: C1 under 10 s, N0 under 2 s, C2 under 30 s, U1 to U4 under 10 s, S1 to S7 under 10 s, K under 2 s; V0b under 2 s; the
MCP tests under 30 s.

## Not in scope

The FAN-01 study (reading 2, question 1); R1's repair-and-penalty (63) and resampling for the box (question 2); the generalised
`c_c` (61) and the lazy eigendecomposition of B.2; uniform restart points with a start given (question 5); the descriptor text
(question 4); separable or diagonal variants, mirrored sampling, or noise handling.

## Open questions for the maintainer (each with a recommendation)

1. **F-260's status and the real-fan study.** Options: (a) split the FAN-01 study into a new item that depends on a converged
   FAN-01 solve (the stall line of ADRs 0110 to 0181) and on blade-parameter keys, and set F-260 done when clause 1 is met;
   (b) keep F-260 partial until FAN-01 converges; (c) run the study now on the first-order passage of ADR 0176 as "the converged
   solver" (it ends `not_converged` at 1500 iterations there, and first order underpredicts the torque by about 35 %).
   Recommendation: (a); the roadmap follows the decision rule (partial) until the maintainer decides.
2. **The box.** As built, an out-of-box sample is ranked last and not evaluated (R1 (62)), which costs no evaluation but ignores
   the objective there, so an optimum on a bound is approached from one side only (U3 records it). Options: (a) as built; (b) a
   follow-up option for R1's repair-and-penalty (63), whose weight `alpha` is problem dependent. Recommendation: (a) unless U3 or a
   design study shows a poor bound optimum.
3. **Defaults.** `restarts` 0 (one CMA-ES run) and `sigma0` 0.3 (R1's footnote), against R2's restarts and `(B - A)/2`.
   Recommendation: keep, since a restart policy on a CFD case is a cost decision; the documentation names `restarts` for
   multimodal problems.
4. **The tool description.** `design_create_study`'s description does not name `nsga2` or `cma_es` (ADR 0209 Q6, ADR 0220 Q5,
   F-310); changing it changes both tools-list goldens. Recommendation: one registered edit of the description and both goldens,
   with F-310.
5. **Restart points.** As built, every restart starts from the given start; R2 starts each run at a uniform random point.
   Options: (a) as built; (b) R2's rule as an option. Recommendation: (a), recorded against C1r and C2.

## Addenda

Results follow in addenda.

### Addendum 1: what was built and the results (2026-11-08)

Commits on `f260-cmaes` (from main 539956da): registration 940caa4; V0b record c785636 (test only, recorded twice on main's code
with equal values before any code); a128912 (`design::cmaes`, `Method::CmaEs`, the tools' `cma_es`, with `f260_cmaes.rs`: the
tools come with the study change because `method_json` matches every `Method` variant, as D6 says); 78ca950 (`design_f260.rs`);
documents and this addendum after it. Every check was run once, in release: the library checks and V0b on a128912's tree, the
tool checks on 78ca950's. No limit, set-up or test was changed after a run.

Built as registered, with four disclosed differences, none of which changes a check:

* `State` has one more public field than D1 lists, `eig` (the eigenvalues), which ConditionCov reads instead of squaring `D`.
* The V0b test file was committed (c785636) before `cargo fmt`; a128912 formats it (line breaks only; the recorded values are
  unchanged and are met after the code).
* Before the registered run, clippy's `manual_div_ceil` made the median ranks read `lambda.div_ceil(2) - 1` and
  `(lambda + 1).div_ceil(2) - 1` (the same integers as `floor((lambda + 1)/2) - 1` and `ceil((lambda + 1)/2) - 1`), and the S5
  closure uses the `optim::Eval` alias.
* `cmaes.rs` has one unit test that is not in the registration (`refusals_evaluate_nothing`, the library refusals of S5 again).

#### Results

| Id | Measured | Limit | Result |
|---|---|---|---|
| K0 | `R_4(o) = 0`, `R_4(x0) = 36`, `R_10(x0) = 90`, `R_4(o + e_1) = 1`, `R_10(o) = 0`, `E(e_10) = 1e6`, `E(e_1) = 1` | `1e-12` | met |
| K1 | all eight `(n, lambda)` within `1e-14` relative; the positive weights sum to 1 and the negative weights to `-min(alpha)` within `1e-14`. The smallest alpha (recorded): `alpha_mu_eff^-` for (1, 4), (2, 6), (3, 7) and (2, 2); `alpha_mu^-` for (4, 8), (10, 10), (30, 14); `alpha_posdef^-` for (4, 50) | as stated | met |
| K2 | 20 seeded matrices (sizes 1 to 12): reconstruction at most `5.7e-15 max abs(A)`, orthogonality at most `2.9e-15`, ascending; `[[2, 1], [1, 2]]` gives 1 and 3, `diag(3, 1, 2)` gives 1, 2, 3 with the permuted unit vectors | `1e-12`, `1e-13`, `1e-15` | met |
| K3 | mean 0.00363, variance 0.99669; `abs(F - Phi)` 2.9e-4, 8.4e-4, 1.7e-3, 1.6e-4, 5e-5 at t = -2, -1, 0, 1, 2; the first 10 draws equal the formula bit for bit | 0.01, 0.01, 0.005 | met |
| K4 | (a) `h_sigma` 1 and (b) `h_sigma` 0 (set-up held): `m`, `sigma`, `p_sigma`, `p_c`, `C` within `1e-13` of the transcription; `g` 5 | `1e-13` | met |
| K5 | the 17 cases give the registered reason or `None` | exact | met |
| **C1** | **20 of 20 seeds reach `f <= 1e-8`** (the best `f` evaluates to 0 in floating point on every seed). The target is first reached in the run with `lambda` 8 (seeds 6, 8, 18), 16 (10), 32 (4, 7, 12, 14, 16, 17, 19), 64 (1, 2, 5, 9, 15, 20), 128 (3) or 256 (11, 13), after 731 to 28 727 evaluations (median 6 029); every seed then spends the budget (39 505 to 39 959 evaluations in 7 runs, `lambda` 8 to 512, the first six runs stopped by TolFun, the last by the budget) | at least 18 of 20 | **met** |
| N0 | all 40 Nelder-Mead runs (20 seeds, default options and `start` + `adaptive`) end at `f = 35.818405`, the local minimum next to `x0`, after 2443 to 3289 evaluations (every restart converges back to it) | all above `1e-8` | held (set-up) |
| C1c | best `f` of the 40 000 Latin-hypercube points 8.7728 | above `1e-8` | held (set-up) |
| C1r | without restarts (one run, `lambda` 8): 3 of 20 seeds (6, 8, 18) reach the target; the others stop by TolFun in local minima with `f` 0.995 to 11.46 | none | recorded |
| C2 | `R_10`, 100 000 evaluations: CMA-ES 14 of 20 seeds (first success after 46 725 to 85 245 evaluations; the six others end at `f = 0.995`, one basin off in one coordinate); Nelder-Mead 0 of 20 (every run at `f = 89.546`) | none | recorded |
| U1 | sphere: 20 of 20, `f` 4.1e-16 to 9.4e-15 after 2308 to 2671 evaluations, every run stopped by TolFun | `1e-10` on 20 | met |
| U2 | ellipsoid (condition `1e6`): 20 of 20, `f` 5.6e-16 to 4.7e-15 after 4631 to 5351 evaluations, every run stopped by TolFun | `1e-10` on 20 | met |
| U3 | bounded sphere: `f - f*` 1.2e-11 to 7.8e-11, largest `5.12 - x_i` at most 2.9e-11, 2137 to 2908 evaluations, every run stopped by TolX | none | recorded |
| U4 | Rosenbrock from random starts (the ADR 0067 O1 rule): 20 of 20 within 789 to 799 evaluations (1 or 2 runs) | none | recorded |
| S1 | 4975 records equal the 4975 points the library asked for, bit for bit; the same best point (`f` 0.995) | exact | met |
| S2 | seed 4: 1995 records, equal in two runs and with 4 workers; seed 5 (1994 records) differs | exact | met |
| S3 | 218 proposals, 1995 records equal to the evaluator-driven run | exact | met |
| S4 | k = 37: the resume evaluated 1958 of 1995, a third launch 0; records equal | exact | met |
| S5 | the 8 refusals (`no parameters`; `the population must be at least 2, got 1`; `sigma0 must be positive and finite, got 0`, `-0.1`, `NaN`, `inf`; `the evaluation budget 7 is smaller than the population 8`; `parameter 1: the bounds must be finite with lo < hi (got 2, 2)`), nothing evaluated | as stated | met |
| S6 | no point outside the box in any C, N, U or S run; the sphere run with `max_evals` 1000: 1000 evaluations (1000 counted) in 2 runs, stopped by TolFun and then by `Budget` | exact | met |
| S7 | every C1 run starts at `x0` bit for bit, with `lambda` 8, 16, ..., 512; without a start point, 4 runs from 4 different points in the box | exact | met |
| T1 | `study.json` `method` `{"budget":300,"pop":null,"restarts":2,"seed":1,"sigma0":0.3,"type":"cma_es"}`; the two first batches equal; 50 proposals to `complete`, 296 records for 296 distinct candidates; `design_list_study`'s best is record 290, the smallest `f` of `evaluations.jsonl` (0.995, recorded) | as stated | met |
| T2 | the seven refusals, each -32602 naming its key (`method.budget: cma_es needs a budget (no default)`; `method.pop: cma_es needs a population of at least 2, got 1`; `method.sigma0: expected a positive number, got 0` and `got a`; `method.budget: 5 is smaller than the population 6`; `method.sigma0: only cma_es takes this key`; `method.type: 'cmaes' is not nelder_mead, differential_evolution, nsga2, cma_es, factorial, lhs or sobol`); nothing written; the accepted study written as `{"budget":300,"pop":10,"restarts":0,"seed":1,"sigma0":0.2,"type":"cma_es"}` | as stated | met |
| V0b | after the code: parsed `0x9f0ff07822512596`, written `0x5a06b394eb85c3a8`, equal to the record | bit-identical | met |
| V0a, V0c, V0d | `baseline_hashes.rs`, `case_hashes_w48u.rs`, `Cargo.lock` and the goldens unedited (`git diff 539956da` empty for them); no existing test file changed (the modified source files are `design/mod.rs`, `study.rs` and `tools.rs`; three test files and `cmaes.rs` added) | pass | see the gate |

**Predictions.** PC1 met for the count (20 of 20) and falsified in part for the population and the cost: 14 of 20 seeds first
reached the target with `lambda` 32 to 128 as predicted, but three with 8, one with 16 and two with 256, after 731 to 28 727
evaluations (predicted 5 000 to 25 000). PC2 met (35.818405 on every run). PC3 met (3 of 20). PC4 met (14 of 20). PC5 met (at most
2671 and 5351 evaluations). PC6 met (`f - f*` below `1e-10` on every seed).

**A finding on the tutorial.** Page 28 says that "for the default population size lambda, we meet the lower bound `c_mu sum w_j =
-c_1`". That holds when `alpha_mu^-` is the smallest of the three alphas of (53). K1 shows that with the default population for
n = 1, 2 and 3 `alpha_mu_eff^-` is the smallest, so there `c_1 + c_mu sum w_j > 0` and (47) keeps a small decay of `C`. rcfd follows
Table 1 (the minimum of the three), as registered.

#### Mutations

Each applied alone to 78ca950 by the scratch script `w49i-work/f260-mut/apply.py`, its test run once in release, the file restored
and compared with the commit (`git diff --quiet HEAD -- crates` and no untracked file under `crates`: clean after every one).

| Id | Test | Measured | Result |
|---|---|---|---|
| MC1 | U2 | 0 of 20 seeds reach `1e-10`; best `f` 79 to 507 when the 20 000 evaluations run out | fails |
| MC2 | U1 | 0 of 20; every run stops by TolXUp with `f` 18 to 55 (seed 1: 5 evaluations in 108 generations, the samples leaving the box) | fails |
| MC3 | C1 (its S7 assertion) | run 1 has `lambda` 8 instead of 16; recorded with a scratch copy of C1 without the S7 assertions: 4 of 20 seeds reach the target with ten runs of `lambda` 8 | fails |
| MC4 | C1 (its box assertion) | seed 1 asks for `x = -6.738` outside `[-5.12, 5.12]` | fails |
| MC5 | U1 | 0 of 20; `f` 5.9 to 25 (seed 1 stops by Stagnation) | fails |
| MC6 | C1 (its S7 assertion) | run 1 of seed 1 starts at a random point | fails |
| MC7 | K4 (b) | `p_c` -0.6147 against 0.02206 (the transcription's `h_sigma` 0) | fails |
| MC8 | K4 (a) | `C` entry 1.0249 against 0.9465 | fails |
| MC9 | K5 | 19 equal best values give EqualFunValues instead of `None` | fails |
| MC10 | T1 | `study.json` `budget` 100 for 300 | fails |
| MC11 | V0b | written hash `0x616565bd5555ba40` against the record `0x5a06b394eb85c3a8` | fails |

**Status.** Every registered check is met, the set-up checks (N0, C1c, K4 (b), S7's two runs) hold and every mutation fails its
test, so clause 1 of the acceptance is met. Clause 2, the FAN-01 study, is not attempted (reading 2). By the decision rule F-260 is
**partial** in the roadmap, subject to the gate below; question 1 asks whether to split the real-fan study off and call F-260 done.
