# ADR 0621: F-698 registration: a Gaussian-process surrogate over the design ledger and an expected-improvement proposal of the next variant

Status: **DRAFT STUB 2026-11-15, not registered.** The maintainer answered the open questions on 2026-11-15 (section \"Maintainer decisions\"); the limits are still proposals until the implementing agent registers them. Written by a drafting agent for the roadmap loop, before any code of F-698.
Nothing in the repository was edited, no cargo build or test was run. The limits in the check table are proposals; they become
fixed when the implementing agent registers this text (after the smoke runs named in "Before registration") and are not changed
after a registered run. Disclosed: a rough numpy prototype (about 100 lines, written from the textbook formulas, not rcfd code,
kept outside the repository, beside this draft as `gp_proto.py`); its numbers are quoted under "Estimates behind
the limits". The open questions at the end are for the maintainer; none is taken here.

Roadmap item F-698 (proposed deps F-268, F-249, F-697), part H3 of [fan-project-breakdown.md](../fan-project-breakdown.md):
"Surrogate model (for example Gaussian process) fitted to past evaluations to choose the next variant cheaply". Number 0621
given by the lead.

Parents: [ADR 0067](0067-wave32c-design-loop.md) (the design loop, the store, propose / record, the batch interface of the
optimisers), [ADR 0121](0121-wave46d-design-ledger-and-tolerance-study-driver.md) (the design ledger, the `Model` trait),
[ADR 0240](0240-f268-registration-sobol-sensitivity-indices.md) (a library driver over `Model` that logs to the ledger: the
pattern followed here), [ADR 0209](0209-f249-registration-nsga2-studies-nelder-mead-options-and-extended-warm-start.md),
[ADR 0220](0220-f297-registration-nsga2-per-variable-exchange-and-the-30-parameter-zdt1-check.md) and
[ADR 0222](0222-f260-registration-cma-es-and-the-real-fan-study.md) (the optimisers this sits beside),
[ADR 0119](0119-wave46b-reduced-order-axial-fan-tier.md) (the reduced-order fan, part C7), ADR 0620 (F-697, the objective
definition, drafted in parallel; assumed below to give one scalar, or one vector, per variant).

House rules (as ADR 0222): limits fixed before the code and the runs; a miss becomes an ignored `*_known_miss` test with the limit
intact; each registered run made once; every new path opt-in and the defaults bit-identical (`baseline_hashes.rs`,
`case_hashes_w48u.rs`, the tools-list goldens and every existing test file unedited); no case-schema key; no new third-party
crate, `Cargo.lock` unchanged; no commit trailers; clean-room.

Clean-room and sources: the method is taken from the published texts in "References" (Gaussian-process regression from Rasmussen
and Williams 2006; the expected-improvement criterion from Jones, Schonlau and Welch 1998) and from first principles. No source
listing of any Gaussian-process, kriging or Bayesian-optimisation code is to be opened, and no vendor documentation. The
references below are cited from memory by this draft; the implementing agent reads them and records in
`docs/reference-data.md` what was read and what was not, as ADR 0222 did.

## What H3 asks

A CFD evaluation of a fan variant will cost minutes to hours. NSGA-II, CMA-ES, differential evolution and Nelder-Mead decide the
next point from a population or a simplex and need hundreds to thousands of evaluations. H3 asks for a model fitted to all the
evaluations made so far that predicts the objective, with an uncertainty, at any untried variant, and that picks the next variant
where the expected gain is largest. The fit and the pick cost milliseconds to seconds, so they pay for themselves as soon as an
evaluation costs more than that.

## What the code read shows (read only, 2026-11-15)

* `crates/rcfd-api/src/design/ledger.rs` (ADR 0121): `Entry { kind, study, variant, params: Vec<Quantity>, evaluator,
  evaluator_version, metrics: Vec<Quantity>, verdict, reason, fidelity, seed, provenance, .. }`; `Verdict` is `Ok`, `Diverged`,
  `ConstraintRejected` or `NotEvaluated`; `Fidelity` is `Analytic`, `ReducedOrder` or `Cfd`; `Ledger::read`, `Ledger::append`,
  `Filter`, `best_so_far`, `pareto_front`; the `Model` trait (`id`, `version`, `fidelity`, `evaluate(&[Quantity]) -> Outcome`) and
  `FnModel` for closures.
* `design/optim.rs`: `Fitness { f, violation }` (`+inf` for a failed evaluation, Deb's rules), the batch callback
  `Eval = dyn FnMut(&[Vec<f64>]) -> Result<Vec<Fitness>, Stop>`, `Bounds`, `OptResult`, `nelder_mead`, `differential_evolution`,
  `projected_gradient` (a caller-supplied gradient, Armijo backtracking). Every optimiser is a deterministic function of (bounds,
  options, seed, evaluation results), which is what lets `design_propose` replay a method against the store with no saved state.
* `design/study.rs`: `Method` has `Doe`, `NelderMead`, `NelderMeadWith`, `Differential`, `Nsga2`, `Nsga2With`, `CmaEs`;
  `Study::run`, `Study::propose`, `Study::record`. ADR 0222 point 5: the MCP descriptor of `design_create_study` takes `method` as
  a free object, so a new method type needed no descriptor change and no golden change there.
* `design/sensitivity.rs` and `design/uq.rs`: library drivers `run(spec, model: &dyn Model, ledger: Option<&Ledger>)`; no service
  method, because a Rust closure cannot cross MCP.
* `design/fan_rom_eval.rs` and `crates/rcfd-api/tests/fan_rom_design_w46b.rs`: `FanRomEvaluator` gives `q`, `dp_s`, `dp_t`,
  `power`, `torque`, `eta_s`, `eta_t`, `tone1_pa` per condition in well under a millisecond, and a stalled or missing operating
  point is a failed condition with a located reason. This is the first non-analytic target.
* `design/sampling.rs`: `latin_hypercube(n, d, seed)`, `sobol(n, d)` with `SOBOL_MAX_DIM = 10`, `scale`.
  `design/uq.rs`: `norm_cdf`, `norm_inv` (through `rcfd_reactor::mixfrac::erfc`). `design/cmaes.rs`: `normal(&mut Rng)`
  (Box-Muller), `sym_eigen`. `rcfd_core::rng::Rng`: `new(seed)`, `uniform()`, `range`, `below`, `shuffle`.
* Linear algebra: `rcfd-api` already depends on `rcfd-linalg`. `rcfd_linalg::eigen::symmetric_eigen` (dense, cyclic Jacobi) is
  public. A dense Cholesky factorisation exists four times, each private to its file (`eigen.rs` `generalized_unchecked`,
  `eigen_general.rs` `cholesky`, `amg_elastic.rs` `spd_inverse`, `precond.rs` block Jacobi); there is **no public dense
  Cholesky**, and no dense triangular solve. The matrices here are dense, symmetric positive definite and small (n by n for n
  evaluations, n at most a few thousand), so about 40 lines of Cholesky and two triangular solves are needed; no crate
  (question 9).
* No surrogate, kriging or Gaussian-process code exists. [design-loop.md](../design-loop.md) lists "surrogate models" under "Not
  delivered"; [ADR 0136](0136-fidelity-tiers-xhigh-to-xlow.md) lists H3 and H7 as not done.

## Decision proposal

### D1. Where

Two new files in `crates/rcfd-api/src/design/`, both library only and called by nothing that exists:

* `surrogate.rs`: the Gaussian process (kernel, fit, prediction, leave-one-out diagnostics). It knows nothing of the ledger or
  the optimisers.
* `bayes.rs`: the proposal (observations from a ledger or a store, acquisition, its maximisation, the batch rule, the driver
  `run` over a `Model`, the ledger entries).

Whether the method also becomes a `Method` of `study.rs`, and so reachable through the existing MCP study tools, is question 1.

### D2. Coordinates and scaling

Inputs are mapped to the unit cube with the declared bounds, `u_i = (x_i - lo_i)/(hi_i - lo_i)`, as CMA-ES does (ADR 0222 D1).
A parameter with `lo >= hi` or a non-finite bound is refused. Outputs are standardised for the fit (subtract the mean of the
usable values, divide by their standard deviation; a zero standard deviation is replaced by 1) and predictions are mapped back.
The objective is minimised; a `Sense::Maximize` objective is negated on the way in.

### D3. The model (Rasmussen and Williams 2006, chapter 2; Jones, Schonlau and Welch 1998, section 2)

`y(u) = mu + Z(u) + e`, with `Z` a zero-mean Gaussian process of covariance `s2 k(u, u')` and `e` independent noise of variance
`s2 g` (`g` is the nugget ratio).

* **Kernel** (question 2): Matern with smoothness 5/2 and one length scale per parameter,
  `k(u, u') = (1 + r + r^2/3) exp(-r)`, `r = sqrt(5 sum_i (u_i - u'_i)^2 / l_i^2)` (Rasmussen and Williams eq. 4.17; Stein 1999
  for the family). The squared exponential `exp(-r^2/2)` (with `r` without the factor `sqrt 5`) is the second kernel, opt-in.
* **Mean**: an unknown constant `mu` (ordinary kriging, as Jones et al.), estimated by generalised least squares:
  `mu = (1^T K^-1 y)/(1^T K^-1 1)` with `K = k(U, U) + (g + j) I`.
* **Process variance**: `s2 = (y - mu 1)^T K^-1 (y - mu 1)/n` (the maximum-likelihood value; `mu` and `s2` are profiled out, so
  the search is over the `d` length scales and, when noise is on, `g`).
* **Jitter** `j = 1e-8` is always added to the diagonal (relative to the unit prior variance) so that the factorisation succeeds
  for near-duplicate points. If the Cholesky pivot test fails, the jitter is multiplied by 100 and the factorisation repeated, at
  most three times (to `1e-2`); then the fit is refused with a located message. The jitter used is reported.
* **Prediction** at `u` with `k* = k(U, u)`, `v = L^-1 k*`:
  mean `m(u) = mu + k*^T K^-1 (y - mu 1)`;
  variance `s^2(u) = s2 [1 - v^T v + (1 - 1^T K^-1 k*)^2 / (1^T K^-1 1)]` (the last term is the uncertainty of `mu`; Jones et
  al. eq. 9, Rasmussen and Williams eq. 2.42 with one basis function), floored at 0. This is the variance of the latent function;
  the variance of a new noisy observation adds `s2 g`.
* **Factorisation**: dense Cholesky `K = L L^T`, row-major lower, with the relative pivot test the existing private copies use
  (`pivot <= (row + 1) eps K_ii` refuses). Solves by forward and back substitution. Cost `n^3/3` per likelihood value.
* **Leave-one-out** residuals and variances from the one factorisation (Rasmussen and Williams eq. 5.10 to 5.12, extended for
  the estimated mean; Dubrule 1983), reported as the standardised residuals `(y_i - m_-i)/s_-i`. They are the model check a user
  can run with no extra evaluation (Jones et al. section 2, the diagnostic plots).

### D4. Hyperparameter fit

Maximise the concentrated log likelihood `-(n/2) ln s2 - sum_i ln L_ii` (constants dropped) over `ln l_i` in
`[ln 0.05, ln 10]` (unit-cube lengths) and, with noise on, `ln g` in `[ln 1e-8, ln 1]`.

* Starts: the centre of the box in log space and `S - 1` further points of a seeded Latin hypercube in the box; `S = 10`
  (question 3 gives the search method and the number).
* From the best `3` starts, a local search; the best value wins, ties by start order.
* Draw order from `Rng::new(seed)` is fixed and written in the registered text, as ADR 0222 did, so that the fit is a
  deterministic function of (observations, options, seed).
* Bounds on the length scales are the regularisation; there is no prior on the hyperparameters and no averaging over them
  (question 3 (c) names the alternative).
* Refusals (located messages, nothing evaluated): fewer than `d + 2` usable observations (the initial design of D7 covers this
  case), a non-finite input or output, more than `n_max = 2000` observations (question 10).

### D5. Noise (question 4)

Default: noise-free interpolation (`g = 0`, jitter only). This is right for the analytic functions and for the reduced-order fan,
which are deterministic. Opt-in `Noise::Fitted`: one nugget ratio `g` for all points, fitted with the length scales
(homoscedastic). Opt-in `Noise::Fixed(sd)`: a known standard deviation in output units, for a CFD objective whose iteration or
sampling error has been measured. Two ledger entries at the same point (within `1e-12` in the unit cube): with noise off their
mean is one observation; with noise on they stay two observations. With noise on, the incumbent is the lowest posterior **mean**
over the evaluated points, not the lowest observed value (the plug-in rule; Picheny, Wagner and Ginsbourger 2013).

### D6. Acquisition (Jones, Schonlau and Welch 1998 eq. 15; Mockus, Tiesis and Zilinskas 1978)

Expected improvement over the incumbent `f_min`: with `z = (f_min - m)/s`,
`EI(u) = (f_min - m) Phi(z) + s phi(z)` for `s > 0`, and `max(f_min - m, 0)` for `s = 0`; `Phi` from `uq::norm_cdf`.

* **Constraints of the study** (question 6): each inequality metric gets its own Gaussian process, and the acquisition is EI
  times the product of the probabilities that each constraint holds (Schonlau, Welch and Jones 1998; Gardner et al. 2014). With
  no feasible point yet, the acquisition is the probability of feasibility alone.
* **Maximising it**: `N_c = 1000 d` candidates in the unit cube (a seeded Latin hypercube; the Sobol sequence stops at 10
  parameters) plus `200` normal perturbations of standard deviation `0.02` and `0.1` around each of the best 5 evaluated points,
  clamped into the box; the best 5 candidates by acquisition are each refined by the existing `optim::nelder_mead` (budget
  `100 d`); the best refined point wins, ties by candidate order. A candidate closer than `1e-6` (unit cube, Euclidean) to an
  evaluated or pending point is excluded.
* **Stop**: `Complete` when the largest EI is below `1e-6` of the standard deviation of the observed values on three proposals in
  a row (a rule in the spirit of Jones et al., who stop at 1 percent of the best value), or at the budget.
* **Batch of q points** (question 7): the kriging-believer rule (Ginsbourger, Le Riche and Carraro 2010): after a point is
  chosen it is added as if its value were the predicted mean, hyperparameters held, and the next is chosen. `q = 1` by default.
  Ledger entries with verdict `not_evaluated` (proposed, running) are treated the same way, so that two sessions do not propose
  the same point.

### D7. Initial design

With fewer than `n0` usable observations the proposal is the next point of a seeded Latin hypercube of `n0` points that is not
yet in the ledger; no model is fitted. `n0 = max(5 d, 10)` (question 5; Jones et al. use about `10 d`).

### D8. Failed evaluations (question 6)

The ledger has three kinds of entry that carry no objective value:

* `constraint_rejected` (refused before evaluation: geometry, bounds): cost nothing. They train a feasibility model only.
* `diverged` (the evaluation ran and failed; for the reduced-order fan, a stalled operating point): they must push later
  proposals away, or EI returns to the same hole because the uncertainty there stays high. Proposal: imputation after Forrester,
  Sobester and Keane (2006): fit on the `ok` points; give each failed point the value `m(u) + s(u)` of that fit (predicted mean
  plus one standard deviation, a pessimistic value that is still finite); refit on all points. The entry itself is never
  changed, and the summary states how many values were imputed.
* `not_evaluated`: pending, D6.

A fit with fewer than `d + 2` `ok` points uses the initial design instead. An objective that is not finite on an `ok` entry is
treated as `diverged` and counted.

### D9. Reading the ledger

`bayes::observations(entries, &Filter, &params, &objective)` returns the rows in a canonical order (sorted by the unit-cube
coordinates, then by ledger index), so that the proposal does not depend on the order in which sessions appended. Rows are taken
from entries of `kind = evaluation` that pass the filter and carry every declared parameter with the declared unit; an entry
missing a parameter is skipped and counted. One fit uses **one fidelity tier and one evaluator id**; entries of another tier or
evaluator are refused unless the filter excludes them (mixing tiers is H7, not this item). A change of `evaluator_version` among
the rows is reported in the result and, by default, refused (question 8).

The objective is what F-697 (ADR 0620) defines: assumed here as a function from an entry's metrics to one finite number (or
`None`), or to a vector for question 11. Until F-697 is merged the checks use "metric name plus sense", which `ledger::best_so_far`
already takes.

### D10. Driver and ledger entries

`bayes::run(spec, model: &dyn Model, ledger: Option<&Ledger>)`, as `sensitivity::run`: read, propose, evaluate, append, repeat to
the budget. Each proposal is appended as an `evaluation` entry with provenance
`{driver: "gp_ei", iteration, acquisition: "ei", ei, predicted_mean, predicted_sd, incumbent, length_scales, nugget, jitter,
n_obs, n_imputed}` and the seed, so that every pick can be audited afterwards and the predicted value compared with the result.
One `surrogate_summary` entry closes a run (best value, evaluations, failures, the leave-one-out statistics).

### D11. Interface (sketch, names to be fixed at registration)

```rust
// surrogate.rs
pub enum Kernel { Matern52, SquaredExponential }
pub enum Noise { None, Fitted, Fixed(f64) }
pub struct GpOptions { pub kernel: Kernel, pub noise: Noise, pub starts: usize, pub seed: u64 }
pub struct Gp { /* u, y, length_scales, nugget, jitter, mu, s2, L, alpha */ }
impl Gp {
    pub fn fit(u: &[Vec<f64>], y: &[f64], o: &GpOptions) -> Result<Gp, String>;
    pub fn with_hyper(u: &[Vec<f64>], y: &[f64], kernel: Kernel, length_scales: &[f64], nugget: f64) -> Result<Gp, String>;
    pub fn predict(&self, u: &[f64]) -> (f64, f64);            // mean, standard deviation of the latent function
    pub fn log_likelihood(&self) -> f64;
    pub fn loo(&self) -> Vec<(f64, f64)>;                      // leave-one-out mean and standard deviation
}
pub fn cholesky(a: &[f64], n: usize) -> Result<Vec<f64>, (usize, f64)>;
// bayes.rs
pub fn expected_improvement(mean: f64, sd: f64, incumbent: f64) -> f64;
pub enum Observation { Value(f64), Failed, Rejected, Pending }
pub struct BayesOptions { pub gp: GpOptions, pub n0: usize, pub batch: usize, pub budget: usize, pub seed: u64 }
pub fn propose(b: &Bounds, obs: &[(Vec<f64>, Observation)], o: &BayesOptions) -> Result<Proposal, String>;
pub fn minimise(b: &Bounds, o: &BayesOptions, eval: &mut Eval<'_>) -> Result<OptResult, Stop>;   // beside nelder_mead, cmaes::run
pub fn run(spec: &BayesSpec, model: &dyn Model, ledger: Option<&Ledger>) -> Result<BayesResult, String>;
```

`minimise` has the batch signature of the other optimisers and is a deterministic function of (bounds, options, seed, results),
so it can be compared with them on the same closure and, if question 1 says so, replayed by `Study::propose`.

## Registration: checks and limits (to be fixed before code)

Proposed file `crates/rcfd-api/tests/f698_surrogate.rs` (new; no existing test edited). "Target" on a test function means a value
within 1 percent of the global minimum (Jones et al. 1998, the criterion of their table of results). Seeds 1 to 20 unless stated.
Test functions on their usual boxes, from the literature and checked by their own closed forms (G0): Branin on
`[-5, 10] x [0, 15]` (minimum 0.397887 at three points), Hartmann 3 on `[0, 1]^3` (-3.86278), Hartmann 6 on `[0, 1]^6`
(-3.32237) (Dixon and Szego 1978).

| Id | What | Proposed limit | Estimate behind the limit |
|---|---|---|---|
| G0 | test functions at their published minimisers | Branin within `1e-6` at all three; Hartmann 3 and 6 within `1e-5` | closed forms |
| G1 | `cholesky`: `L L^T` against `K` for a random SPD matrix, n = 50; a matrix with a negative eigenvalue | `<= 1e-12` relative, max norm; refused with the row | first principles |
| G2 | posterior with given hyperparameters against a hand-derived closed form for n = 1 and n = 2 (mean, variance, the mean-uncertainty term) | `<= 1e-12` | first principles |
| G3 | interpolation, noise off: Branin, 40 Latin-hypercube points, 10 seeds: largest `abs(m(u_i) - y_i)` over the standard deviation of `y`; largest `s(u_i)` over `sqrt(s2)` | `<= 1e-2`; `<= 1e-2` | prototype: worst 1.0e-3 (jitter 1e-8; long length scales make `K` ill-conditioned) |
| G4 | gradient of the log likelihood (if question 3 takes (b)) against central differences, 5 random hyperparameter points, d = 3, n = 30 | `<= 1e-5` relative | first principles |
| G5 | hyperparameter recovery: 20 draws of a d = 1 process with `l = 0.2`, n = 100, noise ratio 0.01, `Noise::Fitted` | median fitted `l` within a factor 1.5; median fitted `g` within a factor 3 | not prototyped: smoke first |
| G6a | coverage with the true hyperparameters: 50 draws from the prior (d = 2, `l = (0.3, 0.3)`), 40 training and 200 test points each: share of test values inside `m +- 1.96 s` | pooled share in `[0.92, 0.98]` | exact in expectation (0.95); test points of one draw are correlated, so the band is wide |
| G6b | coverage with fitted hyperparameters: Branin, 40 Latin-hypercube points, 2000 test points, 10 seeds | each seed `>= 0.90`; error `rms(y - m)/sd(y) <= 0.10` | prototype: 0.997 to 1.000; 0.010 to 0.038 |
| G6c | the same on Hartmann 6 with 120 points | recorded, no limit | prototype (crude fit): coverage 0.865 to 0.959, error 0.52 to 0.88: the surface is hard to predict globally at this n |
| G7 | leave-one-out from the one factorisation against n explicit refits with the hyperparameters held, n = 25 | `<= 1e-9` | first principles |
| E1 | `expected_improvement` against numerical quadrature of `max(f_min - y, 0)` under `N(m, s^2)` at 20 (m, s) pairs; at `s = 0` | `<= 1e-10`; equals `max(f_min - m, 0)` | closed form |
| E2 | no proposal within `1e-6` of an evaluated or pending point; every proposal inside the box | 0 violations in every run of this table | D6 |
| O1 | Branin, `n0 = 10`, budget 60: seeds that reach the target; median and largest number of evaluations at the target | `>= 18 of 20`; median `<= 35`; recorded | prototype: 20 of 20, median 29, range 20 to 31 |
| O2 | Hartmann 6, `n0 = 30`, budget 200, seeds 1 to 10: seeds that reach the target; the largest number of evaluations among those that do; the best value of every seed | `>= 5 of 10`; `<= 100`; every seed `<= -3.15` (at least the second minimum near -3.20) | prototype: 6 of 10, at 46 to 61 evaluations; the other 4 do not reach it within 200 (see risk 10) |
| O3 | Hartmann 6, same closure and seeds, budget 200: `cmaes::run` (default options) and `differential_evolution` (default population): seeds that reach the target | the count of `bayes::minimise` (O2) is at least the larger of the two counts plus 2 | from memory: both need several hundred evaluations or stop in the local minimum near -3.20, so 0 to 2 of 10 is expected; smoke first |
| O4 | Branin, Nelder-Mead (default, and ADR 0209 options) from the same 20 seeds, budget 60 | recorded, no limit | Nelder-Mead is a good local method on this smooth function; no claim is made against it |
| N1 | noise: Branin plus seeded normal noise of standard deviation 1.0 (range of Branin on the box about 300), `Noise::Fitted`, budget 80: the true value at the point of lowest posterior mean | `<= 0.397887 + 1.0` on `>= 16 of 20` | not prototyped: smoke first |
| F1 | failures: Branin with `diverged` returned inside the disc of radius 0.15 (unit cube) around the minimiser `(pi, 2.275)`, budget 60: seeds that reach the target (another minimiser); share of evaluations that fail after the initial design | `>= 18 of 20`; `<= 0.25` | not prototyped: smoke first |
| F2 | negative control of F1: the same with imputation switched off (failed points dropped) | the failed share is larger than in F1 (recorded numbers) | D8: EI keeps returning to the hole |
| R1 | reduced-order fan (`FanRomEvaluator`, set-up C of ADR 0119): parameters stagger offset in [-8, 8] degrees, camber in [0.02, 0.08], chord scale in [0.7, 1.3]; objective from F-697 (fallback: maximise `eta_s` with `q >= Q_D` as the constraint); reference best from a 21^3 factorial (9261 evaluations): budget 50, seeds 1 to 10 | best feasible value within 1 percent of the reference range of the reference best on `>= 9 of 10`; every stalled point logged with its reason | not prototyped: smoke first; the parameter boxes may need to change so that part of the box stalls |
| R2 | the same budget with `cmaes::run` and Nelder-Mead | recorded | comparison only |
| L1 | `run` with a ledger file: every evaluation is an entry with the provenance of D10; reading the file back and calling `propose` gives the next point of the uninterrupted run | bit-equal | D9, D10 |
| L2 | the ledger lines shuffled (seeded): `propose` | bit-equal to L1 | D9 canonical order |
| L3 | entries of two fidelity tiers, or two evaluator ids, without a filter; an entry without a declared parameter | refused with a located message; skipped and counted | D9 |
| D1 | determinism: O1 seed 1 run twice, and with 1 and 8 workers for the candidate scoring | every proposal bit-equal | D4, D6 draw order |
| D2 | `minimise` replayed through an evaluator that answers from a store (propose mode) | the same points as the direct run | ADR 0067 |
| B1 | defaults: `baseline_hashes.rs`, `case_hashes_w48u.rs`, `f249_default_bits.rs`, `f260_default_bits.rs`, `f297_default_bits.rs`, the tools-list goldens (`default_f1.rs`, `project_list_f270.rs`, `tool_list_f275.rs`) | pass, unedited | nothing existing calls the new files |
| NC1 | uniform random search on Branin, budget 60, same seeds | `<= 4 of 20` reach the target | prototype: 0 of 20 within 60 evaluations, 6 of 20 within 5000 |
| NC2 | G6a with the length scales fixed at a tenth of the truth | pooled coverage outside `[0.92, 0.98]` or error ratio above 3 times G6a's | the check can fail |

Mutations (scratch copy, not committed), each must fail a named check: the sign of `f_min - m` in EI (E1, O1); the
mean-uncertainty term of the variance dropped (G2); the jitter dropped (G3 or G1 refusal on a duplicate point); `s2` divided by
`n - 1` (G2); the factor `sqrt 5` of the kernel dropped (G2); imputation with `m - s` instead of `m + s` (F1); the canonical sort
removed (L2); the exclusion radius removed (E2).

### Estimates behind the limits (numpy prototype, disclosed)

Matern 5/2, constant mean, `s2` profiled, jitter `1e-8`, length scales in `[0.05, 10]` fitted by a crude random search (not the
D4 method), EI maximised over 2000 uniform candidates plus local perturbations, refit at every step.

* Branin, `n0 = 10`, target 1 percent, seeds 1 to 20: evaluations at the target 29, 26, 29, 29, 29, 27, 20, 29, 28, 29, 29, 28,
  30, 26, 29, 31, 29, 27, 30, 24 (median 29). Jones et al. (1998) report a number of this size for Branin (from memory, 28;
  UNVERIFIED).
* Uniform random search on Branin, budget 5000: reaches the target on 6 of 20 seeds (at 85, 809, 897, 1374, 1661 and 3142
  evaluations; the other 14 never), and on none within 60.
* Coverage and error: the rows G3, G6b, G6c.
* Hartmann 6, `n0 = 30`, budget 200, target 1 percent, seeds 1 to 10: evaluations at the target 58, 61, none, 53, 47, 46, none,
  48, none, none: 6 of 10 reach it, each within 61 evaluations, and 4 do not within 200. The best value of the 4 was not printed;
  the likely cause is the second minimum near -3.20, which is 3.6 percent above the global one (to be confirmed in the smoke
  run). Jones et al. (1998) report 121 evaluations on this function (from memory, UNVERIFIED), with a different stopping rule.
* Uniform random search on Hartmann 6, budget 20 000, 5 seeds: none reaches the target.

The prototype is not the registered method. Its numbers size the limits; they are not results of F-698.

### Before registration

The rows marked "smoke first" (G5, N1, F1, O3, R1) have no estimate, and O2 has a weak one. Following ADR 0121 ("Smoke results before registration",
other seeds than the registered ones), the implementing agent runs them once on seeds 101 to 105 with a scratch build, discloses
the numbers, fixes the limits, and only then registers. Limits with a prototype estimate are registered as proposed unless the
smoke run shows the estimate was wrong, in which case the change and the reason are written into the registered text.

### Decision rule (proposed)

F-698 is **done** when G0 to G7, E1, E2, O1, O2, O3, F1, R1, L1 to L3, D1, D2 and B1 pass and every mutation fails its check.
It is **partial** when the model and proposal checks pass on the analytic functions but R1 (the reduced-order fan) or O3 (the
comparison) misses; the miss is an ignored `*_known_miss` test with the measured number.

## Limits of this item

* One fidelity tier per fit. Joining the reduced-order tier and CFD (co-kriging, a correction model) is H7.
* No gradient observations (the adjoint of ADR 0091 could supply them later).
* `n` up to `n_max`; no sparse or inducing-point approximation.
* No hyperparameter averaging; the uncertainty of the length scales is ignored, so the predicted standard deviation is too small
  when `n` is small (G6c records it).
* Good in low dimension. Above about 20 parameters a stationary Gaussian process with one length scale per parameter needs more
  points than the method saves; CMA-ES stays the tool there.
* Every fan result of this item is on the reduced-order model, a stated proxy until it is checked against measured fan data.

## Known risks

1. **Ill-conditioning.** Long fitted length scales with noise off make `K` close to singular (prototype G3: interpolation error
   `1e-3` of the output spread, not `1e-8`). The jitter ladder of D3 bounds it; the lower length-scale bound and the upper bound
   `10` matter. A fit that needs jitter `1e-2` is no longer an interpolator and says so.
2. **Overconfidence.** Maximum-likelihood length scales from few points give error bars that are too narrow (prototype: Hartmann 6
   at n = 60, coverage 0.75 to 0.93). EI then exploits too early. The leave-one-out report is the user's check.
3. **Likelihood with several maxima.** The multi-start of D4 can pick a different maximum after one more point, so the proposals
   are not smooth in the data. Determinism holds, robustness to the seed is only measured (O1 over 20 seeds).
4. **Failure regions.** Imputation is a heuristic. If most of the box fails (a badly chosen box for the fan), the model learns
   little and the run degenerates to a space-filling search. F1 and R1 measure it on one case each.
5. **Cost.** A fit is `S` local searches of some hundreds of likelihood values at `n^3/3` each: at n = 1000 about a minute on one
   core, at n = 2000 about ten. Cheap against CFD, not against the reduced-order model, where CMA-ES is the better tool when the
   budget is thousands of evaluations. Refit every `k` evaluations is a possible option, not proposed.
6. **Stale or mixed data.** The ledger keeps entries of older evaluator versions and other tiers. Fitting across a version change
   silently fits two different functions (question 8).
7. **Dependence on F-697.** The objective interface is assumed. If F-697 returns a vector with weights that change between
   sessions, the fit must be on the metrics and the scalarisation applied afterwards, which changes D9 and the acquisition
   (question 11).
8. **Comparison fairness.** O3 compares on a function where a global model is known to do well. The claim is evaluations to
   target at budgets of tens to hundreds, nothing more; O4 and R2 record the cases where the local methods are as good.
9. **A fifth private dense Cholesky** (question 9), and cubic memory traffic with the row-major `Vec<f64>`; acceptable at
   `n_max`.
10. **EI stops exploring.** In the prototype 4 of 10 Hartmann 6 runs stay away from the global minimum for 140 evaluations after
    the 6 others have found it within 61. Expected improvement with maximum-likelihood hyperparameters is known to do this when
    the fitted error bars are too narrow (risk 2). The limit O2 is set at 5 of 10 for that reason and is not a strong claim.
    Possible remedies, none proposed here: hyperparameter averaging (question 3 (c)), a restart from a new space-filling batch
    when the stop rule of D6 fires, or a lower-confidence-bound acquisition.

## Maintainer decisions (2026-11-15)

The maintainer answered the open questions of the design-loop drafts (ADRs 0620 to 0624) on 2026-11-15, after the stub was
written; the questions below are kept as asked. Shared decisions:

- D1, exposure: library only in F-268, F-697, F-698, F-699 and F-701. Their MCP tools and service methods land in one item,
  F-702, together with F-339, so the tools-list goldens change once.
- D2, measured tier: `Fidelity::Measured` is added in the library only (F-700), written only to private ledgers; the MCP
  fidelity enums stay unchanged until F-702.
- D3, privacy: strict split. Measured values, fidelity pairs with a measured source, the fitted correction and corrected
  predictions go to a caller-given private ledger, refused with a located error when it lies inside the repository tree. A
  public ledger gets rounded aggregates only (3 significant digits, at least 5 pairs).
- D4, record: the fidelity pair and discrepancy record of ADR 0622 is the only one; ADR 0136 is amended to point to it.
- D5, ledger fields: new fields are optional top-level keys of `rcfd-ledger-1`, omitted when unset; `provenance` conventions
  until they land (F-700).
- D6, split and order: F-268 keeps the ledger sink, the `FanRomModel` adapter and the bands; the shared ledger fields and
  queries are F-700. Order: F-268 and F-700, then F-697, then F-698 and F-699 in parallel, then F-701.
- B1, measured data format: the CSV of `docs/fan-measurement-protocol.md` with a variant-id column added.
- B2, hypothesis family level: 0.05 with the `6 / (pi^2 i^2)` series, Holm as a batch option.
- B3, estimated limits: a limit this text marks as an estimate or as "to be fixed after a smoke run" is fixed by the
  implementing agent from a disclosed smoke run on separate seeds, or a written calculation, and registered before any
  acceptance run; it is not moved after a run. A negative control that passes by a thin margin is fixed by strengthening the
  test, never by moving the limit.
- B4, ADR 0240 Q4: the NC1 miss on ST1 (|z| 3.87 against 4) is accepted as a known miss; ADR 0240 Q5: fixed N.

Every other open question of this ADR takes the recommendation written with it. Here: Q1 is D1 (the `gp_ei` study method is a follow-up); Q12 takes F-697's objective, which is built first (D6); deps gain F-700.

## OPEN QUESTIONS for the maintainer

1. **Exposure beside the optimisers.** (a) Library only: `design::surrogate`, `design::bayes` (`propose`, `minimise`, `run` over
   a `Model` and the ledger), as the Sobol indices of ADR 0240. No tool, no key. (b) Also a `Method::GpEi { budget, n0, seed, .. }`
   in `study.rs`, so that `design_create_study`, `design_propose` and `design_run_study` accept `{"type": "gp_ei"}`. By ADR 0222
   point 5 this needs no descriptor change and no golden change, but it is a new value the study tools accept, it adds keys to
   `study.json` (`n0`, `kernel`, `noise`), and each `design_propose` replay refits the model from scratch (seconds at n in the
   hundreds). (c) (b) plus a new tool `design.ledger_propose` that reads a ledger file: changes the tools-list goldens.
   **Recommendation: (a) in this item, (b) as a follow-up item once R1 passes; (c) not now** (it joins F-339, the open tool
   exposure of ADR 0240 Q2).
2. **Default kernel.** (a) Matern 5/2 with a length scale per parameter. (b) Squared exponential (the kernel of Jones et al. with
   exponent 2). (c) Jones et al.'s power-exponential kernel with a fitted exponent per parameter (doubles the hyperparameters).
   **Recommendation: (a)**, with (b) opt-in: the squared exponential assumes an infinitely smooth response, which fan curves with
   a stall kink are not, and it is the worse-conditioned of the two (Stein 1999; Snoek, Larochelle and Adams 2012).
3. **Hyperparameter search.** (a) The existing `optim::nelder_mead` from the best starts: no new code, no gradient, poor beyond
   about 10 parameters. (b) The analytic gradient of the log likelihood (Rasmussen and Williams eq. 5.9) with the existing
   `optim::projected_gradient` in the log box: scales to more parameters, one more formula to verify (check G4). (c) Averaging
   over hyperparameters by sampling (Snoek et al. 2012): better calibrated, a sampler to write and verify, much more cost.
   Number of starts `S`: 10 proposed. **Recommendation: (b), `S = 10`**; (c) not in this item.
4. **Noise default.** (a) Noise off (jitter only); `Fitted` and `Fixed(sd)` opt-in. (b) `Fitted` always. **Recommendation: (a)**:
   the first targets are deterministic, and a fitted nugget on few points tends to explain real structure as noise. For CFD
   objectives the user should set `Fixed(sd)` from a measured iteration scatter.
5. **Initial design size.** (a) `n0 = max(5 d, 10)`. (b) `n0 = 10 d` (the rule of thumb of Jones et al.). (c) `n0 = d + 2`, the
   least that can be fitted. **Recommendation: (a)**: the prototype reaches the Branin target at 29 evaluations with 10 initial
   points; `10 d` spends a larger share of a CFD budget before the model is used. Existing ledger entries count toward `n0`.
6. **Failed evaluations and constraints.** (a) Imputation of `diverged` points at `m + s` (Forrester, Sobester and Keane 2006),
   and probability of feasibility for the declared constraints (Schonlau, Welch and Jones 1998). (b) Drop failed points and add
   only the exclusion radius. (c) A separate classifier (a Gaussian process on the 0/1 failure label) multiplying EI.
   **Recommendation: (a)**; (b) is the negative control F2; (c) is cleaner in theory and is a follow-up if F1 or R1 show that
   imputation distorts the model near the boundary of the failure region.
7. **Batch proposals.** (a) `q = 1` default, kriging believer for `q > 1` and for pending entries (Ginsbourger et al. 2010).
   (b) `q = 1` only. (c) The exact multi-point EI (no closed form beyond q = 2; Monte Carlo). **Recommendation: (a)**: the runner
   can evaluate several CFD variants at once, and the believer rule costs nothing extra.
8. **Entries of different evaluator versions.** (a) Refuse a fit across versions unless the caller passes
   `allow_mixed_versions`; report the versions. (b) Use all and report. (c) Use only the newest version. **Recommendation: (a)**:
   ADR 0121 says to change the version "whenever the evaluator's results may change", so mixing is a decision, not a default.
9. **Dense Cholesky.** (a) A private `cholesky` and two triangular solves in `surrogate.rs` (public within the module for check
   G1), as `cmaes.rs` carries its own `sym_eigen`. (b) A new public `rcfd_linalg::dense` module (Cholesky, solves) that the four
   existing private copies could later use; touches a shared crate. No external crate in either case. **Recommendation: (a) in
   this item**, and a separate clean-up item for (b) if the maintainer wants the copies merged (they differ in pivot tests and
   error types, so merging is not free).
10. **Size cap.** `n_max` for one fit: (a) 2000 (about 32 MB for `K` and `L`, minutes per fit); (b) 500; (c) no cap. Above the
    cap: refuse, or fit on the best `n_max` points plus a space-filling remainder? **Recommendation: (a) and refuse**; subset
    selection changes the model silently.
11. **Multi-objective.** F-697 may give a vector (flow, noise, power). (a) Out of scope here; scalar objective only, with
    F-697's scalarisation. (b) ParEGO (Knowles 2006): one Gaussian process on a randomly weighted augmented Tchebycheff
    scalarisation, a new weight per proposal; small addition, checked on ZDT1 against the NSGA-II numbers of ADR 0220. (c) One
    process per objective and expected hypervolume improvement (Emmerich, Giannakoglou and Naujoks 2006): the better method,
    much more code. **Recommendation: (a) for F-698, (b) as a follow-up item** once F-697 is merged and its vector form is known.
12. **The fan check R1.** The parameters (stagger offset, camber, chord scale), their boxes and the fallback objective
    (maximise `eta_s` subject to `q >= Q_D`) are this draft's choice and are not derived from F-697. (a) As drafted. (b) Wait for
    F-697 and take its objective and parameter set. **Recommendation: (b) if F-697 is merged before F-698 is registered, else
    (a)**, with the objective named in the registered text.
13. **Acceptance of the comparison.** O3 asks that, at 200 evaluations on Hartmann 6, the surrogate reaches the target on at
    least 2 more seeds of 10 than CMA-ES and differential evolution. (a) Keep a comparative limit (the roadmap item claims
    "cheaply"). (b) Record the comparison without a limit and keep only the absolute limits O1, O2. **Recommendation: (a)**,
    fixed after the smoke run, because an absolute count alone does not show the surrogate is worth its code.
14. **Reliability on multimodal functions (risk 10).** The prototype finds the Hartmann 6 minimum on 6 of 10 seeds. (a) Accept
    plain EI with the limit `>= 5 of 10` and record the misses. (b) Add an exploration restart (a new Latin-hypercube batch of
    `d` points when the stop rule fires before the budget) and raise the limit after a smoke run. (c) Make the acquisition a
    choice (EI, lower confidence bound) and register both. **Recommendation: (a) for the registration, with (b) tried in the
    smoke run and taken only if it raises the count without hurting O1**; the first real use (a fan objective over 3 to 6
    parameters) is closer to Branin than to Hartmann 6.

## References (cited from memory by this draft; to be read and recorded by the implementing agent)

- C. E. Rasmussen, C. K. I. Williams (2006), Gaussian Processes for Machine Learning, MIT Press (chapter 2: regression,
  eq. 2.25, 2.26, 2.42; chapter 4: Matern kernel, eq. 4.17; chapter 5: marginal likelihood and its gradient, eq. 5.8, 5.9,
  leave-one-out, eq. 5.10 to 5.12). Equation numbers UNVERIFIED.
- D. R. Jones, M. Schonlau, W. J. Welch (1998), Efficient global optimization of expensive black-box functions, Journal of
  Global Optimization 13, 455-492 (the kriging predictor and its error, expected improvement, the diagnostics, the test results
  on Branin, Goldstein-Price, Hartmann 3 and 6; the 1 percent criterion and the `10 d` rule; numbers UNVERIFIED).
- J. Mockus, V. Tiesis, A. Zilinskas (1978), The application of Bayesian methods for seeking the extremum, in Towards Global
  Optimization 2, North-Holland, 117-129 (expected improvement).
- M. Schonlau, W. J. Welch, D. R. Jones (1998), Global versus local search in constrained optimization of computer models, IMS
  Lecture Notes 34, 11-25 (probability of feasibility).
- J. R. Gardner, M. J. Kusner, Z. Xu, K. Q. Weinberger, J. P. Cunningham (2014), Bayesian optimization with inequality
  constraints, ICML.
- A. I. J. Forrester, A. Sobester, A. J. Keane (2006), Optimization with missing data, Proceedings of the Royal Society A 462,
  935-945 (imputation of failed evaluations).
- D. Ginsbourger, R. Le Riche, L. Carraro (2010), Kriging is well-suited to parallelize optimization, in Computational
  Intelligence in Expensive Optimization Problems, Springer, 131-162 (kriging believer, constant liar).
- V. Picheny, T. Wagner, D. Ginsbourger (2013), A benchmark of kriging-based infill criteria for noisy optimization, Structural
  and Multidisciplinary Optimization 48, 607-626 (the incumbent under noise).
- M. L. Stein (1999), Interpolation of Spatial Data: Some Theory for Kriging, Springer (the Matern family).
- J. Snoek, H. Larochelle, R. P. Adams (2012), Practical Bayesian optimization of machine learning algorithms, NIPS (Matern 5/2
  as the default, hyperparameter averaging).
- O. Dubrule (1983), Cross validation of kriging in a unique neighborhood, Mathematical Geology 15, 687-699 (leave-one-out
  without refitting).
- J. Knowles (2006), ParEGO: a hybrid algorithm with on-line landscape approximation for expensive multiobjective optimization
  problems, IEEE Transactions on Evolutionary Computation 10, 50-66.
- M. T. M. Emmerich, K. C. Giannakoglou, B. Naujoks (2006), Single- and multiobjective evolutionary optimization assisted by
  Gaussian random field metamodels, IEEE Transactions on Evolutionary Computation 10, 421-439.
- L. C. W. Dixon, G. P. Szego (1978), The global optimization problem: an introduction, in Towards Global Optimization 2 (the
  Branin and Hartmann test functions).
- Cholesky factorisation, triangular solves, the normal density and distribution: first principles.

## For the lead: roadmap entry

See `roadmap-entry.json` beside this file (F-698, phase 9, deps F-268, F-249, F-697, status todo). Possible follow-ups, not
created: the study method and tool exposure (question 1 (b)), ParEGO (question 11 (b)), a shared dense module in `rcfd-linalg`
(question 9 (b)).
