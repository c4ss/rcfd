# ADR 0121: Wave 46D, a durable design ledger (H4) and a tolerance/uncertainty study driver (F1)

Status: **ACCEPTED (registered 2026-10-26)**. The limits, seeds and sample sizes below were fixed before any registered test ran. The
house rules of ADRs 0110-0117 apply:
* a missed limit becomes an ignored `*_known_miss` test with the limit intact;
* a defective set-up gets a new corrected test, registered here as an amendment before it runs;
* defaults stay bit-identical, and `crates/rcfd-api/tests/baseline_hashes.rs` is not re-recorded.

Written at main 3175951 (worktree `<scratch dir>`, branch `w46d`, target `<target dir>`). Clean-room: first
principles and published statistics only. Parents: ADR 0067 (design loop, store, MCP design tools), `docs/fan-project-breakdown.md`
parts H4 and F1.

## Context

`docs/fan-project-breakdown.md` asks for (H4) a ledger of every evaluated variant that survives sessions and lets a result be re-run,
and (F1) a tolerance study that samples perturbations, evaluates them and reports the spread and the tolerances that matter. What
exists (checked in the source, not taken from the breakdown):

* `design::store` (ADR 0067): one JSON-lines store per study, keyed by the exact bit pattern of the parameter vector, bound to one
  study signature. It has no units, no evaluator id or version, no solver version, no geometry hash, no verdict classes beyond
  ok/failed, no fidelity tier and no time stamp. It is the resume store of one optimiser run, not a cross-study ledger.
* `design::sampling` (seeded Latin hypercube, Sobol), `design::rng` (xoshiro256**), `design::pareto` (non-dominated sort),
  `design::study::parallel_map` (order-preserving worker pool), and `rcfd_reactor::mixfrac` (`erfc`, `erfc_inv`, `reg_inc_beta`).
* No tolerance or uncertainty driver, no input distributions other than uniform boxes, no sensitivity measure.

Agents 46A (fan perturbation operators) and 46B (reduced-order fan evaluator) work in parallel. Nothing here depends on them: the
driver takes any evaluator through a generic trait, and every check uses analytic evaluators.

## What I already knew when registering (disclosed)

* All code was written before this registration, because the rule-d smoke needs it: `design/ledger.rs`, `design/uq.rs`, their test
  files, the four service methods and MCP tools, and the M1 test.
* The ignored smokes `smoke_w46d` (U1-U8 set-ups with seeds 1-13, not the registered seeds) and `smoke_w46d_ledger` (the L3 and L4
  harnesses with 50 entries and 4 x 50 threads) ran before registration. Results below.
* The smoke found one defective negative control: in U2 the wrong model scaled the input sd by 1.1 **and** shifted the mean by 0.1 sd;
  at the lower tail the shift cancels the scaling (MC linear p = 0.05: exact 6.0959 inside [5.9728, 6.1075]). Before registration the
  U2 wrong model was changed to the sd scaling alone (U1 keeps the shift, where it is caught at 14 to 25 SE). The corrected U2 control
  was smoked once more with the smoke seeds: every p = 0.05 and 0.95 exact quantile fell outside.
* The MCP tests of `rcfd-mcp` (M1 among them, and the existing T1/T2 with the four tools added) ran twice during development. M1 is a
  functional pass/fail check without a tunable limit; the first run showed that T1's shared argument object needed `params`/`metrics`
  per schema type (a test harness fix, not a limit) and that `tests.rs` counts the tools (29 -> 33).
* L1, L2, L5, L6 and U1-U8 with the registered seeds had not run.

## Decisions

1. **D1 Ledger file and entry** (`crates/rcfd-api/src/design/ledger.rs`).
   * One JSON object per line, keys sorted (serde_json `BTreeMap`), `format = "rcfd-ledger-1"` on every line (no header line, so two
     first appends cannot race on a header). The entry index is the position of the valid line; nothing else numbers entries.
   * Fields: `kind` (`evaluation`, or a summary kind such as `uq_summary`), `study`, `variant`, `params` and `metrics` as lists of
     `{name, value, unit}` (unit required, `"1"` for dimensionless; names unique), `evaluator`, `evaluator_version`, `solver_version`
     (stamped `rcfd <version>` plus ` (<rev>)` when the build sets `RCFD_GIT_REV`; a `git archive` build has no revision),
     `geometry_hash`, `settings` (free JSON: what re-runs it), `verdict` (`ok`, `diverged`, `constraint_rejected`, `not_evaluated`;
     a located `reason` is required unless `ok`; `ok` requires finite metrics), `fidelity` (`analytic`, `reduced_order`, `cfd`),
     `recorded_at_ms` (from an injectable clock), `wall_s`, `seed`, `provenance` (free JSON).
   * Numbers: the workspace enables serde_json `float_roundtrip`, so every finite value reads back bit for bit; a NaN metric of a
     non-ok entry is stored as null and reads back as NaN.
2. **D2 Crash safety and concurrency.**
   * A batch of entries is serialised first, then written with one `write_all` on a file opened for append, then `sync_data`.
   * If the file does not end with `\n` (a torn line from a killed writer), the write starts with `\n`, so the fragment stays a
     separate line. Nothing is truncated or rewritten, ever.
   * The reader skips and reports every line that is not a complete entry (`skipped_lines`); a complete last line without its newline
     counts (no proper prefix of a JSON object parses).
   * Threads of one process share one `Ledger` (an in-process mutex). Separate handles and processes rely on the append-mode write
     of one line being atomic; there is no OS file lock (`File::lock` is Rust 1.89, the MSRV is 1.75). Single-writer-process is the
     stated assumption; the MCP append reads the index back after the write, which is exact only under that assumption.
3. **D3 Queries.** Filter (kind, verdict, study, evaluator, fidelity); Pareto membership for `[(metric, sense)]` (reusing
   `pareto::front`); best so far for `(metric, sense)` with the running-best trace; history of one parameter; CSV and JSON export.
   Pareto and best use only `ok` evaluation entries that carry the metrics, and refuse with a located error a set that mixes
   evaluators or tiers, or a metric recorded in two units (filter first).
4. **D4 Re-run.** `Model` trait (id, version, tier, `evaluate(&[Quantity]) -> Outcome`) and `FnModel` for closures. `rerun_check(entry,
   model, rtol, atol)` refuses another evaluator id or version, re-evaluates from the stored named parameters, and compares metrics
   (per metric `|new - recorded| <= atol + rtol |recorded|`), units, verdict and geometry hash. `compare_metrics` does the comparison
   alone, for a re-run done elsewhere (the MCP tool).
5. **D5 Tools.** Service methods `design.ledger_append`, `design.ledger_query`, `design.ledger_export`, `design.ledger_rerun_check`
   and MCP tools `design_ledger_append` (write_files), `design_ledger_query` (read), `design_ledger_export` (write_files),
   `design_ledger_rerun_check` (read). Paths go through the sandbox (1004 outside). F1 is library-only: a closure evaluator cannot
   cross MCP; an MCP tolerance tool needs a named evaluator (46B's reduced-order model is the candidate) and is a proposed item.
6. **D6 Tolerance driver** (`crates/rcfd-api/src/design/uq.rs`).
   * Inputs: named parameters with units and a distribution: uniform; normal truncated to `[lo, hi]` (infinite bounds allowed;
     refused when the kept mass is below 1e-12, because `Phi^-1(Phi(a) + u (Phi(b) - Phi(a)))` loses accuracy when both bounds lie far
     in one tail); beta(a, b) on `[lo, hi]` (inverse by bisection on `reg_inc_beta`: cheap because the function exists).
   * Sampling by inverse CDF of uniforms clamped into the open unit interval. Monte Carlo: one xoshiro256** stream, so the first n
     samples do not depend on the batch size. Latin hypercube: replicated, one independent LHS of `batch` points per replicate (an LHS
     cannot be extended and stay stratified).
   * Stopping rule: after each batch (and at least `min_batches`), stop when every controlled metric has `SE <= max(se_abs, se_rel
     |mean|)`; stop with `converged = false` at `max_samples`. MC: `SE = s / sqrt(n)`. LHS: `SE = sd(replicate means) /
     sqrt(replicates)`, at least 2 replicates.
   * Per metric: mean, sd (n - 1), SE, quantile `X_(ceil(n p))` with the distribution-free order-statistic interval `[X_(l), X_(u)]`
     chosen from the binomial CDF (`P(B <= k) = I_{1-p}(n - k, k + 1)`) with each tail at most `(1 - confidence) / 2`, and its exact
     coverage `F(u - 1) - F(l - 1)`. The coverage is exact for independent samples (MC); for LHS samples it is the i.i.d. interval,
     an approximation.
   * Probability of violating each requirement with the Clopper-Pearson interval. A failed sample (verdict not ok, panics included)
     counts as a violation of every requirement; metric statistics are over the ok samples only. Failures are never dropped silently:
     each is a ledger entry with its reason.
   * Sensitivity: **standardised regression coefficients** (SRC_i = b_i s(x_i) / s(y) of a linear least-squares fit) with R^2, and
     the ranking by |SRC|. Limits: SRC measures linear effects only; it misses interactions and non-monotone effects (R^2 well below 1
     says the ranking is not trustworthy) and assumes independent inputs. It would mis-rank an Ishigami-type function, so it is
     verified on a linear-plus-interaction function. Morris and Sobol indices are not provided.
   * Every sample is a ledger entry (`kind = evaluation`, provenance `{driver: uq, sampler, sample, batch}`, the seed); one
     `uq_summary` entry closes the study (metrics `<m>.mean`, `.sd`, `.se`, `.q<p>`, `requirement<i>.<m>.p_violation`; the full
     result, the distributions and the stopping rule in its provenance). Samples are appended per batch in sample order from the
     calling thread, so the file does not depend on the worker count.
7. **D7 Store repair.** `Store::open` removed a torn last line by rewriting the whole file, which a kill during the rewrite turns into a
   total loss. It now truncates in place to the last newline (`File::set_len`); the resulting bytes are the same as before for LF files.
8. **D8 No default changes.** No existing path calls the ledger or the driver; the existing design tools are not wired to log into the
   ledger (that would change their files: a proposed item).

## Registered checks

Common: `K = 4` standard errors. Under the normal approximation one comparison fails by chance with probability 2 (1 - Phi(4)) =
6.3e-5. Seeds are fixed, so each test is deterministic; the probabilities below are the a-priori chance that a correct implementation
misses with the registered seeds.

### Library ledger (`crates/rcfd-api/src/design/ledger_tests.rs`)

* **L1 round trip and validation** (`l1_round_trip_is_exact_and_validation_is_located`).
  * Six entries, chord values 0.1, 1/3, sqrt 2, 1e-300, 123456.789, -0.0, appended in two batches; one diverged entry with a NaN
    metric, one constraint-rejected, one with `wall_s`.
  * Limit: every number reads back with the same bit pattern (NaN as NaN), every other field equal, the solver version and the clock
    stamp filled.
  * Six refusals with located messages (empty unit, duplicate metric, infinite metric on ok, missing reason, missing evaluator version,
    NaN parameter); a refused batch writes nothing.
  * Negative control: a line with another `format` is skipped and reported.
* **L2 torn tail at every byte offset** (`l2_torn_tail_at_every_byte_offset`).
  * A three-entry file is cut at every byte offset 0..=len.
  * Limits: the reader returns exactly the entries whose JSON text lies inside the cut, equal to those written; at most one skipped
    line; after an append the new entry is readable at the next index and the earlier ones are unchanged.
  * Negative control: an append without the newline guard loses the new entry at one cut or more.
* **L3 kill during append** (`l3_kill_during_append_leaves_a_readable_ledger`, child `l3_kill_child`).
  * A child process (the test binary itself) appends 16 KB entries in a loop; the parent kills it after it has seen at least 200
    entries.
  * Limits: the ledger reads; the entries are contiguous `v0, v1, ...` with intact content; their count is at least the count seen
    before the kill; at most one skipped line; an append afterwards is readable at the next index.
  * A tear cannot be produced on demand by a kill, so L3 has no negative control of its own; L2's naive append is the control of
    the guard. Whether the kill tore the tail is reported.
* **L4 concurrent appends** (`l4_concurrent_appends`).
  * 8 threads x 250 entries (padded to 0-600 extra bytes), the even threads through one handle, the odd through a second handle of
    the same file.
  * Limits: 2000 entries, no skipped line, each thread's entries all present and in its own order.
  * Negative control: two half lines interleaved by hand give 0 entries and 2 skipped lines.
* **L5 queries and export** (`l5_queries_and_export`).
  * 11 points on noise = flow^2 (flow max, noise min), 11 dominated copies (noise + 0.05), one diverged entry with a better flow.
  * Limits: the Pareto set is exactly the 11 curve points; the diverged entry is excluded from best and Pareto; the best-so-far trace
    equals the brute-force running minimum; the history has every entry; the CSV columns carry units and the numbers parse back to the
    same bits.
  * Negative controls: a set with two evaluators is refused; a metric in two units is refused (located).
* **L6 re-run check** (`l6_rerun_check`).
  * Limit: re-running an analytic entry with the same model passes at `rtol = atol = 0` (bit-identical).
  * Negative controls: a model whose flow is off by 1e-6 relative fails at `rtol = 1e-9`; another version is refused; another unit
    and a missing metric are flagged.

### Tolerance driver (`crates/rcfd-api/src/design/uq_tests.rs`)

Fixed-n runs use n = 20 000: MC in 10 batches of 2000, LHS as 20 replicates of 1000.

* **U1 mean and variance against closed forms** (`u1_mean_and_variance_against_closed_forms`; seeds MC 46001, LHS 46002).
  * Linear `y = 3 + 2 x1 - 1.5 x2 + 0.5 x3`, x1 ~ N(1, 0.5^2), x2 ~ N(-2, 0.2^2), x3 ~ N(0, 1): mean 8, variance 1.34.
  * Quadratic `y = x1^2 + 3 x2^2 + 2 x3`, x1 ~ N(1, 0.5^2), x2 ~ N(0, 0.3^2), x3 ~ N(0, 1).
  * Closed forms per term `c X^2 + b X`, X ~ N(mu, s^2), beta = b + 2 c mu: mean `c (mu^2 + s^2) + b mu`, kappa2 `beta^2 s^2 + 2 c^2 s^4`,
    kappa4 `48 c^2 beta^2 s^6 + 48 c^4 s^8` (the cumulants of `a Z + g Z^2`); cumulants add over independent terms. Quadratic: mean 1.52,
    kappa2 5.2708, kappa4 3.4426.
  * Statistics: `z_mean = (ybar - mu) / sqrt(kappa2 / n)`, `z_var = (s^2 - kappa2) / sqrt(kappa4 / n + 2 kappa2^2 / (n - 1))` (the exact
    variance of the sample variance).
  * Limit: |z| <= 4 for 8 comparisons (2 functions x mean/variance x 2 samplers). Family false-failure <= 5.1e-4 (normal
    approximation). For LHS the i.i.d. SE overstates the SE of the mean (LHS variance is at most the MC variance of n - 1 points,
    McKay et al. 1979 / Owen 1997, UNVERIFIED citation detail), so the LHS mean comparisons are conservative; the LHS variance comparison
    uses the i.i.d. formula as an approximation.
  * Negative control: the same inputs mapped by `mu + 1.1 (x - mu) + 0.1 s` (a wrong model of the tolerances). Predicted for the
    linear function: mean +0.12 = 14.7 SE, variance x 1.21 = 21 SE. Limit: |z_mean| > 4 and |z_var| > 4 in all 4 wrong runs.
* **U2 quantiles against exact quantiles** (`u2_quantiles_against_exact_quantiles`; seeds 46003, 46004).
  * The U1 linear output (normal: `q_p = 8 + sqrt(1.34) Phi^-1(p)`) and `y = exp(x)`, x ~ N(0, 0.5^2) (`q_p = exp(0.5 Phi^-1(p))`);
    p = 0.05, 0.5, 0.95; confidence 0.999; both samplers.
  * Limits: reported coverage >= 0.999, and the exact quantile inside `[lo, hi]` for all 12. MC false-failure <= 1e-3 per quantile
    (exact, conservative); LHS approximate.
  * Negative control: the input sd scaled by 1.1. Predicted shift of the linear q_0.95: 0.19 against a half-width of about 0.06. Limit:
    the exact quantile outside the interval at p = 0.05 and 0.95 (8 comparisons; the median does not move).
* **U3 the stated confidence** (`u3_interval_coverage_is_the_stated_confidence`; seed 46005).
  * 2000 replications of n = 100 standard normal samples: the order-statistic interval of the 0.9 quantile at confidence 0.9; and 2000
    replications of 100 Bernoulli(0.05) trials: the Clopper-Pearson interval at 0.9.
  * Limit: empirical coverage >= 0.9 - 4 sqrt(0.09 / 2000) = 0.8732 for both (both intervals are conservative by construction).
  * Negative controls: the rank +-1 interval `[X_(89), X_(91)]` (exact coverage about 0.26) and the Wald interval for p = 0.01
    (coverage at most 1 - 0.99^100 = 0.634): both below 0.8732.
* **U4 SRC recovers the known ranking** (`u4_src_ranking_recovers_the_known_order`; seeds 46006, 46007).
  * `y = x1 + 4 x2 + 10 x3 + 0.5 x1 x2`, x ~ N(0, (2, 0.4, 0.1)^2) independent. With zero means the interaction is uncorrelated with
    every input, so the regression coefficients are (1, 4, 10), s_y^2 = 4 + 2.56 + 1 + 0.16 = 7.72, SRC = (0.7198, 0.5759, 0.3599),
    R^2 = 7.56 / 7.72 = 0.97927.
  * Limits: ranking (x1, x2, x3) for both samplers; max |SRC - exact| <= 0.02 (about 5 SE: the SE of an SRC is about
    sigma_res / (s_y sqrt n) = 0.001 plus 0.0036 from the sd estimate); |R^2 - exact| <= 0.01.
  * Negative control: ranking by the raw coefficients |b| gives (x3, x2, x1) != the truth.
* **U5 stopping rule** (`u5_stopping_rule_on_the_standard_error`; seeds 46008, 46009).
  * MC, linear function, `se_abs = 0.01`, batch 500, max 100 000. Expected n* = 1.34 / 0.01^2 = 13 400.
    Limits: converged; reported SE <= 0.01; n in [0.95 n*, 1.05 n* + 500] = [12 730, 14 570] (s/sigma within +-2 % at n ~ 1e4, plus one
    batch of rounding); |mean - 8| <= 4 x 0.01.
  * LHS, U4 function (the interaction is not reduced by LHS, so about 32 replicates are needed), `se_abs = 0.005`, batch 200,
    min_batches 10. Limits: converged; reported SE <= 0.005; |mean - 0| <= 5 x 0.005 (t with >= 9 degrees of freedom:
    P(|t_9| > 5) is about 7e-4).
  * Negative control: MC with `max_samples = 2000`: limit `converged = false`, n = 2000, reported SE > 0.01 (the driver does not claim
    the target).
* **U6 reproducibility** (`u6_same_seed_gives_a_byte_identical_ledger`; seeds 46010, 46011).
  * U4 function, 2000 samples (batches of 500), one requirement, a fixed clock: the ledger file (2000 entries plus the summary).
  * Limits: byte-identical for 1 and 3 workers with the same seed, for MC and LHS; the ledger range is (0, 2000).
  * Negative control: seed 46011 gives different bytes.
* **U7 input distributions and the normal quantile** (`u7_input_distributions_and_the_normal_quantile`; seed 46012).
  * `norm_inv` at 0.975, 0.95, 0.999, 0.5 against 1.959963984540054, 1.6448536269514722, 3.090232306167813, 0 (standard table values
    quoted from memory, UNVERIFIED in the last digit), and the symmetric points: limit 1e-12.
  * n = 20 000 MC of uniform(2, 5), normal(1, 0.5) truncated to [0.5, 2] (closed-form truncated-normal mean and variance), beta(2, 5).
    Limits: |z_mean| <= 4 (SE from the exact variance) and |z_var| <= 4 (SE from the sample fourth moment); 6 comparisons.
  * Negative control: the beta(2, 5) sample against the beta(2, 4) moments: |z_mean| > 4 (predicted 42).
* **U8 failures and requirements** (`u8_failures_count_as_violations`; seed 46013).
  * x ~ U(0, 1) mm; the evaluator returns `diverged` with a reason for x > 0.9 and panics for x < 0.002; else y = x. Requirement y <= 0.8.
    n = 5000 MC, confidence 0.999. Truth: P(fail) = 0.102, P(violation or fail) = 0.202.
  * Limits: 0.202 inside the Clopper-Pearson interval; the ok fraction within 4 SE of 0.898; the number of diverged ledger entries with a
    non-empty reason equals the failed count.
  * Negative control: the rate that ignores failures (violations / n, about 0.1) lies outside the interval.

### MCP tools (`crates/rcfd-mcp/src/design_tests.rs`)

* **M1 round trip through the tools** (`m1_ledger_round_trip_through_the_tools`).
  * Five ok entries (chord 0.1, 1/3, sqrt 2, 1e-300, 123456.789) and one diverged entry with a null metric, appended with
    `design_ledger_append` into `fan/ledger.jsonl` inside the sandbox.
  * Limits: indices 0-5 returned; `design_ledger_query` returns every value bit for bit with its unit, verdict, tier, settings, a
    stamped solver version and time; verdict filter; Pareto (flow max, noise min) = [0, 1, 2, 3, 4]; best index 4 with a 5-step trace;
    history of 6; the JSON export parses back equal to the query entries; the CSV export of the ok entries has 6 lines;
    `design_ledger_rerun_check` passes with identical metrics at zero tolerance.
  * Negative controls: a metric moved by 1e-12 relative fails at zero tolerance (and passes at rtol 1e-9); an empty unit and a missing
    reason are refused (-32602); after a second evaluator is added, best-so-far without an evaluator filter is refused (-32602), and
    with the filter it returns index 4.
* **T1/T2 (ADR 0067, extended)**: the two write tools are refused in read-only mode and seen by the approval hook, the two read tools
  are not; the four tools return 1004 for paths outside the sandbox.

### Status decision rule

H4 is done when L1-L6 and M1 pass; F1 is partial in any case (library only, analytic evaluators; no fan evaluator, no MCP tool) and
"driver verified" when U1-U8 pass. A miss is an ignored `*_known_miss` test with the limit intact and keeps the part partial.

### Run order

1. Commit this ADR alone.
2. Commit the implementation.
3. Run each registered test once (debug profile; the numbers are profile independent because rcfd uses no fast-math), then the
   gate on a fresh archive.

## Smoke results before registration (rule d; seeds 1-13, not the registered seeds)

* U1 (z_mean, z_var): MC linear (-0.82, -1.03), MC quadratic (-1.43, -2.11), LHS linear (0.0006, -0.07), LHS quadratic (-0.003, -0.99);
  wrong models 13.8-25.5 SE on both statistics. Cumulant check of the quadratic at n = 4e5: z = (-0.68, -0.02).
* U2: all 12 exact quantiles inside, coverage 0.99904-0.99907; after the correction above, all 8 wrong-model tail quantiles outside.
* U3: order statistic 0.948, rank +-1 0.266, Clopper-Pearson 0.930, Wald 0.653.
* U4: ranking (x1, x2, x3) for both samplers, max SRC error 0.0047 (MC) and 0.0009 (LHS), R^2 error 2e-6 and 2.1e-4; raw ranking
  (x3, x2, x1).
* U5: MC n = 14 000, SE 0.00983, |mean error| 0.0048; LHS 31 replicates (6200 samples), SE 0.00496, |mean| 0.0134; capped run
  n = 2000, SE 0.0259, not converged.
* U6: identical bytes for 1 and 3 workers (LHS).
* U7: z within 0.12 for the three distributions; beta(2, 4) control z_mean -37.7.
* U8: p 0.2114 in [0.1927, 0.2309], ok fraction 0.890, 550 failed, rate ignoring failures 0.1014.
* Binomial CDF through `reg_inc_beta` against direct summation (n = 40, p = 0.3): max error 6e-15.
* L3/L4 harness: 51 entries seen and present after the kill, no torn tail; 4 threads x 50 over two handles, all present.
* Run time of the whole U smoke: 21 s in the debug profile.

## Out of scope

* MCP tool for the tolerance driver (needs a named evaluator), Morris and Sobol sensitivity indices, correlated inputs, resuming a
  killed tolerance study, an OS file lock for several writer processes, logging the existing `design.*` study evaluations into the
  ledger automatically, and any fan-specific perturbation (46A) or evaluator (46B).

## Files

* `crates/rcfd-api/src/design/ledger.rs`, `ledger_tests.rs` (new): the ledger, queries, the `Model` trait, the re-run check.
* `crates/rcfd-api/src/design/uq.rs`, `uq_tests.rs` (new): the tolerance driver.
* `crates/rcfd-api/src/design/mod.rs`, `store.rs` (D7), `tools.rs` (four service methods), `crates/rcfd-api/src/service.rs` (method list).
* `crates/rcfd-mcp/src/lib.rs` (four tools), `design_tests.rs` (M1, T1/T2 lists), `tests.rs` (tool count 33).
* `docs/design-loop.md`, `docs/mcp-tools.md`, `docs/service-protocol.md`.

## References

* M. D. McKay, R. J. Beckman, W. J. Conover (1979), A comparison of three methods for selecting values of input variables in the
  analysis of output from a computer code, Technometrics 21, 239-245 (Latin hypercube sampling).
* A. B. Owen (1997), Monte Carlo variance of scrambled net quadrature / LHS variance bound (UNVERIFIED which paper states the n - 1
  bound; used only to say the LHS mean check is conservative).
* C. J. Clopper, E. S. Pearson (1934), The use of confidence or fiducial limits illustrated in the case of the binomial, Biometrika 26,
  404-413.
* Distribution-free quantile intervals from order statistics: first principles (the number of samples below the p-quantile is
  Bin(n, p)); standard in H. A. David, H. N. Nagaraja, Order Statistics (UNVERIFIED edition).
* Standardised regression coefficients: A. Saltelli et al., Global Sensitivity Analysis: The Primer (2008), chapter 1 (UNVERIFIED
  chapter).
* Cumulants of `a Z + g Z^2` and the variance of the sample variance: first principles (chi-square cumulants `k 2^(r-1) (r-1)!`).

## Addendum: measured values (one registered run each; debug profile, `target-w46d`, at 2cf1ae9, rebased to 4843da9; main moved by docs only)

All registered checks passed; no known miss, no amendment.

| Check | Measured | Limit |
|---|---|---|
| L1 | bit-exact round trip of 6 entries (incl. -0.0, 1e-300, NaN metric); 6 located refusals; foreign line skipped | as registered |
| L2 | 1768 cuts: count, content and the appended entry right at every cut, at most 1 skipped line; naive append lost the new entry at 1764 cuts | as registered; control > 0 |
| L3 | 200 entries seen before the kill, 203 contiguous entries after, 0 skipped, no torn tail this run; append afterwards at index 203 | as registered |
| L4 | 2000 entries, 0 skipped, every thread complete and ordered (two handles); interleaved halves: 0 entries, 2 skipped | as registered |
| L5 | Pareto = the 11 curve points; best-so-far = brute force; mixed evaluator and mixed unit refused | as registered |
| L6 | re-run bit-identical (difference 0); 1e-6 perturbation fails at rtol 1e-9; version 2 refused; unit flagged | as registered |
| U1 z (mean, var) | MC linear (-1.617, -0.216), MC quadratic (-1.344, -0.796), LHS linear (+0.006, +0.652), LHS quadratic (+0.008, +0.926) | \|z\| <= 4 |
| U1 control | wrong models 12.9-25.5 SE on both statistics | \|z\| > 4 |
| U2 | 12 of 12 exact quantiles inside the 0.999 intervals, coverage 0.99904-0.99907; wrong model: 8 of 8 tail quantiles outside | as registered |
| U3 | order statistic 0.9275, Clopper-Pearson 0.9420; controls: rank +-1 0.2445, Wald 0.6210 | >= 0.8732; controls < 0.8732 |
| U4 | ranking (x1, x2, x3) both samplers; max SRC error 0.00224 (MC), 0.00249 (LHS); R^2 error 0.00029, 0.00003; raw ranking (x3, x2, x1) | <= 0.02, <= 0.01 |
| U5 MC | n 13 500, SE 0.00992, \|mean error\| 0.0298, converged | n in [12 730, 14 570], SE <= 0.01, <= 0.04 |
| U5 LHS | 29 replicates (5800 samples), SE 0.00495, \|mean\| 0.0058, converged | SE <= 0.005, <= 0.025 |
| U5 control | capped at 2000: SE 0.0261, not converged | as registered |
| U6 | MC 1 180 548 bytes, LHS 1 164 689 bytes: identical for 1 and 3 workers; other seed differs | as registered |
| U7 | norm_inv errors 3.8e-15, 1.1e-15, 8.4e-15, 0; distributions z within 0.73; beta(2, 4) control z_mean -38.4 | 1e-12; \|z\| <= 4; > 4 |
| U8 | p 0.1986 in [0.1804, 0.2177] (contains 0.202); ok fraction 0.9022 (0.98 SE from 0.898); 489 failed entries with reasons; ignoring failures 0.1008 (outside) | as registered |
| M1 | all registered assertions hold | as registered |

Run time of the 47 `design::` library tests: 37.5 s (debug). The U5 MC mean error is 3.0 SE: inside the 4-SE limit, reported because it
is the largest z of the run.

What these numbers are not: they verify the driver and the ledger on analytic functions. They say nothing about the spread of a real
fan's flow, power, noise or vibration: no fan perturbation model (46A) or fan evaluator (46B) is attached, and F1 stays partial.

## Amendment 1 (written after the addendum's runs, before any run of L3b)

L3 has no negative control of its own, and its registered run did not tear the tail, so the kill-then-recover path of a torn line was
exercised only by L2's synthetic cuts. Added, without changing any registered limit:

* **Harness:** the L3 child is killed and reaped by a drop guard also when the parent fails an assertion, and the child loop stops
  after 20 000 entries (before, a failed parent could leave an endless writer).
* **L3b kill inside a line** (`l3b_kill_inside_a_line`). The same child with `RCFD_W46D_TEAR_CHILD` set writes each line in two
  raw halves 100 ms apart (it simulates a writer whose write is cut, not `Ledger::append`); the parent kills it once at least 10 entries
  exist and the last byte is not a newline.
  * Limits: after the kill the last byte is not a newline; the reader returns exactly the complete lines, contiguous, with exactly one
    skipped line; a guarded `Ledger::append` is readable at the next index (one skipped line remains).
  * Negative control: a naive append (no newline guard) to the same killed file is lost (the entry count stays).
* Smoke before registration (`smoke_w46d_ledger`, at least 3 entries): 59 361 bytes, 3 complete lines, 3 entries, 1 skipped line, torn tail.
* Measured (registered run, at 0661647): torn tail after the kill, 10 complete lines, 10 entries, 1 skipped line; guarded append
  readable at index 10; the naive append lost the entry. L3 re-run with the guard: 200 seen, 202 after, no tear. Pass.
