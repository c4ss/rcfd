# ADR 0240: F-268 registration: first-order and total Sobol sensitivity indices for the tolerance study driver

Status: **REGISTERED 2026-11-12**, written before any code of F-268 and before any run of its checks. Not yet accepted by the
maintainer. Results go in the addendum at the end; nothing above the addendum is changed after a run.

Parents: [ADR 0121](0121-wave46d-design-ledger-and-tolerance-study-driver.md) (wave 46D: the design ledger, the tolerance driver
`design::uq` with SRC sensitivities; "Morris and Sobol sensitivity indices" in its out-of-scope list),
[ADR 0067](0067-wave32c-design-loop.md) (the design loop, `design::rng`, `design::sampling`). Roadmap item F-268 (depends on
F-249).

Number: the lead gave 0240 to F-268. Branch `f268-sobol-indices` from main 1dd5a0e0, worktree `w49i-work/f268`.

House rules (as ADR 0231 and 0235):

- limits are fixed here, before the code and the runs, and are not changed after a run;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact and the measured number recorded, and the cause is
  diagnosed, not tuned away;
- a defective set-up gets a new corrected test by an amendment written (and committed) before the corrected run, and the old test
  stays as an ignored record;
- defaults stay bit-identical: `uq::run`, the ledger, the design tools, studies and maps are not changed in behaviour; the new
  capability is a new opt-in function; `crates/rcfd-api/tests/baseline_hashes.rs`, hash tables and goldens (MCP tools-list, CLI help)
  are not edited or re-recorded; no existing test is edited;
- each registered run is made once, and nothing is re-run to pick a result;
- no new third-party crate, `Cargo.lock` unchanged, no `unsafe`, unsigned commits, no trailers, no push.

Files this branch does not touch (other branches work on them): `crates/rcfd-fv/src/{overset,immersed,compressible,compressible_bc}.rs`,
`crates/rcfd-api/src/sim.rs`, `crates/rcfd-api/src/schema.rs`, and the roadmap files (`docs/roadmap.json`, `docs/ROADMAP.md`,
`docs/roadmap-overview.md`, `scripts/roadmap_progress.py`, the SVGs).

## What F-268 asks and what exists

Acceptance (quoted, binding): "Every design variant is recorded in a ledger with its inputs, solver version and results, and a
tolerance study driver samples manufacturing spread and reports output spread against registered limits (ADR 0121)."

Note (quoted): "Wave 46D (ADR 0121). Size M. Every model is a stated proxy until checked against measured fan data, which does not
exist yet. Not done: uncertainty bands on the reduced-order model, automatic ledger logging from runs, Sobol sensitivity indices."

What exists, checked in the source at 1dd5a0e0 (not taken from the note):

- `design::ledger` (ADR 0121 D1 to D5): the JSON-lines ledger with inputs, units, evaluator and solver version, verdict and tier per
  entry; queries, export, the re-run check; four service methods and four MCP tools. The ledger clause of the acceptance is met for
  entries that a caller appends; nothing logs runs into it automatically.
- `design::uq` (ADR 0121 D6): `uq::run(spec, model, ledger)` samples named inputs (uniform, truncated normal, beta) by Monte Carlo
  or replicated Latin hypercube, stops on a standard-error rule, reports mean, sd, SE, order-statistic quantile intervals, the
  probability of violating each requirement (Clopper-Pearson) and the standardised regression coefficients (SRC) with R^2. Every
  sample is a ledger entry and one `uq_summary` closes the study. This is the "samples manufacturing spread and reports output spread
  against registered limits" clause, on analytic evaluators (U1 to U8 passed).
- Sensitivity: only SRC. ADR 0121 D6 states its limit: a linear measure that misses interactions and non-monotone effects (it would
  mis-rank an Ishigami-type function). No variance-based (Sobol) index exists anywhere in `crates/` (searched: `sobol` occurs only
  as the quasi-random sequence `sampling::sobol` of ADR 0067 and as the study method name `sobol`; `sensitivity` only in the adjoint
  shape sensitivities of ADR 0091 and in the SRC text of `uq.rs`).

What this branch does: the third "not done" item, Sobol sensitivity indices, as a new opt-in library function next to `uq::run`.
The other two items (uncertainty bands on the reduced-order model, automatic ledger logging from runs) are not attempted here and
keep F-268 partial (see the decision rule).

## What was read and computed before this text (disclosed)

- **Read:** ADR 0121 in full; ADR 0231 and 0235 for the form; `crates/rcfd-api/src/design/uq.rs` in full; the head of
  `uq_tests.rs` (helpers `normal`, `model`, `fixed_spec`, the U6 fixed-clock ledger harness); `ledger.rs` (`Entry`, `validate`,
  `Ledger::append`, `clock`, `Outcome`, `Model`, `FnModel`, `entry_of`); `design/mod.rs`; `design/rng.rs` (re-export of
  `rcfd_core::rng::Rng`, xoshiro256** seeded by SplitMix64); `study::parallel_map` (order-preserving worker pool); the F-268 entry
  of `docs/roadmap.json`.
- **Computed beforehand, with an independent numpy script of my own** (not the code under test, which did not exist): the closed
  forms below, and the Monte Carlo spread of the estimators of the design section on the two test functions, to set the limits.
  Ishigami with a dummy input at N = 16384, 200 replications (numpy generator seed 12345); the linear function at N = 16384, 200
  replications (seed 777); the coverage of the delta-method interval at N = 1024 over 1000 replications. The numbers are in the
  "estimates behind the limits" table. This is the only prior knowledge of how the estimators behave; no Rust code was run.

## Design (registered)

### Where

New module `crates/rcfd-api/src/design/sensitivity.rs` with tests in `sensitivity_tests.rs` (as `uq.rs` / `uq_tests.rs`), declared
in `design/mod.rs`. One visibility change in `uq.rs`: the private `evaluate_guarded` (the panic guard and verdict normalisation of
an evaluation) becomes `pub(crate)` so that both drivers treat a failed evaluation the same way; its body is unchanged. Nothing else
in `uq.rs`, the ledger, the tools, the service or the MCP crate changes.

### Definitions (Sobol 1993; Homma and Saltelli 1996)

For `Y = f(X_1, ..., X_d)` with independent inputs and `V = Var(Y)`, the first-order index and the total index of input `i` are

- `S_i = V_i / V`, `V_i = Var(E[Y | X_i])` (the share of the variance explained by `X_i` alone);
- `ST_i = VT_i / V`, `VT_i = E[Var(Y | X_~i)] = V - Var(E[Y | X_~i])` (everything that involves `X_i`, interactions included),

where `X_~i` is every input except `X_i`. `0 <= S_i <= ST_i <= 1`; for an additive `f`, `S_i = ST_i` and `sum S_i = 1`; an input
that `f` does not depend on has `S_i = ST_i = 0`. Independence of the inputs is required (the decomposition of the variance into
these terms does not hold for dependent inputs); every distribution of `uq` is independent per input.

### Sampling design (Saltelli 2002; Saltelli et al. 2010)

- Two independent `N x d` matrices `A` and `B` of input samples. For each `i`, `A_B^(i)` is `A` with column `i` taken from `B`.
- The model is evaluated on the rows of `A`, `B` and the `d` matrices `A_B^(i)`: **cost `N (d + 2)` evaluations**, giving every
  `S_i` and `ST_i` for every metric.
- Uniforms: one `Rng` stream (seed from the spec); row `j` draws `2 d` uniforms in order, the first `d` for `A_j`, the next `d` for
  `B_j`, each `((next_u64 >> 11) + 0.5) / 2^53` (the Monte Carlo uniforms of `uq::run`). So the first `N'` rows of a run with `N > N'`
  equal the rows of a run with `N'` (a prefix property, checked in C5). Inputs by the inverse CDF of `uq::Dist` (`Dist::inv_cdf`).
- Plain Monte Carlo only. A quasi-random (Sobol-sequence) design converges faster but its error has no simple standard error and
  `sampling::sobol` stops at 10 dimensions (that is `d <= 5` for `A` and `B` together); not provided (limit, Q3).
- Evaluation order: all `N (d + 2)` points in the order row 0 (`A_0`, `B_0`, `A_B^(1)_0`, ..., `A_B^(d)_0`), row 1, ..., through
  `study::parallel_map` (order-preserving), so the result and the ledger do not depend on the worker count.

### Estimators and their derivation

Write `a_j = f(A_j)`, `b_j = f(B_j)`, `c_j^i = f(A_B^(i)_j)` for one metric, over the `n` usable rows (below).

- Mean and variance: `m = (1 / 2n) sum_j (a_j + b_j)`; `V^ = (1 / 2n) sum_j [(a_j - m)^2 + (b_j - m)^2]` (the pooled variance of
  the `2n` values `a`, `b`, divisor `2n`).
- **First order** (Saltelli et al. 2010, Table 2 (b), centred): `S_i^ = [(1 / n) sum_j (b_j - m) (c_j^i - a_j)] / V^`.
  Derivation: `B_j` and `A_B^(i)_j` share exactly `X_i`, and `A_j` is independent of `B_j`. So
  `E[b (c - a)] = E[b c] - E[b] E[a] = E[E[Y | X_i]^2] - f0^2 = Var(E[Y | X_i]) = V_i`. Subtracting the constant `m` from `b` adds
  `m E[c - a] = 0` (both are distributed as `Y`), so the centred form has the same expectation (up to the `O(1/n)` term from `m`
  being estimated) and does not let a large mean `f0` inflate the variance of the estimator (Saltelli et al. 2010 give the
  uncentred form; the centring is my own first-principles step, disclosed).
- **Total** (Jansen 1999; Saltelli et al. 2010, Table 2 (f)): `ST_i^ = [(1 / 2n) sum_j (a_j - c_j^i)^2] / V^`.
  Derivation: `A_j` and `A_B^(i)_j` share every input but `X_i`, so `E[(a - c)^2] / 2 = V - Cov(a, c) = V - Var(E[Y | X_~i]) = VT_i`.
- **Dummy input:** if `f` does not read `X_i`, then `c_j^i` and `a_j` are the same evaluation of the same inputs, bit for bit
  (deterministic model), so every term `(c - a)` is exactly zero and `S_i^ = ST_i^ = 0` exactly (a signed zero allowed), whatever `n`.
- **Standard error (delta method, first principles):** each index is a ratio `R = u-bar / V^` of the mean of a row quantity `u_j`
  (`(b_j - m)(c_j^i - a_j)` or `(a_j - c_j^i)^2 / 2`) and the mean of `w_j = ((a_j - m)^2 + (b_j - m)^2) / 2`. The rows are
  independent, so to first order `R^ - R = (1 / n) sum_j psi_j` with `psi_j = (u_j - u-bar - R^ (w_j - V^)) / V^`, and
  `SE = sd(psi) / sqrt(n)` (sample sd, divisor `n - 1`). The variation of `m` is neglected (second order). This is the standard
  linearisation of a ratio estimator; Saltelli et al. (2010) use bootstrap intervals instead (from memory; not used, because the
  delta method is cheaper and its coverage is checked here, C4).
- The estimators may fall outside `[0, 1]` (sampling error); they are reported unclipped, with their SE.

These are the published estimators as I recall them: Saltelli et al. (2010) is cited **from memory** (the table letters (b) and (f)
UNVERIFIED); the expectations above are derived here and do not rely on the citation.

### Usable rows and failures

A row is usable when all `d + 2` of its evaluations are `ok` (verdict `ok`, finite metrics, no panic; `uq::evaluate_guarded`). A row
with any failure is dropped as a whole (its `a`, `b` and `c^i` belong together); the estimators use the `n <= N` usable rows. The
result reports `n_rows`, `n_rows_used`, `n_evaluations` and `n_failed_evaluations`. Dropping rows conditions the inputs on success,
so with input-dependent failures the indices are those of the conditioned inputs, which are then not independent: stated in the
result's text and not corrected. Fewer than 2 usable rows, or `V^ = 0`, give NaN indices and SE (no error), and a summary entry with
verdict `not_evaluated` and a reason.

### Interface

```rust
pub struct SobolSpec { pub name: String, pub params: Vec<UncertainParam>, pub n: usize, pub seed: u64, pub workers: usize }
pub struct SobolIndex { pub param: String, pub first: f64, pub first_se: f64, pub total: f64, pub total_se: f64 }
pub struct SobolMetric { pub name: String, pub unit: String, pub mean: f64, pub variance: f64, pub indices: Vec<SobolIndex> }
pub struct SobolResult { pub n_rows: usize, pub n_rows_used: usize, pub n_evaluations: usize,
                         pub n_failed_evaluations: usize, pub metrics: Vec<SobolMetric> }
pub fn run(spec: &SobolSpec, model: &dyn Model, ledger: Option<&Ledger>) -> Result<SobolResult, String>;
pub fn estimate(a: &[f64], b: &[f64], c: &[Vec<f64>]) -> (f64, f64, Vec<(f64, f64, f64, f64)>); // mean, V, (S, se, ST, se)
pub fn result_json(spec: &SobolSpec, res: &SobolResult) -> Value;
```

- `validate` refuses, with located messages: no parameter, an empty name or unit, a name given twice, an invalid distribution (the
  `Dist::validate` text), `n < 2`, `workers == 0`, `n (d + 2)` overflowing `usize`. A metric set that changes between `ok` samples is
  an error (as `uq::run`).
- Ledger (optional): every evaluation is an entry (`kind = evaluation`, variant `<name>#<j>.A`, `.B`, `.AB<i>` with `i` from 1,
  provenance `{driver: "sobol", matrix: "A" | "B" | "AB", column: i (AB only, 1-based), row: j}`, the seed), appended per row block in
  evaluation order from the calling thread after the evaluations; one `sobol_summary` entry closes the run (metrics `<m>.mean`
  (metric unit), `<m>.variance` (unit `(<u>)^2`, or `1`), and per parameter `<m>.S.<p>`, `<m>.S_se.<p>`, `<m>.ST.<p>`,
  `<m>.ST_se.<p>` (unit `1`); the full result in its provenance).
- No service method, no MCP tool, no CLI command (the evaluator is a Rust closure that cannot cross MCP, as for `uq::run` in ADR 0121
  D5; a tool would also change the MCP tools-list goldens). Remaining step, reported.

## Closed forms of the test functions

**Ishigami** (Ishigami and Homma 1990; the indices as in Saltelli et al. 2008, Primer, from memory; derived here):
`f = sin x1 + a sin^2 x2 + b x3^4 sin x1`, `x1, x2, x3 ~ U(-pi, pi)`, `a = 7`, `b = 0.1`, and a dummy `x4 ~ U(0, 1)` that `f` does
not read (so `d = 4`). With `E sin^2 = 1/2`, `E sin^4 = 3/8`, `E x^4 = pi^4 / 5`, `E x^8 = pi^8 / 9` for `x ~ U(-pi, pi)`:

- `V1 = Var(sin x1 (1 + b E x3^4)) = (1 + b pi^4 / 5)^2 / 2 = 4.345885`; `V2 = a^2 Var(sin^2 x2) = a^2 / 8 = 6.125`; `V3 = 0`;
- `V13 = b^2 Var(x3^4) E sin^2 x1 = b^2 (pi^8 / 9 - pi^8 / 25) / 2 = 8 b^2 pi^8 / 225 = 3.373700`;
- `V = V1 + V2 + V13 = 13.844588` (equal to `a^2/8 + b pi^4/5 + b^2 pi^8/18 + 1/2`).
- `S = (0.3139052, 0.4424111, 0, 0)`, `ST = (0.5575889, 0.4424111, 0.2436837, 0)`.

**Linear additive with mixed distributions and a dummy:** `y = 1 + 2 x1 + x2 - 0.5 x3`, `x1 ~ N(0, 1)` (untruncated),
`x2 ~ U(0, 3)`, `x3 ~ Beta(2, 5)` on `[0, 4]`, dummy `x4 ~ U(0, 1)` not read. Variances of the terms: `4`, `0.75`,
`0.25 * 16 * (2 * 5) / (7^2 * 8) = 0.1020408`; `V = 4.8520408`. `S = ST = (0.8243954, 0.1545741, 0.0210305, 0)`.

## Estimates behind the limits (numpy, disclosed above)

Empirical sd of the estimator over 200 replications at `N = 16384` (the predicted SE, `SE_p`), and the mean delta-method SE:

| Function | index | x1 | x2 | x3 |
| --- | --- | --- | --- | --- |
| Ishigami | `SE_p` of S | 0.0076 | 0.0066 | 0.0074 |
| Ishigami | `SE_p` of ST | 0.0118 | 0.0054 | 0.0035 |
| Ishigami | mean delta SE (S; ST) | 0.0078; 0.0110 | 0.0069; 0.0052 | 0.0072; 0.0034 |
| linear | `SE_p` of S | 0.0101 | 0.0043 | 0.0016 |
| linear | `SE_p` of ST | 0.0072 | 0.0021 | 0.00027 |
| linear | mean delta SE (S; ST) | 0.0100; 0.0074 | 0.0042; 0.0018 | 0.0016; 0.00028 |

The replication-to-replication spread of the delta SE was about 1 percent (linear function). Coverage of `|R^ - R| <= 1.96 SE` on
Ishigami at `N = 1024` (1000 replications): 0.937 to 0.954 for the six non-dummy indices; of `0.98 SE`: 0.655 to 0.704.

## Registered checks

All in `crates/rcfd-api/src/design/sensitivity_tests.rs`, run in the release profile
(`cargo test -p rcfd-api --release --lib design::sensitivity`). `K = 4`: under the normal approximation one comparison fails by
chance with probability 6.3e-5; a family of 12 comparisons with probability below 8e-4.

| Id | Check (seed) | Limit |
| --- | --- | --- |
| C1 | Ishigami with the dummy, `N = 16384` (cost 98 304 evaluations), seed 268001 | for each of S1, S2, S3, ST1, ST2, ST3: `|est - exact| <= 4 SE_p` with `SE_p` of the table; the reported SE within `[0.75, 1.33] SE_p`; S4, ST4, and both their SEs exactly 0 |
| C2 | linear additive with the dummy, `N = 16384`, seed 268002 | the same form for the six non-dummy indices with the linear `SE_p`; the dummy exactly 0 (indices and SEs); the reported mean within `4 sqrt(V / 2N) = 0.0688` of 1 and `|V^ - 4.8520408| <= 0.25` (about 5 SE of a pooled variance of a nearly normal output: `sqrt(2 V^2 / 2N) = 0.038`; the beta and uniform terms make it 0.25 conservative) |
| C3 | interaction is seen (C1's run) | `ST3 - S3 >= 0.2` and `ST1 - S1 >= 0.18` (exact differences 0.2437 and 0.2437; with `SE_p` the margins are at least 4.5 SE of the difference, taking the two estimates as independent: `sqrt(0.0074^2 + 0.0035^2) = 0.0082` and `sqrt(0.0118^2 + 0.0076^2) = 0.0140`; registered as its own line because it is the property SRC lacks) |
| C4 | SE coverage: 400 runs of Ishigami with the dummy at `N = 1024`, seeds 268100 to 268499 | for each of the six non-dummy indices the fraction with `|est - exact| <= 1.96 SE` is at least 0.90 (predicted 0.94 to 0.95; 0.90 is 4 binomial SE, `sqrt(0.95 * 0.05 / 400) = 0.011`, below 0.95) |
| C5 | determinism: Ishigami with the dummy, `N = 1024`, seed 268003, a fixed ledger clock | ledger bytes and the result (every float by bits) identical for 1 and 4 workers; seed 268004 gives different bytes; the rows of a run with `N = 512` (seed 268003) equal the first 512 rows of the `N = 1024` run (every parameter of every A and B entry by bits) |
| C6 | design structure and cost: Ishigami with the dummy, `N = 64`, seed 268005, ledger | `n_evaluations = 64 * 6 = 384`; 385 ledger entries (384 `evaluation` and one `sobol_summary`, verdict ok); for every row `j` and column `i` the `AB<i>` entry equals the `A` entry in every parameter except `x_i`, which equals the `B` entry's `x_i` (bits); the summary metrics `y.S.<p>` and `y.ST.<p>` equal the result's indices by bits |
| C7 | failures: the linear model returning `diverged` (with a reason) when `x2 > 2.9` and panicking when `x2 < 0.01`, `N = 2048`, seed 268006, ledger | `n_failed_evaluations` equals the number of non-ok evaluation entries in the ledger, each with a non-empty reason; `n_rows_used` equals `N` minus the number of rows (counted from the ledger) with at least one non-ok entry; every index finite; the summary verdict ok |
| C8 | refusals | each of: no parameter, empty unit, a name twice, `Dist::Uniform { lo: 1, hi: 1 }`, `n = 1`, `workers = 0` is an `Err` naming the key (`params`, `unit`, the name, the parameter name, `n`, `workers`); a model whose metric names change between samples is an `Err`; nothing is written to the ledger by a refused spec |

### Negative controls

| Id | Control | Limit |
| --- | --- | --- |
| NC1 | C1's estimates against the closed forms of Ishigami with `a = 6` (`S2 = ST2 = 0.3682612`, `ST1 = 0.6317388`, `ST3 = 0.2760895`) | S2, ST2, ST1, ST3 each outside `4 SE_p` of the wrong value (predicted distances 0.074, 0.074, 0.074, 0.032 against `4 SE_p` 0.026, 0.022, 0.047, 0.014); the limit of C1 can fail. S1 (`0.3556493`, distance 0.042 against 0.030) is reported, not a limit |
| NC2 | the existing SRC of `uq::run` on the same Ishigami inputs (MC, `n = 20000`, seed 268007, unchanged code) | `SRC_2^2 < 0.01` while C1's `S2 > 0.4`: SRC misses the non-monotone input that the first-order index finds (sin^2 x2 is even, so its linear correlation with x2 is zero) |
| NC3 | C4's coverage with `0.98 SE` (half the width) | at most 0.80 for each of the six indices (predicted 0.66 to 0.70): the coverage check can fail |
| NC4 | C5's seed 268004 | different ledger bytes (in C5) |

### Mutations (scratch copy, not committed)

Each applied alone to a `git archive` copy of the final commit (scratch worktree `w49i-work/f268-scratch`, its own target) by an
exact text replacement, the sensitivity tests run once in release, the file restored.

| Id | Mutation | Must fail |
| --- | --- | --- |
| MU1 | `A_B^(i)` keeps column `i` of `A` (so it equals `A`) | C1, C2 (every ST and S zero) |
| MU2 | the first-order multiplier `(a_j - m)` instead of `(b_j - m)` (expectation `-VT_i`) | C1, C2 |
| MU3 | the total estimator without the factor 1/2 | C1, C2 |
| MU4 | `B_j` drawn from the same uniforms as `A_j` | C1, C2 |
| MU5 | `SE = sd(psi) / sqrt(2 n)` (the `2n` pooled values taken as the sample count) | C1 (SE ratio 0.71 < 0.75) and C4 (coverage of 1.39 SE about 0.83 < 0.90) |
| MU6 | a row dropped only when its `A` or `B` evaluation fails | C7 |

### Predictions (mine)

- PR1: C1 to C8 met; the largest `|z|` of C1 and C2 below 3.
- PR2: C4 coverage 0.93 to 0.96.
- PR3: the release run of the sensitivity tests under 30 s.

## Decision rule (registered)

The rule decides whether the "Sobol sensitivity indices" item of F-268's note is met; F-268 as a whole stays **partial** in every
case, because "uncertainty bands on the reduced-order model" and "automatic ledger logging from runs" are not attempted here, and
the models stay analytic proxies (no measured fan data).

- **done (Sobol part)**: C1 to C8 met, NC1 to NC3 hold, MU1 to MU6 each make a registered check fail, and the defaults unchanged
  (no existing test edited; `uq_tests` U1 to U8 and `ledger_tests` pass at the final commit).
- **partial (Sobol part)**: C5, C6, C8 and the exact dummy zeros met, but a closed-form or coverage limit (C1 to C4) missed (a known
  miss, diagnosed), or a control or mutation not caught.
- **todo (Sobol part)**: C5 or C6 missed (non-deterministic or a wrong design), a dummy index not exactly zero, or an existing test
  changed; not merged.

## Limits of this item

- Independent inputs only; correlated manufacturing spread is not supported (as `uq`).
- Plain Monte Carlo; error `O(N^-1/2)`; no quasi-random design, no bootstrap interval, no stopping rule on the SE of an index (a
  fixed `N`).
- The delta-method SE is first order; at small `N` or for outputs with heavy tails it can be optimistic (C4 checks it at
  `N = 1024` on Ishigami only).
- Indices with failed rows dropped describe the conditioned inputs (stated above).
- Verified on analytic functions; says nothing about the sensitivity of a real fan's outputs. No MCP tool or service method.

## Open questions and choices

- **Q1. Centring of the first-order estimator.** (a) centred by the pooled mean (registered above); (b) Saltelli et al. (2010) as
  published, uncentred. Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation): (a), because its
  variance does not grow with the mean of the output.
- **Q2. Exposure.** (a) library only (registered); (b) a service method and MCP tool for a named evaluator (the reduced-order fan
  evaluator `design::fan_rom_eval`), which changes the MCP tools-list goldens. Not taken: (b) changes a golden; open, listed in the
  addendum.
- **Q3. Quasi-random design.** (a) Monte Carlo only (registered); (b) an opt-in Sobol-sequence design for `d <= 5`. Taken on the
  standing instruction (take the recommendation): (a); (b) proposed as a follow-up only if wanted.

## References

- I. M. Sobol (1993), Sensitivity estimates for nonlinear mathematical models, Mathematical Modelling and Computational
  Experiments 1, 407-414 (definition of the indices; from memory).
- T. Homma, A. Saltelli (1996), Importance measures in global sensitivity analysis of nonlinear models, Reliability Engineering and
  System Safety 52, 1-17 (total index; from memory).
- M. J. W. Jansen (1999), Analysis of variance designs for model output, Computer Physics Communications 117, 35-43 (the total
  estimator; from memory).
- A. Saltelli (2002), Making best use of model evaluations to compute sensitivity indices, Computer Physics Communications 145,
  280-297 (the `A`, `B`, `A_B^(i)` design; from memory).
- A. Saltelli, P. Annoni, I. Azzini, F. Campolongo, M. Ratto, S. Tarantola (2010), Variance based sensitivity analysis of model
  output. Design and estimator for the total sensitivity index, Computer Physics Communications 181, 259-270 (estimators (b) and (f);
  from memory, table letters UNVERIFIED).
- T. Ishigami, T. Homma (1990), An importance quantification technique in uncertainty analysis for computer models, Proc. ISUMA'90,
  398-403 (the test function; from memory). Its indices are derived above.
- Delta method for a ratio of means: first principles.
