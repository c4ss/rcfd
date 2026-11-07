# ADR 0220: F-297 registration: a per-variable exchange of the children's values in NSGA-II, the operator read against the papers, and the 30-parameter ZDT1 check again

Status: **REGISTERED 2026-11-07**, written before any code of F-297 and before any run of its checks. Not yet accepted by the
maintainer. Before this text no rcfd code or test was run for F-297; four papers were read (the reading section below). Results
go in the addendum at the end.

Parents: [ADR 0209](0209-f249-registration-nsga2-studies-nelder-mead-options-and-extended-warm-start.md) (F-249: NSGA-II, its
operator D1, the checks MO1 to MO4, the known miss MO1z, the diagnostic DG1 and question 5), [ADR 0067](0067-wave32c-design-loop.md)
(the design loop, the store, propose / record). Roadmap item F-297 (depends on F-249, merged).

Number: the lead gave 0220 to F-297. Branch `f297-nsga2-crossover` from main 36466d0, worktree `w49i-work/f297`.

House rules (ADR 0130, as in ADR 0209 and ADR 0216):

- limits are fixed here, before the code and the runs, and are not changed after a run;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact and the measured number recorded, never tuned;
- a defective set-up gets a new corrected test by an amendment written before the corrected run, and the old one stays as an
  ignored record;
- every new path is opt-in and the defaults stay bit-identical: `crates/rcfd-api/tests/baseline_hashes.rs` and
  `case_hashes_w48u.rs` are not edited or re-recorded; no golden is edited or re-recorded (the tools-list goldens of
  `crates/rcfd-mcp/tests/golden/`); no case-schema key is added; no existing test file is edited (the ignored ADR 0209 known miss
  `mo1z_nsga2_on_zdt1_meets_the_analytic_front_known_miss` stays as it is);
- each registered run is made once, and nothing is re-run to pick a result;
- no new third-party crate, `Cargo.lock` unchanged, no commit trailers.

Clean-room and sources: the operator is taken from the four papers listed below, read for this text. No source listing of any
optimisation code was opened: the web searches for the papers also listed pages of implementation source code (of other
NSGA-II and SBX implementations); none of them was opened. `docs/reference-data.md` records what was read and what was not
(documentation commit).

## What F-297 asks

The roadmap entry: a new registration ADR re-registers MO1z (ZDT1 with 30 parameters, C within its limit) with the per-variable
coordinate exchange between parents added to NSGA-II (ADR 0209 diagnostic DG1: C about 1.5e-3 with it), after the operator is
read from Deb 2002 and the crossover and mutation papers; the MO1z known miss is then met or recorded again with its numbers;
defaults bit-identical. From ADR 0209 question 5, recommendation (b) taken by the lead.

## What the code read shows (main 36466d0, read only)

1. **`rcfd_api::design::nsga2`** (`crates/rcfd-api/src/design/nsga2.rs`): `Options { max_evals, pop, seed, x0 }` (no `Default`),
   `run(bounds, &Options, &mut MoEval)`, the pure helpers `sbx_beta`, `poly_delta`, `select`. Step 3 of ADR 0209 D1 is inline in
   `run`: two tournaments, `rng.uniform() < P_CROSSOVER`, then for every variable `beta = sbx_beta(rng.uniform(), 20)` and the
   children `0.5((1 + beta) p1 + (1 - beta) p2)`, `0.5((1 - beta) p1 + (1 + beta) p2)`, then per child per variable `rng.uniform()
   < 1/d` and `x += poly_delta(rng.uniform(), 20) (hi - lo)`, then the clamp.
2. **Literal constructions**: `nsga2::Options { .. }` is built field by field in `crates/rcfd-api/tests/f249_moo.rs` (MO2b, MO2f)
   and in the module's own unit tests; `Method::Nsga2 { budget, pop, seed }` in `f249_moo.rs` (MO1z, MO1f, `small`). A field added
   to either breaks those files, which may not be edited.
3. **`design::study`**: `Method::Nsga2` runs `nsga2::run` with every record mapped to `MoFitness` (`objective_vector`,
   `violation_of`, a record without objectives the failed fitness) and returns the member of the last population that is best by
   the weighted scalar. Nothing outside `design/` matches on `Method`.
4. **`design::tools`**: `parse_method` builds `Method::Nsga2` from `{type: "nsga2", budget, pop, seed}` (budget and pop required);
   `method_json` writes exactly `{type, budget, pop, seed}`; other keys of `method` are ignored except `initial_simplex` and
   `coefficients`, which only `nelder_mead` takes. `crates/rcfd-mcp/tests/design_f249.rs` N6 asserts the keys of a default `nsga2`
   study's `study.json` (`budget, pop, seed, type`). The MCP descriptor of `design_create_study` gives `method` as a free object, so
   a new key needs no schema or description change (the goldens stay byte-identical).
5. **The known miss**: `f249_moo.rs` `mo1z_nsga2_on_zdt1_meets_the_analytic_front_known_miss` (ignored), ADR 0209 amendment 1:
   C 0.543 / 0.378 / 0.353, G 0.288 / 0.207 / 0.291, I 0.254 / 0.141 / 0.220 for seeds 1 to 3. DG1 (ADR 0209 addendum 1, scratch,
   code not kept): with the two children's values exchanged per variable with probability 1/2 after SBX (one more
   `rng.uniform()` per variable, drawn after that variable's `u`), C 1.496e-3 / 1.323e-3 / 1.594e-3, G 1.284e-3 / 1.376e-3 /
   1.431e-3, I 4.081e-4 / 3.822e-4 / 4.322e-4 (2460, 2499, 2400 Pareto members). The comparison DG1 used for "probability 1/2"
   (`< 0.5`, `<= 0.5` or `>= 0.5`) is not recorded.

## The reading (done before this text)

| Id | Paper | Copy read | What it states (quoted where it matters) |
|---|---|---|---|
| R1 | K. Deb, A. Pratap, S. Agarwal, T. Meyarivan, "A fast and elitist multiobjective genetic algorithm: NSGA-II", IEEE Trans. Evol. Comput. 6 (2002) 182-197 | `cse.unr.edu/~sushil/class/gas/papers/nsga2.pdf` (the journal pages) | Section IV-A: "the simulated binary crossover (SBX) operator and polynomial mutation [6] for real-coded GAs. The crossover probability of pc = 0.9 and a mutation probability of pm = 1/n ... are used. For real-coded NSGA-II, we use distribution indexes [6] for crossover and mutation operators as ηc = 20 and ηm = 20"; 25 000 evaluations, population 100, 250 generations; [6] is Deb and Agrawal (1995). Table I: ZDT1 (n = 30, [0, 1], `f1 = x1`, `f2 = g [1 - sqrt(x1/g)]`, `g = 1 + 9 (sum_{i=2}^n x_i)/(n - 1)`) and FON (n = 3, [-4, 4], `1 - exp(-sum (x_i -+ 1/sqrt 3)^2)`). Section III-C: binary tournament on the crowded comparison (rank, then crowding distance), fronts filled in order, the last by descending crowding distance. Section on the rotated problem: "a generic search operator, such as the variablewise SBX operator used here". Table II (10 runs): the convergence metric of real-coded NSGA-II on ZDT1 is 0.033482 (mean), variance 0.004750; on FON 0.001931 |
| R2 | K. Deb, R. B. Agrawal, "Simulated binary crossover for continuous search space", Complex Systems 9 (1995) 115-148 | the publisher's PDF, `content.wolfram.com/sites/13/2018/02/09-2-2.pdf` | Eq. (6): the spread factor `beta = |(c1 - c2)/(p1 - p2)|`, the children equidistant from the parents' mid-point; eqs. (19), (20): `C(beta) = 0.5 (n + 1) beta^n` for `beta <= 1`, `0.5 (n + 1) / beta^(n + 2)` for `beta > 1`. Section 3.1, multi-variable problems: a single-point crossover in the k-th variable "creates a new value of the kth variable and swaps the (k + 1)st to Nth variables between the parents"; "we use a mechanism similar to uniform crossover for multiple variables ... for choosing which variables to cross ... we choose to perform SBX in each variable with probability 0.5. But in the case of single-variable functions ... with probability 1.0". Children may lie anywhere in the real line; section 6 suggests a bounded distribution as an extension; mutation is not used in the paper |
| R3 | K. Deb, S. Agrawal, "A niched-penalty approach for constraint handling in genetic algorithms", ICANNGA 1999 | `homes.cs.washington.edu/~sagarwal/icannga.pdf` | Appendix A, SBX: `u` in [0, 1]; `beta_q = (u alpha)^(1/(eta_c + 1))` if `u <= 1/alpha`, else `(1/(2 - u alpha))^(1/(eta_c + 1))`, `alpha = 2 - beta^-(eta_c + 1)`, `beta = 1 + 2 min[(y1 - y_l), (y_u - y2)]/(y2 - y1)`; `c1 = 0.5 [(y1 + y2) - beta_q |y2 - y1|]`, `c2 = 0.5 [(y1 + y2) + beta_q |y2 - y1|]`; "It is assumed here that y1 < y2. A simple modification to the above equation can be made for y1 > y2. For handling multiple variables, each variable is chosen with a probability 0.5 and the above SBX operator is applied variable-by-variable." Polynomial mutation, citing Deb and Goyal (1996): eq. (12) `delta_q = [2u + (1 - 2u)(1 - delta)^(eta_m + 1)]^(1/(eta_m + 1)) - 1` for `u <= 0.5`, else `1 - [2(1 - u) + 2(u - 0.5)(1 - delta)^(eta_m + 1)]^(1/(eta_m + 1))`, `delta = min[(y - y_l), (y_u - y)]/(y_u - y_l)`, `c = y + delta_q (y_u - y_l)` |
| R4 | K. Deb, D. Deb, "Analyzing mutation schemes for real-parameter genetic algorithms", KanGAL report 2012016 (published in Int. J. Artif. Intell. Soft Comput. 4 (2014) 1-28) | `egr.msu.edu/~kdeb/papers/k2012016.pdf` | Section 2: polynomial mutation, attributed to Deb and Agrawal (R3): `p' = p + delta_L (p - x_L)` for `u <= 0.5`, `p + delta_R (x_U - p)` for `u > 0.5`, `delta_L = (2u)^(1/(1 + eta_m)) - 1`, `delta_R = 1 - (2(1 - u))^(1/(1 + eta_m))` |

Not obtained: K. Deb, M. Goyal, "A combined genetic adaptive search (GeneAS) for engineering design", Computer Science and
Informatics 26 (1996) 30-45 (no open copy found; the IAS repository record returned 404). Not read: Deb's 2001 book.

**Findings of the reading** (they set the decisions below; no rcfd number was used):

- **F1.** The constants of ADR 0209 D1 (crossover probability 0.9, both indices 20, mutation probability 1/d, population 100,
  25 000 evaluations) and the ZDT1 and FON definitions of ADR 0209 are those of R1.
- **F2.** ADR 0209's SBX is R2's distribution sampled by inversion, and equals R3's procedure without the bound term (`alpha = 2`,
  the limit of unbounded variables): `beta_q = (2u)^(1/(eta + 1))` for `u <= 1/2`, `(1/(2 - 2u))^(1/(eta + 1))` otherwise; and for
  `y1 < y2`, `0.5 [(y1 + y2) - beta_q (y2 - y1)] = 0.5 [(1 + beta_q) y1 + (1 - beta_q) y2]`, ADR 0209's first child. ADR 0209's
  polynomial mutation is R3's eq. (12) without the bound term (`delta = 1`). The comparisons `u < 0.5` (ADR 0209) and `u <= 0.5`
  (R3, R4) differ only at `u = 0.5`.
- **F3.** What ADR 0209 does differently from what was read: (a) every variable of a crossed pair is crossed, while R2 and R3
  cross each variable with probability 0.5; (b) children and mutants are clamped into the box, while R3 bounds the SBX and the
  mutation distributions (R2's children are unbounded) and R4 scales the mutation by the distance to the bound on the side of the
  step.
- **F4.** Neither R2 nor R3 states an exchange of the two children's values between the children. R3 gives `c1` the lower and
  `c2` the higher value for `y1 < y2` and leaves the case `y1 > y2` to "a simple modification", so which child receives which
  value is not fixed by the text; ADR 0209 keeps the first child next to the first parent in every variable. R2 motivates its
  variable-wise rule by single-point crossover, which swaps the variables beyond the cross site between the parents, and by
  uniform crossover. So the per-variable exchange of DG1 is not stated in the papers read; it is the lead's operator (from DG1),
  and its link to the papers is that analogy.
- **F5.** R1's own result on ZDT1 (mean convergence metric 0.0335 over 10 runs; that metric is the mean distance of the final
  population to 500 front points, not the metric C below) does not predict that its operator meets C <= 1e-2.

## Decisions (fixed now)

**D1. The operator `Crossover::Exchange`, opt-in.** In `rcfd_api::design::nsga2`:

```rust
pub enum Crossover { Sbx, Exchange }   // Default: Sbx
```

* `Sbx` is ADR 0209 D1 step 3, unchanged.
* `Exchange`: as `Sbx` (the pair is crossed when one `rng.uniform()` is below 0.9; otherwise both children are copies and no
  draw of this step is made), and in a crossed pair, for each variable j in order: `u_j = rng.uniform()`, `beta = sbx_beta(u_j,
  20)`, the two values as ADR 0209, then `w_j = rng.uniform()`; when `w_j < 0.5` the two children's values of variable j are
  exchanged. Then the mutation and the clamp of ADR 0209, child 1 first. Draw order of one pair: the two tournaments, the
  crossover coin, `u_1, w_1, ..., u_d, w_d` (crossed pairs only), child 1's mutation draws, child 2's.
* Why this rule and not the papers' variable-wise probability 0.5: the roadmap item asks for DG1's exchange; F4 shows that the
  papers do not state an exchange, and with ADR 0209's order of the children, crossing each variable with probability 0.5 alone
  would still keep each child next to its own parent in every variable. The papers' rule is measured by the diagnostics DP1 and
  DP2 (recorded, not adopted; question 2). The bounded forms of F3 (b) are not adopted (question 4).
* `w_j < 0.5` is fixed now; DG1's comparison is not recorded, so X1 equals DG1's numbers only if DG1 used the same comparison
  and draw position (recorded, not required).

**D2. The library API.** `nsga2::Options` and `nsga2::run` are unchanged (item 2); `run(b, o, e)` is `run_with(b, o,
Crossover::Sbx, e)`. New: `nsga2::run_with(bounds, &Options, Crossover, &mut MoEval)` and the pure helper
`nsga2::vary_pair(p1, p2, bounds, Crossover, &mut Rng) -> (Vec<f64>, Vec<f64>)`, step 3 after the two tournaments (coin,
crossover, mutation, clamp), public for the replica check X4a; `run_with` calls it. On the `Sbx` path the arithmetic and the draws
are those of ADR 0209 in the same order (V0).

**D3. The study.** A new variant `Method::Nsga2With { budget, pop, seed, crossover: Crossover }` (as `NelderMeadWith` in ADR
0209: a field added to `Method::Nsga2` would break `f249_moo.rs`). `Method::Nsga2 { budget, pop, seed }` runs exactly as
`Nsga2With { .., crossover: Sbx }` (one code path).

**D4. The tools.** `method.crossover` for `nsga2`: `"sbx"` or `"exchange"`. Without the key the method is `Method::Nsga2` and
`method_json` writes `{type, budget, pop, seed}` as now; with it, `Method::Nsga2With` and `method_json` writes `{type, budget,
pop, seed, crossover}`. Refusals (-32602): another value, `method.crossover: '<v>' is not sbx or exchange`; the key on another
method type, `method.crossover: only nsga2 takes this key` (as ADR 0209 does for `initial_simplex`; until now such a key was
ignored: no `study.json` written by `method_json` holds it). The MCP descriptor and its description are unchanged (item 4;
question 5).

**D5. What does not change.** The default NSGA-II path (library, study and tools), its proposals, resume and `study.json`; every
other method; no case-schema key, case hash, baseline, golden or existing test file.

**D6. Commits.** (1) this text; (2) the V0 record (test only, run on main 36466d0's code); (3) the library and study code with
`f297_moo.rs`; (4) the tools with `design_f297.rs`; (5) documents (`docs/design-loop.md`, `docs/reference-data.md`, roadmap);
(6) the addendum.

## Checks and limits (fixed now)

The problems and metrics are those of ADR 0209 (ZDT1 d = 30 in [0, 1]^30, FON d = 3 in [-4, 4]^3; `D(f)`, C, G, I), with the
problem and metric code of `f249_moo.rs` copied unchanged into the new test file. Limits: **C <= 1e-2, G <= 5e-2, I <= 1e-2**
(ADR 0209), S non-empty.

### Library checks (`crates/rcfd-api/tests/f297_moo.rs`)

| Id | Check | Limit |
|---|---|---|
| X1 | **MO1z with the exchange** (the acceptance): ZDT1 d = 30 through `Study` (in-memory store, `workers` 1), `Method::Nsga2With { budget: 25000, pop: 100, seed, crossover: Exchange }`, seeds 1, 2, 3; no point outside the box | C, G and I for every seed |
| X1n | negative control and identity: the same set-up with `crossover: Sbx`, seed 1: its records (index, key, params, every metric, bit for bit) equal those of `Method::Nsga2 { 25000, 100, 1 }`, and its C fails | records equal; C > 1e-2 |
| X2 | FON through `Study`, `Nsga2With { budget: 10000, pop: 100, seed, crossover: Exchange }`, seeds 1, 2, 3 (the problem ADR 0209 MO1f met) | C, G and I for every seed |
| X3a | determinism: ZDT1 d = 30, pop 20, budget 400, `Exchange`: two runs with seed 4 give equal records; `workers` 1 and 4 give equal records; seeds 4 and 5 differ; `Exchange` and `Sbx` with seed 4 differ | exact |
| X3b | box and budget: no point any X1 to X3 run asked for lies outside the box (exact comparison); `run_with(.., Exchange, ..)` asks for `pop floor(budget/pop)` points for budgets 400 and 410 with pop 20 (400, 20 generations) | exact |
| X3c | propose / record: the X3a study (seed 4) driven by `Study::propose` and `Study::record` gives the records of the evaluator-driven run (index, params, metrics) | exact |
| X3d | resume: the X3a study stopped after k = 37 new evaluations (allowance) and resumed from a file store gives the records of an uninterrupted run; the resume evaluates `total - 37` | exact |
| X4a | the replica: for seeds s = 1 to 20, parents drawn uniform in [-1, 2]^5 from `Rng::new(1000 + s)`, `vary_pair(p1, p2, b, Exchange, Rng::new(s))` equals the test's own replica of D1 (draws from `Rng::new(s)` in the order of D1, the formulas of ADR 0209 written out again, the exchange when `w < 0.5`, mutation, clamp), bit for bit, and the same for `Sbx` (no `w` draws); over the 20 seeds the replica must have made at least one exchange, one crossed variable without exchange, and one pair that was not crossed (set-up) | bit for bit; the counts non-zero |
| X4b | `run` and `run_with(.., Sbx, ..)` on ZDT1 d = 30, pop 20, budget 400, seed 4, `x0` = 0.5 in every variable: the requested points, the values and the `MoResult` (population, ranks, evaluations, generations) are bit-identical | exact |

### Tool checks (`crates/rcfd-mcp/tests/design_f297.rs`, the MCP server in process as `design_f249.rs`)

| Id | Check | Limit |
|---|---|---|
| T1 | an `nsga2` study with `"crossover": "exchange"` on ZDT1 d = 2 (the MO4 set-up of ADR 0209: dummy targets, `budget` 400, `pop` 20, seed 1) driven by propose / record until `complete`; a second study without the key gets the same first batch and the same records of it | `complete` reached; `design_list_study.pareto` equals `pareto::front` recomputed from `evaluations.jsonl` (exact); `study.json` `method` has exactly the keys `budget, crossover, pop, seed, type` with `crossover` `"exchange"`; the first batches are equal and the second batches differ; max D of the Pareto set recorded |
| T2 | refusals (-32602, the message names `method.crossover`, nothing written): `nsga2` with `"crossover": "uniform"`, with `"crossover": 1`; `nelder_mead` with `"crossover": "exchange"`; `lhs` with `"crossover": "sbx"`. Accepted: `nsga2` with `"crossover": "sbx"`, whose `study.json` `method` has the keys `budget, crossover, pop, seed, type` with `"sbx"`, and whose first two proposed batches (the first recorded in both) equal those of the same study without the key | as stated |

### Default bits (V0)

| Id | Check | Limit |
|---|---|---|
| V0a | `baseline_hashes.rs` and `case_hashes_w48u.rs` unedited; they pass in release | pass |
| V0b | `crates/rcfd-api/tests/f297_default_bits.rs`, recorded on main 36466d0 with this text and the test only, committed before any code: FNV-1a (as `f249_default_bits.rs`) of (1) `nsga2::run` on ZDT1 d = 30, pop 20, budget 400, seed 4, `x0` = 0.5 (every requested point and value, then the `MoResult`); (2) `nsga2::run` with constraints and failures: ZDT1 d = 5, pop 20, budget 200, seed 7, violation `max(0, 0.3 - x_1) + max(0, x_2 - 0.6)`, every point with `x_3 > 0.9` failed; (3) the records of the `Study` runs `Method::Nsga2 { 25000, 100, 1 }` on ZDT1 d = 30 and `{ 10000, 100, 2 }` on FON; (4) the proposals of the `Study` ZDT1 d = 30, pop 20, budget 400, seed 4, driven by propose / record (every batch) | bit-identical after the code |
| V0c | the tools-list goldens unedited: `default_f1.rs`, `project_list_f270.rs`, `tool_list_f275.rs` pass; `design_f249.rs` (N6: the keys of a default `nsga2` `study.json`; MO4) passes | pass |
| V0d | every existing test passes unedited; the ignored ADR 0209 MO1z known miss stays ignored and unedited | pass |
| V0e | the gate (fmt per package, clippy `--workspace --all-targets -D warnings`, every test in release on a git-archive copy, `scripts/gate_test.py`, 3 jobs, under the machine gate lock, the free memory at the start disclosed) on the final commit; a known flaky test that fails is run again alone on the gate's build | pass, apart from the known flakes |

### Mutations (scratch, not committed)

Each is applied alone to the final tree, the named test is run once in release, and it must fail; the file is restored and
compared with the commit.

| Id | Mutation | Must fail |
|---|---|---|
| MX1 | `Exchange` draws every `w_j` but never exchanges | X1 (seed 1, C) |
| MX2 | the exchange when `w_j >= 0.5` | X4a |
| MX3 | `w_j` drawn before `u_j` | X4a |
| MX4 | `Study::run` gives `Nsga2With { Exchange }` the `Sbx` operator | X1 (seed 1, C) |
| MX5 | the tools parse `"exchange"` as `Sbx` | T1 |
| MX6 | `run` calls `run_with(.., Exchange, ..)` (a changed default) | V0b |
| MX7 | `method_json` of `Method::Nsga2` also writes `"crossover": "sbx"` | V0c (`design_f249` N6) |

### Diagnostics (scratch, not committed, recorded only; registered now so that they are not chosen after a result)

Run once each on the final tree after the checks, numbers in the addendum; they are not checks and nothing is adopted from them:

* **DP1**, the papers' rule without an exchange (R2, R3; ADR 0209's children order): in a crossed pair each variable is crossed
  with probability 0.5 (one draw `v_j < 0.5`, then `u_j` for a crossed variable), the others copied. MO1z (X1's set-up), seeds 1
  to 3: C, G, I.
* **DP2**, the papers' rule with the exchange: as DP1, and a crossed variable's two values exchanged when `w_j < 0.5` (drawn
  after `u_j`). Seeds 1 to 3.
* **DL**, R3's literal order: every variable crossed, the first child always receives the lower of the two values. MO1z seed 1
  and FON (X2's set-up) seed 1.

### Classes and the decision rule

V0 is a precondition of landing: a miss of V0a, V0b or V0c means the change is not merged and the cause is recorded. X1n and
the X4a counts are set-up checks: if one does not hold, or a mutation does not make its test fail, the check is not shown to be
able to fail; that is a defective set-up, corrected by an amendment before any re-run, never counted as a pass. A miss of any
other limit is reported with its number and kept as an ignored `*_known_miss` test with the limit intact.

**Decision rule.** X1 is the acceptance's check ("C within its limit", with G and I as in ADR 0209). F-297 is **done** when X1
and every other registered check are met, X1n and the X4a counts hold and every mutation fails its test; the reading clause is
then taken as met by R1 to R4, with Deb and Goyal (1996) not obtained (question 1). A miss of X1 (on any seed, any of C, G, I):
**partial**, with the numbers recorded again as the acceptance allows. A miss of another check with X1 met: partial, the miss
named.

## Predictions (mine)

| Id | Prediction |
|---|---|
| P1 | X1 met on all three seeds with C 1.3e-3 to 1.6e-3, G about 1.3e-3, I about 4e-4 (DG1), probably equal to DG1's numbers (moderate confidence that DG1 used `< 0.5` after `u_j`) |
| P2 | X1n: C 0.543 on seed 1 (ADR 0209's value; the same path) |
| P3 | X2 met, C between 2e-3 and 6e-3 (moderate confidence, about 3 in 4; ADR 0209 measured 3.5e-3 to 4.4e-3 without the exchange) |
| P4 | DP1 misses C (above 0.1); DP2 meets C; DL gives a smaller C than X1 on ZDT1 (its first child collects the lower values, and ZDT1's optimum lies on the lower bound in x_2 to x_30) |

## Budgets

In release: X1 about 10 s, X2 about 6 s, X1n about 6 s, the rest under 3 s; V0b about 10 s; the MCP tests under 30 s.

## Not in scope

The papers' variable-wise probability 0.5, the bounded SBX and the bounded mutation as options (questions 2 and 4); a change of
the default operator (question 3); the tool description (question 5); the other ADR 0209 questions (N3, the warm start).

## Open questions for the maintainer (each with a recommendation)

1. **Done or partial (the reading clause).** The polynomial mutation was read in R3 (whose appendix states it and which R4
   credits for it) and R4, not in Deb and Goyal (1996), which was not obtained. Options: (a) R3 and R4 count as the mutation paper;
   (b) partial until Deb and Goyal (1996) is read. Recommendation: (a); the item's change is the crossover, and the mutation is
   unchanged.
2. **The papers' variable-wise rule.** R2 and R3 cross each variable with probability 0.5; the built `Exchange` crosses every
   variable and exchanges with probability 1/2 (DG1). Options: (a) as built; (b) a further option with the papers' rule (DP1 or
   DP2); (c) replace `Exchange` by DP2 before any study uses it. Recommendation: written in the addendum from DP1 and DP2, with
   (a) unless DP2 is clearly better on X1 and X2.
3. **The default operator.** Options: (a) `sbx` stays the default (as built: every existing `nsga2` study replays and resumes as
   before); (b) `create_study` writes `"crossover": "exchange"` into new `nsga2` studies (a study without the key keeps `sbx`, so
   existing studies replay unchanged), which changes the tools' default output and needs the bit-identity rule lifted for it.
   Recommendation: (b) if X1 is met, since the default misses ZDT1 by a factor of 35 to 54; the maintainer's decision.
4. **Bounds.** ADR 0209 clamps children and mutants; R3 bounds the SBX and mutation distributions and R4 scales the mutation to
   the bound on the side of the step. Options: (a) keep the clamp; (b) a follow-up registration of the bounded forms as options.
   Recommendation: (a) unless a design study shows members piling up on a bound that is not optimal.
5. **The tool description.** `design_create_study`'s description does not name `nsga2` or its keys (ADR 0209 question 6, still
   open); `crossover` is documented in `docs/design-loop.md` and named by its error messages. Recommendation as ADR 0209 Q6 (b): a
   registered edit of the description and both goldens in a separate change.

## Addenda

Results follow in addenda.
