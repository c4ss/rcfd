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

## Addenda

## Amendment 1 (2026-11-16): registration, before any committed code of F-698 and before any run of its checks

Status: **REGISTERED 2026-11-16**, written before any code of F-698 was committed to the branch and before any run of the checks
below. Written by the implementing agent on branch `f698-work` (from 6400c929, the F-697 tip on F-700 and F-268), worktree
`w49i-work/f698`, scratch `w49i-work/f698-scratch`. Every choice the maintainer would normally be asked about is taken on the
maintainer's standing instruction of 2026-11-11 (take the recommendation), on top of the maintainer decisions of 2026-11-15
above.

### What was read

`AGENT_RULES.md`; the brief; F-698's roadmap acceptance and note (binding, quoted below); this ADR (stub, maintainer decisions,
questions); ADR 0620 (F-697: `FanObjective`, `ObjectiveModel`, `FanRomTarget`, set-up F) and its addendum; ADR 0623 (F-268) and
ADR 0632 (F-700: optional ledger fields `parents`, `fitted`, the `training_set` query of its D4, which refuses mixed evaluators,
versions, tiers and levels); `docs/fan-project-breakdown.md` part H3; `loop-drafts/decisions.md`. The stub's prototype
`loop-drafts/h3-surrogate/gp_proto.py` was not run. Source: `design/ledger.rs` (`Entry`, `Model`, `entry_of`, `training_set`,
`entry_ref`), `design/optim.rs` (`Fitness`, `Eval`, `nelder_mead`, `differential_evolution`, `projected_gradient`),
`design/cmaes.rs` (`run`, `Options`, `normal`), `design/sampling.rs` (`latin_hypercube`, `full_factorial`), `design/study.rs`
(`parallel_map`: results in index order for any worker count; `Sense`, `Cmp`, `ParamDef`), `design/uq.rs` (`norm_cdf` through
`rcfd_reactor::mixfrac::erfc`, a series below 2 and a continued fraction above; `evaluate_guarded`), `design/sensitivity.rs`
(the driver pattern), `design/fan_objective.rs` and `tests/f697_fan_objective.rs` (set-up F).

No paper was opened for this registration (none is in the repository). Every citation below is from memory, as in the stub;
every formula used is derived here or guarded by a closed form or a numerical check of its own (recorded in
`docs/reference-data.md` with the code).

The binding acceptance:

> Every limit not stated here is registered in ADR 0621 before the code. An opt-in library Gaussian process (constant mean, one length scale per parameter, hyperparameters by maximum likelihood, dense Cholesky, no new crate) fitted to the evaluated variants of a design ledger: reproduces its training values within 1e-2 of their standard deviation with noise off (Branin, 40 points, 10 seeds); its 95 percent interval covers between 0.92 and 0.98 of held-out values of functions drawn from its own prior and at least 0.90 on Branin with fitted hyperparameters, with an rms prediction error of at most 0.10 of the output standard deviation; expected improvement matches numerical quadrature within 1e-10. The proposal loop from 10 initial points reaches within 1 percent of the Branin minimum 0.397887 on at least 18 of 20 seeds within 60 evaluations with a median of at most 35, where uniform random search reaches it on at most 4 of 20; on Hartmann 6 it reaches within 1 percent of -3.32237 on at least 5 of 10 seeds within 200 evaluations and on at least 2 more seeds than CMA-ES (ADR 0222) and differential evolution at the same budget; with a region of failed evaluations it still reaches the Branin target on at least 18 of 20 seeds with at most 25 percent failed evaluations after the initial design; on the reduced-order fan model (ADR 0119) it comes within 1 percent of the best of a 21^3 factorial reference within 50 evaluations on at least 9 of 10 seeds. Every proposal is a ledger entry with its predicted mean and standard deviation; the proposal is bit-identical for a repeated run, for any worker count and for any order of the ledger lines; defaults, baselines and goldens are unchanged.

### Disclosed: a scratch prototype and its smoke runs (seeds 101 to 110 only), before this registration

To fix the limits the stub marks "smoke first", the design below was written as a prototype in a scratch copy
(`f698-scratch/proto`, a `git archive` of 6400c929, outside the worktree) with two smoke test files there
(`zz_f698_smoke.rs`, `zz_f698_fan_smoke.rs`; logs `smoke_s2.log`, `smoke_s2b.log`). Nothing of it is in the branch before this
commit. The code committed after this registration is that prototype (formatted and documented); the test file of the checks is
written after this registration. No seed of a registered check was used (smoke seeds 101 to 110).

| Smoke | Set-up | Result |
|---|---|---|
| S1 | Branin, `n0 = 10`, budget 60, seeds 101 to 105 | target reached at 23, 28, 23, 28, 22 evaluations; uniform random search 0 of 5 within 60 |
| S2 | Hartmann 6, `n0 = 30`, budget 200, seeds 101 to 105 | GP: target at 47, 55, none (best -3.20316, the second minimum), 58, 52 (about 100 s per seed); CMA-ES defaults, budget 200: 3 of 5 (at 135, 186, 159); DE defaults (population 60): 0 of 5 (best -2.76) |
| S3 | Branin, failures inside the unit-cube disc of radius 0.15 around (pi, 2.275), budget 60 | target at 26, 32, 24, 21, 30; failed share after the initial design 0.02, 0.02, 0.02, 0, 0.04. Imputation and failure model off: 0 of 5, failed share 0.74 to 0.88 |
| S4 | Branin plus N(0, 1) noise, `Noise::Fitted`, budget 80 | true value at the point of lowest posterior mean 0.399, 0.463, 0.642, 0.487, 0.459 (limit 1.398) |
| S5 | G5 set-up, 5 draws | fitted l 0.244, 0.184, 0.168, 0.364, 0.243 (median 0.243); g 4.7e-3, 9.5e-3, 2.6e-2, 5.2e-4, 7.2e-3 (median 7.2e-3) |
| S5 | G6a set-up, 10 draws | pooled coverage 0.947; with the length scales 10 times too long (l = 3) 0.840 |
| S6 | G3 and G6b set-ups, LHS seeds 101 to 103 | interpolation error 6.5e-4 to 7.6e-4 of sd(y); sd at the data 1.0e-4 of sqrt(s2); coverage 0.993 to 1.000; rms 0.017 to 0.027 of sd(y); one fit 0.03 s |
| S7 | reduced-order fan, set-up F3 below, a 6^3 grid and GP runs of 50 (seeds 101 to 103) | 97 of 216 grid points fail (stall at one of the points); 216 evaluations in 23 s on 8 threads. Mean `eta_s`: grid range 0.312 to 0.344, within 1 percent at evaluation 2 to 4 (a trivial check), 40 to 42 of 50 failed. Weighted flow: best at a box corner, reached at 16. Mean power: grid best 1.4719 W; with imputation alone the GP ended at 1.68, 2.01, 1.81 W (41 of 50 failed); with the failure model (below) at 1.4454, 1.4461, 1.4462 W, within 1 percent of the grid best at 16 to 19, 31 to 33 failed |
| S8 | constraint set-up K below, seeds 101 to 105 | feasible within 1 percent of 0.18 at evaluation 11 or 12 each; 20 to 21 infeasible after the initial design |
| S9 | likelihood gradient against central differences (both kernels, nugget on, LHS seed 101) | 7.7e-11 and 1.5e-10 relative |
| S9 | leave-one-out against refits (LHS seed 102) | means 5e-15 of sd(y); sd 3.4e-8 against the refit's latent sd, 5.3e-15 against the refit's observation sd (latent variance plus `s2 (g + jitter)`): the leave-one-out variance is that of the observation, and G7 is registered so |

Design changes made from the smoke, before registration: (1) a failure model, question 6 (c), whose recommendation made it a
follow-up "if F1 or R1 show that imputation distorts the model near the boundary of the failure region": S7 shows it, so it is
taken now; (2) an exploration rule, question 14 (b), in a stateless variant (take the candidate farthest from the data when the
best acquisition is below `1e-6` of the spread of the values), was tried on S2 and changed no result, so by the recommendation
("taken only if it raises the count") it is not taken and is not in the code; (3) the fan objective, question 12 (b), F-697's
layer: mean power, because mean efficiency makes the 1 percent check trivial (S7).

What the smoke says about limits the acceptance fixes: S2 makes **O3 likely to miss** (GP 4 of 5, CMA-ES 3 of 5 on the smoke
seeds; the acceptance needs at least 2 more of 10). The limit is kept as the acceptance gives it.

### Design (fixed now)

Two new files, `crates/rcfd-api/src/design/surrogate.rs` and `crates/rcfd-api/src/design/bayes.rs`, and two module lines (and
the doc line) in `design/mod.rs`. Nothing existing calls them; no other existing source file is edited (`ledger.rs` is not).

**surrogate.rs** (the stub's D2 to D4, with the following fixed):

* `Kernel::{Matern52 (default), SquaredExponential}`; `correlation(kernel, a, b, l)`; Matern `(1 + r + r^2/3) e^-r`,
  `r = sqrt(5 sum (d_i/l_i)^2)`; SE `exp(-sum (d_i/l_i)^2 / 2)`.
* `Noise::{None (default), Fitted, Nugget(g)}`: the stub's `Fixed(sd)` becomes a fixed nugget **ratio** `g` (noise variance over
  process variance), because with `s2` profiled a fixed absolute variance has no closed-form profile (deviation).
* Outputs standardised (mean, population sd; sd 0 replaced by 1); `K = R + (g + j) I`, jitter `j = 1e-8`, multiplied by 100 at
  most 3 times on a failed factorisation, then a located refusal. `mu = 1^T K^-1 y / 1^T K^-1 1`, `s2 = (y - mu 1)^T K^-1
  (y - mu 1) / n`, concentrated log likelihood `L = -(n/2) ln s2 - sum ln L_ii`.
* `cholesky(a, n) -> Result<L, (row, pivot)>` row-major lower, refusing `pivot <= (row + 1) eps a_ii`; `solve_lower`,
  `solve_upper_t` (question 9 (a)).
* `predict(u) -> (mean, sd)` of the latent function, variance `s2 [1 - k^T K^-1 k + (1 - 1^T K^-1 k)^2 / 1^T K^-1 1]` floored at
  0, in output units.
* Gradient (question 3 (b)), derived: with `a = K^-1 (y - mu 1)` and mu, s2 at their profile maximum, the envelope theorem gives
  `dL/dt = (a^T (dK/dt) a / s2 - tr(K^-1 dK/dt)) / 2`; `dk/d ln l_i = (5/3)(1 + r) e^-r (d_i/l_i)^2` (Matern: `dk/dr = -(r/3)(1
  + r) e^-r`, `dr/d ln l_i = -5 (d_i/l_i)^2 / r`), `k (d_i/l_i)^2` (SE); `dK/d ln g = g I`.
* `fit`: search in `ln l_i` in `[ln 0.05, ln 10]` and, for `Fitted`, `ln g` in `[ln 1e-8, ln 1]`; starts: the box centre and
  the 9 points of `latin_hypercube(9, dim, seed)` scaled to the box, in that order; the best 3 by likelihood (ties by start
  order) refined by `optim::projected_gradient` on `-L` (40 iterations, step 0.1, Armijo 1e-4, 30 halvings); the best final
  value wins (first on ties). A start whose factorisation fails has value `-inf`. Refusals: fewer than `d + 2` or more than
  2000 observations (question 10 (a)), a non-finite input or output, mismatched sizes.
* `loo()`: leave-one-out mean and sd from one factorisation (Dubrule 1983, from memory; derived: with `Q = K^-1 - K^-1 1 1^T
  K^-1 / 1^T K^-1 1`, `Q y = a`, residual `a_i / Q_ii`, variance `s2 / Q_ii`), hyperparameters and `s2` held, mean
  re-estimated: the variance of the observation `y_i` (latent plus `s2 (g + j)`).
* `log_likelihood_and_gradient(with_g)`, `length_scales`, `nugget`, `jitter`, `process_variance`, `mean`, `n`, `kernel`.
* Workers: start values and refinements through `study::parallel_map` (index order kept): the same bits for any count.

**bayes.rs** (the stub's D6 to D10):

* `expected_improvement(m, s, f_min) = (f_min - m) Phi(z) + s phi(z)`, `z = (f_min - m)/s`, `max(f_min - m, 0)` at `s = 0`;
  `prob_feasible(m, s) = Phi(-m/s)` (1 or 0 at `s = 0`).
* `Observation { x, status: Ok { f, c } | Failed | Pending }` in parameter space, `f` minimised, `c_k <= 0` feasible; an `Ok`
  with a non-finite value is `Failed`.
* `BayesOptions { gp, n0: None -> max(5 d, 10), batch 1, seed 1, workers 1, impute true, candidates_per_dim 1000, exclusion
  1e-6, classify_failures true }`.
* `propose(bounds, obs, options)`: (1) map to the unit cube; (2) **canonical order**: sort by the unit coordinates
  (lexicographic, `total_cmp`), then status, then value bits; with `Noise::None`, bit-identical `Ok` points are merged into
  their mean (the stub's 1e-12 merge radius becomes bit identity; near-duplicates are handled by the jitter); (3) while fewer
  than `n0` observations of any status exist, the next points of `latin_hypercube(n0, d, seed)` not within 1e-12 of an
  observation (`initial`); (4) with fewer than `d + 2` `Ok` points, untaken points of further hypercubes with derived seeds
  (`filling`); (5) else fit the objective process on the `Ok` points (gp seed derived from `seed` and the number of
  observations by FNV-1a); with `impute`, every failed point gets `m + s` of that fit and the process is refitted with the
  hyperparameters held (Forrester, Sobester and Keane 2006); one fitted process per constraint on the `Ok` points (question 6
  (a)); with `classify_failures` and at least one failure, a process fitted to the label -1 (ok) / +1 (failed) of every `Ok` and
  failed point, whose `prob_feasible` multiplies the acquisition (least-squares classification, Rasmussen and Williams 2006
  section 6.5, from memory); (6) incumbent: the lowest feasible `f` (noise off) or the lowest posterior mean at feasible `Ok`
  points (noise on); none feasible: the acquisition is the feasibility product alone (`pof`); (7) pending points, and each
  earlier point of a batch, are added at their predicted mean (kriging believer, hyperparameters held); (8) candidates:
  `latin_hypercube(1000 d, d, derived seed)` plus 200 clamped normal perturbations (sd 0.02 for the first 100, 0.1 for the
  next 100) of each of the best 5 `Ok` points (feasible first, then by `f`, then canonical order); a candidate within
  `exclusion` of an observation or an earlier pick scores `-inf`; the best 5 by score (ties by index) are refined by
  `nelder_mead` in the unit box (budget `100 d`, restarts 0, initial step 0.05, x0 the candidate); the best refined point wins
  (first on ties). Scoring and refinement run through `parallel_map`. Each `Pick` records kind, acquisition, ei, mean, sd,
  feasibility, incumbent, length scales, nugget, jitter, `n_obs`, `n_imputed`, `n_pending`.
* No stop rule (deviation from D6): the loop runs to the budget; a stop before it can only lower the counts the acceptance
  measures, and the exploration variant was not taken.
* `minimise(bounds, options, budget, eval)`: the batch interface of the optimisers; a point with `f` not finite or violation
  `> 0` is a failed point (declared constraints are modelled by `run` only); the result is the best point by Deb's rules.
* `observations(entries, idx, spec)`: `evaluation` entries of `idx`; an entry without a declared parameter is skipped and
  counted; then `ledger::training_set` (F-700), which refuses a mix of evaluators, versions, tiers or levels (question 8 (a)
  without the `allow_mixed_versions` escape, deviation: no check needs it); a declared unit that differs from the ledger's is
  refused; `diverged` and `constraint_rejected` are failed points (the stub's "feasibility model only" for
  `constraint_rejected` becomes the failure model, which covers both), `not_evaluated` pending.
* `run(spec, model, ledger)`: `BayesSpec { study, params (name, unit, lo, hi), objective, sense, constraints (metric, cmp,
  limit), budget, options, clock_ms }`; reads the ledger's entries of `study` from `model.id()`, proposes, evaluates the batch
  through `parallel_map` with `uq::evaluate_guarded`, appends one `evaluation` entry per point (variant `<study>/gp_ei/<n>`,
  seed, provenance `{driver "gp_ei", iteration, batch_index, acquisition, acquisition_value, ei, predicted_mean, predicted_sd,
  feasibility, incumbent, objective, sense, length_scales, nugget, jitter, n_obs, n_imputed, n_pending}`, mean and incumbent in
  the natural sign), repeats to the budget, then appends one `surrogate_summary` entry (metrics evaluations, failed,
  observations, best, `loo_rms_z`, `loo_cover95`; F-700 `fitted` = `{rcfd.design.surrogate.gp, 1, FNV-1a of the hyperparameter
  bits}` and `parents` = references with hashes to the `Ok` rows). `propose_from_entries(entries, spec, model_id)` is the next
  proposal of `run` from a ledger read.

### Set-ups (fixed now)

* **Branin** on `[-5, 10] x [0, 15]`, `(x2 - b x1^2 + c x1 - 6)^2 + 10 (1 - t) cos x1 + 10`, `b = 5.1/(4 pi^2)`, `c = 5/pi`,
  `t = 1/(8 pi)`; minimum 0.397887 at (-pi, 12.275), (pi, 2.275), (9.42478, 2.475). Target: `<= 0.397887 (1 + 0.01)`.
* **Hartmann 3 and 6** on the unit cube (Dixon and Szego 1978; constants from memory, checked by G0): minima -3.86278 at
  (0.114614, 0.555649, 0.852547) and -3.32237 at (0.20169, 0.150011, 0.476874, 0.275332, 0.311652, 0.6573). Target: `<= f* (1 -
  0.01)`.
* **F3, the reduced-order fan**: ADR 0620's `FanRomTarget` on its set-up F (`fan_f`, 3000 rpm, rho 1.2, nu 1.5e-5, c0 343,
  observer 1 m at 90 deg, n_scan 100, `RomModel::default()`, `Tolerances::default()`, no plan), geometry `x = (stagger offset
  [deg] in [-3, 3] added to every station's stagger, chord scale [1] in [0.8, 1.2] multiplying every chord, camber scale [1] in
  [0.5, 1.5] multiplying every camber)`; points R1, R2, R3 (k 5e3, 1.5e4, 2.5e4 Pa s^2/m^6, weights 0.5, 0.3, 0.2); one term
  `power`, `Min`, `Nominal`, `Over::Mean`; no limit; objective workers 1; as a `ledger::Model` through `ObjectiveModel`, metric
  `power@nominal@mean` [W]. A stall at any point makes the candidate `diverged` with the located reason. Reference: the
  `full_factorial([21, 21, 21])` grid on the box (9261 candidates), `P_ref` its lowest feasible value. Target: `<= P_ref (1 +
  0.01)`.
* **K, a constraint**: minimise `(a - 0.8)^2 + (b - 0.8)^2` subject to `a + b <= 1` on `[0, 1]^2` (closed form (0.5, 0.5), 0.18),
  `n0 = 10`, budget 40. Target: feasible and `<= 0.18 (1 + 0.01)`.

### Checks (limits fixed now)

Test file `crates/rcfd-api/tests/f698_surrogate.rs` (new). Release profile. Default options unless stated; "seed s" is
`BayesOptions::seed` (and the uniform-search or optimiser seed). E2 applies to every proposal loop of the table. Clauses: (a)
interpolation, (b) coverage and error, (c) EI, (d) Branin and random search, (e) Hartmann 6 and the comparison, (f) failures,
(g) the fan, (h) ledger, repeat, workers, line order, (i) defaults.

| Id | Clause | What | Limit |
|---|---|---|---|
| G0 | set-ups | Branin at its three minimisers; Hartmann 3 and 6 at theirs | within 1e-6 of 0.397887; within 1e-5 of -3.86278 and -3.32237 |
| G1a | Cholesky | `A = B B^T + 50 I`, B 50 x 50 uniform(-1, 1) from Rng 698_001: max abs of `L L^T - A` over max abs of `A`; `diag(1, -1)` | `<= 1e-12`; refused at row 1 |
| G1b | jitter | `Gp::with_hyper` on 10 Branin LHS points (seed 698_002) with the first point repeated, l (0.3, 0.3), noise off | succeeds with jitter 1e-8 |
| G2 | model | d 1, u (0.2, 0.7), y (1, 3), l 0.4, g 0, both kernels: process variance against `delta^2/(a - rho)` (`delta = (y1 - y2)/2`, `a = 1 + 1e-8`, `rho = k(u1, u2)`), mean against `(y1 + y2)/2`, prediction at 0.45 and 0.9 against `m = mu + delta (k1 - k2)/(a - rho)`, `var = s2 [1 - (a (k1^2 + k2^2) - 2 rho k1 k2)/(a^2 - rho^2) + (1 - (k1 + k2)/(a + rho))^2 (a + rho)/2]`, kernels written out in the test | relative `<= 1e-12` each |
| G3 | (a) | Branin, 40 LHS points, LHS seeds 698_101 to 698_110 (gp seed the same), fitted, noise off: max `abs(m(u_i) - y_i)` / sd(y); max `s(u_i)` / sqrt(s2) | `<= 1e-2`; `<= 1e-2` |
| G4 | gradient | Hartmann 3 on 30 LHS points (seed 698_201), 5 hyperparameter points (`ln l_i` uniform in [ln 0.1, 0], `ln g` in [ln 1e-4, ln 0.1], Rng 698_202), both kernels, with the nugget component: max abs difference to central differences (h 1e-5) over the max abs of the differences | `<= 1e-5` |
| G5 | fit | 20 draws, d 1, l 0.2, g 0.01, 100 LHS points (seed 698_300 + k), draw from Rng(698_300 + k), `Noise::Fitted` (gp seed k): median fitted l; median fitted g | in [0.2/1.5, 0.3]; in [0.01/3, 0.03] |
| G6a | (b) | 50 draws, d 2, l (0.3, 0.3), 40 LHS training points (seed 698_400 + k), 200 uniform test points and then the joint draw (jitter 1e-10) from Rng(698_400 + k); `with_hyper` at the true l, g 0: pooled share of test values inside `m +- 1.96 s` | in [0.92, 0.98] |
| NC2 | control | G6a with l (3, 3) | pooled share `< 0.92` |
| G6b | (b) | Branin, 40 LHS points (seeds 698_501 to 698_510), fitted; 2000 uniform test points from Rng(seed + 1000): share inside `m +- 1.96 s`; `rms(y - m)` / sd(y) | each seed `>= 0.90`; each `<= 0.10` |
| G6c | record | Hartmann 6, 120 LHS points, seeds 698_601 to 698_605: coverage and error | recorded |
| G7 | loo | Branin, 25 LHS points (seed 698_701), l (0.3, 0.3), g 0: `loo()` against 25 refits without the point (mean; sd as `sqrt(s_refit^2 s2/s2_refit + s2 j)`) | max abs difference over sd(y) `<= 1e-9` |
| E1 | (c) | `expected_improvement(0, s, z s)` for s in {0.01, 0.1, 1, 3}, z in {-6, -2, 0, 1, 4} against Simpson's rule for `s int_{z-40}^{z} (z - t) phi(t) dt`, 400000 intervals; at s = 0: (m 2, f 1) and (m 1, f 2) | abs `<= 1e-10`; exactly 0 and 1 |
| E2 | design | every proposal of every loop of this table: inside the box; unit-cube distance to every earlier point `>= 1e-6` | 0 violations |
| P1 | batch | Branin, the 15 points of LHS seed 698_950 evaluated: `propose` with batch 4: min pairwise unit-cube distance; the batch-1 pick added as `Pending`: distance of the new batch-1 pick to it | `>= 1e-3`; `>= 1e-3` |
| O1 | (d) | Branin, `n0 = 10`, budget 60, seeds 1 to 20 (`minimise`): seeds that reach the target; median evaluations to it (a miss counts as 61) | `>= 18 of 20`; `<= 35` |
| NC1 | (d) | uniform random search, Rng(seed), 60 points, seeds 1 to 20 | `<= 4 of 20` |
| O4 | record | Nelder-Mead, default options, budget 60, seed s, on O1 | recorded |
| O2 | (e) | Hartmann 6, `n0 = 30`, budget 200, seeds 1 to 10: seeds that reach the target; largest evaluations to it; every seed's best | `>= 5 of 10`; `<= 100`; `<= -3.15` |
| O3 | (e) | Hartmann 6, budget 200, seeds 1 to 10: `cmaes::run` (defaults, max_evals 200) and `differential_evolution` (defaults, max_evals 200): seeds that reach the target | O2's count `>= max(CMA-ES, DE) + 2` |
| F1 | (f) | O1 with every point inside the unit-cube disc of radius 0.15 around (pi, 2.275) failing (`Fitness::FAILED`): seeds that reach the target; failed share among evaluations 11 to 60 | `>= 18 of 20`; every seed `<= 0.25` |
| F2 | control | F1 with `impute` and `classify_failures` off | pooled failed share larger than F1's |
| N1 | noise | Branin plus N(0, 1) (Box-Muller from Rng(698_800 + s), evaluation order), `Noise::Fitted`, `n0 = 10`, budget 80, seeds 1 to 20; after the run a `Noise::Fitted` fit (gp seed 698_899) on all points: the true Branin value at the evaluated point of lowest posterior mean | `<= 1.397887` on `>= 16 of 20` |
| K1 | constraint | set-up K through `run` (an `FnModel`), seeds 1 to 10: seeds whose best feasible value meets the target; infeasible evaluations after the initial design | `>= 9 of 10`; recorded |
| R1 | (g) | F3, `run` with budget 50 (`n0 = 15`), seeds 1 to 10, against `P_ref`: seeds with a feasible value `<= P_ref (1.01)`; every failed entry `diverged` with a reason naming its point; failed counts; seeds within 1 percent of the reference range (recorded) | `>= 9 of 10`; all |
| R2 | record | F3, budget 50: `cmaes::run` (defaults) and `nelder_mead` (defaults, seed s), seeds 1 to 10 (failure = `FAILED`) | recorded |
| L1 | (h) | Branin `FnModel` (id `test.branin`), `run` budget 25, seed 698_901, clock 1_700_000_000_000, ledger file: every evaluation entry has the provenance keys of the design, `predicted_mean` and `predicted_sd` finite after the initial 10; `propose_from_entries` on the read file against pick 26 of a run with budget 26 and no ledger; a run of 15 then a run of 10 on one ledger against the 25-run | bit-equal; evaluation lines byte-equal |
| L2 | (h) | L1's lines shuffled (Rng 698_902), written and read: `propose_from_entries` | bit-equal to L1's |
| L3 | design | entries of one evaluator at `analytic` and `reduced_order`; of two evaluator ids in `idx`; one entry without a declared parameter | refused, located, naming the mix; refused; skipped and counted 1 |
| D1 | (h) | O1 seed 1 twice at workers 1 and once at workers 8; F1 seed 1 at workers 1 and 8 | every proposal bit-equal |
| D2 | design | O1 seed 2 replayed through an evaluator answering from the direct run's table | the same points, bit for bit |
| B1 | (i) | `cargo test -p rcfd-api --release --lib design::`, `--test f697_fan_objective`, `--test fan_rom_design_w46b`, `--test f249_default_bits`, `--test f260_default_bits`, `--test f297_default_bits`, `--test baseline_hashes` | pass; no existing test file edited |
| B2 | (i) | rcfd-mcp `default_f1`, `tool_list_f275`, `project_list_f270`; `git diff 6400c929 -- crates/rcfd-mcp` | pass; empty |
| B3 | — | `cargo fmt -p rcfd-api --check`; `cargo clippy -p rcfd-api --all-targets -- -D warnings` | clean |

Heavy records (each `#[ignore]`, run once): O2 with O3 (about 100 s per seed, seeds in parallel) and R1 with R2 (the
9261-point reference, about 12 minutes on 12 threads). Fast tests on the same paths: Hartmann 3 (`n0 = 15`, budget 40, seed 1,
recorded, E2 checked) and F3 with budget 18, seed 1 (entries, provenance and failure reasons checked, no target).

### Mutations (scratch copy, one at a time)

| Id | Mutation | Expected to fail |
|---|---|---|
| M1 | EI with the sign of `f_min - m` flipped | E1, O1 |
| M2 | the mean-uncertainty term of the variance dropped | G2 |
| M3 | jitter 0 | G1b |
| M4 | `s2` divided by `n - 1` | G2 |
| M5 | Matern without the factor `sqrt 5` | G2 |
| M6 | imputation at `m - s` | F1 (doubt: the failure model also steers away) |
| M7 | canonical sort removed | L2 |
| M8 | exclusion radius 0 | E2 (doubt: noise-free EI is 0 at a data point; N1 may show it) |
| M9 | trace term of the gradient with the wrong sign | G4 |
| M10 | leave-one-out without the mean correction (`Q_ii = (K^-1)_ii`) | G7 |
| M11 | believer removed (pending and earlier batch points ignored) | P1 |
| M12 | `classify_failures` false by default | R1 |

### Decision rule

* **done**: every check met (O4, G6c, R2 recorded) and every mutation except M6 and M8 caught.
* **partial**: G0 to G7, E1, E2, P1, O1, NC1, L1 to L3, D1, D2 and B1 to B3 met, and a limit of O2, O3, F1, F2, N1, K1, R1, G5,
  G6a, G6b or NC2 missed (an ignored `*_known_miss` record with the limit intact), or a mutation other than M6 and M8 survives.
* **todo** (the lead does not take the code): G2, E1, O1, L2, D1, B1 or B2 missed.

### Limits (not covered)

* Library only: no service method, MCP tool, study method `gp_ei` (question 1 (b), a follow-up), case key or golden change.
* One fidelity tier, one evaluator and one version per fit (H7 joins tiers); no `allow_mixed_versions`.
* Scalar objective only (question 11 (a)); ParEGO is a follow-up.
* No hyperparameter averaging, no stop rule, no exploration restart; `n <= 2000`; dense `O(n^3)` per likelihood value.
* `minimise` treats infeasible points as failed; declared constraints only through `run`.
* Every fan result is on the reduced-order model with mean power over three impedance points as a proxy objective; it ranks
  variants of the model, not real fans (no measured data).

### Open questions taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

Q2 (a) Matern 5/2, SE opt-in; Q3 (b) analytic gradient with `projected_gradient`, S = 10; Q4 (a) noise off; Q5 (a) `n0 = max(5
d, 10)`; Q6 (a), with (c) brought forward by the smoke (above); Q7 (a); Q8 (a) as a refusal; Q9 (a); Q10 (a) refuse; Q11 (a);
Q12 (b) F-697's layer; Q13 (a) the comparative limit kept; Q14 (a) plain EI (the (b) variant tried in the smoke changed nothing).

## Amendment 2 (2026-11-16, after the first registered run, before any corrected run): the kriging believer's incumbent

The registered run of the fast tests (commit 49f7f75e, log `f698-scratch/run1.log`) met every limit it covers except **P1
batch**: the four points of the batch-4 proposal have a minimum pairwise unit-cube distance of 0.0000 (limit `>= 1e-3`); the
pending part of P1 was met (0.911).

Diagnosis (from the code, no further run): the kriging believer of the registered design adds each chosen point to the data
at its predicted mean, but the committed `Models::believe` left the incumbent unchanged. A believed point whose predicted mean
is below the incumbent then sits in a neighbourhood where the posterior sd is about 0 and the posterior mean is below the
incumbent, so its expected improvement is about `f_min - m > 0`, and the next pick lands just outside the exclusion radius
next to it. In the rule of Ginsbourger, Le Riche and Carraro (2010, from memory) the believed values are data, so they enter the
incumbent like any other value. The code missed the registered design; the set-up and the limit are right.

Correction (registered now): `believe` also updates the incumbent with the believed mean when the believed point is feasible
under the believed constraint means (all `<= 0`; no constraints: always), as `min(incumbent, m)` (or `m` when there was no
incumbent). This code is reached only by pending entries and by the second and later points of a batch: every other registered
check ran with batch 1 and no pending entry, so its bits do not change and its result stands.

The first P1 test stays in the file as the ignored record `p1_batch_and_pending_points_are_spread_known_miss` (limit intact,
measured 0.0000 recorded). A corrected test `p1b_batch_and_pending_points_are_spread` with the same set-up and the same limits
(`>= 1e-3` for the batch and for the pending point) is run once on the corrected code. The decision rule is unchanged, with
P1b in the place of P1.

## Addendum 1 (2026-11-16): results

Code: commits 49f7f75e (surrogate, bayes, tests), e60fb3cc (the amendment 2 correction), 7a92c192 (clippy with the same
bits, docs). Each registered check ran once. The logs are in `f698-scratch` (`run1.log`, `run_p1b.log`, `run_o2o3.log`,
`run_r1r2.log`, `b1.log`, `mutations.log`, `mut_*.log`).

### Results

| Check | Limit | Measured | Met |
|---|---|---|---|
| G0 test functions | 1e-6 / 1e-5 | Branin 0.397887358 at all three; H3 -3.86277979, H6 -3.32236801 | yes |
| G1a Cholesky | <= 1e-12; diag(1, -1) refused at row 1 | 8.2e-16; refused at row 1 | yes |
| G1b duplicate point | factorises, jitter 1e-8 | jitter 1e-8 | yes |
| G2 two-point closed form | rel <= 1e-12 | 2.7e-15 (both kernels) | yes |
| G3 Branin interpolation (10 seeds) | <= 1e-2, sd <= 1e-2 | worst 1.12e-3, sd 1.0e-4 | yes |
| G4 likelihood gradient | <= 1e-5 | 9.7e-10 | yes |
| G5 medians | l in [0.133, 0.3], g in [0.00333, 0.03] | 0.1735, 1.467e-2 | yes |
| G6a pooled coverage | [0.92, 0.98] | 0.9379 | yes |
| NC2 (l = 3) | < 0.92 | 0.8646 | yes |
| G6b each seed | coverage >= 0.90, rms <= 0.10 | worst 0.990, 0.0465 | yes |
| G6c Hartmann 6 | recorded | coverage 0.890 to 0.946, rms/sd 0.50 to 0.74 | recorded |
| G7 leave-one-out | <= 1e-9 | 1.9e-15 | yes |
| E1 EI against quadrature | <= 1e-10, s = 0 exact | 2.8e-14, exact | yes |
| E2 box and exclusion | 0 violations | 0 in every driver test | yes |
| P1 batch (first run) | >= 1e-3 | 0.0000 (pending part 0.911) | **no**, see amendment 2 |
| P1b batch, pending (corrected) | >= 1e-3 | 0.1604, 0.9085 | yes |
| O1 Branin | >= 18/20, median <= 35 | 20/20, median 28 (23 to 32) | yes |
| NC1 random search | <= 4/20 | 0/20 | yes |
| O4 Nelder-Mead | recorded | 14/20 | recorded |
| O2 Hartmann 6 | >= 5/10, largest <= 100, every best <= -3.15 | 6/10, largest 55, worst best -3.20316 | yes |
| O3 against CMA-ES and DE | GP >= max + 2 | GP 6, CMA-ES 1, DE 0 | yes |
| F1 failure disc | >= 18/20, each share <= 0.25 | 20/20, worst 0.080 (pooled 0.030) | yes |
| F2 without imputation and classifier | pooled > F1 | 0.862 > 0.030 | yes |
| N1 noisy Branin | >= 16/20 within 1 | 20/20 (worst true value 0.556) | yes |
| K1 constraint | >= 9/10 within 1 percent | 10/10 (0.1800003 to 0.1800042) | yes |
| R1 reduced-order fan | >= 9/10 within P_ref 1.01; failures located | 10/10, best / P_ref 0.9930 to 0.9935, reached by evaluation 18 to 22; all located | yes |
| R2 CMA-ES, Nelder-Mead | recorded | 0/10, 8/10 | recorded |
| L1 provenance, next point, resume | complete, bit-equal, lines equal | all true | yes |
| L2 shuffled lines | bit-equal | true | yes |
| L3 refusals, skips | mixed levels and mixed evaluators refused; 1 skipped | as stated | yes |
| D1 workers 1 and 8 | bit-equal | true (O1 seed 1, F1 seed 1) | yes |
| D2 replay | bit-equal | true | yes |
| B1 existing tests | pass | lib design:: 76, f697 22, w46b 1, f249 4, f260 1, f297 3, baseline_hashes 3 | yes |
| B2 rcfd-mcp | pass, diff empty | default_f1 2, tool_list_f275 7, project_list_f270 1; diff empty | yes |
| B3 fmt, clippy | clean | clean after 7a92c192 | yes |

The 21^3 reference (5129 of 9261 points feasible, P_ref 1.455477 W at stagger +3 deg, chord 0.86, camber 0.5, 516 s) is a
grid optimum. Every GP run found a lower power (about 1.4455 W, 0.7 percent below), so the ratio is below 1. Of the 50
evaluations of a fan run, 34 to 39 failed (stall), most of them in the initial design and next to the stall boundary, where
the optimum sits.

### Mutations

| | Mutation | Caught by |
|---|---|---|
| M1 | EI sign | E1, O1 |
| M2 | no mean-uncertainty term | G2 |
| M3 | jitter 0 | G1b |
| M4 | process variance divisor n - 1 | G2 |
| M5 | Matern without sqrt 5 | G2 |
| M6 | imputation at m - s | **survived** (registered doubt: the failure model steers away on its own) |
| M7 | no canonical sort | L1 |
| M8 | exclusion 0 | E2 (O1, F1, K1) |
| M9 | trace term sign | G4 |
| M10 | LOO without the mean term | G7 |
| M11 | no believer | P1b |
| M12 | classify_failures off by default | R1 (0 of 10, best / P_ref 1.14 to 1.37) |

Control (the unmutated copy, all non-ignored tests): pass.

### Deviations

* P1 missed on the first run because the believer did not update the incumbent. Amendment 2 recorded the diagnosis and the
  correction before the corrected run. P1 is kept as the ignored `p1_batch_and_pending_points_are_spread_known_miss`, and P1b
  met. Only batches and pending entries reach the corrected code, so the bits of every other check are unchanged.
* Clippy required `!(x > y)` to be written as `x.is_nan() || x <= y`, which gives the same values, NaN included.
* M6 survived, as its registration expected might happen: F1 does not separate the two imputations while the classifier is on.

### Decision

**done**, by the decision rule of amendment 1 with P1b in the place of P1: every check met (O4, G6c and R2 recorded), and
every mutation except M6 and M8 caught (M8 was caught too). The result holds only for the reduced-order tier: no fan result is
checked against measured data.

### Remaining questions (taken on the maintainer's standing instruction of 2026-11-11; the recommendation is given in each)

1. A `gp_ei` study method and an MCP tool: recommend a follow-up after F-702 (Q1 b), not in this item.
2. M6 is not separated: recommend a check with the classifier off and imputation alone (the F2 set-up) in a follow-up, with no
   change here.
3. High failure share on the fan (34 to 39 of 50): recommend an initial design filtered by a cheap stall pre-check of the
   reduced-order model, in a follow-up. The classifier already keeps the later picks feasible.
4. Hartmann 6 stuck at -3.20316 in 4 of 10 runs: recommend a follow-up for hyperparameter averaging or an exploration restart.
5. Dense linear algebra now lives in `surrogate`: recommend the shared module of Q9 b only when a second user appears.

## For the lead: roadmap entries

> Proposed status: done

> Fan breakdown part H3 (docs/fan-project-breakdown.md). ADR 0621, 2026-11-16. DONE (reduced-order tier only). Library only, opt-in (no case key, no MCP tool, no default changed): design::surrogate is ordinary kriging with a Matern 5/2 correlation (squared exponential opt-in), length scales and an optional nugget fitted by maximum likelihood from ten starts with the analytic gradient, a 1e-8 jitter, prediction with the mean-uncertainty term and leave-one-out from one factorisation; design::bayes proposes the next variant by expected improvement after a Latin-hypercube start, imputes failed points at m + s and multiplies by a failure model, takes constraints by feasibility probability, believes pending and batch points at their mean, sorts the observations so the ledger line order does not matter, and drives a ledger Model with the prediction in the provenance of each entry and a surrogate_summary entry naming the fitted model and its parents. Checks: two-point closed form within 2.7e-15, likelihood gradient within 1e-9, leave-one-out within 2e-15 of refits, EI within 3e-14 of quadrature; coverage 0.938 on prior draws (0.865 with a wrong length scale), hyperparameters recovered (median length scale 0.174 for 0.2); the Branin minimum within 1 percent on 20 of 20 seeds in a median of 28 evaluations (random search 0 of 20), Hartmann 6 on 6 of 10 within 55 evaluations against CMA-ES 1 and DE 0 at 200, a failure disc avoided (3 percent failed against 86 percent without the failure handling), noisy Branin 20 of 20, a constraint 10 of 10; on the reduced-order fan (stagger, chord and camber scales, power at three impedance points) 10 of 10 runs within 1 percent of a 9261-point grid optimum by evaluation 22, with every failure located; ledger resume, shuffled lines, worker counts and replay bit-equal; baseline_hashes and the design and tools-list tests unchanged. The batch believer first missed its spread check and was corrected under a recorded amendment. Eleven of twelve mutations caught (the imputation sign survived, as registered). Not covered: multi-fidelity joining (H7), multi-objective EI, the study method and the MCP tool; every number ranks variants of the model, not real fans (no measured data).

> Follow-up proposed: Gaussian-process study method and MCP tool. Deps F-698, F-702. Acceptance: a study with method gp_ei runs through the study store and the design MCP tools with the same entries as bayes::run for the same spec and seed (bytes equal), the tools-list golden changes only by the listed addition, and every F-698 check keeps its result.

> Follow-up proposed: Robust expected improvement on multimodal functions. Deps F-698. Acceptance: on Hartmann 6 (n0 30, budget 200, seeds 1 to 10) at least 9 of 10 runs within 1 percent of the minimum with hyperparameter averaging or a registered exploration restart, every F-698 check keeps its result, and a check with the failure classifier off separates imputation at m + s from m - s (the surviving mutation M6).

> Follow-up proposed: Stall pre-check for the fan initial design. Deps F-698, F-697. Acceptance: on the F-698 R1 set-up the failed share of the 50 evaluations falls below 0.3 on every seed from 1 to 10 while R1 still holds on 9 of 10, with the pre-check evaluations of the reduced-order model counted in the ledger.
